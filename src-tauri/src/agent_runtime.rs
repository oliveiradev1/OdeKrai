use crate::{
    agent::{self, AgentBackend, AgentEvent, AgentLimits, ToolExecution},
    policy::Origin,
    providers::{Block, Message, ProviderReply, ToolCall, ToolDef},
    security::{self, skills::Skill, SecurityState},
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tauri::{ipc::Channel, State};
use tokio_util::sync::CancellationToken;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentAvailability {
    available: bool,
    reason: Option<String>,
    workspace_path: Option<String>,
}

#[derive(Clone, Debug, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentLimitsConfig {
    pub max_turns: usize,
    pub max_duration_seconds: u64,
    pub max_tokens: u64,
    pub max_external_actions: usize,
}

impl Default for AgentLimitsConfig {
    fn default() -> Self {
        Self {
            max_turns: 8,
            max_duration_seconds: 120,
            max_tokens: 16_000,
            max_external_actions: 2,
        }
    }
}

impl AgentLimitsConfig {
    fn validate(&self) -> Result<(), String> {
        if !(1..=32).contains(&self.max_turns)
            || !(10..=600).contains(&self.max_duration_seconds)
            || !(128..=200_000).contains(&self.max_tokens)
            || self.max_external_actions > 20
        {
            return Err("Os limites do agente estão fora da faixa permitida.".into());
        }
        Ok(())
    }

    fn runtime_limits(&self) -> AgentLimits {
        AgentLimits {
            max_turns: self.max_turns,
            max_duration: std::time::Duration::from_secs(self.max_duration_seconds),
            max_tokens: self.max_tokens,
            max_external_actions: self.max_external_actions,
            repeated_call_limit: 3,
        }
    }
}

#[tauri::command]
pub fn get_agent_limits(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
) -> Result<AgentLimitsConfig, String> {
    crate::require_main(&caller)?;
    get_agent_limits_inner(&app)
}

fn get_agent_limits_inner(app: &tauri::AppHandle) -> Result<AgentLimitsConfig, String> {
    let read = |key: &str| -> Result<u64, String> {
        crate::security::db_setting(app, key)?
            .parse()
            .map_err(|_| "Os limites salvos do agente são inválidos.".into())
    };
    let value = AgentLimitsConfig {
        max_turns: read("agent_max_turns")? as usize,
        max_duration_seconds: read("agent_max_duration_seconds")?,
        max_tokens: read("agent_max_tokens")?,
        max_external_actions: read("agent_max_external_actions")? as usize,
    };
    value.validate()?;
    Ok(value)
}

#[tauri::command]
pub fn set_agent_limits(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    limits: AgentLimitsConfig,
) -> Result<(), String> {
    crate::require_main(&caller)?;
    limits.validate()?;
    for (key, value) in [
        ("agent_max_turns", limits.max_turns as u64),
        ("agent_max_duration_seconds", limits.max_duration_seconds),
        ("agent_max_tokens", limits.max_tokens),
        (
            "agent_max_external_actions",
            limits.max_external_actions as u64,
        ),
    ] {
        crate::security::set_db_setting(&app, key, &value.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn agent_availability(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    state: State<'_, SecurityState>,
    provider_id: String,
    model: String,
) -> Result<AgentAvailability, String> {
    crate::require_main(&caller)?;
    if state.is_halted() {
        return Ok(AgentAvailability {
            available: false,
            reason: Some("A parada de emergência está ativa.".into()),
            workspace_path: None,
        });
    }
    let workspace = match crate::agent_home::workspace_path(&app) {
        Ok(path) => path,
        Err(_) => {
            return Ok(AgentAvailability {
                available: false,
                reason: Some("Configure a pasta de trabalho do agente.".into()),
                workspace_path: None,
            });
        }
    };
    if crate::agent_home::system_prompt(&app)?.is_none() {
        return Ok(AgentAvailability {
            available: false,
            reason: Some("Configure a pasta e a identidade do agente.".into()),
            workspace_path: None,
        });
    }
    if let Some(reason) =
        crate::providers::agent_provider_unavailable_reason(&app, &provider_id, &model)
    {
        return Ok(AgentAvailability {
            available: false,
            reason: Some(reason.into()),
            workspace_path: Some(workspace.to_string_lossy().into_owned()),
        });
    }
    if !security::skills::skill_turn_offer(&app)?.has_response_context() {
        return Ok(AgentAvailability {
            available: false,
            reason: Some(
                "Ative uma habilidade disponível e conclua os requisitos da integração em Configurações → Habilidades.".into(),
            ),
            workspace_path: Some(workspace.to_string_lossy().into_owned()),
        });
    }
    Ok(AgentAvailability {
        available: true,
        reason: None,
        workspace_path: Some(workspace.to_string_lossy().into_owned()),
    })
}

struct RuntimeBackend<'a> {
    app: tauri::AppHandle,
    security: &'a SecurityState,
    conversation_id: String,
    targets: Vec<crate::history::ConversationProviderTarget>,
    active_skills: Vec<Skill>,
    events: Channel<AgentEvent>,
    daily_token_limit: u64,
    plan_card: Option<crate::history::AgentActionCard>,
}

fn start_plan_step(plan: &mut crate::history::AgentPlan, action_label: &str) -> Option<usize> {
    if let Some(index) = plan
        .steps
        .iter()
        .position(|step| step.status == crate::history::AgentPlanStepStatus::Pending)
    {
        plan.steps[index].status = crate::history::AgentPlanStepStatus::InProgress;
        Some(index)
    } else {
        if plan.outside_plan_actions.len() < 8 {
            plan.outside_plan_actions
                .push(action_label.chars().take(120).collect());
        }
        None
    }
}

fn finish_plan_step(plan: &mut crate::history::AgentPlan, index: usize, succeeded: bool) -> bool {
    let Some(step) = plan.steps.get_mut(index) else {
        return false;
    };
    if step.status != crate::history::AgentPlanStepStatus::InProgress {
        return false;
    }
    step.status = if succeeded {
        crate::history::AgentPlanStepStatus::Completed
    } else {
        crate::history::AgentPlanStepStatus::Failed
    };
    true
}

impl RuntimeBackend<'_> {
    fn begin_plan_action(&mut self, action_label: &str) -> Result<Option<usize>, String> {
        let Some(card) = self.plan_card.as_mut() else {
            return Ok(None);
        };
        let Some(plan) = card.plan.as_mut() else {
            return Ok(None);
        };
        let index = start_plan_step(plan, action_label);
        let snapshot = card.clone();
        crate::history::save_agent_action_card(&self.app, &self.conversation_id, &snapshot)?;
        let _ = self.events.send(AgentEvent::ActionCard {
            card: Box::new(snapshot),
        });
        Ok(index)
    }

    fn finish_plan_action(&mut self, index: usize, succeeded: bool) -> Result<(), String> {
        let Some(card) = self.plan_card.as_mut() else {
            return Ok(());
        };
        let Some(plan) = card.plan.as_mut() else {
            return Ok(());
        };
        if !finish_plan_step(plan, index, succeeded) {
            return Ok(());
        }
        let snapshot = card.clone();
        crate::history::save_agent_action_card(&self.app, &self.conversation_id, &snapshot)?;
        let _ = self.events.send(AgentEvent::ActionCard {
            card: Box::new(snapshot),
        });
        Ok(())
    }
}

impl AgentBackend for RuntimeBackend<'_> {
    fn model_turn<'a>(
        &'a mut self,
        messages: &'a [Message],
        tools: &'a [ToolDef],
        _: &'a CancellationToken,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderReply, String>> + Send + 'a>> {
        Box::pin(async move {
            crate::providers::agent_model_turn(
                &self.app,
                self.security,
                self.targets.clone(),
                messages,
                tools,
                self.daily_token_limit,
            )
            .await
        })
    }

    fn execute_tool<'a>(
        &'a mut self,
        call: &'a ToolCall,
        tainted: bool,
        cancel: &'a CancellationToken,
    ) -> Pin<Box<dyn Future<Output = Result<ToolExecution, String>> + Send + 'a>> {
        Box::pin(async move {
            let skill = self
                .active_skills
                .iter()
                .find(|skill| skill.id == call.name)
                .cloned()
                .ok_or_else(|| "A habilidade não está ativa; chamada recusada.".to_string())?;
            let conversation_id = self.conversation_id.clone();
            let plan_step_index = if skill.id == "plan.propose" {
                None
            } else {
                self.begin_plan_action(skill.name)?
            };
            let args_hash = format!("{:x}", Sha256::digest(call.args.to_string().as_bytes()));
            crate::history::persist_agent_record(
                &self.app,
                &conversation_id,
                "tool_call",
                skill.id,
                None,
                &format!("arguments_sha256={args_hash}"),
            )?;
            let risk = skill.risk(&call.args)?;
            let risk = match risk {
                crate::policy::Risk::Leitura => "leitura",
                crate::policy::Risk::EscritaReversivel => "escrita reversível",
                crate::policy::Risk::ComunicacaoExterna => "comunicação externa",
                crate::policy::Risk::DestrutivoFinanceiro => "destrutivo ou financeiro",
                crate::policy::Risk::ExecucaoDeCodigo => "execução de código (alto)",
                crate::policy::Risk::CapturaSensivel => "captura sensível",
            };
            let summary = action_card_summary(&skill, &call.args);
            let untrusted_result = skill.id == "code.delegate"
                || security::skills::result_is_untrusted(&self.app, skill.id, &call.args)?;
            let action_started = std::time::Instant::now();
            let mut card = crate::history::AgentActionCard {
                id: call.id.clone(),
                skill_id: skill.id.into(),
                title: skill.name.into(),
                risk: risk.into(),
                decision: "automática".into(),
                status: "Planejada".into(),
                summary,
                result_summary: String::new(),
                diff: None,
                preview: None,
                artifacts: Vec::new(),
                timeline: vec![crate::history::AgentTimelineEntry {
                    label: "Etapa do agente".into(),
                    status: "planejada".into(),
                    duration_ms: None,
                    at: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64,
                }],
                duration_ms: None,
                approval_expires_at: None,
                untrusted_result,
                undo_descriptor: None,
                plan: None,
                created_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64,
            };
            crate::history::save_agent_action_card(&self.app, &conversation_id, &card)?;
            let _ = self.events.send(AgentEvent::ActionCard {
                card: Box::new(card.clone()),
            });
            card.status = "Executando".into();
            card.timeline.push(crate::history::AgentTimelineEntry {
                label: format!("Chamada de ferramenta: {}", skill.name),
                status: "executando".into(),
                duration_ms: None,
                at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64,
            });
            crate::history::save_agent_action_card(&self.app, &conversation_id, &card)?;
            let _ = self.events.send(AgentEvent::ActionCard {
                card: Box::new(card.clone()),
            });
            let approved_by_user = Arc::new(AtomicBool::new(false));
            let approval_state = approved_by_user.clone();
            let channel = self.events.clone();
            let app = self.app.clone();
            let card_conversation = conversation_id.clone();
            let approval_card = card.clone();
            let stream_app = self.app.clone();
            let stream_events = self.events.clone();
            let stream_conversation = conversation_id.clone();
            let mut stream_card = card.clone();
            let mut on_output = move |chunk: String| {
                let cleaned: String = chunk
                    .chars()
                    .map(|character| {
                        if character.is_control() {
                            ' '
                        } else {
                            character
                        }
                    })
                    .collect();
                stream_card.result_summary.push_str(&cleaned);
                if stream_card.result_summary.len() > 480 {
                    let mut end = 480;
                    while !stream_card.result_summary.is_char_boundary(end) {
                        end -= 1;
                    }
                    stream_card.result_summary.truncate(end);
                }
                let _ = crate::history::save_agent_action_card(
                    &stream_app,
                    &stream_conversation,
                    &stream_card,
                );
                let _ = stream_events.send(AgentEvent::ActionCard {
                    card: Box::new(stream_card.clone()),
                });
            };
            let mut on_approval = move |approval_id: String, preview: String, expires_at: u64| {
                approval_state.store(true, Ordering::SeqCst);
                let mut waiting = approval_card.clone();
                waiting.decision = "aguardando".into();
                waiting.status = "Aguardando aprovação".into();
                let mut bounded_preview = preview;
                if bounded_preview.len() > 32_000 {
                    let mut boundary = 32_000;
                    while !bounded_preview.is_char_boundary(boundary) {
                        boundary -= 1;
                    }
                    bounded_preview.truncate(boundary);
                }
                waiting.preview = Some(bounded_preview);
                waiting.approval_expires_at = Some(expires_at.min(i64::MAX as u64) as i64);
                waiting.timeline.push(crate::history::AgentTimelineEntry {
                    label: "Aguardando aprovação na janela nativa".into(),
                    status: "aguardando aprovação".into(),
                    duration_ms: None,
                    at: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64,
                });
                let _ = crate::history::save_agent_action_card(&app, &card_conversation, &waiting);
                let _ = channel.send(AgentEvent::ActionCard {
                    card: Box::new(waiting.clone()),
                });
                let _ = channel.send(AgentEvent::AwaitingApproval {
                    id: approval_id.clone(),
                    audit_id: approval_id,
                    preview: waiting.preview.clone().unwrap_or_default(),
                    expires_at,
                });
            };
            let result = skill
                .execute(
                    self.app.clone(),
                    self.security,
                    call.args.clone(),
                    Origin::Model,
                    tainted,
                    cancel,
                    &mut on_approval,
                    &mut on_output,
                )
                .await;
            let result = match result {
                Ok(result) => result,
                Err(_) => {
                    if let Some(index) = plan_step_index {
                        self.finish_plan_action(index, false)?;
                    }
                    card.decision = if approved_by_user.load(Ordering::SeqCst) {
                        "aprovada".into()
                    } else {
                        "automática".into()
                    };
                    card.status = "Falhou".into();
                    card.duration_ms =
                        Some(action_started.elapsed().as_millis().min(i64::MAX as u128) as i64);
                    card.timeline.push(crate::history::AgentTimelineEntry {
                        label: format!("Resultado da ferramenta: {} falhou", skill.name),
                        status: "erro".into(),
                        duration_ms: card.duration_ms,
                        at: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs() as i64,
                    });
                    card.result_summary =
                        "A ferramenta falhou; nenhum resultado foi confirmado.".into();
                    crate::history::save_agent_action_card(&self.app, &conversation_id, &card)?;
                    let _ = self.events.send(AgentEvent::ActionCard {
                        card: Box::new(card),
                    });
                    return Err("A ferramenta falhou durante a execução.".into());
                }
            };
            let status = result.status.clone();
            card.undo_descriptor = result.undo_descriptor.clone();
            if skill.id == "plan.propose" && status == "completed" {
                if let Some(plan) = result
                    .result
                    .as_deref()
                    .and_then(|text| serde_json::from_str::<crate::history::AgentPlan>(text).ok())
                {
                    card.title = format!("Plano: {}", plan.title);
                    card.summary =
                        "Plano informativo. Cada ação continua sujeita às permissões locais."
                            .into();
                    card.result_summary = "Este plano não executa nem autoriza ações.".into();
                    card.plan = Some(plan);
                }
            }
            if status == "completed" {
                let artifact_path = match skill.id {
                    "files.write"
                    | "files.append"
                    | "doc.create_markdown"
                    | "doc.create_text"
                    | "sheet.create_csv"
                    | "workspace.write_file" => {
                        call.args.get("path").and_then(serde_json::Value::as_str)
                    }
                    "screen.capture" => call.args.get("path").and_then(serde_json::Value::as_str),
                    "calendar.create_event" => {
                        call.args.get("path").and_then(serde_json::Value::as_str)
                    }
                    "files.copy" | "files.move" | "files.rename" => call
                        .args
                        .get("destination")
                        .and_then(serde_json::Value::as_str),
                    _ => None,
                };
                if let Some(path) =
                    artifact_path.filter(|path| !path.is_empty() && path.len() <= 32_767)
                {
                    card.artifacts = vec![crate::history::AgentArtifact {
                        path: path.to_owned(),
                        kind: if skill.id.starts_with("doc.") {
                            "documento".into()
                        } else if skill.id == "sheet.create_csv" {
                            "planilha".into()
                        } else if skill.id == "calendar.create_event" {
                            "evento de calendário".into()
                        } else if skill.id == "screen.capture" {
                            "captura de tela".into()
                        } else {
                            "arquivo".into()
                        },
                    }];
                }
            }
            card.decision = if status == "configuration_pending" {
                "automática".into()
            } else if approved_by_user.load(Ordering::SeqCst) {
                if result.message.contains("expir") {
                    "expirada".into()
                } else if status == "completed" || status == "failed" {
                    "aprovada".into()
                } else {
                    "negada".into()
                }
            } else if status == "denied" {
                "negada".into()
            } else {
                "automática".into()
            };
            card.status = match status.as_str() {
                "completed" => "Concluída",
                "configuration_pending" => "Configuração pendente",
                "denied" if result.message.contains("expir") => "Expirada",
                "denied" => "Negada",
                "failed" => "Falhou",
                _ => "Interrompida",
            }
            .into();
            card.duration_ms =
                Some(action_started.elapsed().as_millis().min(i64::MAX as u128) as i64);
            card.timeline.push(crate::history::AgentTimelineEntry {
                label: format!("Resultado da ferramenta: {}", card.status.to_lowercase()),
                status: card.status.to_lowercase(),
                duration_ms: card.duration_ms,
                at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64,
            });
            card.result_summary = match status.as_str() {
                "completed" => "Ação concluída; o resultado segue na resposta do OdeKrai.".into(),
                "configuration_pending" => "Falta cadastrar um webhook do Discord.".into(),
                "denied" if result.message.contains("expir") => {
                    "O prazo de aprovação terminou.".into()
                }
                "denied" => "A ação foi negada ou bloqueada pelas regras locais.".into(),
                "failed" => "A ação aprovada não pôde ser concluída.".into(),
                _ => "A execução foi interrompida.".into(),
            };
            if status == "completed" && skill.id == "calendar.create_event" {
                let field = |name: &str| -> String {
                    call.args
                        .get(name)
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("Não informado")
                        .chars()
                        .map(|character| {
                            if character.is_control() {
                                ' '
                            } else {
                                character
                            }
                        })
                        .take(100)
                        .collect()
                };
                card.result_summary = format!(
                    "Evento: {}\nData: {}\nLocal: {}\nGravado em: {}",
                    field("title"),
                    field("start"),
                    field("location"),
                    field("path")
                );
            }
            if status == "completed" && skill.id == "files.delete" {
                card.result_summary =
                    "Arquivo enviado à Lixeira do Windows; restaure pela Lixeira do Windows."
                        .into();
            }
            if let Some(index) = plan_step_index {
                self.finish_plan_action(index, status == "completed")?;
            }
            if skill.id == "plan.propose" && card.plan.is_some() {
                self.plan_card = Some(card.clone());
            }
            crate::history::save_agent_action_card(&self.app, &conversation_id, &card)?;
            if skill.id == "code.delegate" && status == "completed" {
                if let Some(result) = result.result.as_deref() {
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(result) {
                        if value.get("applied") == Some(&serde_json::Value::Bool(false)) {
                            card.diff = value
                                .get("diff")
                                .and_then(serde_json::Value::as_str)
                                .map(str::to_owned);
                            card.result_summary = "Diff pronto para revisão; nenhuma alteração foi aplicada ao projeto original.".into();
                        }
                    }
                }
            }
            crate::history::save_agent_action_card(&self.app, &conversation_id, &card)?;
            let _ = self.events.send(AgentEvent::ActionCard {
                card: Box::new(card.clone()),
            });
            let result_text = result.result.as_deref().unwrap_or(&result.message);
            let result_hash = format!("{:x}", Sha256::digest(result_text.as_bytes()));
            crate::history::persist_agent_record(
                &self.app,
                &conversation_id,
                "tool_result",
                skill.id,
                Some(&result.id),
                &format!("status={status}; result_sha256={result_hash}"),
            )?;
            Ok(ToolExecution {
                content: result.result.unwrap_or(result.message),
                audit_id: Some(result.id),
                denied: status == "denied" || status == "configuration_pending",
                external_communication: skill.id == "simulated.send_message",
                untrusted: untrusted_result || status != "completed",
            })
        })
    }
}

fn action_card_summary(skill: &Skill, params: &serde_json::Value) -> String {
    let mut summary = skill.description.to_string();
    if skill.id == "code.delegate" {
        if let Some(project) = params.get("project").and_then(serde_json::Value::as_str) {
            summary.push_str(&format!(" Projeto: {project}."));
        }
        if let Some(task) = params.get("task").and_then(serde_json::Value::as_str) {
            let task: String = task
                .chars()
                .map(|character| {
                    if character.is_control() {
                        ' '
                    } else {
                        character
                    }
                })
                .collect();
            summary.push_str(&format!(" Tarefa: {task}"));
        }
    }
    if let Some(path) = params.get("path").and_then(serde_json::Value::as_str) {
        if let Some(name) = std::path::Path::new(path)
            .file_name()
            .and_then(|name| name.to_str())
        {
            if !name.is_empty() && name.len() <= 160 {
                summary.push_str(&format!(" Arquivo: {name}."));
            }
        }
    }
    summary.chars().take(480).collect()
}

// Tauri derives these parameters from the frontend command payload.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn agent_run(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    state: State<'_, SecurityState>,
    conversation_id: String,
    message: String,
    daily_token_limit: u64,
    on_event: Channel<AgentEvent>,
) -> Result<String, String> {
    crate::require_main(&caller)?;
    let error_emitted = Arc::new(AtomicBool::new(false));
    let result = agent_run_inner(AgentRunContext {
        app: app.clone(),
        state: &state,
        conversation_id,
        message,
        daily_token_limit,
        on_event: on_event.clone(),
        error_emitted: error_emitted.clone(),
    })
    .await;
    match result {
        Ok(answer) => Ok(answer),
        Err(error) => {
            if error_emitted.load(Ordering::SeqCst) || error == "Execu\u{00e7}\u{00e3}o parada." {
                return Err(error);
            }
            let (kind, message, technical_details) = classify_agent_error(&error);
            crate::diagnostics::log_agent_error(&app, kind, technical_details);
            let _ = on_event.send(AgentEvent::Error {
                kind: kind.into(),
                message: message.into(),
                technical_details: Some(technical_details.into()),
            });
            Err(message.into())
        }
    }
}

struct AgentRunContext<'a> {
    app: tauri::AppHandle,
    state: &'a SecurityState,
    conversation_id: String,
    message: String,
    daily_token_limit: u64,
    on_event: Channel<AgentEvent>,
    error_emitted: Arc<AtomicBool>,
}

async fn agent_run_inner(context: AgentRunContext<'_>) -> Result<String, String> {
    let AgentRunContext {
        app,
        state,
        conversation_id,
        message,
        daily_token_limit,
        on_event,
        error_emitted,
    } = context;
    if message.trim().is_empty() || message.len() > 1_000_000 {
        return Err("Informe uma mensagem válida para o agente.".into());
    }
    if state.is_halted() {
        return Err("O agente está parado pelo kill switch.".into());
    }
    let (provider_id, model, enabled, targets, _) =
        crate::history::load_agent_context(&app, &conversation_id)?;
    if !enabled {
        return Err("Modo agente indisponível: ele está desativado nesta conversa. Ative-o nas configurações da conversa.".into());
    }
    if crate::agent_home::system_prompt(&app)?.is_none() {
        return Err(
            "Modo agente indisponível: configure a pasta e a identidade aprovada do agente.".into(),
        );
    }
    crate::history::save_agent_user_message(&app, &conversation_id, &message)?;
    let mut messages = crate::history::load_agent_context(&app, &conversation_id)?.4;
    let skill_offer = security::skills::skill_turn_offer(&app)?;
    if !skill_offer.has_response_context() {
        return Err(
            "Modo agente indisponível: ative uma habilidade disponível e conclua os requisitos da integração em Configurações → Habilidades.".into(),
        );
    }
    let active_skills = skill_offer.offered_skills();
    if let Some(reason) =
        crate::providers::agent_provider_unavailable_reason(&app, &provider_id, &model)
    {
        return Err(reason.into());
    }
    let prompt = crate::agent_home::system_prompt(&app)?
        .ok_or("Configure a pasta e a identidade aprovada do agente.")?;
    messages.insert(
        0,
        Message {
            role: "system".into(),
            blocks: vec![Block::Text {
                text: format!(
                    "{prompt}\n\nData e hora locais no momento da execução: {}. Use este relógio e o fuso do sistema para interpretar datas relativas. Nunca presuma duração de eventos: pergunte ao usuário se o fim não foi informado.\n\n{}",
                    crate::calendar::current_local_time(),
                    skill_offer.system_context()
                ),
            }],
        },
    );
    let tools = skill_offer.model_tools();
    let token = CancellationToken::new();
    {
        let mut runs = state.agent_runs.lock().await;
        if runs.contains_key(&conversation_id) {
            return Err("Já existe uma execução do agente nesta conversa.".into());
        }
        runs.insert(conversation_id.clone(), token.clone());
    }
    let mut backend = RuntimeBackend {
        app: app.clone(),
        security: state,
        conversation_id: conversation_id.clone(),
        targets,
        active_skills,
        events: on_event.clone(),
        daily_token_limit,
        plan_card: None,
    };
    let result = agent::run_agent(
        &mut backend,
        &mut messages,
        &tools,
        get_agent_limits_inner(&app)?.runtime_limits(),
        &token,
        |event| {
            if let AgentEvent::Error {
                ref kind,
                ref technical_details,
                ..
            } = event
            {
                error_emitted.store(true, Ordering::SeqCst);
                crate::diagnostics::log_agent_error(
                    &app,
                    kind,
                    technical_details
                        .as_deref()
                        .unwrap_or("Falha classificada sem detalhe técnico."),
                );
            }
            let _ = on_event.send(event);
        },
    )
    .await;
    state.agent_runs.lock().await.remove(&conversation_id);
    match result {
        Ok(answer) => {
            crate::history::save_agent_final(&app, &conversation_id, &answer)?;
            Ok(answer)
        }
        Err(error) => Err(error),
    }
}

fn classify_agent_error(error: &str) -> (&'static str, &'static str, &'static str) {
    let normalized = error.to_ascii_lowercase();
    if normalized.contains("ferramenta falhou") {
        return (
            "tool_execution",
            "A ferramenta não concluiu a ação. Confira a pasta e as permissões antes de tentar novamente.",
            "Etapa: execução da ferramenta; resultado não confirmado.",
        );
    }
    if normalized.contains("pasta configurada foi movida")
        || normalized.contains("configure a pasta")
        || normalized.contains("pasta e a identidade")
    {
        return (
            "agent_home",
            "A pasta do OdeKrai está ausente ou incompleta. Resolva o caminho em Identidade e memória.",
            "Verificação local da pasta pessoal ou da identidade falhou.",
        );
    }
    if normalized.contains("habilidade") && normalized.contains("ative") {
        return (
            "skills_disabled",
            "Ative ao menos uma habilidade em Configurações → Habilidades.",
            "Nenhuma habilidade elegível está ativa.",
        );
    }
    if normalized.contains("tempo limite") {
        return (
            "timeout",
            "O provedor demorou para responder. Tente novamente ou use outro provedor conectado.",
            "A solicitação excedeu o tempo limite configurado.",
        );
    }
    if normalized.contains("limite de tokens desta execução") {
        return (
            "execution_token_limit",
            "A execução atingiu o limite de tokens configurado. Reduza o pedido ou ajuste os limites do agente.",
            "Bloqueio local: limite de tokens por execução.",
        );
    }
    let failure = crate::agent::classify_provider_error(error);
    (failure.kind, failure.message, failure.technical_details)
}

#[cfg(test)]
mod limits_tests {
    use super::*;

    #[test]
    fn configurable_agent_limits_are_bounded() {
        let defaults = AgentLimitsConfig::default();
        assert!(defaults.validate().is_ok());
        assert!(AgentLimitsConfig {
            max_turns: 0,
            ..defaults.clone()
        }
        .validate()
        .is_err());
        assert!(AgentLimitsConfig {
            max_duration_seconds: 601,
            ..defaults.clone()
        }
        .validate()
        .is_err());
        assert!(AgentLimitsConfig {
            max_tokens: 127,
            ..defaults.clone()
        }
        .validate()
        .is_err());
        assert!(AgentLimitsConfig {
            max_external_actions: 21,
            ..defaults
        }
        .validate()
        .is_err());
    }
}

#[cfg(test)]
mod plan_state_tests {
    use super::*;

    fn plan() -> crate::history::AgentPlan {
        crate::history::AgentPlan {
            title: "Preparar relatório".into(),
            steps: vec![
                crate::history::AgentPlanStep {
                    description: "Ler dados".into(),
                    status: crate::history::AgentPlanStepStatus::Pending,
                },
                crate::history::AgentPlanStep {
                    description: "Criar arquivo".into(),
                    status: crate::history::AgentPlanStepStatus::Pending,
                },
            ],
            outside_plan_actions: Vec::new(),
        }
    }

    #[test]
    fn real_actions_advance_plan_and_extras_are_reported_without_blocking() {
        let mut plan = plan();
        let first = start_plan_step(&mut plan, "Ler dados").unwrap();
        assert_eq!(
            plan.steps[first].status,
            crate::history::AgentPlanStepStatus::InProgress
        );
        assert!(finish_plan_step(&mut plan, first, true));
        assert_eq!(
            plan.steps[first].status,
            crate::history::AgentPlanStepStatus::Completed
        );

        let second = start_plan_step(&mut plan, "Criar arquivo").unwrap();
        assert!(finish_plan_step(&mut plan, second, false));
        assert_eq!(
            plan.steps[second].status,
            crate::history::AgentPlanStepStatus::Failed
        );
        assert_eq!(start_plan_step(&mut plan, "Consultar lembrete"), None);
        assert_eq!(plan.outside_plan_actions, ["Consultar lembrete"]);
    }
}

#[cfg(test)]
mod agent_error_tests {
    use super::classify_agent_error;

    #[test]
    fn missing_agent_home_points_to_identity_recovery() {
        let (kind, message, details) =
            classify_agent_error("A pasta configurada foi movida ou alterada.");
        assert_eq!(kind, "agent_home");
        assert!(message.contains("Identidade e memória"));
        assert!(details.contains("Verificação local"));
    }
}

#[tauri::command]
pub async fn agent_cancel(
    caller: tauri::WebviewWindow,
    state: State<'_, SecurityState>,
    conversation_id: String,
) -> Result<bool, String> {
    crate::require_main(&caller)?;
    let runs = state.agent_runs.lock().await;
    if let Some(token) = runs.get(&conversation_id) {
        token.cancel();
        Ok(true)
    } else {
        Ok(false)
    }
}
