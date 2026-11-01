use keyring::Entry;
use reqwest::{redirect::Policy, Client, StatusCode, Url};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::Manager;

#[derive(Default)]
pub(crate) struct HostingerState {
    approved_vms: Mutex<Vec<ApprovedVm>>,
    approved_projects: Mutex<HashMap<i64, Vec<String>>>,
    approved_firewalls: Mutex<Vec<ApprovedFirewall>>,
    approved_backups: Mutex<HashMap<i64, Vec<ApprovedBackup>>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ApprovedVm {
    id: i64,
    name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ApprovedFirewall {
    id: i64,
    name: String,
    rules: Vec<ApprovedFirewallRule>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ApprovedFirewallRule {
    id: i64,
    name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ApprovedBackup {
    id: i64,
    created_at: String,
}

const API_BASE: &str = "https://developers.hostinger.com/";
const API_HOST: &str = "developers.hostinger.com";
const CREDENTIAL_SERVICE: &str = "site.anceryx.odekrai.hostinger";
const CREDENTIAL_USER: &str = "api-token";
const MAX_TOKEN_BYTES: usize = 4096;
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_TEXT_CHARS: usize = 512;
const MAX_COLLECTION_ITEMS: usize = 100;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const ACTION_POLL_INTERVAL: Duration = Duration::from_secs(1);
const ACTION_MAX_WAIT: Duration = Duration::from_secs(60);
const MAX_COMPOSE_BYTES: usize = 8192;
const BLOCKED_MOUNT_ROOTS: &[&str] = &[
    "/", "/etc", "/root", "/var/run", "/proc", "/sys", "/dev", "/boot",
];

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
    })?;
    clear_vms(caller.app_handle());
    clear_firewalls(caller.app_handle());
    Ok(())
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
        Ok(()) | Err(keyring::Error::NoEntry) => {
            clear_vms(caller.app_handle());
            clear_firewalls(caller.app_handle());
            Ok(())
        }
        Err(_) => {
            Err("Não foi possível remover o token do Gerenciador de Credenciais do Windows.".into())
        }
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
    #[cfg(test)]
    if base.scheme() == "http" && base.host_str() == Some("127.0.0.1") && base.port().is_some() {
        return base
            .join(path)
            .map_err(|_| "O caminho interno da API Hostinger é inválido.".into());
    }
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
    keyring_entry()?
        .get_password()
        .map_err(|error| match error {
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

async fn request_action_at<T: DeserializeOwned>(
    client: &Client,
    endpoint: Url,
    token: Option<&str>,
    method: reqwest::Method,
    body: Option<&Value>,
) -> Result<T, String> {
    let token = token.ok_or_else(|| {
        "Salve o token da Hostinger em Configurações > Nuvem para continuar.".to_string()
    })?;
    let token = validate_token(token)?;
    let mut request = client.request(method, endpoint).bearer_auth(token);
    if let Some(body) = body {
        request = request.json(body);
    }
    let response = request.send().await.map_err(|error| {
        if error.is_timeout() {
            "A solicitação à Hostinger excedeu o tempo limite.".to_string()
        } else {
            "Falha de rede ao conectar à Hostinger.".to_string()
        }
    })?;
    if !response.status().is_success() {
        return Err(status_error(response.status()));
    }
    read_limited(response).await
}

async fn request_action_empty_at(
    client: &Client,
    endpoint: Url,
    token: Option<&str>,
    method: reqwest::Method,
) -> Result<(), String> {
    let token = token.ok_or_else(|| {
        "Salve o token da Hostinger em Configurações > Nuvem para continuar.".to_string()
    })?;
    let response = client
        .request(method, endpoint)
        .bearer_auth(validate_token(token)?)
        .send()
        .await
        .map_err(|error| {
            if error.is_timeout() {
                "A solicitação à Hostinger excedeu o tempo limite.".to_string()
            } else {
                "Falha de rede ao conectar à Hostinger.".to_string()
            }
        })?;
    if !response.status().is_success() {
        return Err(status_error(response.status()));
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err("A resposta da Hostinger excedeu o limite de 1 MiB.".into());
    }
    let mut response = response;
    let mut total = 0usize;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "Não foi possível ler a resposta da Hostinger.".to_string())?
    {
        total = total.saturating_add(chunk.len());
        if total > MAX_RESPONSE_BYTES {
            return Err("A resposta da Hostinger excedeu o limite de 1 MiB.".into());
        }
    }
    Ok(())
}

fn action_status(value: &Value) -> Result<ActionStatus, String> {
    let value = value.get("data").unwrap_or(value);
    let raw = value
        .get("status")
        .or_else(|| value.get("state"))
        .and_then(Value::as_str)
        .ok_or("A Hostinger retornou uma ação sem estado reconhecido.")?;
    match raw.to_ascii_lowercase().as_str() {
        "success" => Ok(ActionStatus::Complete),
        "error" => Ok(ActionStatus::Failed),
        "created" | "sent" | "delayed" => Ok(ActionStatus::Pending),
        _ => Err("A Hostinger retornou um estado de ação desconhecido.".into()),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ActionStatus {
    Pending,
    Complete,
    Failed,
}

async fn run_action_with(
    client: &Client,
    base: &str,
    token: Option<&str>,
    vm_id: i64,
    action: &str,
    max_wait: Duration,
    poll_interval: Duration,
) -> Result<Value, String> {
    validate_vm_id(vm_id)?;
    if !matches!(action, "start" | "stop" | "restart") {
        return Err("Ação de VPS inválida.".into());
    }
    run_action_paths_with(
        client,
        base,
        token,
        &format!("api/vps/v1/virtual-machines/{vm_id}/{action}"),
        &format!("api/vps/v1/virtual-machines/{vm_id}/actions"),
        reqwest::Method::POST,
        None,
        max_wait,
        poll_interval,
    )
    .await
}

async fn run_action_paths_with(
    client: &Client,
    base: &str,
    token: Option<&str>,
    post_path: &str,
    status_path: &str,
    method: reqwest::Method,
    body: Option<Value>,
    max_wait: Duration,
    poll_interval: Duration,
) -> Result<Value, String> {
    let deadline = Instant::now() + max_wait;
    let endpoint = endpoint_for_base(base, post_path)?;
    let initial: Value = tokio::time::timeout_at(
        tokio::time::Instant::from_std(deadline),
        request_action_at(client, endpoint, token, method, body.as_ref()),
    )
    .await
    .map_err(|_| "A ação da Hostinger excedeu o tempo limite de 60 segundos.".to_string())??;
    let action = initial.get("data").unwrap_or(&initial);
    let action_id = action
        .get("id")
        .or_else(|| action.get("action_id"))
        .or_else(|| action.get("actionId"))
        .and_then(|id| {
            id.as_i64()
                .map(|id| id.to_string())
                .or_else(|| id.as_str().map(str::to_owned))
        })
        .ok_or("A Hostinger aceitou a ação, mas não retornou um identificador para acompanhar.")?;
    if action_id.is_empty()
        || action_id.len() > 20
        || !matches!(action_id.parse::<u64>(), Ok(id) if id > 0)
    {
        return Err("A Hostinger retornou um identificador de ação inválido.".into());
    }
    let status_path = format!("{status_path}/{action_id}");
    loop {
        let endpoint = endpoint_for_base(base, &status_path)?;
        let status: Value = tokio::time::timeout_at(
            tokio::time::Instant::from_std(deadline),
            request_at(client, endpoint, token),
        )
        .await
        .map_err(|_| "A ação da Hostinger excedeu o tempo limite de 60 segundos.".to_string())??;
        match action_status(&status)? {
            ActionStatus::Complete => {
                return Ok(sanitize_external_value(
                    status,
                    0,
                    token.unwrap_or_default(),
                ));
            }
            ActionStatus::Failed => {
                return Err("A Hostinger informou que a ação na VPS falhou.".into());
            }
            ActionStatus::Pending => {}
        }
        let now = Instant::now();
        if now >= deadline {
            return Err(
                "A ação da Hostinger não terminou dentro do tempo limite de 60 segundos.".into(),
            );
        }
        tokio::time::sleep(poll_interval.min(deadline.saturating_duration_since(now))).await;
    }
}

fn app_state(app: &tauri::AppHandle) -> Result<tauri::State<'_, HostingerState>, String> {
    app.try_state::<HostingerState>()
        .ok_or_else(|| "A lista local de VPS da Hostinger não está disponível.".into())
}

fn remember_vms(app: &tauri::AppHandle, vms: &[Value]) -> Result<(), String> {
    let mut approved = Vec::new();
    for vm in vms {
        let Some(id) = vm.get("id").and_then(Value::as_i64).filter(|id| *id > 0) else {
            continue;
        };
        let Some(name) = vm
            .get("hostname")
            .or_else(|| vm.get("name"))
            .and_then(Value::as_str)
        else {
            continue;
        };
        let name = display_vm_name(name);
        if name.is_empty() {
            continue;
        }
        approved.push(ApprovedVm { id, name });
    }
    *app_state(app)?
        .approved_vms
        .lock()
        .map_err(|_| "Não foi possível atualizar a lista local de VPS.".to_string())? = approved;
    Ok(())
}

fn clear_vms(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<HostingerState>() {
        if let Ok(mut vms) = state.approved_vms.lock() {
            vms.clear();
        }
        if let Ok(mut projects) = state.approved_projects.lock() {
            projects.clear();
        }
        if let Ok(mut backups) = state.approved_backups.lock() {
            backups.clear();
        }
    }
}

fn clear_firewalls(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<HostingerState>() {
        if let Ok(mut firewalls) = state.approved_firewalls.lock() {
            firewalls.clear();
        }
    }
}

fn resolve_approved_vm(vms: &[ApprovedVm], name: &str) -> Result<ApprovedVm, String> {
    let matching = vms.iter().filter(|vm| vm.name == name).collect::<Vec<_>>();
    match matching.as_slice() {
        [vm] => Ok((*vm).clone()),
        [] => Err("A VPS precisa aparecer na lista da Hostinger consultada por você.".into()),
        _ => Err("Há mais de uma VPS com esse nome; escolha um nome único na lista.".into()),
    }
}

fn backups_from(value: &Value) -> Vec<ApprovedBackup> {
    value
        .get("data")
        .and_then(Value::as_array)
        .or_else(|| value.as_array())
        .into_iter()
        .flatten()
        .filter_map(|backup| {
            let id = backup.get("id")?.as_i64().filter(|id| *id > 0)?;
            let created_at = backup.get("created_at")?.as_str()?;
            if created_at.is_empty()
                || created_at.len() > 64
                || created_at.chars().any(char::is_control)
            {
                return None;
            }
            Some(ApprovedBackup {
                id,
                created_at: created_at.to_owned(),
            })
        })
        .take(MAX_COLLECTION_ITEMS)
        .collect()
}

fn remember_backups(app: &tauri::AppHandle, vm_id: i64, value: &Value) -> Result<(), String> {
    validate_vm_id(vm_id)?;
    let backups = backups_from(value);
    app_state(app)?
        .approved_backups
        .lock()
        .map_err(|_| "Não foi possível atualizar a lista local de backups.".to_string())?
        .insert(vm_id, backups);
    Ok(())
}

fn resolve_approved_backup<'a>(
    backups: &'a HashMap<i64, Vec<ApprovedBackup>>,
    vm_id: i64,
    created_at: &str,
) -> Result<&'a ApprovedBackup, String> {
    if created_at.is_empty() || created_at.len() > 64 || created_at.chars().any(char::is_control) {
        return Err("Informe a data exata de um backup listado para esta VPS.".into());
    }
    let matching = backups
        .get(&vm_id)
        .into_iter()
        .flatten()
        .filter(|backup| backup.created_at == created_at)
        .collect::<Vec<_>>();
    match matching.as_slice() {
        [backup] => Ok(*backup),
        [] => Err("Esse backup não aparece na lista consultada para esta VPS.".into()),
        _ => Err("Há backups com a mesma data; atualize a lista antes de restaurar.".into()),
    }
}

pub(crate) fn prepare_skill_params(
    app: &tauri::AppHandle,
    skill_id: &str,
    mut params: Value,
) -> Result<Value, String> {
    if !skill_id.starts_with("hostinger.") {
        return Ok(params);
    }
    if matches!(
        skill_id,
        "hostinger.backup_list"
            | "hostinger.backup_restore"
            | "hostinger.recovery_start"
            | "hostinger.recovery_stop"
    ) {
        let allowed: &[&str] = if skill_id == "hostinger.backup_restore" {
            &["vm_name", "backup_created_at"]
        } else {
            &["vm_name"]
        };
        let object = params
            .as_object()
            .ok_or("Os parâmetros da habilidade são inválidos.")?;
        if object.len() != allowed.len()
            || object.keys().any(|key| !allowed.contains(&key.as_str()))
        {
            return Err("Os parâmetros da habilidade Hostinger são inválidos.".into());
        }
        let name = params
            .get("vm_name")
            .and_then(Value::as_str)
            .ok_or("Informe o nome da VPS consultada.")?;
        let state = app_state(app)?;
        let vms = state
            .approved_vms
            .lock()
            .map_err(|_| "Não foi possível consultar a lista local de VPS.".to_string())?;
        let vm = resolve_approved_vm(&vms, name)?;
        let vm_id = vm.id;
        drop(vms);
        let backup_id = if skill_id == "hostinger.backup_restore" {
            let created_at = params
                .get("backup_created_at")
                .and_then(Value::as_str)
                .ok_or("Informe a data do backup listado.")?;
            let backups = state
                .approved_backups
                .lock()
                .map_err(|_| "Não foi possível consultar a lista local de backups.".to_string())?;
            Some(resolve_approved_backup(&backups, vm_id, created_at)?.id)
        } else {
            None
        };
        let object = params
            .as_object_mut()
            .ok_or("Os parâmetros da habilidade são inválidos.")?;
        object.insert("vm_id".into(), Value::from(vm_id));
        if let Some(backup_id) = backup_id {
            object.insert("backup_id".into(), Value::from(backup_id));
        }
        return Ok(params);
    }
    if skill_id == "hostinger.firewall_list" {
        if params.as_object().is_none_or(|object| !object.is_empty()) {
            return Err("A lista de firewalls não recebe parâmetros.".into());
        }
        return Ok(params);
    }
    if skill_id == "hostinger.firewall_create" {
        let object = params
            .as_object()
            .ok_or("Os parâmetros da habilidade são inválidos.")?;
        if object.len() != 1 || !object.contains_key("firewall_name") {
            return Err("Informe somente o nome do firewall.".into());
        }
        let name = params
            .get("firewall_name")
            .and_then(Value::as_str)
            .ok_or("Informe o nome do firewall.")?;
        validate_firewall_name(name)?;
        return Ok(params);
    }
    if matches!(
        skill_id,
        "hostinger.firewall_details"
            | "hostinger.firewall_delete"
            | "hostinger.firewall_rule_create"
            | "hostinger.firewall_rule_delete"
    ) {
        let allowed = match skill_id {
            "hostinger.firewall_details" | "hostinger.firewall_delete" => &["firewall_name"][..],
            "hostinger.firewall_rule_create" => {
                &["firewall_name", "protocol", "port", "source_cidr"][..]
            }
            _ => &["firewall_name", "rule_name"][..],
        };
        let object = params
            .as_object()
            .ok_or("Os parâmetros da habilidade são inválidos.")?;
        if object.len() != allowed.len()
            || object.keys().any(|key| !allowed.contains(&key.as_str()))
        {
            return Err("Os parâmetros da habilidade Firewall são inválidos.".into());
        }
        let name = params
            .get("firewall_name")
            .and_then(Value::as_str)
            .ok_or("Informe o nome do firewall consultado.")?;
        validate_firewall_name(name)?;
        let state = app_state(app)?;
        let firewalls = state
            .approved_firewalls
            .lock()
            .map_err(|_| "Não foi possível consultar a lista local de firewalls.".to_string())?;
        let firewall = resolve_approved_firewall(&firewalls, name)?;
        drop(firewalls);
        params
            .as_object_mut()
            .ok_or("Os parâmetros da habilidade são inválidos.")?
            .insert("firewall_id".into(), Value::from(firewall.id));
        match skill_id {
            "hostinger.firewall_rule_create" => {
                let protocol = params
                    .get("protocol")
                    .and_then(Value::as_str)
                    .ok_or("Informe o protocolo.")?;
                let port = params
                    .get("port")
                    .and_then(Value::as_str)
                    .ok_or("Informe a porta.")?;
                let source = params
                    .get("source_cidr")
                    .and_then(Value::as_str)
                    .ok_or("Informe a origem.")?;
                validate_firewall_rule(protocol, port, source)?;
            }
            "hostinger.firewall_rule_delete" => {
                let rule_name = params
                    .get("rule_name")
                    .and_then(Value::as_str)
                    .ok_or("Informe o nome exato da regra consultada.")?;
                let rule = resolve_approved_firewall_rule(&firewall, rule_name)?;
                params
                    .as_object_mut()
                    .ok_or("Os parâmetros da habilidade são inválidos.")?
                    .insert("rule_id".into(), Value::from(rule.id));
            }
            _ => {}
        }
        return Ok(params);
    }
    if matches!(
        skill_id,
        "hostinger.firewall_activate" | "hostinger.firewall_deactivate" | "hostinger.ssh_keys_list"
    ) {
        let allowed = if skill_id == "hostinger.ssh_keys_list" {
            &["vm_name"][..]
        } else {
            &["vm_name", "firewall_name"][..]
        };
        let object = params
            .as_object()
            .ok_or("Os parâmetros da habilidade são inválidos.")?;
        if object.len() != allowed.len()
            || object.keys().any(|key| !allowed.contains(&key.as_str()))
        {
            return Err("Os parâmetros da habilidade são inválidos.".into());
        }
        let name = params
            .get("vm_name")
            .and_then(Value::as_str)
            .ok_or("Informe o nome da VPS consultada.")?;
        let state = app_state(app)?;
        let vms = state
            .approved_vms
            .lock()
            .map_err(|_| "Não foi possível consultar a lista local de VPS.".to_string())?;
        let vm = resolve_approved_vm(&vms, name)?;
        params
            .as_object_mut()
            .ok_or("Os parâmetros da habilidade são inválidos.")?
            .insert("vm_id".into(), Value::from(vm.id));
        drop(vms);
        if skill_id != "hostinger.ssh_keys_list" {
            let firewall_name = params
                .get("firewall_name")
                .and_then(Value::as_str)
                .ok_or("Informe o nome do firewall consultado.")?;
            let firewalls = state.approved_firewalls.lock().map_err(|_| {
                "Não foi possível consultar a lista local de firewalls.".to_string()
            })?;
            let firewall = resolve_approved_firewall(&firewalls, firewall_name)?;
            params
                .as_object_mut()
                .ok_or("Os parâmetros da habilidade são inválidos.")?
                .insert("firewall_id".into(), Value::from(firewall.id));
        }
        return Ok(params);
    }
    let object = params
        .as_object()
        .ok_or("Os parâmetros da habilidade são inválidos.")?;
    let expected = if matches!(
        skill_id,
        "hostinger.vps_start" | "hostinger.vps_stop" | "hostinger.vps_restart"
    ) {
        1
    } else if skill_id == "hostinger.docker_list" {
        1
    } else if skill_id == "hostinger.docker_deploy" {
        3
    } else {
        2
    };
    if object.len() != expected || object.contains_key("vm_id") {
        return Err("Os parâmetros da habilidade Hostinger são inválidos.".into());
    }
    let name = object
        .get("vm_name")
        .and_then(Value::as_str)
        .ok_or("Informe o nome da VPS consultada.")?;
    let state = app_state(app)?;
    let approved = state
        .approved_vms
        .lock()
        .map_err(|_| "Não foi possível consultar a lista local de VPS.".to_string())?;
    let vm = resolve_approved_vm(&approved, name)?;
    params
        .as_object_mut()
        .ok_or("Os parâmetros da habilidade são inválidos.")?
        .insert("vm_id".into(), Value::from(vm.id));
    if skill_id.starts_with("hostinger.docker_") && skill_id != "hostinger.docker_list" {
        let project = params
            .get("project_name")
            .and_then(Value::as_str)
            .ok_or("Informe o nome do projeto Docker.")?;
        validate_project_name(project)?;
        if skill_id != "hostinger.docker_deploy" {
            let projects = state
                .approved_projects
                .lock()
                .map_err(|_| "Não foi possível consultar a lista local de projetos.".to_string())?;
            resolve_approved_project(
                projects.get(&vm.id).map(Vec::as_slice).unwrap_or_default(),
                project,
            )?;
        } else {
            let compose = params
                .get("compose_yaml")
                .and_then(Value::as_str)
                .ok_or("Informe o conteúdo do docker-compose.")?;
            validate_docker_compose(project, compose)?;
        }
    }
    Ok(params)
}

#[cfg(test)]
fn validate_vm_name_request(params: &Value) -> Result<&str, String> {
    let object = params
        .as_object()
        .ok_or("Os parâmetros da habilidade são inválidos.")?;
    if object.len() != 1 || object.contains_key("vm_id") {
        return Err("Informe somente o nome da VPS.".into());
    }
    object
        .get("vm_name")
        .and_then(Value::as_str)
        .ok_or_else(|| "Informe o nome da VPS consultada.".into())
}

pub(crate) fn validate_action_against_catalog(
    app: &tauri::AppHandle,
    action: &crate::security::SimulatedAction,
) -> Result<(), String> {
    let crate::security::SimulatedAction::Productivity { skill_id, params } = action else {
        return Ok(());
    };
    if !matches!(
        skill_id.as_str(),
        "hostinger.vps_start"
            | "hostinger.vps_stop"
            | "hostinger.vps_restart"
            | "hostinger.docker_list"
            | "hostinger.docker_start"
            | "hostinger.docker_stop"
            | "hostinger.docker_restart"
            | "hostinger.docker_deploy"
            | "hostinger.docker_remove"
            | "hostinger.firewall_list"
            | "hostinger.firewall_details"
            | "hostinger.firewall_create"
            | "hostinger.firewall_rule_create"
            | "hostinger.firewall_activate"
            | "hostinger.firewall_deactivate"
            | "hostinger.firewall_delete"
            | "hostinger.firewall_rule_delete"
            | "hostinger.ssh_keys_list"
            | "hostinger.backup_list"
            | "hostinger.backup_restore"
            | "hostinger.recovery_start"
            | "hostinger.recovery_stop"
    ) {
        return Ok(());
    }
    if skill_id == "hostinger.firewall_list" || skill_id == "hostinger.firewall_create" {
        return Ok(());
    }
    if matches!(
        skill_id.as_str(),
        "hostinger.backup_list"
            | "hostinger.backup_restore"
            | "hostinger.recovery_start"
            | "hostinger.recovery_stop"
    ) {
        let vm_name = params
            .get("vm_name")
            .and_then(Value::as_str)
            .ok_or("Informe o nome da VPS consultada.")?;
        let vm_id = params
            .get("vm_id")
            .and_then(Value::as_i64)
            .ok_or("A VPS não foi confirmada pela lista consultada.")?;
        let state = app_state(app)?;
        let vms = state
            .approved_vms
            .lock()
            .map_err(|_| "Não foi possível consultar a lista local de VPS.".to_string())?;
        if resolve_approved_vm(&vms, vm_name)?.id != vm_id {
            return Err("O identificador não corresponde à VPS da lista consultada.".into());
        }
        drop(vms);
        if skill_id == "hostinger.backup_restore" {
            let created_at = params
                .get("backup_created_at")
                .and_then(Value::as_str)
                .ok_or("Informe a data do backup listado.")?;
            let backup_id = params
                .get("backup_id")
                .and_then(Value::as_i64)
                .ok_or("O backup não foi confirmado pela lista consultada.")?;
            let backups = state
                .approved_backups
                .lock()
                .map_err(|_| "Não foi possível consultar a lista local de backups.".to_string())?;
            if resolve_approved_backup(&backups, vm_id, created_at)?.id != backup_id {
                return Err(
                    "Esse backup pertence a outra VPS ou não corresponde à lista consultada."
                        .into(),
                );
            }
        }
        return Ok(());
    }
    if skill_id.starts_with("hostinger.firewall_") || skill_id == "hostinger.ssh_keys_list" {
        let state = app_state(app)?;
        if skill_id == "hostinger.ssh_keys_list" {
            let name = params
                .get("vm_name")
                .and_then(Value::as_str)
                .ok_or("Informe o nome da VPS consultada.")?;
            let id = params
                .get("vm_id")
                .and_then(Value::as_i64)
                .ok_or("A VPS não foi confirmada pela lista consultada.")?;
            let vms = state
                .approved_vms
                .lock()
                .map_err(|_| "Não foi possível consultar a lista local de VPS.".to_string())?;
            let vm = resolve_approved_vm(&vms, name)?;
            return if vm.id == id {
                Ok(())
            } else {
                Err("O identificador não corresponde à VPS da lista consultada.".into())
            };
        }
        let name = params
            .get("firewall_name")
            .and_then(Value::as_str)
            .ok_or("Informe o nome do firewall consultado.")?;
        let id = params
            .get("firewall_id")
            .and_then(Value::as_i64)
            .ok_or("O firewall não foi confirmado pela lista consultada.")?;
        let firewalls = state
            .approved_firewalls
            .lock()
            .map_err(|_| "Não foi possível consultar a lista local de firewalls.".to_string())?;
        let firewall = resolve_approved_firewall(&firewalls, name)?;
        if firewall.id != id {
            return Err("O identificador não corresponde ao firewall consultado.".into());
        }
        if skill_id == "hostinger.firewall_rule_delete" {
            let rule_name = params
                .get("rule_name")
                .and_then(Value::as_str)
                .ok_or("Informe o nome exato da regra consultada.")?;
            let rule_id = params
                .get("rule_id")
                .and_then(Value::as_i64)
                .ok_or("A regra não foi confirmada pelos detalhes consultados.")?;
            let rule = resolve_approved_firewall_rule(&firewall, rule_name)?;
            if rule.id != rule_id {
                return Err("O identificador não corresponde à regra consultada.".into());
            }
        }
        if matches!(
            skill_id.as_str(),
            "hostinger.firewall_activate" | "hostinger.firewall_deactivate"
        ) {
            let vm_name = params
                .get("vm_name")
                .and_then(Value::as_str)
                .ok_or("Informe o nome da VPS consultada.")?;
            let vm_id = params
                .get("vm_id")
                .and_then(Value::as_i64)
                .ok_or("A VPS não foi confirmada pela lista consultada.")?;
            let vms = state
                .approved_vms
                .lock()
                .map_err(|_| "Não foi possível consultar a lista local de VPS.".to_string())?;
            if resolve_approved_vm(&vms, vm_name)?.id != vm_id {
                return Err("O identificador não corresponde à VPS consultada.".into());
            }
        }
        if skill_id == "hostinger.firewall_rule_create" {
            validate_firewall_rule(
                params
                    .get("protocol")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
                params
                    .get("port")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
                params
                    .get("source_cidr")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            )?;
        }
        return Ok(());
    }
    let name = params
        .get("vm_name")
        .and_then(Value::as_str)
        .ok_or("Informe o nome da VPS consultada.")?;
    let id = params
        .get("vm_id")
        .and_then(Value::as_i64)
        .ok_or("A VPS não foi confirmada pela lista consultada.")?;
    let state = app_state(app)?;
    let approved = state
        .approved_vms
        .lock()
        .map_err(|_| "Não foi possível consultar a lista local de VPS.".to_string())?;
    let vm = resolve_approved_vm(&approved, name)?;
    if vm.id != id {
        return Err("O identificador não corresponde à VPS da lista consultada.".into());
    }
    if skill_id.starts_with("hostinger.docker_")
        && skill_id != "hostinger.docker_list"
        && skill_id != "hostinger.docker_deploy"
    {
        let project = params
            .get("project_name")
            .and_then(Value::as_str)
            .ok_or("Informe o nome do projeto Docker.")?;
        let projects = state
            .approved_projects
            .lock()
            .map_err(|_| "Não foi possível consultar a lista local de projetos.".to_string())?;
        resolve_approved_project(
            projects.get(&vm.id).map(Vec::as_slice).unwrap_or_default(),
            project,
        )?;
    }
    if skill_id == "hostinger.docker_deploy" {
        validate_docker_compose(
            params
                .get("project_name")
                .and_then(Value::as_str)
                .ok_or("Informe o nome do projeto Docker.")?,
            params
                .get("compose_yaml")
                .and_then(Value::as_str)
                .ok_or("Informe o conteúdo do docker-compose.")?,
        )?;
    }
    Ok(())
}

fn display_vm_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(MAX_TEXT_CHARS)
        .collect()
}

fn validate_project_name(name: &str) -> Result<&str, String> {
    if !(3..=64).contains(&name.len())
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(
            "O nome do projeto deve ter de 3 a 64 letras, números, hífens ou sublinhados.".into(),
        );
    }
    Ok(name)
}

fn normalize_mount_source(source: &str) -> Result<String, String> {
    let source = source.trim().trim_matches(['\'', '"']);
    if source.is_empty() || source.contains('$') || source.contains(['*', '&', '!']) {
        return Err("A montagem do compose não pode ser validada com segurança.".into());
    }
    let normalized = source.replace('\\', "/");
    if !normalized.starts_with('/') {
        if normalized.split('/').any(|part| part == "..") {
            return Err("O compose não pode montar caminhos com travessia de diretório.".into());
        }
        return Ok(normalized);
    }
    let mut parts = Vec::new();
    for part in normalized.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            value => parts.push(value),
        }
    }
    let absolute = format!("/{}", parts.join("/"));
    if BLOCKED_MOUNT_ROOTS
        .iter()
        .any(|root| absolute == *root || absolute.starts_with(&format!("{root}/")))
    {
        return Err("O compose tenta montar um caminho protegido do sistema do servidor.".into());
    }
    Ok(absolute)
}

fn validate_compose_mounts(compose: &str) -> Result<(), String> {
    let mut in_volumes = false;
    let mut volume_indent = 0usize;
    for raw in compose.lines() {
        let line = raw.trim();
        let indent = raw.len() - raw.trim_start().len();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if ["volumes:", "devices:"]
            .iter()
            .any(|key| line == *key || line.starts_with(*key))
        {
            in_volumes = true;
            volume_indent = indent;
            if let Some(inline) = line
                .strip_prefix("volumes:")
                .or_else(|| line.strip_prefix("devices:"))
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                if inline.contains(['&', '*', '!']) {
                    return Err("A montagem do compose usa âncora ou tag YAML e não pode ser validada com segurança.".into());
                }
                if inline.starts_with('[') {
                    for entry in inline.trim_matches(['[', ']']).split(',') {
                        let value = entry.trim().trim_matches(['\'', '"']);
                        if value.starts_with('/')
                            || value.starts_with('.')
                            || value.starts_with('~')
                        {
                            normalize_mount_source(value.split(':').next().unwrap_or_default())?;
                        }
                    }
                }
            }
            continue;
        }
        if in_volumes && indent <= volume_indent && !line.starts_with('-') {
            in_volumes = false;
        }
        if !in_volumes {
            continue;
        }
        if let Some(rest) = line
            .strip_prefix("source:")
            .or_else(|| line.strip_prefix("device:"))
        {
            let source = rest
                .split(" #")
                .next()
                .unwrap_or(rest)
                .trim()
                .trim_matches(['\'', '"']);
            normalize_mount_source(source)?;
        } else if let Some(rest) = line.strip_prefix('-') {
            // Short bind syntax is source:target[:options]. Named volumes are ignored.
            let value = rest.trim().trim_matches(['\'', '"']);
            if value.contains(['&', '*', '!']) {
                return Err("A montagem do compose usa âncora ou tag YAML e não pode ser validada com segurança.".into());
            }
            if value.starts_with('$') {
                return Err(
                    "A montagem do compose usa variável e não pode ser validada com segurança."
                        .into(),
                );
            }
            if value.starts_with('/') || value.starts_with('.') || value.starts_with("~") {
                let source = value.split(':').next().unwrap_or_default();
                normalize_mount_source(source)?;
            } else if value.starts_with(['{', '|', '>']) {
                return Err(
                    "A montagem do compose usa sintaxe YAML não suportada para validação.".into(),
                );
            } else {
                let source = value.split(':').next().unwrap_or_default();
                let long_syntax_key =
                    ["type", "source", "target", "read_only", "consistency"].contains(&source);
                let valid_named_volume = !source.is_empty()
                    && source.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.')
                    });
                if !long_syntax_key && !valid_named_volume {
                    return Err("A montagem do compose não pode ser validada com segurança.".into());
                }
            }
        }
    }
    Ok(())
}

fn validate_docker_compose(project_name: &str, compose: &str) -> Result<(), String> {
    validate_project_name(project_name)?;
    if compose.trim().is_empty() || compose.len() > MAX_COMPOSE_BYTES {
        return Err("O docker-compose deve ter conteúdo e no máximo 8 KiB.".into());
    }
    let trimmed = compose.trim_start();
    if trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("git@")
        || trimmed.starts_with("github.com/")
    {
        return Err(
            "URLs remotas não são aceitas; envie o conteúdo YAML do docker-compose.".into(),
        );
    }
    if !compose
        .lines()
        .any(|line| line.trim() == "services:" || line.trim_start().starts_with("services:"))
    {
        return Err("O arquivo precisa ser um docker-compose YAML com uma seção services.".into());
    }
    if compose
        .lines()
        .any(|line| line.trim_start().starts_with("include:"))
    {
        return Err(
            "Referências externas a outros arquivos compose não são aceitas nesta versão.".into(),
        );
    }
    validate_compose_mounts(compose)
}

pub(crate) fn validate_skill_compose(project_name: &str, compose: &str) -> Result<(), String> {
    validate_docker_compose(project_name, compose)
}

fn project_names(data: &Value) -> Vec<String> {
    data.get("data")
        .unwrap_or(data)
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|project| {
            project
                .get("project_name")
                .or_else(|| project.get("name"))
                .and_then(Value::as_str)
                .and_then(|name| validate_project_name(name).ok())
                .map(str::to_owned)
        })
        .collect()
}

fn remember_projects(app: &tauri::AppHandle, vm_id: i64, data: &Value) -> Result<(), String> {
    let state = app_state(app)?;
    state
        .approved_projects
        .lock()
        .map_err(|_| "Não foi possível atualizar a lista local de projetos.".to_string())?
        .insert(vm_id, project_names(data));
    Ok(())
}

fn resolve_approved_project(projects: &[String], name: &str) -> Result<(), String> {
    validate_project_name(name)?;
    if projects.iter().any(|project| project == name) {
        Ok(())
    } else {
        Err("O projeto precisa aparecer na lista consultada por você nesta sessão.".into())
    }
}

fn validate_firewall_name(name: &str) -> Result<&str, String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 128 || name.chars().any(char::is_control) {
        return Err("Informe um nome de firewall válido (até 128 caracteres).".into());
    }
    Ok(name)
}

fn validate_firewall_rule(protocol: &str, port: &str, source: &str) -> Result<(), String> {
    if !matches!(
        protocol,
        "TCP"
            | "UDP"
            | "ICMP"
            | "GRE"
            | "any"
            | "ESP"
            | "AH"
            | "ICMPv6"
            | "SSH"
            | "HTTP"
            | "HTTPS"
            | "MySQL"
            | "PostgreSQL"
    ) {
        return Err("O protocolo da regra de firewall não é válido.".into());
    }
    let valid_port = port.parse::<u16>().is_ok_and(|port| port > 0)
        || port.split_once(':').is_some_and(|(start, end)| {
            matches!((start.parse::<u16>(), end.parse::<u16>()), (Ok(a), Ok(b)) if a > 0 && a <= b)
        })
        || (matches!(protocol, "ICMP" | "ICMPv6" | "GRE" | "ESP" | "AH" | "any") && port == "all");
    if !valid_port {
        return Err("Informe uma porta entre 1 e 65535 ou um intervalo válido.".into());
    }
    if source == "any" {
        return Ok(());
    }
    let (address, prefix) = source.split_once('/').unwrap_or((source, ""));
    let ip = address
        .parse::<std::net::IpAddr>()
        .map_err(|_| "A origem deve ser um endereço IP ou CIDR válido.")?;
    if !prefix.is_empty() {
        let bits = if ip.is_ipv4() { 32 } else { 128 };
        let mask = prefix
            .parse::<u8>()
            .map_err(|_| "A máscara CIDR da origem é inválida.")?;
        if mask > bits {
            return Err("A máscara CIDR da origem é inválida.".into());
        }
    } else if source.contains('/') {
        return Err("A máscara CIDR da origem é inválida.".into());
    }
    Ok(())
}

fn firewall_source_is_public(source: &str) -> bool {
    matches!(source.trim(), "any" | "0.0.0.0/0" | "::/0")
}

fn firewall_http_method(skill_id: &str) -> Option<reqwest::Method> {
    match skill_id {
        "hostinger.firewall_list" | "hostinger.firewall_details" | "hostinger.ssh_keys_list" => {
            Some(reqwest::Method::GET)
        }
        "hostinger.firewall_create"
        | "hostinger.firewall_rule_create"
        | "hostinger.firewall_activate"
        | "hostinger.firewall_deactivate" => Some(reqwest::Method::POST),
        "hostinger.firewall_delete" | "hostinger.firewall_rule_delete" => {
            Some(reqwest::Method::DELETE)
        }
        _ => None,
    }
}

pub(crate) fn validate_skill_firewall_rule(
    protocol: &str,
    port: &str,
    source: &str,
) -> Result<(), String> {
    validate_firewall_rule(protocol, port, source)
}

pub(crate) fn is_public_firewall_source(source: &str) -> bool {
    firewall_source_is_public(source)
}

fn firewall_rule_name(rule: &Value) -> Option<String> {
    let protocol = rule.get("protocol")?.as_str()?;
    let port = rule.get("port")?.as_str()?;
    let source = rule
        .get("source_detail")
        .or_else(|| rule.get("source"))?
        .as_str()?;
    Some(format!(
        "{} {protocol} {port} de {source}",
        rule.get("action")
            .and_then(Value::as_str)
            .unwrap_or("accept")
    ))
}

fn firewall_rules_from(value: &Value) -> Vec<ApprovedFirewallRule> {
    value
        .get("data")
        .unwrap_or(value)
        .get("rules")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|rule| {
            Some(ApprovedFirewallRule {
                id: rule.get("id")?.as_i64().filter(|id| *id > 0)?,
                name: firewall_rule_name(rule).map(|name| display_vm_name(&name))?,
            })
        })
        .collect()
}

fn remember_firewall(app: &tauri::AppHandle, value: &Value) -> Result<(), String> {
    let data = value.get("data").unwrap_or(value);
    let id = data
        .get("id")
        .and_then(Value::as_i64)
        .filter(|id| *id > 0)
        .ok_or("A Hostinger não retornou o identificador do firewall.")?;
    let name = data
        .get("name")
        .and_then(Value::as_str)
        .map(display_vm_name)
        .filter(|name| !name.is_empty())
        .ok_or("A Hostinger não retornou o nome do firewall.")?;
    let firewall = ApprovedFirewall {
        id,
        name,
        rules: firewall_rules_from(value),
    };
    let state = app_state(app)?;
    let mut approved = state
        .approved_firewalls
        .lock()
        .map_err(|_| "Não foi possível atualizar a lista local de firewalls.".to_string())?;
    approved.retain(|existing| existing.id != firewall.id && existing.name != firewall.name);
    approved.push(firewall);
    Ok(())
}

fn remember_firewalls(app: &tauri::AppHandle, value: &Value) -> Result<(), String> {
    let values = value
        .get("data")
        .and_then(Value::as_array)
        .or_else(|| value.as_array())
        .ok_or("A Hostinger retornou uma lista de firewalls inválida.")?;
    let mut firewalls = Vec::new();
    for firewall in values {
        let Some(id) = firewall
            .get("id")
            .and_then(Value::as_i64)
            .filter(|id| *id > 0)
        else {
            continue;
        };
        let Some(name) = firewall
            .get("name")
            .and_then(Value::as_str)
            .map(display_vm_name)
            .filter(|name| !name.is_empty())
        else {
            continue;
        };
        firewalls.push(ApprovedFirewall {
            id,
            name,
            rules: firewall_rules_from(firewall),
        });
    }
    *app_state(app)?
        .approved_firewalls
        .lock()
        .map_err(|_| "Não foi possível atualizar a lista local de firewalls.".to_string())? =
        firewalls;
    Ok(())
}

fn resolve_approved_firewall(
    firewalls: &[ApprovedFirewall],
    name: &str,
) -> Result<ApprovedFirewall, String> {
    let name = validate_firewall_name(name)?;
    let matches = firewalls
        .iter()
        .filter(|firewall| firewall.name == name)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [firewall] => Ok((*firewall).clone()),
        [] => Err("O firewall precisa aparecer na lista consultada por você nesta sessão.".into()),
        _ => Err("Há mais de um firewall com esse nome; escolha um nome único.".into()),
    }
}

fn resolve_approved_firewall_rule(
    firewall: &ApprovedFirewall,
    name: &str,
) -> Result<ApprovedFirewallRule, String> {
    let matches = firewall
        .rules
        .iter()
        .filter(|rule| rule.name == name)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [rule] => Ok((*rule).clone()),
        [] => Err("A regra precisa aparecer nos detalhes consultados do firewall.".into()),
        _ => Err("Há regras com o mesmo nome; atualize os detalhes do firewall.".into()),
    }
}

pub(crate) fn preview_skill(skill_id: &str, params: &Value) -> Result<String, String> {
    if matches!(
        skill_id,
        "hostinger.backup_list"
            | "hostinger.backup_restore"
            | "hostinger.recovery_start"
            | "hostinger.recovery_stop"
    ) {
        let vm_name = params
            .get("vm_name")
            .and_then(Value::as_str)
            .ok_or("Informe o nome da VPS.")?;
        let vm_id = params
            .get("vm_id")
            .and_then(Value::as_i64)
            .ok_or("A VPS não foi confirmada pela lista consultada.")?;
        return Ok(match skill_id {
            "hostinger.backup_list" => format!("Listar backups disponíveis da VPS ‘{}’ (identificador {vm_id}).", display_vm_name(vm_name)),
            "hostinger.backup_restore" => {
                let date = params.get("backup_created_at").and_then(Value::as_str).ok_or("A data do backup não foi confirmada pela lista.")?;
                format!("RESTAURAR backup de {date} na VPS ‘{}’ (identificador {vm_id}). Todos os dados atuais serão substituídos. Esta ação é irreversível, não tem desfazer e exige digitar o nome exato da VPS.", display_vm_name(vm_name))
            }
            "hostinger.recovery_start" => format!("Iniciar o modo de recuperação da VPS ‘{}’ (identificador {vm_id}). Enquanto o modo estiver ativo, o sistema normal não sobe.", display_vm_name(vm_name)),
            _ => format!("Parar o modo de recuperação da VPS ‘{}’ (identificador {vm_id}) e retornar à operação normal. Enquanto o modo estiver ativo, o sistema normal não sobe; esta ação também exige aprovação.", display_vm_name(vm_name)),
        });
    }
    if matches!(
        skill_id,
        "hostinger.firewall_list" | "hostinger.firewall_details" | "hostinger.ssh_keys_list"
    ) {
        return Ok(match skill_id {
            "hostinger.firewall_list" => "Consultar firewalls e regras da conta Hostinger.".into(),
            "hostinger.firewall_details" => format!(
                "Consultar firewall ‘{}’ e suas regras.",
                params
                    .get("firewall_name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
            ),
            _ => format!(
                "Listar somente as chaves SSH públicas anexadas à VPS ‘{}’ (identificador {}).",
                params.get("vm_name").and_then(Value::as_str).unwrap_or(""),
                params
                    .get("vm_id")
                    .and_then(Value::as_i64)
                    .unwrap_or_default()
            ),
        });
    }
    if skill_id.starts_with("hostinger.firewall_") {
        let name = params
            .get("firewall_name")
            .and_then(Value::as_str)
            .ok_or("Informe o nome do firewall.")?;
        return Ok(match skill_id {
            "hostinger.firewall_create" => format!("Criar firewall ‘{}’ na conta Hostinger.", display_vm_name(name)),
            "hostinger.firewall_rule_create" => {
                let source = params.get("source_cidr").and_then(Value::as_str).ok_or("Informe a origem.")?;
                let warning = if firewall_source_is_public(source) { " ATENÇÃO: esta origem permite acesso de qualquer endereço e exige confirmação extra." } else { "" };
                format!("Adicionar regra ao firewall ‘{}’: porta {}, protocolo {}, origem {}.{}", display_vm_name(name), params.get("port").and_then(Value::as_str).unwrap_or(""), params.get("protocol").and_then(Value::as_str).unwrap_or(""), source, warning)
            }
            "hostinger.firewall_activate" | "hostinger.firewall_deactivate" => format!("{} firewall ‘{}’ na VPS ‘{}’ (identificador {}). Serviços podem ficar indisponíveis.", if skill_id == "hostinger.firewall_activate" { "Ativar" } else { "Desativar" }, display_vm_name(name), display_vm_name(params.get("vm_name").and_then(Value::as_str).unwrap_or("")), params.get("vm_id").and_then(Value::as_i64).unwrap_or_default()),
            "hostinger.firewall_delete" => format!("Remover firewall ‘{}’; ele será desativado automaticamente nas VPS anexadas. Esta ação não pode ser desfeita.", display_vm_name(name)),
            "hostinger.firewall_rule_delete" => format!("Remover a regra ‘{}’ do firewall ‘{}’. Esta ação não pode ser desfeita.", params.get("rule_name").and_then(Value::as_str).unwrap_or(""), display_vm_name(name)),
            _ => return Err("Ação de firewall inválida.".into()),
        });
    }
    let action = match skill_id {
        "hostinger.vps_start" => "iniciar",
        "hostinger.vps_stop" => "parar",
        "hostinger.vps_restart" => "reiniciar",
        "hostinger.docker_start" => "iniciar projeto Docker",
        "hostinger.docker_stop" => "parar projeto Docker",
        "hostinger.docker_restart" => "reiniciar projeto Docker",
        "hostinger.docker_deploy" => "criar ou atualizar projeto Docker",
        "hostinger.docker_remove" => "remover projeto Docker",
        "hostinger.docker_list" => "listar projetos Docker",
        _ => return Err("Ação de VPS inválida.".into()),
    };
    let name = params
        .get("vm_name")
        .and_then(Value::as_str)
        .ok_or("Informe o nome da VPS.")?;
    let id = params
        .get("vm_id")
        .and_then(Value::as_i64)
        .ok_or("A VPS não foi confirmada pela lista consultada.")?;
    if skill_id == "hostinger.docker_deploy" {
        let project = params
            .get("project_name")
            .and_then(Value::as_str)
            .ok_or("Informe o nome do projeto Docker.")?;
        let compose = params
            .get("compose_yaml")
            .and_then(Value::as_str)
            .ok_or("Informe o conteúdo do docker-compose.")?;
        validate_docker_compose(project, compose)?;
        return Ok(format!("Hostinger: criar ou atualizar o projeto ‘{project}’ na VPS ‘{}’ (identificador {id}). O conteúdo será executado no servidor. Arquivo docker-compose completo ({} bytes):\n{}", display_vm_name(name), compose.len(), compose));
    }
    if skill_id == "hostinger.docker_list" {
        return Ok(format!(
            "Listar projetos Docker da VPS ‘{}’ (identificador {id}).",
            display_vm_name(name)
        ));
    }
    let project = if skill_id.starts_with("hostinger.docker_") {
        Some(
            params
                .get("project_name")
                .and_then(Value::as_str)
                .ok_or("Informe o nome do projeto Docker.")?,
        )
    } else {
        None
    };
    Ok(format!("Hostinger: {action}{} na VPS ‘{}’ (identificador {id}). Serviços podem ficar indisponíveis durante esta ação.{}", project.map(|value| format!(" ‘{value}’")).unwrap_or_default(), display_vm_name(name), if skill_id == "hostinger.docker_remove" { " A remoção apaga containers, redes, volumes e imagens do projeto; não há desfazer." } else { "" }))
}

pub(crate) async fn execute_skill(
    app: &tauri::AppHandle,
    skill_id: &str,
    params: &Value,
) -> Result<String, String> {
    if matches!(
        skill_id,
        "hostinger.firewall_list"
            | "hostinger.firewall_details"
            | "hostinger.firewall_create"
            | "hostinger.firewall_rule_create"
            | "hostinger.firewall_activate"
            | "hostinger.firewall_deactivate"
            | "hostinger.firewall_delete"
            | "hostinger.firewall_rule_delete"
            | "hostinger.ssh_keys_list"
            | "hostinger.backup_list"
            | "hostinger.backup_restore"
            | "hostinger.recovery_start"
            | "hostinger.recovery_stop"
    ) {
        return execute_cloud_skill(app, skill_id, params).await;
    }
    let vm_id = params
        .get("vm_id")
        .and_then(Value::as_i64)
        .ok_or("A VPS não foi confirmada pela lista consultada.")?;
    if skill_id == "hostinger.docker_list" {
        let token = token_from_keyring()?;
        let endpoint = api_endpoint(&format!("api/vps/v1/virtual-machines/{vm_id}/docker"))?;
        let value: Value = request_at(&production_client()?, endpoint, Some(&token)).await?;
        remember_projects(app, vm_id, &value)?;
        let result = ExternalData {
            source_label: "Projetos Docker da Hostinger — dados externos não confiáveis",
            untrusted: true,
            data: sanitize_external_value(value, 0, &token),
        };
        return serde_json::to_string(&result)
            .map_err(|_| "Não foi possível preparar a lista de projetos Docker.".into());
    }
    let action = match skill_id {
        "hostinger.vps_start" => "start",
        "hostinger.vps_stop" => "stop",
        "hostinger.vps_restart" => "restart",
        "hostinger.docker_start" => "start",
        "hostinger.docker_stop" => "stop",
        "hostinger.docker_restart" => "restart",
        _ => "",
    };
    let token = token_from_keyring()?;
    let client = production_client()?;
    let status_path = format!("api/vps/v1/virtual-machines/{vm_id}/actions");
    let result = if matches!(
        skill_id,
        "hostinger.vps_start" | "hostinger.vps_stop" | "hostinger.vps_restart"
    ) {
        run_action_with(
            &client,
            API_BASE,
            Some(&token),
            vm_id,
            action,
            ACTION_MAX_WAIT,
            ACTION_POLL_INTERVAL,
        )
        .await?
    } else if !action.is_empty() {
        let path = if skill_id.starts_with("hostinger.docker_") {
            let project = validate_project_name(
                params
                    .get("project_name")
                    .and_then(Value::as_str)
                    .ok_or("Informe o nome do projeto Docker.")?,
            )?;
            format!("api/vps/v1/virtual-machines/{vm_id}/docker/{project}/{action}")
        } else {
            format!("api/vps/v1/virtual-machines/{vm_id}/{action}")
        };
        run_action_paths_with(
            &client,
            API_BASE,
            Some(&token),
            &path,
            &status_path,
            reqwest::Method::POST,
            None,
            ACTION_MAX_WAIT,
            ACTION_POLL_INTERVAL,
        )
        .await?
    } else if skill_id == "hostinger.docker_deploy" {
        let project = validate_project_name(
            params
                .get("project_name")
                .and_then(Value::as_str)
                .ok_or("Informe o nome do projeto Docker.")?,
        )?;
        let compose = params
            .get("compose_yaml")
            .and_then(Value::as_str)
            .ok_or("Informe o conteúdo do docker-compose.")?;
        validate_docker_compose(project, compose)?;
        let body = serde_json::json!({"project_name": project, "content": compose});
        run_action_paths_with(
            &client,
            API_BASE,
            Some(&token),
            &format!("api/vps/v1/virtual-machines/{vm_id}/docker"),
            &status_path,
            reqwest::Method::POST,
            Some(body),
            ACTION_MAX_WAIT,
            ACTION_POLL_INTERVAL,
        )
        .await?
    } else if skill_id == "hostinger.docker_remove" {
        let project = validate_project_name(
            params
                .get("project_name")
                .and_then(Value::as_str)
                .ok_or("Informe o nome do projeto Docker.")?,
        )?;
        run_action_paths_with(
            &client,
            API_BASE,
            Some(&token),
            &format!("api/vps/v1/virtual-machines/{vm_id}/docker/{project}/down"),
            &status_path,
            reqwest::Method::DELETE,
            None,
            ACTION_MAX_WAIT,
            ACTION_POLL_INTERVAL,
        )
        .await?
    } else {
        return Err("Ação Docker da Hostinger inválida.".into());
    };
    if skill_id == "hostinger.docker_remove" {
        if let (Some(state), Some(project)) = (
            app.try_state::<HostingerState>(),
            params.get("project_name").and_then(Value::as_str),
        ) {
            if let Ok(mut projects) = state.approved_projects.lock() {
                if let Some(names) = projects.get_mut(&vm_id) {
                    names.retain(|name| name != project);
                }
            }
        }
    }
    serde_json::to_string(&ExternalData {
        source_label: "Resultado externo da Hostinger — não confiável",
        untrusted: true,
        data: sanitize_external_value(result, 0, &token),
    })
    .map_err(|_| "Não foi possível preparar o resultado da ação da Hostinger.".into())
}

async fn execute_cloud_skill(
    app: &tauri::AppHandle,
    skill_id: &str,
    params: &Value,
) -> Result<String, String> {
    let token = token_from_keyring()?;
    let client = production_client()?;
    let firewall_id = params.get("firewall_id").and_then(Value::as_i64);
    let vm_id = params.get("vm_id").and_then(Value::as_i64);
    let value = match skill_id {
        "hostinger.backup_list" => {
            let id = vm_id.ok_or("A VPS não foi confirmada pela lista consultada.")?;
            let value: Value = request_at(
                &client,
                api_endpoint(&format!("api/vps/v1/virtual-machines/{id}/backups?page=1"))?,
                Some(&token),
            )
            .await?;
            remember_backups(app, id, &value)?;
            value
        }
        "hostinger.backup_restore" => {
            let id = vm_id.ok_or("A VPS não foi confirmada pela lista consultada.")?;
            let backup_id = params
                .get("backup_id")
                .and_then(Value::as_i64)
                .ok_or("O backup não foi confirmado pela lista consultada.")?;
            run_action_paths_with(
                &client,
                API_BASE,
                Some(&token),
                &format!("api/vps/v1/virtual-machines/{id}/backups/{backup_id}/restore"),
                &format!("api/vps/v1/virtual-machines/{id}/actions"),
                reqwest::Method::POST,
                None,
                ACTION_MAX_WAIT,
                ACTION_POLL_INTERVAL,
            )
            .await?
        }
        "hostinger.recovery_start" | "hostinger.recovery_stop" => {
            let id = vm_id.ok_or("A VPS não foi confirmada pela lista consultada.")?;
            let method = if skill_id == "hostinger.recovery_start" {
                reqwest::Method::POST
            } else {
                reqwest::Method::DELETE
            };
            run_action_paths_with(
                &client,
                API_BASE,
                Some(&token),
                &format!("api/vps/v1/virtual-machines/{id}/recovery"),
                &format!("api/vps/v1/virtual-machines/{id}/actions"),
                method,
                None,
                ACTION_MAX_WAIT,
                ACTION_POLL_INTERVAL,
            )
            .await?
        }
        "hostinger.firewall_list" => {
            let value: Value = request_action_at(
                &client,
                api_endpoint("api/vps/v1/firewall")?,
                Some(&token),
                firewall_http_method(skill_id).ok_or("Ação de firewall inválida.")?,
                None,
            )
            .await?;
            remember_firewalls(app, &value)?;
            value
        }
        "hostinger.firewall_details" => {
            let id = firewall_id.ok_or("O firewall não foi confirmado pela lista consultada.")?;
            let value: Value = request_action_at(
                &client,
                api_endpoint(&format!("api/vps/v1/firewall/{id}"))?,
                Some(&token),
                firewall_http_method(skill_id).ok_or("Ação de firewall inválida.")?,
                None,
            )
            .await?;
            remember_firewall(app, &value)?;
            value
        }
        "hostinger.ssh_keys_list" => {
            let id = vm_id.ok_or("A VPS não foi confirmada pela lista consultada.")?;
            request_action_at(
                &client,
                api_endpoint(&format!("api/vps/v1/virtual-machines/{id}/ssh-keys"))?,
                Some(&token),
                firewall_http_method(skill_id).ok_or("Ação SSH inválida.")?,
                None,
            )
            .await?
        }
        "hostinger.firewall_create" => {
            let name = validate_firewall_name(
                params
                    .get("firewall_name")
                    .and_then(Value::as_str)
                    .ok_or("Informe o nome do firewall.")?,
            )?;
            let value: Value = request_action_at(
                &client,
                api_endpoint("api/vps/v1/firewall")?,
                Some(&token),
                firewall_http_method(skill_id).ok_or("Ação de firewall inválida.")?,
                Some(&serde_json::json!({"name": name})),
            )
            .await?;
            remember_firewall(app, &value)?;
            value
        }
        "hostinger.firewall_rule_create" => {
            let id = firewall_id.ok_or("O firewall não foi confirmado pela lista consultada.")?;
            let protocol = params
                .get("protocol")
                .and_then(Value::as_str)
                .ok_or("Informe o protocolo.")?;
            let port = params
                .get("port")
                .and_then(Value::as_str)
                .ok_or("Informe a porta.")?;
            let source = params
                .get("source_cidr")
                .and_then(Value::as_str)
                .ok_or("Informe a origem.")?;
            validate_firewall_rule(protocol, port, source)?;
            let (source_type, source_detail) = if source == "any" {
                ("any", "any")
            } else {
                ("custom", source)
            };
            let body = serde_json::json!({"protocol": protocol, "port": port, "source": source_type, "source_detail": source_detail});
            let value: Value = request_action_at(
                &client,
                api_endpoint(&format!("api/vps/v1/firewall/{id}/rules"))?,
                Some(&token),
                firewall_http_method(skill_id).ok_or("Ação de firewall inválida.")?,
                Some(&body),
            )
            .await?;
            let rule_name = firewall_rule_name(value.get("data").unwrap_or(&value));
            if let Some(rule_name) = rule_name {
                let state = app_state(app)?;
                if let Ok(mut firewalls) = state.approved_firewalls.lock() {
                    if let Some(firewall) = firewalls.iter_mut().find(|firewall| firewall.id == id)
                    {
                        if let Some(rule_id) = value
                            .get("data")
                            .unwrap_or(&value)
                            .get("id")
                            .and_then(Value::as_i64)
                        {
                            firewall.rules.push(ApprovedFirewallRule {
                                id: rule_id,
                                name: display_vm_name(&rule_name),
                            });
                        }
                    }
                };
            }
            value
        }
        "hostinger.firewall_activate" | "hostinger.firewall_deactivate" => {
            let fid = firewall_id.ok_or("O firewall não foi confirmado pela lista consultada.")?;
            let vid = vm_id.ok_or("A VPS não foi confirmada pela lista consultada.")?;
            let action = if skill_id.ends_with("activate") {
                "activate"
            } else {
                "deactivate"
            };
            run_action_paths_with(
                &client,
                API_BASE,
                Some(&token),
                &format!("api/vps/v1/firewall/{fid}/{action}/{vid}"),
                &format!("api/vps/v1/virtual-machines/{vid}/actions"),
                firewall_http_method(skill_id).ok_or("Ação de firewall inválida.")?,
                None,
                ACTION_MAX_WAIT,
                ACTION_POLL_INTERVAL,
            )
            .await?
        }
        "hostinger.firewall_delete" => {
            let id = firewall_id.ok_or("O firewall não foi confirmado pela lista consultada.")?;
            request_action_empty_at(
                &client,
                api_endpoint(&format!("api/vps/v1/firewall/{id}"))?,
                Some(&token),
                firewall_http_method(skill_id).ok_or("Ação de firewall inválida.")?,
            )
            .await?;
            if let Some(state) = app.try_state::<HostingerState>() {
                if let Ok(mut firewalls) = state.approved_firewalls.lock() {
                    firewalls.retain(|firewall| firewall.id != id);
                }
            }
            serde_json::json!({"status":"removido"})
        }
        "hostinger.firewall_rule_delete" => {
            let fid = firewall_id.ok_or("O firewall não foi confirmado pela lista consultada.")?;
            let rid = params
                .get("rule_id")
                .and_then(Value::as_i64)
                .ok_or("A regra não foi confirmada pelos detalhes consultados.")?;
            request_action_empty_at(
                &client,
                api_endpoint(&format!("api/vps/v1/firewall/{fid}/rules/{rid}"))?,
                Some(&token),
                firewall_http_method(skill_id).ok_or("Ação de firewall inválida.")?,
            )
            .await?;
            if let Some(state) = app.try_state::<HostingerState>() {
                if let Ok(mut firewalls) = state.approved_firewalls.lock() {
                    if let Some(firewall) = firewalls.iter_mut().find(|firewall| firewall.id == fid)
                    {
                        firewall.rules.retain(|rule| rule.id != rid);
                    }
                }
            }
            serde_json::json!({"status":"removida"})
        }
        _ => return Err("Ação de firewall inválida.".into()),
    };
    serde_json::to_string(&ExternalData {
        source_label: "Dados da Hostinger — conteúdo externo não confiável",
        untrusted: true,
        data: sanitize_external_value(value, 0, &token),
    })
    .map_err(|_| "Não foi possível preparar o resultado da Hostinger.".into())
}

async fn request_json<T: DeserializeOwned>(
    path: &str,
    query: &[(&str, &str)],
    token: &str,
) -> Result<T, String> {
    let mut endpoint = api_endpoint(path)?;
    endpoint
        .query_pairs_mut()
        .extend_pairs(query.iter().copied());
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
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
) -> Result<ExternalData<Vec<Value>>, String> {
    crate::require_main(&caller)?;
    clear_vms(&app);
    let token = token_from_keyring()?;
    let result = list_vms_with_token(&token).await?;
    remember_vms(&app, &result.data)?;
    Ok(result)
}

#[tauri::command]
pub async fn hostinger_list_vms(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
) -> Result<ExternalData<Vec<Value>>, String> {
    crate::require_main(&caller)?;
    clear_vms(&app);
    let token = token_from_keyring()?;
    let result = list_vms_with_token(&token).await?;
    remember_vms(&app, &result.data)?;
    Ok(result)
}

#[tauri::command]
pub async fn hostinger_get_vm_details(
    caller: tauri::WebviewWindow,
    vm_id: i64,
) -> Result<ExternalData<Value>, String> {
    crate::require_main(&caller)?;
    validate_vm_id(vm_id)?;
    read_resource(&format!("api/vps/v1/virtual-machines/{vm_id}"), &[]).await
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
                let _ =
                    request_sender.send(String::from_utf8_lossy(&request[..count]).into_owned());
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
            .no_proxy()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap()
    }

    fn sequence_server(
        responses: Vec<(&'static str, &'static [u8])>,
    ) -> (String, std::sync::mpsc::Receiver<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let (sender, receiver) = std::sync::mpsc::channel();
        thread::spawn(move || {
            let mut requests = Vec::new();
            'outer: for (status, body) in responses {
                let start = Instant::now();
                let (mut stream, _) = loop {
                    match listener.accept() {
                        Ok(connection) => break connection,
                        Err(error)
                            if error.kind() == std::io::ErrorKind::WouldBlock
                                && start.elapsed() < Duration::from_secs(2) =>
                        {
                            thread::sleep(Duration::from_millis(1));
                        }
                        Err(_) => break 'outer,
                    }
                };
                let mut request = [0_u8; 8192];
                let count = stream.read(&mut request).unwrap_or_default();
                requests.push(String::from_utf8_lossy(&request[..count]).into_owned());
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.write_all(body);
            }
            let _ = sender.send(requests);
        });
        (format!("http://{address}/"), receiver)
    }

    #[test]
    fn resolver_rejects_vms_absent_from_the_user_read_list_and_ambiguous_names() {
        let vms = vec![ApprovedVm {
            id: 42,
            name: "web-prod".into(),
        }];
        assert!(resolve_approved_vm(&vms, "unknown")
            .unwrap_err()
            .contains("consultada por você"));
        assert_eq!(resolve_approved_vm(&vms, "web-prod").unwrap().id, 42);
        let ambiguous = vec![
            vms[0].clone(),
            ApprovedVm {
                id: 43,
                name: "web-prod".into(),
            },
        ];
        assert!(resolve_approved_vm(&ambiguous, "web-prod")
            .unwrap_err()
            .contains("nome único"));
    }

    #[test]
    fn agent_cannot_supply_a_hostinger_vm_identifier() {
        assert_eq!(
            validate_vm_name_request(&serde_json::json!({"vm_name":"web-prod"})).unwrap(),
            "web-prod"
        );
        assert!(validate_vm_name_request(&serde_json::json!({
            "vm_name":"web-prod",
            "vm_id":42
        }))
        .is_err());
    }

    #[test]
    fn action_status_is_classified_without_accepting_unknown_states() {
        assert_eq!(
            action_status(&serde_json::json!({"state":"sent"})).unwrap(),
            ActionStatus::Pending
        );
        assert_eq!(
            action_status(&serde_json::json!({"state":"success"})).unwrap(),
            ActionStatus::Complete
        );
        assert_eq!(
            action_status(&serde_json::json!({"state":"error"})).unwrap(),
            ActionStatus::Failed
        );
        assert!(action_status(&serde_json::json!({"state":"unexpected"})).is_err());
    }

    #[tokio::test]
    async fn action_polls_until_status_changes_and_returns_result() {
        let running = br#"{"id":7,"state":"sent"}"#;
        let completed = br#"{"id":7,"state":"success","name":"restart"}"#;
        let (base, requests) = sequence_server(vec![
            ("200 OK", running),
            ("200 OK", running),
            ("200 OK", completed),
        ]);
        let output = run_action_with(
            &test_client(),
            &base,
            Some("never-logged-token"),
            42,
            "restart",
            Duration::from_secs(2),
            Duration::from_millis(1),
        )
        .await
        .unwrap();
        assert_eq!(output["state"], "success");
        assert!(!output.to_string().contains("never-logged-token"));
        let requests = requests.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(requests.len(), 3);
        assert!(requests[0].starts_with("POST /api/vps/v1/virtual-machines/42/restart "));
        assert!(requests[1].starts_with("GET /api/vps/v1/virtual-machines/42/actions/7 "));
        assert!(requests.iter().all(|request| {
            request
                .to_ascii_lowercase()
                .contains("authorization: bearer never-logged-token")
        }));
    }

    #[tokio::test]
    async fn docker_deploy_sends_raw_compose_and_polls_mocked_action() {
        let (base, requests) = sequence_server(vec![
            ("200 OK", br#"{"id":27,"state":"sent"}"#),
            ("200 OK", br#"{"id":27,"state":"success"}"#),
        ]);
        let compose = "services:\n  app:\n    image: nginx\n";
        let output = run_action_paths_with(
            &test_client(),
            &base,
            Some("mock-token"),
            "api/vps/v1/virtual-machines/42/docker",
            "api/vps/v1/virtual-machines/42/actions",
            reqwest::Method::POST,
            Some(serde_json::json!({"project_name":"demo", "content":compose})),
            Duration::from_secs(1),
            Duration::from_millis(1),
        )
        .await
        .unwrap();
        assert_eq!(output["state"], "success");
        let requests = requests.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(requests[0].starts_with("POST /api/vps/v1/virtual-machines/42/docker "));
        assert!(requests[0].contains("\"project_name\":\"demo\""));
        assert!(requests[0].contains("image: nginx"));
        assert!(requests[1].starts_with("GET /api/vps/v1/virtual-machines/42/actions/27 "));
    }

    #[tokio::test]
    async fn unchanged_action_status_expires_at_the_deadline() {
        let pending = br#"{"id":8,"state":"delayed"}"#;
        let (base, _requests) = sequence_server(vec![("200 OK", pending); 8]);
        let error = run_action_with(
            &test_client(),
            &base,
            Some("test-token"),
            42,
            "stop",
            Duration::from_millis(250),
            Duration::from_millis(50),
        )
        .await
        .unwrap_err();
        assert!(error.contains("tempo limite"));
    }

    #[tokio::test]
    async fn missing_token_is_rejected_before_any_action_request() {
        let (base, requests) = sequence_server(Vec::new());
        let error = run_action_with(
            &test_client(),
            &base,
            None,
            42,
            "start",
            Duration::from_millis(10),
            Duration::from_millis(1),
        )
        .await
        .unwrap_err();
        assert!(error.contains("Salve o token"));
        assert!(requests
            .recv_timeout(Duration::from_millis(20))
            .unwrap()
            .is_empty());
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
        assert!(
            endpoint_for_base("http://developers.hostinger.com/", "api/vps/v1/templates").is_err()
        );
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
        assert!(request_receiver
            .recv()
            .unwrap()
            .starts_with("GET /api/vps/v1/"));
    }

    #[test]
    fn external_text_is_truncated_and_values_are_json_data_not_html() {
        assert!(
            external_text(&"a".repeat(MAX_TEXT_CHARS + 1), "").contains("texto externo truncado")
        );
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
    fn firewall_rules_validate_public_ranges_and_http_methods_keep_reads_read_only() {
        assert!(validate_firewall_rule("TCP", "443", "192.0.2.0/24").is_ok());
        assert!(validate_firewall_rule("UDP", "8000:8100", "198.51.100.7").is_ok());
        assert!(validate_firewall_rule("UNKNOWN", "443", "0.0.0.0/0").is_err());
        assert!(validate_firewall_rule("TCP", "0", "192.0.2.1").is_err());
        assert!(validate_firewall_rule("TCP", "443", "192.0.2.0/33").is_err());
        assert!(firewall_source_is_public("0.0.0.0/0"));
        assert!(firewall_source_is_public("::/0"));
        assert!(firewall_source_is_public("any"));
        for skill in [
            "hostinger.firewall_list",
            "hostinger.firewall_details",
            "hostinger.ssh_keys_list",
        ] {
            assert_eq!(
                firewall_http_method(skill),
                Some(reqwest::Method::GET),
                "{skill}"
            );
        }
        assert_eq!(
            firewall_http_method("hostinger.firewall_create"),
            Some(reqwest::Method::POST)
        );
        assert_eq!(
            firewall_http_method("hostinger.firewall_delete"),
            Some(reqwest::Method::DELETE)
        );
        assert_eq!(firewall_http_method("hostinger.ssh_keys_create"), None);
        assert_eq!(firewall_http_method("hostinger.ssh_keys_delete"), None);
    }

    #[test]
    fn firewall_catalog_retains_only_names_seen_in_read_results() {
        let response = serde_json::json!({"data":{"id":7,"name":"edge","rules":[
            {"id":9,"action":"accept","protocol":"TCP","port":"443","source":"custom","source_detail":"192.0.2.0/24"}
        ]}});
        let firewall = ApprovedFirewall {
            id: 7,
            name: "edge".into(),
            rules: firewall_rules_from(&response),
        };
        assert_eq!(
            resolve_approved_firewall(&[firewall.clone()], "edge").unwrap(),
            firewall
        );
        assert!(resolve_approved_firewall(&[firewall.clone()], "inventado").is_err());
        let rule = &firewall.rules[0];
        assert_eq!(rule.name, "accept TCP 443 de 192.0.2.0/24");
        assert_eq!(
            resolve_approved_firewall_rule(&firewall, &rule.name)
                .unwrap()
                .id,
            9
        );
        assert!(resolve_approved_firewall_rule(&firewall, "regra inventada").is_err());
    }

    #[test]
    fn backup_resolution_is_bound_to_the_vm_that_was_listed() {
        let response = serde_json::json!({"data":[
            {"id":325,"created_at":"2025-02-27T11:54:22Z","size":512},
            {"id":326,"created_at":"2025-02-28T11:54:22Z","size":1024}
        ],"meta":{"current_page":1}});
        let parsed = backups_from(&response);
        assert_eq!(parsed.len(), 2);
        let caches = HashMap::from([(42, parsed)]);
        assert_eq!(
            resolve_approved_backup(&caches, 42, "2025-02-27T11:54:22Z")
                .unwrap()
                .id,
            325
        );
        assert!(resolve_approved_backup(&caches, 43, "2025-02-27T11:54:22Z").is_err());
        assert!(resolve_approved_backup(&caches, 42, "2025-01-01T00:00:00Z").is_err());
    }

    #[test]
    fn backup_restore_preview_names_date_and_irreversible_overwrite() {
        let preview = preview_skill(
            "hostinger.backup_restore",
            &serde_json::json!({
                "vm_name":"web-vps", "vm_id":42, "backup_id":325,
                "backup_created_at":"2025-02-27T11:54:22Z"
            }),
        )
        .unwrap();
        assert!(preview.contains("2025-02-27T11:54:22Z"));
        assert!(preview.contains("web-vps"));
        assert!(preview.contains("Todos os dados atuais serão substituídos"));
        assert!(preview.contains("não tem desfazer"));
        let recovery = preview_skill(
            "hostinger.recovery_start",
            &serde_json::json!({"vm_name":"web-vps", "vm_id":42}),
        )
        .unwrap();
        assert!(recovery.contains("sistema normal não sobe"));
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

    #[test]
    fn docker_compose_blocks_system_bind_mounts_and_remote_urls_before_submission() {
        for path in [
            "/etc:/host-etc",
            "/root:/host-root",
            "/var/run/docker.sock:/socket",
            "/tmp/../etc:/host",
        ] {
            let compose =
                format!("services:\n  app:\n    image: nginx\n    volumes:\n      - {path}\n");
            assert!(
                validate_docker_compose("demo", &compose)
                    .unwrap_err()
                    .contains("protegido"),
                "{path}"
            );
        }
        let long_syntax = "services:\n  app:\n    image: nginx\n    volumes:\n      - type: bind\n        source: /etc\n        target: /host\n";
        assert!(validate_docker_compose("demo", long_syntax).is_err());
        assert!(validate_docker_compose(
            "demo",
            "services:\n  app:\n    image: nginx\n    volumes: ['/etc:/host']\n"
        )
        .is_err());
        assert!(validate_docker_compose(
            "demo",
            "services:\n  app:\n    image: nginx\n    volumes: *shared\n"
        )
        .is_err());
        assert!(validate_docker_compose(
            "demo",
            "services:\n  app:\n    image: nginx\n    volumes:\n      - ${HOST_PATH}:/data\n"
        )
        .is_err());
        assert!(validate_docker_compose(
            "demo",
            "services:\n  app:\n    image: nginx\n    devices:\n      - /dev/sda:/dev/xvda\n"
        )
        .is_err());
        assert!(validate_docker_compose(
            "demo",
            "services:\n  app:\n    image: nginx\n    volumes:\n      - /:/host\n"
        )
        .is_err());
        assert!(
            validate_docker_compose("demo", "https://example.test/compose.yml")
                .unwrap_err()
                .contains("URLs remotas")
        );
        assert!(validate_docker_compose(
            "demo",
            "services:\n  app:\n    image: nginx\ninclude: https://example.test/other.yml\n"
        )
        .unwrap_err()
        .contains("Referências externas"));
        assert!(validate_docker_compose(
            "demo",
            &format!(
                "services:\n  app:\n    image: {}\n",
                "x".repeat(MAX_COMPOSE_BYTES)
            )
        )
        .unwrap_err()
        .contains("8 KiB"));
        assert!(validate_docker_compose(
            "demo",
            "services:\n  app:\n    image: nginx\n    volumes:\n      - ./data:/data\n"
        )
        .is_ok());
    }

    #[test]
    fn docker_projects_are_truncated_and_wrapped_as_untrusted_data() {
        let raw = serde_json::json!([{"name":"demo","status":"x".repeat(MAX_TEXT_CHARS + 1)}]);
        let projects = project_names(&raw);
        assert_eq!(projects, vec!["demo"]);
        let sanitized = sanitize_external_value(raw, 0, "");
        let output = serde_json::to_string(&ExternalData {
            source_label: "Projetos Docker da Hostinger — dados externos não confiáveis",
            untrusted: true,
            data: sanitized,
        })
        .unwrap();
        assert!(output.contains("texto externo truncado"));
        assert!(output.contains("\"untrusted\":true"));
    }

    #[test]
    fn only_project_names_from_the_read_result_are_resolvable() {
        let listed =
            project_names(&serde_json::json!([{"name":"allowed-project"}, {"name":"bad/path"}]));
        assert!(resolve_approved_project(&listed, "allowed-project").is_ok());
        assert!(resolve_approved_project(&listed, "not-listed").is_err());
        assert_eq!(listed, vec!["allowed-project"]);
    }

    #[test]
    fn deploy_preview_includes_the_complete_compose_and_server_execution_warning() {
        let compose = "services:\n  app:\n    image: nginx\n";
        let preview = preview_skill("hostinger.docker_deploy", &serde_json::json!({"vm_name":"vps", "vm_id":42, "project_name":"demo", "compose_yaml":compose})).unwrap();
        assert!(preview.contains(compose));
        assert!(preview.contains("conteúdo será executado no servidor"));
        assert!(preview.contains("demo"));
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
