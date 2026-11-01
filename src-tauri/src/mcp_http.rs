//! Transporte remoto MCP Streamable HTTP (revisão 2026-07-28).

use base64::{engine::general_purpose::STANDARD, Engine};
use reqwest::{
    blocking::Client,
    header::{self, HeaderValue},
    redirect::Policy,
    StatusCode,
};
use serde_json::{json, Value};
use std::{
    io::Read,
    sync::{atomic::Ordering, Mutex},
    time::Duration,
};
use url::Url;

const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
pub const PROTOCOL_VERSION: &str = "2026-07-28";

pub fn validate_endpoint(raw: &str) -> Result<Url, String> {
    let url =
        Url::parse(raw).map_err(|_| "Informe o endereço HTTPS do servidor MCP.".to_string())?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.host(), Some(url::Host::Domain(_)))
        || url
            .host_str()
            .is_some_and(|host| host.eq_ignore_ascii_case("localhost"))
    {
        return Err(
            "O servidor MCP remoto precisa usar HTTPS e um domínio fixo, sem credenciais.".into(),
        );
    }
    Ok(url)
}

pub struct McpHttpClient {
    endpoint: Url,
    auth: AuthMode,
    transport: Box<dyn HttpTransport>,
    next_id: std::sync::atomic::AtomicU64,
}

enum AuthMode {
    Personal(String),
    OAuth {
        server_id: String,
        access: Mutex<Option<crate::mcp_oauth::OAuthAccessToken>>,
    },
}

struct HttpReply {
    status: StatusCode,
    content_type: String,
    final_url: Url,
    body: Vec<u8>,
}

trait HttpTransport: Send + Sync {
    fn post(
        &self,
        endpoint: &Url,
        token: &str,
        method: &str,
        name: Option<&str>,
        body: Value,
    ) -> Result<HttpReply, String>;
}

struct ReqwestTransport(Client);

impl HttpTransport for ReqwestTransport {
    fn post(
        &self,
        endpoint: &Url,
        token: &str,
        method: &str,
        name: Option<&str>,
        body: Value,
    ) -> Result<HttpReply, String> {
        let mut request = self
            .0
            .post(endpoint.clone())
            .bearer_auth(token)
            .header(header::ACCEPT, "application/json, text/event-stream")
            .header("MCP-Protocol-Version", PROTOCOL_VERSION)
            .header("Mcp-Method", method)
            .json(&body);
        if let Some(name) = name {
            request = request.header("Mcp-Name", encode_header_value(name)?);
        }

        let response = request
            .send()
            .map_err(|_| "Falha de rede ao acessar o servidor MCP.".to_string())?;
        let status = response.status();
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_ascii_lowercase();
        let final_url = response.url().clone();
        let mut bytes = Vec::new();
        response
            .take((MAX_RESPONSE_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| "Não foi possível ler a resposta MCP.".to_string())?;

        Ok(HttpReply {
            status,
            content_type,
            final_url,
            body: bytes,
        })
    }
}

impl McpHttpClient {
    pub fn connect(endpoint: &str, token: String) -> Result<Self, String> {
        let endpoint = validate_endpoint(endpoint)?;
        if token.is_empty() || token.len() > 32_768 || token.chars().any(char::is_control) {
            return Err("Cadastre um token de acesso válido para o servidor MCP.".into());
        }
        let client = Client::builder()
            .redirect(Policy::none())
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|_| "Não foi possível preparar a conexão segura MCP.".to_string())?;
        Ok(Self {
            endpoint,
            auth: AuthMode::Personal(token),
            transport: Box::new(ReqwestTransport(client)),
            next_id: std::sync::atomic::AtomicU64::new(1),
        })
    }

    pub fn connect_oauth(
        endpoint: &str,
        server_id: &str,
        initial_access: Option<crate::mcp_oauth::OAuthAccessToken>,
    ) -> Result<Self, String> {
        let endpoint = validate_endpoint(endpoint)?;
        let stored = crate::mcp_oauth::load(server_id)?
            .ok_or("Reconecte a autorização OAuth do servidor MCP.")?;
        if stored.resource != endpoint.as_str() {
            return Err(
                "A autorização OAuth foi emitida para outro servidor MCP; conecte novamente."
                    .into(),
            );
        }
        let client = Client::builder()
            .redirect(Policy::none())
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|_| "Não foi possível preparar a conexão segura MCP.".to_string())?;
        Ok(Self {
            endpoint,
            auth: AuthMode::OAuth {
                server_id: server_id.to_owned(),
                access: Mutex::new(initial_access),
            },
            transport: Box::new(ReqwestTransport(client)),
            next_id: std::sync::atomic::AtomicU64::new(1),
        })
    }

    pub fn is_alive(&self) -> bool {
        true
    }

    #[cfg(test)]
    fn with_transport(endpoint: &str, token: &str, transport: Box<dyn HttpTransport>) -> Self {
        Self {
            endpoint: validate_endpoint(endpoint).unwrap(),
            auth: AuthMode::Personal(token.to_owned()),
            transport,
            next_id: std::sync::atomic::AtomicU64::new(1),
        }
    }

    pub fn tools_list(&self) -> Result<Value, String> {
        self.request("tools/list", json!({ "_meta": metadata() }))
    }

    pub fn tools_call(&self, name: &str, arguments: Value) -> Result<Value, String> {
        self.request(
            "tools/call",
            json!({ "name": name, "arguments": arguments, "_meta": metadata() }),
        )
    }

    fn request(&self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let body = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });
        let token = self.bearer_token(false)?;
        let mut response = self.transport.post(
            &self.endpoint,
            &token,
            method,
            name.as_deref(),
            body.clone(),
        )?;
        if response.status == StatusCode::UNAUTHORIZED
            && matches!(&self.auth, AuthMode::OAuth { .. })
        {
            let renewed = self.bearer_token(true)?;
            response =
                self.transport
                    .post(&self.endpoint, &renewed, method, name.as_deref(), body)?;
        }
        if !same_origin(&self.endpoint, &response.final_url) {
            return Err("O servidor MCP respondeu por um host diferente do cadastrado.".into());
        }
        parse_response(response.status, &response.content_type, &response.body)
    }

    fn bearer_token(&self, force_refresh: bool) -> Result<String, String> {
        match &self.auth {
            AuthMode::Personal(token) => Ok(token.clone()),
            AuthMode::OAuth { server_id, access } => {
                let mut access = access
                    .lock()
                    .map_err(|_| "Estado OAuth MCP indisponível.".to_string())?;
                if force_refresh
                    || access.as_ref().is_none_or(|token| {
                        crate::mcp_oauth::should_refresh(
                            token.expires_at,
                            std::time::Instant::now(),
                        )
                    })
                {
                    *access = Some(crate::mcp_oauth::refresh_access(server_id)?);
                }
                access
                    .as_ref()
                    .map(|token| token.value.clone())
                    .ok_or_else(|| "Reconecte a autorização OAuth do servidor MCP.".into())
            }
        }
    }
}

fn same_origin(expected: &Url, actual: &Url) -> bool {
    expected.scheme() == actual.scheme()
        && expected.host_str() == actual.host_str()
        && expected.port_or_known_default() == actual.port_or_known_default()
}

fn parse_response(status: StatusCode, content_type: &str, bytes: &[u8]) -> Result<Value, String> {
    if status.is_redirection() {
        return Err(
            "O servidor MCP tentou redirecionar a conexão; redirecionamentos são recusados.".into(),
        );
    }
    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        return Err(
            "O token foi recusado. Confira se ele tem as permissões e se não expirou.".into(),
        );
    }
    if !status.is_success() {
        return Err(format!("O servidor MCP retornou HTTP {}.", status.as_u16()));
    }
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err("A resposta do servidor MCP excedeu o limite seguro de leitura.".into());
    }

    let payload = if content_type.starts_with("text/event-stream") {
        parse_sse(bytes)?
    } else if content_type.starts_with("application/json") {
        serde_json::from_slice(bytes).map_err(|_| "Resposta JSON MCP inválida.".to_string())?
    } else {
        return Err("O servidor MCP retornou um formato de resposta não suportado.".into());
    };
    if payload.get("error").is_some() {
        return Err("O servidor MCP informou erro na solicitação.".into());
    }
    let result = payload
        .get("result")
        .cloned()
        .ok_or_else(|| "Resposta MCP sem resultado.".to_string())?;
    Ok(crate::mcp::truncate_untrusted_result(result))
}

fn metadata() -> Value {
    json!({
        "io.modelcontextprotocol/protocolVersion": PROTOCOL_VERSION,
        "io.modelcontextprotocol/clientInfo": {
            "name": "OdeKrai",
            "version": env!("CARGO_PKG_VERSION")
        },
        "io.modelcontextprotocol/clientCapabilities": {}
    })
}

fn encode_header_value(value: &str) -> Result<HeaderValue, String> {
    let plain_ascii =
        value.bytes().all(|byte| (0x20..=0x7e).contains(&byte)) && !value.starts_with("=?base64?");
    let value = if plain_ascii {
        value.to_owned()
    } else {
        format!("=?base64?{}?=", STANDARD.encode(value.as_bytes()))
    };
    HeaderValue::from_bytes(value.as_bytes())
        .map_err(|_| "Nome da ferramenta MCP inválido para HTTP.".into())
}

fn parse_sse(bytes: &[u8]) -> Result<Value, String> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| "Fluxo SSE MCP não está em UTF-8.".to_string())?;
    let mut event_data = Vec::new();
    let mut last_message = None;
    for line in text.lines().chain(std::iter::once("")) {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.is_empty() {
            if !event_data.is_empty() {
                let candidate = serde_json::from_str::<Value>(&event_data.join("\n"))
                    .map_err(|_| "Evento SSE MCP malformado.".to_string())?;
                if candidate.get("result").is_some() || candidate.get("error").is_some() {
                    last_message = Some(candidate);
                }
                event_data.clear();
            }
        } else if let Some(value) = line.strip_prefix("data:") {
            event_data.push(value.trim_start());
        } else if line.starts_with(':')
            || line.starts_with("event:")
            || line.starts_with("id:")
            || line.starts_with("retry:")
        {
            continue;
        }
    }
    last_message.ok_or_else(|| "Fluxo SSE MCP sem evento de resultado.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockServer(HttpReply);

    impl HttpTransport for MockServer {
        fn post(
            &self,
            _endpoint: &Url,
            _token: &str,
            _method: &str,
            _name: Option<&str>,
            _body: Value,
        ) -> Result<HttpReply, String> {
            Ok(HttpReply {
                status: self.0.status,
                content_type: self.0.content_type.clone(),
                final_url: self.0.final_url.clone(),
                body: self.0.body.clone(),
            })
        }
    }

    fn client(
        status: StatusCode,
        content_type: &str,
        final_url: &str,
        body: Vec<u8>,
    ) -> McpHttpClient {
        McpHttpClient::with_transport(
            "https://mcp.example.org/mcp",
            "test-token",
            Box::new(MockServer(HttpReply {
                status,
                content_type: content_type.into(),
                final_url: Url::parse(final_url).unwrap(),
                body,
            })),
        )
    }

    #[test]
    fn rejects_non_https_and_credentials() {
        assert!(validate_endpoint("http://example.org/mcp").is_err());
        assert!(validate_endpoint("https://user:pass@example.org/mcp").is_err());
        assert!(validate_endpoint("https://example.org/mcp?access_token=secret").is_err());
    }

    #[test]
    fn rejects_malformed_sse() {
        assert!(parse_sse(b"data: {nope}\n\n").is_err());
    }

    #[test]
    fn rejects_redirect_response() {
        let error = client(
            StatusCode::FOUND,
            "application/json",
            "https://mcp.example.org/mcp",
            vec![],
        )
        .tools_list()
        .unwrap_err();
        assert!(error.contains("redirecionar"));
    }

    #[test]
    fn missing_token_is_rejected_without_leaking_value() {
        let error = McpHttpClient::connect("https://example.org/mcp", String::new())
            .err()
            .unwrap();
        assert!(error.contains("token"));
    }

    #[test]
    fn server_host_is_not_a_request_parameter() {
        let client =
            McpHttpClient::connect("https://mcp.example.org/endpoint", "secret-token".into())
                .unwrap();
        assert_eq!(client.endpoint.host_str(), Some("mcp.example.org"));
    }

    #[test]
    fn large_json_result_is_truncated_before_return() {
        let response = format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{{\"content\":[{{\"type\":\"text\",\"text\":\"{}\"}}]}}}}",
            "x".repeat(64 * 1024)
        );
        let parsed = client(
            StatusCode::OK,
            "application/json",
            "https://mcp.example.org/mcp",
            response.into_bytes(),
        )
        .tools_list()
        .unwrap();
        assert_eq!(parsed["truncated"], true);
        assert!(parsed["preview"].as_str().unwrap().len() < 64 * 1024);
    }

    #[test]
    fn errors_never_include_bearer_token() {
        let error = client(
            StatusCode::UNAUTHORIZED,
            "application/json",
            "https://mcp.example.org/mcp",
            b"secret-token".to_vec(),
        )
        .tools_list()
        .unwrap_err();
        assert_eq!(
            error,
            "O token foi recusado. Confira se ele tem as permissões e se não expirou."
        );
        assert!(!error.contains("secret-token"));
    }

    #[test]
    fn maps_401_and_403_to_the_same_safe_pat_message() {
        for status in [StatusCode::UNAUTHORIZED, StatusCode::FORBIDDEN] {
            let error = parse_response(status, "application/json", b"private response body")
                .unwrap_err();
            assert_eq!(
                error,
                "O token foi recusado. Confira se ele tem as permissões e se não expirou."
            );
            assert!(!error.contains("private response body"));
        }
    }

    #[test]
    fn sse_accepts_final_response_after_notification() {
        let value = parse_sse(
            b"data: {\"jsonrpc\":\"2.0\",\"method\":\"notifications/progress\"}\n\ndata: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}\n\n",
        )
        .unwrap();
        assert_eq!(value["id"], 1);
    }

    #[test]
    fn rejects_changed_response_host() {
        let error = client(
            StatusCode::OK,
            "application/json",
            "https://other.example/mcp",
            br#"{"result":{"tools":[]}}"#.to_vec(),
        )
        .tools_list()
        .unwrap_err();
        assert!(error.contains("host diferente"));
    }

    #[test]
    fn malformed_sse_from_mock_server_is_rejected() {
        let error = client(
            StatusCode::OK,
            "text/event-stream",
            "https://mcp.example.org/mcp",
            b"data: {broken}\n\n".to_vec(),
        )
        .tools_list()
        .unwrap_err();
        assert!(error.contains("SSE MCP malformado"));
    }

    #[test]
    fn response_limit_is_fixed() {
        assert_eq!(MAX_RESPONSE_BYTES, 8 * 1024 * 1024);
    }
}
