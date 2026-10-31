use chrono::{DateTime, FixedOffset, Utc};
use serde_json::Value;
use sha2::Digest;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use tauri_plugin_opener::OpenerExt;

const MAX_TEXT_BYTES: usize = 1_000_000;

fn string_param<'a>(params: &'a Value, key: &str) -> Result<&'a str, String> {
    params[key]
        .as_str()
        .ok_or_else(|| format!("O parâmetro {key} da habilidade de produtividade é inválido."))
}

fn array_param<'a>(params: &'a Value, key: &str) -> Result<&'a Vec<Value>, String> {
    params[key]
        .as_array()
        .ok_or_else(|| format!("A lista {key} da habilidade de produtividade é inválida."))
}

pub(crate) fn validate(skill_id: &str, params: &Value) -> Result<(), String> {
    if skill_id == "screen.capture" {
        let object = params.as_object().ok_or("Parâmetros inválidos.")?;
        if object.len() != 2 {
            return Err("Os parâmetros da captura de tela são inválidos.".into());
        }
        let path = string_field(object, "path", 120)?;
        let capture_id = string_field(object, "_capture_id", 128)?;
        if !path.ends_with(".jpg")
            || path.starts_with('.')
            || path.contains("..")
            || !path
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        {
            return Err("Informe um nome simples para o arquivo de captura .jpg.".into());
        }
        if capture_id.is_empty()
            || !capture_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err("Identificador interno da captura inválido.".into());
        }
        return Ok(());
    }
    if skill_id.starts_with("reminder.") {
        return crate::reminders::validate_params(skill_id, params);
    }
    if skill_id.starts_with("memory.") {
        return crate::memory_skills::validate_params(skill_id, params);
    }
    if skill_id == "sheet.create_xlsx" {
        return crate::spreadsheets::validate_create(params);
    }
    if skill_id == "sheet.read" {
        let object = params
            .as_object()
            .ok_or("Parâmetros da planilha inválidos.")?;
        if object.len() != 2
            || object
                .keys()
                .any(|key| !["path", "max_rows"].contains(&key.as_str()))
        {
            return Err("Os parâmetros de leitura não correspondem ao esquema permitido.".into());
        }
        let path = string_field(object, "path", 4096)?;
        if !Path::new(path).extension().is_some_and(|extension| {
            extension.eq_ignore_ascii_case("csv") || extension.eq_ignore_ascii_case("xlsx")
        }) {
            return Err("A leitura aceita somente arquivos .csv ou .xlsx.".into());
        }
        if Path::new(path).is_absolute() {
            crate::security::skills::validate_raw_folder_path(path)?;
        } else {
            crate::security::workspace::relative_path(path)?;
        }
        if object["max_rows"]
            .as_u64()
            .is_none_or(|limit| !(1..=1000).contains(&limit))
        {
            return Err("O limite deve ficar entre 1 e 1.000 linhas.".into());
        }
        return Ok(());
    }
    if skill_id.starts_with("calendar.") {
        let object = params.as_object().ok_or("Parâmetros inválidos.")?;
        if skill_id == "calendar.create_event"
            && object.get("end").and_then(Value::as_str).is_none()
        {
            return Err("A duração não foi informada; pergunte o horário de término antes de criar o evento.".into());
        }
        let (expected, required): (&[&str], &[&str]) = match skill_id {
            "calendar.create_event" => (
                &[
                    "title",
                    "start",
                    "end",
                    "location",
                    "description",
                    "alert_minutes",
                    "recurrence",
                ],
                &[
                    "title",
                    "start",
                    "end",
                    "location",
                    "description",
                    "recurrence",
                ],
            ),
            "calendar.list_events" => (&["from", "to"], &["from", "to"]),
            "calendar.update_event" => (&["event"], &["event"]),
            "calendar.delete_event" => (&["id"], &["id"]),
            _ => return Err("Habilidade de agenda desconhecida.".into()),
        };
        if object.len() != expected.len()
            || object.keys().any(|key| !expected.contains(&key.as_str()))
            || required.iter().any(|key| !object.contains_key(*key))
        {
            return Err("Os parâmetros da agenda são inválidos.".into());
        }
        match skill_id {
            "calendar.create_event" => {
                let title = string_field(object, "title", 200)?;
                if title.trim().is_empty() {
                    return Err("Informe o título do evento.".into());
                }
                let start = parse_datetime(string_field(object, "start", 64)?)?;
                let end = parse_datetime(string_field(object, "end", 64)?)?;
                if end <= start {
                    return Err("O fim do evento precisa ocorrer depois do início.".into());
                }
                if string_field(object, "location", 500)?.contains('\0')
                    || string_field(object, "description", 4000)?.contains('\0')
                {
                    return Err("Os dados do evento são inválidos.".into());
                }
                if !matches!(
                    object["recurrence"].as_str(),
                    Some("none" | "daily" | "weekly" | "monthly")
                ) {
                    return Err("Recorrência inválida.".into());
                }
                if object
                    .get("alert_minutes")
                    .is_some_and(|v| !v.is_null() && v.as_u64().is_none_or(|n| n > 10080))
                {
                    return Err("Alerta inválido.".into());
                }
            }
            "calendar.list_events" => {
                let from = parse_datetime(string_field(object, "from", 64)?)?;
                let to = parse_datetime(string_field(object, "to", 64)?)?;
                if to <= from {
                    return Err("Intervalo inválido.".into());
                }
            }
            "calendar.update_event" => {
                serde_json::from_value::<crate::calendar::CalendarEvent>(object["event"].clone())
                    .map_err(|_| "Evento inválido.".to_string())?;
            }
            "calendar.delete_event" => {
                let id = string_field(object, "id", 80)?;
                if id.is_empty() {
                    return Err("Identificador inválido.".into());
                }
            }
            _ => return Err("Habilidade de calendário desconhecida.".into()),
        }
        return Ok(());
    }
    if skill_id == "code.delegate" {
        let object = params.as_object().ok_or("Parâmetros inválidos.")?;
        if object.len() != 3
            || !object.contains_key("project")
            || !object.contains_key("task")
            || !object.contains_key("_delegate_id")
        {
            return Err("Os parâmetros da delegação de código são inválidos.".into());
        }
        let project = object["project"].as_str().ok_or("Projeto inválido.")?;
        let task = object["task"].as_str().ok_or("Tarefa inválida.")?;
        let run_id = object["_delegate_id"]
            .as_str()
            .ok_or("Execução inválida.")?;
        crate::code_delegate::validate_request(project, task)?;
        if run_id.is_empty()
            || !run_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err("Identificador interno da execução inválido.".into());
        }
        return Ok(());
    }
    if skill_id == "discord.send" {
        let object = params.as_object().ok_or("Parâmetros inválidos.")?;
        if object.len() != 2 || !object.contains_key("webhook_id") || !object.contains_key("text") {
            return Err("Os parâmetros do envio ao Discord são inválidos.".into());
        }
        let id = object["webhook_id"].as_str().ok_or("Webhook inválido.")?;
        let text = object["text"].as_str().ok_or("Texto inválido.")?;
        if !id.starts_with("discord-") || text.trim().is_empty() || text.chars().count() > 2000 {
            return Err("Informe webhook cadastrado e texto de até 2000 caracteres.".into());
        }
        if text.to_lowercase().contains("@everyone") || text.to_lowercase().contains("@here") {
            return Err("A mensagem não pode conter @everyone nem @here.".into());
        }
        return Ok(());
    }
    if skill_id == "web.fetch" {
        return crate::web_fetch::validate_skill_params(params);
    }
    if skill_id.starts_with("system.")
        || skill_id.starts_with("audio.")
        || skill_id.starts_with("media.")
        || skill_id.starts_with("wallpaper.")
        || skill_id.starts_with("display.")
        || skill_id.starts_with("theme.")
        || skill_id == "power.lock"
        || skill_id == "power.sleep"
        || skill_id == "settings.open"
        || skill_id == "notification.show"
        || skill_id == "app.launch"
        || skill_id.starts_with("clipboard.")
    {
        return crate::system_actions::validate(skill_id, params);
    }
    let object = params.as_object().ok_or("Parâmetros inválidos.")?;
    let expected: &[&str] = match skill_id {
        "calendar.create_event" => &[
            "path",
            "title",
            "start",
            "end",
            "location",
            "description",
            "alert_minutes",
        ],
        "mail.draft" => &["to", "subject", "body"],
        "doc.create_markdown" | "doc.create_text" => &["path", "content"],
        "sheet.create_csv" => &["path", "delimiter", "encoding", "headers", "rows"],
        _ => return Err("Habilidade de produtividade desconhecida.".into()),
    };
    let mail_path =
        skill_id == "mail.draft" && object.get("path").and_then(Value::as_str).is_some();
    if object.len() != expected.len() + usize::from(mail_path)
        || object
            .keys()
            .any(|key| !expected.contains(&key.as_str()) && !(mail_path && key == "path"))
    {
        return Err("Os parâmetros da habilidade não correspondem ao esquema permitido.".into());
    }
    let path = object
        .get("path")
        .and_then(Value::as_str)
        .ok_or("O caminho está ausente.")?;
    if Path::new(path).is_absolute() {
        crate::security::skills::validate_raw_folder_path(path)?;
    } else {
        crate::security::workspace::relative_path(path)?;
    }
    let content = object.get("content").and_then(Value::as_str);
    match skill_id {
        "calendar.create_event" => {
            validate_extension(path, "ics")?;
            let title = string_field(object, "title", 200)?;
            let start = string_field(object, "start", 64)?;
            let end = object.get("end").and_then(Value::as_str);
            let location = string_field(object, "location", 500)?;
            let description = string_field(object, "description", 4000)?;
            if title.trim().is_empty()
                || start.is_empty()
                || location.contains('\0')
                || description.contains('\0')
            {
                return Err("Os dados do evento são inválidos.".into());
            }
            let start = parse_datetime(start)?;
            if let Some(end) = end {
                if parse_datetime(end)? <= start {
                    return Err("O fim do evento precisa ocorrer depois do início.".into());
                }
            }
            if object
                .get("alert_minutes")
                .is_some_and(|v| !v.is_null() && v.as_u64().is_none_or(|n| n > 10080))
            {
                return Err("O alerta deve estar entre 0 e 10080 minutos.".into());
            }
        }
        "mail.draft" => {
            let to = string_field(object, "to", 320)?;
            let subject = string_field(object, "subject", 998)?;
            let body = string_field(object, "body", 100_000)?;
            if to.trim().is_empty()
                || to.contains(['\r', '\n', '\0'])
                || subject.contains(['\r', '\n', '\0'])
                || body.contains('\0')
            {
                return Err("Os dados do rascunho de e-mail são inválidos.".into());
            }
        }
        "doc.create_markdown" => {
            validate_extension(path, "md")?;
            validate_content(content)?;
        }
        "doc.create_text" => {
            validate_extension(path, "txt")?;
            validate_content(content)?;
        }
        "sheet.create_csv" => {
            validate_extension(path, "csv")?;
            let delimiter = object
                .get("delimiter")
                .and_then(Value::as_str)
                .ok_or("Separador ausente.")?;
            if !matches!(delimiter, "comma" | "semicolon" | "tab") {
                return Err("Separador CSV inválido.".into());
            }
            let encoding = object
                .get("encoding")
                .and_then(Value::as_str)
                .ok_or("Codificação ausente.")?;
            if !matches!(encoding, "utf8_bom" | "windows_1252") {
                return Err("Codificação CSV inválida.".into());
            }
            let headers = object
                .get("headers")
                .and_then(Value::as_array)
                .ok_or("Cabeçalhos ausentes.")?;
            let rows = object
                .get("rows")
                .and_then(Value::as_array)
                .ok_or("Linhas ausentes.")?;
            if headers.is_empty() || headers.len() > 100 || rows.len() > 10000 {
                return Err("A planilha excede os limites permitidos.".into());
            }
            for row in rows {
                let values = row.as_array().ok_or("Linha CSV inválida.")?;
                if values.len() != headers.len()
                    || values
                        .iter()
                        .any(|v| v.as_str().is_none_or(|s| s.len() > 4096))
                {
                    return Err("Todas as linhas devem ter a quantidade de colunas dos cabeçalhos e valores textuais curtos.".into());
                }
            }
            if headers
                .iter()
                .any(|cell| cell.as_str().is_none_or(|value| value.len() > 256))
            {
                return Err("Os cabeçalhos devem conter textos de até 256 bytes.".into());
            }
        }
        _ => return Err("Habilidade de produtividade desconhecida.".into()),
    }
    Ok(())
}

pub(crate) fn preview(skill_id: &str, params: &Value, root: &Path) -> Result<String, String> {
    if skill_id.starts_with("reminder.") {
        crate::reminders::validate_params(skill_id, params)?;
        return Ok(match skill_id {
            "reminder.create" => format!(
                "Criar lembrete: {} para {}.",
                params["title"], params["when"]
            ),
            "reminder.cancel" => format!("Cancelar lembrete {}.", params["id"]),
            _ => "Listar lembretes pendentes.".into(),
        });
    }
    if skill_id.starts_with("memory.") {
        return crate::memory_skills::preview(skill_id, params);
    }
    if skill_id.starts_with("calendar.") {
        validate(skill_id, params)?;
        return Ok(match skill_id {
            "calendar.create_event" => format!(
                "Salvar na Agenda local: {} — {}",
                params["title"], params["start"]
            ),
            "calendar.update_event" => {
                format!("Atualizar evento {} na Agenda local", params["event"]["id"])
            }
            "calendar.delete_event" => format!("Apagar evento {} da Agenda local", params["id"]),
            _ => format!(
                "Consultar Agenda local entre {} e {}",
                params["from"], params["to"]
            ),
        });
    }
    if skill_id.starts_with("system.")
        || skill_id.starts_with("audio.")
        || skill_id.starts_with("media.")
        || skill_id.starts_with("wallpaper.")
        || skill_id.starts_with("display.")
        || skill_id.starts_with("theme.")
        || skill_id == "power.lock"
        || skill_id == "power.sleep"
        || skill_id == "settings.open"
        || skill_id == "notification.show"
        || skill_id == "app.launch"
        || skill_id.starts_with("clipboard.")
    {
        return crate::system_actions::preview(skill_id, params, root);
    }
    if skill_id == "mail.draft" {
        validate(skill_id, params)?;
        return Ok(format!(
            "Rascunho de e-mail (não será enviado)\nPara: {}\nAssunto: {}\n\nCorpo:\n{}",
            params["to"].as_str().unwrap_or(""),
            params["subject"].as_str().unwrap_or(""),
            params["body"].as_str().unwrap_or("")
        ));
    }
    validate(skill_id, params)?;
    if skill_id == "discord.send" {
        return Err("A prévia do Discord exige acesso seguro ao cadastro do webhook.".into());
    }
    if skill_id == "sheet.create_xlsx" {
        let target =
            crate::security::workspace::preview_target(root, string_param(params, "path")?)?;
        return Ok(format!(
            "Criar XLSX em {} com {} aba(s).",
            target.display(),
            array_param(params, "sheets")?.len()
        ));
    }
    if skill_id == "sheet.read" {
        let target = crate::security::workspace::safe_target(root, string_param(params, "path")?)?;
        return Ok(format!(
            "Ler até {} linhas da planilha {}.",
            params["max_rows"],
            target.display()
        ));
    }
    let path = resolve(root, params, false)?;
    if skill_id == "calendar.create_event" {
        let start = parse_datetime(string_param(params, "start")?)?;
        Ok(format!(
            "Criar evento em {}\nTítulo: {}\nInício local: {}\nFim: {}\nLocal: {}\nDescrição: {}\nAlerta: {}\nSerá aberto no calendário padrão para confirmação; não envia para serviços de calendário e não usa OAuth.",
            path.display(),
            string_param(params, "title")?,
            start.with_timezone(&Local).format("%d/%m/%Y %H:%M %Z"),
            params["end"].as_str().and_then(|value| parse_datetime(value).ok()).map(|value| value.with_timezone(&Local).format("%d/%m/%Y %H:%M %Z").to_string()).unwrap_or_else(|| "não informado".into()),
            string_param(params, "location")?,
            string_param(params, "description")?,
            params["alert_minutes"].as_u64().map(|minutes| format!("{minutes} min")).unwrap_or_else(|| "nenhum".into())
        ))
    } else if skill_id.starts_with("doc.") {
        let content = string_param(params, "content")?;
        let excerpt: String = content.chars().take(4000).collect();
        let omitted = content.chars().count().saturating_sub(4000);
        Ok(format!(
            "Criar arquivo em:\n{}\n\nConteúdo{}:\n{}",
            path.display(),
            if omitted > 0 {
                format!(" ({} caracteres restantes fora deste resumo)", omitted)
            } else {
                String::new()
            },
            excerpt
        ))
    } else if skill_id == "sheet.create_csv" {
        let headers = params["headers"].to_string();
        let rows = array_param(params, "rows")?;
        let excerpt =
            serde_json::to_string(&rows.iter().take(5).collect::<Vec<_>>()).unwrap_or_default();
        Ok(format!("Criar CSV em {}\nSeparador: {}\nCodificação: {}\nCabeçalhos: {}\nLinhas: {}\nAmostra (até 5 linhas): {}", path.display(), params["delimiter"], params["encoding"], headers, rows.len(), excerpt))
    } else {
        Ok(format!("Criar arquivo em:\n{}", path.display()))
    }
}

pub(crate) fn execute(
    skill_id: &str,
    params: &Value,
    root: &Path,
    app: Option<&tauri::AppHandle>,
) -> Result<String, String> {
    if skill_id.starts_with("reminder.") {
        validate(skill_id, params)?;
        return crate::reminders::execute_skill(
            app.ok_or("Lembretes só podem ser usados no aplicativo.")?,
            skill_id,
            params,
        );
    }
    if skill_id.starts_with("memory.") {
        validate(skill_id, params)?;
        return crate::memory_skills::execute(
            app.ok_or("Memória só pode ser usada no aplicativo.")?,
            skill_id,
            params,
        );
    }
    if skill_id.starts_with("calendar.") {
        validate(skill_id, params)?;
        return crate::calendar::execute_skill(
            app.ok_or("A Agenda só pode ser usada no aplicativo.")?,
            skill_id,
            params,
        );
    }
    validate(skill_id, params)?;
    if skill_id == "discord.send" {
        return Err("O envio ao Discord requer a execução assíncrona aprovada.".into());
    }
    if skill_id == "sheet.create_xlsx" {
        return crate::spreadsheets::create_xlsx(root, params);
    }
    if skill_id == "sheet.read" {
        return crate::spreadsheets::read(root, params);
    }
    if skill_id.starts_with("system.")
        || skill_id.starts_with("audio.")
        || skill_id.starts_with("media.")
        || skill_id.starts_with("wallpaper.")
        || skill_id.starts_with("display.")
        || skill_id.starts_with("theme.")
        || skill_id == "power.lock"
        || skill_id == "power.sleep"
        || skill_id == "settings.open"
        || skill_id == "notification.show"
        || skill_id == "app.launch"
        || skill_id.starts_with("clipboard.")
    {
        return crate::system_actions::execute(
            skill_id,
            params,
            root,
            app.ok_or("A habilidade exige aprovação antes de executar.")?,
        );
    }
    if skill_id == "mail.draft" {
        let to = params["to"].as_str().unwrap_or("");
        let subject = params["subject"].as_str().unwrap_or("");
        let body = params["body"].as_str().unwrap_or("");
        let mailto = build_mailto(to, subject, Some(body));
        let path = resolve(root, params, false)?;
        if mailto.len() > 1800 {
            if app.is_none() {
                return Err("O rascunho só pode ser aberto após aprovação.".into());
            }
            let short_mailto = build_mailto(to, subject, None);
            if short_mailto.len() > 1800 {
                return Err(
                    "Destinatário e assunto excedem o limite seguro do link mailto.".into(),
                );
            }
            let body_path = path.with_file_name(format!(
                "{}.txt",
                path.file_stem().unwrap_or_default().to_string_lossy()
            ));
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&body_path)
                .map_err(|_| {
                    "Não foi possível criar o arquivo do corpo do rascunho.".to_string()
                })?;
            file.write_all(body.as_bytes())
                .and_then(|_| file.sync_data())
                .map_err(|_| "Não foi possível salvar o corpo do rascunho.".to_string())?;
            app.ok_or("O aplicativo padrão só pode ser aberto após aprovação.")?
                .opener()
                .open_url(short_mailto, None::<&str>)
                .map_err(|_| "Não foi possível abrir o aplicativo de e-mail padrão.".to_string())?;
            return Ok(format!("Rascunho aberto no aplicativo de e-mail padrão. O corpo completo foi salvo em {} para anexar. Nenhuma mensagem foi enviada.", body_path.display()));
        }
        if app.is_none() {
            return Err("O rascunho só pode ser aberto após aprovação.".into());
        }
        app.ok_or("O aplicativo padrão só pode ser aberto após aprovação.")?
            .opener()
            .open_url(mailto, None::<&str>)
            .map_err(|_| "Não foi possível abrir o aplicativo de e-mail padrão.".to_string())?;
        return Ok(
            "Rascunho aberto no aplicativo de e-mail padrão. Nenhuma mensagem foi enviada.".into(),
        );
    }
    let path = resolve(root, params, true)?;
    let bytes = match skill_id {
        "calendar.create_event" => create_ics(params)?.into_bytes(),
        "doc.create_markdown" | "doc.create_text" => {
            string_param(params, "content")?.as_bytes().to_vec()
        }
        "sheet.create_csv" => create_csv(params)?,
        _ => return Err("Habilidade de produtividade desconhecida.".into()),
    };
    if bytes.len() > MAX_TEXT_BYTES {
        return Err("O arquivo excede 1 MB.".into());
    }
    if skill_id == "calendar.create_event" && app.is_none() {
        return Err("O calendário padrão só pode ser aberto após aprovação no aplicativo.".into());
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|_| "Não foi possível criar o arquivo; ele pode já existir.".to_string())?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_data())
        .map_err(|_| "Não foi possível gravar o arquivo.".to_string())?;
    if skill_id == "calendar.create_event" {
        let app =
            app.ok_or("O calendário padrão só pode ser aberto após aprovação no aplicativo.")?;
        app.opener()
            .open_path(path.to_string_lossy(), None::<&str>)
            .map_err(|_| {
                "O evento foi salvo, mas não foi possível abrir o calendário padrão.".to_string()
            })?;
        return Ok(format!("Arquivo .ics criado e aberto para confirmação: {}. Não há integração direta com Google Calendar nem OAuth nesta etapa.", path.display()));
    }
    Ok(format!("Arquivo criado: {}", path.display()))
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\n', "\r\n")
        .bytes()
    {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

fn build_mailto(to: &str, subject: &str, body: Option<&str>) -> String {
    let mut url = format!(
        "mailto:{}?subject={}",
        percent_encode(to),
        percent_encode(subject)
    );
    if let Some(body) = body {
        url.push_str("&body=");
        url.push_str(&percent_encode(body));
    }
    url
}

#[cfg(test)]
mod mail_draft_tests {
    use super::{build_mailto, percent_encode};
    use serde_json::json;

    #[test]
    fn discord_text_is_bounded_and_forbids_broadcast_mentions() {
        let valid = json!({"webhook_id":"discord-123456789012345678","text":"x".repeat(2000)});
        assert!(super::validate("discord.send", &valid).is_ok());
        let oversized = json!({"webhook_id":"discord-123456789012345678","text":"x".repeat(2001)});
        assert!(super::validate("discord.send", &oversized).is_err());
        let mention = json!({"webhook_id":"discord-123456789012345678","text":"@here"});
        assert!(super::validate("discord.send", &mention).is_err());
    }

    #[test]
    fn mailto_values_encode_special_characters_and_normalize_newlines() {
        assert_eq!(
            percent_encode("Olá, A&B? #x\nlinha 2"),
            "Ol%C3%A1%2C%20A%26B%3F%20%23x%0D%0Alinha%202"
        );
    }

    #[test]
    fn complete_mailto_encodes_header_and_body_separators() {
        assert_eq!(
            build_mailto("a+b@example.com", "Olá & oi", Some("x? #\ny")),
            "mailto:a%2Bb%40example.com?subject=Ol%C3%A1%20%26%20oi&body=x%3F%20%23%0D%0Ay"
        );
    }
}

use chrono::Local;

fn resolve(root: &Path, params: &Value, create_parents: bool) -> Result<PathBuf, String> {
    let raw = params["path"].as_str().ok_or("O caminho está ausente.")?;
    let relative = Path::new(raw);
    if relative.is_absolute()
        || relative
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
    {
        return Err("Use um caminho relativo dentro da pasta permitida.".into());
    }
    let path = if create_parents {
        crate::security::workspace::create_safe_target(root, raw)?
    } else {
        crate::security::workspace::preview_target(root, raw)?
    };
    if path.exists() || crate::security::workspace::has_reparse(&path) {
        return Err("O destino já existe ou é um link; a criação foi recusada.".into());
    }
    Ok(path)
}

fn string_field<'a>(
    object: &'a serde_json::Map<String, Value>,
    name: &str,
    max: usize,
) -> Result<&'a str, String> {
    let value = object
        .get(name)
        .and_then(Value::as_str)
        .ok_or("Campo textual inválido.")?;
    if value.len() > max {
        return Err("Um campo de texto excede o limite permitido.".into());
    }
    Ok(value)
}

fn validate_content(content: Option<&str>) -> Result<(), String> {
    if content.is_none_or(|text| text.len() > MAX_TEXT_BYTES) {
        return Err("O conteúdo deve ser texto UTF-8 de até 1 MB.".into());
    }
    Ok(())
}

fn validate_extension(path: &str, extension: &str) -> Result<(), String> {
    if Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case(extension))
    {
        Ok(())
    } else {
        Err(format!("O arquivo precisa usar a extensão .{extension}."))
    }
}

fn parse_datetime(value: &str) -> Result<DateTime<FixedOffset>, String> {
    DateTime::parse_from_rfc3339(value).map_err(|_| {
        "Use data e hora ISO 8601 com fuso, por exemplo 2030-06-10T14:30:00-03:00.".into()
    })
}

fn escape_ical(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\n', "\\n")
}

fn fold_ical(line: &str) -> String {
    let mut out = String::new();
    let mut count = 0;
    for ch in line.chars() {
        let size = ch.len_utf8();
        if count + size > 75 {
            out.push_str("\r\n ");
            count = 1;
        }
        out.push(ch);
        count += size;
    }
    out.push_str("\r\n");
    out
}

fn create_ics(params: &Value) -> Result<String, String> {
    validate("calendar.create_event", params)?;
    let start = parse_datetime(string_param(params, "start")?)?.with_timezone(&Utc);
    let now = Utc::now();
    let uid_hash = format!("{:x}", sha2::Sha256::digest(start.to_rfc3339().as_bytes()));
    let uid = format!(
        "{}-{}@odekrai.local",
        now.timestamp_nanos_opt().unwrap_or_default(),
        &uid_hash[..16]
    );
    let mut lines = vec![
        "BEGIN:VCALENDAR".into(),
        "VERSION:2.0".into(),
        "PRODID:-//OdeKrai//Calendario 1.0//PT-BR".into(),
        "CALSCALE:GREGORIAN".into(),
        "BEGIN:VEVENT".into(),
        format!("UID:{uid}"),
        format!("DTSTAMP:{}", now.format("%Y%m%dT%H%M%SZ")),
        format!("DTSTART:{}", start.format("%Y%m%dT%H%M%SZ")),
        format!("SUMMARY:{}", escape_ical(string_param(params, "title")?)),
    ];
    if let Some(end) = params["end"].as_str() {
        lines.push(format!(
            "DTEND:{}",
            parse_datetime(end)?
                .with_timezone(&Utc)
                .format("%Y%m%dT%H%M%SZ")
        ));
    }
    let location = string_param(params, "location")?;
    let description = string_param(params, "description")?;
    if !location.is_empty() {
        lines.push(format!("LOCATION:{}", escape_ical(location)));
    }
    if !description.is_empty() {
        lines.push(format!("DESCRIPTION:{}", escape_ical(description)));
    }
    if let Some(minutes) = params["alert_minutes"].as_u64() {
        lines.extend([
            "BEGIN:VALARM".into(),
            "ACTION:DISPLAY".into(),
            format!(
                "DESCRIPTION:{}",
                escape_ical(string_param(params, "title")?)
            ),
            format!("TRIGGER:-PT{minutes}M"),
            "END:VALARM".into(),
        ]);
    }
    lines.extend(["END:VEVENT".into(), "END:VCALENDAR".into()]);
    Ok(lines.iter().map(|line| fold_ical(line)).collect())
}

fn create_csv(params: &Value) -> Result<Vec<u8>, String> {
    let delimiter = match string_param(params, "delimiter")? {
        "comma" => ',',
        "semicolon" => ';',
        "tab" => '\t',
        _ => return Err("Separador CSV inválido.".into()),
    };
    let mut text = String::new();
    let headers: Vec<&str> = array_param(params, "headers")?
        .iter()
        .map(|value| value.as_str().ok_or("Cabeçalho CSV inválido."))
        .collect::<Result<_, _>>()?;
    append_csv_row(&mut text, &headers, delimiter)?;
    for row in array_param(params, "rows")? {
        let values: Vec<&str> = row
            .as_array()
            .ok_or("Linha CSV inválida.")?
            .iter()
            .map(|value| value.as_str().ok_or("Valor de texto CSV inválido."))
            .collect::<Result<_, _>>()?;
        append_csv_row(&mut text, &values, delimiter)?;
    }
    if params["encoding"] == "utf8_bom" {
        let mut bytes = vec![0xef, 0xbb, 0xbf];
        bytes.extend_from_slice(text.as_bytes());
        return Ok(bytes);
    }
    encode_windows_1252(&text)
}

fn encode_windows_1252(text: &str) -> Result<Vec<u8>, String> {
    const EXTENDED: [Option<char>; 32] = [
        Some('€'),
        None,
        Some('‚'),
        Some('ƒ'),
        Some('„'),
        Some('…'),
        Some('†'),
        Some('‡'),
        Some('ˆ'),
        Some('‰'),
        Some('Š'),
        Some('‹'),
        Some('Œ'),
        None,
        Some('Ž'),
        None,
        None,
        Some('‘'),
        Some('’'),
        Some('“'),
        Some('”'),
        Some('•'),
        Some('–'),
        Some('—'),
        Some('˜'),
        Some('™'),
        Some('š'),
        Some('›'),
        Some('œ'),
        None,
        Some('ž'),
        Some('Ÿ'),
    ];
    text.chars()
        .map(|ch| {
            let code = ch as u32;
            if code <= 0x7f || (0xa0..=0xff).contains(&code) {
                return Ok(code as u8);
            }
            EXTENDED
                .iter()
                .position(|entry| *entry == Some(ch))
                .map(|index| 0x80 + index as u8)
                .ok_or_else(|| {
                    "O conteúdo não pode ser representado em Windows-1252; escolha UTF-8 com BOM."
                        .to_string()
                })
        })
        .collect()
}

fn append_csv_row(out: &mut String, values: &[&str], delimiter: char) -> Result<(), String> {
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            out.push(delimiter);
        }
        let value = if value
            .trim_start()
            .chars()
            .next()
            .is_some_and(|ch| matches!(ch, '=' | '+' | '-' | '@'))
        {
            format!("'{}", value)
        } else {
            value.to_string()
        };
        if value.contains(delimiter)
            || value.contains('"')
            || value.chars().any(|ch| matches!(ch, '\n' | '\r'))
        {
            out.push('"');
            out.push_str(&value.replace('"', "\"\""));
            out.push('"');
        } else {
            out.push_str(&value);
        }
    }
    out.push_str("\r\n");
    if out.len() > MAX_TEXT_BYTES {
        return Err("O CSV excede 1 MB.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn creates_rfc5545_utc_ics_with_crlf_escaping_and_folding() {
        let params = json!({"title":"Reunião, equipe; \"A\"","start":"2030-06-10T14:30:00-03:00","end":"2030-06-10T15:30:00-03:00","location":"São Paulo","description":"linha 1\nlinha 2 ".repeat(8),"alert_minutes":15,"recurrence":"none"});
        let ics = create_ics(&params).unwrap();
        assert!(ics.starts_with("BEGIN:VCALENDAR\r\n"));
        assert!(ics.contains("DTSTART:20300610T173000Z\r\n"));
        assert!(ics.contains("DTEND:20300610T183000Z\r\n"));
        assert!(ics.contains("UID:"));
        assert!(ics.contains("TRIGGER:-PT15M\r\n"));
        assert!(ics.contains("LOCATION:São Paulo\r\n"));
        assert!(ics.contains("SUMMARY:Reunião\\, equipe\\; \"A\"\r\n"));
        assert!(ics.contains("\r\n "));
        assert!(!ics.replace("\r\n", "").contains(['\r', '\n']));
        assert!(ics.ends_with("END:VCALENDAR\r\n"));
        assert!(ics.lines().all(|line| line.len() <= 75));
    }

    #[test]
    fn rejects_events_without_timezone_or_with_non_increasing_end() {
        let mut p = json!({"title":"x","start":"2030-01-01T10:00:00-03:00","end":"2030-01-01T09:00:00-03:00","location":"","description":"","alert_minutes":null,"recurrence":"none"});
        assert!(validate("calendar.create_event", &p).is_err());
        p["start"] = json!("2030-01-01T10:00:00");
        p["end"] = Value::Null;
        assert!(validate("calendar.create_event", &p)
            .unwrap_err()
            .contains("pergunte o horário de término"));
    }

    #[test]
    fn csv_preserves_portuguese_accents_and_escapes_delimiters_for_excel() {
        let p = json!({"path":"x.csv","delimiter":"semicolon","encoding":"utf8_bom","headers":["nome","observação"],"rows":[["João","vírgula, e \"aspas\""]]});
        let bytes = create_csv(&p).unwrap();
        assert_eq!(&bytes[..3], &[0xef, 0xbb, 0xbf]);
        let text = std::str::from_utf8(&bytes[3..]).unwrap();
        assert!(text.contains("observação\r\nJoão;\"vírgula, e \"\"aspas\"\"\"\r\n"));
        assert!(create_csv(&json!({"path":"x.csv","delimiter":"semicolon","encoding":"utf8_bom","headers":["formula"],"rows":[["=2+2"]]})).unwrap().windows(5).any(|part| part == b"'=2+2"));
    }

    #[test]
    fn windows_1252_csv_rejects_unrepresentable_characters() {
        assert_eq!(
            encode_windows_1252("ação €").unwrap(),
            vec![0x61, 0xe7, 0xe3, 0x6f, 0x20, 0x80]
        );
        assert!(encode_windows_1252("emoji 😀").is_err());
    }

    #[test]
    fn malformed_csv_delimiter_returns_an_error_instead_of_panicking() {
        let error = create_csv(&json!({"delimiter":"invalid","headers":[],"rows":[]})).unwrap_err();
        assert!(error.contains("Separador CSV inválido"));
    }

    #[test]
    fn productivity_tools_reject_unrecognized_fields_in_rust() {
        let p = json!({"path":"x.txt","content":"ok","risk":"destructive"});
        assert!(validate("doc.create_text", &p).is_err());
    }

    #[test]
    fn sheet_read_accepts_absolute_path_shape_for_allowed_folder_resolution() {
        let path = std::env::temp_dir().join("read.xlsx");
        let params = json!({"path":path.to_string_lossy(),"max_rows":10});
        assert!(validate("sheet.read", &params).is_ok());
    }

    #[test]
    fn csv_creation_is_new_only_and_writes_into_the_selected_root() {
        let root = std::env::temp_dir().join(format!(
            "odekrai-csv-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        let params = json!({"path":"report.csv","delimiter":"comma","encoding":"utf8_bom","headers":["name"],"rows":[["Ana"]]});
        assert!(execute("sheet.create_csv", &params, &root, None).is_ok());
        let bytes = fs::read(root.join("report.csv")).unwrap();
        assert!(bytes.starts_with(&[0xef, 0xbb, 0xbf]));
        assert!(execute("sheet.create_csv", &params, &root, None).is_err());
        let _ = fs::remove_dir_all(root);
    }
}
