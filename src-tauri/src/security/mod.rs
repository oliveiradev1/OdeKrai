use crate::{
    agent_home,
    policy::{self, Decision, Mode, Origin, Risk},
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, VecDeque},
    fs::{self, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

mod approvals;
mod audit;
mod dispatch;
mod kill_switch;
pub mod skills;
pub(crate) mod workspace;
#[cfg(test)]
pub(crate) use approvals::ApprovalBroker;
pub(crate) use approvals::ApprovalDecision;
use approvals::{
    active_pending_count, display_hash_matches, is_expired, lock_pending, queue_has_capacity,
    take_pending_approval, PendingAction,
};
use audit::verify_audit_path;
use dispatch::*;
pub use kill_switch::{handle_global_shortcut, init_shortcut, load_halted};
use workspace::*;

#[cfg(debug_assertions)]
#[tauri::command]
pub async fn submit_simulated_action(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    state: State<'_, SecurityState>,
    request: ActionRequest,
) -> Result<ActionResult, String> {
    crate::require_main(&caller)?;
    dispatch::submit_simulated_action_inner(app, state, request).await
}

#[tauri::command]
pub async fn undo_agent_action(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    state: State<'_, SecurityState>,
    descriptor: crate::history::UndoDescriptor,
    confirm_modified: bool,
) -> Result<ActionResult, String> {
    crate::require_main(&caller)?;
    if !matches!(
        descriptor.skill_id.as_str(),
        "wallpaper.restore" | "display.restore_brightness" | "theme.restore" | "files.restore"
    ) || descriptor.state_id.is_empty()
        || descriptor.state_id.len() > 128
        || descriptor.state_id.chars().any(char::is_control)
    {
        return Err("Este resultado não possui uma restauração disponível.".into());
    }
    let is_file_restore = descriptor.skill_id == "files.restore";
    let state_id = descriptor.state_id.clone();
    let action = SimulatedAction::Productivity {
        skill_id: descriptor.skill_id.clone(),
        params: if is_file_restore {
            let path = descriptor
                .path
                .as_deref()
                .ok_or("O cartão não identifica o arquivo para restaurar.")?;
            let (root, normalized) = skills::resolve_action_target(
                &app,
                &SimulatedAction::Productivity {
                    skill_id: "files.restore".into(),
                    params: serde_json::json!({"path":path,"state_id":state_id.clone(),"confirm_modified":confirm_modified}),
                },
                true,
            )?;
            let SimulatedAction::Productivity { params, .. } = normalized else {
                unreachable!()
            };
            let normalized_path = params["path"]
                .as_str()
                .ok_or("Caminho de restauração inválido.")?;
            let (_, changed) = workspace::inspect_file_undo(&root, &state_id, normalized_path)?;
            if changed && !confirm_modified {
                return Ok(ActionResult { id: state_id, status: "confirmation_required".into(), message: "O arquivo foi alterado depois da ação. Confirme se quer restaurar mesmo assim; a versão atual será preservada em .history.".into(), result: None, undo_descriptor: None, confirmation_required: true });
            }
            serde_json::json!({"path":normalized_path,"state_id":state_id.clone(),"confirm_modified":confirm_modified})
        } else {
            serde_json::json!({"history_id": state_id.clone()})
        },
    };
    let result =
        dispatch::submit_simulated_action_inner(app.clone(), state, ActionRequest { action })
            .await?;
    if result.status == "completed" {
        crate::history::clear_undo_descriptor_for_state(&app, &state_id)?;
    }
    Ok(result)
}

#[tauri::command]
pub async fn get_pending_approval(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    state: State<'_, SecurityState>,
) -> Result<Option<PendingView>, String> {
    crate::require_caller(&caller, &["approval"])?;
    dispatch::get_pending_approval_inner(app, state).await
}

#[tauri::command]
pub fn get_pending_approval_count(
    caller: tauri::WebviewWindow,
    state: State<'_, SecurityState>,
) -> Result<usize, String> {
    crate::require_main(&caller)?;
    dispatch::get_pending_approval_count_inner(state)
}

#[tauri::command]
pub fn show_pending_approval_window(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    state: State<'_, SecurityState>,
) -> Result<(), String> {
    crate::require_main(&caller)?;
    if dispatch::get_pending_approval_count_inner(state)? == 0 {
        return Err("Não há aprovações pendentes.".into());
    }
    dispatch::open_approval(&app)
}

#[tauri::command]
pub fn get_full_approval_preview(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    state: State<'_, SecurityState>,
    id: String,
    display_hash: String,
) -> Result<String, String> {
    crate::require_caller(&caller, &["approval"])?;
    dispatch::get_full_approval_preview_inner(app, state, id, display_hash)
}

#[tauri::command]
pub async fn approval_respond(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    state: State<'_, SecurityState>,
    id: String,
    display_hash: String,
    approve: bool,
) -> Result<ActionResult, String> {
    crate::require_caller(&caller, &["approval"])?;
    dispatch::approval_respond_inner(app, state, id, display_hash, approve).await
}

#[tauri::command]
pub fn halt_all(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    state: State<'_, SecurityState>,
) -> Result<(), String> {
    kill_switch::halt_all_inner(app, caller, state)
}

#[tauri::command]
pub fn resume_all(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    state: State<'_, SecurityState>,
) -> Result<(), String> {
    kill_switch::resume_all_inner(app, caller, state)
}

const DEFAULT_APPROVAL_TTL: u64 = 120;
const MIN_APPROVAL_TTL: u64 = 30;
const MAX_APPROVAL_TTL: u64 = 600;
const MAX_PENDING: usize = 3;
const MAX_FILE_BYTES: usize = 1_000_000;
const RATE_LIMIT: usize = 5;

pub struct SecurityState {
    halted: AtomicBool,
    pending: Mutex<VecDeque<PendingAction>>,
    rate: Mutex<HashMap<String, VecDeque<Instant>>>,
    audit_lock: Mutex<()>,
    shortcut_error: Mutex<Option<String>>,
    quick_shortcut_error: Mutex<Option<String>>,
    active_kill_shortcut_id: Mutex<Option<u32>>,
    active_quick_shortcut_id: Mutex<Option<u32>>,
    quick_focus_requested: AtomicBool,
    pub(crate) approval_broker: approvals::ApprovalBroker,
    pub(crate) agent_runs: tokio::sync::Mutex<HashMap<String, tokio_util::sync::CancellationToken>>,
}

impl SecurityState {
    pub fn new(halted: bool) -> Self {
        Self {
            halted: AtomicBool::new(halted),
            pending: Mutex::new(VecDeque::new()),
            rate: Mutex::new(HashMap::new()),
            audit_lock: Mutex::new(()),
            shortcut_error: Mutex::new(None),
            quick_shortcut_error: Mutex::new(None),
            active_kill_shortcut_id: Mutex::new(None),
            active_quick_shortcut_id: Mutex::new(None),
            quick_focus_requested: AtomicBool::new(false),
            approval_broker: approvals::ApprovalBroker::new(
                MAX_PENDING,
                Duration::from_secs(DEFAULT_APPROVAL_TTL),
            ),
            agent_runs: tokio::sync::Mutex::new(HashMap::new()),
        }
    }

    pub fn is_halted(&self) -> bool {
        self.halted.load(Ordering::SeqCst)
    }

    pub fn set_initial_halted(&self, halted: bool) {
        self.halted.store(halted, Ordering::SeqCst);
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "tool", deny_unknown_fields)]
pub enum SimulatedAction {
    #[serde(rename = "files.list")]
    FilesList {
        path: String,
        depth: u8,
        limit: usize,
    },
    #[serde(rename = "files.search")]
    FilesSearch {
        path: String,
        query: String,
        by: String,
        depth: u8,
        max_file_bytes: usize,
        limit: usize,
    },
    #[serde(rename = "files.read")]
    FilesRead { path: String },
    #[serde(rename = "productivity")]
    Productivity {
        skill_id: String,
        params: serde_json::Value,
    },
    #[serde(rename = "workspace.read_file")]
    ReadFile { path: String },
    #[serde(rename = "workspace.write_file")]
    WriteFile { path: String, content: String },
    #[serde(rename = "workspace.delete_file")]
    DeleteFile { path: String },
    #[serde(rename = "simulated.send_message")]
    SendMessage { recipient: String, message: String },
    #[serde(rename = "simulated.payment")]
    Payment { amount_cents: u64, currency: String },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionRequest {
    pub action: SimulatedAction,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionSettings {
    pub mode: Mode,
    pub shortcut: String,
    pub shortcut_error: Option<String>,
    pub halted: bool,
    pub approval_ttl_seconds: u64,
    pub overrides: Vec<ToolOverride>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolOverride {
    pub tool: String,
    pub more_permissive: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionResult {
    pub id: String,
    pub status: String,
    pub message: String,
    pub result: Option<String>,
    #[serde(default)]
    pub undo_descriptor: Option<crate::history::UndoDescriptor>,
    #[serde(default)]
    pub confirmation_required: bool,
}

pub(crate) fn audit_provider_fallback(
    app: &tauri::AppHandle,
    state: &SecurityState,
    from: &str,
    to: &str,
    reason: &str,
) -> Result<(), String> {
    dispatch::record(
        app,
        state,
        &id(),
        "provider.failover",
        Risk::Leitura,
        &Origin::Model,
        "retry",
        reason,
        "not_required",
        0,
        format!("from={from}; to={to}"),
        "continued",
        None,
    )
}

pub(crate) fn audit_provider_usage_limit(
    app: &tauri::AppHandle,
    state: &SecurityState,
    provider_id: &str,
    used: u64,
    limit: u64,
) -> Result<(), String> {
    dispatch::record(
        app,
        state,
        &id(),
        "provider.daily_token_limit",
        Risk::Leitura,
        &Origin::User,
        "stop",
        "daily_token_limit",
        "not_required",
        0,
        format!("provider={provider_id}; used={used}; limit={limit}"),
        "blocked",
        Some("O limite diário de tokens foi atingido.".into()),
    )
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingView {
    pub id: String,
    pub tool: String,
    pub risk: Risk,
    pub origin: String,
    pub summary: String,
    pub preview: String,
    pub preview_image: Option<String>,
    pub full_preview: bool,
    pub expires_at: u64,
    pub display_hash: String,
}

#[derive(Clone, Serialize, Deserialize)]
struct AuditPayload {
    timestamp: u64,
    id: String,
    tool: String,
    risk: Risk,
    mode: Mode,
    origin: String,
    decision: String,
    rule_id: String,
    approval: String,
    latency_ms: u64,
    params_summary: String,
    status: String,
    error: Option<String>,
    previous_hash: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AuditLine {
    #[serde(flatten)]
    payload: AuditPayload,
    hash: String,
}

fn clipped(text: &str, max: usize) -> String {
    let mut output: String = text.chars().take(max).collect();
    if text.chars().count() > max {
        output.push_str("\n[conteúdo truncado]");
    }
    output
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
fn id() -> String {
    format!(
        "act-{}-{}",
        now_millis(),
        ID_COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}
static ID_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

pub(crate) fn database(app: &tauri::AppHandle) -> Result<Connection, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|_| "Não foi possível localizar os dados do aplicativo.".to_string())?;
    fs::create_dir_all(&dir)
        .map_err(|_| "Não foi possível preparar os dados do aplicativo.".to_string())?;
    let db = Connection::open(dir.join("permissions.sqlite3"))
        .map_err(|_| "Não foi possível abrir as configurações de segurança.".to_string())?;
    let schema = format!("PRAGMA journal_mode=WAL; CREATE TABLE IF NOT EXISTS permission_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL); CREATE TABLE IF NOT EXISTS tool_overrides (tool TEXT PRIMARY KEY, more_permissive INTEGER NOT NULL CHECK(more_permissive IN (0,1))); CREATE TABLE IF NOT EXISTS skill_settings (skill_id TEXT PRIMARY KEY, enabled INTEGER NOT NULL CHECK(enabled IN (0,1))); CREATE TABLE IF NOT EXISTS allowed_folders (path TEXT PRIMARY KEY, access TEXT NOT NULL CHECK(access IN ('read_only','read_write'))); CREATE TABLE IF NOT EXISTS trusted_external_domains (domain TEXT PRIMARY KEY); CREATE TABLE IF NOT EXISTS allowed_apps (app_id TEXT PRIMARY KEY, name TEXT NOT NULL, path TEXT NOT NULL); CREATE TABLE IF NOT EXISTS discord_webhooks (id TEXT PRIMARY KEY, name TEXT NOT NULL, last_sent INTEGER NOT NULL DEFAULT 0); CREATE TABLE IF NOT EXISTS mcp_servers (id TEXT PRIMARY KEY, name TEXT NOT NULL, command TEXT NOT NULL, args_json TEXT NOT NULL, env_names_json TEXT NOT NULL, enabled INTEGER NOT NULL CHECK(enabled IN (0,1))); INSERT OR IGNORE INTO permission_settings(key,value) VALUES ('mode','sempre_perguntar'),('shortcut','Ctrl+Shift+F12'),('island_quick_shortcut_enabled','false'),('island_quick_shortcut','Ctrl+Alt+Space'),('shardcloud_monthly_budget_cents',''),('halted','false'),('approval_ttl_seconds','{DEFAULT_APPROVAL_TTL}'),('agent_max_turns','8'),('agent_max_duration_seconds','120'),('agent_max_tokens','16000'),('agent_max_external_actions','2');");
    db.execute_batch(&schema)
        .map_err(|_| "Não foi possível preparar as configurações de segurança.".to_string())?;
    Ok(db)
}

pub(crate) fn db_setting(app: &tauri::AppHandle, key: &str) -> Result<String, String> {
    database(app)?
        .query_row(
            "SELECT value FROM permission_settings WHERE key=?1",
            [key],
            |row| row.get(0),
        )
        .map_err(|_| "Não foi possível ler as configurações de segurança.".into())
}

pub(crate) fn set_db_setting(app: &tauri::AppHandle, key: &str, value: &str) -> Result<(), String> {
    database(app)?.execute("INSERT INTO permission_settings(key,value) VALUES (?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key,value]).map_err(|_| "Não foi possível salvar as configurações de segurança.".to_string())?;
    Ok(())
}

fn normalize_external_domain(raw: &str) -> Result<String, String> {
    if raw.is_empty()
        || raw.len() > 253
        || raw.trim() != raw
        || raw.chars().any(char::is_control)
        || raw.contains(['/', '\\', '@', '?', '#'])
        || (raw.contains(':') && !(raw.starts_with('[') && raw.ends_with(']')))
    {
        return Err("Informe somente um domínio, sem esquema, porta ou caminho.".into());
    }
    let url = url::Url::parse(&format!("https://{raw}"))
        .map_err(|_| "O domínio informado é inválido.")?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.path() != "/"
    {
        return Err("Informe somente um domínio, sem esquema, porta ou caminho.".into());
    }
    url.host_str()
        .map(|host| host.trim_end_matches('.').to_ascii_lowercase())
        .filter(|host| !host.is_empty())
        .ok_or_else(|| "O domínio informado é inválido.".into())
}

fn insert_external_domain(app: &tauri::AppHandle, domain: &str) -> Result<(), String> {
    let domain = normalize_external_domain(domain)?;
    let db = database(app)?;
    let exists: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM trusted_external_domains WHERE domain=?1)",
            [&domain],
            |row| row.get(0),
        )
        .map_err(|_| "Não foi possível ler os domínios confiáveis.".to_string())?;
    if !exists {
        let count: i64 = db
            .query_row("SELECT COUNT(*) FROM trusted_external_domains", [], |row| {
                row.get(0)
            })
            .map_err(|_| "Não foi possível ler os domínios confiáveis.".to_string())?;
        if count >= 100 {
            return Err("A lista já contém 100 domínios confiáveis.".into());
        }
    }
    db.execute(
        "INSERT OR IGNORE INTO trusted_external_domains(domain) VALUES (?1)",
        [&domain],
    )
    .map_err(|_| "Não foi possível salvar o domínio confiável.".to_string())?;
    Ok(())
}

pub(crate) fn external_domain_is_trusted(
    app: &tauri::AppHandle,
    domain: &str,
) -> Result<bool, String> {
    let domain = normalize_external_domain(domain)?;
    external_domain_is_trusted_in_database(&database(app)?, &domain)
        .map_err(|_| "Não foi possível verificar os domínios confiáveis.".into())
}

fn external_domain_is_trusted_in_database(db: &Connection, domain: &str) -> rusqlite::Result<bool> {
    db.query_row(
        "SELECT EXISTS(SELECT 1 FROM trusted_external_domains WHERE domain=?1)",
        [domain],
        |row| row.get(0),
    )
}

pub(crate) fn remember_fetch_domain(app: &tauri::AppHandle, domain: &str) -> Result<(), String> {
    insert_external_domain(app, domain)
}

#[tauri::command]
pub fn get_trusted_external_domains(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
) -> Result<Vec<String>, String> {
    super::require_main(&caller)?;
    let db = database(&app)?;
    let mut statement = db
        .prepare("SELECT domain FROM trusted_external_domains ORDER BY domain")
        .map_err(|_| "Não foi possível listar os domínios confiáveis.".to_string())?;
    let domains = statement
        .query_map([], |row| row.get(0))
        .map_err(|_| "Não foi possível listar os domínios confiáveis.".to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "Não foi possível listar os domínios confiáveis.".into());
    domains
}

#[tauri::command]
pub fn trust_external_domain(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    domain: String,
) -> Result<(), String> {
    super::require_main(&caller)?;
    insert_external_domain(&app, &domain)
}

#[tauri::command]
pub fn forget_external_domain(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    domain: String,
) -> Result<(), String> {
    super::require_main(&caller)?;
    let domain = normalize_external_domain(&domain)?;
    database(&app)?
        .execute(
            "DELETE FROM trusted_external_domains WHERE domain=?1",
            [&domain],
        )
        .map_err(|_| "Não foi possível remover o domínio confiável.".to_string())?;
    Ok(())
}

pub fn effective_mode(app: &tauri::AppHandle) -> Result<Mode, String> {
    parse_mode(&db_setting(app, "mode")?)
}

fn parse_mode(value: &str) -> Result<Mode, String> {
    match value {
        "sempre_perguntar" => Ok(Mode::SemprePerguntar),
        "aprovar_por_mim" => Ok(Mode::AprovarPorMim),
        "controle_total" => Ok(Mode::ControleTotal),
        _ => Err("Modo de permissão inválido; ações bloqueadas.".into()),
    }
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Mode::SemprePerguntar => "sempre_perguntar",
            Mode::AprovarPorMim => "aprovar_por_mim",
            Mode::ControleTotal => "controle_total",
        }
    }
}

fn overrides(app: &tauri::AppHandle) -> Result<Vec<ToolOverride>, String> {
    let db = database(app)?;
    let mut stmt = db
        .prepare("SELECT tool,more_permissive FROM tool_overrides ORDER BY tool")
        .map_err(|_| "Não foi possível ler as exceções de ferramenta.".to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ToolOverride {
                tool: row.get(0)?,
                more_permissive: row.get::<_, i64>(1)? != 0,
            })
        })
        .map_err(|_| "Não foi possível ler as exceções de ferramenta.".to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|_| "Não foi possível ler as exceções de ferramenta.".into())
}

fn override_enabled(app: &tauri::AppHandle, tool: &str) -> Result<bool, String> {
    database(app)?
        .query_row(
            "SELECT more_permissive FROM tool_overrides WHERE tool=?1",
            [tool],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map(|value| value.unwrap_or(0) != 0)
        .map_err(|_| "Não foi possível ler a exceção da ferramenta.".into())
}

#[tauri::command]
pub async fn get_permission_settings(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    state: State<'_, SecurityState>,
) -> Result<PermissionSettings, String> {
    super::require_main(&caller)?;
    let shortcut_error = state
        .shortcut_error
        .lock()
        .map_err(|_| "Estado do atalho indisponível.")?
        .clone();
    Ok(PermissionSettings {
        mode: effective_mode(&app)?,
        shortcut: db_setting(&app, "shortcut")?,
        shortcut_error,
        halted: state.is_halted(),
        approval_ttl_seconds: db_setting(&app, "approval_ttl_seconds")?
            .parse()
            .map_err(|_| "Tempo de aprovação inválido; ações bloqueadas.")?,
        overrides: overrides(&app)?,
    })
}

#[cfg(windows)]
pub(crate) async fn confirm_native(message: String) -> bool {
    tauri::async_runtime::spawn_blocking(move || native_confirm_blocking(&message))
        .await
        .unwrap_or(false)
}

#[cfg(windows)]
fn native_confirm_blocking(message: &str) -> bool {
    use std::ffi::c_void;
    #[link(name = "user32")]
    unsafe extern "system" {
        fn MessageBoxW(hwnd: *mut c_void, text: *const u16, caption: *const u16, kind: u32) -> i32;
    }
    let text: Vec<u16> = format!("{message}\0").encode_utf16().collect();
    let caption: Vec<u16> = "Confirmação do OdeKrai\0".encode_utf16().collect();
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            caption.as_ptr(),
            0x00000004 | 0x00000030 | 0x0000001000 | 0x00000100,
        ) == 6
    }
}

#[cfg(not(windows))]
pub(crate) async fn confirm_native(_: String) -> bool {
    false
}

#[tauri::command]
pub async fn set_permission_mode(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    mode: Mode,
) -> Result<(), String> {
    super::require_main(&caller)?;
    let current = effective_mode(&app)?;
    if current == mode {
        return Ok(());
    }
    if !confirm_native(format!("Alterar o modo de permissão de {} para {}? Esta confirmação ocorre em uma janela nativa do Windows.", current.label(), mode.label())).await {
        return Err("Alteração de modo cancelada pelo usuário.".into());
    }
    set_db_setting(&app, "mode", mode.label())?;
    let _ = app.emit("permission-settings-updated", ());
    Ok(())
}

#[tauri::command]
pub async fn set_tool_override(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    tool: String,
    more_permissive: bool,
) -> Result<(), String> {
    super::require_main(&caller)?;
    if !known_tool(&tool) {
        return Err("Ferramenta desconhecida.".into());
    }
    if more_permissive && !override_enabled(&app, &tool)? && !confirm_native(format!("Permitir que {tool} execute com menos confirmações que o modo global? Esta exceção nunca ignora bloqueios fixos nem conteúdo externo.")).await {
        return Err("Exceção não ativada.".into());
    }
    database(&app)?.execute("INSERT INTO tool_overrides(tool,more_permissive) VALUES (?1,?2) ON CONFLICT(tool) DO UPDATE SET more_permissive=excluded.more_permissive", params![tool, more_permissive as i64]).map_err(|_| "Não foi possível salvar a exceção.".to_string())?;
    let _ = app.emit("permission-settings-updated", ());
    Ok(())
}

fn known_tool(tool: &str) -> bool {
    [
        "workspace.read_file",
        "workspace.write_file",
        "workspace.delete_file",
        "simulated.send_message",
    ]
    .contains(&tool)
}

#[tauri::command]
pub async fn set_kill_shortcut(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    shortcut: String,
) -> Result<(), String> {
    super::require_main(&caller)?;
    if shortcut.len() > 80 || shortcut.trim() != shortcut || !shortcut.contains('+') {
        return Err("Formato de atalho inválido.".into());
    }
    let parsed = shortcut
        .parse::<tauri_plugin_global_shortcut::Shortcut>()
        .map_err(|_| "Formato de atalho inválido.".to_string())?;
    let parsed_id = parsed.id();
    let previous = db_setting(&app, "shortcut")?;
    if previous == shortcut {
        return Ok(());
    }
    let manager = app.global_shortcut();
    let _ = manager.unregister(previous.as_str());
    if let Err(error) = manager.register(parsed) {
        let _ = manager.register(previous.as_str());
        let message = format!("O atalho está em conflito ou não pôde ser registrado: {error}");
        *app.state::<SecurityState>()
            .shortcut_error
            .lock()
            .map_err(|_| "Estado do atalho indisponível.")? = Some(message.clone());
        return Err(message);
    }
    if let Err(error) = set_db_setting(&app, "shortcut", &shortcut) {
        let _ = manager.unregister(shortcut.as_str());
        let _ = manager.register(previous.as_str());
        return Err(error);
    }
    *app.state::<SecurityState>()
        .shortcut_error
        .lock()
        .map_err(|_| "Estado do atalho indisponível.")? = None;
    *app.state::<SecurityState>()
        .active_kill_shortcut_id
        .lock()
        .map_err(|_| "Estado do atalho indisponível.")? = Some(parsed_id);
    let _ = app.emit("permission-settings-updated", ());
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IslandQuickShortcutSettings {
    shortcut: String,
    enabled: bool,
    error: Option<String>,
}

#[tauri::command]
pub async fn get_island_quick_shortcut(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
) -> Result<IslandQuickShortcutSettings, String> {
    super::require_main(&caller)?;
    let state = app.state::<SecurityState>();
    let error = state
        .quick_shortcut_error
        .lock()
        .map_err(|_| "Estado do atalho indisponível.")?
        .clone();
    Ok(IslandQuickShortcutSettings {
        shortcut: db_setting(&app, "island_quick_shortcut")
            .unwrap_or_else(|_| "Ctrl+Alt+Space".into()),
        enabled: db_setting(&app, "island_quick_shortcut_enabled")
            .is_ok_and(|value| value == "true"),
        error,
    })
}

#[tauri::command]
pub async fn set_island_quick_shortcut(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    shortcut: String,
    enabled: bool,
) -> Result<(), String> {
    super::require_main(&caller)?;
    kill_switch::update_quick_shortcut(&app, &shortcut, enabled).await?;
    let _ = app.emit("island-quick-shortcut-updated", ());
    Ok(())
}

#[tauri::command]
pub async fn take_pet_quick_focus_request(
    caller: tauri::WebviewWindow,
    state: State<'_, SecurityState>,
) -> Result<bool, String> {
    super::require_caller(&caller, &["pet"])?;
    Ok(kill_switch::consume_quick_focus_request(&state))
}

#[tauri::command]
pub fn set_approval_ttl(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    seconds: u64,
) -> Result<(), String> {
    super::require_main(&caller)?;
    if !(MIN_APPROVAL_TTL..=MAX_APPROVAL_TTL).contains(&seconds) {
        return Err(format!(
            "O prazo deve ficar entre {MIN_APPROVAL_TTL} e {MAX_APPROVAL_TTL} segundos."
        ));
    }
    set_db_setting(&app, "approval_ttl_seconds", &seconds.to_string())?;
    let _ = app.emit("permission-settings-updated", ());
    Ok(())
}

#[tauri::command]
pub fn list_audit(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    state: State<'_, SecurityState>,
) -> Result<Vec<AuditLine>, String> {
    super::require_main(&caller)?;
    let _lock = state
        .audit_lock
        .lock()
        .map_err(|_| "Trava da auditoria indisponível.".to_string())?;
    let logs = agent_home::audit_logs_path(&app)?;
    let all = verify_audit_path(&logs)?;
    Ok(all.into_iter().rev().take(500).collect())
}

#[tauri::command]
pub fn verify_audit(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    state: State<'_, SecurityState>,
) -> Result<bool, String> {
    super::require_main(&caller)?;
    let _lock = state
        .audit_lock
        .lock()
        .map_err(|_| "Trava da auditoria indisponível.".to_string())?;
    verify_audit_path(&agent_home::audit_logs_path(&app)?).map(|_| true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "odekrai-security-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir.canonicalize().unwrap()
    }

    #[test]
    fn simulated_action_schema_rejects_unknown_fields_and_payment_is_fixed_block() {
        assert!(serde_json::from_str::<ActionRequest>(
            r#"{"action":{"tool":"workspace.read_file","path":"a.txt","risk":"allow"}}"#
        )
        .is_err());
        assert_eq!(
            action_metadata(&SimulatedAction::Payment {
                amount_cents: 100,
                currency: "USD".into()
            })
            .3,
            Some("fixed_block_payment")
        );
    }

    #[test]
    fn external_domains_require_first_approval_and_trust_is_exact_per_domain() {
        let db = Connection::open_in_memory().unwrap();
        db.execute(
            "CREATE TABLE trusted_external_domains(domain TEXT PRIMARY KEY)",
            [],
        )
        .unwrap();
        let first = normalize_external_domain("Example.com").unwrap();
        assert!(!external_domain_is_trusted_in_database(&db, &first).unwrap());
        db.execute(
            "INSERT INTO trusted_external_domains(domain) VALUES (?1)",
            [&first],
        )
        .unwrap();
        assert!(external_domain_is_trusted_in_database(&db, &first).unwrap());
        assert!(!external_domain_is_trusted_in_database(&db, "other.example").unwrap());
    }

    #[test]
    fn paths_reject_traversal_absolute_reserved_and_internal_directories() {
        for path in [
            "../secret",
            "C:\\Windows\\x",
            "\\\\server\\share\\x",
            "NUL.txt",
            "COM1.txt",
            "LPT³.log",
            ".history/secret",
            ".trash/item",
        ] {
            assert!(relative_path(path).is_err(), "{path}");
        }
        assert!(relative_path("docs/note.md").is_ok());
    }

    #[test]
    fn fixed_blocks_cover_credentials_payments_and_secret_markers() {
        assert_eq!(
            action_metadata(&SimulatedAction::ReadFile {
                path: ".env".into()
            })
            .3,
            Some("fixed_block_secret_path")
        );
        assert_eq!(
            action_metadata(&SimulatedAction::ReadFile {
                path: "keys/id_ed25519".into()
            })
            .3,
            Some("fixed_block_secret_path")
        );
        assert_eq!(
            action_metadata(&SimulatedAction::SendMessage {
                recipient: "x@example.invalid".into(),
                message: "password=secret".into()
            })
            .3,
            Some("fixed_block_secret_exfiltration")
        );
        assert_eq!(
            action_metadata(&SimulatedAction::Productivity {
                skill_id: "mail.draft".into(),
                params: serde_json::json!({"path":"mail-draft.txt","to":"a@example.invalid","subject":"x","body":"password=secret"}),
            })
            .3,
            Some("fixed_block_secret_exfiltration")
        );
        assert_eq!(
            action_metadata(&SimulatedAction::Payment {
                amount_cents: 1,
                currency: "USD".into()
            })
            .3,
            Some("fixed_block_payment")
        );
        assert!(contains_secret_marker("-----BEGIN PRIVATE KEY-----"));
        assert_eq!(
            action_metadata(&SimulatedAction::WriteFile {
                path: "notes.txt".into(),
                content: "password=do-not-save".into(),
            })
            .3,
            Some("fixed_block_secret_storage")
        );
    }

    #[test]
    fn fixed_path_blocks_protect_identity_logs_databases_and_configuration_for_all_tools() {
        for path in [
            "ODEKRAI.md",
            "nested/ODEKRAI.md",
            "logs/audit.jsonl",
            "config/providers.json",
            "data/profile.sqlite3",
            "cache/index.db",
            ".env.production",
        ] {
            for action in [
                SimulatedAction::ReadFile { path: path.into() },
                SimulatedAction::WriteFile {
                    path: path.into(),
                    content: "safe".into(),
                },
                SimulatedAction::DeleteFile { path: path.into() },
            ] {
                assert!(
                    action_metadata(&action).3.is_some(),
                    "fixed block missing for {path}"
                );
            }
        }
        assert!(relative_path("../outside.txt").is_err());
    }

    #[test]
    fn workspace_read_blocks_known_secret_markers_in_file_contents() {
        let root = temp_dir();
        fs::write(root.join("notes.txt"), "password=do-not-read").unwrap();
        let action = SimulatedAction::ReadFile {
            path: "notes.txt".into(),
        };
        assert!(WorkspaceRead.execute(&action, &root).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn approval_hash_and_queue_capacity_fail_closed() {
        let action = SimulatedAction::WriteFile {
            path: "a.txt".into(),
            content: "one".into(),
        };
        let expected = digest(serde_json::to_string(&action).unwrap().as_bytes());
        assert!(display_hash_matches(&action, &expected, &expected));
        assert!(!display_hash_matches(&action, &expected, "wrong"));
        let changed = SimulatedAction::WriteFile {
            path: "a.txt".into(),
            content: "two".into(),
        };
        assert!(!display_hash_matches(&changed, &expected, &expected));
        let mut queue = VecDeque::new();
        assert!(queue_has_capacity(&queue));
        for number in 0..MAX_PENDING {
            queue.push_back(PendingAction {
                id: number.to_string(),
                action: action.clone(),
                origin: Origin::User,
                risk: Risk::EscritaReversivel,
                summary: String::new(),
                preview: String::new(),
                preview_image: None,
                digest: expected.clone(),
                expires: Instant::now() + Duration::from_secs(30),
                expires_at: 0,
            });
        }
        assert!(!queue_has_capacity(&queue));
        assert_eq!(active_pending_count(&queue, Instant::now()), MAX_PENDING);
        queue.front_mut().unwrap().expires = Instant::now() - Duration::from_secs(1);
        assert_eq!(
            active_pending_count(&queue, Instant::now()),
            MAX_PENDING - 1
        );
        let consumed = take_pending_approval(&mut queue, "0", &expected).unwrap();
        assert!(take_pending_approval(&mut queue, &consumed.id, &expected).is_err());
        assert!(queue_has_capacity(&queue));
        assert!(
            queue.iter().all(|item| item.id != consumed.id),
            "an approval cannot be replayed after it is consumed"
        );
        let mut empty_queue = VecDeque::new();
        assert!(matches!(
            take_pending_approval(&mut empty_queue, "missing", &expected),
            Err("approval_unavailable")
        ));
    }

    #[test]
    fn poisoned_approval_queue_fails_closed() {
        let state = SecurityState::new(false);
        let poisoned = std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    let _guard = state
                        .pending
                        .lock()
                        .expect("test lock should start healthy");
                    panic!("poison the test mutex");
                })
                .join()
                .is_err()
        });
        assert!(poisoned);
        assert!(lock_pending(&state).is_err());
    }

    #[test]
    fn approval_expiration_uses_the_configured_deadline() {
        let now = Instant::now();
        assert!(!is_expired(now + Duration::from_secs(30), now));
        assert!(is_expired(now - Duration::from_secs(1), now));
    }

    #[test]
    fn audit_write_failure_does_not_create_an_actionable_log() {
        let root = temp_dir();
        let missing = root.join("missing").join("audit.jsonl");
        assert!(OpenOptions::new()
            .create(true)
            .append(true)
            .open(&missing)
            .is_err());
        assert!(!missing.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn simulated_workspace_tools_backup_and_trash_without_permanent_delete() {
        let root = temp_dir();
        let write = WorkspaceWrite;
        let first = SimulatedAction::WriteFile {
            path: "notes/test.txt".into(),
            content: "versão 1".into(),
        };
        assert!(write.execute(&first, &root).is_ok());
        let second = SimulatedAction::WriteFile {
            path: "notes/test.txt".into(),
            content: "versão 2".into(),
        };
        let second_result = write.execute(&second, &root);
        assert!(second_result.is_ok(), "{second_result:?}");
        assert_eq!(
            fs::read_to_string(root.join("notes/test.txt")).unwrap(),
            "versão 2"
        );
        let history = fs::read_dir(root.join(".history"))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(fs::read_to_string(history[0].path()).unwrap(), "versão 1");
        let delete = SimulatedAction::DeleteFile {
            path: "notes/test.txt".into(),
        };
        let delete_result = WorkspaceDelete.execute(&delete, &root);
        assert!(delete_result.is_ok(), "{delete_result:?}");
        assert!(!root.join("notes/test.txt").exists());
        assert_eq!(fs::read_dir(root.join(".trash")).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn simulated_external_tools_never_create_network_side_effects() {
        let send = SimulatedAction::SendMessage {
            recipient: "person@example.invalid".into(),
            message: "hello".into(),
        };
        assert!(SimulatedSend
            .execute(&send, Path::new(""))
            .unwrap()
            .contains("Nenhuma mensagem foi enviada"));
        let payment = SimulatedAction::Payment {
            amount_cents: 500,
            currency: "USD".into(),
        };
        assert!(SimulatedPayment.execute(&payment, Path::new("")).is_err());
    }

    #[test]
    fn tool_rate_bucket_limits_per_tool_without_network() {
        let state = SecurityState::new(false);
        let tool = WorkspaceRead;
        for _ in 0..RATE_LIMIT {
            assert!(allowed_rate(&state, &tool).unwrap());
        }
        assert!(!allowed_rate(&state, &tool).unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn path_resolution_rejects_symlinks() {
        use std::os::unix::fs::symlink;
        let root = temp_dir();
        let outside = temp_dir();
        fs::write(outside.join("secret"), "no").unwrap();
        symlink(&outside, root.join("linked")).unwrap();
        assert!(safe_target(&root, "linked/secret").is_err());
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(outside).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn path_resolution_rejects_windows_reparse_points() {
        use std::os::windows::fs::symlink_dir;
        let root = temp_dir();
        let outside = temp_dir();
        fs::write(outside.join("secret.txt"), "no").unwrap();
        let link = root.join("linked");
        if symlink_dir(&outside, &link).is_ok() {
            assert!(safe_target(&root, "linked/secret.txt").is_err());
        }
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(outside).unwrap();
    }

    #[test]
    fn path_resolution_rejects_hard_links() {
        let root = temp_dir();
        let source = root.join("original.txt");
        let alias = root.join("alias.txt");
        fs::write(&source, "content").unwrap();
        if fs::hard_link(&source, &alias).is_ok() {
            assert!(safe_target(&root, "alias.txt").is_err());
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn audit_hash_chain_detects_tampering_and_breaks() {
        let logs = temp_dir();
        let payload = AuditPayload {
            timestamp: 1,
            id: "a".into(),
            tool: "read".into(),
            risk: Risk::Leitura,
            mode: Mode::SemprePerguntar,
            origin: "usuário".into(),
            decision: "allow".into(),
            rule_id: "workspace_read".into(),
            approval: "not_required".into(),
            latency_ms: 0,
            params_summary: "path=x".into(),
            status: "completed".into(),
            error: None,
            previous_hash: String::new(),
        };
        let line = AuditLine {
            hash: digest(&serde_json::to_vec(&payload).unwrap()),
            payload,
        };
        fs::write(
            logs.join("1.jsonl"),
            format!("{}\n", serde_json::to_string(&line).unwrap()),
        )
        .unwrap();
        assert_eq!(verify_audit_path(&logs).unwrap().len(), 1);
        fs::write(
            logs.join("1.jsonl"),
            format!(
                "{}\n",
                serde_json::to_string(&AuditLine {
                    payload: line.payload,
                    hash: "bad".into()
                })
                .unwrap()
            ),
        )
        .unwrap();
        assert!(verify_audit_path(&logs).is_err());
        fs::remove_dir_all(logs).unwrap();
    }

    #[test]
    fn pending_parameter_hash_changes_when_action_changes() {
        let a = SimulatedAction::WriteFile {
            path: "a.txt".into(),
            content: "one".into(),
        };
        let b = SimulatedAction::WriteFile {
            path: "a.txt".into(),
            content: "two".into(),
        };
        assert_ne!(
            digest(serde_json::to_string(&a).unwrap().as_bytes()),
            digest(serde_json::to_string(&b).unwrap().as_bytes())
        );
    }
}
