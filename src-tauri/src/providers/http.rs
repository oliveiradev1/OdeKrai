use super::catalog::catalog_entry;
use reqwest::{header, redirect::Policy, Client};
use serde_json::Value;
use std::time::Duration;

const MAX_PROVIDER_ERROR_BODY_BYTES: usize = 16 * 1024;

pub(super) fn http_client(timeout_seconds: u64) -> Result<Client, String> {
    Client::builder()
        .redirect(Policy::none())
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(timeout_seconds))
        .build()
        .map_err(|_| "Não foi possível preparar uma conexão segura.".to_string())
}

pub(super) fn provider_client(
    provider_id: &str,
    base_url: &str,
    timeout_seconds: u64,
) -> Result<Client, String> {
    if provider_id != "local" {
        return http_client(timeout_seconds);
    }
    let url = reqwest::Url::parse(base_url.trim())
        .map_err(|_| "Informe um endereço local válido.".to_string())?;
    let addresses = super::endpoint::validate_local_url(&url)?;
    Client::builder()
        .redirect(Policy::none())
        .connect_timeout(Duration::from_secs(1))
        .timeout(Duration::from_secs(timeout_seconds.min(30)))
        .resolve_to_addrs(url.host_str().unwrap_or_default(), &addresses)
        .build()
        .map_err(|_| "Não foi possível preparar a conexão local.".to_string())
}

pub(super) fn provider_http_error(status: u16) -> String {
    match status {
        400 => format!(
            "O modelo ou o suporte às ferramentas foi recusado (HTTP {status}). Confira o identificador e escolha um modelo compatível com ferramentas."
        ),
        401 | 403 => format!(
            "O provedor recusou a chave ou o acesso (HTTP {status}). Confira se a chave está ativa e tem acesso ao modelo em Conectores de IA."
        ),
        404 => {
            "O modelo ou endpoint não existe (HTTP 404). Confira o identificador em Conectores de IA.".into()
        }
        408 => {
            "A solicitação atingiu o tempo limite (HTTP 408). Tente novamente ou use outro provedor conectado.".into()
        }
        429 => {
            "O provedor limitou a frequência das requisições (HTTP 429). Aguarde um pouco e tente novamente.".into()
        }
        500..=599 => format!("O serviço do provedor está indisponível (HTTP {status})."),
        _ => format!("O provedor respondeu com HTTP {status}."),
    }
}

pub(super) fn provider_http_error_with_body(status: u16, body: &str) -> String {
    let codes = provider_error_codes(body);
    let normalized = body.to_ascii_lowercase();
    match status {
        401 | 403 => provider_http_error(status),
        429 if codes.iter().any(|code| is_quota_error_code(code)) => {
            "A cota ou o saldo da conta do provedor acabou (HTTP 429). Confira o faturamento e os limites da sua conta.".into()
        }
        429 => provider_http_error(status),
        400 | 404 if codes.iter().any(|code| is_model_missing_code(code)) => {
            "O modelo não existe ou não está disponível para esta conta. Confira o identificador em Conectores de IA.".into()
        }
        400 | 404 if normalized.contains("function") && normalized.contains("name") => format!(
            "O provedor recusou o nome de uma ferramenta (HTTP {status}). Confira a compatibilidade do modelo."
        ),
        400 | 404 if normalized.contains("schema") || normalized.contains("parameters") => format!(
            "O provedor recusou o schema de ferramenta (HTTP {status}). Escolha um modelo compatível com ferramentas."
        ),
        400 | 404 if body_indicates_unsupported_tools(&normalized, &codes) => {
            "Este modelo não oferece suporte às ferramentas solicitadas. Escolha um modelo compatível com ferramentas em Conectores de IA.".into()
        }
        404 => provider_http_error(status),
        400 if normalized.contains("reasoning_effort") => {
            "O modelo recusou a configuração de raciocínio. Confira a compatibilidade do modelo ou escolha outro.".into()
        }
        400 => provider_http_error(status),
        _ => provider_http_error(status),
    }
}

fn provider_error_codes(body: &str) -> Vec<String> {
    fn collect(value: &Value, codes: &mut Vec<String>) {
        match value {
            Value::Object(object) => {
                for (key, value) in object {
                    if matches!(
                        key.to_ascii_lowercase().as_str(),
                        "code" | "error_code" | "type"
                    ) {
                        if let Some(code) = value.as_str() {
                            codes.push(code.to_ascii_lowercase());
                        }
                    }
                    collect(value, codes);
                }
            }
            Value::Array(items) => items.iter().for_each(|item| collect(item, codes)),
            _ => {}
        }
    }

    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return partial_provider_error_codes(body);
    };
    let mut codes = Vec::new();
    collect(&value, &mut codes);
    codes
}

fn partial_provider_error_codes(body: &str) -> Vec<String> {
    let mut codes = Vec::new();
    for key in ["\"code\"", "\"error_code\"", "\"type\""] {
        for (key_start, _) in body.match_indices(key) {
            let bytes = body.as_bytes();
            let preceding_slashes = bytes[..key_start]
                .iter()
                .rev()
                .take_while(|byte| **byte == b'\\')
                .count();
            if preceding_slashes % 2 == 1 {
                continue;
            }

            let mut value_start = key_start + key.len();
            while bytes
                .get(value_start)
                .is_some_and(|byte| byte.is_ascii_whitespace())
            {
                value_start += 1;
            }
            if bytes.get(value_start) != Some(&b':') {
                continue;
            }
            value_start += 1;
            while bytes
                .get(value_start)
                .is_some_and(|byte| byte.is_ascii_whitespace())
            {
                value_start += 1;
            }
            if bytes.get(value_start) != Some(&b'\"') {
                continue;
            }

            let mut escaped = false;
            for value_end in value_start + 1..bytes.len() {
                let byte = bytes[value_end];
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == b'\"' {
                    if let Ok(code) = serde_json::from_str::<String>(&body[value_start..=value_end]) {
                        codes.push(code.to_ascii_lowercase());
                    }
                    break;
                }
            }
        }
    }
    codes
}

fn is_quota_error_code(code: &str) -> bool {
    [
        "quota",
        "billing",
        "credit",
        "balance",
        "payment_required",
        "resource_exhausted",
    ]
    .iter()
    .any(|marker| code.contains(marker))
}

fn is_model_missing_code(code: &str) -> bool {
    code.contains("model_not_found") || code.contains("model_missing") || code == "not_found"
}

fn body_indicates_unsupported_tools(normalized: &str, codes: &[String]) -> bool {
    (normalized.contains("function") && normalized.contains("name"))
        || normalized.contains("tool")
        || normalized.contains("schema")
        || normalized.contains("parameters")
        || codes.iter().any(|code| {
            code.contains("tool") || code.contains("function") || code.contains("unsupported")
        })
}

pub(super) async fn read_provider_error_body(mut response: reqwest::Response) -> String {
    let mut body = Vec::with_capacity(MAX_PROVIDER_ERROR_BODY_BYTES);
    while body.len() < MAX_PROVIDER_ERROR_BODY_BYTES {
        let Ok(Some(chunk)) = response.chunk().await else {
            break;
        };
        let remaining = MAX_PROVIDER_ERROR_BODY_BYTES - body.len();
        body.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
    }
    String::from_utf8_lossy(&body).into_owned()
}

pub(super) fn header_value(value: &str) -> Result<header::HeaderValue, String> {
    header::HeaderValue::from_str(value)
        .map_err(|_| "A credencial contém caracteres inválidos para uma requisição.".to_string())
}

pub(super) trait AuthRequest {
    fn pipe_auth(self, provider_id: &str, credential: &str) -> Result<Self, String>
    where
        Self: Sized;
}

impl AuthRequest for reqwest::RequestBuilder {
    fn pipe_auth(self, provider_id: &str, credential: &str) -> Result<Self, String> {
        if provider_id == "local" && credential.is_empty() {
            return Ok(self);
        }
        let entry = catalog_entry(provider_id)
            .ok_or("A autenticação deste provedor não está cadastrada.")?;
        match entry.auth_header.as_deref() {
            Some("x-api-key") => Ok(self.header("x-api-key", header_value(credential)?)),
            Some("api-key") => Ok(self.header("api-key", header_value(credential)?)),
            Some("Authorization") => {
                Ok(self.header(header::AUTHORIZATION, bearer_value(credential)?))
            }
            _ => Err("O método de autenticação deste provedor ainda não está implementado.".into()),
        }
    }
}

fn bearer_value(value: &str) -> Result<header::HeaderValue, String> {
    header::HeaderValue::from_str(&format!("Bearer {value}"))
        .map_err(|_| "A credencial contém caracteres inválidos para uma requisição.".to_string())
}

pub(super) fn safe_transport_error(error: &reqwest::Error) -> String {
    if error.is_timeout() {
        "O provedor demorou para responder. Tente novamente.".into()
    } else if error.is_connect() {
        "Não foi possível conectar ao provedor. Verifique sua conexão.".into()
    } else {
        "A comunicação segura com o provedor falhou.".into()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        provider_http_error_with_body, read_provider_error_body, safe_transport_error,
    };
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
        time::Duration,
    };

    #[tokio::test]
    async fn mock_http_errors_map_to_safe_actionable_agent_categories() {
        let cases = [
            (
                400,
                r#"{"error":{"message":"Invalid function name: files.read private-user-content"}}"#,
                "nome de uma ferramenta",
                "invalid_tool_name",
            ),
            (
                401,
                r#"{"error":{"code":"invalid_api_key","message":"invalid api key private-user-content sk-test-secret"}}"#,
                "HTTP 401",
                "invalid_credential",
            ),
            (
                403,
                r#"{"error":{"message":"model access denied private-user-content"}}"#,
                "recusou a chave",
                "invalid_credential",
            ),
            (
                408,
                r#"{"error":{"code":"request_timeout","message":"private-user-content"}}"#,
                "tempo limite",
                "timeout",
            ),
            (
                429,
                r#"{"error":{"code":"insufficient_quota","message":"private-user-content"}}"#,
                "cota ou o saldo",
                "quota_or_balance",
            ),
            (
                429,
                r#"{"error":{"code":"rate_limit_exceeded","message":"private-user-content"}}"#,
                "frequência das requisições",
                "rate_limit",
            ),
            (
                400,
                r#"{"error":{"code":"invalid_request_error","message":"invalid request private-user-content"}}"#,
                "HTTP 400",
                "model_or_tools",
            ),
            (
                404,
                r#"{"error":{"code":"model_not_found","message":"private-user-content"}}"#,
                "modelo não existe",
                "model_not_found",
            ),
            (
                404,
                r#"{"error":{"code":"unsupported_tools","message":"tools are not available for this model"}}"#,
                "não oferece suporte às ferramentas",
                "unsupported_tools",
            ),
            (
                500,
                r#"{"error":{"message":"internal error private-user-content"}}"#,
                "HTTP 500",
                "provider_unavailable",
            ),
        ];

        for (status, body, expected, expected_kind) in cases {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let response_body = body.to_owned();
            let server = thread::spawn(move || {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = [0_u8; 1024];
                let _ = socket.read(&mut request);
                write!(
                    socket,
                    "HTTP/1.1 {status} Error\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response_body}",
                    response_body.len()
                )
                .unwrap();
            });
            let response = reqwest::Client::new()
                .get(format!("http://{address}/mock"))
                .send()
                .await
                .unwrap();
            let actual_status = response.status().as_u16();
            let response_body = read_provider_error_body(response).await;
            let safe_error = provider_http_error_with_body(actual_status, &response_body);
            server.join().unwrap();

            assert_eq!(actual_status, status);
            assert!(safe_error.contains(expected), "{safe_error}");
            assert!(!safe_error.contains("private-user-content"));
            assert!(!safe_error.contains("sk-test-secret"));
            let failure = crate::agent::classify_provider_error(&safe_error);
            assert_eq!(failure.kind, expected_kind, "{safe_error}");
            assert!(!failure.message.contains("private-user-content"));
            assert!(!failure.technical_details.contains("sk-test-secret"));
        }
    }

    #[tokio::test]
    async fn mock_timeout_and_network_errors_have_distinct_categories() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = [0_u8; 1024];
            let _ = socket.read(&mut request);
            thread::sleep(Duration::from_millis(100));
        });
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(20))
            .build()
            .unwrap();
        let timeout_error = client
            .get(format!("http://{address}/mock"))
            .send()
            .await
            .unwrap_err();
        let safe_timeout = safe_transport_error(&timeout_error);
        assert_eq!(crate::agent::classify_provider_error(&safe_timeout).kind, "timeout");
        server.join().unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let unused_address = listener.local_addr().unwrap();
        drop(listener);
        let network_error = reqwest::Client::new()
            .get(format!("http://{unused_address}/mock"))
            .send()
            .await
            .unwrap_err();
        let safe_network = safe_transport_error(&network_error);
        assert_eq!(crate::agent::classify_provider_error(&safe_network).kind, "network");
    }

    #[tokio::test]
    async fn error_body_reading_is_limited_to_sixteen_kibibytes() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = [0_u8; 1024];
            let _ = socket.read(&mut request);
            let body = format!(
                "{{\"error\":{{\"code\":\"insufficient_quota\",\"padding\":\"{}\"}}}}",
                "x".repeat(20 * 1024)
            );
            let _ = write!(
                socket,
                "HTTP/1.1 429 Too Many Requests\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
        });
        let response = reqwest::Client::new()
            .get(format!("http://{address}/mock"))
            .send()
            .await
            .unwrap();
        let body = read_provider_error_body(response).await;
        server.join().unwrap();
        assert_eq!(body.len(), 16 * 1024);
        let safe_error = provider_http_error_with_body(429, &body);
        assert!(safe_error.contains("cota ou o saldo"));
        assert!(!safe_error.contains(&"x".repeat(100)));
    }
}
