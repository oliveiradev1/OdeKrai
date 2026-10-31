use rusqlite::OptionalExtension;
use serde_json::Value;
use std::path::{Path, PathBuf};
use tauri_plugin_opener::OpenerExt;
use url::Url;

fn string_param<'a>(params: &'a Value, key: &str) -> Result<&'a str, String> {
    params[key]
        .as_str()
        .ok_or_else(|| format!("O parâmetro {key} da habilidade do sistema é inválido."))
}

const SETTINGS_PAGES: &[(&str, &str)] = &[
    ("som", "ms-settings:sound"),
    ("tela", "ms-settings:display"),
    ("rede", "ms-settings:network-status"),
    ("bluetooth", "ms-settings:bluetooth"),
    ("notificacoes", "ms-settings:privacy-notifications"),
    ("aplicativos", "ms-settings:appsfeatures"),
    ("privacidade", "ms-settings:privacy"),
    ("camera", "ms-settings:privacy-webcam"),
    ("microfone", "ms-settings:privacy-microphone"),
    ("localizacao", "ms-settings:privacy-location"),
    ("bateria", "ms-settings:batterysaver"),
    ("sobre", "ms-settings:about"),
];

pub(crate) fn validate(skill_id: &str, params: &Value) -> Result<(), String> {
    let object = params.as_object().ok_or("Parâmetros inválidos.")?;
    let expected: &[&str] = match skill_id {
        "audio.get_volume"
        | "media.play_pause"
        | "media.next"
        | "media.previous"
        | "wallpaper.restore"
        | "power.lock"
        | "display.get_brightness"
        | "display.restore_brightness"
        | "power.sleep"
        | "theme.restore" => &[],
        "audio.set_volume" => &["volume"],
        "audio.mute" => &["muted"],
        "wallpaper.set" => &["path"],
        "display.set_brightness" => &["brightness"],
        "theme.set" => &["mode"],
        "system.open_url" => &["url"],
        "system.open_path" | "system.reveal_in_folder" => &["path"],
        "settings.open" => &["page"],
        "notification.show" => &["title", "body"],
        "app.launch" => &["app_id"],
        "clipboard.write" => &["text"],
        "clipboard.read" => &[],
        "system.info" => &[],
        _ => return Err("Habilidade do sistema desconhecida.".into()),
    };
    let restore = matches!(
        skill_id,
        "wallpaper.restore" | "display.restore_brightness" | "theme.restore"
    );
    let keys_valid = object
        .keys()
        .all(|key| expected.contains(&key.as_str()) || (restore && key == "history_id"));
    if !keys_valid || (!restore && object.len() != expected.len()) || (restore && object.len() > 1)
    {
        return Err("Os parâmetros não correspondem à habilidade permitida.".into());
    }
    if restore
        && object.get("history_id").is_some_and(|value| {
            value.as_str().is_none_or(|id| {
                id.is_empty() || id.len() > 128 || id.chars().any(char::is_control)
            })
        })
    {
        return Err("O identificador do estado anterior é inválido.".into());
    }
    match skill_id {
        "audio.get_volume"
        | "media.play_pause"
        | "media.next"
        | "media.previous"
        | "wallpaper.restore"
        | "power.lock"
        | "display.get_brightness"
        | "display.restore_brightness"
        | "power.sleep"
        | "theme.restore" => {}
        "wallpaper.set" => {
            let path = bounded_string(object, "path", 32767)?;
            if Path::new(path)
                .extension()
                .and_then(|ext| ext.to_str())
                .is_none_or(|ext| {
                    !["png", "jpg", "bmp"]
                        .iter()
                        .any(|valid| ext.eq_ignore_ascii_case(valid))
                })
            {
                return Err("Formato inválido. Use uma imagem PNG, JPG ou BMP.".into());
            }
        }
        "display.set_brightness" => {
            if object
                .get("brightness")
                .and_then(Value::as_u64)
                .is_none_or(|value| value > 100)
            {
                return Err("O brilho precisa ser um número inteiro entre 0 e 100.".into());
            }
        }
        "theme.set" => {
            if !matches!(
                object.get("mode").and_then(Value::as_str),
                Some("claro" | "escuro")
            ) {
                return Err("O tema precisa ser claro ou escuro.".into());
            }
        }
        "audio.set_volume" => {
            if object
                .get("volume")
                .and_then(Value::as_u64)
                .is_none_or(|value| value > 100)
            {
                return Err("O volume precisa ser um número inteiro entre 0 e 100.".into());
            }
        }
        "audio.mute" => {
            if object.get("muted").and_then(Value::as_bool).is_none() {
                return Err("Informe se o áudio deve ficar sem som.".into());
            }
        }
        "system.open_url" => {
            let raw = bounded_string(object, "url", 2048)?;
            let normalized =
                crate::external_links::validate_external_url(raw).map_err(str::to_string)?;
            let url = Url::parse(&normalized).map_err(|_| "URL inválida.")?;
            if !matches!(url.scheme(), "http" | "https") {
                return Err("Somente URLs HTTP e HTTPS podem ser abertas.".into());
            }
        }
        "system.open_path" | "system.reveal_in_folder" => {
            let raw = bounded_string(object, "path", 32767)?;
            if Path::new(raw).is_absolute() {
                crate::security::skills::validate_raw_folder_path(raw)?;
            } else {
                crate::security::workspace::relative_path(raw)?;
            }
            if blocked_extension(Path::new(raw)) {
                return Err("Arquivos executáveis e scripts não podem ser abertos.".into());
            }
        }
        "settings.open" => {
            let page = bounded_string(object, "page", 40)?;
            if !SETTINGS_PAGES.iter().any(|(key, _)| *key == page) {
                return Err("Esta página de Configurações não está permitida.".into());
            }
        }
        "notification.show" => {
            if bounded_string(object, "title", 120)?.trim().is_empty()
                || bounded_string(object, "body", 2000)?.trim().is_empty()
            {
                return Err("Informe o título e o texto da notificação.".into());
            }
        }
        "app.launch" => {
            let id = bounded_string(object, "app_id", 24)?;
            if !crate::security::skills::valid_app_id(id) {
                return Err("Identificador de aplicativo inválido.".into());
            }
        }
        "clipboard.write" => {
            let text = object
                .get("text")
                .and_then(Value::as_str)
                .ok_or("Informe o texto a copiar.")?;
            if text.len() > 65536 || text.contains('\0') {
                return Err("O texto da área de transferência excede 64 KiB ou contém caracteres inválidos.".into());
            }
        }
        "clipboard.read" => {}
        "system.info" => {}
        _ => return Err("Habilidade do sistema desconhecida.".into()),
    }
    Ok(())
}

pub(crate) fn preview(skill_id: &str, params: &Value, root: &Path) -> Result<String, String> {
    validate(skill_id, params)?;
    Ok(match skill_id {
        "audio.get_volume" => "Consultar o volume principal do Windows.".into(),
        "audio.set_volume" => format!("Alterar o volume principal do Windows para {}%.", params["volume"]),
        "audio.mute" => if params["muted"] == true { "Silenciar o áudio principal do Windows." } else { "Reativar o áudio principal do Windows." }.into(),
        "media.play_pause" => "Enviar tecla de reproduzir/pausar para o Windows.".into(),
        "media.next" => "Enviar tecla de próxima faixa para o Windows.".into(),
        "media.previous" => "Enviar tecla de faixa anterior para o Windows.".into(),
        "wallpaper.set" => format!("Alterar o papel de parede para {}.", params["path"].as_str().unwrap_or("")),
        "wallpaper.restore" => "Restaurar o papel de parede anterior.".into(),
        "power.lock" => "Solicitar o bloqueio da sessão do Windows.".into(),
        "display.get_brightness" => "Consultar o brilho atual da tela; pode não haver suporte por software.".into(),
        "display.set_brightness" => format!("Alterar o brilho da tela para {}%.", params["brightness"]),
        "display.restore_brightness" => "Restaurar o brilho anterior da tela.".into(),
        "power.sleep" => "O computador vai suspender agora. A aprovação é obrigatória.".into(),
        "theme.set" => format!("Alterar o tema experimental do Windows para {}.", params["mode"]),
        "theme.restore" => "Restaurar o tema anterior do Windows.".into(),
        "system.open_url" => format!(
            "Abrir no navegador padrão: {}",
            string_param(params, "url")?
        ),
        "system.open_path" => format!(
            "Abrir arquivo ou pasta permitida: {}",
            checked_path(params, root)?.display()
        ),
        "system.reveal_in_folder" => format!(
            "Mostrar no Explorador de Arquivos: {}",
            checked_path(params, root)?.display()
        ),
        "settings.open" => format!(
            "Abrir Configurações do Windows: {}",
            page_uri(string_param(params, "page")?)
                .ok_or("A página de Configurações solicitada não está disponível.")?
        ),
        "notification.show" => format!(
            "Exibir notificação local: {}",
            string_param(params, "title")?
        ),
        "app.launch" => format!(
            "Iniciar aplicativo cadastrado {} sem argumentos.",
            string_param(params, "app_id")?
        ),
        "clipboard.write" => format!(
            "Substituir a área de transferência por texto ({} bytes):\n{}",
            string_param(params, "text")?.len(),
            string_param(params, "text")?
        ),
        "clipboard.read" => "Ler o texto atual da área de transferência; o conteúdo será tratado como dado não confiável.".into(),
        "system.info" => "Consultar informações locais resumidas do Windows, memória, discos das pastas permitidas, bateria e tempo ligado.".into(),
        _ => return Err("Habilidade do sistema desconhecida.".into()),
    })
}

pub(crate) fn preview_allowed_app(
    app: &tauri::AppHandle,
    params: &Value,
) -> Result<String, String> {
    validate("app.launch", params)?;
    let (name, path) = lookup_allowed_app(
        &crate::security::database(app)?,
        string_param(params, "app_id")?,
    )?
    .ok_or("O aplicativo não está cadastrado como permitido.")?;
    Ok(format!(
        "Iniciar aplicativo cadastrado: {name}\nExecutável: {path}\nSem argumentos."
    ))
}

pub(crate) fn execute(
    skill_id: &str,
    params: &Value,
    root: &Path,
    app: &tauri::AppHandle,
) -> Result<String, String> {
    validate(skill_id, params)?;
    match skill_id {
        "audio.get_volume" | "audio.set_volume" | "audio.mute" | "media.play_pause"
        | "media.next" | "media.previous" => crate::system_controls::execute(skill_id, params),
        "wallpaper.set"
        | "wallpaper.restore"
        | "power.lock"
        | "display.get_brightness"
        | "display.set_brightness"
        | "display.restore_brightness"
        | "power.sleep"
        | "theme.set"
        | "theme.restore" => {
            Err("Esta habilidade precisa ser despachada com contexto de segurança.".into())
        }
        "system.open_url" => {
            let normalized =
                crate::external_links::validate_external_url(string_param(params, "url")?)
                    .map_err(str::to_string)?;
            app.opener()
                .open_url(normalized, None::<&str>)
                .map_err(|_| "Não foi possível abrir o navegador padrão.".to_string())?;
            Ok("URL aberta no navegador padrão.".into())
        }
        "system.open_path" => {
            let path = checked_path(params, root)?;
            app.opener()
                .open_path(path.to_string_lossy(), None::<&str>)
                .map_err(|_| "Não foi possível abrir o arquivo ou a pasta.".to_string())?;
            Ok("Arquivo ou pasta aberta.".into())
        }
        "system.reveal_in_folder" => {
            let path = checked_path(params, root)?;
            app.opener().reveal_item_in_dir(path).map_err(|_| {
                "Não foi possível mostrar o item no Explorador de Arquivos.".to_string()
            })?;
            Ok("Item mostrado no Explorador de Arquivos.".into())
        }
        "settings.open" => {
            let uri = page_uri(string_param(params, "page")?)
                .ok_or("A página de Configurações solicitada não está disponível.")?;
            app.opener()
                .open_url(uri, None::<&str>)
                .map_err(|_| "Não foi possível abrir Configurações do Windows.".to_string())?;
            Ok("Página de Configurações aberta.".into())
        }
        "notification.show" => {
            use tauri_plugin_notification::NotificationExt;
            app.notification()
                .builder()
                .title(string_param(params, "title")?)
                .body(string_param(params, "body")?)
                .show()
                .map_err(|_| "Não foi possível exibir a notificação.".to_string())?;
            Ok("Notificação exibida.".into())
        }
        "app.launch" => {
            let app_id = string_param(params, "app_id")?;
            let (_, path) = lookup_allowed_app(&crate::security::database(app)?, app_id)?
                .ok_or("O aplicativo não está cadastrado como permitido.")?;
            let executable = PathBuf::from(path);
            if crate::security::workspace::has_reparse(&executable)
                || !executable.is_file()
                || !executable
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
            {
                return Err(
                    "O executável cadastrado não passou pela validação de segurança.".into(),
                );
            }
            std::process::Command::new(executable)
                .spawn()
                .map_err(|_| "Não foi possível iniciar o aplicativo cadastrado.".to_string())?;
            Ok("Aplicativo cadastrado iniciado sem argumentos.".into())
        }
        "clipboard.write" => {
            use tauri_plugin_clipboard_manager::ClipboardExt;
            app.clipboard()
                .write_text(string_param(params, "text")?.to_string())
                .map_err(|_| "Não foi possível atualizar a área de transferência.".to_string())?;
            Ok("Texto copiado para a área de transferência.".into())
        }
        "clipboard.read" => {
            use tauri_plugin_clipboard_manager::ClipboardExt;
            let text = app
                .clipboard()
                .read_text()
                .map_err(|_| "Não foi possível ler texto da área de transferência.".to_string())?;
            if text.len() > 65536 {
                return Err("O texto da área de transferência excede o limite de 64 KiB.".into());
            }
            Ok(text)
        }
        "system.info" => system_information(app),
        _ => Err("Habilidade do sistema desconhecida.".into()),
    }
}

pub(crate) fn execute_managed_control(
    app: &tauri::AppHandle,
    skill_id: &str,
    params: &Value,
    origin: &str,
    operation_id: &str,
) -> Result<String, String> {
    validate(skill_id, params)?;
    let db = crate::security::database(app)?;
    db.execute_batch("CREATE TABLE IF NOT EXISTS wallpaper_history (id INTEGER PRIMARY KEY, previous_path TEXT, created_at INTEGER NOT NULL, origin TEXT NOT NULL, restored_at INTEGER); CREATE TABLE IF NOT EXISTS brightness_history (id INTEGER PRIMARY KEY, previous_value INTEGER NOT NULL, created_at INTEGER NOT NULL, origin TEXT NOT NULL, restored_at INTEGER); CREATE TABLE IF NOT EXISTS theme_history (id INTEGER PRIMARY KEY, apps_type INTEGER, apps_data BLOB, system_type INTEGER, system_data BLOB, created_at INTEGER NOT NULL, origin TEXT NOT NULL, restored_at INTEGER)")
        .map_err(|_| "Não foi possível preparar o histórico do papel de parede.".to_string())?;
    let mut roots = vec![crate::agent_home::workspace_path(app)?];
    let mut statement = db
        .prepare("SELECT path FROM allowed_folders ORDER BY path")
        .map_err(|_| "Não foi possível consultar as pastas permitidas.".to_string())?;
    let folders = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|_| "Não foi possível consultar as pastas permitidas.".to_string())?;
    roots.extend(folders.filter_map(Result::ok).map(PathBuf::from));
    crate::system_controls::execute_managed_with_id(
        &mut crate::system_controls::NativeAdapter,
        skill_id,
        params,
        &db,
        &roots,
        origin,
        operation_id,
    )
}

#[tauri::command]
pub fn open_agent_artifact(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    path: String,
) -> Result<(), String> {
    crate::require_main(&caller)?;
    let path = validate_agent_artifact_path(&app, &path)?;
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|_| "Não foi possível abrir o arquivo permitido.".to_string())
}

#[tauri::command]
pub fn reveal_agent_artifact(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    path: String,
) -> Result<(), String> {
    crate::require_main(&caller)?;
    let path = validate_agent_artifact_path(&app, &path)?;
    app.opener()
        .reveal_item_in_dir(path)
        .map_err(|_| "Não foi possível mostrar o arquivo permitido.".to_string())
}

fn validate_agent_artifact_path(app: &tauri::AppHandle, raw: &str) -> Result<PathBuf, String> {
    if raw.is_empty() || raw.len() > 32_767 || raw.chars().any(char::is_control) {
        return Err("O caminho do arquivo não é válido.".into());
    }
    let db = crate::security::database(app)?;
    let mut roots = vec![crate::agent_home::workspace_path(app)?];
    let mut statement = db
        .prepare("SELECT path FROM allowed_folders ORDER BY path")
        .map_err(|_| "Não foi possível consultar as pastas permitidas.".to_string())?;
    let folders = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|_| "Não foi possível consultar as pastas permitidas.".to_string())?;
    roots.extend(folders.filter_map(Result::ok).map(PathBuf::from));
    let params = serde_json::json!({"path": raw});
    roots
        .iter()
        .find_map(|root| checked_path(&params, root).ok())
        .ok_or_else(|| {
            "O arquivo não existe em uma pasta permitida ou não é seguro para abrir.".into()
        })
}

pub(crate) fn preview_managed_control(
    app: &tauri::AppHandle,
    skill_id: &str,
    params: &Value,
) -> Result<String, String> {
    validate(skill_id, params)?;
    let db = crate::security::database(app)?;
    db.execute_batch("CREATE TABLE IF NOT EXISTS wallpaper_history (id INTEGER PRIMARY KEY, previous_path TEXT, created_at INTEGER NOT NULL, origin TEXT NOT NULL, restored_at INTEGER); CREATE TABLE IF NOT EXISTS brightness_history (id INTEGER PRIMARY KEY, previous_value INTEGER NOT NULL, created_at INTEGER NOT NULL, origin TEXT NOT NULL, restored_at INTEGER); CREATE TABLE IF NOT EXISTS theme_history (id INTEGER PRIMARY KEY, apps_type INTEGER, apps_data BLOB, system_type INTEGER, system_data BLOB, created_at INTEGER NOT NULL, origin TEXT NOT NULL, restored_at INTEGER)")
        .map_err(|_| "Não foi possível preparar o histórico do papel de parede.".to_string())?;
    for table in ["wallpaper_history", "brightness_history", "theme_history"] {
        let exists: bool = db
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM pragma_table_info(?1) WHERE name='operation_id')",
                [table],
                |row| row.get(0),
            )
            .map_err(|_| "Não foi possível verificar o histórico de desfazer.".to_string())?;
        if !exists {
            db.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN operation_id TEXT"))
                .map_err(|_| "Não foi possível atualizar o histórico de desfazer.".to_string())?;
        }
    }
    let mut roots = vec![crate::agent_home::workspace_path(app)?];
    let mut statement = db
        .prepare("SELECT path FROM allowed_folders ORDER BY path")
        .map_err(|_| "Não foi possível consultar as pastas permitidas.".to_string())?;
    let folders = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|_| "Não foi possível consultar as pastas permitidas.".to_string())?;
    roots.extend(folders.filter_map(Result::ok).map(PathBuf::from));
    match skill_id {
        "wallpaper.set" => {
            let path = crate::system_controls::validate_wallpaper_path(
                params["path"]
                    .as_str()
                    .ok_or("Informe o caminho da imagem.")?,
                &roots,
            )?;
            Ok(format!(
                "Alterar o papel de parede para {}.",
                path.display()
            ))
        }
        "wallpaper.restore" => {
            let available: bool = db
                .query_row("SELECT EXISTS(SELECT 1 FROM wallpaper_history WHERE restored_at IS NULL AND (?1 IS NULL OR operation_id=?1))", [params.get("history_id").and_then(Value::as_str)], |row| row.get(0))
                .map_err(|_| {
                    "Não foi possível consultar o histórico do papel de parede.".to_string()
                })?;
            if !available {
                return Err("Não há papel de parede anterior para restaurar.".into());
            }
            Ok("Restaurar o papel de parede anterior.".into())
        }
        "power.lock" => {
            Ok("Solicitar o bloqueio da sessão do Windows; a aprovação é obrigatória.".into())
        }
        "display.get_brightness" => Ok("Consultar o brilho atual da tela.".into()),
        "display.set_brightness" => Ok(format!(
            "Alterar o brilho da tela para {}%.",
            params["brightness"]
        )),
        "display.restore_brightness" => {
            let available: bool = db
                .query_row("SELECT EXISTS(SELECT 1 FROM brightness_history WHERE restored_at IS NULL AND (?1 IS NULL OR operation_id=?1))", [params.get("history_id").and_then(Value::as_str)], |row| row.get(0))
                .map_err(|_| "Não foi possível consultar o histórico do brilho.".to_string())?;
            if !available {
                return Err("Não há brilho anterior para restaurar.".into());
            }
            Ok("Restaurar o brilho anterior.".into())
        }
        "power.sleep" => Ok("O computador vai suspender agora. A aprovação é obrigatória.".into()),
        "theme.set" => Ok(format!(
            "Alterar o tema experimental do Windows para {}.",
            params["mode"]
        )),
        "theme.restore" => {
            let available: bool = db
                .query_row("SELECT EXISTS(SELECT 1 FROM theme_history WHERE restored_at IS NULL AND (?1 IS NULL OR operation_id=?1))", [params.get("history_id").and_then(Value::as_str)], |row| row.get(0))
                .map_err(|_| "Não foi possível consultar o histórico do tema.".to_string())?;
            if !available {
                return Err("Não há tema anterior para restaurar.".into());
            }
            Ok("Restaurar o tema anterior do Windows.".into())
        }
        _ => Err("Habilidade de controle local desconhecida.".into()),
    }
}

fn lookup_allowed_app(
    db: &rusqlite::Connection,
    app_id: &str,
) -> Result<Option<(String, String)>, String> {
    db.query_row(
        "SELECT name,path FROM allowed_apps WHERE app_id=?1",
        [app_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .optional()
    .map_err(|_| "Não foi possível verificar o aplicativo permitido.".into())
}

#[cfg(windows)]
fn system_information(app: &tauri::AppHandle) -> Result<String, String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::{
        Storage::FileSystem::GetDiskFreeSpaceExW,
        System::{
            Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS},
            SystemInformation::{GetTickCount64, GlobalMemoryStatusEx, MEMORYSTATUSEX},
        },
    };

    // SAFETY: MEMORYSTATUSEX is a C ABI output struct whose zero state is valid before dwLength is set.
    let mut memory: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
    memory.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
    // SAFETY: `memory` is a valid writable struct with dwLength initialized as required by the API.
    if unsafe { GlobalMemoryStatusEx(&mut memory) } == 0 {
        return Err("Não foi possível consultar a memória do sistema.".into());
    }
    let mut output = format!(
        "Sistema: Windows\nMemória física: {} total; {} disponível\nTempo ligado: {}\n",
        format_bytes(memory.ullTotalPhys),
        format_bytes(memory.ullAvailPhys),
        // SAFETY: GetTickCount64 has no pointer arguments or caller-maintained invariants.
        format_duration(unsafe { GetTickCount64() } / 1000),
    );

    let mut folders = Vec::<PathBuf>::new();
    if let Ok(workspace) = crate::agent_home::workspace_path(app) {
        folders.push(workspace);
    }
    if let Ok(db) = crate::security::database(app) {
        if let Ok(mut statement) = db.prepare("SELECT path FROM allowed_folders ORDER BY path") {
            if let Ok(rows) = statement.query_map([], |row| row.get::<_, String>(0)) {
                folders.extend(rows.filter_map(Result::ok).map(PathBuf::from));
            }
        }
    }
    folders.sort();
    folders.dedup();
    output.push_str("Discos das pastas permitidas:\n");
    for (index, folder) in folders.iter().enumerate() {
        if crate::security::workspace::has_reparse(folder) || !folder.is_dir() {
            continue;
        }
        let mut wide: Vec<u16> = folder.as_os_str().encode_wide().collect();
        wide.push(0);
        let mut available = 0_u64;
        let mut total = 0_u64;
        let mut free = 0_u64;
        // SAFETY: `wide` is NUL-terminated and all output pointers reference live local integers.
        if unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut available, &mut total, &mut free) } != 0
        {
            output.push_str(&format!(
                "Pasta permitida {}: {} total; {} livre; {} disponível\n",
                index + 1,
                format_bytes(total),
                format_bytes(free),
                format_bytes(available),
            ));
        }
    }

    // SAFETY: SYSTEM_POWER_STATUS is a C ABI output struct with a valid zero initialization.
    let mut power: SYSTEM_POWER_STATUS = unsafe { std::mem::zeroed() };
    // SAFETY: `power` is a live writable output struct for the duration of this call.
    if unsafe { GetSystemPowerStatus(&mut power) } != 0 {
        if power.BatteryFlag & 128 != 0 || power.BatteryLifePercent == 255 {
            output.push_str("Bateria: não detectada ou sem medição disponível\n");
        } else {
            let power_source = match power.ACLineStatus {
                0 => "na bateria",
                1 => "na tomada",
                _ => "alimentação desconhecida",
            };
            let charging = if power.BatteryFlag & 8 != 0 {
                "carregando"
            } else {
                "sem indicação de carregamento"
            };
            output.push_str(&format!(
                "Bateria: {}%; {power_source}; {charging}\n",
                power.BatteryLifePercent
            ));
        }
    } else {
        output.push_str("Bateria: informação indisponível\n");
    }
    Ok(output)
}

#[cfg(not(windows))]
fn system_information(_: &tauri::AppHandle) -> Result<String, String> {
    Err("As informações do sistema estão disponíveis somente no Windows.".into())
}

fn format_bytes(bytes: u64) -> String {
    format!("{:.1} GiB", bytes as f64 / 1024_f64.powi(3))
}

fn format_duration(seconds: u64) -> String {
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let minutes = (seconds % 3_600) / 60;
    format!("{days} d {hours} h {minutes} min")
}

pub(crate) fn domain_for_open_url(params: &Value) -> Result<Option<String>, String> {
    if params.get("url").is_none() {
        return Ok(None);
    }
    let normalized = crate::external_links::validate_external_url(string_param(params, "url")?)
        .map_err(str::to_string)?;
    let url = Url::parse(&normalized).map_err(|_| "URL inválida.")?;
    Ok(url.host_str().map(str::to_ascii_lowercase))
}

pub(crate) fn page_uri(page: &str) -> Option<&'static str> {
    SETTINGS_PAGES
        .iter()
        .find(|(key, _)| *key == page)
        .map(|(_, uri)| *uri)
}

fn checked_path(params: &Value, root: &Path) -> Result<PathBuf, String> {
    let raw = Path::new(params["path"].as_str().ok_or("Informe um caminho.")?);
    let candidate = if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        root.join(raw)
    };
    if crate::security::workspace::has_reparse(&candidate) {
        return Err("Links e junctions não podem ser abertos.".into());
    }
    let canonical = candidate
        .canonicalize()
        .map_err(|_| "O caminho não existe ou não pôde ser validado.")?;
    let canonical_root = root
        .canonicalize()
        .map_err(|_| "A pasta permitida não pôde ser validada.")?;
    let permitted = if raw.is_absolute() {
        crate::security::skills::validate_raw_folder_path(&canonical.to_string_lossy())?;
        // The dispatcher resolved the selected allowed folder as `root`.
        canonical.starts_with(&canonical_root)
    } else {
        canonical.starts_with(&canonical_root)
    };
    if !permitted || !canonical.is_file() && !canonical.is_dir() || blocked_extension(&canonical) {
        return Err(
            "O caminho não pertence a uma pasta permitida ou não é seguro para abrir.".into(),
        );
    }
    Ok(canonical)
}

fn blocked_extension(path: &Path) -> bool {
    const BLOCKED: &[&str] = &[
        "exe",
        "bat",
        "cmd",
        "com",
        "ps1",
        "psm1",
        "psd1",
        "vbs",
        "vbe",
        "js",
        "jse",
        "wsf",
        "wsh",
        "jar",
        "msi",
        "msp",
        "scr",
        "lnk",
        "reg",
        "hta",
        "cpl",
        "msc",
        "dll",
        "sys",
        "sh",
        "bash",
        "py",
        "pyw",
        "rb",
        "pl",
        "php",
        "class",
        "appx",
        "msix",
        "pif",
        "gadget",
        "workflow",
        "url",
        "website",
        "scf",
        "application",
        "appref-ms",
        "msu",
        "docm",
        "dotm",
        "xlsm",
        "xltm",
        "xlam",
        "pptm",
        "potm",
        "ppam",
        "html",
        "htm",
        "xhtml",
        "svg",
        "mht",
        "mhtml",
    ];
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| {
            BLOCKED
                .iter()
                .any(|blocked| extension.eq_ignore_ascii_case(blocked))
        })
}

fn bounded_string<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
    max: usize,
) -> Result<&'a str, String> {
    let value = object
        .get(key)
        .and_then(Value::as_str)
        .ok_or("Campo textual inválido.")?;
    if value.is_empty() || value.len() > max || value.chars().any(char::is_control) {
        return Err("Um campo textual excede os limites permitidos.".into());
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rejects_non_http_schemes_and_private_addresses() {
        for url in [
            "file:///C:/secret",
            "javascript:alert(1)",
            "data:text/plain,x",
            "custom:thing",
            "http://127.0.0.1",
            "http://192.168.1.2",
        ] {
            assert!(
                validate("system.open_url", &json!({"url":url})).is_err(),
                "{url}"
            );
        }
        assert!(validate("system.open_url", &json!({"url":"https://example.com"})).is_ok());
    }

    #[test]
    fn settings_page_is_a_closed_uri_mapping() {
        assert_eq!(page_uri("tela"), Some("ms-settings:display"));
        assert!(page_uri("file:///C:/").is_none());
        assert!(validate("settings.open", &json!({"page":"ms-settings:display"})).is_err());
    }

    #[test]
    fn malformed_open_url_preview_returns_a_skill_error() {
        assert!(
            preview("system.open_url", &serde_json::json!({}), Path::new("."))
                .unwrap_err()
                .contains("não correspondem à habilidade permitida")
        );
    }

    #[test]
    fn blocks_executable_and_script_extensions_case_insensitively() {
        for path in [
            "setup.EXE",
            "run.ps1",
            "open.JS",
            "shortcut.lnk",
            "setup.msi",
            "internet.url",
            "macro.xlsm",
            "document.html",
        ] {
            assert!(blocked_extension(Path::new(path)), "{path}");
        }
        assert!(!blocked_extension(Path::new("notes.txt")));
    }

    #[test]
    fn system_paths_reject_traversal_and_domain_lookup_uses_the_link_policy() {
        assert!(validate("system.open_path", &json!({"path":"../secret.txt"})).is_err());
        assert_eq!(
            domain_for_open_url(&json!({"url":"https://Example.com/page"})).unwrap(),
            Some("example.com".into())
        );
        assert!(domain_for_open_url(&json!({"url":"http://127.0.0.1"})).is_err());
    }

    #[test]
    fn artifact_paths_must_exist_inside_the_selected_allowed_root_and_be_non_executable() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("odekrai-artifact-{}-{stamp}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("result.txt"), "ok").unwrap();
        std::fs::write(root.join("payload.exe"), "not executed").unwrap();
        assert!(checked_path(&json!({"path":"result.txt"}), &root).is_ok());
        assert!(checked_path(&json!({"path":"../outside.txt"}), &root).is_err());
        assert!(checked_path(&json!({"path":"payload.exe"}), &root).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn app_launch_rejects_identifiers_that_are_not_registered() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute(
            "CREATE TABLE allowed_apps(app_id TEXT PRIMARY KEY,name TEXT,path TEXT)",
            [],
        )
        .unwrap();
        assert!(lookup_allowed_app(&db, "app-0123456789abcdef0123")
            .unwrap()
            .is_none());
        assert!(validate("app.launch", &json!({"app_id":"app-0123456789abcdef0123"})).is_ok());
        assert!(validate("app.launch", &json!({"app_id":"app-invented"})).is_err());
    }

    #[test]
    fn clipboard_read_is_empty_schema_and_returns_an_untrusted_skill_result() {
        assert!(validate("clipboard.read", &json!({})).is_ok());
        assert!(validate("clipboard.read", &json!({"text":"forjado"})).is_err());
        assert!(validate("clipboard.write", &json!({"text":"texto"})).is_ok());
        assert!(validate("clipboard.write", &json!({"text":"x\u{0000}y"})).is_err());
        let read = crate::security::skills::registry()
            .into_iter()
            .find(|skill| skill.id == "clipboard.read")
            .unwrap();
        assert_eq!(
            read.result_trust,
            crate::security::skills::ResultTrust::Untrusted
        );
    }

    #[test]
    fn system_info_has_no_arguments_and_formatted_values_omit_path_data() {
        assert!(validate("system.info", &json!({})).is_ok());
        assert!(validate("system.info", &json!({"path":"C:/Users/name"})).is_err());
        assert_eq!(format_duration(90_061), "1 d 1 h 1 min");
        assert!(format_bytes(8 * 1024 * 1024 * 1024).contains("8.0 GiB"));
    }
}
