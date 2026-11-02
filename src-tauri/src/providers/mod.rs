#[cfg(test)]
use reqwest::Client;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
#[cfg(test)]
use std::time::Duration;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
};
use tauri::{ipc::Channel, Manager, State};
use tokio_util::sync::CancellationToken;

mod anthropic;
pub(crate) mod audio_transcription;
mod cancellation;
mod catalog;
mod chat_completions;
mod credentials;
mod endpoint;
mod failover;
mod http;
mod llm_provider;
use endpoint::provider_endpoint;
use http::{
    provider_client, provider_http_error, provider_http_error_with_body,
    read_provider_error_body, safe_transport_error, AuthRequest,
};
mod perplexity;
mod stream;
mod types;
#[cfg(test)]
use catalog::{capabilities_with_model_cache, CatalogEntry};
use catalog::{catalog_entry, effective_capabilities};
use credentials::{credential_entry, supports_credentials};
use llm_provider::{AnthropicProvider, ChatCompletionsProvider, LlmProvider, PerplexityProvider};
use stream::consume_stream_response;
pub use types::{
    Block, ChatRequest, Message, ProviderCapabilities, ProviderEvent, ToolCall, ToolDef, Usage,
};

pub(crate) fn supports_vision_model(provider_id: &str, model: &str) -> bool {
    match provider_id {
        // The official OpenAI vision guide demonstrates this exact Chat Completions model ID.
        "openai" => model == "gpt-6-astra",
        // Anthropic's current model overview confirms image input for its current models;
        // the explicit list keeps arbitrary IDs and compatible proxies disabled.
        "anthropic" => matches!(
            model,
            "claude-opus-5-5" | "claude-sonnet-5-5" | "claude-haiku-5-5" | "claude-fable-5-1"
        ),
        _ => false,
    }
}

#[cfg(test)]
mod vision_model_tests {
    use super::*;
    use base64::Engine as _;

    fn jpeg_message(data: &[u8]) -> Message {
        Message {
            role: "user".into(),
            blocks: vec![Block::Image {
                media_type: "image/jpeg".into(),
                data_base64: base64::engine::general_purpose::STANDARD.encode(data),
            }],
        }
    }

    fn test_jpeg() -> Vec<u8> {
        let image = image::DynamicImage::ImageRgb8(image::ImageBuffer::from_pixel(
            2,
            2,
            image::Rgb([30, 60, 90]),
        ));
        let mut bytes = std::io::Cursor::new(Vec::new());
        image
            .write_to(&mut bytes, image::ImageFormat::Jpeg)
            .unwrap();
        bytes.into_inner()
    }

    #[test]
    fn vision_requires_an_explicitly_documented_provider_and_exact_model_id() {
        assert!(supports_vision_model("openai", "gpt-6-astra"));
        assert!(supports_vision_model("anthropic", "claude-sonnet-5-5"));
        assert!(!supports_vision_model("openai", "gpt-6-luna"));
        assert!(!supports_vision_model(
            "anthropic",
            "claude-custom-proxy-model"
        ));
        assert!(!supports_vision_model("local", "gpt-6-astra"));
    }

    #[test]
    fn images_are_limited_to_one_valid_jpeg_user_block() {
        let image = jpeg_message(&test_jpeg()).blocks.remove(0);
        let valid = Message {
            role: "user".into(),
            blocks: vec![image.clone()],
        };
        assert!(valid_messages(&[valid]));
        assert!(!valid_messages(&[Message {
            role: "assistant".into(),
            blocks: vec![image.clone()],
        }]));
        assert!(!valid_messages(&[Message {
            role: "user".into(),
            blocks: vec![image.clone(), image],
        }]));
        assert!(!valid_messages(&[Message {
            role: "user".into(),
            blocks: vec![Block::Image {
                media_type: "image/png".into(),
                data_base64: base64::engine::general_purpose::STANDARD.encode(test_jpeg()),
            }],
        }]));
    }

    #[test]
    fn image_request_is_decoded_resized_and_reencoded_before_provider_use() {
        let message = jpeg_message(&test_jpeg());
        let normalized = normalize_request_images(vec![message]).unwrap();
        let Block::Image { data_base64, .. } = &normalized[0].blocks[0] else {
            panic!("imagem normalizada esperada")
        };
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(data_base64)
            .unwrap();
        assert!(bytes.len() <= crate::screen_capture::MAX_SCREENSHOT_BYTES);
        assert_eq!(
            image::guess_format(&bytes).unwrap(),
            image::ImageFormat::Jpeg
        );
    }

    #[test]
    fn image_request_rejects_invalid_and_oversized_data_before_provider_use() {
        assert!(normalize_request_images(vec![jpeg_message(b"invalid")]).is_err());
        let oversized = vec![0; crate::screen_capture::MAX_SCREENSHOT_BYTES + 1];
        assert!(normalize_request_images(vec![jpeg_message(&oversized)]).is_err());
    }
}
static INTERNAL_REQUEST_ID: AtomicU64 = AtomicU64::new(1);
pub use cancellation::cancel_all;
#[cfg(test)]
use cancellation::cancel_requests;
use cancellation::cancellable_request;
const MAX_CREDENTIAL_BYTES: usize = 4096;
const MAX_MESSAGE_BYTES: usize = 1_000_000;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelOption {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModelOption {
    pub base_url: String,
    pub model: ModelOption,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModelDiscovery {
    pub online_endpoints: Vec<String>,
    pub models: Vec<LocalModelOption>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionStage {
    pub id: String,
    pub status: String,
    pub message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionTest {
    pub stages: Vec<ConnectionStage>,
    pub overall: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProviderMessage {
    pub role: String,
    pub content: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProviderCitation {
    pub title: String,
    pub url: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderReply {
    pub content: String,
    pub citations: Vec<ProviderCitation>,
    pub tool_calls: Vec<ToolCall>,
    pub usage: Option<Usage>,
    pub stop_reason: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderTarget {
    provider_id: String,
    base_url: String,
    model: String,
}

#[derive(Default)]
pub struct ProviderRequests(Mutex<HashMap<String, CancellationToken>>);

#[derive(Clone)]
enum EventSink {
    Channel(Channel<ProviderEvent>),
    Callback(Arc<dyn Fn(ProviderEvent) + Send + Sync>),
}

impl EventSink {
    fn send(&self, event: ProviderEvent) {
        match self {
            Self::Channel(channel) => {
                let _ = channel.send(event);
            }
            Self::Callback(callback) => callback(event),
        }
    }
}

#[tauri::command]
pub fn get_provider_capabilities(
    caller: tauri::WebviewWindow,
    app: tauri::AppHandle,
    provider_id: String,
    model: String,
) -> Result<ProviderCapabilities, String> {
    super::require_main(&caller)?;
    effective_capabilities(&app, &provider_id, &model)
}

#[tauri::command]
pub async fn set_provider_tools_opt_in(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    provider_id: String,
    model: String,
    enabled: bool,
) -> Result<(), String> {
    super::require_main(&caller)?;
    let entry = catalog_entry(&provider_id).ok_or("O provedor não está no catálogo do OdeKrai.")?;
    if !entry.capabilities.tools {
        return Err("Este adaptador não oferece tool calling.".into());
    }
    if enabled && !entry.capabilities.verified
        && !crate::security::confirm_native(format!("Permitir tool calling não verificado para {provider_id} / {model}? As chamadas continuarão sujeitas às permissões locais.")).await
    {
        return Err("A confirmação nativa foi recusada.".into());
    }
    crate::history::cache_tool_opt_in(&app, &provider_id, &model, enabled)
}

#[tauri::command]
pub fn save_provider_credential(
    caller: tauri::WebviewWindow,
    provider_id: String,
    credential: String,
) -> Result<(), String> {
    super::require_main(&caller)?;
    let credential = credential.trim();
    if credential.is_empty() || credential.len() > MAX_CREDENTIAL_BYTES {
        return Err("Informe uma credencial válida (até 4.096 bytes).".into());
    }
    credential_entry(&provider_id)?
        .set_password(credential)
        .map_err(|_| "Não foi possível salvar a credencial no Gerenciador do Windows.".into())
}

#[tauri::command]
pub async fn has_provider_credential(
    caller: tauri::WebviewWindow,
    provider_id: String,
) -> Result<bool, String> {
    super::require_main(&caller)?;
    match credential_entry(&provider_id)?.get_password() {
        Ok(_) => Ok(true),
        Err(keyring::Error::NoEntry) => Ok(false),
        Err(_) => Err("Não foi possível consultar o Gerenciador de Credenciais do Windows.".into()),
    }
}

#[tauri::command]
pub fn remove_provider_credential(
    caller: tauri::WebviewWindow,
    provider_id: String,
) -> Result<(), String> {
    super::require_main(&caller)?;
    match credential_entry(&provider_id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err("Não foi possível remover a credencial do Gerenciador do Windows.".into()),
    }
}

#[tauri::command]
pub async fn cancel_provider_request(
    caller: tauri::WebviewWindow,
    state: State<'_, ProviderRequests>,
    request_id: String,
) -> Result<bool, String> {
    super::require_main(&caller)?;
    let token = state
        .0
        .lock()
        .map_err(|_| "Não foi possível cancelar a solicitação.".to_string())?
        .get(&request_id)
        .cloned();
    if let Some(token) = token {
        token.cancel();
        Ok(true)
    } else {
        Ok(false)
    }
}

#[tauri::command]
pub fn validate_provider_endpoint(
    caller: tauri::WebviewWindow,
    provider_id: String,
    base_url: String,
) -> Result<(), String> {
    super::require_main(&caller)?;
    provider_endpoint(&provider_id, &base_url).map(|_| ())
}

#[tauri::command]
pub async fn list_provider_models(
    caller: tauri::WebviewWindow,
    state: State<'_, ProviderRequests>,
    security: State<'_, crate::security::SecurityState>,
    provider_id: String,
    base_url: String,
) -> Result<Vec<ModelOption>, String> {
    super::require_main(&caller)?;
    cancellable_request(&state, &security, fetch_models(&provider_id, &base_url)).await
}

#[tauri::command]
pub async fn discover_local_models(
    caller: tauri::WebviewWindow,
    app: tauri::AppHandle,
    state: State<'_, ProviderRequests>,
    security: State<'_, crate::security::SecurityState>,
    port: Option<u32>,
) -> Result<LocalModelDiscovery, String> {
    super::require_main(&caller)?;
    let endpoints = local_model_probe_endpoints(port)?;
    cancellable_request(&state, &security, async {
        let mut found = Vec::new();
        let mut online_endpoints = Vec::new();
        let mut invalid_response = false;
        for (port, base_url) in endpoints {
            match fetch_models_with_auth("local", &base_url, false).await {
                Ok(models) => {
                    online_endpoints.push(base_url.clone());
                    let result = if models.is_empty() {
                        crate::diagnostics::LocalModelProbeResult::Empty
                    } else {
                        crate::diagnostics::LocalModelProbeResult::Found
                    };
                    crate::diagnostics::log_local_model_server_probe(
                        &app,
                        port,
                        result,
                        models.len(),
                    );
                    found.extend(models.into_iter().map(|model| LocalModelOption {
                        base_url: base_url.clone(),
                        model,
                    }));
                }
                Err(message) if !message.contains("conectar") && !message.contains("demorou") => {
                    invalid_response = true;
                    crate::diagnostics::log_local_model_server_probe(
                        &app,
                        port,
                        crate::diagnostics::LocalModelProbeResult::InvalidResponse,
                        0,
                    );
                }
                Err(_) => {
                    crate::diagnostics::log_local_model_server_probe(
                        &app,
                        port,
                        crate::diagnostics::LocalModelProbeResult::Unavailable,
                        0,
                    );
                }
            }
        }
        finish_local_discovery(online_endpoints, found, invalid_response)
    })
    .await
}

fn local_model_probe_endpoints(port: Option<u32>) -> Result<Vec<(u16, String)>, String> {
    let mut endpoints = vec![
        (11434, "http://127.0.0.1:11434/v1".to_string()),
        (8080, "http://127.0.0.1:8080/v1".to_string()),
        (9931, "http://127.0.0.1:9931/v1".to_string()),
    ];
    if let Some(port) = port {
        if !(1024..=65535).contains(&port) {
            return Err("Informe uma porta entre 1024 e 65535.".into());
        }
        let endpoint = (port as u16, format!("http://127.0.0.1:{port}/v1"));
        if !endpoints.contains(&endpoint) {
            endpoints.push(endpoint);
        }
    }
    Ok(endpoints)
}

fn finish_local_discovery(
    online_endpoints: Vec<String>,
    models: Vec<LocalModelOption>,
    invalid_response: bool,
) -> Result<LocalModelDiscovery, String> {
    if online_endpoints.is_empty() && invalid_response {
        return Err(
            "O servidor local respondeu com dados inválidos ou não compatíveis com a API OpenAI."
                .into(),
        );
    }
    if online_endpoints.is_empty() {
        return Err("Servidor local não encontrado nas portas verificadas.".into());
    }
    Ok(LocalModelDiscovery {
        online_endpoints,
        models,
    })
}

async fn fetch_models(provider_id: &str, base_url: &str) -> Result<Vec<ModelOption>, String> {
    fetch_models_with_auth(provider_id, base_url, true).await
}

async fn fetch_models_with_auth(
    provider_id: &str,
    base_url: &str,
    include_local_credential: bool,
) -> Result<Vec<ModelOption>, String> {
    let mut url = provider_endpoint(provider_id, base_url)?;
    let entry =
        catalog_entry(provider_id).ok_or("O catálogo deste provedor não está disponível.")?;
    let path = entry.model_list_path.as_deref().ok_or(
        "A documentação oficial deste provedor não oferece listagem de modelos neste endpoint.",
    )?;
    let credential = if provider_id == "local" && !include_local_credential {
        String::new()
    } else {
        match credential_entry(provider_id)?.get_password() {
            Ok(value) => value,
            Err(keyring::Error::NoEntry) if provider_id == "local" => String::new(),
            _ => return Err("Salve uma credencial para listar os modelos.".into()),
        }
    };
    let base_path = if provider_id == "azure-openai" {
        url.path()
            .strip_suffix("/chat/completions")
            .unwrap_or(url.path())
            .trim_end_matches('/')
            .to_string()
    } else if provider_id == "local" {
        "/v1".to_string()
    } else if entry.base_url.is_empty() {
        url.path().trim_end_matches('/').to_string()
    } else {
        Url::parse(&entry.base_url)
            .map(|u| u.path().trim_end_matches('/').to_string())
            .unwrap_or_default()
    };
    url.set_path(&format!("{}{}", base_path, path));
    let client = provider_client(
        provider_id,
        base_url,
        if provider_id == "local" { 1 } else { 30 },
    )?;
    let mut request = client.get(url).pipe_auth(provider_id, &credential)?;
    if provider_id == "anthropic" {
        request = request.header("anthropic-version", "2023-06-01");
    }
    let response = request
        .send()
        .await
        .map_err(|error| safe_transport_error(&error))?;
    if !response.status().is_success() {
        return Err(provider_http_error(response.status().as_u16()));
    }
    let payload: Value = response
        .json()
        .await
        .map_err(|_| "O provedor retornou uma lista inválida.".to_string())?;
    parse_model_options(payload, entry.model_list_shape.as_deref())
}

fn parse_model_options(
    payload: Value,
    model_list_shape: Option<&str>,
) -> Result<Vec<ModelOption>, String> {
    let values = if model_list_shape == Some("data") {
        payload.get("data").and_then(Value::as_array)
    } else {
        payload.as_array()
    }
    .ok_or("A resposta não corresponde ao formato documentado do provedor.")?;
    Ok(values
        .iter()
        .filter_map(|value| {
            let id = value
                .get("id")
                .or_else(|| value.get("name"))?
                .as_str()?
                .to_string();
            if id.is_empty() || id.len() > 300 {
                return None;
            }
            Some(ModelOption {
                name: id.clone(),
                id,
            })
        })
        .collect())
}

#[tauri::command]
pub async fn test_provider_connection(
    caller: tauri::WebviewWindow,
    requests: State<'_, ProviderRequests>,
    security: State<'_, crate::security::SecurityState>,
    provider_id: String,
    base_url: String,
    model: String,
) -> Result<ConnectionTest, String> {
    super::require_main(&caller)?;
    cancellable_request(
        &requests,
        &security,
        test_provider_connection_inner(&provider_id, &base_url, &model),
    )
    .await
}

async fn test_provider_connection_inner(
    provider_id: &str,
    base_url: &str,
    model: &str,
) -> Result<ConnectionTest, String> {
    let credential = match credential_entry(provider_id)?.get_password() {
        Ok(value) => value,
        Err(keyring::Error::NoEntry) if provider_id == "local" => String::new(),
        _ => return Err("Salve uma credencial para testar a conexão.".into()),
    };
    let endpoint = provider_endpoint(provider_id, base_url)?;
    let mut stages = vec![
        ConnectionStage {
            id: "credential".into(),
            status: if provider_id == "local" && credential.is_empty() {
                "not_checked"
            } else {
                "passed"
            }
            .into(),
            message: if provider_id == "local" && credential.is_empty() {
                "Credencial opcional para este servidor local."
            } else {
                "Credencial encontrada no Gerenciador do Windows."
            }
            .into(),
        },
        ConnectionStage {
            id: "host".into(),
            status: "passed".into(),
            message: format!(
                "Host HTTPS permitido: {}.",
                endpoint.host_str().unwrap_or("")
            ),
        },
    ];
    match fetch_models(provider_id, base_url).await {
        Ok(models) => {
            stages.push(ConnectionStage {
                id: "network".into(),
                status: "passed".into(),
                message: if provider_id == "local" {
                    "Conexão local concluída."
                } else {
                    "Conexão HTTPS concluída."
                }
                .into(),
            });
            stages.push(ConnectionStage {
                id: "authentication".into(),
                status: "passed".into(),
                message: if provider_id == "local" && credential.is_empty() {
                    "O servidor local respondeu sem exigir credencial."
                } else {
                    "A API aceitou a credencial."
                }
                .into(),
            });
            let exists = models.iter().any(|item| item.id == model);
            stages.push(ConnectionStage {
                id: "model".into(),
                status: if exists { "passed" } else { "failed" }.into(),
                message: if exists {
                    "O modelo aparece na lista da conta."
                } else {
                    "O modelo não aparece na lista da conta; confira o identificador."
                }
                .into(),
            });
            Ok(ConnectionTest {
                stages,
                overall: if exists { "passed" } else { "failed" }.into(),
            })
        }
        Err(message) if message.contains("não oferece listagem") => {
            stages.extend([
                ConnectionStage {
                    id: "network".into(),
                    status: "not_checked".into(),
                    message: "Este endpoint não oferece uma verificação sem custo.".into(),
                },
                ConnectionStage {
                    id: "authentication".into(),
                    status: "not_checked".into(),
                    message: "A autenticação só será confirmada ao enviar uma solicitação.".into(),
                },
                ConnectionStage {
                    id: "model".into(),
                    status: "not_checked".into(),
                    message: "Não foi possível confirmar o modelo sem chamar a API de geração."
                        .into(),
                },
            ]);
            Ok(ConnectionTest {
                stages,
                overall: "partial".into(),
            })
        }
        Err(message) => Err(message),
    }
}

fn ping_body(provider_id: &str, model: &str) -> Option<Value> {
    match provider_id {
        "anthropic" => Some(
            json!({ "model": model, "max_tokens": 1, "messages": [{"role":"user","content":"ping"}] }),
        ),
        "perplexity" => None,
        _ => Some(
            json!({ "model": model, "max_tokens": 1, "messages": [{"role":"user","content":"ping"}] }),
        ),
    }
}

#[tauri::command]
pub async fn ping_provider(
    caller: tauri::WebviewWindow,
    requests: State<'_, ProviderRequests>,
    security: State<'_, crate::security::SecurityState>,
    provider_id: String,
    base_url: String,
    model: String,
) -> Result<(), String> {
    super::require_main(&caller)?;
    cancellable_request(
        &requests,
        &security,
        ping_provider_inner(&provider_id, &base_url, &model),
    )
    .await
}

async fn ping_provider_inner(provider_id: &str, base_url: &str, model: &str) -> Result<(), String> {
    let endpoint = provider_endpoint(provider_id, base_url)?;
    let body = ping_body(provider_id, model)
        .ok_or("O ping opcional não está disponível para este adaptador.")?;
    let credential = match credential_entry(provider_id)?.get_password() {
        Ok(value) => value,
        Err(keyring::Error::NoEntry) if provider_id == "local" => String::new(),
        _ => return Err("Salve uma credencial para testar a geração.".into()),
    };
    let client = provider_client(
        provider_id,
        base_url,
        if provider_id == "local" { 1 } else { 30 },
    )?;
    let mut request = client
        .post(endpoint)
        .pipe_auth(provider_id, &credential)?
        .json(&body);
    if provider_id == "anthropic" {
        request = request.header("anthropic-version", "2023-06-01");
    }
    let response = request
        .send()
        .await
        .map_err(|error| safe_transport_error(&error))?;
    if !response.status().is_success() {
        return Err(provider_http_error(response.status().as_u16()));
    }
    Ok(())
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn send_provider_message(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    state: State<'_, ProviderRequests>,
    security: State<'_, crate::security::SecurityState>,
    provider_id: String,
    base_url: String,
    model: String,
    messages: Vec<Message>,
    request_id: String,
) -> Result<ProviderReply, String> {
    super::require_main(&caller)?;
    if security.is_halted() {
        return Err("O OdeKrai está parado pelo kill switch.".into());
    }
    send_message_inner(
        app,
        state,
        provider_id,
        base_url,
        model,
        messages,
        Vec::new(),
        Vec::new(),
        0,
        request_id,
        None,
        security,
    )
    .await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn stream_provider_message(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    state: State<'_, ProviderRequests>,
    security: State<'_, crate::security::SecurityState>,
    provider_id: String,
    base_url: String,
    model: String,
    messages: Vec<Message>,
    tools: Vec<ToolDef>,
    fallbacks: Vec<ProviderTarget>,
    daily_token_limit: u64,
    request_id: String,
    on_chunk: Channel<ProviderEvent>,
) -> Result<ProviderReply, String> {
    super::require_main(&caller)?;
    if security.is_halted() {
        return Err("O OdeKrai está parado pelo kill switch.".into());
    }
    let mut messages = messages;
    if messages.iter().any(|message| message.role == "system") {
        return Err("A mensagem de sistema só pode ser criada pelo aplicativo.".into());
    }
    if let Some(prompt) = super::agent_home::system_prompt(&app)? {
        messages.insert(
            0,
            Message {
                role: "system".into(),
                blocks: vec![Block::Text { text: prompt }],
            },
        );
    }
    send_message_inner(
        app,
        state,
        provider_id,
        base_url,
        model,
        messages,
        tools,
        fallbacks,
        daily_token_limit,
        request_id,
        Some(EventSink::Channel(on_chunk)),
        security,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn send_message_inner(
    app: tauri::AppHandle,
    state: State<'_, ProviderRequests>,
    provider_id: String,
    base_url: String,
    model: String,
    messages: Vec<Message>,
    tools: Vec<ToolDef>,
    fallbacks: Vec<ProviderTarget>,
    daily_token_limit: u64,
    request_id: String,
    on_chunk: Option<EventSink>,
    security: State<'_, crate::security::SecurityState>,
) -> Result<ProviderReply, String> {
    if security.is_halted() {
        return Err("O OdeKrai está parado pelo kill switch.".into());
    }
    if request_id.is_empty()
        || request_id.len() > 80
        || !request_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err("Identificador de solicitação inválido.".into());
    }
    if model.trim().is_empty() || model.len() > 200 {
        return Err("Informe um ID de modelo válido.".into());
    }
    let messages = normalize_request_images(messages)?;
    if !valid_messages(&messages) || !valid_tools(&tools) {
        return Err("A conversa está vazia ou excede o limite permitido.".into());
    }
    if on_chunk.is_none() && messages.iter().any(|message| message.role == "system") {
        return Err("A mensagem de sistema só pode ser criada pelo aplicativo.".into());
    }
    let mut targets = vec![ProviderTarget {
        provider_id,
        base_url,
        model,
    }];
    targets.extend(fallbacks);
    if targets.len() > 16 {
        return Err("A lista de provedores excede o limite permitido.".into());
    }
    let targets = eligible_provider_targets(&app, targets, !tools.is_empty())?;
    if targets.is_empty() {
        return Err("Nenhum provedor configurado tem credencial e capacidades disponíveis.".into());
    }
    let used_today = crate::history::provider_usage_total_today(&app)?;
    if crate::history::daily_token_limit_exceeded(used_today, daily_token_limit) {
        crate::security::audit_provider_usage_limit(
            &app,
            &security,
            &targets[0].provider_id,
            used_today,
            daily_token_limit,
        )?;
        return Err(
            "O limite diário de tokens já foi ultrapassado. Tente novamente amanhã ou ajuste o limite em Configurações."
                .into(),
        );
    }

    let cancellation = CancellationToken::new();
    {
        let mut requests = state
            .0
            .lock()
            .map_err(|_| "Não foi possível iniciar a solicitação.".to_string())?;
        if requests.contains_key(&request_id) {
            return Err("Já existe uma solicitação com este identificador.".into());
        }
        requests.insert(request_id.clone(), cancellation.clone());
    }
    if security.is_halted() {
        cancellation.cancel();
        if let Ok(mut requests) = state.0.lock() {
            requests.remove(&request_id);
        }
        return Err("O OdeKrai está parado pelo kill switch.".into());
    }

    let observed_output = Arc::new(AtomicBool::new(false));
    let result = tokio::select! {
        _ = cancellation.cancelled() => Err("Solicitação cancelada.".into()),
        response = async {
            let request = ChatRequest { model: String::new(), messages, tools, streaming: on_chunk.is_some() };
            let response = failover::try_candidates(
                &targets,
                |target| {
                    let mut attempt = request.clone();
                    attempt.model = target.model.clone();
                    let provider_id = target.provider_id.clone();
                    let base_url = target.base_url.clone();
                    let channel = on_chunk.clone();
                    let seen_output = observed_output.clone();
                    let app = &app;
                    async move {
                        if let Some(channel) = channel.as_ref() {
                            channel.send(ProviderEvent::ProviderSelected { provider_id: provider_id.clone() });
                        }
                        request_provider(app, &provider_id, &base_url, attempt, channel, Some(seen_output)).await
                    }
                },
                |from, to, reason| crate::security::audit_provider_fallback(&app, &security, &from.provider_id, &to.provider_id, reason),
                || observed_output.load(Ordering::SeqCst),
            ).await;
            let (index, reply) = match response {
                Ok(result) => result,
                Err(error) => {
                    if let Some(channel) = on_chunk.as_ref() {
                        let kind = if failover::is_offline_network_failure(&error) {
                            "offline_network"
                        } else {
                            "provider"
                        };
                        channel.send(ProviderEvent::Error { kind: kind.into(), status: None, message: error.clone() });
                    }
                    return Err(error);
                }
            };
            let target = &targets[index];
            if let Some(usage) = reply.usage.as_ref() {
                let total = crate::history::record_provider_usage(&app, &target.provider_id, usage.input, usage.output)?;
                if crate::history::daily_token_limit_exceeded(total, daily_token_limit) {
                    crate::security::audit_provider_usage_limit(&app, &security, &target.provider_id, total, daily_token_limit)?;
                    if let Some(channel) = on_chunk.as_ref() {
                        channel.send(ProviderEvent::Error {
                            kind: "daily_token_limit".into(), status: None,
                            message: "O limite diário de tokens foi atingido. A resposta foi interrompida.".into(),
                        });
                    }
                    return Err("O limite diário de tokens foi atingido. Ajuste o limite em Configurações ou tente novamente amanhã.".into());
                }
            }
            Ok(reply)
        } => response,
    };
    if let Ok(mut requests) = state.0.lock() {
        requests.remove(&request_id);
    }
    result
}

fn eligible_provider_targets(
    app: &tauri::AppHandle,
    targets: Vec<ProviderTarget>,
    tools_required: bool,
) -> Result<Vec<ProviderTarget>, String> {
    let mut seen = std::collections::HashSet::new();
    failover::filter_candidates(targets, |target| {
        if !seen.insert(target.provider_id.clone()) {
            return Ok(false);
        }
        let Some(entry) = catalog_entry(&target.provider_id) else {
            return Ok(false);
        };
        if target.provider_id.len() > 80
            || target.base_url.len() > 2_000
            || target.model.trim().is_empty()
            || target.model.len() > 200
        {
            return Ok(false);
        }
        let has_credential = match credential_entry(&target.provider_id)?.get_password() {
            Ok(_) => true,
            Err(keyring::Error::NoEntry) => false,
            Err(_) => {
                return Err("Não foi possível consultar as credenciais dos provedores.".into())
            }
        };
        if !has_credential {
            return Ok(false);
        }
        if tools_required {
            let capabilities = effective_capabilities(app, &target.provider_id, &target.model)?;
            if !entry.capabilities.tools || !capabilities.tools || !capabilities.verified {
                return Ok(false);
            }
        }
        Ok(true)
    })
}

fn valid_messages(messages: &[Message]) -> bool {
    if messages.is_empty() || messages.len() > 100 {
        return false;
    }
    let mut total_bytes = 0usize;
    for message in messages {
        if !matches!(
            message.role.as_str(),
            "system" | "user" | "assistant" | "tool"
        ) {
            return false;
        }
        let mut image_count = 0usize;
        for block in &message.blocks {
            let size = match block {
                Block::Text { text } | Block::ToolResult { content: text, .. } => text.len(),
                Block::ToolCall { id, name, args } => {
                    id.len() + name.len() + args.to_string().len()
                }
                Block::Image {
                    media_type,
                    data_base64,
                } => {
                    image_count += 1;
                    if message.role != "user"
                        || media_type != "image/jpeg"
                        || data_base64.is_empty()
                        || data_base64.len() > crate::screen_capture::MAX_SCREENSHOT_BYTES * 2
                    {
                        return false;
                    }
                    use base64::Engine as _;
                    if base64::engine::general_purpose::STANDARD
                        .decode(data_base64)
                        .is_err()
                    {
                        return false;
                    }
                    data_base64.len()
                }
            };
            if image_count > 1 {
                return false;
            }
            if size > MAX_MESSAGE_BYTES {
                return false;
            }
            total_bytes = total_bytes.saturating_add(size);
            if total_bytes > MAX_MESSAGE_BYTES {
                return false;
            }
        }
    }
    true
}

fn normalize_request_images(mut messages: Vec<Message>) -> Result<Vec<Message>, String> {
    use base64::Engine as _;
    for message in &mut messages {
        for block in &mut message.blocks {
            if let Block::Image {
                media_type,
                data_base64,
            } = block
            {
                if media_type != "image/jpeg"
                    || data_base64.len() > crate::screen_capture::MAX_SCREENSHOT_BYTES * 2
                {
                    return Err(
                        "A imagem excede o limite permitido ou tem formato inválido.".into(),
                    );
                }
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(data_base64.as_str())
                    .map_err(|_| "A imagem temporária é inválida.".to_string())?;
                if bytes.len() > crate::screen_capture::MAX_SCREENSHOT_BYTES {
                    return Err("A imagem excede o limite de 512 KiB.".into());
                }
                let (normalized, _, _) = crate::screen_capture::normalize_image(&bytes)?;
                *data_base64 = base64::engine::general_purpose::STANDARD.encode(normalized);
            }
        }
    }
    Ok(messages)
}

fn valid_tools(tools: &[ToolDef]) -> bool {
    tools.len() <= 64
        && tools.iter().all(|tool| {
            !tool.name.is_empty()
                && tool.name.len() <= 64
                && tool
                    .name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
                && tool.description.len() <= 4_000
                && tool.json_schema.is_object()
                && tool.json_schema.to_string().len() <= 64 * 1024
        })
}

async fn request_provider(
    app: &tauri::AppHandle,
    provider_id: &str,
    base_url: &str,
    request_data: ChatRequest,
    on_chunk: Option<EventSink>,
    observed_output: Option<Arc<AtomicBool>>,
) -> Result<ProviderReply, String> {
    let (request_data, api_tool_names) = if provider_id == "openai" {
        let (request, names) = chat_completions::normalize_tool_names(request_data)?;
        (request, Some(names))
    } else {
        (request_data, None)
    };
    if request_data.messages.iter().any(|message| {
        message
            .blocks
            .iter()
            .any(|block| matches!(block, Block::Image { .. }))
    }) && !supports_vision_model(provider_id, &request_data.model)
    {
        return Err(
            "O provedor e o modelo selecionados não têm suporte a imagens verificado.".into(),
        );
    }
    let endpoint = provider_endpoint(provider_id, base_url)?;
    let entry = credential_entry(provider_id)?;
    let credential = entry.get_password().map_err(|error| match error {
        keyring::Error::NoEntry if provider_id == "local" => String::new(),
        keyring::Error::NoEntry => {
            "Salve uma credencial para este provedor antes de conversar.".to_string()
        }
        _ => "Não foi possível recuperar a credencial do Gerenciador do Windows.".to_string(),
    })?;
    let client = provider_client(
        provider_id,
        base_url,
        if provider_id == "local" { 30 } else { 90 },
    )?;

    let chat_capabilities = effective_capabilities(app, provider_id, &request_data.model)?;
    let protocol_capabilities = match provider_id {
        "anthropic" => AnthropicProvider.capabilities(),
        "perplexity" => PerplexityProvider.capabilities(),
        _ => ChatCompletionsProvider {
            capabilities: chat_capabilities,
            include_usage: provider_id == "openai",
            reasoning_effort: None,
        }
        .capabilities(),
    };
    let capabilities = ProviderCapabilities {
        streaming: chat_capabilities.streaming && protocol_capabilities.streaming,
        tools: chat_capabilities.tools && protocol_capabilities.tools,
        vision: chat_capabilities.vision && protocol_capabilities.vision,
        image_generation: chat_capabilities.image_generation
            && protocol_capabilities.image_generation,
        verified: chat_capabilities.verified && protocol_capabilities.verified,
    };
    if (!request_data.tools.is_empty()
        || request_data.messages.iter().any(|message| {
            message
                .blocks
                .iter()
                .any(|block| matches!(block, Block::ToolCall { .. } | Block::ToolResult { .. }))
        }))
        && (!capabilities.tools || !capabilities.verified)
    {
        return Err("O uso de ferramentas não está verificado para este provedor.".into());
    }

    let response = match provider_id {
        "anthropic" => {
            AnthropicProvider
                .chat_stream(&client, endpoint, provider_id, &credential, &request_data)
                .await?
        }
        "perplexity" => {
            PerplexityProvider
                .chat_stream(&client, endpoint, provider_id, &credential, &request_data)
                .await?
        }
        _ => {
            ChatCompletionsProvider {
                capabilities: chat_capabilities,
                include_usage: provider_id == "openai",
                reasoning_effort: openai_reasoning_effort(
                    provider_id,
                    &request_data.model,
                    !request_data.tools.is_empty(),
                ),
            }
            .chat_stream(&client, endpoint, provider_id, &credential, &request_data)
            .await?
        }
    };
    let status = response.status();
    if !status.is_success() {
        let status_code = status.as_u16();
        let response_body = read_provider_error_body(response).await;
        if status_code == 400
            && !request_data.tools.is_empty()
            && cache_unsupported_tools_response(status.as_u16(), true, &response_body, || {
                crate::history::cache_tools_unsupported(app, provider_id, &request_data.model)
            })?
        {
            let message =
                "Este modelo recusou o uso de ferramentas. O OdeKrai guardou essa informação para este modelo."
                    .to_string();
            if let Some(channel) = on_chunk.as_ref() {
                channel.send(ProviderEvent::Error {
                    kind: "unsupported_tools".into(),
                    status: Some(400),
                    message: message.clone(),
                });
            }
            return Err(message);
        }
        if let Some(channel) = on_chunk.as_ref() {
            let message = provider_http_error_with_body(status_code, &response_body);
            channel.send(ProviderEvent::Error {
                kind: "http".into(),
                status: Some(status_code),
                message,
            });
        }
        return Err(provider_http_error_with_body(status_code, &response_body));
    }
    if let Some(channel) = on_chunk {
        if provider_id != "perplexity" {
            let event_names = api_tool_names.clone();
            let mut reply = consume_stream_response(provider_id, response, move |mut event| {
                if matches!(
                    &event,
                    ProviderEvent::TextDelta { .. } | ProviderEvent::ToolCallComplete { .. }
                ) {
                    if let Some(observed_output) = observed_output.as_ref() {
                        observed_output.store(true, Ordering::SeqCst);
                    }
                }
                let valid_tool_name = event_names
                    .as_ref()
                    .is_none_or(|names| chat_completions::restore_event_name(&mut event, names));
                if valid_tool_name {
                    channel.send(event);
                } else {
                    channel.send(ProviderEvent::Error {
                        kind: "invalid_response".into(),
                        status: None,
                        message: "O provedor retornou uma ferramenta que não foi anunciada.".into(),
                    });
                }
            })
            .await?;
            if let Some(names) = api_tool_names.as_ref() {
                chat_completions::restore_tool_names(&mut reply, names)?;
            }
            return Ok(reply);
        }
    }
    let payload: Value = response
        .json()
        .await
        .map_err(|_| "O provedor retornou uma resposta inválida.".to_string())?;
    let mut reply = parse_provider_reply(provider_id, &payload)?;
    if let Some(names) = api_tool_names.as_ref() {
        chat_completions::restore_tool_names(&mut reply, names)?;
    }
    Ok(reply)
}

fn openai_reasoning_effort(
    provider_id: &str,
    model: &str,
    tools_requested: bool,
) -> Option<&'static str> {
    (provider_id == "openai" && model == "gpt-6-luna" && tools_requested).then_some("none")
}

pub(crate) async fn agent_model_turn(
    app: &tauri::AppHandle,
    security: &crate::security::SecurityState,
    configured_targets: Vec<crate::history::ConversationProviderTarget>,
    messages: &[Message],
    tools: &[ToolDef],
    daily_token_limit: u64,
) -> Result<ProviderReply, String> {
    if !valid_tools(tools) {
        return Err("As ferramentas disponíveis para o agente têm definições inválidas.".into());
    }
    let targets = configured_targets
        .into_iter()
        .map(|target| ProviderTarget {
            base_url: if target.base_url.trim().is_empty() {
                catalog_entry(&target.provider_id)
                    .map_or(target.base_url.clone(), |entry| entry.base_url)
            } else {
                target.base_url
            },
            provider_id: target.provider_id,
            model: target.model,
        })
        .collect();
    let mut targets = eligible_provider_targets(app, targets, true)?;
    if messages.iter().any(|message| {
        message
            .blocks
            .iter()
            .any(|block| matches!(block, Block::Image { .. }))
    }) {
        targets.retain(|target| supports_vision_model(&target.provider_id, &target.model));
    }
    if targets.is_empty() {
        return Err("Modo agente indisponível: nenhum provedor configurado tem ferramentas verificadas e credencial válida.".into());
    }
    let used_today = crate::history::provider_usage_total_today(app)?;
    if crate::history::daily_token_limit_exceeded(used_today, daily_token_limit) {
        crate::security::audit_provider_usage_limit(
            app,
            security,
            &targets[0].provider_id,
            used_today,
            daily_token_limit,
        )?;
        return Err("O limite diário de tokens já foi ultrapassado. Tente novamente amanhã ou ajuste o limite em Configurações.".into());
    }
    let app_handle = app.clone();
    let observed = Arc::new(AtomicBool::new(false));
    let sink = EventSink::Callback(Arc::new(|_| {}));
    let result = failover::try_candidates(
        &targets,
        |candidate| {
            let app = app_handle.clone();
            let provider_id = candidate.provider_id.clone();
            let base_url = candidate.base_url.clone();
            let sink = sink.clone();
            let observed = observed.clone();
            let request = ChatRequest {
                model: candidate.model.clone(),
                messages: messages.to_vec(),
                tools: tools.to_vec(),
                streaming: true,
            };
            async move {
                request_provider(
                    &app,
                    &provider_id,
                    &base_url,
                    request,
                    Some(sink),
                    Some(observed),
                )
                .await
            }
        },
        |from, to, reason| {
            crate::security::audit_provider_fallback(
                app,
                security,
                &from.provider_id,
                &to.provider_id,
                reason,
            )
        },
        || observed.load(Ordering::SeqCst),
    )
    .await?;
    let (index, reply) = result;
    if let Some(usage) = reply.usage.as_ref() {
        let total = crate::history::record_provider_usage(
            app,
            &targets[index].provider_id,
            usage.input,
            usage.output,
        )?;
        if crate::history::daily_token_limit_exceeded(total, daily_token_limit) {
            crate::security::audit_provider_usage_limit(
                app,
                security,
                &targets[index].provider_id,
                total,
                daily_token_limit,
            )?;
            return Err("O limite diário de tokens foi atingido. A resposta foi interrompida; ajuste o limite ou tente novamente amanhã.".into());
        }
    }
    Ok(reply)
}

pub(crate) fn verified_tool_support(
    app: &tauri::AppHandle,
    provider_id: &str,
    model: &str,
) -> bool {
    effective_capabilities(app, provider_id, model)
        .is_ok_and(|capabilities| capabilities.tools && capabilities.verified)
}

pub(crate) fn agent_provider_unavailable_reason(
    app: &tauri::AppHandle,
    provider_id: &str,
    model: &str,
) -> Option<&'static str> {
    let Some(entry) = catalog_entry(provider_id) else {
        return Some("Este provedor não está disponível no OdeKrai.");
    };
    if !entry.capabilities.tools {
        return Some("Este provedor não oferece suporte a ferramentas.");
    }
    if provider_id != "local" {
        let credential = match credential_entry(provider_id) {
            Ok(credential) => credential,
            Err(_) => {
                return Some("Não foi possível consultar a credencial no Gerenciador do Windows.")
            }
        };
        match credential.get_password() {
            Ok(_) => {}
            Err(keyring::Error::NoEntry) => {
                return Some("Salve uma credencial para este provedor em Conectores de IA.")
            }
            Err(_) => {
                return Some("Não foi possível consultar a credencial no Gerenciador do Windows.")
            }
        }
    }
    match effective_capabilities(app, provider_id, model) {
        Ok(capabilities) if capabilities.tools && capabilities.verified => None,
        _ => Some("Este modelo não tem suporte verificado a ferramentas. Escolha outro modelo em Conectores de IA."),
    }
}

pub(crate) fn agent_provider_ready(app: &tauri::AppHandle, provider_id: &str, model: &str) -> bool {
    let Some(entry) = catalog_entry(provider_id) else {
        return false;
    };
    eligible_provider_targets(
        app,
        vec![ProviderTarget {
            provider_id: provider_id.to_string(),
            base_url: entry.base_url,
            model: model.to_string(),
        }],
        true,
    )
    .is_ok_and(|targets| !targets.is_empty())
}

fn looks_like_unsupported_tools(response_body: &str) -> bool {
    let body = response_body.to_ascii_lowercase();
    [
        "does not support tools",
        "doesn't support tools",
        "does not support tool calling",
        "tool calling is not supported",
        "function calling is not supported",
        "tools are not supported for this model",
        "tool use is not supported for this model",
    ]
    .iter()
    .any(|phrase| body.contains(phrase))
}

fn cache_unsupported_tools_response(
    status: u16,
    tools_requested: bool,
    response_body: &str,
    cache: impl FnOnce() -> Result<(), String>,
) -> Result<bool, String> {
    if status == 400 && tools_requested && looks_like_unsupported_tools(response_body) {
        cache()?;
        Ok(true)
    } else {
        Ok(false)
    }
}

#[cfg(test)]
fn stream_delta<'a>(provider_id: &str, payload: &'a Value) -> Option<&'a str> {
    if provider_id == "anthropic" {
        if payload["type"] == "content_block_delta" && payload["delta"]["type"] == "text_delta" {
            payload["delta"]["text"].as_str()
        } else {
            None
        }
    } else {
        payload["choices"][0]["delta"]["content"].as_str()
    }
}

fn parse_provider_reply(provider_id: &str, payload: &Value) -> Result<ProviderReply, String> {
    if provider_id == "perplexity" {
        return perplexity::parse_reply(payload);
    }

    let (content, tool_calls, usage, stop_reason) = if provider_id == "anthropic" {
        let blocks = payload["content"]
            .as_array()
            .ok_or_else(|| "O provedor retornou resposta em formato inesperado.".to_string())?;
        let content = blocks
            .iter()
            .filter(|part| part["type"] == "text")
            .filter_map(|part| part["text"].as_str())
            .collect::<Vec<_>>()
            .join("");
        let tool_calls = blocks
            .iter()
            .filter(|part| part["type"] == "tool_use")
            .map(|part| {
                let args = part
                    .get("input")
                    .cloned()
                    .filter(Value::is_object)
                    .ok_or_else(|| {
                        "O provedor retornou argumentos de ferramenta inválidos.".to_string()
                    })?;
                Ok(ToolCall {
                    id: part["id"].as_str().unwrap_or_default().to_owned(),
                    name: part["name"].as_str().unwrap_or_default().to_owned(),
                    args,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let usage = Some(Usage {
            input: payload["usage"]["input_tokens"].as_u64().unwrap_or(0),
            output: payload["usage"]["output_tokens"].as_u64().unwrap_or(0),
        });
        (
            content,
            tool_calls,
            usage,
            payload["stop_reason"].as_str().map(str::to_owned),
        )
    } else {
        let message = &payload["choices"][0]["message"];
        let content = message["content"].as_str().unwrap_or_default().to_owned();
        let tool_calls = message["tool_calls"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|call| {
                let raw = call["function"]["arguments"].as_str().unwrap_or_default();
                let args = serde_json::from_str(raw).map_err(|_| {
                    "O provedor retornou argumentos de ferramenta inválidos.".to_string()
                })?;
                Ok(ToolCall {
                    id: call["id"].as_str().unwrap_or_default().to_owned(),
                    name: call["function"]["name"]
                        .as_str()
                        .unwrap_or_default()
                        .to_owned(),
                    args,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let usage = payload
            .get("usage")
            .filter(|value| value.is_object())
            .map(|usage| Usage {
                input: usage["prompt_tokens"].as_u64().unwrap_or(0),
                output: usage["completion_tokens"].as_u64().unwrap_or(0),
            });
        (
            content,
            tool_calls,
            usage,
            payload["choices"][0]["finish_reason"]
                .as_str()
                .map(str::to_owned),
        )
    };

    if content.trim().is_empty() && tool_calls.is_empty() {
        return Err("O provedor retornou uma resposta vazia ou em formato inesperado.".into());
    }
    Ok(ProviderReply {
        content: content.trim().to_string(),
        citations: Vec::new(),
        tool_calls,
        usage,
        stop_reason,
    })
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kill_switch_cancels_every_registered_provider_request() {
        let state = ProviderRequests::default();
        let first = CancellationToken::new();
        let second = CancellationToken::new();
        {
            let mut requests = state.0.lock().unwrap();
            requests.insert("one".into(), first.clone());
            requests.insert("two".into(), second.clone());
        }
        cancel_requests(&state);
        assert!(first.is_cancelled());
        assert!(second.is_cancelled());
    }

    #[tokio::test]
    async fn cancellable_request_returns_when_its_token_is_cancelled() {
        let requests = ProviderRequests::default();
        let security = crate::security::SecurityState::new(false);
        let request = cancellable_request(
            &requests,
            &security,
            std::future::pending::<Result<(), String>>(),
        );
        tokio::pin!(request);
        tokio::select! {
            result = &mut request => panic!("request unexpectedly completed: {result:?}"),
            _ = tokio::task::yield_now() => {},
        }
        cancel_requests(&requests);
        let error = request.await.unwrap_err();
        assert!(error.contains("kill switch"));
    }

    use serde_json::json;

    #[test]
    fn perplexity_uses_instructions_for_system_prompt() {
        let body = perplexity::perplexity_body(
            "sonar",
            &[
                ProviderMessage {
                    role: "system".into(),
                    content: "Regras fixas".into(),
                },
                ProviderMessage {
                    role: "user".into(),
                    content: "Olá".into(),
                },
            ],
        );
        assert_eq!(body["instructions"], "Regras fixas");
        assert_eq!(body["input"], "user: Olá");
    }

    #[test]
    fn builds_provider_specific_endpoints_without_allowing_host_confusion() {
        assert_eq!(
            provider_endpoint("anthropic", "https://api.anthropic.com")
                .unwrap()
                .as_str(),
            "https://api.anthropic.com/v1/messages"
        );
        assert_eq!(
            provider_endpoint("anthropic", "https://api.anthropic.com/v1/messages")
                .unwrap()
                .as_str(),
            "https://api.anthropic.com/v1/messages"
        );
        assert_eq!(
            provider_endpoint("perplexity", "https://api.perplexity.ai/v1/agent")
                .unwrap()
                .as_str(),
            "https://api.perplexity.ai/v1/agent"
        );
        assert!(provider_endpoint("openai", "https://api.openai.com.attacker.invalid/v1").is_err());
        assert!(provider_endpoint("openai", "http://api.openai.com/v1").is_err());
        assert!(provider_endpoint("vertex-ai", "https://aiplatform.googleapis.com").is_err());
        assert!(provider_endpoint("groq", "https://api.x.ai/v1").is_err());
        assert!(provider_endpoint("groq", "https://api.groq.com/v1").is_err());
        assert_eq!(
            provider_endpoint("anthropic", "https://api.anthropic.com/v1/messages")
                .unwrap()
                .path(),
            "/v1/messages"
        );
    }

    #[test]
    fn local_provider_accepts_only_loopback_http() {
        assert_eq!(
            provider_endpoint("local", "http://127.0.0.1:11434/v1")
                .unwrap()
                .path(),
            "/v1/chat/completions"
        );
        assert_eq!(
            provider_endpoint("local", "http://[::1]:8080/v1")
                .unwrap()
                .host_str(),
            Some("[::1]")
        );
        assert!(provider_endpoint("local", "http://localhost:11434/v1").is_ok());
        assert!(provider_endpoint("local", "http://192.168.1.2:11434/v1").is_err());
        assert!(provider_endpoint("local", "https://127.0.0.1:11434/v1").is_err());
        assert!(provider_endpoint("local", "http://example.com:11434/v1").is_err());
    }

    #[test]
    fn local_model_listing_parses_openai_data_and_rejects_invalid_payloads() {
        let models =
            parse_model_options(json!({"data":[{"id":"qwen-local"}]}), Some("data")).unwrap();
        assert_eq!(models[0].id, "qwen-local");
        assert!(parse_model_options(json!({"models": []}), Some("data")).is_err());
    }

    #[test]
    fn local_discovery_separates_online_service_from_loaded_models() {
        let empty =
            finish_local_discovery(vec!["http://127.0.0.1:11434/v1".into()], Vec::new(), false)
                .unwrap();
        assert_eq!(empty.online_endpoints.len(), 1);
        assert!(empty.models.is_empty());

        assert_eq!(
            finish_local_discovery(Vec::new(), Vec::new(), false).unwrap_err(),
            "Servidor local não encontrado nas portas verificadas."
        );
        assert!(finish_local_discovery(Vec::new(), Vec::new(), true)
            .unwrap_err()
            .contains("dados inválidos"));
    }

    #[test]
    fn local_probe_port_is_optional_bounded_and_loopback_only() {
        let defaults = local_model_probe_endpoints(None).unwrap();
        assert_eq!(defaults.len(), 3);
        assert!(defaults
            .iter()
            .all(|(_, url)| url.starts_with("http://127.0.0.1:")));
        assert!(defaults.iter().any(|(port, _)| *port == 9931));

        for port in [0, 1023, 65536] {
            assert!(local_model_probe_endpoints(Some(port))
                .unwrap_err()
                .contains("1024 e 65535"));
        }
        assert!(local_model_probe_endpoints(Some(1024))
            .unwrap()
            .contains(&(1024, "http://127.0.0.1:1024/v1".to_string())));
        assert!(local_model_probe_endpoints(Some(65535))
            .unwrap()
            .contains(&(65535, "http://127.0.0.1:65535/v1".to_string())));
    }

    #[tokio::test]
    async fn local_server_absence_fails_without_network_access() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let client = provider_client("local", &format!("http://127.0.0.1:{port}/v1"), 1).unwrap();
        let result = client
            .get(format!("http://127.0.0.1:{port}/v1/models"))
            .send()
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn local_mock_server_lists_models() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let payload = r#"{"data":[{"id":"qwen-mock"}]}"#.to_string();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = [0_u8; 1024];
            let _ = socket.read(&mut request);
            write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", payload.len(), payload).unwrap();
        });
        let client = provider_client(
            "local",
            &format!("http://127.0.0.1:{}/v1", address.port()),
            2,
        )
        .unwrap();
        let response = client
            .get(format!("http://{address}/v1/models"))
            .send()
            .await
            .unwrap();
        let payload = response.json::<Value>().await.unwrap();
        server.join().unwrap();
        let models = parse_model_options(payload, Some("data")).unwrap();
        assert_eq!(models[0].id, "qwen-mock");
    }

    #[tokio::test]
    async fn local_mock_server_redirect_is_returned_without_following() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = [0_u8; 1024];
            let _ = socket.read(&mut request);
            write!(socket, "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{}/v1/models\r\nContent-Length: 0\r\nConnection: close\r\n\r\n", address.port()).unwrap();
        });
        let client = provider_client(
            "local",
            &format!("http://127.0.0.1:{}/v1", address.port()),
            2,
        )
        .unwrap();
        let response = client
            .get(format!("http://{address}/v1/models"))
            .send()
            .await
            .unwrap();
        server.join().unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::FOUND);
    }

    #[test]
    fn provider_catalog_deserializes_from_embedded_resource() {
        let anthropic = catalog_entry("anthropic").unwrap();
        assert_eq!(anthropic.auth_header.as_deref(), Some("x-api-key"));
        assert_eq!(anthropic.chat_path.as_deref(), Some("/v1/messages"));
        assert_eq!(anthropic.model_list_path.as_deref(), Some("/v1/models"));
    }

    #[test]
    fn provider_catalog_declares_only_verified_tool_adapters() {
        let entries = serde_json::from_str::<Vec<CatalogEntry>>(include_str!(
            "../../resources/provider-catalog.json"
        ))
        .unwrap();
        for entry in entries {
            if matches!(
                entry.id.as_str(),
                "openai" | "anthropic" | "groq" | "cerebras" | "mistral" | "openrouter"
            ) {
                assert!(entry.capabilities.tools);
                assert!(entry.capabilities.verified);
            } else {
                assert!(!entry.capabilities.tools, "{}", entry.id);
                assert!(!entry.capabilities.verified, "{}", entry.id);
            }
        }
    }

    #[test]
    fn third_party_tool_capabilities_have_current_verification_dates() {
        let entries = serde_json::from_str::<Vec<Value>>(include_str!(
            "../../resources/provider-catalog.json"
        ))
        .unwrap();
        for id in ["gemini", "groq", "cerebras", "mistral", "openrouter"] {
            let entry = entries.iter().find(|entry| entry["id"] == id).unwrap();
            assert_eq!(entry["verifiedAt"], "2026-10-25", "{id}");
        }
        let gemini = entries
            .iter()
            .find(|entry| entry["id"] == "gemini")
            .unwrap();
        assert!(!gemini["capabilities"]["tools"].as_bool().unwrap());
        assert!(!gemini["capabilities"]["verified"].as_bool().unwrap());
    }

    #[test]
    fn cached_model_rejection_overrides_declared_tools_support() {
        let declared = ProviderCapabilities {
            streaming: true,
            tools: true,
            vision: false,
            image_generation: false,
            verified: true,
        };
        assert!(!capabilities_with_model_cache(declared, Some(false)).tools);
        assert!(capabilities_with_model_cache(declared, None).tools);
        let unverified = ProviderCapabilities {
            verified: false,
            ..declared
        };
        let opted_in = capabilities_with_model_cache(unverified, Some(true));
        assert!(opted_in.tools && opted_in.verified);
    }

    #[test]
    fn unsupported_tool_response_detection_is_narrow_and_sanitized() {
        assert!(looks_like_unsupported_tools(
            r#"{"error":{"message":"This model does not support tools"}}"#
        ));
        assert!(!looks_like_unsupported_tools(
            r#"{"error":{"message":"Invalid JSON body"}}"#
        ));
    }

    #[test]
    fn mock_http_400_for_unsupported_tools_populates_model_cache_without_exposing_body() {
        let mut cache_written = false;
        let provider_body =
            r#"{"error":{"message":"This model does not support tools; secret detail"}}"#;
        let handled = cache_unsupported_tools_response(400, true, provider_body, || {
            cache_written = true;
            Ok(())
        })
        .unwrap();
        assert!(handled && cache_written);
        let safe_message = provider_http_error(400);
        assert!(!safe_message.contains("secret detail"));

        let mut not_cached = false;
        assert!(
            !cache_unsupported_tools_response(400, true, "Invalid request", || {
                not_cached = true;
                Ok(())
            })
            .unwrap()
        );
        assert!(!not_cached);
    }

    #[test]
    fn parses_nonstream_tool_calls_and_usage_for_both_verified_adapters() {
        let openai = parse_provider_reply(
            "openai",
            &json!({
                "choices": [{ "finish_reason": "tool_calls", "message": {
                    "content": null,
                    "tool_calls": [{ "id": "call-1", "function": {
                        "name": "lookup", "arguments": "{\"q\":\"ok\"}"
                    }}]
                }}],
                "usage": { "prompt_tokens": 8, "completion_tokens": 2 }
            }),
        )
        .unwrap();
        assert!(openai.content.is_empty());
        assert_eq!(openai.tool_calls[0].args, json!({ "q": "ok" }));
        assert_eq!(
            openai.usage,
            Some(Usage {
                input: 8,
                output: 2
            })
        );

        let anthropic = parse_provider_reply(
            "anthropic",
            &json!({
                "stop_reason": "tool_use",
                "content": [{ "type": "tool_use", "id": "toolu-1", "name": "lookup", "input": { "q": "ok" } }],
                "usage": { "input_tokens": 8, "output_tokens": 2 }
            }),
        )
        .unwrap();
        assert_eq!(anthropic.tool_calls[0].args, json!({ "q": "ok" }));
        assert_eq!(
            anthropic.usage,
            Some(Usage {
                input: 8,
                output: 2
            })
        );
    }

    #[test]
    fn rejects_invalid_nonstream_tool_arguments() {
        let response = json!({
            "choices": [{ "finish_reason": "tool_calls", "message": {
                "content": null,
                "tool_calls": [{ "id": "call-1", "function": {
                    "name": "lookup", "arguments": "{broken"
                }}]
            }}]
        });
        assert!(parse_provider_reply("openai", &response).is_err());
        assert!(parse_provider_reply("openai", &json!({})).is_err());
        assert!(parse_provider_reply("openai", &json!({ "choices": [] })).is_err());
    }

    #[test]
    fn unsupported_provider_is_rejected_before_accessing_credential_store() {
        let error = credential_entry("vertex-ai").unwrap_err();
        assert!(error.contains("não aceita credenciais"));
    }

    #[test]
    fn maps_provider_statuses_to_safe_portuguese_errors() {
        for (status, expected) in [
            (400, "HTTP 400"),
            (401, "HTTP 401"),
            (403, "HTTP 403"),
            (404, "HTTP 404"),
            (408, "HTTP 408"),
            (429, "HTTP 429"),
            (503, "HTTP 503"),
        ] {
            let message = provider_http_error(status);
            assert!(message.contains(expected));
            assert!(!message.contains("test-secret"));
        }
        assert!(provider_http_error(429).contains("frequência das requisições"));
    }

    #[test]
    fn openai_gpt_6_luna_requests_tool_compatible_reasoning_only_for_tools() {
        assert_eq!(
            openai_reasoning_effort("openai", "gpt-6-luna", true),
            Some("none")
        );
        assert_eq!(openai_reasoning_effort("openai", "gpt-6-luna", false), None);
        assert_eq!(openai_reasoning_effort("local", "gpt-6-luna", true), None);
        assert_eq!(openai_reasoning_effort("openai", "other-model", true), None);
    }

    #[test]
    fn validates_internal_skill_ids_before_provider_name_normalization() {
        let skill_tool = ToolDef {
            name: "google.calendar.update_event".into(),
            description: "Atualiza um evento".into(),
            json_schema: json!({ "type": "object", "properties": {}, "required": [], "additionalProperties": false }),
        };
        assert!(valid_tools(std::slice::from_ref(&skill_tool)));

        let invalid_tool = ToolDef {
            name: "google calendar".into(),
            ..skill_tool
        };
        assert!(!valid_tools(&[invalid_tool]));
    }

    #[test]
    fn optional_ping_is_limited_to_one_output_token() {
        for provider in ["openai", "anthropic", "groq", "azure-openai", "bedrock"] {
            let body = ping_body(provider, "model").unwrap();
            assert_eq!(body["max_tokens"], 1);
            assert_eq!(body["messages"][0]["content"], "ping");
        }
        assert!(ping_body("perplexity", "model").is_none());
    }

    #[test]
    fn parses_anthropic_text_and_perplexity_citations() {
        let anthropic = parse_provider_reply(
            "anthropic",
            &json!({ "content": [{ "type": "text", "text": "Resposta" }] }),
        )
        .unwrap();
        assert_eq!(anthropic.content, "Resposta");

        let perplexity = parse_provider_reply(
            "perplexity",
            &json!({ "output": [{ "type": "message", "content": [{ "type": "output_text", "text": "Resposta", "annotations": [{ "type": "url_citation", "url": "https://example.com/source", "title": "Fonte" }] }] }] }),
        )
        .unwrap();
        assert_eq!(perplexity.content, "Resposta");
        assert_eq!(perplexity.citations[0].title, "Fonte");
    }

    #[tokio::test]
    async fn sends_adapter_urls_headers_and_bodies_to_mock_server() {
        use std::io::{Read, Write};
        for entry in serde_json::from_str::<Vec<CatalogEntry>>(include_str!(
            "../../resources/provider-catalog.json"
        ))
        .unwrap()
        {
            let provider = entry.id.as_str();
            if !supports_credentials(provider) || provider == "local" {
                continue;
            }
            let base_url = match provider {
                "azure-openai" => "https://resource.openai.azure.com/openai/v1".to_string(),
                _ => entry.base_url.clone(),
            };
            let endpoint = provider_endpoint(provider, &base_url).unwrap();
            let body = if provider == "perplexity" {
                json!({ "model": "test-model", "input": "ping" })
            } else {
                ping_body(provider, "test-model").unwrap()
            };
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let expected_path = endpoint.path().to_string();
            let server = std::thread::spawn(move || {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut chunk = [0_u8; 4096];
                loop {
                    let count = socket.read(&mut chunk).unwrap();
                    if count == 0 {
                        break;
                    }
                    bytes.extend_from_slice(&chunk[..count]);
                    if let Some(header_end) = bytes.windows(4).position(|part| part == b"\r\n\r\n")
                    {
                        let headers =
                            String::from_utf8_lossy(&bytes[..header_end]).to_ascii_lowercase();
                        let content_length = headers
                            .lines()
                            .find_map(|line| line.strip_prefix("content-length: "))
                            .and_then(|value| value.parse::<usize>().ok())
                            .unwrap_or(0);
                        if bytes.len() >= header_end + 4 + content_length {
                            break;
                        }
                    }
                }
                let request = String::from_utf8_lossy(&bytes).to_ascii_lowercase();
                socket
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}")
                    .unwrap();
                (request, expected_path)
            });
            let client = Client::new();
            client
                .post(format!("http://{address}{}", endpoint.path()))
                .pipe_auth(provider, "test-secret")
                .unwrap()
                .json(&body)
                .send()
                .await
                .unwrap();
            let (request, expected_path) = server.join().unwrap();
            let first_line = request.lines().next().unwrap_or_default();
            assert!(
                first_line.contains(&expected_path),
                "URL incorreta para {provider}: {first_line}"
            );
            let expected_header = match entry.auth_header.as_deref() {
                Some("x-api-key") => "x-api-key: test-secret",
                Some("api-key") => "api-key: test-secret",
                _ => "authorization: bearer test-secret",
            };
            assert!(
                request.contains(expected_header),
                "cabeçalho incorreto para {provider}"
            );
            assert!(
                request.contains("\"model\":\"test-model\""),
                "corpo incorreto para {provider}"
            );
            if provider == "anthropic" {
                assert!(!request.contains("authorization:"));
            }
            if provider == "azure-openai" {
                assert!(!request.contains("authorization:"));
            }
        }
    }

    #[test]
    fn extracts_only_text_from_stream_events() {
        assert_eq!(
            stream_delta(
                "anthropic",
                &json!({"type":"content_block_delta","delta":{"type":"text_delta","text":"Olá"}})
            ),
            Some("Olá")
        );
        assert_eq!(
            stream_delta(
                "anthropic",
                &json!({"type":"content_block_delta","delta":{"type":"input_json_delta","partial_json":"secret"}})
            ),
            None
        );
        assert_eq!(
            stream_delta(
                "openai",
                &json!({"choices":[{"delta":{"content":"Tudo bem"}}]})
            ),
            Some("Tudo bem")
        );
    }

    #[tokio::test]
    async fn mock_chat_completions_stream_emits_text_tool_call_usage_and_done() {
        use std::io::{Read, Write};

        let first_arguments = r#"{"q":"#;
        let second_arguments = r#""ok"}"#;
        let frames = [
            json!({ "choices": [{ "delta": { "content": "Texto" } }] }),
            json!({ "choices": [{ "delta": { "tool_calls": [{ "index": 0, "id": "call-1", "function": { "name": "lookup", "arguments": first_arguments } }] } }] }),
            json!({ "choices": [{ "delta": { "tool_calls": [{ "index": 0, "function": { "arguments": second_arguments } }] } }] }),
            json!({ "choices": [{ "delta": {}, "finish_reason": "tool_calls" }] }),
            json!({ "choices": [], "usage": { "prompt_tokens": 11, "completion_tokens": 5 } }),
        ];
        let body = frames
            .iter()
            .map(|frame| format!("data: {frame}\n\n"))
            .collect::<String>()
            + "data: [DONE]\n\n";
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server_body = body.clone();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            let mut chunk = [0u8; 1024];
            loop {
                let read = socket.read(&mut chunk).unwrap();
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..read]);
                if request.windows(4).any(|part| part == b"\r\n\r\n") {
                    break;
                }
            }
            write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                server_body.len(),
                server_body
            )
            .unwrap();
        });
        let response = provider_client(
            "local",
            &format!("http://127.0.0.1:{}/v1", address.port()),
            3,
        )
        .unwrap()
        .get(format!("http://{address}/stream"))
        .send()
        .await
        .unwrap();
        let mut events = Vec::new();
        let reply = consume_stream_response("openai", response, |event| events.push(event))
            .await
            .unwrap();
        server.join().unwrap();

        assert_eq!(reply.content, "Texto");
        assert_eq!(reply.tool_calls.len(), 1);
        assert_eq!(reply.tool_calls[0].args, json!({ "q": "ok" }));
        assert_eq!(
            reply.usage,
            Some(Usage {
                input: 11,
                output: 5
            })
        );
        assert!(events.contains(&ProviderEvent::TextDelta {
            text: "Texto".into()
        }));
        assert!(events.contains(&ProviderEvent::Usage {
            input: 11,
            output: 5
        }));
        assert!(events.iter().any(|event| matches!(
            event,
            ProviderEvent::ToolCallComplete { id, .. } if id == "call-1"
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            ProviderEvent::Done {
                stop_reason: Some(reason)
            } if reason == "tool_calls"
        )));
    }

    #[tokio::test]
    async fn cancellation_after_partial_stream_chunk_drops_the_remaining_response() {
        use std::io::{Read, Write};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            let mut buffer = [0u8; 1024];
            while !request.windows(4).any(|part| part == b"\r\n\r\n") {
                let count = socket.read(&mut buffer).unwrap();
                if count == 0 {
                    return;
                }
                request.extend_from_slice(&buffer[..count]);
            }
            let partial = format!(
                "data: {}\n\n",
                json!({ "choices": [{ "delta": {
                    "content": "received",
                    "tool_calls": [{ "index": 0, "id": "call-1", "function": {
                        "name": "lookup", "arguments": "{\\\"key\\\":"
                    }}]
                }}] })
            );
            let _ = write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{:X}\r\n{}\r\n",
                partial.len(), partial
            );
            let _ = socket.flush();
            std::thread::sleep(Duration::from_millis(500));
            let _ = socket.write_all(b"0\r\n\r\n");
        });

        let response = Client::new()
            .get(format!("http://{address}/stream"))
            .send()
            .await
            .unwrap();
        {
            let (text_seen, text_received) = tokio::sync::oneshot::channel();
            let mut text_seen = Some(text_seen);
            let consume = consume_stream_response("openai", response, move |event| {
                if matches!(event, ProviderEvent::TextDelta { .. }) {
                    if let Some(sender) = text_seen.take() {
                        let _ = sender.send(());
                    }
                }
            });
            tokio::pin!(consume);
            let cancellation = CancellationToken::new();
            let cancel_after_first_delta = cancellation.clone();
            tokio::select! {
                result = &mut consume => panic!("stream terminou antes do cancelamento: {result:?}"),
                _ = async move {
                    let _ = text_received.await;
                    cancel_after_first_delta.cancel();
                    cancellation.cancelled().await;
                } => {}
            }
        }
        server.join().unwrap();
    }
}
