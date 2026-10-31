use serde_json::{json, Value};
use std::{fs, io::Write, path::Path};

const MAX_NOTE_BYTES: usize = 8 * 1024;
const MAX_SEARCH_RESULTS: usize = 20;

fn string_param<'a>(params: &'a Value, key: &str) -> Result<&'a str, String> {
    params[key]
        .as_str()
        .ok_or_else(|| format!("O parâmetro {key} da memória é inválido."))
}

fn object_keys(value: &Value, allowed: &[&str]) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or("Parâmetros de memória inválidos.")?;
    if object.keys().any(|key| !allowed.contains(&key.as_str())) || object.len() != allowed.len() {
        return Err("Os parâmetros de memória não correspondem ao esquema permitido.".into());
    }
    Ok(())
}

fn valid_cpf(digits: &str) -> bool {
    if digits.len() != 11
        || digits
            .chars()
            .all(|digit| digit == digits.chars().next().unwrap_or('0'))
    {
        return false;
    }
    let values: Vec<u32> = digits.bytes().map(|byte| (byte - b'0') as u32).collect();
    for size in [9, 10] {
        let sum: u32 = values[..size]
            .iter()
            .enumerate()
            .map(|(index, value)| value * (size as u32 + 1 - index as u32))
            .sum();
        let check = (sum * 10 % 11) % 10;
        if values[size] != check {
            return false;
        }
    }
    true
}

fn valid_cnpj(digits: &str) -> bool {
    if digits.len() != 14
        || digits
            .chars()
            .all(|digit| digit == digits.chars().next().unwrap_or('0'))
    {
        return false;
    }
    let values: Vec<u32> = digits.bytes().map(|byte| (byte - b'0') as u32).collect();
    for (size, weights) in [
        (12, &[5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2][..]),
        (13, &[6, 5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2][..]),
    ] {
        let sum: u32 = values[..size]
            .iter()
            .zip(weights)
            .map(|(value, weight)| value * *weight)
            .sum();
        let remainder = sum % 11;
        let check = if remainder < 2 { 0 } else { 11 - remainder };
        if values[size] != check {
            return false;
        }
    }
    true
}

fn valid_card(digits: &str) -> bool {
    if !(13..=19).contains(&digits.len()) {
        return false;
    }
    let mut sum = 0;
    let mut double = false;
    for byte in digits.bytes().rev() {
        let mut digit = (byte - b'0') as u32;
        if double {
            digit *= 2;
            if digit > 9 {
                digit -= 9;
            }
        }
        sum += digit;
        double = !double;
    }
    sum % 10 == 0
}

fn has_document_or_card(text: &str) -> bool {
    let mut digits = String::new();
    let matches = |digits: &str| valid_cpf(digits) || valid_cnpj(digits) || valid_card(digits);
    for ch in text.chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
        } else if !digits.is_empty() && !" .-/()".contains(ch) {
            if matches(&digits) {
                return true;
            }
            digits.clear();
        }
    }
    matches(&digits)
}

pub(crate) fn contains_sensitive_data(text: &str) -> bool {
    crate::security::workspace::contains_secret_marker(text) || has_document_or_card(text)
}

pub(crate) fn validate_params(skill_id: &str, params: &Value) -> Result<(), String> {
    match skill_id {
        "memory.list" => object_keys(params, &[]),
        "memory.search" => {
            object_keys(params, &["query"])?;
            let query = params["query"]
                .as_str()
                .ok_or("Informe o termo de busca.")?;
            if query.trim().is_empty() || query.chars().count() > 256 {
                return Err("Termo de busca inválido.".into());
            }
            Ok(())
        }
        "memory.propose_note" => {
            object_keys(params, &["name", "text"])?;
            let name = params["name"].as_str().ok_or("Nome da nota inválido.")?;
            crate::agent_home::memory_name(name)?;
            let text = params["text"].as_str().ok_or("Texto da nota inválido.")?;
            if text.trim().is_empty() || text.len() > MAX_NOTE_BYTES {
                return Err("A nota precisa ter conteúdo e até 8 KiB.".into());
            }
            if contains_sensitive_data(text) || contains_sensitive_data(name) {
                return Err("A proposta contém dado confidencial (chave, documento ou cartão) e não pode ser salva na memória.".into());
            }
            Ok(())
        }
        _ => Err("Habilidade de memória desconhecida.".into()),
    }
}

fn names(directory: &Path) -> Result<Vec<String>, String> {
    let mut names = Vec::new();
    for entry in
        fs::read_dir(directory).map_err(|_| "Não foi possível listar a memória.".to_string())?
    {
        let entry = entry.map_err(|_| "Não foi possível listar a memória.".to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if crate::agent_home::memory_name(&name).is_ok()
            && entry.file_type().is_ok_and(|kind| kind.is_file())
        {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

fn note_content(directory: &Path, name: &str) -> Result<String, String> {
    crate::agent_home::memory_name(name)?;
    let path = directory.join(name);
    if crate::security::workspace::has_reparse(&path) {
        return Err("Arquivo de memória redirecionado.".into());
    }
    let bytes = fs::read(path).map_err(|_| "Não foi possível ler a memória.".to_string())?;
    if bytes.len() > MAX_NOTE_BYTES {
        return Err("Nota de memória excede 8 KiB.".into());
    }
    String::from_utf8(bytes).map_err(|_| "Nota de memória não está em UTF-8.".into())
}

fn search(directory: &Path, query: &str) -> Result<Value, String> {
    let needle = query.to_lowercase();
    let mut found = Vec::new();
    for name in names(directory)? {
        let content = note_content(directory, &name)?;
        if contains_sensitive_data(&content) {
            return Err(
                "A busca foi interrompida porque uma nota contém dado confidencial reconhecido."
                    .into(),
            );
        }
        if content.to_lowercase().contains(&needle) {
            let excerpt: String = content.chars().take(1200).collect();
            found.push(json!({"name":name,"excerpt":excerpt}));
            if found.len() >= MAX_SEARCH_RESULTS {
                break;
            }
        }
    }
    Ok(Value::Array(found))
}

fn write_new_note(directory: &Path, name: &str, text: &str) -> Result<(), String> {
    crate::agent_home::memory_name(name)?;
    if contains_sensitive_data(text) {
        return Err("A proposta contém dado confidencial (chave, documento ou cartão) e não pode ser salva na memória.".into());
    }
    if text.trim().is_empty() || text.len() > MAX_NOTE_BYTES {
        return Err("A nota precisa ter conteúdo e até 8 KiB.".into());
    }
    let path = directory.join(name);
    if crate::security::workspace::has_reparse(&path) {
        return Err("Arquivo de memória redirecionado.".into());
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| "Não foi possível criar a nota; o nome pode já existir.".to_string())?;
    file.write_all(text.as_bytes())
        .and_then(|_| file.sync_data())
        .map_err(|_| "Não foi possível gravar a nota aprovada.".to_string())
}

pub(crate) fn preview(skill_id: &str, params: &Value) -> Result<String, String> {
    validate_params(skill_id, params)?;
    match skill_id {
        "memory.propose_note" => Ok(format!(
            "Gravar em memory/{} após aprovação explícita. Texto exato:\n\n{}",
            string_param(params, "name")?,
            string_param(params, "text")?
        )),
        "memory.list" => Ok("Listar nomes das notas da memória manual local.".into()),
        "memory.search" => Ok(format!(
            "Pesquisar na memória manual pelo termo: {}",
            string_param(params, "query")?
        )),
        _ => Err("Habilidade de memória desconhecida.".into()),
    }
}

pub(crate) fn execute(
    app: &tauri::AppHandle,
    skill_id: &str,
    params: &Value,
) -> Result<String, String> {
    validate_params(skill_id, params)?;
    let directory = crate::agent_home::memory_dir(app)?;
    match skill_id {
        "memory.list" => serde_json::to_string(&names(&directory)?)
            .map_err(|_| "Não foi possível preparar a lista de memória.".into()),
        "memory.search" => {
            serde_json::to_string(&search(&directory, string_param(params, "query")?)?)
                .map_err(|_| "Não foi possível preparar a busca de memória.".into())
        }
        "memory.propose_note" => {
            write_new_note(
                &directory,
                string_param(params, "name")?,
                string_param(params, "text")?,
            )?;
            Ok(format!(
                "Nota aprovada gravada em memory/{}.",
                string_param(params, "name")?
            ))
        }
        _ => Err("Habilidade de memória desconhecida.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> std::path::PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "odekrai-memory-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&directory).unwrap();
        directory
    }

    #[test]
    fn refuses_secrets_documents_and_payment_cards_in_notes() {
        for text in [
            "api_key=secret",
            "CPF 529.982.247-25",
            "cartão 4111 1111 1111 1111",
        ] {
            assert!(contains_sensitive_data(text), "{text}");
            assert!(write_new_note(&temp_dir(), "nota.md", text).is_err());
        }
        let directory = temp_dir();
        write_new_note(&directory, "nota.md", "Preferência por respostas curtas.").unwrap();
        assert!(write_new_note(&directory, "nota.md", "Sobrescrita").is_err());
        assert_eq!(
            note_content(&directory, "nota.md").unwrap(),
            "Preferência por respostas curtas."
        );
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn approval_preview_contains_exact_note_text() {
        let text = "Usar respostas curtas.\nNão guardar credenciais.";
        let preview = preview(
            "memory.propose_note",
            &json!({"name":"preferencias.md","text":text}),
        )
        .unwrap();
        assert!(preview.ends_with(text));
    }

    #[test]
    fn malformed_memory_preview_returns_a_validation_error() {
        let error = preview("memory.propose_note", &json!({})).unwrap_err();
        assert!(error.contains("não correspondem ao esquema permitido"));
    }

    #[test]
    fn search_does_not_return_notes_with_recognized_sensitive_values() {
        let directory = temp_dir();
        fs::write(directory.join("nota.md"), "CPF 529.982.247-25").unwrap();
        assert!(search(&directory, "CPF").is_err());
        let _ = fs::remove_dir_all(directory);
    }
}
