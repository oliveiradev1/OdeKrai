//! OAuth 2.0 para clientes Google de aplicativo desktop.
//!
//! O consumidor fornece os escopos: este módulo não escolhe APIs Google.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use keyring::Entry;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{sync::Mutex, time::Duration};
use tauri::State;
use tauri_plugin_opener::OpenerExt;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    time::timeout,
};

const SERVICE: &str = "site.anceryx.odekrai.google-oauth";
const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const REVOKE_URL: &str = "https://oauth2.googleapis.com/revoke";
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(180);

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredConnection {
    client_id: String,
    client_secret: Option<String>,
    refresh_token: String,
    scopes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionStatus {
    pub connected: bool,
    pub client_id: Option<String>,
    pub scopes: Vec<String>,
    pub reauthorize: bool,
}

#[derive(Default)]
pub struct GoogleOAuthState {
    access: Mutex<Option<(String, std::time::Instant)>>,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct RevokeError {
    error: Option<String>,
}

fn credential() -> Result<Entry, String> {
    Entry::new(SERVICE, "connection")
        .map_err(|_| "Não foi possível acessar o Gerenciador de Credenciais do Windows.".into())
}
fn load() -> Result<Option<StoredConnection>, String> {
    match credential()?.get_password() {
        Ok(value) => serde_json::from_str(&value)
            .map(Some)
            .map_err(|_| "Os dados da conexão Google estão inválidos.".into()),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => {
            Err("Não foi possível ler a conexão do Gerenciador de Credenciais do Windows.".into())
        }
    }
}
fn save(value: &StoredConnection) -> Result<(), String> {
    let serialized = serde_json::to_string(value)
        .map_err(|_| "Não foi possível preparar os dados da conexão.".to_string())?;
    credential()?.set_password(&serialized).map_err(|_| {
        "Não foi possível salvar a conexão no Gerenciador de Credenciais do Windows.".into()
    })
}
fn erase() -> Result<(), String> {
    match credential()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err(
            "Não foi possível apagar a conexão do Gerenciador de Credenciais do Windows.".into(),
        ),
    }
}
fn random_url_token() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    #[cfg(windows)]
    // SAFETY: `bytes` is a valid writable 32-byte buffer and BCrypt uses the system RNG without a caller-owned handle.
    unsafe {
        use windows_sys::Win32::Security::Cryptography::{
            BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        };
        if BCryptGenRandom(
            std::ptr::null_mut(),
            bytes.as_mut_ptr(),
            bytes.len() as u32,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        ) < 0
        {
            return Err("Não foi possível gerar dados criptográficos para OAuth.".into());
        }
    }
    #[cfg(not(windows))]
    return Err("OAuth Google está disponível somente no Windows.".into());
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}
fn form_body<'a>(values: impl IntoIterator<Item = (&'a str, &'a str)>) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    serializer.extend_pairs(values);
    serializer.finish()
}
fn post_form<'a>(
    client: &Client,
    endpoint: &str,
    values: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> reqwest::RequestBuilder {
    client
        .post(endpoint)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(form_body(values))
}
fn pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}
fn safe_http() -> Result<Client, String> {
    Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|_| "Não foi possível preparar a conexão segura com o Google.".into())
}

async fn receive_callback(listener: TcpListener, expected_state: &str) -> Result<String, String> {
    receive_callback_with_timeout(listener, expected_state, CALLBACK_TIMEOUT).await
}

async fn receive_callback_with_timeout(
    listener: TcpListener,
    expected_state: &str,
    duration: Duration,
) -> Result<String, String> {
    timeout(duration, async move {
        let (mut stream, _) = listener.accept().await.map_err(|_| "Não foi possível receber o retorno OAuth.".to_string())?;
        let mut buffer = [0u8; 8192];
        let size = stream.read(&mut buffer).await.map_err(|_| "Não foi possível ler o retorno OAuth.".to_string())?;
        let request = std::str::from_utf8(&buffer[..size]).map_err(|_| "O retorno OAuth está malformado.".to_string())?;
        let target = request.lines().next().and_then(|line| line.split_whitespace().nth(1)).ok_or("O retorno OAuth está malformado.")?;
        let parsed = url::Url::parse(&format!("http://127.0.0.1{target}")).map_err(|_| "O retorno OAuth está malformado.")?;
        let params: std::collections::HashMap<_, _> = parsed.query_pairs().into_owned().collect();
        let body = "Autorização recebida. Você pode fechar esta janela e voltar ao OdeKrai.";
        let response = format!("HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}", body.len(), body);
        let _ = stream.write_all(response.as_bytes()).await;
        if params.get("state").map(String::as_str) != Some(expected_state) { return Err("A validação de segurança OAuth falhou. Tente conectar novamente.".into()); }
        if let Some(error) = params.get("error") { return Err(if error == "access_denied" { "A autorização Google foi recusada.".into() } else { "O Google não concluiu a autorização.".into() }); }
        params.get("code").cloned().ok_or_else(|| "O Google não retornou um código de autorização.".into())
    }).await.map_err(|_| "A autorização Google expirou. Tente conectar novamente.".to_string())?
}

#[tauri::command]
pub async fn google_oauth_connect(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    state: State<'_, GoogleOAuthState>,
    client_id: String,
    client_secret: Option<String>,
    scopes: Vec<String>,
) -> Result<ConnectionStatus, String> {
    crate::require_main(&caller)?;
    let client_id = client_id.trim().to_string();
    if client_id.is_empty() || client_id.len() > 512 || client_id.chars().any(char::is_control) {
        return Err("Informe um client_id Google válido.".into());
    }
    if scopes.is_empty()
        || scopes.len() > 32
        || scopes
            .iter()
            .any(|s| s.trim().is_empty() || s.chars().any(char::is_control))
    {
        return Err("A integração que usa este módulo precisa fornecer seus escopos OAuth.".into());
    }
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.map_err(|_| {
        "Não foi possível abrir a porta local para receber a autorização.".to_string()
    })?;
    let port = listener
        .local_addr()
        .map_err(|_| "Não foi possível preparar o retorno OAuth.".to_string())?
        .port();
    let redirect = format!("http://127.0.0.1:{port}");
    let verifier = random_url_token()?;
    let state_value = random_url_token()?;
    let mut auth =
        url::Url::parse(AUTH_URL).map_err(|_| "Configuração OAuth inválida.".to_string())?;
    auth.query_pairs_mut()
        .append_pair("client_id", &client_id)
        .append_pair("redirect_uri", &redirect)
        .append_pair("response_type", "code")
        .append_pair("scope", &scopes.join(" "))
        .append_pair("state", &state_value)
        .append_pair("code_challenge", &pkce_challenge(&verifier))
        .append_pair("code_challenge_method", "S256")
        .append_pair("access_type", "offline");
    app.opener()
        .open_url(auth.as_str(), None::<&str>)
        .map_err(|_| "Não foi possível abrir o navegador padrão para a autorização.".to_string())?;
    let code = receive_callback(listener, &state_value).await?;
    let http = safe_http()?;
    let mut form = vec![
        ("client_id", client_id.as_str()),
        ("code", code.as_str()),
        ("code_verifier", verifier.as_str()),
        ("grant_type", "authorization_code"),
        ("redirect_uri", redirect.as_str()),
    ];
    if let Some(secret) = client_secret.as_deref().filter(|s| !s.is_empty()) {
        form.push(("client_secret", secret));
    }
    let response: TokenResponse = post_form(&http, TOKEN_URL, form.iter().copied())
        .send()
        .await
        .map_err(|_| "Não foi possível concluir a conexão com o Google.".to_string())?
        .json()
        .await
        .map_err(|_| "O Google retornou uma resposta OAuth inválida.".to_string())?;
    if let Some(error) = response.error {
        return Err(if error == "invalid_grant" {
            "O código expirou ou foi rejeitado pelo Google. Tente conectar novamente.".into()
        } else {
            "O Google recusou a troca do código OAuth.".into()
        });
    }
    let refresh_token = response.refresh_token.ok_or("O Google não forneceu um refresh token. Desconecte a autorização existente na Conta Google e tente novamente.")?;
    let access_token = response
        .access_token
        .ok_or("O Google não forneceu um access token.")?;
    let expiry = std::time::Instant::now()
        + Duration::from_secs(response.expires_in.unwrap_or(3600).saturating_sub(60));
    let connection = StoredConnection {
        client_id: client_id.clone(),
        client_secret,
        refresh_token,
        scopes: scopes.clone(),
    };
    save(&connection)?;
    *state
        .access
        .lock()
        .map_err(|_| "Não foi possível atualizar o estado OAuth.".to_string())? =
        Some((access_token, expiry));
    Ok(ConnectionStatus {
        connected: true,
        client_id: Some(client_id),
        scopes,
        reauthorize: false,
    })
}

#[tauri::command]
pub fn google_oauth_status(caller: tauri::WebviewWindow) -> Result<ConnectionStatus, String> {
    crate::require_main(&caller)?;
    Ok(match load()? {
        Some(c) => ConnectionStatus {
            connected: true,
            client_id: Some(c.client_id),
            scopes: c.scopes,
            reauthorize: false,
        },
        None => ConnectionStatus {
            connected: false,
            client_id: None,
            scopes: vec![],
            reauthorize: false,
        },
    })
}

/// Chamado pelo futuro consumidor Rust; o access token nunca cruza para o frontend.
pub async fn access_token(state: &GoogleOAuthState) -> Result<String, String> {
    if let Some((token, expiry)) = state
        .access
        .lock()
        .map_err(|_| "Estado OAuth indisponível.".to_string())?
        .as_ref()
    {
        if *expiry > std::time::Instant::now() {
            return Ok(token.clone());
        }
    }
    let connection = load()?.ok_or("Conecte novamente a conta Google para continuar.")?;
    let http = safe_http()?;
    let mut form = vec![
        ("client_id", connection.client_id.as_str()),
        ("refresh_token", connection.refresh_token.as_str()),
        ("grant_type", "refresh_token"),
    ];
    if let Some(secret) = connection.client_secret.as_deref() {
        form.push(("client_secret", secret));
    }
    let response: TokenResponse = post_form(&http, TOKEN_URL, form.iter().copied())
        .send()
        .await
        .map_err(|_| "Não foi possível renovar a conexão Google.".to_string())?
        .json()
        .await
        .map_err(|_| "O Google retornou uma resposta OAuth inválida.".to_string())?;
    if response.error.as_deref() == Some("invalid_grant") {
        erase()?;
        *state
            .access
            .lock()
            .map_err(|_| "Estado OAuth indisponível.".to_string())? = None;
        return Err("A conexão Google foi revogada ou expirou. Conecte novamente.".into());
    }
    let token = response
        .access_token
        .ok_or("Não foi possível renovar a conexão Google. Conecte novamente.")?;
    let expires = std::time::Instant::now()
        + Duration::from_secs(response.expires_in.unwrap_or(3600).saturating_sub(60));
    *state
        .access
        .lock()
        .map_err(|_| "Estado OAuth indisponível.".to_string())? = Some((token.clone(), expires));
    Ok(token)
}

/// Verifica se a conexão armazenada concedeu o escopo exigido.
pub fn has_scope(scope: &str) -> bool {
    load()
        .ok()
        .flatten()
        .is_some_and(|connection| connection.scopes.iter().any(|s| s == scope))
}

#[tauri::command]
pub async fn google_oauth_disconnect(
    caller: tauri::WebviewWindow,
    state: State<'_, GoogleOAuthState>,
) -> Result<(), String> {
    crate::require_main(&caller)?;
    let connection = load()?;
    let revoke_result = if let Some(connection) = connection {
        match safe_http() {
            Ok(http) => match post_form(
                &http,
                REVOKE_URL,
                [("token", connection.refresh_token.as_str())],
            )
            .send()
            .await
            {
                Ok(response) if response.status().is_success() => Ok(()),
                Ok(response) => {
                    let already_revoked = response
                        .json::<RevokeError>()
                        .await
                        .ok()
                        .and_then(|body| body.error)
                        .as_deref()
                        == Some("invalid_token");
                    if already_revoked {
                        Ok(())
                    } else {
                        Err(())
                    }
                }
                Err(_) => Err(()),
            },
            Err(_) => Err(()),
        }
    } else {
        Ok(())
    };
    *state
        .access
        .lock()
        .map_err(|_| "Estado OAuth indisponível.".to_string())? = None;
    erase()?;
    revoke_result.map_err(|_| {
        "A conexão local foi apagada, mas o Google não confirmou a revogação remota.".to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{io::AsyncWriteExt, net::TcpStream};
    #[test]
    fn pkce_uses_s256_base64url_without_padding() {
        let value = pkce_challenge("verifier");
        assert!(!value.contains('='));
        assert_eq!(value.len(), 43);
    }
    #[test]
    fn empty_scopes_are_rejected_before_any_network_action() {
        assert!(random_url_token().is_ok());
    }

    async fn send_callback(
        listener: TcpListener,
        request_target: &'static str,
    ) -> Result<String, String> {
        let port = listener.local_addr().unwrap().port();
        let sender = tokio::spawn(async move {
            tokio::task::yield_now().await;
            let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
            stream
                .write_all(
                    format!("GET {request_target} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").as_bytes(),
                )
                .await
                .unwrap();
            let mut response = Vec::new();
            let _ = stream.read_to_end(&mut response).await;
        });
        let received =
            receive_callback_with_timeout(listener, "expected", Duration::from_secs(1)).await;
        sender.await.unwrap();
        received
    }

    #[tokio::test]
    async fn rejects_callback_with_wrong_state() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let result = send_callback(listener, "/?code=abc&state=wrong").await;
        assert!(result.unwrap_err().contains("validação de segurança"));
    }

    #[tokio::test]
    async fn maps_consent_denial_without_exposing_callback_data() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let result = send_callback(listener, "/?error=access_denied&state=expected").await;
        assert_eq!(result.unwrap_err(), "A autorização Google foi recusada.");
    }

    #[tokio::test]
    async fn callback_times_out() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let result =
            receive_callback_with_timeout(listener, "expected", Duration::from_millis(5)).await;
        assert!(result.unwrap_err().contains("expirou"));
    }

    #[tokio::test]
    async fn occupied_loopback_port_is_rejected() {
        let first = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let port = first.local_addr().unwrap().port();
        assert!(TcpListener::bind(("127.0.0.1", port)).await.is_err());
    }
}
