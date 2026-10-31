use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Camada que separa a validação e o despacho dos efeitos nativos do Windows.
pub(crate) trait SystemAdapter {
    fn volume(&mut self) -> Result<u8, String>;
    fn set_volume(&mut self, value: u8) -> Result<(), String>;
    fn mute(&mut self, muted: bool) -> Result<(), String>;
    fn media_key(&mut self, key: MediaKey) -> Result<(), String>;
    fn wallpaper(&mut self) -> Result<Option<String>, String>;
    fn set_wallpaper(&mut self, path: Option<&str>) -> Result<(), String>;
    fn lock_workstation(&mut self) -> Result<(), String>;
    fn brightness(&mut self) -> Result<Option<u8>, String>;
    fn set_brightness(&mut self, value: u8) -> Result<(), String>;
    fn sleep(&mut self) -> Result<(), String>;
    fn theme_values(&mut self) -> Result<ThemeSnapshot, String>;
    fn set_theme(&mut self, light: bool) -> Result<(), String>;
    fn restore_theme(&mut self, snapshot: &ThemeSnapshot) -> Result<(), String>;
}

const UNSUPPORTED_BRIGHTNESS: &str = "este monitor não permite controlar o brilho por software";
type ThemeHistoryRow = Option<(Option<i64>, Option<Vec<u8>>, Option<i64>, Option<Vec<u8>>)>;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ThemeSnapshot {
    pub apps_use_light_theme: Option<RegistryValue>,
    pub system_uses_light_theme: Option<RegistryValue>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RegistryValue {
    pub kind: u32,
    pub data: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MediaKey {
    PlayPause,
    Next,
    Previous,
}

pub(crate) fn execute_with<A: SystemAdapter>(
    adapter: &mut A,
    skill_id: &str,
    params: &Value,
) -> Result<String, String> {
    match skill_id {
        "audio.get_volume" => Ok(format!("Volume principal: {}%.", adapter.volume()?)),
        "audio.set_volume" => {
            let value = params["volume"]
                .as_u64()
                .ok_or("Informe um volume entre 0 e 100.")?;
            if value > 100 {
                return Err("O volume precisa estar entre 0 e 100.".into());
            }
            adapter.set_volume(value as u8)?;
            Ok(format!("Volume definido para {value}%"))
        }
        "audio.mute" => {
            let muted = params["muted"]
                .as_bool()
                .ok_or("Informe se o áudio deve ficar sem som.")?;
            adapter.mute(muted)?;
            Ok(if muted {
                "Áudio silenciado."
            } else {
                "Áudio reativado."
            }
            .into())
        }
        "media.play_pause" => {
            adapter.media_key(MediaKey::PlayPause)?;
            Ok("Comando de reproduzir/pausar enviado.".into())
        }
        "media.next" => {
            adapter.media_key(MediaKey::Next)?;
            Ok("Comando de próxima faixa enviado.".into())
        }
        "media.previous" => {
            adapter.media_key(MediaKey::Previous)?;
            Ok("Comando de faixa anterior enviado.".into())
        }
        "wallpaper.set" => {
            Err("A habilidade requer validação de pasta e persistência SQLite.".into())
        }
        "wallpaper.restore" => Err("A habilidade requer persistência SQLite.".into()),
        "power.lock" => {
            adapter.lock_workstation()?;
            Ok("Solicitação de bloqueio enviada ao Windows.".into())
        }
        "display.get_brightness" => match adapter.brightness()? {
            Some(value) => Ok(format!("Brilho da tela: {value}%")),
            None => Err(UNSUPPORTED_BRIGHTNESS.into()),
        },
        "display.set_brightness" => {
            let value = params["brightness"]
                .as_u64()
                .ok_or("Informe um brilho inteiro entre 0 e 100.")?;
            if value > 100 {
                return Err("O brilho precisa estar entre 0 e 100.".into());
            }
            adapter.set_brightness(value as u8)?;
            Ok(format!(
                "Brilho definido para {value}%. Use display.restore_brightness para desfazer."
            ))
        }
        "display.restore_brightness" => Err("A habilidade requer persistência SQLite.".into()),
        "power.sleep" => {
            adapter.sleep()?;
            Ok("O computador vai suspender agora.".into())
        }
        "theme.set" => {
            let mode = params["mode"]
                .as_str()
                .ok_or("Informe o tema claro ou escuro.")?;
            let light = match mode {
                "claro" => true,
                "escuro" => false,
                _ => return Err("O tema precisa ser claro ou escuro.".into()),
            };
            adapter.set_theme(light)?;
            Ok("Tema alterado; o Windows foi avisado para atualizar os aplicativos.".into())
        }
        "theme.restore" => Err("A habilidade requer persistência SQLite.".into()),
        _ => Err("Habilidade de controle local desconhecida.".into()),
    }
}

#[cfg(test)]
#[derive(Default)]
pub(crate) struct SimulatedAdapter {
    pub volume: u8,
    pub muted: bool,
    pub media_commands: Vec<MediaKey>,
    pub current_wallpaper: Option<String>,
    pub locked: bool,
    pub brightness: Option<u8>,
    pub sleep_requested: bool,
    pub theme: ThemeSnapshot,
}

#[cfg(test)]
impl SystemAdapter for SimulatedAdapter {
    fn volume(&mut self) -> Result<u8, String> {
        Ok(self.volume)
    }
    fn set_volume(&mut self, value: u8) -> Result<(), String> {
        self.volume = value;
        Ok(())
    }
    fn mute(&mut self, muted: bool) -> Result<(), String> {
        self.muted = muted;
        Ok(())
    }
    fn media_key(&mut self, key: MediaKey) -> Result<(), String> {
        self.media_commands.push(key);
        Ok(())
    }
    fn wallpaper(&mut self) -> Result<Option<String>, String> {
        Ok(self.current_wallpaper.clone())
    }
    fn set_wallpaper(&mut self, path: Option<&str>) -> Result<(), String> {
        self.current_wallpaper = path.map(str::to_owned);
        Ok(())
    }
    fn lock_workstation(&mut self) -> Result<(), String> {
        self.locked = true;
        Ok(())
    }
    fn brightness(&mut self) -> Result<Option<u8>, String> {
        Ok(self.brightness)
    }
    fn set_brightness(&mut self, value: u8) -> Result<(), String> {
        if let Some(brightness) = &mut self.brightness {
            *brightness = value;
            Ok(())
        } else {
            Err(UNSUPPORTED_BRIGHTNESS.into())
        }
    }
    fn sleep(&mut self) -> Result<(), String> {
        self.sleep_requested = true;
        Ok(())
    }
    fn theme_values(&mut self) -> Result<ThemeSnapshot, String> {
        Ok(self.theme.clone())
    }
    fn set_theme(&mut self, light: bool) -> Result<(), String> {
        let value = if light { 1_u32 } else { 0_u32 }.to_le_bytes().to_vec();
        let registry_value = Some(RegistryValue {
            kind: REG_DWORD_VALUE,
            data: value,
        });
        self.theme = ThemeSnapshot {
            apps_use_light_theme: registry_value.clone(),
            system_uses_light_theme: registry_value,
        };
        Ok(())
    }
    fn restore_theme(&mut self, snapshot: &ThemeSnapshot) -> Result<(), String> {
        self.theme = snapshot.clone();
        Ok(())
    }
}

const REG_DWORD_VALUE: u32 = 4;

pub(crate) fn execute(skill_id: &str, params: &Value) -> Result<String, String> {
    execute_with(&mut NativeAdapter, skill_id, params)
}

#[cfg(test)]
pub(crate) fn execute_managed_with<A: SystemAdapter>(
    adapter: &mut A,
    skill_id: &str,
    args: &Value,
    db: &Connection,
    allowed_roots: &[PathBuf],
    origin: &str,
) -> Result<String, String> {
    execute_managed_with_id(adapter, skill_id, args, db, allowed_roots, origin, "")
}

pub(crate) fn execute_managed_with_id<A: SystemAdapter>(
    adapter: &mut A,
    skill_id: &str,
    args: &Value,
    db: &Connection,
    allowed_roots: &[PathBuf],
    origin: &str,
    operation_id: &str,
) -> Result<String, String> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS wallpaper_history (id INTEGER PRIMARY KEY, previous_path TEXT, created_at INTEGER NOT NULL, origin TEXT NOT NULL, restored_at INTEGER)")
        .map_err(|_| "Não foi possível preparar o histórico do papel de parede.".to_string())?;
    db.execute_batch("CREATE TABLE IF NOT EXISTS brightness_history (id INTEGER PRIMARY KEY, previous_value INTEGER NOT NULL, created_at INTEGER NOT NULL, origin TEXT NOT NULL, restored_at INTEGER)")
        .map_err(|_| "Não foi possível preparar o histórico do brilho.".to_string())?;
    db.execute_batch("CREATE TABLE IF NOT EXISTS theme_history (id INTEGER PRIMARY KEY, apps_type INTEGER, apps_data BLOB, system_type INTEGER, system_data BLOB, created_at INTEGER NOT NULL, origin TEXT NOT NULL, restored_at INTEGER)")
        .map_err(|_| "Não foi possível preparar o histórico do tema.".to_string())?;
    for table in ["wallpaper_history", "brightness_history", "theme_history"] {
        let has_operation_id: bool = db
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM pragma_table_info(?1) WHERE name='operation_id')",
                [table],
                |row| row.get(0),
            )
            .map_err(|_| "Não foi possível verificar o histórico de desfazer.".to_string())?;
        if !has_operation_id {
            db.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN operation_id TEXT"))
                .map_err(|_| "Não foi possível atualizar o histórico de desfazer.".to_string())?;
        }
    }
    match skill_id {
        "wallpaper.set" => {
            let raw = args["path"]
                .as_str()
                .ok_or("Informe o caminho da imagem.")?;
            let path = validate_wallpaper_path(raw, allowed_roots)?;
            let previous = adapter.wallpaper()?;
            db.execute(
                "INSERT INTO wallpaper_history(previous_path,created_at,origin,operation_id) VALUES (?1,?2,?3,?4)",
                params![previous, unix_seconds(), origin, operation_id],
            )
            .map_err(|_| "Não foi possível guardar o papel de parede anterior.".to_string())?;
            if let Err(error) = adapter.set_wallpaper(Some(&path.to_string_lossy())) {
                let _ = db.execute("UPDATE wallpaper_history SET restored_at=?1 WHERE operation_id=?2 AND restored_at IS NULL", params![unix_seconds(), operation_id]);
                return Err(error);
            }
            Ok("Papel de parede alterado. Use wallpaper.restore para desfazer.".into())
        }
        "wallpaper.restore" => {
            let history_id = args.get("history_id").and_then(Value::as_str);
            let previous: Option<Option<String>> = if let Some(history_id) = history_id {
                db.query_row(
                    "SELECT previous_path FROM wallpaper_history WHERE operation_id=?1 AND restored_at IS NULL",
                    [history_id],
                    |row| row.get(0),
                ).optional()
            } else {
                db.query_row(
                    "SELECT previous_path FROM wallpaper_history WHERE restored_at IS NULL ORDER BY id DESC LIMIT 1",
                    [],
                    |row| row.get(0),
                ).optional()
            }
                .map_err(|_| "Não foi possível consultar o papel de parede anterior.".to_string())?;
            let previous = previous.ok_or("Não há papel de parede anterior para restaurar.")?;
            adapter.set_wallpaper(previous.as_deref())?;
            let updated = if let Some(history_id) = history_id {
                db.execute("UPDATE wallpaper_history SET restored_at=?1 WHERE operation_id=?2 AND restored_at IS NULL", params![unix_seconds(), history_id])
            } else {
                db.execute("UPDATE wallpaper_history SET restored_at=?1 WHERE id=(SELECT id FROM wallpaper_history WHERE restored_at IS NULL ORDER BY id DESC LIMIT 1)", [unix_seconds()])
            }
            .map_err(|_| "Não foi possível registrar a restauração do papel de parede.".to_string())?;
            if updated == 0 {
                return Err(
                    "O estado anterior do papel de parede não está mais disponível.".into(),
                );
            }
            Ok("Papel de parede anterior restaurado.".into())
        }
        "display.get_brightness" => execute_with(adapter, skill_id, args),
        "display.set_brightness" => {
            let value = args["brightness"]
                .as_u64()
                .ok_or("Informe um brilho inteiro entre 0 e 100.")?;
            if value > 100 {
                return Err("O brilho precisa estar entre 0 e 100.".into());
            }
            let previous = adapter.brightness()?.ok_or(UNSUPPORTED_BRIGHTNESS)?;
            db.execute("INSERT INTO brightness_history(previous_value,created_at,origin,operation_id) VALUES (?1,?2,?3,?4)", params![previous, unix_seconds(), origin, operation_id])
                .map_err(|_| "Não foi possível guardar o brilho anterior.".to_string())?;
            if let Err(error) = adapter.set_brightness(value as u8) {
                let _ = db.execute("UPDATE brightness_history SET restored_at=?1 WHERE operation_id=?2 AND restored_at IS NULL", params![unix_seconds(), operation_id]);
                return Err(error);
            }
            Ok(format!(
                "Brilho definido para {value}%. Use display.restore_brightness para desfazer."
            ))
        }
        "display.restore_brightness" => {
            let history_id = args.get("history_id").and_then(Value::as_str);
            let previous: Option<i64> = if let Some(history_id) = history_id {
                db.query_row("SELECT previous_value FROM brightness_history WHERE operation_id=?1 AND restored_at IS NULL", [history_id], |row| row.get(0)).optional()
            } else { db.query_row("SELECT previous_value FROM brightness_history WHERE restored_at IS NULL ORDER BY id DESC LIMIT 1", [], |row| row.get(0)).optional() }
                .map_err(|_| "Não foi possível consultar o brilho anterior.".to_string())?;
            let previous = previous.ok_or("Não há brilho anterior para restaurar.")?;
            adapter.set_brightness(previous as u8)?;
            let updated = if let Some(history_id) = history_id { db.execute("UPDATE brightness_history SET restored_at=?1 WHERE operation_id=?2 AND restored_at IS NULL", params![unix_seconds(), history_id]) } else { db.execute("UPDATE brightness_history SET restored_at=?1 WHERE id=(SELECT id FROM brightness_history WHERE restored_at IS NULL ORDER BY id DESC LIMIT 1)", [unix_seconds()]) }
                .map_err(|_| "Não foi possível registrar a restauração do brilho.".to_string())?;
            if updated == 0 {
                return Err("O estado anterior do brilho não está mais disponível.".into());
            }
            Ok("Brilho anterior restaurado.".into())
        }
        "theme.set" => {
            let mode = args["mode"]
                .as_str()
                .ok_or("Informe o tema claro ou escuro.")?;
            let light = match mode {
                "claro" => true,
                "escuro" => false,
                _ => return Err("O tema precisa ser claro ou escuro.".into()),
            };
            let previous = adapter.theme_values()?;
            db.execute("INSERT INTO theme_history(apps_type,apps_data,system_type,system_data,created_at,origin,operation_id) VALUES (?1,?2,?3,?4,?5,?6,?7)", params![previous.apps_use_light_theme.as_ref().map(|value| value.kind as i64), previous.apps_use_light_theme.as_ref().map(|value| value.data.as_slice()), previous.system_uses_light_theme.as_ref().map(|value| value.kind as i64), previous.system_uses_light_theme.as_ref().map(|value| value.data.as_slice()), unix_seconds(), origin, operation_id])
                .map_err(|_| "Não foi possível guardar o tema anterior.".to_string())?;
            if let Err(error) = adapter.set_theme(light) {
                db.execute("UPDATE theme_history SET restored_at=?1 WHERE operation_id=?2 AND restored_at IS NULL", params![unix_seconds(), operation_id]).map_err(|_| "Não foi possível registrar a falha ao alterar o tema.".to_string())?;
                return Err(error);
            }
            Ok("Tema alterado; o Windows foi avisado para atualizar os aplicativos. Use theme.restore para desfazer.".into())
        }
        "theme.restore" => {
            let history_id = args.get("history_id").and_then(Value::as_str);
            let row: ThemeHistoryRow = if let Some(history_id) = history_id { db.query_row("SELECT apps_type,apps_data,system_type,system_data FROM theme_history WHERE operation_id=?1 AND restored_at IS NULL", [history_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))).optional() } else { db.query_row("SELECT apps_type,apps_data,system_type,system_data FROM theme_history WHERE restored_at IS NULL ORDER BY id DESC LIMIT 1", [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))).optional() }
                .map_err(|_| "Não foi possível consultar o tema anterior.".to_string())?;
            let (apps_type, apps_data, system_type, system_data) =
                row.ok_or("Não há tema anterior para restaurar.")?;
            let snapshot = ThemeSnapshot {
                apps_use_light_theme: registry_value_from_db(apps_type, apps_data)?,
                system_uses_light_theme: registry_value_from_db(system_type, system_data)?,
            };
            adapter.restore_theme(&snapshot)?;
            let updated = if let Some(history_id) = history_id { db.execute("UPDATE theme_history SET restored_at=?1 WHERE operation_id=?2 AND restored_at IS NULL", params![unix_seconds(), history_id]) } else { db.execute("UPDATE theme_history SET restored_at=?1 WHERE id=(SELECT id FROM theme_history WHERE restored_at IS NULL ORDER BY id DESC LIMIT 1)", [unix_seconds()]) }
                .map_err(|_| "Não foi possível registrar a restauração do tema.".to_string())?;
            if updated == 0 {
                return Err("O estado anterior do tema não está mais disponível.".into());
            }
            Ok("Tema anterior restaurado.".into())
        }
        _ => execute_with(adapter, skill_id, args),
    }
}

pub(crate) fn validate_wallpaper_path(
    raw: &str,
    allowed_roots: &[PathBuf],
) -> Result<PathBuf, String> {
    let candidate = Path::new(raw);
    if !candidate.is_absolute() || crate::security::workspace::has_reparse(candidate) {
        return Err("O caminho da imagem precisa estar em uma pasta permitida.".into());
    }
    let canonical = candidate
        .canonicalize()
        .map_err(|_| "A imagem não existe ou não pôde ser validada.".to_string())?;
    if crate::security::workspace::has_reparse(&canonical) || !canonical.is_file() {
        return Err("O caminho precisa ser um arquivo de imagem comum.".into());
    }
    let extension = canonical
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("");
    if !["png", "jpg", "bmp"]
        .iter()
        .any(|valid| extension.eq_ignore_ascii_case(valid))
    {
        return Err("Formato inválido. Use uma imagem PNG, JPG ou BMP.".into());
    }
    let mut file = std::fs::File::open(&canonical)
        .map_err(|_| "Não foi possível ler a imagem selecionada.".to_string())?;
    let mut header = [0_u8; 8];
    let bytes_read = std::io::Read::read(&mut file, &mut header)
        .map_err(|_| "Não foi possível validar o formato da imagem.".to_string())?;
    let valid_signature = match extension.to_ascii_lowercase().as_str() {
        "png" => bytes_read == 8 && header == [137, 80, 78, 71, 13, 10, 26, 10],
        "jpg" => bytes_read >= 3 && header[..3] == [0xff, 0xd8, 0xff],
        "bmp" => bytes_read >= 2 && header[..2] == *b"BM",
        _ => false,
    };
    if !valid_signature {
        return Err("O conteúdo do arquivo não corresponde a uma imagem PNG, JPG ou BMP.".into());
    }
    let permitted = allowed_roots
        .iter()
        .filter_map(|root| root.canonicalize().ok())
        .any(|root| canonical.starts_with(root));
    if !permitted {
        return Err("O caminho da imagem não pertence a uma pasta permitida.".into());
    }
    Ok(canonical)
}

fn registry_value_from_db(
    kind: Option<i64>,
    data: Option<Vec<u8>>,
) -> Result<Option<RegistryValue>, String> {
    match (kind, data) {
        (None, None) => Ok(None),
        (Some(kind), Some(data)) => u32::try_from(kind)
            .map(|kind| Some(RegistryValue { kind, data }))
            .map_err(|_| "O histórico do tema está inválido.".to_string()),
        _ => Err("O histórico do tema está incompleto.".into()),
    }
}

fn unix_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(windows)]
pub(crate) struct NativeAdapter;

#[cfg(windows)]
impl SystemAdapter for NativeAdapter {
    fn volume(&mut self) -> Result<u8, String> {
        on_audio_thread(|endpoint| unsafe { endpoint.GetMasterVolumeLevelScalar() })
            .map(|value| (value.clamp(0.0, 1.0) * 100.0).round() as u8)
    }

    fn set_volume(&mut self, value: u8) -> Result<(), String> {
        on_audio_thread(move |endpoint| unsafe {
            endpoint
                .SetMasterVolumeLevelScalar(value as f32 / 100.0, &windows::core::GUID::zeroed())
        })
    }

    fn mute(&mut self, muted: bool) -> Result<(), String> {
        on_audio_thread(move |endpoint| unsafe {
            endpoint.SetMute(muted, &windows::core::GUID::zeroed())
        })
    }

    fn media_key(&mut self, key: MediaKey) -> Result<(), String> {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY,
        };
        let vk = match key {
            MediaKey::PlayPause => 0xB3,
            MediaKey::Next => 0xB0,
            MediaKey::Previous => 0xB1,
        };
        let make = |flags| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(vk),
                    wScan: 0,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        let inputs = [make(Default::default()), make(KEYEVENTF_KEYUP)];
        let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
        if sent == inputs.len() as u32 {
            Ok(())
        } else {
            Err("O Windows não aceitou o comando de mídia.".into())
        }
    }

    fn wallpaper(&mut self) -> Result<Option<String>, String> {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            SystemParametersInfoW, SPI_GETDESKWALLPAPER,
        };
        let mut buffer = [0_u16; 260];
        let ok = unsafe {
            SystemParametersInfoW(
                SPI_GETDESKWALLPAPER,
                buffer.len() as u32,
                buffer.as_mut_ptr().cast(),
                0,
            )
        };
        if ok == 0 {
            return Err("Não foi possível consultar o papel de parede atual.".into());
        }
        let length = buffer
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(buffer.len());
        if length == 0 {
            Ok(None)
        } else {
            Ok(Some(String::from_utf16_lossy(&buffer[..length])))
        }
    }

    fn set_wallpaper(&mut self, path: Option<&str>) -> Result<(), String> {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            SystemParametersInfoW, SPIF_SENDCHANGE, SPIF_UPDATEINIFILE, SPI_SETDESKWALLPAPER,
        };
        let mut wide: Vec<u16> = path
            .map(std::ffi::OsStr::new)
            .map(|path| path.encode_wide().collect())
            .unwrap_or_default();
        let parameter = if path.is_some() {
            wide.push(0);
            wide.as_mut_ptr().cast()
        } else {
            std::ptr::null_mut()
        };
        let ok = unsafe {
            SystemParametersInfoW(
                SPI_SETDESKWALLPAPER,
                0,
                parameter,
                SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
            )
        };
        if ok == 0 {
            Err("O Windows não conseguiu alterar o papel de parede.".into())
        } else {
            Ok(())
        }
    }

    fn lock_workstation(&mut self) -> Result<(), String> {
        use windows_sys::Win32::System::Shutdown::LockWorkStation;
        if unsafe { LockWorkStation() } == 0 {
            Err("O Windows não aceitou a solicitação de bloqueio.".into())
        } else {
            Ok(())
        }
    }

    fn brightness(&mut self) -> Result<Option<u8>, String> {
        run_wmi_brightness(None)
    }
    fn set_brightness(&mut self, value: u8) -> Result<(), String> {
        run_wmi_brightness(Some(value)).map(|_| ())
    }
    fn sleep(&mut self) -> Result<(), String> {
        suspend_windows()
    }
    fn theme_values(&mut self) -> Result<ThemeSnapshot, String> {
        registry_theme_values()
    }
    fn set_theme(&mut self, light: bool) -> Result<(), String> {
        registry_set_theme(light)
    }
    fn restore_theme(&mut self, snapshot: &ThemeSnapshot) -> Result<(), String> {
        registry_restore_theme(snapshot)
    }
}

#[cfg(windows)]
fn registry_theme_values() -> Result<ThemeSnapshot, String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE,
    };
    let path: Vec<u16> =
        std::ffi::OsStr::new(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize")
            .encode_wide()
            .chain(Some(0))
            .collect();
    unsafe {
        let mut key = std::ptr::null_mut();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            path.as_ptr(),
            0,
            KEY_QUERY_VALUE | KEY_SET_VALUE,
            &mut key,
        ) != 0
        {
            return Err("Não foi possível abrir as preferências de tema do usuário atual.".into());
        }
        let snapshot: Result<ThemeSnapshot, String> = (|| {
            Ok(ThemeSnapshot {
                apps_use_light_theme: registry_read_value(key, "AppsUseLightTheme")?,
                system_uses_light_theme: registry_read_value(key, "SystemUsesLightTheme")?,
            })
        })();
        RegCloseKey(key);
        snapshot
    }
}

#[cfg(windows)]
unsafe fn registry_read_value(
    key: windows_sys::Win32::System::Registry::HKEY,
    name: &str,
) -> Result<Option<RegistryValue>, String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::{
        Foundation::ERROR_FILE_NOT_FOUND, System::Registry::RegQueryValueExW,
    };
    let name: Vec<u16> = std::ffi::OsStr::new(name)
        .encode_wide()
        .chain(Some(0))
        .collect();
    let mut kind = 0;
    let mut size = 0;
    let status = RegQueryValueExW(
        key,
        name.as_ptr(),
        std::ptr::null_mut(),
        &mut kind,
        std::ptr::null_mut(),
        &mut size,
    );
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    if status != 0 || size > 4096 {
        return Err("Não foi possível ler uma preferência de tema do usuário atual.".into());
    }
    let mut data = vec![0; size as usize];
    let status = RegQueryValueExW(
        key,
        name.as_ptr(),
        std::ptr::null_mut(),
        &mut kind,
        if data.is_empty() {
            std::ptr::null_mut()
        } else {
            data.as_mut_ptr()
        },
        &mut size,
    );
    if status != 0 {
        return Err("Não foi possível ler uma preferência de tema do usuário atual.".into());
    }
    data.truncate(size as usize);
    Ok(Some(RegistryValue { kind, data }))
}

#[cfg(windows)]
fn registry_set_theme(light: bool) -> Result<(), String> {
    let raw = if light { 1_u32 } else { 0_u32 }.to_le_bytes().to_vec();
    let value = RegistryValue {
        kind: REG_DWORD_VALUE,
        data: raw,
    };
    let snapshot = registry_theme_values()?;
    registry_apply_theme_snapshot(
        &ThemeSnapshot {
            apps_use_light_theme: Some(value.clone()),
            system_uses_light_theme: Some(value),
        },
        Some(&snapshot),
    )?;
    notify_theme_change();
    Ok(())
}

#[cfg(windows)]
fn registry_restore_theme(snapshot: &ThemeSnapshot) -> Result<(), String> {
    registry_apply_theme_snapshot(snapshot, None)?;
    notify_theme_change();
    Ok(())
}

#[cfg(windows)]
fn registry_apply_theme_snapshot(
    snapshot: &ThemeSnapshot,
    rollback: Option<&ThemeSnapshot>,
) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE,
    };
    let path: Vec<u16> =
        std::ffi::OsStr::new(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize")
            .encode_wide()
            .chain(Some(0))
            .collect();
    unsafe {
        let mut key = std::ptr::null_mut();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            path.as_ptr(),
            0,
            KEY_QUERY_VALUE | KEY_SET_VALUE,
            &mut key,
        ) != 0
        {
            return Err("Não foi possível abrir as preferências de tema do usuário atual.".into());
        }
        let first = registry_write_or_delete(
            key,
            "AppsUseLightTheme",
            snapshot.apps_use_light_theme.as_ref(),
        );
        let result = first.and_then(|()| {
            registry_write_or_delete(
                key,
                "SystemUsesLightTheme",
                snapshot.system_uses_light_theme.as_ref(),
            )
        });
        if result.is_err() {
            if let Some(previous) = rollback {
                let _ = registry_write_or_delete(
                    key,
                    "AppsUseLightTheme",
                    previous.apps_use_light_theme.as_ref(),
                );
                let _ = registry_write_or_delete(
                    key,
                    "SystemUsesLightTheme",
                    previous.system_uses_light_theme.as_ref(),
                );
            }
        }
        RegCloseKey(key);
        result
    }
}

#[cfg(windows)]
unsafe fn registry_write_or_delete(
    key: windows_sys::Win32::System::Registry::HKEY,
    name: &str,
    value: Option<&RegistryValue>,
) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::{
        Foundation::ERROR_FILE_NOT_FOUND,
        System::Registry::{RegDeleteValueW, RegSetValueExW},
    };
    let name: Vec<u16> = std::ffi::OsStr::new(name)
        .encode_wide()
        .chain(Some(0))
        .collect();
    let status = if let Some(value) = value {
        RegSetValueExW(
            key,
            name.as_ptr(),
            0,
            value.kind,
            if value.data.is_empty() {
                std::ptr::null()
            } else {
                value.data.as_ptr()
            },
            value.data.len() as u32,
        )
    } else {
        let result = RegDeleteValueW(key, name.as_ptr());
        if result == ERROR_FILE_NOT_FOUND {
            0
        } else {
            result
        }
    };
    if status == 0 {
        Ok(())
    } else {
        Err("Não foi possível gravar as preferências de tema do usuário atual.".into())
    }
}

#[cfg(windows)]
fn notify_theme_change() {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
    };
    let detail: Vec<u16> = std::ffi::OsStr::new("ImmersiveColorSet")
        .encode_wide()
        .chain(Some(0))
        .collect();
    let mut result = 0;
    unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            detail.as_ptr() as isize,
            SMTO_ABORTIFHUNG,
            1000,
            &mut result,
        );
    }
}

#[cfg(windows)]
fn run_wmi_brightness(set: Option<u8>) -> Result<Option<u8>, String> {
    use serde::Deserialize;
    #[derive(Deserialize)]
    #[serde(rename = "WmiMonitorBrightness", rename_all = "PascalCase")]
    struct MonitorBrightness {
        active: bool,
        current_brightness: u8,
    }
    #[derive(Deserialize)]
    #[serde(rename = "WmiMonitorBrightnessMethods", rename_all = "PascalCase")]
    struct BrightnessMethod {
        active: bool,
        #[serde(rename = "__PATH")]
        path: String,
    }
    #[derive(serde::Serialize)]
    #[serde(rename_all = "PascalCase")]
    struct SetParams {
        timeout: u32,
        brightness: u8,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct SetResult {
        return_value: u32,
    }

    std::thread::spawn(move || {
        use wmi::WMIConnection;
        let wmi = WMIConnection::with_namespace_path("ROOT\\WMI")
            .map_err(|_| UNSUPPORTED_BRIGHTNESS.to_string())?;
        if let Some(value) = set {
            let methods: Vec<BrightnessMethod> = wmi
                .query()
                .map_err(|_| UNSUPPORTED_BRIGHTNESS.to_string())?;
            let method = methods
                .iter()
                .find(|method| method.active)
                .ok_or_else(|| UNSUPPORTED_BRIGHTNESS.to_string())?;
            let result: SetResult = wmi
                .exec_instance_method::<BrightnessMethod, _>(
                    &method.path,
                    "WmiSetBrightness",
                    SetParams {
                        timeout: 0,
                        brightness: value,
                    },
                )
                .map_err(|_| UNSUPPORTED_BRIGHTNESS.to_string())?;
            if result.return_value != 0 {
                return Err(UNSUPPORTED_BRIGHTNESS.to_string());
            }
            Ok(Some(value))
        } else {
            let monitors: Vec<MonitorBrightness> = wmi
                .query()
                .map_err(|_| UNSUPPORTED_BRIGHTNESS.to_string())?;
            monitors
                .iter()
                .find(|monitor| monitor.active)
                .map(|monitor| Some(monitor.current_brightness))
                .ok_or_else(|| UNSUPPORTED_BRIGHTNESS.to_string())
        }
    })
    .join()
    .map_err(|_| UNSUPPORTED_BRIGHTNESS.to_string())?
}

#[cfg(windows)]
fn suspend_windows() -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::System::Power::SetSuspendState;
    use windows_sys::Win32::{
        Foundation::{CloseHandle, GetLastError},
        Security::{
            AdjustTokenPrivileges, LookupPrivilegeValueW, LUID_AND_ATTRIBUTES,
            SE_PRIVILEGE_ENABLED, TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
        },
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    };
    let name: Vec<u16> = std::ffi::OsStr::new("SeShutdownPrivilege")
        .encode_wide()
        .chain(Some(0))
        .collect();
    unsafe {
        let mut token = std::ptr::null_mut();
        if OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            &mut token,
        ) == 0
        {
            return Err(
                "Não foi possível habilitar SE_SHUTDOWN_NAME para suspender o computador.".into(),
            );
        }
        let close = |()| {
            CloseHandle(token);
        };
        let mut luid = std::mem::zeroed();
        if LookupPrivilegeValueW(std::ptr::null(), name.as_ptr(), &mut luid) == 0 {
            close(());
            return Err("Não foi possível localizar SE_SHUTDOWN_NAME.".into());
        }
        let state = TOKEN_PRIVILEGES {
            PrivilegeCount: 1,
            Privileges: [LUID_AND_ATTRIBUTES {
                Luid: luid,
                Attributes: SE_PRIVILEGE_ENABLED,
            }],
        };
        let mut previous: TOKEN_PRIVILEGES = std::mem::zeroed();
        if AdjustTokenPrivileges(
            token,
            0,
            &state,
            std::mem::size_of::<TOKEN_PRIVILEGES>() as u32,
            &mut previous,
            std::ptr::null_mut(),
        ) == 0
        {
            close(());
            return Err(
                "O processo não possui SE_SHUTDOWN_NAME para suspender o computador.".into(),
            );
        }
        if GetLastError() != 0 {
            let _ = AdjustTokenPrivileges(
                token,
                0,
                &previous,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            close(());
            return Err(
                "O processo não possui SE_SHUTDOWN_NAME para suspender o computador.".into(),
            );
        }
        let result = SetSuspendState(0, 0, 0) != 0;
        let _ = AdjustTokenPrivileges(
            token,
            0,
            &previous,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        close(());
        if result {
            Ok(())
        } else {
            Err("Não foi possível suspender o computador. Verifique se o Windows permite a suspensão.".into())
        }
    }
}

#[cfg(windows)]
fn on_audio_thread<T, F>(operation: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(
            &windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume,
        ) -> windows::core::Result<T>
        + Send
        + 'static,
{
    std::thread::spawn(move || {
        use windows::{
            core::GUID,
            Win32::{
                Media::Audio::{
                    eMultimedia, eRender, Endpoints::IAudioEndpointVolume, IMMDeviceEnumerator,
                },
                System::Com::{
                    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL,
                    COINIT_MULTITHREADED,
                },
            },
        };
        let initialized = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if initialized.0 < 0 {
            return Err("Não foi possível inicializar o controle de áudio do Windows.".to_string());
        }
        struct ComApartment;
        impl Drop for ComApartment {
            fn drop(&mut self) {
                unsafe { CoUninitialize() }
            }
        }
        let _apartment = ComApartment;
        let clsid = GUID::from_u128(0xBCDE0395_E52F_467C_8E3D_C4579291692E);
        let enumerator: IMMDeviceEnumerator = unsafe { CoCreateInstance(&clsid, None, CLSCTX_ALL) }
            .map_err(|_| "Não foi possível localizar o serviço de áudio do Windows.".to_string())?;
        let endpoint = unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia) }
            .map_err(|_| "Não há dispositivo de saída de áudio disponível.".to_string())?;
        let control: IAudioEndpointVolume = unsafe { endpoint.Activate(CLSCTX_ALL, None) }
            .map_err(|_| {
                "Não foi possível acessar o volume do dispositivo de áudio.".to_string()
            })?;
        operation(&control)
            .map_err(|_| "O Windows não conseguiu aplicar a operação de áudio.".to_string())
    })
    .join()
    .map_err(|_| "O controle de áudio falhou.".to_string())?
}

#[cfg(not(windows))]
pub(crate) struct NativeAdapter;

#[cfg(not(windows))]
impl SystemAdapter for NativeAdapter {
    fn volume(&mut self) -> Result<u8, String> {
        Err("O controle de áudio só está disponível no Windows.".into())
    }
    fn set_volume(&mut self, _: u8) -> Result<(), String> {
        Err("O controle de áudio só está disponível no Windows.".into())
    }
    fn mute(&mut self, _: bool) -> Result<(), String> {
        Err("O controle de áudio só está disponível no Windows.".into())
    }
    fn media_key(&mut self, _: MediaKey) -> Result<(), String> {
        Err("Os controles de mídia só estão disponíveis no Windows.".into())
    }
    fn wallpaper(&mut self) -> Result<Option<String>, String> {
        Err("O papel de parede só pode ser consultado no Windows.".into())
    }
    fn set_wallpaper(&mut self, _: Option<&str>) -> Result<(), String> {
        Err("O papel de parede só pode ser alterado no Windows.".into())
    }
    fn lock_workstation(&mut self) -> Result<(), String> {
        Err("O bloqueio só está disponível no Windows.".into())
    }
    fn brightness(&mut self) -> Result<Option<u8>, String> {
        Err(UNSUPPORTED_BRIGHTNESS.into())
    }
    fn set_brightness(&mut self, _: u8) -> Result<(), String> {
        Err(UNSUPPORTED_BRIGHTNESS.into())
    }
    fn sleep(&mut self) -> Result<(), String> {
        Err("A suspensão só está disponível no Windows.".into())
    }
    fn theme_values(&mut self) -> Result<ThemeSnapshot, String> {
        Err("O controle de tema só está disponível no Windows.".into())
    }
    fn set_theme(&mut self, _: bool) -> Result<(), String> {
        Err("O controle de tema só está disponível no Windows.".into())
    }
    fn restore_theme(&mut self, _: &ThemeSnapshot) -> Result<(), String> {
        Err("O controle de tema só está disponível no Windows.".into())
    }
}

#[cfg(not(windows))]
fn run_wmi_brightness(_: Option<u8>) -> Result<Option<u8>, String> {
    Err(UNSUPPORTED_BRIGHTNESS.into())
}
#[cfg(not(windows))]
fn suspend_windows() -> Result<(), String> {
    Err("A suspensão só está disponível no Windows.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn simulated_audio_respects_volume_bounds_and_has_no_native_effects() {
        let mut sim = SimulatedAdapter {
            volume: 33,
            ..Default::default()
        };
        assert_eq!(
            execute_with(&mut sim, "audio.set_volume", &json!({"volume":100})).unwrap(),
            "Volume definido para 100%"
        );
        assert_eq!(sim.volume, 100);
        assert!(execute_with(&mut sim, "audio.set_volume", &json!({"volume":101})).is_err());
        assert_eq!(sim.volume, 100);
        execute_with(&mut sim, "media.next", &json!({})).unwrap();
        assert_eq!(sim.media_commands, vec![MediaKey::Next]);
        assert!(!sim.muted);
    }

    #[test]
    fn wallpaper_change_persists_previous_state_and_restores_it_with_audit_context() {
        let folder = std::env::temp_dir().join(format!(
            "odekrai-wallpaper-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&folder).unwrap();
        let image = folder.join("new.png");
        std::fs::write(&image, b"\x89PNG\r\n\x1a\ncontent").unwrap();
        let canonical_image = image.canonicalize().unwrap();
        let db = Connection::open_in_memory().unwrap();
        let mut sim = SimulatedAdapter {
            current_wallpaper: Some("C:\\old.bmp".into()),
            ..Default::default()
        };
        execute_managed_with(
            &mut sim,
            "wallpaper.set",
            &json!({"path":image}),
            &db,
            std::slice::from_ref(&folder),
            "user",
        )
        .unwrap();
        assert_eq!(
            sim.current_wallpaper.as_deref(),
            Some(canonical_image.to_string_lossy().as_ref())
        );
        let (previous, created_at, origin): (String, i64, String) = db
            .query_row(
                "SELECT previous_path,created_at,origin FROM wallpaper_history",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(previous, "C:\\old.bmp");
        assert!(created_at > 0);
        assert_eq!(origin, "user");
        execute_managed_with(&mut sim, "wallpaper.restore", &json!({}), &db, &[], "model").unwrap();
        assert_eq!(sim.current_wallpaper.as_deref(), Some("C:\\old.bmp"));
        let restored: Option<i64> = db
            .query_row("SELECT restored_at FROM wallpaper_history", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert!(restored.is_some());
        let _ = std::fs::remove_dir_all(folder);
    }

    #[test]
    fn undo_uses_the_requested_operation_id_and_reports_missing_state_without_native_effects() {
        let folder = std::env::temp_dir().join(format!(
            "odekrai-undo-sim-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&folder).unwrap();
        let image = folder.join("new.png");
        std::fs::write(&image, b"\x89PNG\r\n\x1a\ncontent").unwrap();
        let db = Connection::open_in_memory().unwrap();
        let original_theme = ThemeSnapshot {
            apps_use_light_theme: Some(RegistryValue {
                kind: REG_DWORD_VALUE,
                data: 1_u32.to_le_bytes().to_vec(),
            }),
            system_uses_light_theme: Some(RegistryValue {
                kind: REG_DWORD_VALUE,
                data: 1_u32.to_le_bytes().to_vec(),
            }),
        };
        let mut sim = SimulatedAdapter {
            current_wallpaper: Some("old-wallpaper.png".into()),
            brightness: Some(27),
            theme: original_theme.clone(),
            ..Default::default()
        };
        execute_managed_with_id(
            &mut sim,
            "wallpaper.set",
            &json!({"path": image}),
            &db,
            std::slice::from_ref(&folder),
            "user",
            "wall-op",
        )
        .unwrap();
        execute_managed_with_id(
            &mut sim,
            "display.set_brightness",
            &json!({"brightness": 81}),
            &db,
            &[],
            "user",
            "brightness-op",
        )
        .unwrap();
        execute_managed_with_id(
            &mut sim,
            "theme.set",
            &json!({"mode":"escuro"}),
            &db,
            &[],
            "user",
            "theme-op",
        )
        .unwrap();

        execute_managed_with_id(
            &mut sim,
            "wallpaper.restore",
            &json!({"history_id":"wall-op"}),
            &db,
            &[],
            "user",
            "undo-wall",
        )
        .unwrap();
        execute_managed_with_id(
            &mut sim,
            "display.restore_brightness",
            &json!({"history_id":"brightness-op"}),
            &db,
            &[],
            "user",
            "undo-brightness",
        )
        .unwrap();
        execute_managed_with_id(
            &mut sim,
            "theme.restore",
            &json!({"history_id":"theme-op"}),
            &db,
            &[],
            "user",
            "undo-theme",
        )
        .unwrap();
        assert_eq!(sim.current_wallpaper.as_deref(), Some("old-wallpaper.png"));
        assert_eq!(sim.brightness, Some(27));
        assert_eq!(sim.theme, original_theme);
        assert!(execute_managed_with_id(
            &mut sim,
            "display.restore_brightness",
            &json!({"history_id":"missing"}),
            &db,
            &[],
            "user",
            "undo-missing"
        )
        .is_err());
        let origin: String = db
            .query_row(
                "SELECT origin FROM brightness_history WHERE operation_id='brightness-op'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(origin, "user");
        let _ = std::fs::remove_dir_all(folder);
    }

    #[test]
    fn wallpaper_rejects_paths_outside_allowlist_and_unsupported_extensions() {
        let base = std::env::temp_dir().join(format!(
            "odekrai-wallpaper-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let allowed = base.join("allowed");
        let outside = base.join("outside");
        std::fs::create_dir_all(&allowed).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        let allowed_file = allowed.join("bad.gif");
        let outside_file = outside.join("image.png");
        std::fs::write(&allowed_file, b"GIF89a").unwrap();
        std::fs::write(&outside_file, b"\x89PNG\r\n\x1a\ncontent").unwrap();
        let db = Connection::open_in_memory().unwrap();
        let mut sim = SimulatedAdapter::default();
        assert!(execute_managed_with(
            &mut sim,
            "wallpaper.set",
            &json!({"path":outside_file}),
            &db,
            std::slice::from_ref(&allowed),
            "model",
        )
        .is_err());
        assert!(execute_managed_with(
            &mut sim,
            "wallpaper.set",
            &json!({"path":allowed_file}),
            &db,
            &[allowed],
            "model",
        )
        .is_err());
        assert!(
            db.query_row("SELECT COUNT(*) FROM wallpaper_history", [], |row| row
                .get::<_, i64>(0))
                .unwrap()
                == 0
        );
        assert!(sim.current_wallpaper.is_none());
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn lock_skill_is_registered_with_reversible_write_risk() {
        let skill = crate::security::skills::registry()
            .into_iter()
            .find(|skill| skill.id == "power.lock")
            .unwrap();
        assert_eq!(
            skill.risk(&json!({})).unwrap(),
            crate::policy::Risk::EscritaReversivel
        );
        let mut simulated = SimulatedAdapter::default();
        execute_with(&mut simulated, "power.lock", &json!({})).unwrap();
        assert!(simulated.locked);
    }

    #[test]
    fn brightness_is_bounded_supported_and_restorable_using_only_the_simulator() {
        let db = Connection::open_in_memory().unwrap();
        let mut sim = SimulatedAdapter {
            brightness: Some(37),
            ..Default::default()
        };
        assert!(execute_managed_with(
            &mut sim,
            "display.set_brightness",
            &json!({"brightness":101}),
            &db,
            &[],
            "user"
        )
        .is_err());
        for value in [0, 100] {
            execute_managed_with(
                &mut sim,
                "display.set_brightness",
                &json!({"brightness":value}),
                &db,
                &[],
                "user",
            )
            .unwrap();
            assert_eq!(sim.brightness, Some(value));
            execute_managed_with(
                &mut sim,
                "display.restore_brightness",
                &json!({}),
                &db,
                &[],
                "user",
            )
            .unwrap();
            assert_eq!(sim.brightness, Some(37));
        }
        let (previous, origin): (i64, String) = db
            .query_row(
                "SELECT previous_value,origin FROM brightness_history ORDER BY id LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!((previous, origin.as_str()), (37, "user"));
        let mut unsupported = SimulatedAdapter::default();
        assert_eq!(
            execute_with(&mut unsupported, "display.get_brightness", &json!({})).unwrap_err(),
            UNSUPPORTED_BRIGHTNESS
        );
        assert_eq!(
            execute_with(
                &mut unsupported,
                "display.set_brightness",
                &json!({"brightness":50})
            )
            .unwrap_err(),
            UNSUPPORTED_BRIGHTNESS
        );
    }

    #[test]
    fn sleep_adapter_only_records_simulated_request_and_skill_requires_approval() {
        let mut sim = SimulatedAdapter::default();
        assert_eq!(
            execute_with(&mut sim, "power.sleep", &json!({})).unwrap(),
            "O computador vai suspender agora."
        );
        assert!(sim.sleep_requested);
        let skill = crate::security::skills::registry()
            .into_iter()
            .find(|skill| skill.id == "power.sleep")
            .unwrap();
        assert_eq!(
            skill.risk(&json!({})).unwrap(),
            crate::policy::Risk::EscritaReversivel
        );
    }

    #[test]
    fn brightness_skill_risks_distinguish_read_and_reversible_write() {
        let skills = crate::security::skills::registry();
        let get = skills
            .iter()
            .find(|skill| skill.id == "display.get_brightness")
            .unwrap();
        let set = skills
            .iter()
            .find(|skill| skill.id == "display.set_brightness")
            .unwrap();
        assert_eq!(get.risk(&json!({})).unwrap(), crate::policy::Risk::Leitura);
        assert_eq!(
            set.risk(&json!({"brightness": 100})).unwrap(),
            crate::policy::Risk::EscritaReversivel
        );
        assert!(crate::system_actions::validate(
            "display.set_brightness",
            &json!({"brightness":101})
        )
        .is_err());
    }

    #[test]
    fn experimental_theme_validates_saves_previous_values_and_restores_them_in_the_simulator() {
        let original = ThemeSnapshot {
            apps_use_light_theme: Some(RegistryValue {
                kind: REG_DWORD_VALUE,
                data: 1_u32.to_le_bytes().to_vec(),
            }),
            system_uses_light_theme: Some(RegistryValue {
                kind: REG_DWORD_VALUE,
                data: 0_u32.to_le_bytes().to_vec(),
            }),
        };
        let mut sim = SimulatedAdapter {
            theme: original.clone(),
            ..Default::default()
        };
        let db = Connection::open_in_memory().unwrap();
        assert!(crate::system_actions::validate("theme.set", &json!({"mode":"blue"})).is_err());
        assert!(crate::system_actions::validate(
            "theme.set",
            &json!({"mode":"escuro","extra":true})
        )
        .is_err());
        let theme_skill = crate::security::skills::registry()
            .into_iter()
            .find(|skill| skill.id == "theme.set")
            .unwrap();
        assert_eq!(
            theme_skill.risk(&json!({"mode":"escuro"})).unwrap(),
            crate::policy::Risk::EscritaReversivel
        );
        assert!(execute_managed_with(
            &mut sim,
            "theme.set",
            &json!({"mode":"blue"}),
            &db,
            &[],
            "user"
        )
        .is_err());
        assert_eq!(sim.theme, original);
        execute_managed_with(
            &mut sim,
            "theme.set",
            &json!({"mode":"escuro"}),
            &db,
            &[],
            "user",
        )
        .unwrap();
        let dark = Some(RegistryValue {
            kind: REG_DWORD_VALUE,
            data: 0_u32.to_le_bytes().to_vec(),
        });
        assert_eq!(
            sim.theme,
            ThemeSnapshot {
                apps_use_light_theme: dark.clone(),
                system_uses_light_theme: dark
            }
        );
        let (apps_type, apps_data, system_type, system_data, origin): (
            i64,
            Vec<u8>,
            i64,
            Vec<u8>,
            String,
        ) = db
            .query_row(
                "SELECT apps_type,apps_data,system_type,system_data,origin FROM theme_history",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(
            (apps_type, system_type, origin.as_str()),
            (REG_DWORD_VALUE as i64, REG_DWORD_VALUE as i64, "user")
        );
        assert_eq!(apps_data, 1_u32.to_le_bytes());
        assert_eq!(system_data, 0_u32.to_le_bytes());
        execute_managed_with(&mut sim, "theme.restore", &json!({}), &db, &[], "user").unwrap();
        assert_eq!(sim.theme, original);
        let restored: Option<i64> = db
            .query_row("SELECT restored_at FROM theme_history", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert!(restored.is_some());
    }

    #[test]
    fn registered_system_skills_use_closed_schemas_and_rust_risk() {
        for id in [
            "audio.get_volume",
            "audio.set_volume",
            "audio.mute",
            "media.play_pause",
            "media.next",
            "media.previous",
            "wallpaper.set",
            "wallpaper.restore",
            "power.lock",
            "display.get_brightness",
            "display.set_brightness",
            "display.restore_brightness",
            "power.sleep",
            "theme.set",
            "theme.restore",
        ] {
            let skill = crate::security::skills::registry()
                .into_iter()
                .find(|skill| skill.id == id)
                .expect("skill must be registered");
            assert!(skill
                .risk(&match id {
                    "audio.set_volume" => json!({"volume": 40}),
                    "audio.mute" => json!({"muted": true}),
                    "wallpaper.set" => json!({"path":"C:\\wallpaper.png"}),
                    "display.set_brightness" => json!({"brightness": 50}),
                    "theme.set" => json!({"mode":"escuro"}),
                    _ => json!({}),
                })
                .is_ok());
        }
        assert!(crate::system_actions::validate(
            "wallpaper.set",
            &json!({"path":"C:\\wallpaper.gif"})
        )
        .is_err());
        assert!(
            crate::system_actions::validate("audio.set_volume", &json!({"volume": 101})).is_err()
        );
        assert!(crate::system_actions::validate(
            "audio.set_volume",
            &json!({"volume": 20, "extra": true})
        )
        .is_err());
    }
}
