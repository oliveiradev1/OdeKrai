use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use tauri::Manager;

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationProviderTarget {
    pub provider_id: String,
    pub base_url: String,
    pub model: String,
}

const PET_CATALOG: &str = include_str!("../../assets/pets/kenney-mini-characters/pets.json");
const DEFAULT_PET_ID: &str = "character-female-a";

#[derive(Serialize)]
pub struct Conversation {
    id: String,
    title: String,
    created_at: i64,
    preview: String,
    provider_id: String,
    model: String,
    agent_enabled: bool,
}

#[derive(Serialize)]
pub struct StoredMessage {
    role: String,
    content: String,
    created_at: i64,
}

#[derive(Clone, Debug, Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentTimelineEntry {
    pub label: String,
    pub status: String,
    pub duration_ms: Option<i64>,
    pub at: i64,
}

#[derive(Clone, Debug, Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentArtifact {
    pub path: String,
    pub kind: String,
}

#[derive(Clone, Debug, Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UndoDescriptor {
    pub skill_id: String,
    pub state_id: String,
    pub path: Option<String>,
    pub label: String,
}

#[derive(Clone, Debug, Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentPlanStepStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

#[derive(Clone, Debug, Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentPlanStep {
    pub description: String,
    pub status: AgentPlanStepStatus,
}

#[derive(Clone, Debug, Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentPlan {
    pub title: String,
    pub steps: Vec<AgentPlanStep>,
    #[serde(default)]
    pub outside_plan_actions: Vec<String>,
}

#[derive(Clone, Debug, Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentActionCard {
    pub id: String,
    pub skill_id: String,
    pub title: String,
    pub risk: String,
    pub decision: String,
    pub status: String,
    pub summary: String,
    pub result_summary: String,
    pub diff: Option<String>,
    #[serde(default)]
    pub preview: Option<String>,
    #[serde(default)]
    pub artifacts: Vec<AgentArtifact>,
    #[serde(default)]
    pub timeline: Vec<AgentTimelineEntry>,
    #[serde(default)]
    pub duration_ms: Option<i64>,
    #[serde(default)]
    pub approval_expires_at: Option<i64>,
    #[serde(default)]
    pub untrusted_result: bool,
    #[serde(default)]
    pub undo_descriptor: Option<UndoDescriptor>,
    #[serde(default)]
    pub plan: Option<AgentPlan>,
    pub created_at: i64,
}

pub(crate) fn save_agent_action_card(
    app: &tauri::AppHandle,
    conversation_id: &str,
    card: &AgentActionCard,
) -> Result<(), String> {
    if !valid_id(conversation_id)
        || card.id.is_empty()
        || card.id.len() > 128
        || !card
            .id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_ .".contains(&byte))
        || card.skill_id.len() > 64
        || card.title.len() > 120
        || card.risk.len() > 64
        || card.decision.len() > 40
        || card.status.len() > 80
        || card.summary.len() > 500
        || card.result_summary.len() > 500
        || card.diff.as_ref().is_some_and(|diff| diff.len() > 16_000)
        || card
            .preview
            .as_ref()
            .is_some_and(|preview| preview.len() > 32_000)
        || card.artifacts.len() > 32
        || card.timeline.len() > 128
        || card.undo_descriptor.as_ref().is_some_and(|undo| {
            undo.skill_id.len() > 64
                || undo.state_id.len() > 128
                || undo.label.len() > 160
                || undo
                    .path
                    .as_ref()
                    .is_some_and(|path| path.len() > 4096 || path.chars().any(char::is_control))
        })
        || card.plan.as_ref().is_some_and(|plan| {
            plan.title.is_empty()
                || plan.title.chars().count() > 120
                || plan.steps.is_empty()
                || plan.steps.len() > 12
                || plan.outside_plan_actions.len() > 8
                || plan.steps.iter().any(|step| {
                    step.description.is_empty() || step.description.chars().count() > 240
                })
                || plan
                    .outside_plan_actions
                    .iter()
                    .any(|action| action.is_empty() || action.chars().count() > 120)
        })
        || card.artifacts.iter().any(|artifact| {
            artifact.path.is_empty()
                || artifact.path.len() > 32_767
                || artifact.kind.len() > 40
                || artifact.path.chars().any(char::is_control)
        })
        || card.timeline.iter().any(|entry| {
            entry.label.len() > 160
                || entry.status.len() > 80
                || entry.label.chars().any(char::is_control)
                || entry.status.chars().any(char::is_control)
        })
        || card.preview.as_ref().is_some_and(|preview| {
            preview
                .chars()
                .any(|character| character.is_control() && character != '\n' && character != '\t')
        })
        || [
            &card.title,
            &card.risk,
            &card.decision,
            &card.status,
            &card.summary,
            &card.result_summary,
        ]
        .iter()
        .any(|value| value.chars().any(char::is_control))
    {
        return Err("Cartão de ação inválido.".into());
    }
    let changed = database(app)?.execute(
        "INSERT INTO agent_action_cards(conversation_id,id,skill_id,title,risk,decision,status,summary,result_summary,diff,preview,artifacts,timeline,duration_ms,approval_expires_at,untrusted_result,undo_descriptor,plan) SELECT ?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18 WHERE EXISTS(SELECT 1 FROM conversations WHERE id=?1) ON CONFLICT(conversation_id,id) DO UPDATE SET decision=excluded.decision,status=excluded.status,result_summary=excluded.result_summary,diff=excluded.diff,preview=excluded.preview,artifacts=excluded.artifacts,timeline=excluded.timeline,duration_ms=excluded.duration_ms,approval_expires_at=excluded.approval_expires_at,untrusted_result=excluded.untrusted_result,undo_descriptor=excluded.undo_descriptor,plan=excluded.plan",
        params![conversation_id,card.id,card.skill_id,card.title,card.risk,card.decision,card.status,card.summary,card.result_summary,card.diff,card.preview,serde_json::to_string(&card.artifacts).unwrap_or_else(|_| "[]".into()),serde_json::to_string(&card.timeline).unwrap_or_else(|_| "[]".into()),card.duration_ms,card.approval_expires_at,card.untrusted_result,card.undo_descriptor.as_ref().and_then(|undo| serde_json::to_string(undo).ok()),card.plan.as_ref().and_then(|plan| serde_json::to_string(plan).ok())],
    ).map_err(|_| "Não foi possível salvar o cartão da ação.".to_string())?;
    if changed == 1 {
        Ok(())
    } else {
        Err("A conversa do cartão não existe no histórico local.".into())
    }
}

pub(crate) fn clear_undo_descriptor_for_state(
    app: &tauri::AppHandle,
    state_id: &str,
) -> Result<(), String> {
    let db = database(app)?;
    db.execute(
        "UPDATE agent_action_cards SET undo_descriptor=NULL WHERE json_extract(undo_descriptor,'$.stateId')=?1",
        [state_id],
    )
    .map_err(|_| "Não foi possível atualizar o cartão da ação restaurada.".to_string())?;
    Ok(())
}

#[tauri::command]
pub fn load_agent_action_cards(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    conversation_id: String,
) -> Result<Vec<AgentActionCard>, String> {
    super::require_main(&caller)?;
    if !valid_id(&conversation_id) {
        return Err("Conversa inválida.".into());
    }
    let db = database(&app)?;
    let mut statement = db
        .prepare("SELECT id,skill_id,title,risk,decision,status,summary,result_summary,diff,created_at,preview,artifacts,timeline,duration_ms,approval_expires_at,untrusted_result,undo_descriptor,plan FROM agent_action_cards WHERE conversation_id=?1 ORDER BY created_at,id LIMIT 200")
        .map_err(|_| "Não foi possível ler as ações da conversa.".to_string())?;
    let rows = statement
        .query_map([conversation_id], |row| {
            Ok(AgentActionCard {
                id: row.get(0)?,
                skill_id: row.get(1)?,
                title: row.get(2)?,
                risk: row.get(3)?,
                decision: row.get(4)?,
                status: row.get(5)?,
                summary: row.get(6)?,
                result_summary: row.get(7)?,
                diff: row.get(8)?,
                created_at: row.get(9)?,
                preview: row.get(10)?,
                artifacts: row
                    .get::<_, String>(11)
                    .ok()
                    .and_then(|value| serde_json::from_str(&value).ok())
                    .unwrap_or_default(),
                timeline: row
                    .get::<_, String>(12)
                    .ok()
                    .and_then(|value| serde_json::from_str(&value).ok())
                    .unwrap_or_default(),
                duration_ms: row.get(13)?,
                approval_expires_at: row.get(14)?,
                untrusted_result: row.get(15).unwrap_or(false),
                undo_descriptor: row
                    .get::<_, Option<String>>(16)
                    .ok()
                    .flatten()
                    .and_then(|value| serde_json::from_str(&value).ok()),
                plan: row
                    .get::<_, Option<String>>(17)
                    .ok()
                    .flatten()
                    .and_then(|value| serde_json::from_str(&value).ok()),
            })
        })
        .map_err(|_| "Não foi possível ler as ações da conversa.".to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|_| "Não foi possível ler as ações da conversa.".into())
}

fn database(app: &tauri::AppHandle) -> Result<Connection, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|_| "Não foi possível localizar os dados locais.".to_string())?;
    std::fs::create_dir_all(&dir)
        .map_err(|_| "Não foi possível preparar os dados locais.".to_string())?;
    let db = Connection::open(dir.join("conversations.sqlite3"))
        .map_err(|_| "Não foi possível abrir o histórico local.".to_string())?;
    init_schema(&db)?;
    Ok(db)
}

fn init_schema(db: &Connection) -> Result<(), String> {
    db.execute_batch("PRAGMA foreign_keys = ON; PRAGMA secure_delete = ON;
        CREATE TABLE IF NOT EXISTS conversations (id TEXT PRIMARY KEY, title TEXT NOT NULL, created_at INTEGER NOT NULL DEFAULT (unixepoch()), provider_id TEXT NOT NULL DEFAULT '', model TEXT NOT NULL DEFAULT '', agent_enabled INTEGER NOT NULL DEFAULT 0 CHECK(agent_enabled IN (0,1)), provider_targets TEXT NOT NULL DEFAULT '[]');
        CREATE TABLE IF NOT EXISTS messages (id INTEGER PRIMARY KEY, conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE, role TEXT NOT NULL CHECK(role IN ('user','assistant')), content TEXT NOT NULL, created_at INTEGER NOT NULL DEFAULT (unixepoch()));
        CREATE TABLE IF NOT EXISTS agent_messages (id INTEGER PRIMARY KEY, conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE, role TEXT NOT NULL CHECK(role IN ('tool_call','tool_result')), skill_id TEXT NOT NULL, audit_id TEXT, content TEXT NOT NULL, created_at INTEGER NOT NULL DEFAULT (unixepoch()));
        CREATE TABLE IF NOT EXISTS agent_action_cards (conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE, id TEXT NOT NULL, skill_id TEXT NOT NULL, title TEXT NOT NULL, risk TEXT NOT NULL, decision TEXT NOT NULL, status TEXT NOT NULL, summary TEXT NOT NULL, result_summary TEXT NOT NULL DEFAULT '', diff TEXT, created_at INTEGER NOT NULL DEFAULT (unixepoch()), preview TEXT, artifacts TEXT NOT NULL DEFAULT '[]', timeline TEXT NOT NULL DEFAULT '[]', duration_ms INTEGER, approval_expires_at INTEGER, untrusted_result INTEGER NOT NULL DEFAULT 0, undo_descriptor TEXT, plan TEXT, PRIMARY KEY(conversation_id,id));
        CREATE INDEX IF NOT EXISTS messages_conversation ON messages(conversation_id, id);
        CREATE TABLE IF NOT EXISTS pet_preferences (key TEXT PRIMARY KEY, value TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS provider_tool_support (
            provider_id TEXT NOT NULL,
            model TEXT NOT NULL,
            tools_supported INTEGER NOT NULL CHECK(tools_supported IN (0, 1)),
            PRIMARY KEY(provider_id, model)
        );
        CREATE TABLE IF NOT EXISTS provider_usage_daily (
            provider_id TEXT NOT NULL,
            utc_day INTEGER NOT NULL,
            input_tokens INTEGER NOT NULL CHECK(input_tokens >= 0),
            output_tokens INTEGER NOT NULL CHECK(output_tokens >= 0),
            PRIMARY KEY(provider_id, utc_day)
        );")
        .map_err(|_| "Não foi possível preparar o histórico local.".to_string())?;
    for (column, definition) in [
        ("provider_id", "TEXT NOT NULL DEFAULT ''"),
        ("model", "TEXT NOT NULL DEFAULT ''"),
        (
            "agent_enabled",
            "INTEGER NOT NULL DEFAULT 0 CHECK(agent_enabled IN (0,1))",
        ),
        ("provider_targets", "TEXT NOT NULL DEFAULT '[]'"),
    ] {
        let exists: bool = db
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM pragma_table_info('conversations') WHERE name = ?1)",
                [column],
                |row| row.get(0),
            )
            .map_err(|_| "Não foi possível verificar o histórico local.".to_string())?;
        if !exists {
            db.execute_batch(&format!(
                "ALTER TABLE conversations ADD COLUMN {column} {definition}"
            ))
            .map_err(|_| "Não foi possível atualizar o histórico local.".to_string())?;
        }
    }
    for (column, definition) in [
        ("preview", "TEXT"),
        ("artifacts", "TEXT NOT NULL DEFAULT '[]'"),
        ("timeline", "TEXT NOT NULL DEFAULT '[]'"),
        ("duration_ms", "INTEGER"),
        ("approval_expires_at", "INTEGER"),
        ("untrusted_result", "INTEGER NOT NULL DEFAULT 0"),
        ("undo_descriptor", "TEXT"),
        ("plan", "TEXT"),
    ] {
        let exists: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM pragma_table_info('agent_action_cards') WHERE name = ?1)", [column], |row| row.get(0)).map_err(|_| "Não foi possível verificar os cartões de ação.".to_string())?;
        if !exists {
            db.execute_batch(&format!(
                "ALTER TABLE agent_action_cards ADD COLUMN {column} {definition}"
            ))
            .map_err(|_| "Não foi possível atualizar os cartões de ação.".to_string())?;
        }
    }
    Ok(())
}

type AgentContext = (
    String,
    String,
    bool,
    Vec<ConversationProviderTarget>,
    Vec<crate::providers::Message>,
);

pub(crate) fn load_agent_context(
    app: &tauri::AppHandle,
    conversation_id: &str,
) -> Result<AgentContext, String> {
    if !valid_id(conversation_id) {
        return Err("Identificador de conversa inválido.".into());
    }
    let db = database(app)?;
    let (provider_id, model, enabled, targets_json): (String, String, bool, String) = db.query_row(
        "SELECT provider_id, model, agent_enabled, provider_targets FROM conversations WHERE id=?1",
        [conversation_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    ).map_err(|_| "A conversa não existe no histórico local.".to_string())?;
    if provider_id.is_empty() || model.is_empty() {
        return Err(
            "Modo agente indisponível: selecione um provedor e modelo em Conectores de IA.".into(),
        );
    }
    let mut targets = serde_json::from_str::<Vec<ConversationProviderTarget>>(&targets_json)
        .map_err(|_| "As configurações de provedores da conversa estão inválidas.".to_string())?;
    if targets.is_empty() {
        targets.push(ConversationProviderTarget {
            provider_id: provider_id.clone(),
            base_url: String::new(),
            model: model.clone(),
        });
    }
    if targets.len() > 16 {
        return Err("A lista de provedores da conversa excede o limite permitido.".into());
    }
    let mut stmt = db
        .prepare("SELECT role,content FROM messages WHERE conversation_id=?1 ORDER BY id LIMIT 100")
        .map_err(|_| "Não foi possível carregar a conversa para o agente.".to_string())?;
    let rows = stmt
        .query_map([conversation_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|_| "Não foi possível carregar a conversa para o agente.".to_string())?;
    let mut messages = Vec::new();
    for row in rows {
        let (role, content) =
            row.map_err(|_| "Não foi possível carregar a conversa para o agente.".to_string())?;
        messages.push(crate::providers::Message {
            role,
            blocks: vec![crate::providers::Block::Text { text: content }],
        });
    }
    if messages.is_empty() {
        return Err("Modo agente indisponível: a conversa ainda não contém uma mensagem.".into());
    }
    Ok((provider_id, model, enabled, targets, messages))
}

pub(crate) fn save_agent_user_message(
    app: &tauri::AppHandle,
    conversation_id: &str,
    message: &str,
) -> Result<(), String> {
    if !valid_id(conversation_id) || message.trim().is_empty() || message.len() > 1_000_000 {
        return Err("Mensagem inválida para o agente.".into());
    }
    let db = database(app)?;
    let conversation_exists: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM conversations WHERE id=?1)",
            [conversation_id],
            |row| row.get(0),
        )
        .map_err(|_| "Não foi possível validar a conversa.".to_string())?;
    if !conversation_exists {
        return Err("A conversa não existe no histórico local.".into());
    }
    let latest: Option<(String, String)> = db
        .query_row(
            "SELECT role,content FROM messages WHERE conversation_id=?1 ORDER BY id DESC LIMIT 1",
            [conversation_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|_| "Não foi possível verificar a conversa.".to_string())?;
    if latest
        .as_ref()
        .is_some_and(|(role, content)| role == "user" && content == message)
    {
        return Ok(());
    }
    db.execute(
        "INSERT INTO messages(conversation_id,role,content) VALUES (?1,'user',?2)",
        params![conversation_id, message],
    )
    .map_err(|_| "Não foi possível salvar a mensagem do agente.".to_string())?;
    Ok(())
}

pub(crate) fn save_agent_final(
    app: &tauri::AppHandle,
    conversation_id: &str,
    content: &str,
) -> Result<(), String> {
    if content.trim().is_empty() || content.len() > 1_000_000 {
        return Err("A resposta final excede o limite permitido.".into());
    }
    database(app)?.execute(
        "INSERT INTO messages(conversation_id,role,content) SELECT ?1,'assistant',?2 WHERE EXISTS(SELECT 1 FROM conversations WHERE id=?1)",
        params![conversation_id,content],
    ).map_err(|_| "Não foi possível salvar a resposta do agente.".to_string()).and_then(|changed| {
        if changed == 1 { Ok(()) } else { Err("A conversa não existe no histórico local.".into()) }
    })
}

pub(crate) fn agent_mode(app: &tauri::AppHandle, conversation_id: &str) -> Result<bool, String> {
    database(app)?
        .query_row(
            "SELECT agent_enabled FROM conversations WHERE id=?1",
            [conversation_id],
            |row| row.get(0),
        )
        .map_err(|_| "A conversa não existe no histórico local.".to_string())
}

pub(crate) fn set_agent_mode(
    app: &tauri::AppHandle,
    conversation_id: &str,
    enabled: bool,
) -> Result<(), String> {
    let changed = database(app)?
        .execute(
            "UPDATE conversations SET agent_enabled=?2 WHERE id=?1",
            params![conversation_id, enabled],
        )
        .map_err(|_| "Não foi possível salvar o modo agente.".to_string())?;
    if changed == 1 {
        Ok(())
    } else {
        Err("A conversa não existe no histórico local.".into())
    }
}

#[tauri::command]
pub fn get_conversation_agent_mode(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    conversation_id: String,
) -> Result<bool, String> {
    crate::require_main(&caller)?;
    agent_mode(&app, &conversation_id)
}

#[tauri::command]
pub fn set_conversation_agent_mode(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    conversation_id: String,
    enabled: bool,
) -> Result<(), String> {
    crate::require_main(&caller)?;
    if enabled {
        let (provider_id, model, _, _, _) = load_agent_context(&app, &conversation_id)?;
        if crate::agent_home::system_prompt(&app)?.is_none() {
            return Err(
                "Modo agente indisponível: configure a pasta e a identidade aprovada do agente."
                    .into(),
            );
        }
        if crate::security::load_halted(&app) {
            return Err("Modo agente indisponível: a parada de emergência está ativa.".into());
        }
        if !crate::providers::agent_provider_ready(&app, &provider_id, &model) {
            return Err(
                "Modo agente indisponível: conecte um provedor com ferramentas verificadas.".into(),
            );
        }
        if !crate::security::skills::registry()
            .iter()
            .any(|skill| crate::security::skills::is_skill_enabled(&app, skill.id).unwrap_or(false))
        {
            return Err("Modo agente indisponível: ative uma habilidade.".into());
        }
    }
    set_agent_mode(&app, &conversation_id, enabled)
}

pub(crate) fn persist_agent_record(
    app: &tauri::AppHandle,
    conversation_id: &str,
    role: &str,
    skill_id: &str,
    audit_id: Option<&str>,
    safe_record: &str,
) -> Result<(), String> {
    if !valid_id(conversation_id)
        || !matches!(role, "tool_call" | "tool_result")
        || skill_id.is_empty()
        || skill_id.len() > 64
        || safe_record.is_empty()
        || safe_record.len() > 2_048
        || safe_record.chars().any(char::is_control)
    {
        return Err("Registro de execução inválido.".into());
    }
    database(app)?.execute(
        "INSERT INTO agent_messages(conversation_id,role,skill_id,audit_id,content) SELECT ?1,?2,?3,?4,?5 WHERE EXISTS(SELECT 1 FROM conversations WHERE id=?1)",
        params![conversation_id, role, skill_id, audit_id, safe_record],
    ).map_err(|_| "Não foi possível registrar a etapa do agente.".to_string()).and_then(|changed| {
        if changed == 1 { Ok(()) } else { Err("A conversa não existe no histórico local.".into()) }
    })
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderUsageSummary {
    pub provider_id: String,
    pub today_input: u64,
    pub today_output: u64,
    pub seven_day_input: u64,
    pub seven_day_output: u64,
}

fn utc_day() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
        / 86_400
}

pub(crate) fn daily_token_limit_exceeded(used: u64, limit: u64) -> bool {
    limit > 0 && used > limit
}

fn record_provider_usage_db(
    db: &Connection,
    provider_id: &str,
    day: i64,
    input: u64,
    output: u64,
) -> Result<u64, String> {
    if provider_id.is_empty() || provider_id.len() > 80 || provider_id.chars().any(char::is_control)
    {
        return Err("O identificador do provedor é inválido.".into());
    }
    let input = i64::try_from(input)
        .map_err(|_| "O volume de tokens informado excede o limite suportado.".to_string())?;
    let output = i64::try_from(output)
        .map_err(|_| "O volume de tokens informado excede o limite suportado.".to_string())?;
    db.execute(
        "INSERT INTO provider_usage_daily(provider_id, utc_day, input_tokens, output_tokens)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(provider_id, utc_day) DO UPDATE SET
           input_tokens=input_tokens+excluded.input_tokens,
           output_tokens=output_tokens+excluded.output_tokens",
        params![provider_id, day, input, output],
    )
    .map_err(|_| "Não foi possível salvar o uso de tokens localmente.".to_string())?;
    db.query_row(
        "SELECT COALESCE(SUM(input_tokens + output_tokens), 0)
         FROM provider_usage_daily WHERE utc_day=?1",
        [day],
        |row| row.get::<_, i64>(0),
    )
    .map(|total| total.max(0) as u64)
    .map_err(|_| "Não foi possível calcular o uso diário de tokens.".to_string())
}

pub(crate) fn record_provider_usage(
    app: &tauri::AppHandle,
    provider_id: &str,
    input: u64,
    output: u64,
) -> Result<u64, String> {
    record_provider_usage_db(&database(app)?, provider_id, utc_day(), input, output)
}

pub(crate) fn provider_usage_total_today(app: &tauri::AppHandle) -> Result<u64, String> {
    let db = database(app)?;
    db.query_row(
        "SELECT COALESCE(SUM(input_tokens + output_tokens), 0)
         FROM provider_usage_daily WHERE utc_day=?1",
        [utc_day()],
        |row| row.get::<_, i64>(0),
    )
    .map(|total| total.max(0) as u64)
    .map_err(|_| "Não foi possível calcular o uso diário de tokens.".to_string())
}

fn provider_usage_db(db: &Connection, today: i64) -> Result<Vec<ProviderUsageSummary>, String> {
    let mut statement = db
        .prepare(
            "SELECT provider_id,
               COALESCE(SUM(CASE WHEN utc_day=?1 THEN input_tokens ELSE 0 END), 0),
               COALESCE(SUM(CASE WHEN utc_day=?1 THEN output_tokens ELSE 0 END), 0),
               COALESCE(SUM(CASE WHEN utc_day>=?2 THEN input_tokens ELSE 0 END), 0),
               COALESCE(SUM(CASE WHEN utc_day>=?2 THEN output_tokens ELSE 0 END), 0)
             FROM provider_usage_daily WHERE utc_day>=?2 GROUP BY provider_id ORDER BY provider_id",
        )
        .map_err(|_| "Não foi possível consultar o uso de tokens.".to_string())?;
    let rows = statement
        .query_map(params![today, today - 6], |row| {
            Ok(ProviderUsageSummary {
                provider_id: row.get(0)?,
                today_input: row.get::<_, i64>(1)?.max(0) as u64,
                today_output: row.get::<_, i64>(2)?.max(0) as u64,
                seven_day_input: row.get::<_, i64>(3)?.max(0) as u64,
                seven_day_output: row.get::<_, i64>(4)?.max(0) as u64,
            })
        })
        .map_err(|_| "Não foi possível consultar o uso de tokens.".to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|_| "Não foi possível consultar o uso de tokens.".to_string())
}

#[tauri::command]
pub fn get_provider_usage(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
) -> Result<Vec<ProviderUsageSummary>, String> {
    super::require_main(&caller)?;
    provider_usage_db(&database(&app)?, utc_day())
}

pub(crate) fn cached_tool_support(
    app: &tauri::AppHandle,
    provider_id: &str,
    model: &str,
) -> Result<Option<bool>, String> {
    validate_provider_model(provider_id, model)?;
    cached_tool_support_db(&database(app)?, provider_id, model)
}

pub(crate) fn cache_tools_unsupported(
    app: &tauri::AppHandle,
    provider_id: &str,
    model: &str,
) -> Result<(), String> {
    validate_provider_model(provider_id, model)?;
    cache_tool_support_db(&database(app)?, provider_id, model, false)
}

pub(crate) fn cache_tool_opt_in(
    app: &tauri::AppHandle,
    provider_id: &str,
    model: &str,
    enabled: bool,
) -> Result<(), String> {
    validate_provider_model(provider_id, model)?;
    cache_tool_support_db(&database(app)?, provider_id, model, enabled)
}

fn validate_provider_model(provider_id: &str, model: &str) -> Result<(), String> {
    if provider_id.is_empty()
        || provider_id.len() > 80
        || model.trim().is_empty()
        || model.len() > 200
        || provider_id.chars().any(char::is_control)
        || model.chars().any(char::is_control)
    {
        return Err("O provedor ou modelo informado é inválido.".into());
    }
    Ok(())
}

fn cached_tool_support_db(
    db: &Connection,
    provider_id: &str,
    model: &str,
) -> Result<Option<bool>, String> {
    db.query_row(
        "SELECT tools_supported FROM provider_tool_support WHERE provider_id=?1 AND model=?2",
        params![provider_id, model],
        |row| row.get::<_, bool>(0),
    )
    .optional()
    .map_err(|_| "Não foi possível consultar as capacidades locais do modelo.".to_string())
}

fn cache_tool_support_db(
    db: &Connection,
    provider_id: &str,
    model: &str,
    supported: bool,
) -> Result<(), String> {
    db.execute(
        "INSERT INTO provider_tool_support(provider_id, model, tools_supported) VALUES (?1, ?2, ?3)
         ON CONFLICT(provider_id, model) DO UPDATE SET tools_supported=excluded.tools_supported",
        params![provider_id, model, supported],
    )
    .map_err(|_| "Não foi possível atualizar as capacidades locais do modelo.".to_string())?;
    Ok(())
}

#[derive(serde::Deserialize)]
struct PetCatalogEntry {
    id: String,
    file: String,
}

fn valid_pet_catalog(json: &str) -> Result<Vec<PetCatalogEntry>, String> {
    let pets: Vec<PetCatalogEntry> = serde_json::from_str(json)
        .map_err(|_| "O catálogo de personagens está ausente ou inválido.".to_string())?;
    if pets.is_empty()
        || pets.iter().any(|pet| {
            pet.id.is_empty()
                || !pet
                    .id
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
                || !pet.file.strip_suffix(".png").is_some_and(|stem| {
                    !stem.is_empty()
                        && stem.bytes().all(|byte| {
                            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'
                        })
                })
        })
    {
        return Err("O catálogo contém um caminho de personagem inválido.".into());
    }
    Ok(pets)
}

fn pet_id_is_valid(id: &str, manifest: &str) -> bool {
    valid_pet_catalog(manifest).is_ok_and(|pets| pets.iter().any(|pet| pet.id == id))
}

fn selected_pet_from_db(db: &Connection, manifest: &str) -> Result<String, String> {
    let saved: Option<String> = db
        .query_row(
            "SELECT value FROM pet_preferences WHERE key = 'selected'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| "N\u{00e3}o foi poss\u{00ed}vel ler o personagem selecionado.".to_string())?;
    if let Some(id) = saved {
        if pet_id_is_valid(&id, manifest) {
            return Ok(id);
        }
        db.execute(
            "INSERT INTO pet_preferences(key, value) VALUES ('selected', ?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [DEFAULT_PET_ID],
        )
        .map_err(|_| "N\u{00e3}o foi poss\u{00ed}vel atualizar o personagem selecionado.".to_string())?;
    }
    Ok(DEFAULT_PET_ID.to_string())
}

#[tauri::command]
pub async fn get_selected_pet(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
) -> Result<String, String> {
    super::require_caller(&caller, &["main", "pet"])?;
    let db = database(&app)?;
    selected_pet_from_db(&db, PET_CATALOG)
}

fn pet_mode_from_db(db: &Connection) -> Result<String, String> {
    let saved = db
        .query_row(
            "SELECT value FROM pet_preferences WHERE key='mode'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|_| "Não foi possível ler o modo do pet.".to_string())?;
    Ok(saved
        .filter(|mode| mode == "island" || mode == "free")
        .unwrap_or_else(|| "island".into()))
}

#[tauri::command]
pub async fn get_pet_mode(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
) -> Result<String, String> {
    super::require_caller(&caller, &["main", "pet"])?;
    pet_mode_from_db(&database(&app)?)
}

#[tauri::command]
pub async fn set_pet_mode(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    mode: String,
) -> Result<(), String> {
    super::require_main(&caller)?;
    if mode != "island" && mode != "free" {
        return Err("Modo do pet inválido.".into());
    }
    let db = database(&app)?;
    db.execute("INSERT INTO pet_preferences(key,value) VALUES ('mode',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [mode]).map_err(|_| "Não foi possível salvar o modo do pet.".to_string())?;
    Ok(())
}
#[tauri::command]
pub fn set_selected_pet(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    pet_id: String,
) -> Result<(), String> {
    super::require_main(&caller)?;
    if !pet_id_is_valid(&pet_id, PET_CATALOG) {
        return Err("Personagem inválido.".into());
    }
    let db = database(&app)?;
    db.execute("INSERT INTO pet_preferences(key, value) VALUES ('selected', ?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [pet_id])
        .map_err(|_| "Não foi possível salvar o personagem selecionado.".to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn list_conversations(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
) -> Result<Vec<Conversation>, String> {
    super::require_main(&caller)?;
    let db = database(&app)?;
    read_conversations(&db)
}

fn read_conversations(db: &Connection) -> Result<Vec<Conversation>, String> {
    let mut stmt = db
        .prepare(
            "SELECT c.id, c.title, c.created_at, COALESCE((SELECT substr(content, 1, 240) FROM messages WHERE conversation_id = c.id ORDER BY id DESC LIMIT 1), ''), c.provider_id, c.model, c.agent_enabled FROM conversations c ORDER BY COALESCE((SELECT MAX(id) FROM messages WHERE conversation_id = c.id), 0) DESC, c.created_at DESC, c.rowid DESC LIMIT 100",
        )
        .map_err(|_| "Não foi possível ler o histórico local.".to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(Conversation {
                id: row.get(0)?,
                title: row.get(1)?,
                created_at: row.get(2)?,
                preview: row.get(3)?,
                provider_id: row.get(4)?,
                model: row.get(5)?,
                agent_enabled: row.get(6)?,
            })
        })
        .map_err(|_| "Não foi possível ler o histórico local.".to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|_| "Não foi possível ler o histórico local.".to_string())
}

#[tauri::command]
pub fn load_conversation(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    conversation_id: String,
) -> Result<Vec<StoredMessage>, String> {
    super::require_main(&caller)?;
    let db = database(&app)?;
    read_messages(&db, &conversation_id)
}

fn read_messages(db: &Connection, conversation_id: &str) -> Result<Vec<StoredMessage>, String> {
    let mut stmt = db
        .prepare(
            "SELECT role, content, created_at FROM messages WHERE conversation_id = ?1 ORDER BY id LIMIT 100",
        )
        .map_err(|_| "Não foi possível ler a conversa.".to_string())?;
    let rows = stmt
        .query_map([conversation_id], |row| {
            Ok(StoredMessage {
                role: row.get(0)?,
                content: row.get(1)?,
                created_at: row.get(2)?,
            })
        })
        .map_err(|_| "Não foi possível ler a conversa.".to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|_| "Não foi possível ler a conversa.".to_string())
}

#[tauri::command]
#[allow(clippy::too_many_arguments)] // Assinatura consumida pelo comando Tauri existente.
pub fn append_conversation_message(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    conversation_id: String,
    role: String,
    content: String,
    provider_id: Option<String>,
    model: Option<String>,
    provider_targets: Option<Vec<ConversationProviderTarget>>,
) -> Result<(), String> {
    super::require_main(&caller)?;
    if conversation_id.is_empty()
        || conversation_id.len() > 80
        || !conversation_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        || !matches!(role.as_str(), "user" | "assistant")
        || content.trim().is_empty()
        || content.len() > 1_000_000
    {
        return Err("Mensagem inválida para o histórico.".into());
    }
    if provider_id
        .as_ref()
        .is_some_and(|id| id.is_empty() || id.len() > 80 || id.chars().any(char::is_control))
        || model
            .as_ref()
            .is_some_and(|name| name.len() > 200 || name.chars().any(char::is_control))
        || provider_targets.as_ref().is_some_and(|targets| {
            targets.len() > 16
                || targets.iter().any(|target| {
                    target.provider_id.is_empty()
                        || target.provider_id.len() > 80
                        || target.base_url.len() > 2_000
                        || target.model.is_empty()
                        || target.model.len() > 200
                        || target.provider_id.chars().any(char::is_control)
                        || target.base_url.chars().any(char::is_control)
                        || target.model.chars().any(char::is_control)
                })
        })
    {
        return Err("O provedor ou modelo da conversa é inválido.".into());
    }
    let mut db = database(&app)?;
    let tx = db
        .transaction()
        .map_err(|_| "Não foi possível salvar a conversa.".to_string())?;
    if role == "user" {
        let title: String = content.chars().take(80).collect();
        let created = tx
            .execute(
                "INSERT OR IGNORE INTO conversations (id, title) VALUES (?1, ?2)",
                params![conversation_id, title],
            )
            .map_err(|_| "Não foi possível salvar a conversa.".to_string())?;
        if created == 1
            && provider_id
                .as_deref()
                .zip(model.as_deref())
                .is_some_and(|(provider, model)| {
                    !crate::security::load_halted(&app)
                        && crate::security::skills::registry().iter().any(|skill| {
                            crate::security::skills::is_skill_enabled(&app, skill.id)
                                .unwrap_or(false)
                        })
                        && crate::providers::verified_tool_support(&app, provider, model)
                        && crate::agent_home::system_prompt(&app)
                            .is_ok_and(|prompt| prompt.is_some())
                })
        {
            tx.execute(
                "UPDATE conversations SET agent_enabled=1 WHERE id=?1",
                [&conversation_id],
            )
            .map_err(|_| "Não foi possível ativar o modo agente disponível.".to_string())?;
        }
    }
    if provider_id.is_some() || model.is_some() {
        tx.execute(
            "UPDATE conversations SET provider_id = COALESCE(?2, provider_id), model = COALESCE(?3, model) WHERE id = ?1",
            params![conversation_id, provider_id, model],
        )
        .map_err(|_| "Não foi possível salvar o modelo da conversa.".to_string())?;
    }
    if let Some(targets) = provider_targets {
        let serialized = serde_json::to_string(&targets)
            .map_err(|_| "Não foi possível salvar os provedores da conversa.".to_string())?;
        tx.execute(
            "UPDATE conversations SET provider_targets=?2 WHERE id=?1",
            params![conversation_id, serialized],
        )
        .map_err(|_| "Não foi possível salvar os provedores da conversa.".to_string())?;
    }
    let changed = tx.execute("INSERT INTO messages (conversation_id, role, content) SELECT id, ?2, ?3 FROM conversations WHERE id = ?1", params![conversation_id, role, content])
        .map_err(|_| "Não foi possível salvar a conversa.".to_string())?;
    if changed != 1 {
        return Err("A conversa não foi encontrada.".into());
    }
    tx.commit()
        .map_err(|_| "Não foi possível salvar a conversa.".to_string())
}

#[tauri::command]
pub fn delete_conversation(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    conversation_id: String,
) -> Result<(), String> {
    super::require_main(&caller)?;
    if !valid_id(&conversation_id) {
        return Err("Identificador de conversa inválido.".into());
    }
    let db = database(&app)?;
    db.execute("DELETE FROM conversations WHERE id = ?1", [conversation_id])
        .map_err(|_| "Não foi possível apagar a conversa.".to_string())?;
    Ok(())
}

#[tauri::command]
pub fn rename_conversation(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    conversation_id: String,
    title: String,
) -> Result<(), String> {
    super::require_main(&caller)?;
    let title = title.trim();
    if !valid_id(&conversation_id) || !valid_title(title) {
        return Err("O título da conversa é inválido.".into());
    }
    let db = database(&app)?;
    let changed = db
        .execute(
            "UPDATE conversations SET title = ?2 WHERE id = ?1",
            params![conversation_id, title],
        )
        .map_err(|_| "Não foi possível renomear a conversa.".to_string())?;
    if changed != 1 {
        return Err("A conversa não foi encontrada.".into());
    }
    Ok(())
}

#[tauri::command]
pub fn delete_all_conversations(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
) -> Result<(), String> {
    super::require_main(&caller)?;
    let db = database(&app)?;
    db.execute("DELETE FROM conversations", [])
        .map_err(|_| "Não foi possível apagar o histórico.".to_string())?;
    db.execute_batch("VACUUM")
        .map_err(|_| "Não foi possível concluir a limpeza do histórico.".to_string())
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 80 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

fn valid_title(title: &str) -> bool {
    !title.trim().is_empty() && title.chars().count() <= 120
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_message_schema_migrates_legacy_conversations_safely() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE conversations (id TEXT PRIMARY KEY, title TEXT NOT NULL, created_at INTEGER NOT NULL DEFAULT (unixepoch()));
             CREATE TABLE messages (id INTEGER PRIMARY KEY, conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE, role TEXT NOT NULL CHECK(role IN ('user','assistant')), content TEXT NOT NULL, created_at INTEGER NOT NULL DEFAULT (unixepoch()));
             INSERT INTO conversations(id,title) VALUES ('existing','Conversa existente');
             INSERT INTO messages(conversation_id,role,content) VALUES ('existing','user','Olá');",
        ).unwrap();
        init_schema(&db).unwrap();
        let columns = ["provider_id", "model", "agent_enabled", "provider_targets"];
        for column in columns {
            let exists: bool = db
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM pragma_table_info('conversations') WHERE name=?1)",
                    [column],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(exists, "coluna ausente após migração: {column}");
        }
        let legacy_message: String = db
            .query_row(
                "SELECT content FROM messages WHERE conversation_id='existing'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(legacy_message, "Olá");
        db.execute("INSERT INTO agent_messages(conversation_id,role,skill_id,content) VALUES ('existing','tool_call','workspace.read_file','requested')", []).unwrap();
        let action_cards_exist: bool = db
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='agent_action_cards')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(action_cards_exist);
    }

    #[test]
    fn legacy_agent_cards_deserialize_and_card_columns_migrate_without_data_loss() {
        let legacy_json = r#"{"id":"call-legacy","skillId":"files.read","title":"Ler arquivo","risk":"leitura","decision":"automática","status":"Concluída","summary":"Resumo","resultSummary":"Texto","diff":null,"createdAt":10}"#;
        let legacy: AgentActionCard = serde_json::from_str(legacy_json).unwrap();
        assert!(legacy.timeline.is_empty());
        assert!(legacy.artifacts.is_empty());
        assert!(legacy.undo_descriptor.is_none());
        assert!(legacy.plan.is_none());
        assert_eq!(legacy.status, "Concluída");

        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE conversations (id TEXT PRIMARY KEY, title TEXT NOT NULL, created_at INTEGER NOT NULL DEFAULT (unixepoch())); CREATE TABLE agent_action_cards (conversation_id TEXT NOT NULL, id TEXT NOT NULL, skill_id TEXT NOT NULL, title TEXT NOT NULL, risk TEXT NOT NULL, decision TEXT NOT NULL, status TEXT NOT NULL, summary TEXT NOT NULL, result_summary TEXT NOT NULL DEFAULT '', diff TEXT, created_at INTEGER NOT NULL DEFAULT (unixepoch()), PRIMARY KEY(conversation_id,id)); INSERT INTO conversations(id,title) VALUES ('keep','Histórico'); INSERT INTO agent_action_cards(conversation_id,id,skill_id,title,risk,decision,status,summary,result_summary) VALUES ('keep','call-legacy','files.read','Ler arquivo','leitura','automática','Concluída','Resumo','Texto');").unwrap();
        init_schema(&db).unwrap();
        let (status, artifacts, timeline, undo, plan): (String, String, String, Option<String>, Option<String>) = db.query_row("SELECT status,artifacts,timeline,undo_descriptor,plan FROM agent_action_cards WHERE id='call-legacy'", [], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?))).unwrap();
        assert_eq!(status, "Concluída");
        assert_eq!(artifacts, "[]");
        assert_eq!(timeline, "[]");
        assert!(undo.is_none());
        assert!(plan.is_none());
        let has_undo_column: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM pragma_table_info('agent_action_cards') WHERE name='undo_descriptor')", [], |row| row.get(0)).unwrap();
        assert!(has_undo_column);
        let has_plan_column: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM pragma_table_info('agent_action_cards') WHERE name='plan')", [], |row| row.get(0)).unwrap();
        assert!(has_plan_column);
    }

    #[test]
    fn schema_and_foreign_keys_are_valid() {
        let db = Connection::open_in_memory().unwrap();
        init_schema(&db).unwrap();
        assert!(db.execute("INSERT INTO messages (conversation_id, role, content) VALUES ('missing','user','olá')", []).is_err());
    }

    #[test]
    fn tool_support_cache_is_scoped_to_provider_and_model() {
        let db = Connection::open_in_memory().unwrap();
        init_schema(&db).unwrap();
        assert_eq!(
            cached_tool_support_db(&db, "openai", "model-a").unwrap(),
            None
        );
        cache_tool_support_db(&db, "openai", "model-a", false).unwrap();
        assert_eq!(
            cached_tool_support_db(&db, "openai", "model-a").unwrap(),
            Some(false)
        );
        assert_eq!(
            cached_tool_support_db(&db, "openai", "model-b").unwrap(),
            None
        );
        assert_eq!(
            cached_tool_support_db(&db, "anthropic", "model-a").unwrap(),
            None
        );
    }

    #[test]
    fn provider_usage_accumulates_and_reports_today_and_seven_day_totals() {
        let db = Connection::open_in_memory().unwrap();
        init_schema(&db).unwrap();
        assert_eq!(
            record_provider_usage_db(&db, "openai", 100, 10, 5).unwrap(),
            15
        );
        assert_eq!(
            record_provider_usage_db(&db, "openai", 100, 7, 3).unwrap(),
            25
        );
        record_provider_usage_db(&db, "anthropic", 94, 20, 4).unwrap();
        record_provider_usage_db(&db, "openai", 93, 100, 100).unwrap();

        let summary = provider_usage_db(&db, 100).unwrap();
        assert_eq!(summary.len(), 2);
        assert_eq!(summary[0].provider_id, "anthropic");
        assert_eq!(summary[0].today_input, 0);
        assert_eq!(summary[0].seven_day_input, 20);
        assert_eq!(summary[1].provider_id, "openai");
        assert_eq!(summary[1].today_input, 17);
        assert_eq!(summary[1].today_output, 8);
        assert_eq!(summary[1].seven_day_input, 17);
        assert_eq!(summary[1].seven_day_output, 8);
        assert!(!daily_token_limit_exceeded(25, 25));
        assert!(daily_token_limit_exceeded(26, 25));
        assert!(!daily_token_limit_exceeded(1, 0));
    }

    #[test]
    fn usage_table_is_added_to_an_existing_database_without_dropping_history() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE conversations (id TEXT PRIMARY KEY, title TEXT NOT NULL, created_at INTEGER NOT NULL DEFAULT (unixepoch()));
             CREATE TABLE messages (id INTEGER PRIMARY KEY, conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE, role TEXT NOT NULL CHECK(role IN ('user','assistant')), content TEXT NOT NULL, created_at INTEGER NOT NULL DEFAULT (unixepoch()));
             INSERT INTO conversations (id, title) VALUES ('keep', 'Preservada');",
        )
        .unwrap();
        init_schema(&db).unwrap();
        let title: String = db
            .query_row(
                "SELECT title FROM conversations WHERE id='keep'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(title, "Preservada");
        assert!(record_provider_usage_db(&db, "openai", 100, 1, 1).is_ok());
    }

    #[test]
    fn deleting_conversation_cascades_messages() {
        let db = Connection::open_in_memory().unwrap();
        init_schema(&db).unwrap();
        db.execute(
            "INSERT INTO conversations (id, title) VALUES ('one', 'Um')",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO messages (conversation_id, role, content) VALUES ('one', 'user', 'Olá')",
            [],
        )
        .unwrap();
        db.execute("DELETE FROM conversations WHERE id = 'one'", [])
            .unwrap();
        let count: i64 = db
            .query_row("SELECT count(*) FROM messages", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn conversation_list_includes_title_and_creation_time() {
        let db = Connection::open_in_memory().unwrap();
        init_schema(&db).unwrap();
        db.execute(
            "INSERT INTO conversations (id, title, created_at) VALUES ('one', 'Teste', 1798000000)",
            [],
        )
        .unwrap();
        let items = read_conversations(&db).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Teste");
        assert_eq!(items[0].created_at, 1798000000);
        assert_eq!(items[0].preview, "");
        assert_eq!(items[0].provider_id, "");
        assert_eq!(items[0].model, "");
    }

    #[test]
    fn conversation_list_sorts_by_latest_message_activity() {
        let db = Connection::open_in_memory().unwrap();
        init_schema(&db).unwrap();
        db.execute_batch(
            "INSERT INTO conversations (id, title, created_at) VALUES
                ('older-active', 'Mais ativa', 10),
                ('newer-idle', 'Mais nova', 20);
             INSERT INTO messages (conversation_id, role, content, created_at)
                VALUES ('older-active', 'user', 'Retomada recente', 30);",
        )
        .unwrap();

        let items = read_conversations(&db).unwrap();
        assert_eq!(items[0].id, "older-active");
        assert_eq!(items[1].id, "newer-idle");
    }

    #[test]
    fn existing_conversation_schema_migrates_and_lists_provider_model() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE conversations (id TEXT PRIMARY KEY, title TEXT NOT NULL, created_at INTEGER NOT NULL DEFAULT (unixepoch()));
             INSERT INTO conversations(id, title) VALUES ('old', 'Anterior');",
        )
        .unwrap();
        init_schema(&db).unwrap();
        db.execute(
            "UPDATE conversations SET provider_id = ?2, model = ?3 WHERE id = ?1",
            params!["old", "openai", "gpt-test"],
        )
        .unwrap();

        let items = read_conversations(&db).unwrap();
        assert_eq!(items[0].provider_id, "openai");
        assert_eq!(items[0].model, "gpt-test");
    }

    #[test]
    fn loading_selected_conversation_returns_its_messages_and_timestamps() {
        let db = Connection::open_in_memory().unwrap();
        init_schema(&db).unwrap();
        db.execute(
            "INSERT INTO conversations (id, title) VALUES ('one', 'Uma conversa')",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO messages (conversation_id, role, content, created_at) VALUES ('one', 'user', 'Minha mensagem', 1798000000)",
            [],
        )
        .unwrap();
        let items = read_conversations(&db).unwrap();
        assert_eq!(items[0].preview, "Minha mensagem");
        let messages = read_messages(&db, "one").unwrap();
        assert_eq!(messages[0].content, "Minha mensagem");
        assert_eq!(messages[0].created_at, 1798000000);
    }

    #[test]
    fn conversation_title_can_be_renamed_without_changing_its_messages() {
        let db = Connection::open_in_memory().unwrap();
        init_schema(&db).unwrap();
        db.execute(
            "INSERT INTO conversations (id, title) VALUES ('one', 'Original')",
            [],
        )
        .unwrap();
        db.execute(
            "UPDATE conversations SET title = ?2 WHERE id = ?1",
            params!["one", "Título novo"],
        )
        .unwrap();
        let items = read_conversations(&db).unwrap();
        assert_eq!(items[0].title, "Título novo");
    }

    #[test]
    fn conversation_titles_reject_empty_or_oversized_values() {
        assert!(!valid_title("  "));
        assert!(!valid_title(&"a".repeat(121)));
        assert!(valid_title(&"a".repeat(120)));
    }

    #[test]
    fn pet_catalog_rejects_missing_json_and_unsafe_paths() {
        assert!(valid_pet_catalog("").is_err());
        assert!(valid_pet_catalog(r#"[{"id":"bad","file":"../secret.png"}]"#).is_err());
        assert!(valid_pet_catalog(r#"[{"id":"bad","file":"C:secret.png"}]"#).is_err());
        assert!(valid_pet_catalog(PET_CATALOG).is_ok());
    }

    #[test]
    fn selected_pet_defaults_and_persists_in_sqlite() {
        let db = Connection::open_in_memory().unwrap();
        init_schema(&db).unwrap();
        assert_eq!(
            selected_pet_from_db(&db, PET_CATALOG).unwrap(),
            DEFAULT_PET_ID
        );
        db.execute(
            "INSERT INTO pet_preferences(key,value) VALUES ('selected','character-male-a')",
            [],
        )
        .unwrap();
        assert_eq!(
            selected_pet_from_db(&db, PET_CATALOG).unwrap(),
            "character-male-a"
        );
        assert!(!pet_id_is_valid("../secret", PET_CATALOG));
    }

    #[test]
    fn pet_mode_defaults_and_persists_in_sqlite() {
        let db = Connection::open_in_memory().unwrap();
        init_schema(&db).unwrap();
        assert_eq!(pet_mode_from_db(&db).unwrap(), "island");
        db.execute(
            "INSERT INTO pet_preferences(key,value) VALUES ('mode','free')",
            [],
        )
        .unwrap();
        assert_eq!(pet_mode_from_db(&db).unwrap(), "free");
        db.execute(
            "UPDATE pet_preferences SET value='invalid' WHERE key='mode'",
            [],
        )
        .unwrap();
        assert_eq!(pet_mode_from_db(&db).unwrap(), "island");
    }

    #[test]
    fn selected_pet_migrates_unknown_saved_id_to_default() {
        let db = Connection::open_in_memory().unwrap();
        init_schema(&db).unwrap();
        db.execute(
            "INSERT INTO pet_preferences(key,value) VALUES ('selected','character-no-longer-in-catalog')",
            [],
        )
        .unwrap();

        assert_eq!(
            selected_pet_from_db(&db, PET_CATALOG).unwrap(),
            DEFAULT_PET_ID
        );
        let stored: String = db
            .query_row(
                "SELECT value FROM pet_preferences WHERE key='selected'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stored, DEFAULT_PET_ID);
    }
}
