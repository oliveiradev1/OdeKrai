use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::PathBuf,
    sync::{Mutex, OnceLock},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use tauri::{AppHandle, Manager, State, WebviewWindow};

pub struct StartupState {
    previous_run_incomplete: bool,
    marker: PathBuf,
    started: Instant,
}

impl StartupState {
    pub fn safe_fallback() -> Self {
        Self {
            previous_run_incomplete: true,
            marker: PathBuf::new(),
            started: Instant::now(),
        }
    }
}

pub fn setup(app: &AppHandle) -> Result<StartupState, Box<dyn std::error::Error>> {
    let app_data = app.path().app_data_dir()?;
    let logs = app_data.join("logs");
    fs::create_dir_all(&logs)?;
    let diagnostic_log = logs.join("diagnostic.log");
    install_panic_hook(diagnostic_log.clone());
    let marker = app_data.join("initialization-status.txt");
    let marker_exists = marker.exists();
    let contents = if marker_exists {
        fs::read(&marker)?
    } else {
        Vec::new()
    };
    let previous_run_incomplete = previous_run_incomplete(marker_exists, &contents);
    fs::write(&marker, b"incomplete")?;
    write_log(&diagnostic_log, "setup_started", "")?;
    write_log(
        &diagnostic_log,
        "build",
        &format!(
            "version={} commit={} date={} webview_version={}",
            env!("CARGO_PKG_VERSION"),
            option_env!("ODEKRAI_BUILD_COMMIT").unwrap_or("local"),
            option_env!("ODEKRAI_BUILD_DATE").unwrap_or("não definida"),
            tauri::webview_version().unwrap_or_else(|_| "unavailable".into())
        ),
    )?;
    Ok(StartupState {
        previous_run_incomplete,
        marker,
        started: Instant::now(),
    })
}

fn install_panic_hook(diagnostic_log: PathBuf) {
    std::panic::set_hook(Box::new(move |panic| {
        let payload_type = if panic.payload().is::<String>() || panic.payload().is::<&'static str>()
        {
            "string"
        } else {
            "other"
        };
        let location = panic.location();
        let record = format_panic_record(
            payload_type,
            location.map(std::panic::Location::file),
            location.map(std::panic::Location::line),
            location.map(std::panic::Location::column),
        );
        let _ = write_log(&diagnostic_log, "panic", &record);
    }));
}

fn format_panic_record(
    payload_type: &str,
    source_file: Option<&str>,
    line: Option<u32>,
    column: Option<u32>,
) -> String {
    let payload_type = if payload_type == "string" {
        "string"
    } else {
        "other"
    };
    let source = source_file
        .and_then(|file| file.rsplit(['/', '\\']).next())
        .filter(|file| {
            !file.is_empty()
                && file.len() <= 128
                && file
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || "._-".contains(character))
        })
        .unwrap_or("unknown");
    format!(
        "panic_type={payload_type} source={source} line={} column={}",
        line.unwrap_or_default(),
        column.unwrap_or_default()
    )
}

pub fn log_event(app: &AppHandle, event: &str, command: &str, outcome: &str, duration_ms: u64) {
    let Ok(logs) = app.path().app_data_dir().map(|path| path.join("logs")) else {
        return;
    };
    let fields = format!(
        "event={} command={} result={} duration_ms={duration_ms}",
        safe_token(event),
        safe_token(command),
        safe_token(outcome),
    );
    let _ = write_log(&logs.join("diagnostic.log"), "frontend_event", &fields);
}

pub fn log_agent_error(app: &AppHandle, kind: &str, technical_details: &str) {
    let Ok(logs) = app.path().app_data_dir().map(|path| path.join("logs")) else {
        return;
    };
    let fields = format!("kind={} details={technical_details}", safe_token(kind),);
    let _ = write_log(&logs.join("diagnostic.log"), "agent_error", &fields);
}

pub fn log_setup_complete(app: &AppHandle) {
    let duration_ms = app
        .try_state::<StartupState>()
        .map(|state| state.started.elapsed().as_millis().min(u64::MAX as u128) as u64)
        .unwrap_or_default();
    log_event(app, "setup_complete", "", "ok", duration_ms);
}

fn write_log(path: &PathBuf, event: &str, details: &str) -> io::Result<()> {
    static LOG_WRITE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let _guard = LOG_WRITE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let timestamp = timestamp_utc(SystemTime::now());
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    let user_profile = std::env::var("USERPROFILE").ok();
    let details = sanitize_log_details(details, user_profile.as_deref());
    writeln!(file, "{timestamp} {event} {details}")
}

fn timestamp_utc(now: SystemTime) -> String {
    let elapsed = now.duration_since(UNIX_EPOCH).unwrap_or_default();
    let days = (elapsed.as_secs() / 86_400) as i64;
    let seconds_in_day = elapsed.as_secs() % 86_400;
    let shifted_days = days + 719_468;
    let era = if shifted_days >= 0 {
        shifted_days
    } else {
        shifted_days - 146_096
    } / 146_097;
    let day_of_era = shifted_days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_part = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_part + 2) / 5 + 1;
    let month = month_part + if month_part < 10 { 3 } else { -9 };
    if month <= 2 {
        year += 1;
    }
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        seconds_in_day / 3_600,
        (seconds_in_day % 3_600) / 60,
        seconds_in_day % 60,
        elapsed.subsec_millis()
    )
}

fn safe_token(input: &str) -> String {
    input
        .chars()
        .take(64)
        .map(|character| {
            if character.is_ascii_alphanumeric() || "_.-".contains(character) {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn sanitize_log_details(input: &str, user_profile: Option<&str>) -> String {
    let mut details: String = input
        .chars()
        .take(512)
        .map(|character| {
            if character.is_control() {
                '_'
            } else {
                character
            }
        })
        .collect();

    if let Some(profile) = user_profile.filter(|profile| !profile.is_empty()) {
        details = replace_user_profile(&details, profile);
    }

    for marker in [
        "Bearer ",
        "sk-",
        "pplx-",
        "api_key=",
        "api-key=",
        "token=",
        "client_secret=",
        "password=",
        "authorization:",
    ] {
        details = redact_marker_values(&details, marker);
    }
    redact_email_addresses(&details)
}

fn replace_user_profile(input: &str, user_profile: &str) -> String {
    let variants = [
        user_profile.to_string(),
        user_profile.replace('\\', "/"),
        user_profile.replace('/', "\\"),
    ];
    let mut details = input.to_string();
    for variant in variants {
        details = replace_case_insensitive(&details, &variant, "%USERPROFILE%");
    }
    details
}

fn replace_case_insensitive(input: &str, needle: &str, replacement: &str) -> String {
    if needle.is_empty() {
        return input.to_string();
    }
    let needle_lower = needle.to_ascii_lowercase();
    let mut output = input.to_string();
    let mut search_from = 0;
    while let Some(relative_start) = output[search_from..]
        .to_ascii_lowercase()
        .find(&needle_lower)
    {
        let start = search_from + relative_start;
        let end = start + needle.len();
        output.replace_range(start..end, replacement);
        search_from = start + replacement.len();
        if search_from >= output.len() {
            break;
        }
    }
    output
}

fn redact_marker_values(input: &str, marker: &str) -> String {
    let marker_lower = marker.to_ascii_lowercase();
    let mut output = input.to_string();
    let mut search_from = 0;
    loop {
        let lower = output.to_ascii_lowercase();
        let Some(relative_start) = lower[search_from..].find(&marker_lower) else {
            break;
        };
        let start = search_from + relative_start;
        let mut end = start + marker.len();
        while end < output.len()
            && !output.as_bytes()[end].is_ascii_whitespace()
            && !b",;\"'<>)]}".contains(&output.as_bytes()[end])
        {
            end += 1;
        }
        output.replace_range(start..end, "[credencial removida]");
        search_from = start + "[credencial removida]".len();
        if search_from >= output.len() {
            break;
        }
    }
    output
}

fn redact_email_addresses(input: &str) -> String {
    let mut output = input.to_string();
    let mut search_from = 0;
    loop {
        let lower = output.to_ascii_lowercase();
        let Some(relative_at) = lower[search_from..].find('@') else {
            break;
        };
        let at = search_from + relative_at;
        let bytes = output.as_bytes();
        let mut start = at;
        while start > 0 && !is_email_boundary(bytes[start - 1]) {
            start -= 1;
        }
        let mut end = at + 1;
        while end < bytes.len() && !is_email_boundary(bytes[end]) {
            end += 1;
        }
        output.replace_range(start..end, "[email removido]");
        search_from = start + "[email removido]".len();
        if search_from >= output.len() {
            break;
        }
    }
    output
}

fn is_email_boundary(byte: u8) -> bool {
    byte.is_ascii_whitespace() || b",;()[]<>\"'".contains(&byte)
}

fn is_safe_diagnostic(event: &str, command: &str, outcome: &str) -> bool {
    let event_allowed = matches!(
        event,
        "startup_command_start"
            | "startup_command_finish"
            | "frontend_window_error"
            | "frontend_unhandled_rejection"
    );
    let command_allowed = matches!(
        command,
        "get_build_info"
            | "set_close_behavior"
            | "get_startup_state"
            | "get_selected_pet"
            | "get_permission_settings"
            | "get_profile"
            | "agent_home_status"
            | "list_conversations"
            | "has_provider_credential"
            | "load_conversation"
            | "agent_home_read_core"
            | "save_profile"
            | "set_pet_position"
            | "set_pet_topmost"
            | "set_pet_visible"
            | "set_pet_size"
            | "restore_pet"
            | "complete_startup"
            | "window.onerror"
            | "unhandledrejection"
    );
    event_allowed && command_allowed && matches!(outcome, "ok" | "warning" | "error")
}

fn previous_run_incomplete(marker_exists: bool, contents: &[u8]) -> bool {
    marker_exists && contents != b"complete"
}

#[tauri::command]
pub async fn get_startup_state(
    caller: WebviewWindow,
    state: State<'_, StartupState>,
) -> Result<bool, String> {
    crate::require_main(&caller)?;
    Ok(state.previous_run_incomplete)
}

#[tauri::command]
pub async fn complete_startup(
    app: AppHandle,
    caller: WebviewWindow,
    state: State<'_, StartupState>,
) -> Result<(), String> {
    crate::require_main(&caller)?;
    let marker = state.marker.clone();
    let duration_ms = state.started.elapsed().as_millis().min(u64::MAX as u128) as u64;
    tauri::async_runtime::spawn_blocking(move || fs::write(marker, b"complete"))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    log_event(&app, "startup_complete", "", "ok", duration_ms);
    Ok(())
}

// Os parâmetros compõem o contrato IPC consumido pelo frontend.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn record_diagnostic_event(
    app: AppHandle,
    caller: WebviewWindow,
    event: String,
    command: String,
    outcome: String,
    duration_ms: u64,
    section: Option<String>,
    category: Option<String>,
    correlation_id: Option<String>,
) -> Result<(), String> {
    crate::require_main(&caller)?;
    if !is_safe_diagnostic(&event, &command, &outcome)
        || !section.as_deref().is_none_or(is_safe_section)
        || !category.as_deref().is_none_or(is_safe_category)
        || !correlation_id.as_deref().is_none_or(is_safe_correlation_id)
    {
        return Err("Evento de diagnóstico inválido.".into());
    }
    let event = safe_token(&event);
    let command = safe_token(&command);
    let outcome = safe_token(&outcome);
    let section = section.as_deref().map(safe_token).unwrap_or_default();
    let category = category.as_deref().map(safe_token).unwrap_or_default();
    let correlation_id = correlation_id
        .as_deref()
        .map(safe_token)
        .unwrap_or_default();
    tauri::async_runtime::spawn_blocking(move || {
        let Ok(logs) = app.path().app_data_dir().map(|path| path.join("logs")) else {
            return;
        };
        let line_kind = match outcome.as_str() {
            "error" => "diagnostic_error",
            "warning" => "diagnostic_warning",
            _ => "frontend_event",
        };
        let fields = format!(
            "event={event} command={command} section={section} category={category} correlation_id={correlation_id} result={outcome} duration_ms={duration_ms}"
        );
        let _ = write_log(&logs.join("diagnostic.log"), line_kind, &fields);
    })
    .await
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn is_safe_section(value: &str) -> bool {
    matches!(
        value,
        "startup" | "profile" | "permissions" | "chat" | "pet" | "window"
    )
}

fn is_safe_category(value: &str) -> bool {
    matches!(
        value,
        "initialization" | "event_listener" | "window_error" | "promise_rejection" | "preference"
    )
}

fn is_safe_correlation_id(value: &str) -> bool {
    value.len() == 36
        && value.chars().enumerate().all(|(index, character)| {
            if [8, 13, 18, 23].contains(&index) {
                character == '-'
            } else {
                character.is_ascii_hexdigit()
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panic_record_includes_safe_type_and_location_without_payload_or_user_path() {
        let record = format_panic_record(
            "string",
            Some(r"C:\Users\private-user\source\main.rs"),
            Some(42),
            Some(7),
        );
        assert_eq!(record, "panic_type=string source=main.rs line=42 column=7");
        assert!(!record.contains("private-user"));
        assert!(!record.contains("secret-token"));
    }

    #[test]
    fn safe_tokens_remove_payload_characters_and_limit_length() {
        assert_eq!(safe_token("get_profile: segredo"), "get_profile__segredo");
        assert_eq!(safe_token(&"x".repeat(100)).len(), 64);
    }

    #[test]
    fn diagnostic_events_reject_free_text() {
        assert!(is_safe_diagnostic(
            "startup_command_start",
            "get_profile",
            "ok"
        ));
        assert!(is_safe_diagnostic(
            "startup_command_finish",
            "get_profile",
            "error"
        ));
        assert!(is_safe_diagnostic(
            "startup_command_finish",
            "save_profile",
            "error"
        ));

        let untrusted_values = [
            "texto da conversa".to_string(),
            "x".repeat(600),
            "sk-example-secret-key".to_string(),
            "pplx-example-secret-key".to_string(),
            "Bearer example-secret-token".to_string(),
            "ana@example.com".to_string(),
            r"C:\Users\Daniel\Documents\conversa.txt".to_string(),
        ];
        for value in untrusted_values {
            assert!(!is_safe_diagnostic("startup_command_start", &value, "ok"));
            assert!(!is_safe_diagnostic(
                "startup_command_finish",
                "get_profile",
                &value
            ));
            assert!(!is_safe_diagnostic(&value, "get_profile", "ok"));
        }
    }

    #[test]
    fn diagnostic_log_redacts_credentials_emails_and_profile_paths() {
        let details = sanitize_log_details(
            "Bearer bearer-secret sk-api-secret pplx-api-secret api_key=key-secret token=token-secret client_secret=oauth-secret password=private-password ana@example.com C:\\Users\\Daniel\\AppData\\Local\\OdeKrai",
            Some(r"C:\Users\Daniel"),
        );

        for secret in [
            "bearer-secret",
            "sk-api-secret",
            "pplx-api-secret",
            "key-secret",
            "token-secret",
            "oauth-secret",
            "private-password",
            "ana@example.com",
            "C:\\Users\\Daniel",
        ] {
            assert!(
                !details.contains(secret),
                "valor sensível permaneceu: {secret}"
            );
        }
        assert!(details.contains("%USERPROFILE%\\AppData\\Local\\OdeKrai"));
        assert!(sanitize_log_details(&"x".repeat(600), None).len() <= 512);
    }

    #[test]
    fn initialization_marker_distinguishes_complete_and_incomplete() {
        assert!(!previous_run_incomplete(false, b""));
        assert!(!previous_run_incomplete(true, b"complete"));
        assert!(previous_run_incomplete(true, b"incomplete"));
    }

    #[test]
    fn log_timestamp_is_readable_utc() {
        assert_eq!(timestamp_utc(UNIX_EPOCH), "1970-01-01T00:00:00.000Z");
    }

    #[test]
    fn safe_context_rejects_free_text_and_accepts_uuid_correlation_ids() {
        assert!(is_safe_section("chat"));
        assert!(!is_safe_section("mensagem privada"));
        assert!(is_safe_category("promise_rejection"));
        assert!(is_safe_correlation_id(
            "123e4567-e89b-12d3-a456-426614174000"
        ));
        assert!(!is_safe_correlation_id("api_key=segredo"));
    }

    #[test]
    fn concurrent_log_writes_remain_one_complete_event_per_line() {
        let path = std::env::temp_dir().join(format!(
            "odekrai-diagnostics-{}-{}.log",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let writers: Vec<_> = (0..32)
            .map(|index| {
                let path = path.clone();
                std::thread::spawn(move || {
                    write_log(&path, "test_event", &format!("writer={index} result=ok")).unwrap();
                })
            })
            .collect();
        for writer in writers {
            writer.join().unwrap();
        }
        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(contents.lines().count(), 32);
        assert!(contents
            .lines()
            .all(|line| line.contains("test_event writer=") && line.ends_with(" result=ok")));
        fs::remove_file(path).unwrap();
    }
}
