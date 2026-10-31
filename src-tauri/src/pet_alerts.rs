use serde::Serialize;
use std::{collections::VecDeque, sync::Mutex};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

const MAX_ALERTS: usize = 32;

#[derive(Clone, Debug, PartialEq, Eq)]
struct PetAlert {
    id: String,
    state: String,
    message: String,
    priority: u8,
    pinned: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PetAlertSnapshot {
    alert_id: Option<String>,
    state: String,
    message: String,
    count: usize,
    pinned: bool,
    alerts: Vec<PetAlertSummary>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct PetAlertSummary {
    alert_id: String,
    state: String,
    message: String,
}

#[derive(Default)]
pub(crate) struct PetAlertQueue(Mutex<VecDeque<PetAlert>>);

fn snapshot(queue: &VecDeque<PetAlert>) -> PetAlertSnapshot {
    queue.front().map_or_else(
        || PetAlertSnapshot {
            state: "compact".into(),
            ..PetAlertSnapshot::default()
        },
        |active| PetAlertSnapshot {
            alert_id: Some(active.id.clone()),
            state: active.state.clone(),
            message: active.message.clone(),
            count: queue.len(),
            pinned: active.pinned,
            alerts: queue
                .iter()
                .map(|alert| PetAlertSummary {
                    alert_id: alert.id.clone(),
                    state: alert.state.clone(),
                    message: alert.message.clone(),
                })
                .collect(),
        },
    )
}

fn push(queue: &mut VecDeque<PetAlert>, alert: PetAlert) {
    if let Some(index) = queue.iter().position(|current| current.id == alert.id) {
        queue.remove(index);
    }
    if queue.len() == MAX_ALERTS {
        let Some(last) = queue.back() else { return };
        if last.priority >= alert.priority {
            return;
        }
        queue.pop_back();
    }
    let index = queue
        .iter()
        .position(|current| current.priority < alert.priority)
        .unwrap_or(queue.len());
    queue.insert(index, alert);
}

fn safe_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 80
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn update(app: &AppHandle, alerts: &PetAlertQueue) -> Result<(), String> {
    let state = alerts
        .0
        .lock()
        .map_err(|_| "A fila de alertas do pet está indisponível.".to_string())?;
    app.emit("pet-alert-state", snapshot(&state))
        .map_err(|_| "Não foi possível atualizar o estado de alertas do pet.".to_string())
}

fn enqueue(app: &AppHandle, alert: PetAlert) -> Result<(), String> {
    let alerts = app.state::<PetAlertQueue>();
    {
        let mut queue = alerts
            .0
            .lock()
            .map_err(|_| "A fila de alertas do pet está indisponível.".to_string())?;
        push(&mut queue, alert);
    }
    update(app, &alerts)
}

pub(crate) fn enqueue_agent_activity(app: &AppHandle, state: &str) -> Result<(), String> {
    let (id, message, priority) = match state {
        "thinking" => ("agent-progress", "Pensando", 10),
        "responding" => ("agent-progress", "Preparando resposta", 10),
        "task" => ("agent-progress", "Executando tarefa", 10),
        "completed" => ("agent-outcome", "Resposta pronta", 30),
        "error" => ("agent-outcome", "Falha na tarefa", 40),
        "paused" => ("agent-outcome", "Agente parado", 30),
        "notification" => ("agent-outcome", "Notificação", 20),
        "listening" => ("agent-progress", "Ouvindo", 10),
        _ => return Err("Estado de atividade do pet desconhecido.".into()),
    };
    let alerts = app.state::<PetAlertQueue>();
    {
        let mut queue = alerts
            .0
            .lock()
            .map_err(|_| "A fila de alertas do pet está indisponível.".to_string())?;
        if id == "agent-outcome" {
            queue.retain(|alert| alert.id != "agent-progress");
        }
        push(
            &mut queue,
            PetAlert {
                id: id.into(),
                state: state.into(),
                message: message.into(),
                priority,
                pinned: false,
            },
        );
    }
    update(app, &alerts)
}

pub(crate) fn enqueue_approval(app: &AppHandle, id: &str) -> Result<(), String> {
    if !safe_id(id) {
        return Err("Identificador de aprovação inválido para o alerta do pet.".into());
    }
    enqueue(
        app,
        PetAlert {
            id: format!("approval-{id}"),
            state: "awaiting_approval".into(),
            message: "Aguardando aprovação".into(),
            priority: 100,
            pinned: true,
        },
    )
}

pub(crate) fn resolve_approval(app: &AppHandle, id: &str) -> Result<(), String> {
    let alerts = app.state::<PetAlertQueue>();
    {
        let mut queue = alerts
            .0
            .lock()
            .map_err(|_| "A fila de alertas do pet está indisponível.".to_string())?;
        queue.retain(|alert| alert.id != format!("approval-{id}"));
    }
    update(app, &alerts)
}

pub(crate) fn enqueue_reminder(
    app: &AppHandle,
    id: &str,
    title: &str,
    body: &str,
) -> Result<(), String> {
    if !safe_id(id) {
        return Err("Identificador de lembrete inválido para o alerta do pet.".into());
    }
    let title: String = title.chars().take(160).collect();
    let body: String = body.chars().take(400).collect();
    let message = if body.is_empty() {
        title
    } else {
        format!("{title}: {body}")
    };
    enqueue(
        app,
        PetAlert {
            id: format!("reminder-{id}"),
            state: "notification".into(),
            message,
            priority: 50,
            pinned: false,
        },
    )
}

#[tauri::command]
pub(crate) fn get_pet_alert_state(
    caller: WebviewWindow,
    alerts: tauri::State<'_, PetAlertQueue>,
) -> Result<PetAlertSnapshot, String> {
    crate::require_caller(&caller, &["main", "pet"])?;
    let queue = alerts
        .0
        .lock()
        .map_err(|_| "A fila de alertas do pet está indisponível.".to_string())?;
    Ok(snapshot(&queue))
}

#[tauri::command]
pub(crate) fn enqueue_pet_activity(
    app: AppHandle,
    caller: WebviewWindow,
    state: String,
) -> Result<(), String> {
    crate::require_caller(&caller, &["main", "pet"])?;
    enqueue_agent_activity(&app, &state)
}

#[tauri::command]
pub(crate) fn dismiss_pet_alert(
    app: AppHandle,
    caller: WebviewWindow,
    alert_id: String,
) -> Result<(), String> {
    crate::require_caller(&caller, &["main", "pet"])?;
    if !safe_id(&alert_id) {
        return Err("Identificador de alerta inválido.".into());
    }
    let alerts = app.state::<PetAlertQueue>();
    {
        let mut queue = alerts
            .0
            .lock()
            .map_err(|_| "A fila de alertas do pet está indisponível.".to_string())?;
        if queue
            .front()
            .is_some_and(|active| active.id == alert_id && !active.pinned)
        {
            queue.pop_front();
        }
    }
    update(&app, &alerts)
}

#[cfg(test)]
mod tests {
    use super::{push, safe_id, snapshot, PetAlert, MAX_ALERTS};
    use std::collections::VecDeque;

    fn alert(id: &str, state: &str, priority: u8, pinned: bool) -> PetAlert {
        PetAlert {
            id: id.into(),
            state: state.into(),
            message: state.into(),
            priority,
            pinned,
        }
    }

    #[test]
    fn prioritizes_approval_then_preserves_fifo_for_equal_priority() {
        let mut queue = VecDeque::new();
        push(&mut queue, alert("agent", "task", 10, false));
        push(&mut queue, alert("reminder-1", "notification", 50, false));
        push(
            &mut queue,
            alert("approval-1", "awaiting_approval", 100, true),
        );
        push(&mut queue, alert("reminder-2", "notification", 50, false));
        assert_eq!(
            queue
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["approval-1", "reminder-1", "reminder-2", "agent"]
        );
        let current = snapshot(&queue);
        assert_eq!(current.alert_id.as_deref(), Some("approval-1"));
        assert_eq!(current.count, 4);
        assert_eq!(current.alerts.len(), 4);
        assert_eq!(current.alerts[0].state, "awaiting_approval");
        assert!(current.pinned);
    }

    #[test]
    fn replaces_progress_in_place_and_bounds_the_queue() {
        let mut queue = VecDeque::new();
        push(&mut queue, alert("agent-progress", "thinking", 10, false));
        push(&mut queue, alert("reminder", "notification", 50, false));
        push(&mut queue, alert("agent-progress", "task", 10, false));
        assert_eq!(queue.len(), 2);
        assert_eq!(queue.back().unwrap().state, "task");
        for index in 0..=MAX_ALERTS {
            push(
                &mut queue,
                alert(&format!("approval-{index}"), "awaiting_approval", 100, true),
            );
        }
        assert_eq!(queue.len(), MAX_ALERTS);
    }

    #[test]
    fn alert_ids_are_bounded_ascii_tokens() {
        assert!(safe_id("reminder-123_x"));
        assert!(!safe_id(""));
        assert!(!safe_id("../private"));
        assert!(!safe_id("x".repeat(81).as_str()));
    }
}
