use chrono::{DateTime, Datelike, Duration, Local, Months, NaiveDateTime, TimeZone, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tauri::Manager;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEvent {
    pub id: String,
    pub title: String,
    pub start: String,
    pub end: String,
    pub location: String,
    pub description: String,
    pub alert_minutes: Option<u32>,
    pub recurrence: String,
}

fn interval_bounds(from: &str, to: &str) -> Result<(DateTime<Utc>, DateTime<Utc>), String> {
    let start = DateTime::parse_from_rfc3339(from)
        .map_err(|_| "O início do intervalo é inválido.")?
        .with_timezone(&Utc);
    let end = DateTime::parse_from_rfc3339(to)
        .map_err(|_| "O fim do intervalo é inválido.")?
        .with_timezone(&Utc);
    if start >= end {
        return Err("O intervalo da agenda é inválido.".into());
    }
    Ok((start, end))
}

fn validate(event: &CalendarEvent) -> Result<(), String> {
    if event.id.is_empty()
        || event.id.len() > 80
        || event.title.trim().is_empty()
        || event.title.chars().count() > 200
        || event.location.chars().count() > 500
        || event.description.chars().count() > 4000
        || event.start.len() > 64
        || event.end.len() > 64
        || event.alert_minutes.is_some_and(|n| n > 10080)
        || !matches!(
            event.recurrence.as_str(),
            "none" | "daily" | "weekly" | "monthly"
        )
    {
        return Err("Os dados do evento são inválidos.".into());
    }
    let start = DateTime::parse_from_rfc3339(&event.start)
        .map_err(|_| "O início precisa incluir data, hora e fuso.".to_string())?;
    let end = DateTime::parse_from_rfc3339(&event.end)
        .map_err(|_| "O fim precisa incluir data, hora e fuso.".to_string())?;
    if end <= start {
        return Err("O fim do evento precisa ocorrer depois do início.".into());
    }
    if event.title.contains('\0')
        || event.location.contains('\0')
        || event.description.contains('\0')
    {
        return Err("O evento contém caracteres inválidos.".into());
    }
    Ok(())
}

fn db(app: &tauri::AppHandle) -> Result<Connection, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|_| "Não foi possível localizar os dados locais.".to_string())?;
    std::fs::create_dir_all(&dir)
        .map_err(|_| "Não foi possível preparar os dados locais.".to_string())?;
    let conn = Connection::open(dir.join("calendar.sqlite3"))
        .map_err(|_| "Não foi possível abrir a agenda local.".to_string())?;
    init(&conn)?;
    Ok(conn)
}

fn init(conn: &Connection) -> Result<(), String> {
    conn.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE IF NOT EXISTS calendar_events (id TEXT PRIMARY KEY, title TEXT NOT NULL, start TEXT NOT NULL, end TEXT NOT NULL, location TEXT NOT NULL, description TEXT NOT NULL, alert_minutes INTEGER, recurrence TEXT NOT NULL CHECK(recurrence IN ('none','daily','weekly','monthly'))); CREATE TABLE IF NOT EXISTS calendar_settings (key TEXT PRIMARY KEY,value TEXT NOT NULL);")
        .map_err(|_| "Não foi possível preparar a agenda local.".to_string())
}

fn save(conn: &Connection, event: &CalendarEvent) -> Result<(), String> {
    validate(event)?;
    conn.execute("INSERT INTO calendar_events(id,title,start,end,location,description,alert_minutes,recurrence) VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(id) DO UPDATE SET title=excluded.title,start=excluded.start,end=excluded.end,location=excluded.location,description=excluded.description,alert_minutes=excluded.alert_minutes,recurrence=excluded.recurrence", params![event.id,event.title,event.start,event.end,event.location,event.description,event.alert_minutes,event.recurrence])
        .map_err(|_| "Não foi possível salvar o evento.".to_string())?;
    Ok(())
}

fn create_from_skill(conn: &Connection, params: &Value) -> Result<String, String> {
    let title = params["title"]
        .as_str()
        .ok_or("Informe o título do evento.")?;
    let start = params["start"]
        .as_str()
        .ok_or("Informe o início do evento.")?;
    let end = params["end"]
        .as_str()
        .ok_or("Informe o fim do evento; não vou presumir a duração.")?;
    let location = params["location"].as_str().unwrap_or_default();
    let description = params["description"].as_str().unwrap_or_default();
    let alert_minutes = params["alert_minutes"].as_u64().map(|n| n as u32);
    let recurrence = params["recurrence"].as_str().unwrap_or("none");
    let key = format!("{}|{}|{}|{}", title.trim(), start, end, recurrence);
    let id = format!(
        "agent-{}",
        &format!("{:x}", Sha256::digest(key.as_bytes()))[..32]
    );
    let event = CalendarEvent {
        id: id.clone(),
        title: title.into(),
        start: start.into(),
        end: end.into(),
        location: location.into(),
        description: description.into(),
        alert_minutes,
        recurrence: recurrence.into(),
    };
    save(conn, &event)?;
    Ok(format!("Evento salvo na Agenda local do OdeKrai: {} ({}). Alertas ficam pendentes até haver agendador.", event.title, event.start))
}

fn event_from_skill(params: &Value) -> Result<CalendarEvent, String> {
    let title = params["title"]
        .as_str()
        .ok_or("Informe o título do evento.")?;
    let start = params["start"]
        .as_str()
        .ok_or("Informe o início do evento.")?;
    let end = params["end"]
        .as_str()
        .ok_or("Informe o fim do evento; não vou presumir a duração.")?;
    let key = format!(
        "{}|{}|{}|{}",
        title.trim(),
        start,
        end,
        params["recurrence"].as_str().unwrap_or("none")
    );
    let id = format!(
        "agent-{}",
        &format!("{:x}", Sha256::digest(key.as_bytes()))[..32]
    );
    let event = CalendarEvent {
        id,
        title: title.into(),
        start: start.into(),
        end: end.into(),
        location: params["location"].as_str().unwrap_or_default().into(),
        description: params["description"].as_str().unwrap_or_default().into(),
        alert_minutes: params["alert_minutes"].as_u64().map(|n| n as u32),
        recurrence: params["recurrence"].as_str().unwrap_or("none").into(),
    };
    validate(&event)?;
    Ok(event)
}

fn ics_event(event: &CalendarEvent) -> Result<String, String> {
    let start = DateTime::parse_from_rfc3339(&event.start)
        .map_err(|_| "Início inválido.")?
        .with_timezone(&Utc);
    let end = DateTime::parse_from_rfc3339(&event.end)
        .map_err(|_| "Fim inválido.")?
        .with_timezone(&Utc);
    let mut text=format!("BEGIN:VEVENT\r\nUID:{}@odekrai.local\r\nDTSTART:{}\r\nDTEND:{}\r\nSUMMARY:{}\r\nLOCATION:{}\r\nDESCRIPTION:{}\r\n",event.id,start.format("%Y%m%dT%H%M%SZ"),end.format("%Y%m%dT%H%M%SZ"),escape_ics(&event.title),escape_ics(&event.location),escape_ics(&event.description));
    let freq = match event.recurrence.as_str() {
        "daily" => Some("DAILY"),
        "weekly" => Some("WEEKLY"),
        "monthly" => Some("MONTHLY"),
        _ => None,
    };
    if let Some(freq) = freq {
        text.push_str(&format!("RRULE:FREQ={freq}\r\n"));
    }
    text.push_str("END:VEVENT\r\n");
    Ok(text)
}

fn save_to_ics(path: &std::path::Path, event: &CalendarEvent) -> Result<(), String> {
    let event_text = ics_event(event)?;
    let content = if path.exists() {
        let content = std::fs::read_to_string(path)
            .map_err(|_| "Não foi possível ler o calendário .ics configurado.")?;
        if content.len() > 1_000_000 {
            return Err("O calendário .ics excede 1 MB.".into());
        }
        if !content.starts_with("BEGIN:VCALENDAR") || !content.trim_end().ends_with("END:VCALENDAR")
        {
            return Err("O calendário .ics configurado é inválido.".into());
        }
        if content.contains(&format!("UID:{}@odekrai.local", event.id)) {
            return Ok(());
        }
        content
            .trim_end()
            .strip_suffix("END:VCALENDAR")
            .ok_or("Calendário .ics inválido.")?
            .to_string()
            + &event_text
            + "END:VCALENDAR\r\n"
    } else {
        format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//OdeKrai//Agenda//PT-BR\r\n{}END:VCALENDAR\r\n",event_text)
    };
    std::fs::write(path, content)
        .map_err(|_| "Não foi possível gravar o calendário .ics.".to_string())
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarDefault {
    pub mode: String,
    pub path: Option<String>,
    pub google_available: bool,
}

fn default_target(conn: &Connection) -> Result<CalendarDefault, String> {
    let mode = conn
        .query_row(
            "SELECT value FROM calendar_settings WHERE key='default_mode'",
            [],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .map_err(|_| "Não foi possível consultar o calendário padrão.".to_string())?
        .unwrap_or_else(|| "local".into());
    let path = conn
        .query_row(
            "SELECT value FROM calendar_settings WHERE key='ics_path'",
            [],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .map_err(|_| "Não foi possível consultar o arquivo .ics padrão.".to_string())?;
    Ok(CalendarDefault {
        mode,
        path,
        google_available: crate::google_oauth::has_scope(crate::google_calendar::SCOPE),
    })
}

pub(crate) fn google_is_default(app: &tauri::AppHandle) -> bool {
    db(app)
        .and_then(|conn| default_target(&conn))
        .is_ok_and(|target| target.mode == "google")
}

#[tauri::command]
pub fn calendar_get_default(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
) -> Result<CalendarDefault, String> {
    crate::require_main(&caller)?;
    default_target(&db(&app)?)
}

#[tauri::command]
pub fn calendar_set_default(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    mode: String,
    path: Option<String>,
) -> Result<(), String> {
    crate::require_main(&caller)?;
    if !matches!(mode.as_str(), "local" | "ics") {
        if mode == "google" && crate::google_oauth::has_scope(crate::google_calendar::SCOPE) {
            let conn = db(&app)?;
            conn.execute("INSERT INTO calendar_settings(key,value) VALUES('default_mode','google') ON CONFLICT(key) DO UPDATE SET value=excluded.value", [])
                .map_err(|_| "Não foi possível salvar o destino Google Calendar.".to_string())?;
            return Ok(());
        }
        return Err("Destino de calendário inválido.".into());
    }
    if mode == "ics" {
        let raw = path
            .as_deref()
            .ok_or("Escolha um arquivo .ics para o calendário padrão.")?;
        let file = std::path::Path::new(raw);
        if !file.is_absolute()
            || file
                .extension()
                .is_none_or(|ext| !ext.eq_ignore_ascii_case("ics"))
            || file.to_string_lossy().starts_with("\\\\")
        {
            return Err("Escolha um caminho local absoluto para um arquivo .ics.".into());
        }
    }
    let mut conn = db(&app)?;
    let tx = conn
        .transaction()
        .map_err(|_| "Não foi possível iniciar a configuração da Agenda.")?;
    tx.execute("INSERT INTO calendar_settings(key,value) VALUES('default_mode',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[&mode]).map_err(|_|"Não foi possível salvar o destino da Agenda.")?;
    if let Some(path) = path {
        tx.execute("INSERT INTO calendar_settings(key,value) VALUES('ics_path',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[path]).map_err(|_|"Não foi possível salvar o arquivo .ics.")?;
    }
    tx.commit()
        .map_err(|_| "Não foi possível salvar a configuração da Agenda.")?;
    Ok(())
}

pub(crate) fn execute_skill(
    app: &tauri::AppHandle,
    skill_id: &str,
    params: &Value,
) -> Result<String, String> {
    let conn = db(app)?;
    match skill_id {
        "calendar.create_event" => {
            let target = default_target(&conn)?;
            if target.mode == "ics" {
                let path = target
                    .path
                    .ok_or("Configure o arquivo .ics padrão na Agenda.")?;
                let event = event_from_skill(params)?;
                let path = std::path::PathBuf::from(path);
                if path
                    .extension()
                    .is_none_or(|ext| !ext.eq_ignore_ascii_case("ics"))
                {
                    return Err("Arquivo .ics padrão inválido.".into());
                }
                save_to_ics(&path, &event)?;
                use tauri_plugin_opener::OpenerExt;
                app.opener()
                    .open_path(path.to_string_lossy(), None::<&str>)
                    .map_err(|_| {
                        "Evento salvo no arquivo .ics, mas não foi possível abrir o calendário."
                    })?;
                Ok(format!(
                    "Evento salvo e aberto no calendário do sistema pelo arquivo {}.",
                    path.display()
                ))
            } else {
                create_from_skill(&conn, params)
            }
        }
        "calendar.list_events" => {
            let from =
                DateTime::parse_from_rfc3339(params["from"].as_str().ok_or("Início inválido.")?)
                    .map_err(|_| "Início inválido.".to_string())?
                    .with_timezone(&Utc)
                    .to_rfc3339();
            let to = DateTime::parse_from_rfc3339(params["to"].as_str().ok_or("Fim inválido.")?)
                .map_err(|_| "Fim inválido.".to_string())?
                .with_timezone(&Utc)
                .to_rfc3339();
            let mut statement=conn.prepare("SELECT id,title,start,end,location,description,alert_minutes,recurrence FROM calendar_events WHERE julianday(start)<julianday(?2) AND julianday(end)>julianday(?1) ORDER BY julianday(start) LIMIT 1000").map_err(|_| "Não foi possível consultar a Agenda.".to_string())?;
            let rows = statement
                .query_map(params![from, to], |r| {
                    Ok(CalendarEvent {
                        id: r.get(0)?,
                        title: r.get(1)?,
                        start: r.get(2)?,
                        end: r.get(3)?,
                        location: r.get(4)?,
                        description: r.get(5)?,
                        alert_minutes: r.get(6)?,
                        recurrence: r.get(7)?,
                    })
                })
                .map_err(|_| "Não foi possível consultar a Agenda.".to_string())?;
            let events = rows
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| "Não foi possível ler os eventos.".to_string())?;
            serde_json::to_string(&events)
                .map_err(|_| "Não foi possível preparar a lista de eventos.".into())
        }
        "calendar.update_event" => {
            let event: CalendarEvent =
                serde_json::from_value(params.get("event").cloned().ok_or("Evento ausente.")?)
                    .map_err(|_| "Evento inválido.".to_string())?;
            let exists: bool = conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM calendar_events WHERE id=?1)",
                    [&event.id],
                    |row| row.get(0),
                )
                .map_err(|_| "Não foi possível localizar o evento para atualização.".to_string())?;
            if !exists {
                return Err("O evento não foi encontrado; nada foi atualizado.".into());
            }
            save(&conn, &event)?;
            Ok(format!(
                "Evento atualizado na Agenda local do OdeKrai: {}.",
                event.title
            ))
        }
        "calendar.delete_event" => {
            let id = params["id"].as_str().ok_or("Identificador ausente.")?;
            let changed = conn
                .execute("DELETE FROM calendar_events WHERE id=?1", [id])
                .map_err(|_| "Não foi possível apagar o evento.".to_string())?;
            if changed == 0 {
                return Err("O evento não foi encontrado; nada foi apagado.".into());
            }
            Ok("Evento apagado da Agenda local do OdeKrai.".into())
        }
        _ => Err("Habilidade de agenda desconhecida.".into()),
    }
}

pub(crate) fn preview_skill(
    app: &tauri::AppHandle,
    skill_id: &str,
    params: &Value,
) -> Result<String, String> {
    let target = default_target(&db(app)?)?;
    Ok(match skill_id {
        "calendar.create_event" if target.mode == "google" => format!(
            "Criar evento no Google Calendar (calendário principal), após aprovação: {}",
            params
        ),
        "calendar.create_event" if target.mode == "ics" => format!(
            "Salvar evento em {} e abrir o calendário do sistema.",
            target
                .path
                .unwrap_or_else(|| "arquivo .ics não configurado".into())
        ),
        "calendar.create_event" => format!(
            "Salvar evento na Agenda local do OdeKrai: {} — {}",
            params["title"], params["start"]
        ),
        "calendar.update_event" => {
            format!("Atualizar evento {} na Agenda local", params["event"]["id"])
        }
        "calendar.delete_event" => format!(
            "Apagar evento {} da Agenda local (aprovação obrigatória)",
            params["id"]
        ),
        "calendar.list_events" => format!(
            "Consultar a Agenda local entre {} e {}",
            params["from"], params["to"]
        ),
        _ => return Err("Habilidade de agenda desconhecida.".into()),
    })
}

pub(crate) fn current_local_time() -> String {
    format_local_time(Local::now().fixed_offset())
}

fn format_local_time(now: DateTime<chrono::FixedOffset>) -> String {
    now.format("%Y-%m-%d %H:%M:%S %:z (%Z)").to_string()
}

fn unescape_ics(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n' | 'N') => output.push('\n'),
                Some(',') => output.push(','),
                Some(';') => output.push(';'),
                Some('\\') => output.push('\\'),
                Some(other) => {
                    output.push('\\');
                    output.push(other);
                }
                None => output.push('\\'),
            }
        } else {
            output.push(ch);
        }
    }
    output
}

fn escape_ics(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace('\n', "\\n")
}

fn parse_ics_datetime(value: &str) -> Result<String, String> {
    if let Ok(date) = DateTime::parse_from_rfc3339(value) {
        return Ok(date.with_timezone(&Utc).to_rfc3339());
    }
    let utc = value.ends_with('Z');
    let value = value.strip_suffix('Z').unwrap_or(value);
    let naive = NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S")
        .map_err(|_| "Data .ics inválida.".to_string())?;
    if utc {
        return Ok(Utc.from_utc_datetime(&naive).to_rfc3339());
    }
    Local
        .from_local_datetime(&naive)
        .single()
        .map(|date| date.with_timezone(&Utc).to_rfc3339())
        .ok_or_else(|| "Horário local ambíguo no arquivo .ics.".into())
}

fn parse_ics(content: &str) -> Result<Vec<CalendarEvent>, String> {
    if content.len() > 1_000_000 || !content.starts_with("BEGIN:VCALENDAR") {
        return Err("Arquivo .ics inválido ou maior que 1 MB.".into());
    }
    let mut events = Vec::new();
    let mut current: Option<std::collections::HashMap<String, String>> = None;
    for line in content.lines().map(|line| line.trim_end_matches('\r')) {
        if line == "BEGIN:VEVENT" {
            if current.is_some() {
                return Err("Evento .ics aninhado.".into());
            }
            current = Some(Default::default());
            continue;
        }
        if line == "END:VEVENT" {
            let fields = current.take().ok_or("Evento .ics sem início.")?;
            let uid = fields
                .get("UID")
                .cloned()
                .unwrap_or_else(|| format!("import-{}", events.len()));
            let start = parse_ics_datetime(fields.get("DTSTART").ok_or("Evento sem início.")?)?;
            let end = parse_ics_datetime(fields.get("DTEND").ok_or("Evento sem fim.")?)?;
            let recurrence = match fields.get("RRULE").map(String::as_str).unwrap_or("") {
                "" => "none",
                "FREQ=DAILY" => "daily",
                "FREQ=WEEKLY" => "weekly",
                "FREQ=MONTHLY" => "monthly",
                _ => return Err("A recorrência do arquivo .ics não é suportada.".into()),
            };
            let id = format!(
                "ics-{}",
                &format!("{:x}", Sha256::digest(uid.as_bytes()))[..32]
            );
            let event = CalendarEvent {
                id,
                title: unescape_ics(
                    fields
                        .get("SUMMARY")
                        .map(String::as_str)
                        .unwrap_or("Evento"),
                ),
                start,
                end,
                location: unescape_ics(fields.get("LOCATION").map(String::as_str).unwrap_or("")),
                description: unescape_ics(
                    fields.get("DESCRIPTION").map(String::as_str).unwrap_or(""),
                ),
                alert_minutes: None,
                recurrence: recurrence.into(),
            };
            validate(&event)?;
            events.push(event);
            if events.len() > 1000 {
                return Err("O arquivo excede o limite de 1000 eventos.".into());
            }
            continue;
        }
        if let Some(fields) = current.as_mut() {
            if let Some((key, value)) = line.split_once(':') {
                if key.starts_with("DTSTART;") || key.starts_with("DTEND;") {
                    return Err("Parâmetros de data .ics, incluindo TZID e eventos de dia inteiro, não são suportados.".into());
                }
                let name = key.split(';').next().unwrap_or(key);
                if matches!(
                    name,
                    "UID" | "DTSTART" | "DTEND" | "SUMMARY" | "LOCATION" | "DESCRIPTION" | "RRULE"
                ) {
                    fields.insert(name.into(), value.into());
                }
            }
        }
    }
    if current.is_some() {
        return Err("O arquivo contém evento incompleto.".into());
    }
    Ok(events)
}

#[tauri::command]
pub fn calendar_import_ics(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    path: String,
) -> Result<usize, String> {
    crate::require_main(&caller)?;
    let file = std::path::PathBuf::from(path);
    if file
        .extension()
        .is_none_or(|ext| !ext.eq_ignore_ascii_case("ics"))
    {
        return Err("Selecione um arquivo .ics.".into());
    }
    let metadata = std::fs::metadata(&file)
        .map_err(|_| "Não foi possível acessar o arquivo .ics selecionado.")?;
    if !metadata.is_file() || metadata.len() > 1_000_000 {
        return Err("O arquivo .ics precisa ter até 1 MB.".into());
    }
    let content = std::fs::read_to_string(file)
        .map_err(|_| "Não foi possível ler o arquivo .ics como UTF-8.")?;
    let events = parse_ics(&content)?;
    let mut conn = db(&app)?;
    let tx = conn
        .transaction()
        .map_err(|_| "Não foi possível iniciar a importação.")?;
    for event in &events {
        save(&tx, event)?;
    }
    tx.commit()
        .map_err(|_| "Não foi possível concluir a importação.")?;
    Ok(events.len())
}

#[tauri::command]
pub fn calendar_export_ics(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    path: String,
) -> Result<usize, String> {
    crate::require_main(&caller)?;
    let file = std::path::PathBuf::from(path);
    if file
        .extension()
        .is_none_or(|ext| !ext.eq_ignore_ascii_case("ics"))
    {
        return Err("Escolha um destino .ics.".into());
    }
    let conn = db(&app)?;
    let mut statement = conn.prepare("SELECT id,title,start,end,location,description,alert_minutes,recurrence FROM calendar_events ORDER BY julianday(start) LIMIT 1000").map_err(|_| "Não foi possível consultar a Agenda.")?;
    let rows = statement
        .query_map([], |r| {
            Ok(CalendarEvent {
                id: r.get(0)?,
                title: r.get(1)?,
                start: r.get(2)?,
                end: r.get(3)?,
                location: r.get(4)?,
                description: r.get(5)?,
                alert_minutes: r.get(6)?,
                recurrence: r.get(7)?,
            })
        })
        .map_err(|_| "Não foi possível consultar a Agenda.")?;
    let events = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "Não foi possível ler os eventos.")?;
    let mut content =
        String::from("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//OdeKrai//Agenda//PT-BR\r\n");
    for event in &events {
        let start = DateTime::parse_from_rfc3339(&event.start)
            .map_err(|_| "Evento com data inválida.")?
            .with_timezone(&Utc);
        let end = DateTime::parse_from_rfc3339(&event.end)
            .map_err(|_| "Evento com data inválida.")?
            .with_timezone(&Utc);
        content.push_str(&format!("BEGIN:VEVENT\r\nUID:{}@odekrai.local\r\nDTSTART:{}\r\nDTEND:{}\r\nSUMMARY:{}\r\nLOCATION:{}\r\nDESCRIPTION:{}\r\n", event.id, start.format("%Y%m%dT%H%M%SZ"), end.format("%Y%m%dT%H%M%SZ"), escape_ics(&event.title), escape_ics(&event.location), escape_ics(&event.description)));
        let freq = match event.recurrence.as_str() {
            "daily" => Some("DAILY"),
            "weekly" => Some("WEEKLY"),
            "monthly" => Some("MONTHLY"),
            _ => None,
        };
        if let Some(freq) = freq {
            content.push_str(&format!("RRULE:FREQ={freq}\r\n"));
        }
        content.push_str("END:VEVENT\r\n");
    }
    content.push_str("END:VCALENDAR\r\n");
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&file)
        .map_err(|_| "O destino já existe ou não pode ser criado; escolha outro arquivo .ics.")?;
    use std::io::Write;
    output
        .write_all(content.as_bytes())
        .and_then(|_| output.sync_data())
        .map_err(|_| "Não foi possível gravar o arquivo .ics.")?;
    Ok(events.len())
}

#[tauri::command]
pub fn calendar_list_events(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    from: String,
    to: String,
) -> Result<Vec<CalendarEvent>, String> {
    crate::require_main(&caller)?;
    let (range_start, range_end) = interval_bounds(&from, &to)?;
    let from = range_start.to_rfc3339();
    let to = range_end.to_rfc3339();
    let conn = db(&app)?;
    let mut stmt = conn.prepare("SELECT id,title,start,end,location,description,alert_minutes,recurrence FROM calendar_events ORDER BY julianday(start) LIMIT 1000").map_err(|_| "Não foi possível consultar a agenda.".to_string())?;
    let rows = stmt
        .query_map(params![from, to], |r| {
            Ok(CalendarEvent {
                id: r.get(0)?,
                title: r.get(1)?,
                start: r.get(2)?,
                end: r.get(3)?,
                location: r.get(4)?,
                description: r.get(5)?,
                alert_minutes: r.get(6)?,
                recurrence: r.get(7)?,
            })
        })
        .map_err(|_| "Não foi possível consultar a agenda.".to_string())?;
    let events = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "Não foi possível ler os eventos.".to_string())?;
    // Recurrences are projected into the requested range for the UI; the original row stays authoritative.
    let mut expanded = Vec::new();
    for event in events {
        let mut occurrence = event.clone();
        let base_start = DateTime::parse_from_rfc3339(&event.start)
            .map_err(|_| "Evento inválido na agenda.")?
            .with_timezone(&Utc);
        if base_start < range_start {
            let steps = match event.recurrence.as_str() {
                "daily" => (range_start - base_start)
                    .num_days()
                    .saturating_sub(1)
                    .max(0) as u32,
                "weekly" => ((range_start - base_start).num_days() / 7)
                    .saturating_sub(1)
                    .max(0) as u32,
                "monthly" => {
                    let delta = (range_start.year() - base_start.year()) * 12
                        + (range_start.month() as i32 - base_start.month() as i32);
                    delta.saturating_sub(1).max(0) as u32
                }
                _ => 0,
            };
            if steps > 0 {
                let next = match event.recurrence.as_str() {
                    "daily" => base_start + Duration::days(steps as i64),
                    "weekly" => base_start + Duration::weeks(steps as i64),
                    "monthly" => base_start
                        .checked_add_months(Months::new(steps))
                        .ok_or("Recorrência inválida.")?,
                    _ => base_start,
                };
                let duration = DateTime::parse_from_rfc3339(&event.end)
                    .map_err(|_| "Evento inválido na agenda.")?
                    .with_timezone(&Utc)
                    - base_start;
                occurrence.start = next.to_rfc3339();
                occurrence.end = (next + duration).to_rfc3339();
            }
        }
        for _ in 0..500 {
            let start = DateTime::parse_from_rfc3339(&occurrence.start)
                .map_err(|_| "Evento inválido na agenda.".to_string())?
                .with_timezone(&Utc);
            let end = DateTime::parse_from_rfc3339(&occurrence.end)
                .map_err(|_| "Evento inválido na agenda.".to_string())?
                .with_timezone(&Utc);
            if start >= range_end {
                break;
            }
            if end > range_start {
                expanded.push(occurrence.clone());
            }
            let next_start = match event.recurrence.as_str() {
                "daily" => start + Duration::days(1),
                "weekly" => start + Duration::weeks(1),
                "monthly" => start
                    .checked_add_months(Months::new(1))
                    .ok_or("Recorrência inválida.")?,
                _ => break,
            };
            let duration = end - start;
            occurrence.id = format!("{}@{}", event.id, next_start.timestamp());
            occurrence.start = next_start.to_rfc3339();
            occurrence.end = (next_start + duration).to_rfc3339();
        }
    }
    expanded.sort_by_key(|event| {
        DateTime::parse_from_rfc3339(&event.start)
            .map(|date| date.timestamp())
            .unwrap_or_default()
    });
    Ok(expanded)
}

#[tauri::command]
pub fn calendar_save_event(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    event: CalendarEvent,
) -> Result<(), String> {
    crate::require_main(&caller)?;
    save(&db(&app)?, &event)
}

#[tauri::command]
pub fn calendar_delete_event(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    id: String,
) -> Result<(), String> {
    crate::require_main(&caller)?;
    if id.is_empty() || id.len() > 80 {
        return Err("Identificador de evento inválido.".into());
    }
    let changed = db(&app)?
        .execute("DELETE FROM calendar_events WHERE id=?1", [id])
        .map_err(|_| "Não foi possível apagar o evento.".to_string())?;
    if changed == 0 {
        return Err("O evento não foi encontrado; nada foi apagado.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn event() -> CalendarEvent {
        CalendarEvent {
            id: "evt-1".into(),
            title: "Reunião".into(),
            start: "2030-03-10T01:30:00-05:00".into(),
            end: "2030-03-10T02:30:00-05:00".into(),
            location: "".into(),
            description: "".into(),
            alert_minutes: Some(10),
            recurrence: "weekly".into(),
        }
    }

    #[test]
    fn saves_updates_and_lists_events_by_absolute_time() {
        let conn = Connection::open_in_memory().unwrap();
        init(&conn).unwrap();
        let mut e = event();
        save(&conn, &e).unwrap();
        e.title = "Atualizada".into();
        e.recurrence = "monthly".into();
        save(&conn, &e).unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM calendar_events WHERE title='Atualizada' AND recurrence='monthly' AND julianday(start) < julianday(?1) AND julianday(end) > julianday(?2)",
            params!["2030-03-10T08:00:00Z", "2030-03-10T06:00:00Z"],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 1);
        assert_ne!(
            DateTime::parse_from_rfc3339(&e.start)
                .unwrap()
                .with_timezone(&Utc),
            DateTime::parse_from_rfc3339("2030-03-10T01:30:00Z").unwrap()
        );
    }

    #[test]
    fn rejects_missing_timezone_past_end_and_invalid_recurrence() {
        let conn = Connection::open_in_memory().unwrap();
        init(&conn).unwrap();
        let mut e = event();
        e.start = "2030-03-10T01:30:00".into();
        assert!(save(&conn, &e).is_err());
        e = event();
        e.end = "2030-03-10T01:00:00-05:00".into();
        assert!(save(&conn, &e).is_err());
        e = event();
        e.recurrence = "yearly".into();
        assert!(save(&conn, &e).is_err());
    }

    #[test]
    fn invalid_event_list_interval_returns_a_clear_error() {
        assert!(interval_bounds("início inválido", "2030-03-10T10:00:00Z")
            .unwrap_err()
            .contains("início do intervalo"));
    }

    #[test]
    fn agent_create_is_idempotent_and_requires_an_explicit_end() {
        let db_path = std::env::temp_dir().join(format!(
            "odekrai-calendar-{}-{}.sqlite",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let conn = Connection::open(&db_path).unwrap();
        init(&conn).unwrap();
        let params = serde_json::json!({"title":"Entrevista","start":"2030-01-01T13:00:00-03:00","end":"2030-01-01T14:00:00-03:00","location":"","description":"","alert_minutes":null,"recurrence":"none"});
        create_from_skill(&conn, &params).unwrap();
        create_from_skill(&conn, &params).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM calendar_events", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
        let missing = serde_json::json!({"title":"Sem duração","start":"2030-01-01T13:00:00-03:00","location":"","description":"","alert_minutes":null,"recurrence":"none"});
        assert!(create_from_skill(&conn, &missing)
            .unwrap_err()
            .contains("não vou presumir"));
        drop(conn);
        std::fs::remove_file(db_path).unwrap();
    }

    #[test]
    fn imports_untrusted_ics_and_escapes_round_trip_fields() {
        let input = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:meeting-1\r\nDTSTART:20300310T103000Z\r\nDTEND:20300310T113000Z\r\nSUMMARY:Reunião\\, importante\r\nLOCATION:São Paulo\r\nDESCRIPTION:Plano\\nAnotações\r\nRRULE:FREQ=WEEKLY\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let events = parse_ics(input).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].title, "Reunião, importante");
        assert_eq!(events[0].description, "Plano\nAnotações");
        assert_eq!(events[0].recurrence, "weekly");
        assert_eq!(unescape_ics(r"literal \\n"), r"literal \n");
        assert!(parse_ics("BEGIN:VCALENDAR\nBEGIN:VEVENT\nEND:VCALENDAR").is_err());
        assert!(parse_ics("BEGIN:VCALENDAR\nBEGIN:VEVENT\nDTSTART;TZID=America/Sao_Paulo:20300310T103000\nEND:VEVENT\nEND:VCALENDAR").is_err());
        assert!(parse_ics("BEGIN:VCALENDAR\nBEGIN:VEVENT\nDTSTART:20300310T103000Z\nDTEND:20300310T113000Z\nRRULE:FREQ=YEARLY\nEND:VEVENT\nEND:VCALENDAR").is_err());
    }

    #[test]
    fn local_clock_format_uses_the_injected_system_clock_value() {
        let fixed = chrono::DateTime::parse_from_rfc3339("2030-03-10T13:15:00-03:00").unwrap();
        let value = format_local_time(fixed);
        assert!(value.starts_with("2030-03-10 13:15:00 -03:00"));
    }

    #[test]
    fn configured_ics_write_is_idempotent_and_reports_write_failures() {
        let event = event();
        let path = std::env::temp_dir().join(format!(
            "odekrai-calendar-{}-{}.ics",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        save_to_ics(&path, &event).unwrap();
        save_to_ics(&path, &event).unwrap();
        assert_eq!(
            parse_ics(&std::fs::read_to_string(&path).unwrap())
                .unwrap()
                .len(),
            1
        );
        std::fs::remove_file(&path).unwrap();
        assert!(save_to_ics(&std::env::temp_dir(), &event).is_err());
    }
}
