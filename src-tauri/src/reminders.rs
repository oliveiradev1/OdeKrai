use chrono::{DateTime, Local, TimeZone, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
use tauri::Manager;
use tokio::sync::Notify;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Default)]
pub(crate) struct ReminderScheduler {
    changed: Notify,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Reminder {
    pub id: String,
    pub title: String,
    pub when: String,
    pub note: String,
    pub status: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpcomingReminder {
    pub title: String,
    pub when_local: String,
    pub status: String,
}

const UPCOMING_LIMIT: usize = 20;

fn upcoming_at<Tz: TimeZone>(
    connection: &Connection,
    now: DateTime<Utc>,
    window_hours: u32,
    timezone: Tz,
) -> Result<Vec<UpcomingReminder>, String> {
    if !(1..=168).contains(&window_hours) {
        return Err("A janela deve estar entre 1 e 168 horas.".into());
    }
    let end = now + chrono::Duration::hours(i64::from(window_hours));
    let mut statement = connection.prepare("SELECT title,due_at,status FROM reminders WHERE status IN ('pending','notification_error') AND due_at>?1 AND due_at<=?2 ORDER BY due_at LIMIT ?3")
        .map_err(|_| "Não foi possível consultar os próximos lembretes.".to_string())?;
    let rows = statement
        .query_map(
            params![now.timestamp(), end.timestamp(), UPCOMING_LIMIT as i64],
            |row| {
                let title: String = row.get(0)?;
                let timestamp: i64 = row.get(1)?;
                let status: String = row.get(2)?;
                let title = title.chars().take(100).collect::<String>();
                let when = timezone
                    .timestamp_opt(timestamp, 0)
                    .single()
                    .map(|value| value.to_rfc3339())
                    .unwrap_or_default();
                Ok(UpcomingReminder {
                    title,
                    when_local: when,
                    status,
                })
            },
        )
        .map_err(|_| "Não foi possível consultar os próximos lembretes.".to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|_| "Não foi possível ler os próximos lembretes.".to_string())
}

#[tauri::command]
pub async fn get_upcoming_reminders(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    window_hours: u32,
) -> Result<Vec<UpcomingReminder>, String> {
    crate::require_main(&caller)?;
    let enabled = crate::security::skills::is_skill_enabled(&app, "reminder.list")?;
    if !crate::security::skills::skill_exposed_to_agent("reminder.list", enabled, false) {
        return Ok(Vec::new());
    }
    let connection = open(&database_path(&app)?)?;
    upcoming_at(&connection, Utc::now(), window_hours, Local)
}

fn database_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Não foi possível localizar os dados locais dos lembretes.".to_string())?;
    std::fs::create_dir_all(&directory)
        .map_err(|_| "Não foi possível preparar os dados locais dos lembretes.".to_string())?;
    Ok(directory.join("reminders.sqlite3"))
}

fn open(path: &std::path::Path) -> Result<Connection, String> {
    let connection = Connection::open(path)
        .map_err(|_| "Não foi possível abrir os lembretes locais.".to_string())?;
    init(&connection)?;
    Ok(connection)
}

fn init(connection: &Connection) -> Result<(), String> {
    connection
        .execute_batch(
            "PRAGMA journal_mode=WAL;
         CREATE TABLE IF NOT EXISTS reminders (
             id TEXT PRIMARY KEY,
             title TEXT NOT NULL,
             when_iso TEXT NOT NULL,
             due_at INTEGER NOT NULL,
             note TEXT NOT NULL,
             status TEXT NOT NULL CHECK(status IN ('pending','delivered','notification_error'))
         );",
        )
        .map_err(|_| "Não foi possível preparar o armazenamento dos lembretes.".to_string())
}

fn parse_when(value: &str) -> Result<DateTime<Utc>, String> {
    if value.len() > 64 {
        return Err("A data do lembrete é inválida.".into());
    }
    DateTime::parse_from_rfc3339(value)
        .map(|date| date.with_timezone(&Utc))
        .map_err(|_| "Informe a data do lembrete em ISO 8601 com fuso horário.".into())
}

fn validate(title: &str, when: &str, note: &str) -> Result<DateTime<Utc>, String> {
    if title.trim().is_empty() || title.chars().count() > 160 || title.contains('\0') {
        return Err("O título do lembrete deve ter entre 1 e 160 caracteres.".into());
    }
    if note.chars().count() > 2000 || note.contains('\0') {
        return Err("A nota do lembrete excede o limite permitido.".into());
    }
    if crate::memory_skills::contains_sensitive_data(title)
        || crate::memory_skills::contains_sensitive_data(note)
    {
        return Err("O lembrete contém dado confidencial e não pode ser armazenado.".into());
    }
    parse_when(when)
}

pub(crate) fn validate_params(skill_id: &str, params: &Value) -> Result<(), String> {
    let object = params
        .as_object()
        .ok_or("Parâmetros do lembrete inválidos.")?;
    let expected: &[&str] = match skill_id {
        "reminder.create" => &["title", "when", "note"],
        "reminder.list" => &[],
        "reminder.cancel" => &["id"],
        _ => return Err("Habilidade de lembrete desconhecida.".into()),
    };
    if object.len() != expected.len() || object.keys().any(|key| !expected.contains(&key.as_str()))
    {
        return Err("Os parâmetros do lembrete não correspondem ao esquema permitido.".into());
    }
    match skill_id {
        "reminder.create" => {
            validate(
                params["title"].as_str().ok_or("Título inválido.")?,
                params["when"].as_str().ok_or("Data inválida.")?,
                params["note"].as_str().ok_or("Nota inválida.")?,
            )?;
        }
        "reminder.list" => {}
        "reminder.cancel" => {
            if params["id"]
                .as_str()
                .is_none_or(|id| id.is_empty() || id.len() > 80)
            {
                return Err("ID do lembrete inválido.".into());
            }
        }
        _ => unreachable!(),
    }
    Ok(())
}

fn create(
    connection: &Connection,
    title: &str,
    when: &str,
    note: &str,
) -> Result<Reminder, String> {
    let due = validate(title, when, note)?;
    let id = format!(
        "reminder-{}-{}",
        Utc::now().timestamp_millis(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    );
    let reminder = Reminder {
        id,
        title: title.trim().to_string(),
        when: due.to_rfc3339(),
        note: note.to_string(),
        status: "pending".into(),
    };
    connection.execute(
        "INSERT INTO reminders(id,title,when_iso,due_at,note,status) VALUES(?1,?2,?3,?4,?5,'pending')",
        params![reminder.id, reminder.title, reminder.when, due.timestamp(), reminder.note],
    ).map_err(|_| "Não foi possível salvar o lembrete.".to_string())?;
    Ok(reminder)
}

fn list(connection: &Connection) -> Result<Vec<Reminder>, String> {
    let mut statement = connection.prepare(
        "SELECT id,title,when_iso,note,status FROM reminders WHERE status IN ('pending','notification_error') ORDER BY due_at LIMIT 500",
    ).map_err(|_| "Não foi possível consultar os lembretes.".to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok(Reminder {
                id: row.get(0)?,
                title: row.get(1)?,
                when: row.get(2)?,
                note: row.get(3)?,
                status: row.get(4)?,
            })
        })
        .map_err(|_| "Não foi possível consultar os lembretes.".to_string())?;
    let reminders = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "Não foi possível ler os lembretes.".to_string())?;
    if reminders.iter().any(|reminder| {
        crate::memory_skills::contains_sensitive_data(&reminder.title)
            || crate::memory_skills::contains_sensitive_data(&reminder.note)
    }) {
        return Err(
            "A lista contém dado confidencial reconhecido e não será enviada ao agente.".into(),
        );
    }
    Ok(reminders)
}

pub(crate) fn pending_count(app: &tauri::AppHandle) -> Result<usize, String> {
    let connection = open(&database_path(app)?)?;
    Ok(list(&connection)?.len())
}

fn cancel(connection: &Connection, id: &str) -> Result<(), String> {
    let changed = connection
        .execute(
            "DELETE FROM reminders WHERE id=?1 AND status IN ('pending','notification_error')",
            [id],
        )
        .map_err(|_| "Não foi possível cancelar o lembrete.".to_string())?;
    if changed == 0 {
        return Err("Lembrete pendente não encontrado.".into());
    }
    Ok(())
}

fn next_due(connection: &Connection) -> Result<Option<i64>, String> {
    connection
        .query_row(
            "SELECT MIN(due_at) FROM reminders WHERE status='pending'",
            [],
            |row| row.get::<_, Option<i64>>(0),
        )
        .map_err(|_| "Não foi possível consultar o próximo lembrete.".into())
}

fn deliver_due<F>(connection: &Connection, now: i64, mut notify: F) -> Result<usize, String>
where
    F: FnMut(&Reminder) -> Result<(), String>,
{
    let mut statement = connection.prepare(
        "SELECT id,title,when_iso,note,status FROM reminders WHERE status='pending' AND due_at<=?1 ORDER BY due_at LIMIT 100",
    ).map_err(|_| "Não foi possível consultar lembretes vencidos.".to_string())?;
    let rows = statement
        .query_map([now], |row| {
            Ok(Reminder {
                id: row.get(0)?,
                title: row.get(1)?,
                when: row.get(2)?,
                note: row.get(3)?,
                status: row.get(4)?,
            })
        })
        .map_err(|_| "Não foi possível consultar lembretes vencidos.".to_string())?;
    let reminders = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "Não foi possível ler lembretes vencidos.".to_string())?;
    drop(statement);
    let mut delivered = 0;
    for reminder in reminders {
        let status = if notify(&reminder).is_ok() {
            delivered += 1;
            "delivered"
        } else {
            "notification_error"
        };
        connection
            .execute(
                "UPDATE reminders SET status=?1 WHERE id=?2 AND status='pending'",
                params![status, reminder.id],
            )
            .map_err(|_| "Não foi possível atualizar o estado do lembrete.".to_string())?;
    }
    Ok(delivered)
}

pub(crate) fn execute_skill(
    app: &tauri::AppHandle,
    skill_id: &str,
    params: &Value,
) -> Result<String, String> {
    let connection = open(&database_path(app)?)?;
    match skill_id {
        "reminder.create" => {
            let reminder = create(
                &connection,
                params["title"]
                    .as_str()
                    .ok_or("Informe o título do lembrete.")?,
                params["when"]
                    .as_str()
                    .ok_or("Informe quando o lembrete deve tocar.")?,
                params["note"].as_str().unwrap_or_default(),
            )?;
            app.state::<ReminderScheduler>().changed.notify_one();
            Ok(format!(
                "Lembrete salvo: {} para {}.",
                reminder.title, reminder.when
            ))
        }
        "reminder.list" => serde_json::to_string(&list(&connection)?)
            .map_err(|_| "Não foi possível preparar a lista de lembretes.".into()),
        "reminder.cancel" => {
            cancel(
                &connection,
                params["id"].as_str().ok_or("Informe o ID do lembrete.")?,
            )?;
            app.state::<ReminderScheduler>().changed.notify_one();
            Ok("Lembrete cancelado.".into())
        }
        _ => Err("Habilidade de lembrete desconhecida.".into()),
    }
}

pub(crate) fn start_scheduler(app: &tauri::AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = if let Ok(path) = database_path(&app) {
            if let Ok(connection) = open(&path) {
                connection
                    .execute(
                        "UPDATE reminders SET status='pending' WHERE status='notification_error'",
                        [],
                    )
                    .ok()
            } else {
                None
            }
        } else {
            None
        };
        loop {
            let Ok(path) = database_path(&app) else {
                tokio::time::sleep(Duration::from_secs(60)).await;
                continue;
            };
            let Ok(connection) = open(&path) else {
                tokio::time::sleep(Duration::from_secs(60)).await;
                continue;
            };
            match next_due(&connection) {
                Ok(Some(timestamp)) => {
                    let delay = timestamp.saturating_sub(Utc::now().timestamp()).max(0) as u64;
                    let scheduler = app.state::<ReminderScheduler>();
                    let changed = scheduler.changed.notified();
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_secs(delay)), if delay > 0 => {},
                        _ = changed => continue,
                        _ = tokio::time::sleep(Duration::from_millis(1)), if delay == 0 => {},
                    }
                    if let Ok(connection) = open(&path) {
                        let _ = deliver_due(&connection, Utc::now().timestamp(), |reminder| {
                            use tauri_plugin_notification::NotificationExt;
                            let body = if reminder.note.is_empty() {
                                "Seu lembrete venceu."
                            } else {
                                reminder.note.as_str()
                            };
                            let result = app
                                .notification()
                                .builder()
                                .title(&reminder.title)
                                .body(body)
                                .show();
                            if result.is_ok() {
                                let _ = crate::pet_alerts::enqueue_reminder(
                                    &app,
                                    &reminder.id,
                                    &reminder.title,
                                    body,
                                );
                            }
                            result.map_err(|_| "O Windows não exibiu a notificação.".to_string())
                        });
                    }
                }
                Ok(None) | Err(_) => app.state::<ReminderScheduler>().changed.notified().await,
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_explicit_timezone_and_delivers_past_due_once() {
        let connection = Connection::open_in_memory().unwrap();
        init(&connection).unwrap();
        let reminder = create(
            &connection,
            "Entrevista",
            "2020-01-01T13:00:00-03:00",
            "Levar currículo",
        )
        .unwrap();
        assert_eq!(
            parse_when(&reminder.when).unwrap().timestamp(),
            1_577_894_400
        );
        let mut notifications = Vec::new();
        assert_eq!(
            deliver_due(&connection, Utc::now().timestamp(), |item| {
                notifications.push(item.id.clone());
                Ok(())
            })
            .unwrap(),
            1
        );
        assert_eq!(
            deliver_due(&connection, Utc::now().timestamp(), |_| Ok(())).unwrap(),
            0
        );
        assert_eq!(notifications, vec![reminder.id]);
        assert!(list(&connection).unwrap().is_empty());
    }

    #[test]
    fn rejects_missing_timezone_and_cancel_removes_pending_reminder() {
        let connection = Connection::open_in_memory().unwrap();
        init(&connection).unwrap();
        assert!(create(&connection, "Entrevista", "2030-01-01T13:00:00", "").is_err());
        let reminder = create(&connection, "Entrevista", "2030-01-01T13:00:00-03:00", "").unwrap();
        cancel(&connection, &reminder.id).unwrap();
        assert!(list(&connection).unwrap().is_empty());
        assert!(cancel(&connection, &reminder.id).is_err());
    }

    #[test]
    fn notification_error_is_retained_for_next_start_retry() {
        let connection = Connection::open_in_memory().unwrap();
        init(&connection).unwrap();
        let reminder = create(&connection, "Aviso", "2020-01-01T00:00:00Z", "").unwrap();
        assert_eq!(
            deliver_due(&connection, Utc::now().timestamp(), |_| Err(
                "mock failure".into()
            ))
            .unwrap(),
            0
        );
        assert_eq!(list(&connection).unwrap()[0].id, reminder.id);
    }

    #[test]
    fn upcoming_uses_local_timezone_rolls_over_midnight_and_hides_notes() {
        let connection = Connection::open_in_memory().unwrap();
        init(&connection).unwrap();
        create(
            &connection,
            "Próximo",
            "2026-10-31T01:30:00Z",
            "nota privada",
        )
        .unwrap();
        let now = DateTime::parse_from_rfc3339("2026-10-31T01:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let timezone = chrono::FixedOffset::west_opt(3 * 3600).unwrap();
        let rows = upcoming_at(&connection, now, 24, timezone).unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].when_local.starts_with("2026-10-30T22:30:00-03:00"));
        assert!(!serde_json::to_string(&rows)
            .unwrap()
            .contains("nota privada"));
    }

    #[test]
    fn upcoming_excludes_past_orders_and_limits_to_twenty() {
        let connection = Connection::open_in_memory().unwrap();
        init(&connection).unwrap();
        create(&connection, "Passado", "2026-10-30T10:59:00Z", "").unwrap();
        for index in 1..=21 {
            let due = 1_793_360_000 + index * 60;
            connection.execute("INSERT INTO reminders(id,title,when_iso,due_at,note,status) VALUES(?1,?2,'',?3,'','pending')", params![format!("r{index}"), format!("Item {index}"), due]).unwrap();
        }
        let now = DateTime::from_timestamp(1_793_360_000, 0).unwrap();
        let rows = upcoming_at(&connection, now, 24, Utc).unwrap();
        assert_eq!(rows.len(), 20);
        assert!(rows
            .windows(2)
            .all(|pair| pair[0].when_local < pair[1].when_local));
        assert!(!rows.iter().any(|reminder| reminder.title == "Passado"));
        assert!(upcoming_at(&connection, now, 0, Utc).is_err());
    }
}
