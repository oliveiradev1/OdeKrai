//! OAuth para servidores MCP remotos. Nenhum token é persistido fora do Credential Manager.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use keyring::Entry;
use reqwest::{blocking::Client, redirect::Policy, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};
use tauri::{Emitter, State};
use url::Url;

const SERVICE: &str = "site.anceryx.odekrai.mcp";
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(180);
const MAX_METADATA_BYTES: usize = 256 * 1024;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredOAuth {
    pub issuer: String,
    pub resource: String,
    pub client_id: String,
    pub token_endpoint: String,
    #[serde(default)]
    pub revocation_endpoint: Option<String>,
    pub refresh_token: Option<String>,
}

impl std::fmt::Debug for StoredOAuth {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StoredOAuth")
            .field("issuer", &self.issuer)
            .field("resource", &self.resource)
            .field("client_id", &self.client_id)
            .field("token_endpoint", &self.token_endpoint)
            .field("revocation_endpoint", &self.revocation_endpoint)
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

#[derive(Clone)]
pub(crate) struct OAuthAccessToken {
    pub value: String,
    pub expires_at: Instant,
}

impl std::fmt::Debug for OAuthAccessToken {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OAuthAccessToken")
            .field("value", &"[REDACTED]")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

pub(crate) fn should_refresh(expires_at: Instant, now: Instant) -> bool {
    expires_at <= now + Duration::from_secs(60)
}

#[derive(Debug)]
struct ServerMetadata {
    authorization_endpoint: String,
    token_endpoint: String,
    registration_endpoint: Option<String>,
    revocation_endpoint: Option<String>,
    client_id_metadata_document_supported: bool,
    issuer_parameter_supported: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistrationOptions {
    pub dynamic_registration: bool,
}

fn credential(server_id: &str) -> Result<Entry, String> {
    Entry::new(SERVICE, &format!("{server_id}:oauth"))
        .map_err(|_| "Não foi possível acessar o Gerenciador de Credenciais do Windows.".into())
}

pub(crate) fn load(server_id: &str) -> Result<Option<StoredOAuth>, String> {
    match credential(server_id)?.get_password() {
        Ok(raw) => serde_json::from_str(&raw)
            .map(Some)
            .map_err(|_| "A autorização MCP salva está inválida; conecte novamente.".into()),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => {
            Err("Não foi possível ler a autorização MCP no Gerenciador de Credenciais.".into())
        }
    }
}

fn save(server_id: &str, value: &StoredOAuth) -> Result<(), String> {
    let encoded = serde_json::to_string(value)
        .map_err(|_| "Não foi possível preparar a autorização MCP.".to_string())?;
    credential(server_id)?.set_password(&encoded).map_err(|_| {
        "Não foi possível salvar a autorização no Gerenciador de Credenciais do Windows.".into()
    })
}

pub(crate) fn remove(server_id: &str) -> Result<(), String> {
    match credential(server_id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => {
            Err("Não foi possível remover a autorização MCP do Gerenciador de Credenciais.".into())
        }
    }
}

fn safe_http() -> Result<Client, String> {
    Client::builder()
        .redirect(Policy::none())
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|_| "Não foi possível preparar a conexão segura de autorização MCP.".into())
}

fn validate_https(raw: &str) -> Result<Url, String> {
    let url = Url::parse(raw)
        .map_err(|_| "O servidor de autorização MCP retornou um endereço inválido.".to_string())?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || !matches!(url.host(), Some(url::Host::Domain(_)))
    {
        return Err("Os endpoints OAuth MCP precisam usar HTTPS com domínio válido.".into());
    }
    Ok(url)
}

fn same_origin(left: &Url, right: &Url) -> bool {
    left.scheme() == right.scheme()
        && left.host_str() == right.host_str()
        && left.port_or_known_default() == right.port_or_known_default()
}

fn validate_issuer(expected: &str, returned: &str) -> Result<(), String> {
    if expected == returned {
        Ok(())
    } else {
        Err("O emissor nos metadados OAuth não corresponde ao emissor anunciado.".into())
    }
}

fn bounded_json(
    response: reqwest::blocking::Response,
) -> Result<(StatusCode, reqwest::header::HeaderMap, Value), String> {
    let status = response.status();
    let headers = response.headers().clone();
    let mut bytes = Vec::new();
    response
        .take((MAX_METADATA_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| "Não foi possível ler a resposta de autorização MCP.".to_string())?;
    if bytes.len() > MAX_METADATA_BYTES {
        return Err("A resposta de autorização MCP excedeu o limite seguro.".into());
    }
    let value = serde_json::from_slice(&bytes)
        .map_err(|_| "O servidor de autorização MCP retornou JSON inválido.".to_string())?;
    Ok((status, headers, value))
}

fn get_json(client: &Client, endpoint: &str) -> Result<Value, String> {
    let url = validate_https(endpoint)?;
    let response = client
        .get(url.clone())
        .send()
        .map_err(|_| "Falha de rede ao consultar metadados OAuth MCP.".to_string())?;
    reject_redirect(response.status())?;
    if response.url() != &url {
        return Err("Redirecionamento em metadados OAuth MCP foi recusado.".into());
    }
    let (status, _, json) = bounded_json(response)?;
    if !status.is_success() {
        return Err("Não foi possível consultar metadados OAuth MCP.".into());
    }
    Ok(json)
}

fn post_json(client: &Client, endpoint: &str, body: Value) -> Result<Value, String> {
    let url = validate_https(endpoint)?;
    let response = client
        .post(url.clone())
        .json(&body)
        .send()
        .map_err(|_| "Falha de rede no registro OAuth MCP.".to_string())?;
    reject_redirect(response.status())?;
    if response.url() != &url {
        return Err("Redirecionamento no registro OAuth MCP foi recusado.".into());
    }
    let (status, _, json) = bounded_json(response)?;
    if !status.is_success() {
        return Err("O servidor recusou o registro do cliente OAuth do OdeKrai.".into());
    }
    Ok(json)
}

fn well_known_candidates(issuer: &Url) -> Vec<String> {
    let origin = format!(
        "{}://{}{}",
        issuer.scheme(),
        issuer.host_str().unwrap_or_default(),
        issuer
            .port()
            .map(|port| format!(":{port}"))
            .unwrap_or_default()
    );
    let path = issuer.path().trim_end_matches('/');
    if path.is_empty() {
        vec![
            format!("{origin}/.well-known/oauth-authorization-server"),
            format!("{origin}/.well-known/openid-configuration"),
        ]
    } else {
        vec![
            format!("{origin}/.well-known/oauth-authorization-server{path}"),
            format!("{origin}/.well-known/openid-configuration{path}"),
            format!("{}{path}/.well-known/openid-configuration", origin),
        ]
    }
}

fn discover_resource_metadata(client: &Client, endpoint: &Url) -> Result<(Value, String), String> {
    let id = 1;
    let body = json!({"jsonrpc":"2.0","id":id,"method":"tools/list","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientInfo":{"name":"OdeKrai","version":env!("CARGO_PKG_VERSION")},"io.modelcontextprotocol/clientCapabilities":{}}}});
    let response = client
        .post(endpoint.clone())
        .header("MCP-Protocol-Version", "2026-07-28")
        .header("Mcp-Method", "tools/list")
        .header(
            reqwest::header::ACCEPT,
            "application/json, text/event-stream",
        )
        .json(&body)
        .send()
        .map_err(|_| "Falha de rede ao descobrir a autorização MCP.".to_string())?;
    if response.url() != endpoint || response.status().is_redirection() {
        return Err("Redirecionamento na descoberta OAuth MCP foi recusado.".into());
    }
    let challenge = response
        .headers()
        .get(reqwest::header::WWW_AUTHENTICATE)
        .and_then(|header| header.to_str().ok())
        .map(str::to_owned);
    let resource_metadata = challenge.as_deref().and_then(resource_metadata_parameter);
    if response.status() != StatusCode::UNAUTHORIZED {
        return Err("O servidor MCP não informou metadados de recurso protegido.".into());
    }
    if let Some(metadata_url) = resource_metadata {
        let parsed = validate_https(&metadata_url)?;
        if !same_origin(&parsed, endpoint) {
            return Err("A origem dos metadados OAuth não corresponde ao servidor MCP.".into());
        }
        let value = get_json(client, parsed.as_str())?;
        return Ok((value, challenge.unwrap_or_default()));
    }
    for candidate in resource_metadata_candidates(endpoint) {
        if let Ok(metadata) = get_json(client, &candidate) {
            return Ok((metadata, challenge.unwrap_or_default()));
        }
    }
    Err("Não foi possível descobrir os metadados do recurso MCP.".into())
}

fn resource_metadata_candidates(endpoint: &Url) -> Vec<String> {
    let origin = format!(
        "{}://{}{}",
        endpoint.scheme(),
        endpoint.host_str().unwrap_or_default(),
        endpoint
            .port()
            .map(|port| format!(":{port}"))
            .unwrap_or_default()
    );
    let path = endpoint.path().trim_end_matches('/');
    let mut candidates = Vec::new();
    if !path.is_empty() {
        candidates.push(format!(
            "{origin}/.well-known/oauth-protected-resource{path}"
        ));
    }
    candidates.push(format!("{origin}/.well-known/oauth-protected-resource"));
    candidates
}

fn resource_metadata_parameter(challenge: &str) -> Option<String> {
    challenge.split(',').find_map(|part| {
        let (key, value) = part.trim().split_once('=')?;
        if !key.trim().eq_ignore_ascii_case("resource_metadata") {
            return None;
        }
        Some(value.trim().trim_matches('"').to_owned())
    })
}

fn discover_authorization_server(
    client: &Client,
    resource: &Value,
) -> Result<(String, ServerMetadata), String> {
    let issuer = resource
        .get("authorization_servers")
        .and_then(Value::as_array)
        .and_then(|servers| servers.first())
        .and_then(Value::as_str)
        .ok_or("O recurso MCP não anunciou um servidor de autorização.")?
        .to_owned();
    let issuer_url = validate_https(&issuer)?;
    for candidate in well_known_candidates(&issuer_url) {
        let metadata = match get_json(client, &candidate) {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        let returned_issuer = metadata
            .get("issuer")
            .and_then(Value::as_str)
            .unwrap_or_default();
        validate_issuer(&issuer, returned_issuer)?;
        let authorization_endpoint = metadata
            .get("authorization_endpoint")
            .and_then(Value::as_str)
            .ok_or("Metadados OAuth sem authorization_endpoint.")?
            .to_owned();
        let token_endpoint = metadata
            .get("token_endpoint")
            .and_then(Value::as_str)
            .ok_or("Metadados OAuth sem token_endpoint.")?
            .to_owned();
        validate_https(&authorization_endpoint)?;
        validate_https(&token_endpoint)?;
        let registration_endpoint = metadata
            .get("registration_endpoint")
            .and_then(Value::as_str)
            .map(str::to_owned);
        if let Some(endpoint) = registration_endpoint.as_deref() {
            validate_https(endpoint)?;
        }
        let revocation_endpoint = metadata
            .get("revocation_endpoint")
            .and_then(Value::as_str)
            .map(str::to_owned);
        if let Some(endpoint) = revocation_endpoint.as_deref() {
            validate_https(endpoint)?;
        }
        let client_id_metadata_document_supported = metadata
            .get("client_id_metadata_document_supported")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let issuer_parameter_supported = metadata
            .get("authorization_response_iss_parameter_supported")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        return Ok((
            issuer,
            ServerMetadata {
                authorization_endpoint,
                token_endpoint,
                registration_endpoint,
                revocation_endpoint,
                client_id_metadata_document_supported,
                issuer_parameter_supported,
            },
        ));
    }
    Err("Não foi possível descobrir metadados compatíveis para o emissor OAuth.".into())
}

fn inspect_registration_options(endpoint: &str) -> Result<RegistrationOptions, String> {
    let endpoint = crate::mcp_http::validate_endpoint(endpoint)?;
    let client = safe_http()?;
    let (resource, _) = discover_resource_metadata(&client, &endpoint)?;
    let resource_id = resource
        .get("resource")
        .and_then(Value::as_str)
        .ok_or("Metadados MCP sem identificador do recurso protegido.")?;
    if resource_id != endpoint.as_str() {
        return Err("O recurso OAuth anunciado não corresponde ao endpoint MCP cadastrado.".into());
    }
    let (_, metadata) = discover_authorization_server(&client, &resource)?;
    Ok(RegistrationOptions {
        dynamic_registration: metadata.registration_endpoint.is_some(),
    })
}

fn validate_client_id(client_id: &str) -> Result<String, String> {
    let client_id = client_id.trim();
    if client_id.is_empty() || client_id.len() > 2048 || client_id.chars().any(char::is_control) {
        return Err("Informe um identificador de cliente OAuth válido.".into());
    }
    Ok(client_id.to_owned())
}

fn normalize_scopes(scopes: Option<&str>) -> Result<Option<String>, String> {
    let Some(scopes) = scopes.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if scopes.len() > 1024 || scopes.chars().any(char::is_control) {
        return Err("A lista de escopos OAuth é inválida ou excede o limite permitido.".into());
    }
    Ok(Some(scopes.split_whitespace().collect::<Vec<_>>().join(" ")))
}

fn register_client(
    client: &Client,
    metadata: &ServerMetadata,
    redirect_uri: &str,
    supplied_client_id: Option<&str>,
    scopes: Option<&str>,
) -> Result<String, String> {
    register_client_with(metadata, redirect_uri, supplied_client_id, scopes, |endpoint, body| {
        post_json(client, endpoint, body)
    })
}

fn register_client_with(
    metadata: &ServerMetadata,
    redirect_uri: &str,
    supplied_client_id: Option<&str>,
    scopes: Option<&str>,
    mut post: impl FnMut(&str, Value) -> Result<Value, String>,
) -> Result<String, String> {
    if let Some(endpoint) = metadata.registration_endpoint.as_deref() {
        let mut request = json!({
            "client_name":"OdeKrai",
            "application_type":"native",
            "redirect_uris":[redirect_uri],
            "grant_types":["authorization_code","refresh_token"],
            "response_types":["code"],
            "token_endpoint_auth_method":"none"
        });
        if let Some(scopes) = scopes {
            request["scope"] = Value::String(scopes.to_owned());
        }
        let response = post(endpoint, request)?;
        return response
            .get("client_id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty() && id.len() <= 2048 && !id.chars().any(char::is_control))
            .map(str::to_owned)
            .ok_or_else(|| "O registro OAuth não retornou um client_id válido.".into());
    }
    if let Some(client_id) = supplied_client_id {
        return validate_client_id(client_id);
    }
    if metadata.client_id_metadata_document_supported {
        return Err("Este servidor OAuth usa um formato de identificador de cliente ainda não suportado pelo OdeKrai.".into());
    }
    Err("Este servidor exige um identificador de cliente. Consulte a documentação do serviço para criá-lo.".into())
}

fn random_token() -> Result<String, String> {
    crate::google_oauth::random_url_token()
}
fn pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn build_authorization_url(
    metadata: &ServerMetadata,
    client_id: &str,
    redirect_uri: &str,
    state: &str,
    challenge: &str,
    resource: &str,
    scope: Option<&str>,
) -> Result<String, String> {
    let mut url = Url::parse(&metadata.authorization_endpoint)
        .map_err(|_| "authorization_endpoint inválido.".to_string())?;
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("code_challenge", challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", state)
        .append_pair("resource", resource);
    if let Some(scope) = scope.filter(|scope| !scope.is_empty()) {
        url.query_pairs_mut().append_pair("scope", scope);
    }
    Ok(url.to_string())
}

fn scope_from_challenge(challenge: &str, resource: &Value) -> Option<String> {
    if let Some(scope) = challenge.split(',').find_map(|part| {
        let (key, value) = part.trim().split_once('=')?;
        if key.trim().eq_ignore_ascii_case("scope") {
            Some(value.trim().trim_matches('"').to_owned())
        } else {
            None
        }
    }) {
        return Some(scope);
    }
    let scopes = resource.get("scopes_supported")?.as_array()?;
    let values = scopes
        .iter()
        .filter_map(Value::as_str)
        .take(64)
        .collect::<Vec<_>>();
    (!values.is_empty()).then(|| values.join(" "))
}

struct CallbackGate(AtomicBool);
impl CallbackGate {
    fn claim(&self) -> bool {
        !self.0.swap(true, Ordering::SeqCst)
    }
}

fn parse_callback_target(
    target: &str,
    expected_state: &str,
    expected_issuer: &str,
    issuer_required: bool,
) -> Result<(String, Option<String>), String> {
    let parsed = Url::parse(&format!("http://127.0.0.1{target}"))
        .map_err(|_| "O retorno OAuth está malformado.".to_string())?;
    if parsed.path() != "/callback" {
        return Err("Caminho de retorno OAuth inválido.".into());
    }
    let mut params = std::collections::HashMap::new();
    for (key, value) in parsed.query_pairs() {
        if params
            .insert(key.into_owned(), value.into_owned())
            .is_some()
        {
            return Err("O retorno OAuth contém parâmetros repetidos.".into());
        }
    }
    if params.get("state").map(String::as_str) != Some(expected_state) {
        return Err("A validação state do OAuth falhou.".into());
    }
    if let Some(issuer) = params.get("iss") {
        if issuer != expected_issuer {
            return Err("O emissor do retorno OAuth não corresponde ao descoberto.".into());
        }
    } else if issuer_required {
        return Err("O retorno OAuth não informou o emissor exigido.".into());
    }
    if params
        .get("error")
        .is_some_and(|error| error == "access_denied")
    {
        return Err("O consentimento do servidor MCP foi recusado.".into());
    }
    if params.contains_key("error") {
        return Err("O servidor de autorização MCP não concluiu a conexão.".into());
    }
    let code = params
        .get("code")
        .filter(|code| !code.is_empty())
        .cloned()
        .ok_or("O retorno OAuth não contém código de autorização.")?;
    Ok((code, params.get("iss").cloned()))
}

fn reject_redirect(status: StatusCode) -> Result<(), String> {
    if status.is_redirection() {
        Err("Redirecionamento no fluxo OAuth MCP foi recusado.".into())
    } else {
        Ok(())
    }
}

fn accept_callback(
    listener: TcpListener,
    state: &str,
    issuer: &str,
    issuer_required: bool,
) -> Result<String, String> {
    listener
        .set_nonblocking(true)
        .map_err(|_| "Não foi possível preparar o retorno OAuth local.".to_string())?;
    let expected_host = format!(
        "127.0.0.1:{}",
        listener
            .local_addr()
            .map_err(|_| "Não foi possível ler a porta local OAuth.".to_string())?
            .port()
    );
    let started = Instant::now();
    let gate = CallbackGate(AtomicBool::new(false));
    loop {
        if started.elapsed() >= CALLBACK_TIMEOUT {
            return Err("A autorização do servidor MCP expirou. Tente novamente.".into());
        }
        match listener.accept() {
            Ok((mut stream, address)) => {
                if !address.ip().is_loopback() || !gate.claim() {
                    continue;
                }
                stream.set_read_timeout(Some(Duration::from_secs(3))).ok();
                let mut buffer = [0u8; 8192];
                let mut received = 0;
                while received < buffer.len()
                    && !buffer[..received]
                        .windows(4)
                        .any(|part| part == b"\r\n\r\n")
                {
                    let count = stream
                        .read(&mut buffer[received..])
                        .map_err(|_| "Não foi possível ler o retorno OAuth.".to_string())?;
                    if count == 0 {
                        break;
                    }
                    received += count;
                }
                let request = std::str::from_utf8(&buffer[..received])
                    .map_err(|_| "O retorno OAuth está malformado.".to_string())?;
                let mut lines = request.lines();
                let request_line = lines.next().unwrap_or_default();
                let mut request_parts = request_line.split_whitespace();
                if request_parts.next() != Some("GET") {
                    return Err("O retorno OAuth usou um método inválido.".into());
                }
                let target = request_parts
                    .next()
                    .ok_or("O retorno OAuth está malformado.")?;
                if !lines.any(|line| {
                    line.strip_prefix("Host:")
                        .or_else(|| line.strip_prefix("host:"))
                        .is_some_and(|host| host.trim() == expected_host)
                }) {
                    return Err(
                        "O retorno OAuth não corresponde ao endereço local esperado.".into(),
                    );
                }
                let result = parse_callback_target(target, state, issuer, issuer_required);
                let body = if result.is_ok() {
                    "Autorização recebida. Você pode voltar ao OdeKrai."
                } else {
                    "A autorização não foi concluída. Volte ao OdeKrai."
                };
                let response = format!("HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}", body.len(), body);
                let _ = stream.write_all(response.as_bytes());
                return result.map(|(code, _)| code);
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(30))
            }
            Err(_) => return Err("Não foi possível receber o retorno OAuth local.".into()),
        }
    }
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    token_type: Option<String>,
    expires_in: Option<u64>,
    error: Option<String>,
}

fn exchange_code(
    client: &Client,
    metadata: &ServerMetadata,
    client_id: &str,
    code: &str,
    redirect_uri: &str,
    verifier: &str,
    resource: &str,
) -> Result<(String, Option<String>, Instant), String> {
    let endpoint = validate_https(&metadata.token_endpoint)?;
    let response = client
        .post(endpoint.clone())
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("client_id", client_id),
            ("code_verifier", verifier),
            ("resource", resource),
        ])
        .send()
        .map_err(|_| "Falha de rede ao trocar o código OAuth.".to_string())?;
    reject_redirect(response.status())?;
    if response.url() != &endpoint {
        return Err("Redirecionamento no endpoint de token foi recusado.".into());
    }
    let (status, _, body) = bounded_json(response)?;
    if !status.is_success() {
        return Err("O servidor de autorização recusou o código OAuth.".into());
    }
    let tokens: TokenResponse = serde_json::from_value(body)
        .map_err(|_| "Resposta de token OAuth inválida.".to_string())?;
    if let Some(error) = tokens.error {
        return Err(if error == "access_denied" {
            "O consentimento foi recusado.".into()
        } else {
            "A troca do código OAuth falhou.".into()
        });
    }
    if !tokens
        .token_type
        .as_deref()
        .is_some_and(|kind| kind.eq_ignore_ascii_case("bearer"))
    {
        return Err("O servidor OAuth não retornou token Bearer.".into());
    }
    let access = tokens
        .access_token
        .filter(|token| {
            !token.is_empty() && token.len() <= 32_768 && !token.chars().any(char::is_control)
        })
        .ok_or("O servidor OAuth não retornou access token válido.")?;
    let expires = tokens.expires_in.unwrap_or(3600).clamp(30, 86_400);
    Ok((
        access,
        tokens
            .refresh_token
            .filter(|token| !token.is_empty() && token.len() <= 32_768),
        Instant::now() + Duration::from_secs(expires),
    ))
}

pub(crate) fn refresh_access(server_id: &str) -> Result<OAuthAccessToken, String> {
    let mut stored =
        load(server_id)?.ok_or("A autorização MCP expirou. Conecte novamente pelo botão OAuth.")?;
    let refresh_token = stored
        .refresh_token
        .clone()
        .ok_or("O servidor não forneceu refresh token. Conecte novamente pelo botão OAuth.")?;
    let endpoint = validate_https(&stored.token_endpoint)?;
    let client = safe_http()?;
    let response = client
        .post(endpoint.clone())
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token.as_str()),
            ("client_id", stored.client_id.as_str()),
            ("resource", stored.resource.as_str()),
        ])
        .send()
        .map_err(|_| {
            "Falha de rede ao renovar a autorização MCP. Conecte novamente.".to_string()
        })?;
    reject_redirect(response.status())?;
    if response.url() != &endpoint {
        return Err("Redirecionamento na renovação OAuth foi recusado. Conecte novamente.".into());
    }
    let (status, _, body) = bounded_json(response)?;
    if status == StatusCode::UNAUTHORIZED || !status.is_success() {
        return Err("A renovação OAuth falhou. Conecte novamente o servidor MCP.".into());
    }
    let tokens: TokenResponse = serde_json::from_value(body)
        .map_err(|_| "Resposta de renovação OAuth inválida. Conecte novamente.".to_string())?;
    if tokens.error.is_some()
        || !tokens
            .token_type
            .as_deref()
            .is_some_and(|kind| kind.eq_ignore_ascii_case("bearer"))
    {
        return Err("A renovação OAuth foi recusada. Conecte novamente o servidor MCP.".into());
    }
    let access_token = tokens
        .access_token
        .filter(|token| {
            !token.is_empty() && token.len() <= 32_768 && !token.chars().any(char::is_control)
        })
        .ok_or("A renovação OAuth não retornou um access token. Conecte novamente.")?;
    if let Some(rotated_refresh) = tokens
        .refresh_token
        .filter(|token| !token.is_empty() && token.len() <= 32_768)
    {
        stored.refresh_token = Some(rotated_refresh);
        save(server_id, &stored)?;
    }
    let expires = tokens.expires_in.unwrap_or(3600).clamp(30, 86_400);
    Ok(OAuthAccessToken {
        value: access_token,
        expires_at: Instant::now() + Duration::from_secs(expires),
    })
}

trait RevocationTransport {
    fn revoke(
        &mut self,
        endpoint: &Url,
        client_id: &str,
        token: &str,
        token_type_hint: &str,
    ) -> Result<StatusCode, ()>;
}

struct ReqwestRevocationTransport(Client);

impl RevocationTransport for ReqwestRevocationTransport {
    fn revoke(
        &mut self,
        endpoint: &Url,
        client_id: &str,
        token: &str,
        token_type_hint: &str,
    ) -> Result<StatusCode, ()> {
        let response = self
            .0
            .post(endpoint.clone())
            .form(&[
                ("token", token),
                ("token_type_hint", token_type_hint),
                ("client_id", client_id),
            ])
            .send()
            .map_err(|_| ())?;
        if response.url() != endpoint || response.status().is_redirection() {
            return Err(());
        }
        Ok(response.status())
    }
}

fn revoke_tokens_with(
    stored: &StoredOAuth,
    access_token: Option<&str>,
    transport: &mut impl RevocationTransport,
) -> bool {
    let Some(raw_endpoint) = stored.revocation_endpoint.as_deref() else {
        return true;
    };
    let Ok(endpoint) = validate_https(raw_endpoint) else {
        return false;
    };
    let mut tokens = Vec::new();
    if let Some(token) = stored.refresh_token.as_deref() {
        tokens.push((token, "refresh_token"));
    }
    if let Some(token) = access_token {
        tokens.push((token, "access_token"));
    }
    tokens.into_iter().fold(true, |all_revoked, (token, hint)| {
        let revoked = transport
            .revoke(&endpoint, &stored.client_id, token, hint)
            .is_ok_and(|status| status == StatusCode::OK);
        all_revoked && revoked
    })
}

pub(crate) fn revoke_before_remove(server_id: &str, access_token: Option<&str>) -> bool {
    let stored = match load(server_id) {
        Ok(Some(stored)) => stored,
        Ok(None) => return true,
        Err(_) => return false,
    };
    if stored.revocation_endpoint.is_none() {
        return true;
    }
    let Ok(client) = safe_http() else {
        return false;
    };
    revoke_tokens_with(
        &stored,
        access_token,
        &mut ReqwestRevocationTransport(client),
    )
}

pub(crate) fn removal_warning(revoked: bool) -> Option<String> {
    (!revoked).then(|| {
        "Servidor removido deste dispositivo, mas o provedor não confirmou a revogação do token. Revogue-o no painel do provedor, se necessário.".to_string()
    })
}

fn run_authorization(
    app: tauri::AppHandle,
    endpoint: &str,
    server_id: &str,
    supplied_client_id: Option<&str>,
    requested_scopes: Option<&str>,
) -> Result<(StoredOAuth, OAuthAccessToken), String> {
    let endpoint = crate::mcp_http::validate_endpoint(endpoint)?;
    let client = safe_http()?;
    let (resource_metadata, challenge) = discover_resource_metadata(&client, &endpoint)?;
    let resource = resource_metadata
        .get("resource")
        .and_then(Value::as_str)
        .ok_or("Metadados MCP sem identificador do recurso protegido.")?
        .to_owned();
    if resource != endpoint.as_str() {
        return Err("O recurso OAuth anunciado não corresponde ao endpoint MCP cadastrado.".into());
    }
    let (issuer, metadata) = discover_authorization_server(&client, &resource_metadata)?;
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|_| "Não foi possível abrir a porta local de retorno OAuth.".to_string())?;
    let port = listener
        .local_addr()
        .map_err(|_| "Não foi possível ler a porta local OAuth.".to_string())?
        .port();
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");
    let requested_scopes = normalize_scopes(requested_scopes)?;
    let client_id = register_client(
        &client,
        &metadata,
        &redirect_uri,
        supplied_client_id,
        requested_scopes.as_deref(),
    )?;
    let discovered_scope = scope_from_challenge(&challenge, &resource_metadata);
    let effective_scope = requested_scopes.as_deref().or(discovered_scope.as_deref());
    let verifier = random_token()?;
    let state = random_token()?;
    let auth_url = build_authorization_url(
        &metadata,
        &client_id,
        &redirect_uri,
        &state,
        &pkce_challenge(&verifier),
        &resource,
        effective_scope,
    )?;
    crate::external_links::open_oauth_authorization_url(&app, &auth_url)?;
    let code = accept_callback(
        listener,
        &state,
        &issuer,
        metadata.issuer_parameter_supported,
    )?;
    let (access_token, refresh_token, expires_at) = exchange_code(
        &client,
        &metadata,
        &client_id,
        &code,
        &redirect_uri,
        &verifier,
        &resource,
    )?;
    let stored = StoredOAuth {
        issuer,
        resource,
        client_id,
        token_endpoint: metadata.token_endpoint,
        revocation_endpoint: metadata.revocation_endpoint,
        refresh_token,
    };
    save(server_id, &stored)?;
    Ok((
        stored,
        OAuthAccessToken {
            value: access_token,
            expires_at,
        },
    ))
}

#[tauri::command]
pub async fn mcp_oauth_authorize(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    server_id: String,
    client_id: Option<String>,
    scopes: Option<String>,
    runtime: State<'_, crate::mcp_registry::McpRuntime>,
) -> Result<(), String> {
    crate::require_main(&caller)?;
    let server = crate::mcp_registry::get_server(&app, &server_id)?;
    if server.transport != "streamable_http" {
        return Err("OAuth MCP só está disponível para servidores Streamable HTTP.".into());
    }
    let endpoint = server.endpoint.ok_or("Endereço MCP ausente.")?;
    let id = server_id.clone();
    let app_for_flow = app.clone();
    let (_credential, access) =
        tokio::task::spawn_blocking(move || {
            run_authorization(
                app_for_flow,
                &endpoint,
                &id,
                client_id.as_deref(),
                scopes.as_deref(),
            )
        })
            .await
            .map_err(|_| "A tarefa de autorização MCP foi interrompida.".to_string())??;
    Entry::new(SERVICE, &format!("{server_id}:http_bearer"))
        .and_then(|entry| match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error),
        })
        .map_err(|_| "Não foi possível substituir o token pessoal salvo.".to_string())?;
    runtime.remove_connection(&server_id)?;
    runtime
        .1
        .lock()
        .map_err(|_| "Estado OAuth MCP indisponível.".to_string())?
        .insert(server_id.clone(), access);
    crate::mcp_registry::set_enabled_internal(&app, &server_id, true)?;
    let _ = app.emit("skills-updated", ());
    Ok(())
}

#[tauri::command]
pub async fn mcp_oauth_registration_options(
    caller: tauri::WebviewWindow,
    endpoint: String,
) -> Result<RegistrationOptions, String> {
    crate::require_main(&caller)?;
    tokio::task::spawn_blocking(move || inspect_registration_options(&endpoint))
        .await
        .map_err(|_| "A verificação das opções OAuth foi interrompida.".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    struct SimulatedRevocationServer {
        statuses: std::collections::VecDeque<Result<StatusCode, ()>>,
        submitted_tokens: Vec<String>,
    }

    impl RevocationTransport for SimulatedRevocationServer {
        fn revoke(
            &mut self,
            endpoint: &Url,
            client_id: &str,
            token: &str,
            _token_type_hint: &str,
        ) -> Result<StatusCode, ()> {
            assert_eq!(endpoint.host_str(), Some("auth.example"));
            assert_eq!(client_id, "public-client");
            self.submitted_tokens.push(token.to_owned());
            self.statuses.pop_front().unwrap_or(Err(()))
        }
    }

    fn stored_oauth() -> StoredOAuth {
        StoredOAuth {
            issuer: "https://auth.example".into(),
            resource: "https://mcp.example/mcp".into(),
            client_id: "public-client".into(),
            token_endpoint: "https://auth.example/token".into(),
            revocation_endpoint: Some("https://auth.example/revoke".into()),
            refresh_token: Some("refresh-secret-fixture".into()),
        }
    }

    #[test]
    fn debug_redacts_refresh_and_access_tokens() {
        let stored = stored_oauth();
        let access = OAuthAccessToken {
            value: "access-secret-fixture".into(),
            expires_at: Instant::now(),
        };
        let debug = format!("{stored:?} {access:?}");
        assert!(!debug.contains("refresh-secret-fixture"));
        assert!(!debug.contains("access-secret-fixture"));
        assert_eq!(debug.matches("[REDACTED]").count(), 2);
    }

    #[test]
    fn consent_denied_does_not_yield_code() {
        assert!(parse_callback_target(
            "/callback?state=s&error=access_denied",
            "s",
            "https://auth.example",
            false
        )
        .unwrap_err()
        .contains("recusado"));
    }

    #[test]
    fn state_mismatch_is_rejected() {
        assert!(parse_callback_target(
            "/callback?state=other&code=c",
            "expected",
            "https://auth.example",
            false
        )
        .unwrap_err()
        .contains("state"));
    }

    #[test]
    fn repeated_callback_parameters_are_rejected() {
        assert!(parse_callback_target(
            "/callback?state=s&state=other&code=c",
            "s",
            "https://auth.example",
            false
        )
        .unwrap_err()
        .contains("repetidos"));
    }

    #[test]
    fn issuer_mismatch_is_rejected_before_code_exchange() {
        assert!(parse_callback_target(
            "/callback?state=s&code=c&iss=https%3A%2F%2Fevil.example",
            "s",
            "https://auth.example",
            false
        )
        .unwrap_err()
        .contains("emissor"));
    }

    #[test]
    fn required_issuer_may_not_be_omitted() {
        assert!(parse_callback_target(
            "/callback?state=s&code=c",
            "s",
            "https://auth.example",
            true
        )
        .unwrap_err()
        .contains("não informou"));
    }

    #[test]
    fn callback_gate_accepts_exactly_once() {
        let gate = CallbackGate(AtomicBool::new(false));
        assert!(gate.claim());
        assert!(!gate.claim());
    }

    #[test]
    fn redirect_is_not_an_authorization_response() {
        assert!(reject_redirect(StatusCode::FOUND)
            .unwrap_err()
            .contains("Redirecionamento"));
        assert!(reject_redirect(StatusCode::OK).is_ok());
    }

    fn registration_metadata(registration_endpoint: Option<&str>) -> ServerMetadata {
        ServerMetadata {
            authorization_endpoint: "https://auth.example/authorize".into(),
            token_endpoint: "https://auth.example/token".into(),
            registration_endpoint: registration_endpoint.map(str::to_owned),
            revocation_endpoint: None,
            client_id_metadata_document_supported: false,
            issuer_parameter_supported: false,
        }
    }

    #[test]
    fn dynamic_registration_does_not_require_client_id_and_uses_returned_id() {
        let metadata = registration_metadata(Some("https://auth.example/register"));
        let mut posted = false;
        let client_id = register_client_with(
            &metadata,
            "http://127.0.0.1:43210/callback",
            None,
            Some("pages:read"),
            |endpoint, body| {
                posted = true;
                assert_eq!(endpoint, "https://auth.example/register");
                assert!(body.get("client_id").is_none());
                assert_eq!(body["scope"], "pages:read");
                Ok(json!({"client_id":"registered-client"}))
            },
        )
        .unwrap();
        assert!(posted);
        assert_eq!(client_id, "registered-client");
    }

    #[test]
    fn missing_dynamic_registration_requires_manual_client_id() {
        let metadata = registration_metadata(None);
        let error = register_client_with(
            &metadata,
            "http://127.0.0.1:43210/callback",
            None,
            None,
            |_, _| panic!("sem endpoint de registro, nenhuma requisição deve ser enviada"),
        )
        .unwrap_err();
        assert!(error.contains("exige um identificador de cliente"));
    }

    #[test]
    fn supplied_client_id_is_used_when_dynamic_registration_is_unavailable() {
        let metadata = registration_metadata(None);
        let client_id = register_client_with(
            &metadata,
            "http://127.0.0.1:43210/callback",
            Some("manual-client"),
            None,
            |_, _| panic!("registro dinâmico não deve ser chamado"),
        )
        .unwrap();
        assert_eq!(client_id, "manual-client");
    }

    #[test]
    fn oauth_redirect_response_is_refused() {
        assert!(reject_redirect(StatusCode::TEMPORARY_REDIRECT)
            .unwrap_err()
            .contains("Redirecionamento"));
    }

    #[test]
    fn discovered_issuer_must_match_metadata_origin() {
        assert!(
            validate_issuer("https://auth.example", "https://attacker.example")
                .unwrap_err()
                .contains("emissor")
        );
        assert!(validate_issuer("https://auth.example", "https://auth.example").is_ok());
    }

    #[test]
    fn expired_access_token_requires_refresh_before_expiration() {
        let now = Instant::now();
        assert!(should_refresh(now - Duration::from_secs(1), now));
        assert!(should_refresh(now + Duration::from_secs(30), now));
        assert!(!should_refresh(now + Duration::from_secs(120), now));
    }

    #[test]
    fn resource_metadata_must_share_the_mcp_origin() {
        let endpoint = Url::parse("https://mcp.example.org/mcp").unwrap();
        let other_host = Url::parse("https://evil.example/.well-known/prm").unwrap();
        let other_port = Url::parse("https://mcp.example.org:444/.well-known/prm").unwrap();
        assert!(!same_origin(&other_host, &endpoint));
        assert!(!same_origin(&other_port, &endpoint));
    }

    #[test]
    fn revocation_succeeds_for_refresh_and_in_memory_access_tokens() {
        let mut server = SimulatedRevocationServer {
            statuses: VecDeque::from(vec![Ok(StatusCode::OK), Ok(StatusCode::OK)]),
            submitted_tokens: Vec::new(),
        };
        assert!(revoke_tokens_with(
            &stored_oauth(),
            Some("access-secret-fixture"),
            &mut server
        ));
        assert_eq!(
            server.submitted_tokens,
            ["refresh-secret-fixture", "access-secret-fixture"]
        );
    }

    #[test]
    fn revocation_failure_does_not_prevent_attempting_the_other_token() {
        let mut server = SimulatedRevocationServer {
            statuses: VecDeque::from(vec![Err(()), Ok(StatusCode::OK)]),
            submitted_tokens: Vec::new(),
        };
        assert!(!revoke_tokens_with(
            &stored_oauth(),
            Some("access-secret-fixture"),
            &mut server
        ));
        assert_eq!(server.submitted_tokens.len(), 2);
    }

    #[test]
    fn removal_without_tokens_does_not_call_revocation_server() {
        let mut stored = stored_oauth();
        stored.refresh_token = None;
        let mut server = SimulatedRevocationServer {
            statuses: VecDeque::new(),
            submitted_tokens: Vec::new(),
        };
        assert!(revoke_tokens_with(&stored, None, &mut server));
        assert!(server.submitted_tokens.is_empty());
    }

    #[test]
    fn revocation_failure_warning_never_contains_the_token() {
        let warning = removal_warning(false).unwrap();
        assert!(!warning.contains("refresh-secret-fixture"));
        assert!(!warning.contains("access-secret-fixture"));
    }
}
