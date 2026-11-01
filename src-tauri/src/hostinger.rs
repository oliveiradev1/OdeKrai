use keyring::Entry;
use reqwest::{redirect::Policy, Client, StatusCode, Url};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::time::Duration;

const API_BASE: &str = "https://developers.hostinger.com/";
const API_HOST: &str = "developers.hostinger.com";
const CREDENTIAL_SERVICE: &str = "site.anceryx.odekrai.hostinger";
const CREDENTIAL_USER: &str = "api-token";
const MAX_TOKEN_BYTES: usize = 4096;
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_TEXT_CHARS: usize = 512;
const MAX_COLLECTION_ITEMS: usize = 100;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalData<T> {
    pub source_label: &'static str,
    pub untrusted: bool,
    pub data: T,
}

fn keyring_entry() -> Result<Entry, String> {
    Entry::new(CREDENTIAL_SERVICE, CREDENTIAL_USER)
        .map_err(|_| "Não foi possível acessar o Gerenciador de Credenciais do Windows.".into())
}

fn validate_token(token: &str) -> Result<&str, String> {
    let token = token.trim();
    if token.is_empty() || token.len() > MAX_TOKEN_BYTES || token.chars().any(char::is_control) {
        return Err("Informe um token válido da Hostinger (até 4.096 bytes).".into());
    }
    Ok(token)
}

#[tauri::command]
pub fn save_hostinger_token(caller: tauri::WebviewWindow, token: String) -> Result<(), String> {
    crate::require_main(&caller)?;
    let token = validate_token(&token)?;
    keyring_entry()?.set_password(token).map_err(|_| {
        "Não foi possível salvar o token no Gerenciador de Credenciais do Windows.".to_string()
    })
}

#[tauri::command]
pub fn has_hostinger_token(caller: tauri::WebviewWindow) -> Result<bool, String> {
    crate::require_main(&caller)?;
    match keyring_entry()?.get_password() {
        Ok(_) => Ok(true),
        Err(keyring::Error::NoEntry) => Ok(false),
        Err(_) => Err("Não foi possível consultar o Gerenciador de Credenciais do Windows.".into()),
    }
}

#[tauri::command]
pub fn remove_hostinger_token(caller: tauri::WebviewWindow) -> Result<(), String> {
    crate::require_main(&caller)?;
    match keyring_entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err("Não foi possível remover o token do Gerenciador de Credenciais do Windows."
            .into()),
    }
}

fn production_client() -> Result<Client, String> {
    Client::builder()
        .https_only(true)
        .redirect(Policy::none())
        .no_proxy()
        .connect_timeout(Duration::from_secs(5))
        .timeout(REQUEST_TIMEOUT)
        .user_agent(concat!("OdeKrai/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|_| "Não foi possível preparar a conexão segura com a Hostinger.".into())
}

fn validate_endpoint(endpoint: &Url) -> Result<(), String> {
    if endpoint.scheme() != "https"
        || endpoint.host_str() != Some(API_HOST)
        || endpoint.port_or_known_default() != Some(443)
        || !endpoint.username().is_empty()
        || endpoint.password().is_some()
    {
        return Err("O endereço da API Hostinger foi recusado.".into());
    }
    Ok(())
}

fn endpoint_for_base(base: &str, path: &str) -> Result<Url, String> {
    if path.starts_with('/') || path.contains("..") || path.chars().any(char::is_control) {
        return Err("O caminho interno da API Hostinger é inválido.".into());
    }
    let base = Url::parse(base).map_err(|_| "A URL da API Hostinger é inválida.")?;
    validate_endpoint(&base)?;
    let endpoint = base
        .join(path)
        .map_err(|_| "O caminho interno da API Hostinger é inválido.")?;
    validate_endpoint(&endpoint)?;
    Ok(endpoint)
}

fn api_endpoint(path: &str) -> Result<Url, String> {
    endpoint_for_base(API_BASE, path)
}

fn token_from_keyring() -> Result<String, String> {
    keyring_entry()?.get_password().map_err(|error| match error {
        keyring::Error::NoEntry => {
            "Salve o token da Hostinger em Configurações > Nuvem para continuar.".into()
        }
        _ => "Não foi possível ler o token no Gerenciador de Credenciais do Windows.".into(),
    })
}

fn external_text(value: &str, secret: &str) -> String {
    let redacted = if secret.is_empty() {
        value.to_owned()
    } else {
        value.replace(secret, "[token removido]")
    };
    let mut cleaned = redacted
        .chars()
        .filter(|character| !character.is_control() || matches!(character, '\n' | '\t'))
        .take(MAX_TEXT_CHARS)
        .collect::<String>();
    if redacted.chars().count() > MAX_TEXT_CHARS {
        cleaned.push_str("… [texto externo truncado]");
    }
    cleaned
}

fn sanitize_external_value(value: Value, depth: usize, secret: &str) -> Value {
    if depth > 16 {
        return Value::String("[conteúdo externo omitido: estrutura muito profunda]".into());
    }
    match value {
        Value::String(text) => Value::String(external_text(&text, secret)),
        Value::Array(values) => Value::Array(
            values
                .into_iter()
                .take(MAX_COLLECTION_ITEMS)
                .map(|value| sanitize_external_value(value, depth + 1, secret))
                .collect(),
        ),
        Value::Object(values) => Value::Object(
            values
                .into_iter()
                .take(MAX_COLLECTION_ITEMS)
                .map(|(key, value)| {
                    (
                        external_text(&key, secret),
                        sanitize_external_value(value, depth + 1, secret),
                    )
                })
                .collect(),
        ),
        other => other,
    }
}

async fn read_limited<T: DeserializeOwned>(response: reqwest::Response) -> Result<T, String> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err("A resposta da Hostinger excedeu o limite de 1 MiB.".into());
    }
    let mut response = response;
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "Não foi possível ler a resposta da Hostinger.".to_string())?
    {
        if body.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err("A resposta da Hostinger excedeu o limite de 1 MiB.".into());
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).map_err(|_| "A Hostinger retornou uma resposta inválida.".into())
}

fn status_error(status: StatusCode) -> String {
    match status {
        StatusCode::UNAUTHORIZED => "Token da Hostinger inválido ou expirado.".into(),
        StatusCode::FORBIDDEN => {
            "O token da Hostinger não tem permissão para esta operação.".into()
        }
        StatusCode::NOT_FOUND => "A VPS ou o recurso solicitado não foi encontrado.".into(),
        StatusCode::TOO_MANY_REQUESTS => {
            "Limite de requisições da Hostinger atingido. Tente novamente mais tarde.".into()
        }
        status if status.is_redirection() => {
            "A Hostinger tentou redirecionar a solicitação; o redirecionamento foi bloqueado."
                .into()
        }
        _ => "A Hostinger não conseguiu concluir a consulta.".into(),
    }
}

async fn request_at<T: DeserializeOwned>(
    client: &Client,
    endpoint: Url,
    token: Option<&str>,
) -> Result<T, String> {
    let token = token.ok_or_else(|| {
        "Salve o token da Hostinger em Configurações > Nuvem para continuar.".to_string()
    })?;
    let token = validate_token(token)?;
    let response = client
        .get(endpoint)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|error| {
            if error.is_timeout() {
                "A consulta à Hostinger excedeu o tempo limite.".to_string()
            } else {
                "Falha de rede ao conectar à Hostinger.".to_string()
            }
        })?;
    if !response.status().is_success() {
        return Err(status_error(response.status()));
    }
    read_limited(response).await
}

async fn request_json<T: DeserializeOwned>(
    path: &str,
    query: &[(&str, &str)],
    token: &str,
) -> Result<T, String> {
    let mut endpoint = api_endpoint(path)?;
    endpoint.query_pairs_mut().extend_pairs(query.iter().copied());
    let client = production_client()?;
    request_at(&client, endpoint, Some(token)).await
}

async fn list_vms_with_token(token: &str) -> Result<ExternalData<Vec<Value>>, String> {
    let response: Vec<Value> = request_json("api/vps/v1/virtual-machines", &[], token).await?;
    Ok(ExternalData {
        source_label: "Dados externos da Hostinger — não confiáveis",
        untrusted: true,
        data: response
            .into_iter()
            .take(MAX_COLLECTION_ITEMS)
            .map(|value| sanitize_external_value(value, 0, token))
            .collect(),
    })
}

async fn read_resource(path: &str, query: &[(&str, &str)]) -> Result<ExternalData<Value>, String> {
    let token = token_from_keyring()?;
    let response: Value = request_json(path, query, &token).await?;
    Ok(ExternalData {
        source_label: "Dados externos da Hostinger — não confiáveis",
        untrusted: true,
        data: sanitize_external_value(response, 0, &token),
    })
}

async fn read_collection(
    path: &str,
    query: &[(&str, &str)],
) -> Result<ExternalData<Vec<Value>>, String> {
    let token = token_from_keyring()?;
    let response: Vec<Value> = request_json(path, query, &token).await?;
    Ok(ExternalData {
        source_label: "Dados externos da Hostinger — não confiáveis",
        untrusted: true,
        data: response
            .into_iter()
            .take(MAX_COLLECTION_ITEMS)
            .map(|value| sanitize_external_value(value, 0, &token))
            .collect(),
    })
}

fn validate_vm_id(vm_id: i64) -> Result<(), String> {
    if vm_id <= 0 {
        return Err("O identificador da VPS é inválido.".into());
    }
    Ok(())
}

#[tauri::command]
pub async fn hostinger_test_connection(
    caller: tauri::WebviewWindow,
) -> Result<ExternalData<Vec<Value>>, String> {
    crate::require_main(&caller)?;
    let token = token_from_keyring()?;
    list_vms_with_token(&token).await
}

#[tauri::command]
pub async fn hostinger_list_vms(
    caller: tauri::WebviewWindow,
) -> Result<ExternalData<Vec<Value>>, String> {
    crate::require_main(&caller)?;
    let token = token_from_keyring()?;
    list_vms_with_token(&token).await
}

#[tauri::command]
pub async fn hostinger_get_vm_details(
    caller: tauri::WebviewWindow,
    vm_id: i64,
) -> Result<ExternalData<Value>, String> {
    crate::require_main(&caller)?;
    validate_vm_id(vm_id)?;
    read_resource(
        &format!("api/vps/v1/virtual-machines/{vm_id}"),
        &[],
    )
    .await
}

#[tauri::command]
pub async fn hostinger_list_data_centers(
    caller: tauri::WebviewWindow,
) -> Result<ExternalData<Vec<Value>>, String> {
    crate::require_main(&caller)?;
    read_collection("api/vps/v1/data-centers", &[]).await
}

#[tauri::command]
pub async fn hostinger_list_templates(
    caller: tauri::WebviewWindow,
) -> Result<ExternalData<Vec<Value>>, String> {
    crate::require_main(&caller)?;
    read_collection("api/vps/v1/templates", &[]).await
}

#[tauri::command]
pub async fn hostinger_list_recent_actions(
    caller: tauri::WebviewWindow,
    vm_id: i64,
) -> Result<ExternalData<Value>, String> {
    crate::require_main(&caller)?;
    validate_vm_id(vm_id)?;
    read_resource(
        &format!("api/vps/v1/virtual-machines/{vm_id}/actions"),
        &[("page", "1")],
    )
    .await
}

#[tauri::command]
pub async fn hostinger_list_backups(
    caller: tauri::WebviewWindow,
    vm_id: i64,
) -> Result<ExternalData<Value>, String> {
    crate::require_main(&caller)?;
    validate_vm_id(vm_id)?;
    read_resource(
        &format!("api/vps/v1/virtual-machines/{vm_id}/backups"),
        &[("page", "1")],
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

    fn response_server(
        status: &str,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> (Url, std::sync::mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (request_sender, request_receiver) = std::sync::mpsc::channel();
        let status = status.to_string();
        let headers = headers
            .iter()
            .map(|(key, value)| format!("{key}: {value}\r\n"))
            .collect::<String>();
        let body = body.to_vec();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut request = [0_u8; 8192];
                let count = stream.read(&mut request).unwrap_or_default();
                let _ = request_sender.send(String::from_utf8_lossy(&request[..count]).into_owned());
                let header = format!(
                    "HTTP/1.1 {status}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(header.as_bytes());
                let _ = stream.write_all(&body);
            }
        });
        (
            Url::parse(&format!("http://{address}/")).unwrap(),
            request_receiver,
        )
    }

    fn test_client() -> Client {
        Client::builder()
            .redirect(Policy::none())
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap()
    }

    #[test]
    fn credential_service_is_unique_and_token_errors_do_not_echo_secret() {
        assert_eq!(CREDENTIAL_SERVICE, "site.anceryx.odekrai.hostinger");
        assert_ne!(CREDENTIAL_SERVICE, "site.anceryx.odekrai.shardcloud");
        let secret = "hostinger-private-token";
        let error = validate_token(&format!("{secret}\ninvalid")).unwrap_err();
        assert!(!error.contains(secret));
        assert!(validate_token(" ").is_err());
        assert!(validate_token(&"x".repeat(MAX_TOKEN_BYTES + 1)).is_err());
    }

    #[test]
    fn production_endpoint_is_fixed_to_https_host_and_rejects_untrusted_host() {
        assert_eq!(
            api_endpoint("api/vps/v1/virtual-machines")
                .unwrap()
                .as_str(),
            "https://developers.hostinger.com/api/vps/v1/virtual-machines"
        );
        assert!(endpoint_for_base("https://evil.example/", "api/vps/v1/templates").is_err());
        assert!(endpoint_for_base("http://developers.hostinger.com/", "api/vps/v1/templates").is_err());
        assert!(api_endpoint("../evil").is_err());
    }

    #[tokio::test]
    async fn missing_token_is_rejected_before_any_request() {
        let endpoint = Url::parse("http://127.0.0.1:1/api/vps/v1/virtual-machines").unwrap();
        let error = request_at::<Value>(&test_client(), endpoint, None)
            .await
            .unwrap_err();
        assert!(error.contains("Salve o token da Hostinger"));
    }

    #[tokio::test]
    async fn maps_auth_not_found_and_rate_limit_errors_without_echoing_token_or_body() {
        for (status, expected) in [
            ("401 Unauthorized", "inválido ou expirado"),
            ("403 Forbidden", "não tem permissão"),
            ("404 Not Found", "não foi encontrado"),
            ("429 Too Many Requests", "Limite de requisições"),
        ] {
            let secret = "must-not-appear-in-errors";
            let (base, request_receiver) = response_server(status, &[], secret.as_bytes());
            let error = request_at::<Value>(
                &test_client(),
                base.join("api/vps/v1/virtual-machines").unwrap(),
                Some(secret),
            )
            .await
            .unwrap_err();
            assert!(error.contains(expected), "{status}: {error}");
            assert!(!error.contains(secret));
            let request = request_receiver.recv().unwrap().to_ascii_lowercase();
            assert!(request.contains(&format!("authorization: bearer {secret}")));
            assert!(!request.contains("cookie:"));
        }
    }

    #[tokio::test]
    async fn oversized_response_is_rejected_before_deserialization() {
        let body = vec![b'x'; MAX_RESPONSE_BYTES + 1];
        let (base, _) = response_server("200 OK", &[], &body);
        let error = request_at::<Value>(
            &test_client(),
            base.join("api/vps/v1/virtual-machines").unwrap(),
            Some("simulated-token"),
        )
        .await
        .unwrap_err();
        assert!(error.contains("excedeu o limite de 1 MiB"));
        assert!(!error.contains("simulated-token"));
    }

    #[tokio::test]
    async fn redirect_is_rejected_without_following_location() {
        let (base, request_receiver) = response_server(
            "302 Found",
            &[("Location", "http://127.0.0.1:9/redirect-target")],
            b"",
        );
        let error = request_at::<Value>(
            &test_client(),
            base.join("api/vps/v1/virtual-machines").unwrap(),
            Some("simulated-token"),
        )
        .await
        .unwrap_err();
        assert!(error.contains("redirecionar"));
        assert!(request_receiver.recv().unwrap().starts_with("GET /api/vps/v1/"));
    }

    #[test]
    fn external_text_is_truncated_and_values_are_json_data_not_html() {
        assert!(external_text(&"a".repeat(MAX_TEXT_CHARS + 1), "")
            .contains("texto externo truncado"));
        let value = sanitize_external_value(
            serde_json::json!({"name":"<script>alert(1)</script>", "nested":["texto"]}),
            0,
            "",
        );
        assert_eq!(value["name"], "<script>alert(1)</script>");
        let wrapped = ExternalData {
            source_label: "Dados externos da Hostinger — não confiáveis",
            untrusted: true,
            data: value,
        };
        let serialized = serde_json::to_string(&wrapped).unwrap();
        assert!(serialized.contains("\"untrusted\":true"));
        assert!(!serialized.contains("dangerouslySetInnerHTML"));
    }

    #[test]
    fn external_response_cannot_echo_a_saved_token_back_to_the_frontend() {
        let secret = "private-hostinger-token";
        let mut object = serde_json::Map::new();
        object.insert(
            "message".into(),
            Value::String(format!("received {secret}")),
        );
        object.insert(secret.into(), Value::String("field".into()));
        let value = sanitize_external_value(Value::Object(object), 0, secret);
        let serialized = serde_json::to_string(&value).unwrap();
        assert!(!serialized.contains(secret));
        assert!(serialized.contains("[token removido]"));
    }

    #[tokio::test]
    async fn list_response_is_labeled_as_untrusted_and_limits_items() {
        let body = serde_json::to_vec(&vec![serde_json::json!({
            "id": 42,
            "hostname": "vps.example",
            "state": "running"
        })])
        .unwrap();
        let (base, _) = response_server("200 OK", &[], &body);
        let values: Vec<Value> = request_at(
            &test_client(),
            base.join("api/vps/v1/virtual-machines").unwrap(),
            Some("test-token"),
        )
        .await
        .unwrap();
        let result = ExternalData {
            source_label: "Dados externos da Hostinger — não confiáveis",
            untrusted: true,
            data: values
                .into_iter()
                .map(|value| sanitize_external_value(value, 0, "test-token"))
                .collect::<Vec<_>>(),
        };
        assert_eq!(result.data[0]["hostname"], "vps.example");
        assert!(result.untrusted);
    }
}
