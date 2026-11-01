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
const GOOGLE_OAUTH_CALLBACK_PORT: u16 = 43117;
const MAX_METADATA_BYTES: usize = 256 * 1024;
const MAX_GITHUB_PROFILE_BYTES: usize = 64 * 1024;
const OAUTH_CLIENTS_JSON: &str = include_str!("../resources/oauth-clients.json");
const GITHUB_API_USER: &str = "https://api.github.com/user";
const GITHUB_API_REVOKE_GRANT: &str = "https://api.github.com/applications/";

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OAuthClientCatalog {
    allowed_guide_domains: Vec<String>,
    services: std::collections::HashMap<String, OAuthClientDefinition>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OAuthClientDefinition {
    client_id: String,
    guide_url: String,
    dynamic_registration: bool,
    client_secret_required: bool,
    #[serde(default)]
    oauth_scopes_by_endpoint: std::collections::HashMap<String, Vec<String>>,
    endpoints: Vec<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicOAuthClientSettings {
    pub service: String,
    pub client_id: String,
    pub guide_url: String,
    pub dynamic_registration: bool,
    pub client_secret_required: bool,
    pub client_secret_saved: bool,
}

fn oauth_client_catalog() -> Result<OAuthClientCatalog, String> {
    let catalog: OAuthClientCatalog = serde_json::from_str(OAUTH_CLIENTS_JSON)
        .map_err(|_| "A configuração de clientes OAuth do aplicativo está inválida.".to_string())?;
    for definition in catalog.services.values() {
        validate_guide_url(&definition.guide_url, &catalog.allowed_guide_domains)?;
        if !definition.client_id.is_empty()
            && (definition.client_id.len() > 2048
                || definition.client_id.chars().any(char::is_control))
        {
            return Err("Um identificador público de cliente OAuth é inválido.".into());
        }
        for endpoint in &definition.endpoints {
            let parsed = Url::parse(endpoint)
                .map_err(|_| "Um endpoint OAuth público está inválido.".to_string())?;
            if parsed.scheme() != "https"
                || parsed.host_str().is_none()
                || !parsed.username().is_empty()
                || parsed.password().is_some()
            {
                return Err("Os endpoints OAuth públicos precisam usar HTTPS.".into());
            }
        }
    }
    Ok(catalog)
}

fn validate_guide_url(url: &str, allowed_domains: &[String]) -> Result<(), String> {
    let guide = Url::parse(url).map_err(|_| "A URL de um guia OAuth está inválida.".to_string())?;
    if guide.scheme() != "https"
        || !guide
            .host_str()
            .is_some_and(|host| allowed_domains.iter().any(|allowed| allowed == host))
    {
        return Err("A URL de um guia OAuth precisa usar HTTPS em domínio permitido.".into());
    }
    Ok(())
}

fn oauth_service_for_endpoint(
    endpoint: &str,
) -> Result<Option<(String, OAuthClientDefinition)>, String> {
    let catalog = oauth_client_catalog()?;
    for (service, definition) in catalog.services {
        if definition
            .endpoints
            .iter()
            .any(|candidate| candidate == endpoint)
        {
            return Ok(Some((service, definition)));
        }
    }
    Ok(None)
}

fn public_client_secret_credential(service: &str) -> Result<Entry, String> {
    Entry::new(SERVICE, &format!("public-oauth:{service}:client_secret"))
        .map_err(|_| "Não foi possível acessar o Gerenciador de Credenciais do Windows.".into())
}

fn load_public_client_secret(service: &str) -> Result<Option<String>, String> {
    if !oauth_client_catalog()?.services.contains_key(service) {
        return Err("Serviço OAuth público desconhecido.".into());
    }
    match public_client_secret_credential(service)?.get_password() {
        Ok(secret) => Ok(Some(secret)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err("Não foi possível ler o segredo OAuth no Gerenciador de Credenciais.".into()),
    }
}

fn store_public_client_secret(service: &str, secret: &str) -> Result<(), String> {
    public_client_secret_credential(service)?
        .set_password(secret)
        .map_err(|_| {
            "Não foi possível salvar o segredo no Gerenciador de Credenciais do Windows.".into()
        })
}

fn remove_public_client_secret(service: &str) -> Result<(), String> {
    match public_client_secret_credential(service)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err("Não foi possível remover o segredo do Gerenciador de Credenciais.".into()),
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredOAuth {
    pub issuer: String,
    pub resource: String,
    pub client_id: String,
    #[serde(default)]
    pub client_secret: Option<String>,
    #[serde(default)]
    pub service: Option<String>,
    #[serde(default)]
    pub github_login: Option<String>,
    pub token_endpoint: String,
    #[serde(default)]
    pub revocation_endpoint: Option<String>,
    pub refresh_token: Option<String>,
}

impl std::fmt::Debug for StoredOAuth {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StoredOAuth")
            .field("issuer", &"[URL OMITIDA]")
            .field("resource", &"[URL OMITIDA]")
            .field("client_id", &self.client_id)
            .field(
                "client_secret",
                &self.client_secret.as_ref().map(|_| "[REDACTED]"),
            )
            .field("token_endpoint", &"[URL OMITIDA]")
            .field("revocation_endpoint", &"[URL OMITIDA]")
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

struct ServerMetadata {
    authorization_endpoint: String,
    token_endpoint: String,
    registration_endpoint: Option<String>,
    revocation_endpoint: Option<String>,
    client_id_metadata_document_supported: bool,
    issuer_parameter_supported: bool,
}

impl std::fmt::Debug for ServerMetadata {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ServerMetadata")
            .field("authorization_endpoint", &"[URL OMITIDA]")
            .field("token_endpoint", &"[URL OMITIDA]")
            .field(
                "registration_endpoint",
                &self.registration_endpoint.as_ref().map(|_| "[URL OMITIDA]"),
            )
            .field(
                "revocation_endpoint",
                &self.revocation_endpoint.as_ref().map(|_| "[URL OMITIDA]"),
            )
            .field(
                "client_id_metadata_document_supported",
                &self.client_id_metadata_document_supported,
            )
            .field(
                "issuer_parameter_supported",
                &self.issuer_parameter_supported,
            )
            .finish()
    }
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

struct GitHubProfileReply {
    status: StatusCode,
    url: Url,
    body: Vec<u8>,
}

trait GitHubProfileTransport {
    fn get_user(&mut self, token: &str) -> Result<GitHubProfileReply, String>;
}

trait GitHubGrantRevocationTransport {
    fn revoke_grant(
        &mut self,
        client_id: &str,
        client_secret: &str,
        access_token: &str,
    ) -> Result<StatusCode, ()>;
}

struct ReqwestGitHubProfileTransport(Client);

impl GitHubProfileTransport for ReqwestGitHubProfileTransport {
    fn get_user(&mut self, token: &str) -> Result<GitHubProfileReply, String> {
        let mut response = self
            .0
            .get(GITHUB_API_USER)
            .bearer_auth(token)
            .header(reqwest::header::ACCEPT, "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header("User-Agent", concat!("OdeKrai/", env!("CARGO_PKG_VERSION")))
            .send()
            .map_err(|_| "Não foi possível identificar a conta GitHub autorizada.".to_string())?;
        let status = response.status();
        let url = response.url().clone();
        let mut body = Vec::new();
        response
            .by_ref()
            .take((MAX_GITHUB_PROFILE_BYTES + 1) as u64)
            .read_to_end(&mut body)
            .map_err(|_| "Não foi possível ler a conta GitHub autorizada.".to_string())?;
        Ok(GitHubProfileReply { status, url, body })
    }
}

struct ReqwestGitHubGrantRevocationTransport(Client);

impl GitHubGrantRevocationTransport for ReqwestGitHubGrantRevocationTransport {
    fn revoke_grant(
        &mut self,
        client_id: &str,
        client_secret: &str,
        access_token: &str,
    ) -> Result<StatusCode, ()> {
        let mut endpoint = Url::parse(GITHUB_API_REVOKE_GRANT).map_err(|_| ())?;
        endpoint
            .path_segments_mut()
            .map_err(|_| ())?
            .push(client_id)
            .push("grant");
        let response = self
            .0
            .delete(endpoint.clone())
            .basic_auth(client_id, Some(client_secret))
            .header(reqwest::header::ACCEPT, "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2026-03-10")
            .header("User-Agent", concat!("OdeKrai/", env!("CARGO_PKG_VERSION")))
            .json(&json!({ "access_token": access_token }))
            .send()
            .map_err(|_| ())?;
        if response.url() != &endpoint || response.status().is_redirection() {
            return Err(());
        }
        Ok(response.status())
    }
}

fn github_account_with(
    token: &str,
    transport: &mut impl GitHubProfileTransport,
) -> Result<String, String> {
    let response = transport.get_user(token)?;
    if response.url.as_str() != GITHUB_API_USER || response.status.is_redirection() {
        return Err(
            "O GitHub tentou redirecionar a consulta da conta; a resposta foi recusada.".into(),
        );
    }
    if response.status != StatusCode::OK {
        return Err(
            "O GitHub não permitiu identificar a conta autorizada. Reconecte e confira a permissão read:user."
                .into(),
        );
    }
    if response.body.len() > MAX_GITHUB_PROFILE_BYTES {
        return Err("A resposta de perfil do GitHub excedeu o limite seguro.".into());
    }
    let body: Value = serde_json::from_slice(&response.body)
        .map_err(|_| "A resposta de perfil do GitHub está inválida.".to_string())?;
    let login = body
        .get("login")
        .and_then(Value::as_str)
        .filter(|login| {
            !login.is_empty()
                && login.len() <= 39
                && login
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
        .ok_or("O GitHub não retornou um nome de conta válido.")?;
    Ok(login.to_owned())
}

fn github_grant_revoked_with(
    client_id: &str,
    client_secret: &str,
    access_token: &str,
    transport: &mut impl GitHubGrantRevocationTransport,
) -> bool {
    transport
        .revoke_grant(client_id, client_secret, access_token)
        .is_ok_and(|status| status == StatusCode::NO_CONTENT)
}

pub(crate) fn github_oauth_read_scopes() -> &'static str {
    "read:user"
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
    let dynamic_registration = oauth_service_for_endpoint(endpoint.as_str())?
        .map(|(_, config)| config.dynamic_registration && metadata.registration_endpoint.is_some())
        .unwrap_or_else(|| metadata.registration_endpoint.is_some());
    Ok(RegistrationOptions {
        dynamic_registration,
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
    Ok(Some(
        scopes.split_whitespace().collect::<Vec<_>>().join(" "),
    ))
}

fn service_scopes(
    endpoint: &str,
    config: Option<&OAuthClientDefinition>,
    requested_scopes: Option<&str>,
) -> Result<Option<String>, String> {
    if let Some(scopes) =
        config.and_then(|definition| definition.oauth_scopes_by_endpoint.get(endpoint))
    {
        if scopes.is_empty()
            || scopes
                .iter()
                .any(|scope| scope.is_empty() || scope.chars().any(char::is_control))
        {
            return Err("A configuração de escopos OAuth do Google está inválida.".into());
        }
        return Ok(Some(scopes.join(" ")));
    }
    normalize_scopes(requested_scopes)
}

fn missing_client_secret_error(client_secret_required: bool, saved: bool) -> Option<&'static str> {
    (client_secret_required && !saved).then_some(
        "Salve o segredo OAuth deste serviço no Gerenciador de Credenciais antes de conectar.",
    )
}

fn refresh_failure_message(
    status: StatusCode,
    oauth_error: Option<&str>,
    google_workspace: bool,
) -> &'static str {
    if google_workspace
        && (status == StatusCode::UNAUTHORIZED || oauth_error == Some("invalid_grant"))
    {
        "A autorização foi revogada ou expirou. Conecte novamente este serviço Google para continuar."
    } else {
        "A renovação OAuth falhou. Conecte novamente o servidor MCP."
    }
}

fn parse_refresh_response(
    status: StatusCode,
    body: Value,
    google_workspace: bool,
) -> Result<TokenResponse, String> {
    let parsed_tokens = serde_json::from_value::<TokenResponse>(body).ok();
    if status == StatusCode::UNAUTHORIZED || !status.is_success() {
        let oauth_error = parsed_tokens
            .as_ref()
            .and_then(|tokens| tokens.error.as_deref());
        return Err(refresh_failure_message(status, oauth_error, google_workspace).into());
    }
    let tokens = parsed_tokens.ok_or("Resposta de renovação OAuth inválida. Conecte novamente.")?;
    if let Some(error) = tokens.error.as_deref() {
        return Err(refresh_failure_message(status, Some(error), google_workspace).into());
    }
    if !tokens
        .token_type
        .as_deref()
        .is_some_and(|kind| kind.eq_ignore_ascii_case("bearer"))
    {
        return Err("A renovação OAuth foi recusada. Conecte novamente o servidor MCP.".into());
    }
    Ok(tokens)
}

fn loopback_callback_uri(port: u16) -> String {
    format!("http://127.0.0.1:{port}/callback")
}

fn register_client(
    client: &Client,
    metadata: &ServerMetadata,
    redirect_uri: &str,
    supplied_client_id: Option<&str>,
    scopes: Option<&str>,
) -> Result<RegisteredOAuthClient, String> {
    register_client_with(
        metadata,
        redirect_uri,
        supplied_client_id,
        scopes,
        |endpoint, body| post_json(client, endpoint, body),
    )
}

struct RegisteredOAuthClient {
    client_id: String,
    client_secret: Option<String>,
}

impl std::fmt::Debug for RegisteredOAuthClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RegisteredOAuthClient")
            .field("client_id", &self.client_id)
            .field(
                "client_secret",
                &self.client_secret.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

fn register_client_with(
    metadata: &ServerMetadata,
    redirect_uri: &str,
    supplied_client_id: Option<&str>,
    scopes: Option<&str>,
    mut post: impl FnMut(&str, Value) -> Result<Value, String>,
) -> Result<RegisteredOAuthClient, String> {
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
        let client_id: Result<String, String> = response
            .get("client_id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty() && id.len() <= 2048 && !id.chars().any(char::is_control))
            .map(str::to_owned)
            .ok_or_else(|| "O registro OAuth não retornou um client_id válido.".into());
        let client_id = client_id?;
        let client_secret = response
            .get("client_secret")
            .and_then(Value::as_str)
            .map(str::to_owned);
        if client_secret.as_ref().is_some_and(|secret| {
            secret.is_empty() || secret.len() > 16_384 || secret.chars().any(char::is_control)
        }) {
            return Err("O registro OAuth retornou um segredo de cliente inválido.".into());
        }
        return Ok(RegisteredOAuthClient {
            client_id,
            client_secret,
        });
    }
    if let Some(client_id) = supplied_client_id {
        return Ok(RegisteredOAuthClient {
            client_id: validate_client_id(client_id)?,
            client_secret: None,
        });
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
    google_workspace: bool,
) -> Result<String, String> {
    let mut url = Url::parse(&metadata.authorization_endpoint)
        .map_err(|_| "authorization_endpoint inválido.".to_string())?;
    if google_workspace {
        let retained = url
            .query_pairs()
            .filter(|(key, _)| {
                !matches!(
                    key.as_ref(),
                    "scope" | "include_granted_scopes" | "access_type" | "prompt"
                )
            })
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect::<Vec<_>>();
        url.set_query(None);
        for (key, value) in retained {
            url.query_pairs_mut().append_pair(&key, &value);
        }
    }
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
    if google_workspace {
        url.query_pairs_mut()
            .append_pair("access_type", "offline")
            .append_pair("prompt", "consent");
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

impl std::fmt::Debug for TokenResponse {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TokenResponse")
            .field(
                "access_token",
                &self.access_token.as_ref().map(|_| "[REDACTED]"),
            )
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "[REDACTED]"),
            )
            .field("token_type", &self.token_type)
            .field("expires_in", &self.expires_in)
            .field("error", &self.error.as_ref().map(|_| "[OMITTED]"))
            .finish()
    }
}

fn exchange_code(
    client: &Client,
    metadata: &ServerMetadata,
    client_id: &str,
    code: &str,
    redirect_uri: &str,
    verifier: &str,
    resource: &str,
    client_secret: Option<&str>,
) -> Result<(String, Option<String>, Instant), String> {
    let endpoint = validate_https(&metadata.token_endpoint)?;
    let mut form = vec![
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("client_id", client_id),
        ("code_verifier", verifier),
        ("resource", resource),
    ];
    if let Some(secret) = client_secret {
        form.push(("client_secret", secret));
    }
    let response = client
        .post(endpoint.clone())
        .form(&form)
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
    let client_secret = if let Some(secret) = stored.client_secret.clone() {
        Some(secret)
    } else {
        stored
            .service
            .as_deref()
            .map(load_public_client_secret)
            .transpose()?
            .flatten()
    };
    let mut form = vec![
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token.as_str()),
        ("client_id", stored.client_id.as_str()),
        ("resource", stored.resource.as_str()),
    ];
    if let Some(secret) = client_secret.as_deref() {
        form.push(("client_secret", secret));
    }
    let response = client
        .post(endpoint.clone())
        .form(&form)
        .send()
        .map_err(|_| {
            "Falha de rede ao renovar a autorização MCP. Conecte novamente.".to_string()
        })?;
    reject_redirect(response.status())?;
    if response.url() != &endpoint {
        return Err("Redirecionamento na renovação OAuth foi recusado. Conecte novamente.".into());
    }
    let (status, _, body) = bounded_json(response)?;
    let tokens = parse_refresh_response(status, body, stored.service.as_deref() == Some("google"))?;
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
        return false;
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
    if tokens.is_empty() {
        return false;
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
    if stored.service.as_deref() == Some("github") {
        let client_secret = load_public_client_secret("github")
            .ok()
            .flatten()
            .or_else(|| stored.client_secret.clone());
        let Some(client_secret) = client_secret else {
            return false;
        };
        let Ok(client) = safe_http() else {
            return false;
        };
        let mut transport = ReqwestGitHubGrantRevocationTransport(client);
        if access_token.is_some_and(|token| {
            github_grant_revoked_with(&stored.client_id, &client_secret, token, &mut transport)
        }) {
            return true;
        }
        let Ok(refreshed) = refresh_access(server_id) else {
            return false;
        };
        return github_grant_revoked_with(
            &stored.client_id,
            &client_secret,
            &refreshed.value,
            &mut transport,
        );
    }
    if stored.revocation_endpoint.is_none() {
        return false;
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
    let (service, service_config) = oauth_service_for_endpoint(endpoint.as_str())?
        .map(|(service, config)| (Some(service), Some(config)))
        .unwrap_or((None, None));
    let (effective_client_id, client_secret) = if let (Some(service), Some(config)) =
        (service.as_deref(), service_config.as_ref())
    {
        let configured_id = config.client_id.trim();
        if configured_id.is_empty() && !config.dynamic_registration {
            return Err(format!(
                "Este serviço ainda não foi configurado para o OdeKrai. Consulte o guia oficial: {}.",
                config.guide_url
            ));
        }
        let secret = if config.client_secret_required && !configured_id.is_empty() {
            let secret = load_public_client_secret(service)?;
            if let Some(message) =
                missing_client_secret_error(config.client_secret_required, secret.is_some())
            {
                return Err(message.into());
            }
            secret
        } else {
            None
        };
        (
            (!configured_id.is_empty()).then(|| configured_id.to_owned()),
            secret,
        )
    } else {
        (supplied_client_id.map(str::to_owned), None)
    };
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
    let (issuer, mut metadata) = discover_authorization_server(&client, &resource_metadata)?;
    if service_config
        .as_ref()
        .is_some_and(|config| !config.dynamic_registration || effective_client_id.is_some())
    {
        metadata.registration_endpoint = None;
    }
    let callback_port = if service.as_deref() == Some("google") {
        GOOGLE_OAUTH_CALLBACK_PORT
    } else {
        0
    };
    let listener = TcpListener::bind(("127.0.0.1", callback_port))
        .map_err(|_| "Não foi possível abrir a porta local de retorno OAuth.".to_string())?;
    let port = listener
        .local_addr()
        .map_err(|_| "Não foi possível ler a porta local OAuth.".to_string())?
        .port();
    let redirect_uri = loopback_callback_uri(port);
    let requested_scopes =
        service_scopes(endpoint.as_str(), service_config.as_ref(), requested_scopes)?;
    let registered_client = register_client(
        &client,
        &metadata,
        &redirect_uri,
        effective_client_id.as_deref(),
        requested_scopes.as_deref(),
    )?;
    let discovered_scope = scope_from_challenge(&challenge, &resource_metadata);
    let effective_scope = requested_scopes.as_deref().or(discovered_scope.as_deref());
    let verifier = random_token()?;
    let state = random_token()?;
    let auth_url = build_authorization_url(
        &metadata,
        &registered_client.client_id,
        &redirect_uri,
        &state,
        &pkce_challenge(&verifier),
        &resource,
        effective_scope,
        service.as_deref() == Some("google"),
    )?;
    crate::external_links::open_oauth_authorization_url(&app, &auth_url)?;
    let code = accept_callback(
        listener,
        &state,
        &issuer,
        metadata.issuer_parameter_supported,
    )?;
    let effective_client_secret = registered_client
        .client_secret
        .as_deref()
        .or(client_secret.as_deref());
    let (access_token, refresh_token, expires_at) = exchange_code(
        &client,
        &metadata,
        &registered_client.client_id,
        &code,
        &redirect_uri,
        &verifier,
        &resource,
        effective_client_secret,
    )?;
    let github_login = if service.as_deref() == Some("github") {
        Some(github_account_with(
            &access_token,
            &mut ReqwestGitHubProfileTransport(client.clone()),
        )?)
    } else {
        None
    };
    let stored = StoredOAuth {
        issuer,
        resource,
        client_id: registered_client.client_id,
        client_secret: registered_client.client_secret,
        service,
        github_login,
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
    let (_credential, access) = tokio::task::spawn_blocking(move || {
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

#[tauri::command]
pub fn mcp_oauth_public_client_settings(
    caller: tauri::WebviewWindow,
    service: String,
) -> Result<PublicOAuthClientSettings, String> {
    crate::require_main(&caller)?;
    let catalog = oauth_client_catalog()?;
    let definition = catalog
        .services
        .get(&service)
        .ok_or("Serviço OAuth público desconhecido.")?;
    Ok(PublicOAuthClientSettings {
        service: service.clone(),
        client_id: definition.client_id.clone(),
        guide_url: definition.guide_url.clone(),
        dynamic_registration: definition.dynamic_registration,
        client_secret_required: definition.client_secret_required,
        client_secret_saved: load_public_client_secret(&service)?.is_some(),
    })
}

#[tauri::command]
pub fn mcp_oauth_github_account(
    caller: tauri::WebviewWindow,
    server_id: String,
) -> Result<Option<String>, String> {
    crate::require_main(&caller)?;
    let Some(stored) = load(&server_id)? else {
        return Ok(None);
    };
    if stored.service.as_deref() != Some("github") {
        return Ok(None);
    }
    Ok(stored.github_login)
}

#[tauri::command]
pub fn mcp_oauth_save_public_client_secret(
    caller: tauri::WebviewWindow,
    service: String,
    secret: String,
) -> Result<(), String> {
    crate::require_main(&caller)?;
    let catalog = oauth_client_catalog()?;
    let definition = catalog
        .services
        .get(&service)
        .ok_or("Serviço OAuth público desconhecido.")?;
    if !definition.client_secret_required || definition.client_id.trim().is_empty() {
        return Err("Este serviço ainda não aceita um segredo OAuth neste aplicativo.".into());
    }
    if secret.trim().is_empty() || secret.len() > 16_384 || secret.chars().any(char::is_control) {
        return Err("Informe um segredo OAuth válido.".into());
    }
    store_public_client_secret(&service, &secret)
}

#[tauri::command]
pub fn mcp_oauth_remove_public_client_secret(
    caller: tauri::WebviewWindow,
    service: String,
) -> Result<(), String> {
    crate::require_main(&caller)?;
    let catalog = oauth_client_catalog()?;
    if !catalog.services.contains_key(&service) {
        return Err("Serviço OAuth público desconhecido.".into());
    }
    remove_public_client_secret(&service)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockGitHubProfile(GitHubProfileReply, Option<String>);

    struct MockGitHubGrantRevocation {
        status: Result<StatusCode, ()>,
        last_token: Option<String>,
        last_client_id: Option<String>,
    }

    impl GitHubGrantRevocationTransport for MockGitHubGrantRevocation {
        fn revoke_grant(
            &mut self,
            client_id: &str,
            _client_secret: &str,
            access_token: &str,
        ) -> Result<StatusCode, ()> {
            self.last_client_id = Some(client_id.to_owned());
            self.last_token = Some(access_token.to_owned());
            self.status
        }
    }

    impl GitHubProfileTransport for MockGitHubProfile {
        fn get_user(&mut self, token: &str) -> Result<GitHubProfileReply, String> {
            if let Some(expected) = self.1.as_deref() {
                assert_eq!(token, expected);
            }
            Ok(GitHubProfileReply {
                status: self.0.status,
                url: self.0.url.clone(),
                body: self.0.body.clone(),
            })
        }
    }

    fn mock_github_profile(status: StatusCode, body: &[u8]) -> MockGitHubProfile {
        MockGitHubProfile(
            GitHubProfileReply {
                status,
                url: Url::parse(GITHUB_API_USER).unwrap(),
                body: body.to_vec(),
            },
            Some("access-token-fixture".into()),
        )
    }

    #[test]
    fn github_oauth_uses_only_read_user_scope_by_default() {
        assert_eq!(github_oauth_read_scopes(), "read:user");
        assert!(!github_oauth_read_scopes()
            .split_whitespace()
            .any(|scope| { matches!(scope, "repo" | "write:repo_hook" | "admin:org" | "user") }));
    }

    #[test]
    fn github_account_is_read_from_simulated_profile_without_logging_token() {
        let mut profile = mock_github_profile(StatusCode::OK, br#"{"login":"octocat"}"#);
        assert_eq!(
            github_account_with("access-token-fixture", &mut profile).unwrap(),
            "octocat"
        );
        let mut denied =
            mock_github_profile(StatusCode::FORBIDDEN, b"access-token-fixture was rejected");
        let error = github_account_with("access-token-fixture", &mut denied).unwrap_err();
        assert!(!error.contains("access-token-fixture"));
        assert!(!error.contains("was rejected"));
    }

    #[test]
    fn github_account_rejects_redirect_and_invalid_login() {
        let mut redirected = mock_github_profile(StatusCode::FOUND, br#"{"login":"octocat"}"#);
        redirected.0.url = Url::parse("https://attacker.example/user").unwrap();
        assert!(github_account_with("token", &mut redirected)
            .unwrap_err()
            .contains("redirecionar"));
        let mut invalid = mock_github_profile(StatusCode::OK, br#"{"login":"octo cat"}"#);
        assert!(github_account_with("token", &mut invalid).is_err());
    }

    #[test]
    fn github_disconnect_revokes_token_with_the_registered_app_credentials() {
        let mut server = MockGitHubGrantRevocation {
            status: Ok(StatusCode::NO_CONTENT),
            last_token: None,
            last_client_id: None,
        };
        assert!(github_grant_revoked_with(
            "github-client-id",
            "github-client-secret",
            "github-access-token",
            &mut server,
        ));
        assert_eq!(server.last_client_id.as_deref(), Some("github-client-id"));
        assert_eq!(server.last_token.as_deref(), Some("github-access-token"));
    }

    #[test]
    fn github_disconnect_does_not_claim_revocation_without_confirmation() {
        let mut server = MockGitHubGrantRevocation {
            status: Ok(StatusCode::UNAUTHORIZED),
            last_token: None,
            last_client_id: None,
        };
        assert!(!github_grant_revoked_with(
            "github-client-id",
            "github-client-secret",
            "github-access-token",
            &mut server,
        ));
        let mut unavailable = MockGitHubGrantRevocation {
            status: Err(()),
            last_token: None,
            last_client_id: None,
        };
        assert!(!github_grant_revoked_with(
            "github-client-id",
            "github-client-secret",
            "github-access-token",
            &mut unavailable,
        ));
    }
    use std::collections::VecDeque;

    #[test]
    fn embedded_public_oauth_clients_have_no_secrets_and_only_allowlisted_https_guides() {
        let catalog = oauth_client_catalog().expect("catálogo OAuth válido");
        assert_eq!(catalog.services.len(), 5);
        for name in ["github", "figma", "lovable", "notion", "google"] {
            assert!(
                catalog.services.contains_key(name),
                "serviço ausente: {name}"
            );
        }
        assert!(catalog.services["github"].client_secret_required);
        for definition in catalog.services.values() {
            assert!(definition.client_id.is_empty());
            validate_guide_url(&definition.guide_url, &catalog.allowed_guide_domains).unwrap();
        }
        let raw: Value = serde_json::from_str(OAUTH_CLIENTS_JSON).unwrap();
        for service in raw["services"].as_object().unwrap().values() {
            assert!(service.get("clientSecret").is_none());
            assert!(service.get("client_secret").is_none());
        }
        let public_view = serde_json::to_string(&PublicOAuthClientSettings {
            service: "google".into(),
            client_id: String::new(),
            guide_url: concat!(
                "https://developers.google.com/workspace/",
                "guides/configure-mcp-servers"
            )
            .into(),
            dynamic_registration: false,
            client_secret_required: true,
            client_secret_saved: true,
        })
        .unwrap();
        assert!(!public_view.contains("client-secret-fixture"));
        for secret_field in [
            "accessToken",
            "refreshToken",
            "clientSecret",
            "authorizationCode",
            "codeVerifier",
        ] {
            assert!(!public_view.contains(secret_field));
        }
    }

    #[test]
    fn oauth_guide_urls_reject_http_and_unlisted_hosts() {
        let allowed = vec!["docs.example.test".to_string()];
        assert!(validate_guide_url("http://docs.example.test/oauth", &allowed).is_err());
        assert!(validate_guide_url("https://attacker.example/oauth", &allowed).is_err());
        assert!(validate_guide_url("https://docs.example.test/oauth", &allowed).is_ok());
    }

    #[test]
    fn oauth_public_service_mapping_uses_exact_catalog_endpoints() {
        assert_eq!(
            oauth_service_for_endpoint("https://sheetsmcp.googleapis.com/mcp/v1")
                .unwrap()
                .map(|(service, _)| service),
            Some("google".to_string())
        );
        assert!(oauth_service_for_endpoint("https://evil.example/mcp")
            .unwrap()
            .is_none());
    }

    #[test]
    fn google_services_use_fixed_minimal_independent_scopes_and_ignore_ui_scopes() {
        let cases = [
            (
                "https://gmailmcp.googleapis.com/mcp/v1",
                "https://www.googleapis.com/auth/gmail.readonly https://www.googleapis.com/auth/gmail.compose",
            ),
            (
                "https://calendarmcp.googleapis.com/mcp/v1",
                "https://www.googleapis.com/auth/calendar.calendarlist.readonly https://www.googleapis.com/auth/calendar.events.freebusy https://www.googleapis.com/auth/calendar.events.readonly",
            ),
            (
                "https://sheetsmcp.googleapis.com/mcp/v1",
                "https://www.googleapis.com/auth/spreadsheets.readonly",
            ),
        ];
        for (endpoint, expected) in cases {
            let (_, definition) = oauth_service_for_endpoint(endpoint)
                .unwrap()
                .expect("endpoint Google conhecido");
            assert_eq!(
                service_scopes(
                    endpoint,
                    Some(&definition),
                    Some("https://evil.example/full")
                )
                .unwrap()
                .as_deref(),
                Some(expected),
                "escopos indevidos em {endpoint}"
            );
        }
    }

    #[test]
    fn google_authorization_requests_offline_consent_without_incremental_scope_parameter() {
        let mut metadata = registration_metadata(None);
        metadata.authorization_endpoint =
            "https://auth.example/authorize?include_granted_scopes=true&scope=evil".into();
        let authorization = build_authorization_url(
            &metadata,
            "google-client",
            "http://127.0.0.1:43117/callback",
            "state-fixture",
            "pkce-fixture",
            "https://gmailmcp.googleapis.com/mcp/v1",
            Some(
                "https://www.googleapis.com/auth/gmail.readonly https://www.googleapis.com/auth/gmail.compose",
            ),
            true,
        )
        .unwrap();
        let url = Url::parse(&authorization).unwrap();
        let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(
            query.get("access_type").map(String::as_str),
            Some("offline")
        );
        assert_eq!(query.get("prompt").map(String::as_str), Some("consent"));
        assert!(!query.contains_key("include_granted_scopes"));
        assert_eq!(
            query.get("scope").map(String::as_str),
            Some("https://www.googleapis.com/auth/gmail.readonly https://www.googleapis.com/auth/gmail.compose")
        );
        assert_eq!(
            loopback_callback_uri(GOOGLE_OAUTH_CALLBACK_PORT),
            "http://127.0.0.1:43117/callback"
        );
    }

    #[test]
    fn missing_google_client_secret_blocks_authorization_with_clear_message() {
        assert_eq!(
            missing_client_secret_error(true, false),
            Some(
                "Salve o segredo OAuth deste serviço no Gerenciador de Credenciais antes de conectar."
            )
        );
        assert_eq!(missing_client_secret_error(true, true), None);
        assert_eq!(missing_client_secret_error(false, false), None);
    }

    #[test]
    fn revoked_google_refresh_token_requires_a_new_authorization() {
        let response = parse_refresh_response(
            StatusCode::BAD_REQUEST,
            json!({"error":"invalid_grant"}),
            true,
        )
        .err()
        .expect("token revogado deve solicitar nova autorização");
        assert!(response.contains("revogada ou expirou"));
        assert!(response.contains("Conecte novamente"));
    }

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
            client_secret: None,
            service: None,
            github_login: None,
            token_endpoint: "https://auth.example/token".into(),
            revocation_endpoint: Some("https://auth.example/revoke".into()),
            refresh_token: Some("refresh-secret-fixture".into()),
        }
    }

    #[test]
    fn older_oauth_credential_without_github_account_still_loads() {
        let mut value = serde_json::to_value(stored_oauth()).unwrap();
        value.as_object_mut().unwrap().remove("githubLogin");
        let stored: StoredOAuth = serde_json::from_value(value).unwrap();
        assert_eq!(stored.github_login, None);
    }

    #[test]
    fn debug_redacts_refresh_and_access_tokens() {
        let mut stored = stored_oauth();
        stored.client_secret = Some("client-secret-fixture".into());
        let access = OAuthAccessToken {
            value: "access-secret-fixture".into(),
            expires_at: Instant::now(),
        };
        let response = TokenResponse {
            access_token: Some("response-access-fixture".into()),
            refresh_token: Some("response-refresh-fixture".into()),
            token_type: Some("Bearer".into()),
            expires_in: Some(3600),
            error: None,
        };
        let registered = RegisteredOAuthClient {
            client_id: "public-client".into(),
            client_secret: Some("registered-secret-fixture".into()),
        };
        let debug = format!("{stored:?} {access:?} {response:?} {registered:?}");
        assert!(!debug.contains("refresh-secret-fixture"));
        assert!(!debug.contains("client-secret-fixture"));
        assert!(!debug.contains("access-secret-fixture"));
        assert!(!debug.contains("response-access-fixture"));
        assert!(!debug.contains("response-refresh-fixture"));
        assert!(!debug.contains("registered-secret-fixture"));
        assert_eq!(debug.matches("[REDACTED]").count(), 6);
    }

    #[test]
    fn debug_omits_server_urls_that_could_contain_query_credentials() {
        let mut stored = stored_oauth();
        stored.issuer = "https://auth.example/issuer?access_token=url-access-fixture".into();
        stored.resource = "https://mcp.example/mcp?token=url-token-fixture".into();
        stored.token_endpoint =
            "https://auth.example/token?code_verifier=url-verifier-fixture".into();
        stored.revocation_endpoint =
            Some("https://auth.example/revoke?client_secret=url-secret-fixture".into());
        let metadata = ServerMetadata {
            authorization_endpoint: "https://auth.example/authorize?code=auth-code-fixture".into(),
            token_endpoint: "https://auth.example/token?access_token=url-access-fixture".into(),
            registration_endpoint: Some(
                "https://auth.example/register?refresh_token=url-refresh-fixture".into(),
            ),
            revocation_endpoint: Some(
                "https://auth.example/revoke?client_secret=url-secret-fixture".into(),
            ),
            client_id_metadata_document_supported: false,
            issuer_parameter_supported: false,
        };
        let debug = format!("{stored:?} {metadata:?}");

        for secret in [
            "url-access-fixture",
            "url-token-fixture",
            "url-verifier-fixture",
            "url-secret-fixture",
            "auth-code-fixture",
            "url-refresh-fixture",
        ] {
            assert!(!debug.contains(secret), "segredo de URL exposto: {secret}");
        }
        assert!(debug.contains("URL OMITIDA"));
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
    fn github_authorization_requests_read_user_only() {
        let metadata = registration_metadata(None);
        let authorization = build_authorization_url(
            &metadata,
            "github-client",
            "http://127.0.0.1:43210/callback",
            "state-fixture",
            "pkce-fixture",
            "https://api.githubcopilot.com/mcp/",
            Some(github_oauth_read_scopes()),
            false,
        )
        .unwrap();
        let url = Url::parse(&authorization).unwrap();
        let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(query.get("scope").map(String::as_str), Some("read:user"));
        assert_eq!(
            query.get("code_challenge_method").map(String::as_str),
            Some("S256")
        );
        assert_eq!(
            query.get("state").map(String::as_str),
            Some("state-fixture")
        );
    }

    #[test]
    fn github_simulated_authorization_server_flow_keeps_state_and_pkce() {
        let metadata = registration_metadata(None);
        let client = register_client_with(
            &metadata,
            "http://127.0.0.1:43210/callback",
            Some("github-client"),
            Some(github_oauth_read_scopes()),
            |_, _| panic!("cliente GitHub pré-configurado não usa registro dinâmico"),
        )
        .unwrap();
        let auth_url = build_authorization_url(
            &metadata,
            &client.client_id,
            "http://127.0.0.1:43210/callback",
            "state-fixture",
            "pkce-challenge-fixture",
            "https://api.githubcopilot.com/mcp/",
            Some(github_oauth_read_scopes()),
            false,
        )
        .unwrap();
        let auth_url = Url::parse(&auth_url).unwrap();
        let query: std::collections::HashMap<_, _> = auth_url.query_pairs().into_owned().collect();
        assert_eq!(
            query.get("client_id").map(String::as_str),
            Some("github-client")
        );
        assert_eq!(query.get("scope").map(String::as_str), Some("read:user"));
        assert_eq!(
            query.get("code_challenge_method").map(String::as_str),
            Some("S256")
        );
        assert_eq!(
            query.get("code_challenge").map(String::as_str),
            Some("pkce-challenge-fixture")
        );
        assert_eq!(
            parse_callback_target(
                "/callback?state=state-fixture&code=authorization-code",
                "state-fixture",
                "https://auth.example",
                false,
            )
            .unwrap()
            .0,
            "authorization-code"
        );
    }

    #[test]
    fn dynamic_registration_does_not_require_client_id_and_uses_returned_id() {
        let metadata = registration_metadata(Some("https://auth.example/register"));
        let mut posted = false;
        let client = register_client_with(
            &metadata,
            "http://127.0.0.1:43210/callback",
            None,
            Some("pages:read"),
            |endpoint, body| {
                posted = true;
                assert_eq!(endpoint, "https://auth.example/register");
                assert!(body.get("client_id").is_none());
                assert_eq!(body["scope"], "pages:read");
                Ok(json!({
                    "client_id":"registered-client",
                    "client_secret":"registered-secret-fixture"
                }))
            },
        )
        .unwrap();
        assert!(posted);
        assert_eq!(client.client_id, "registered-client");
        assert_eq!(
            client.client_secret.as_deref(),
            Some("registered-secret-fixture")
        );
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
        let client = register_client_with(
            &metadata,
            "http://127.0.0.1:43210/callback",
            Some("manual-client"),
            None,
            |_, _| panic!("registro dinâmico não deve ser chamado"),
        )
        .unwrap();
        assert_eq!(client.client_id, "manual-client");
        assert!(client.client_secret.is_none());
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
    fn removal_without_tokens_is_not_reported_as_revoked() {
        let mut stored = stored_oauth();
        stored.refresh_token = None;
        let mut server = SimulatedRevocationServer {
            statuses: VecDeque::new(),
            submitted_tokens: Vec::new(),
        };
        assert!(!revoke_tokens_with(&stored, None, &mut server));
        assert!(server.submitted_tokens.is_empty());
    }

    #[test]
    fn missing_revocation_endpoint_is_not_reported_as_revoked() {
        let mut stored = stored_oauth();
        stored.revocation_endpoint = None;
        let mut server = SimulatedRevocationServer {
            statuses: Default::default(),
            submitted_tokens: Vec::new(),
        };
        assert!(!revoke_tokens_with(&stored, None, &mut server));
        assert_eq!(server.submitted_tokens.len(), 0);
    }

    #[test]
    fn revocation_failure_warning_never_contains_the_token() {
        let warning = removal_warning(false).unwrap();
        assert!(!warning.contains("refresh-secret-fixture"));
        assert!(!warning.contains("access-secret-fixture"));
    }
}
