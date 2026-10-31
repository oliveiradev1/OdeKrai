// A animação direcional da janela usa uma abordagem inspirada na ilha Windows
// de Coucou; geometria reimplementada para os contratos nativos do OdeKrai.
// Referência MIT: Louis-CFM/coucou, windows-v0.5.3, © Louis Raillé, 2026.
use pet_position::{fit_size, WindowPosition, WindowSize};
use std::{
    fs,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};
use tauri::{
    Emitter, LogicalPosition, LogicalSize, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};
use tauri_plugin_opener::OpenerExt;

#[derive(Default)]
struct MicrophoneOptIn(AtomicBool);

fn microphone_permission_allowed(window_label: &str, enabled: bool, is_microphone: bool) -> bool {
    window_label == "main" && enabled && is_microphone
}

fn microphone_permission_response(
    window_label: &str,
    enabled: bool,
    is_microphone: bool,
) -> tauri::webview::PermissionResponse {
    if microphone_permission_allowed(window_label, enabled, is_microphone) {
        tauri::webview::PermissionResponse::Default
    } else {
        tauri::webview::PermissionResponse::Deny
    }
}

#[tauri::command]
fn set_microphone_opt_in(
    caller: tauri::WebviewWindow,
    state: tauri::State<'_, MicrophoneOptIn>,
    enabled: bool,
) -> Result<(), String> {
    require_main(&caller)?;
    state.0.store(enabled, Ordering::SeqCst);
    Ok(())
}

#[derive(serde::Serialize)]
struct BuildInfo {
    version: String,
    commit: &'static str,
    build_date: &'static str,
    webview_version: String,
    windows_version: String,
}

#[tauri::command]
fn get_build_info(caller: tauri::WebviewWindow) -> Result<BuildInfo, String> {
    require_main(&caller)?;
    Ok(BuildInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        commit: option_env!("ODEKRAI_BUILD_COMMIT").unwrap_or("local"),
        build_date: option_env!("ODEKRAI_BUILD_DATE").unwrap_or("não definida"),
        webview_version: tauri::webview_version().map_err(|error| error.to_string())?,
        windows_version: windows_version(),
    })
}

fn windows_version() -> String {
    #[cfg(windows)]
    {
        let output = std::process::Command::new("cmd.exe")
            .args(["/C", "ver"])
            .output();
        if let Ok(output) = output {
            let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if output.status.success() && !text.is_empty() {
                return text;
            }
        }
    }
    "Indisponível".to_string()
}

#[tauri::command]
fn open_build_commit(app: tauri::AppHandle, caller: tauri::WebviewWindow) -> Result<(), String> {
    require_main(&caller)?;
    let commit = option_env!("ODEKRAI_BUILD_COMMIT").unwrap_or("local");
    if commit == "local"
        || !commit
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err("Este build não tem um commit publicado para abrir.".into());
    }
    app.opener()
        .open_url(
            format!("https://github.com/Anceryx/OdeKrai/commit/{commit}"),
            None::<&str>,
        )
        .map_err(|_| "Não foi possível abrir o commit no navegador padrão.".to_string())
}

#[tauri::command]
fn open_logs_folder(app: tauri::AppHandle, caller: tauri::WebviewWindow) -> Result<(), String> {
    require_main(&caller)?;
    let logs = app
        .path()
        .app_data_dir()
        .map_err(|_| "Não foi possível localizar os logs.".to_string())?
        .join("logs");
    if !logs.is_dir() {
        return Err("A pasta de logs ainda não foi criada.".into());
    }
    std::process::Command::new("explorer.exe")
        .arg(logs)
        .spawn()
        .map(|_| ())
        .map_err(|_| "Não foi possível abrir a pasta de logs.".to_string())
}

pub mod agent;
mod agent_home;
mod agent_runtime;
mod calendar;
mod code_delegate;
mod diagnostics;
mod discord;
mod external_links;
mod google_calendar;
pub mod google_oauth;
mod hardware;
mod history;
mod local_models;
pub mod mcp;
mod mcp_registry;
mod memory_skills;
mod pet_alerts;
mod pet_position;
mod policy;
mod productivity;
mod profile;
mod providers;
mod reminders;
mod screen_capture;
mod security;
mod shardcloud;
mod spreadsheets;
mod system_actions;
mod system_controls;
mod tray;
mod updates;
mod web_fetch;
mod window_sizing;

pub(crate) fn require_main(window: &tauri::WebviewWindow) -> Result<(), String> {
    require_caller(window, &["main"])
}

#[tauri::command]
async fn show_main_window(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
) -> Result<(), String> {
    require_main(&caller)?;
    let window = app
        .get_webview_window("main")
        .ok_or("A janela principal não está disponível.")?;
    if window
        .is_minimized()
        .map_err(|_| "Não foi possível consultar a janela principal.")?
    {
        window
            .unminimize()
            .map_err(|_| "Não foi possível restaurar a janela principal.")?;
    }
    window
        .show()
        .map_err(|_| "Não foi possível exibir a janela principal.")?;
    window
        .set_focus()
        .map_err(|_| "Não foi possível focar a janela principal.".to_string())
}

fn require_caller(window: &tauri::WebviewWindow, allowed: &[&str]) -> Result<(), String> {
    if caller_label_allowed(window.label(), allowed) {
        Ok(())
    } else {
        Err("Comando indisponível para esta janela.".into())
    }
}

fn caller_label_allowed(label: &str, allowed: &[&str]) -> bool {
    allowed.contains(&label)
}

#[cfg(test)]
mod security_caller_tests {
    use super::caller_label_allowed;

    #[test]
    fn approval_commands_only_accept_the_approval_window() {
        assert!(caller_label_allowed("approval", &["approval"]));
        assert!(!caller_label_allowed("main", &["approval"]));
        assert!(!caller_label_allowed("pet", &["approval"]));
    }
}

#[cfg(test)]
mod window_config_tests {
    use serde_json::Value;

    #[test]
    fn main_window_uses_requested_default_and_minimum_size() {
        let config: Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let main = &config["app"]["windows"][0];

        assert_eq!(main["width"], 1250);
        assert_eq!(main["height"], 700);
        assert_eq!(main["minWidth"], 1000);
        assert_eq!(main["minHeight"], 600);
        assert_eq!(main["center"], true);
    }
}

#[cfg(test)]
mod feature_window_lifecycle_tests {
    use super::{visibility_action, VisibilityAction};

    #[test]
    fn disabled_feature_windows_are_closed_and_absent_windows_are_not_created() {
        assert_eq!(visibility_action(false, false), VisibilityAction::Noop);
        assert_eq!(visibility_action(false, true), VisibilityAction::Close);
        assert_eq!(
            visibility_action(true, false),
            VisibilityAction::CreateOrShow
        );
        assert_eq!(
            visibility_action(true, true),
            VisibilityAction::CreateOrShow
        );
    }
}

#[cfg(test)]
mod pet_window_focus_tests {
    use super::PET_WINDOW_FOCUSABLE;

    #[test]
    fn pet_window_does_not_take_keyboard_focus() {
        const { assert!(!PET_WINDOW_FOCUSABLE) };
    }
}

#[cfg(test)]
mod pet_position_tests {
    use super::pet_position::{clamp_position, WindowPosition, WindowSize, WorkArea};

    #[test]
    fn pet_position_is_limited_to_work_area_including_negative_monitor_origins() {
        assert_eq!(
            clamp_position(
                WindowPosition {
                    x: 4000.0,
                    y: -400.0
                },
                WorkArea {
                    left: -1920.0,
                    top: 40.0,
                    width: 1920.0,
                    height: 1040.0,
                },
                WindowSize {
                    width: 300.0,
                    height: 180.0,
                },
            ),
            WindowPosition { x: -300.0, y: 40.0 }
        );
    }

    #[test]
    fn oversized_pet_stays_at_the_work_area_origin() {
        assert_eq!(
            clamp_position(
                WindowPosition { x: 500.0, y: 500.0 },
                WorkArea {
                    left: -100.0,
                    top: 20.0,
                    width: 200.0,
                    height: 120.0,
                },
                WindowSize {
                    width: 300.0,
                    height: 180.0,
                },
            ),
            WindowPosition { x: -100.0, y: 20.0 }
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VisibilityAction {
    CreateOrShow,
    Close,
    Noop,
}

fn visibility_action(visible: bool, exists: bool) -> VisibilityAction {
    match (visible, exists) {
        (true, _) => VisibilityAction::CreateOrShow,
        (false, true) => VisibilityAction::Close,
        (false, false) => VisibilityAction::Noop,
    }
}

#[derive(Default)]
struct PetPositionStore {
    saved: Mutex<Option<pet_position::PersistedPetPosition>>,
    last_write: Mutex<Option<Instant>>,
    expected_move: Mutex<Option<WindowPosition>>,
    transitioning: AtomicBool,
    visibility_transition: tokio::sync::Mutex<()>,
}

const PET_POSITION_FILE: &str = "pet-position.json";
const PET_POSITION_WRITE_INTERVAL: Duration = Duration::from_secs(1);
const PET_WINDOW_FOCUSABLE: bool = false;

fn pet_position_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|directory| directory.join(PET_POSITION_FILE))
        .map_err(|_| "Não foi possível localizar os dados do pet.".to_string())
}

fn save_pet_position(
    app: &tauri::AppHandle,
    saved: pet_position::PersistedPetPosition,
    force: bool,
) {
    let Some(store) = app.try_state::<PetPositionStore>() else {
        return;
    };
    let Ok(mut current) = store.saved.lock() else {
        return;
    };
    *current = Some(saved.clone());
    let now = Instant::now();
    let Ok(mut last_write) = store.last_write.lock() else {
        return;
    };
    if !force
        && last_write.is_some_and(|last| now.duration_since(last) < PET_POSITION_WRITE_INTERVAL)
    {
        return;
    }
    let Ok(path) = pet_position_path(app) else {
        return;
    };
    let Some(parent) = path.parent() else {
        return;
    };
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    let Ok(bytes) = serde_json::to_vec(&saved) else {
        return;
    };
    if fs::write(path, bytes).is_ok() {
        *last_write = Some(now);
    }
}

fn load_pet_position(app: &tauri::AppHandle) {
    let Ok(path) = pet_position_path(app) else {
        return;
    };
    let Ok(bytes) = fs::read(path) else {
        return;
    };
    let Ok(saved) = serde_json::from_slice::<pet_position::PersistedPetPosition>(&bytes) else {
        return;
    };
    if let Some(store) = app.try_state::<PetPositionStore>() {
        if let Ok(mut current) = store.saved.lock() {
            *current = Some(saved);
        }
    }
}

fn monitor_id(monitor: &tauri::Monitor) -> pet_position::MonitorId {
    let area = monitor.work_area();
    pet_position::MonitorId {
        name: monitor.name().cloned(),
        left: area.position.x,
        top: area.position.y,
        width: area.size.width,
        height: area.size.height,
        scale_factor: monitor.scale_factor(),
    }
}

fn monitor_work_area(monitor: &tauri::Monitor) -> pet_position::WorkArea {
    let scale = monitor.scale_factor();
    let work = monitor.work_area();
    pet_position::WorkArea {
        left: work.position.x as f64 / scale,
        top: work.position.y as f64 / scale,
        width: work.size.width as f64 / scale,
        height: work.size.height as f64 / scale,
    }
}

fn window_size_logical(window: &WebviewWindow) -> Result<WindowSize, String> {
    let scale = window.scale_factor().map_err(|error| error.to_string())?;
    let size = window.inner_size().map_err(|error| error.to_string())?;
    Ok(WindowSize {
        width: size.width as f64 / scale,
        height: size.height as f64 / scale,
    })
}

fn pet_window_transition_positions(
    window: &WebviewWindow,
) -> Result<(WindowPosition, WindowPosition, f64), String> {
    let monitor = window
        .current_monitor()
        .map_err(|error| error.to_string())?
        .or_else(|| window.primary_monitor().ok().flatten())
        .ok_or("Não foi possível localizar o monitor do pet.")?;
    let scale = window.scale_factor().map_err(|error| error.to_string())?;
    let physical = window.outer_position().map_err(|error| error.to_string())?;
    let position = WindowPosition {
        x: physical.x as f64 / scale,
        y: physical.y as f64 / scale,
    };
    let size = window_size_logical(window)?;
    let edge = pet_position::nearest_edge(position, size, monitor_work_area(&monitor));
    let outside = pet_position::outside_edge_position(position, size, edge);
    Ok((position, outside, scale))
}

async fn animate_pet_window(
    app: &tauri::AppHandle,
    window: &WebviewWindow,
    start: WindowPosition,
    end: WindowPosition,
    scale: f64,
) -> Result<(), String> {
    const STEPS: u32 = 12;
    const STEP_DURATION: Duration = Duration::from_millis(16);
    if let Some(store) = app.try_state::<PetPositionStore>() {
        store.transitioning.store(true, Ordering::SeqCst);
    }
    let result = async {
        for step in 1..=STEPS {
            tokio::time::sleep(STEP_DURATION).await;
            let position =
                pet_position::transition_position(start, end, step as f64 / STEPS as f64);
            set_physical_position_expected(window, app, position, scale)?;
        }
        Ok(())
    }
    .await;
    tokio::time::sleep(STEP_DURATION).await;
    if let Some(store) = app.try_state::<PetPositionStore>() {
        store.transitioning.store(false, Ordering::SeqCst);
    }
    result
}

fn saved_position_for_monitor(
    app: &tauri::AppHandle,
    id: &pet_position::MonitorId,
) -> Option<pet_position::RelativePosition> {
    let store = app.try_state::<PetPositionStore>()?;
    let saved = store.saved.lock().ok()?;
    let saved = saved.as_ref()?;
    pet_position::saved_for_monitor(saved, id).map(|entry| entry.relative)
}

fn set_position_expected(
    window: &WebviewWindow,
    app: &tauri::AppHandle,
    position: WindowPosition,
) -> Result<(), String> {
    if let Some(store) = app.try_state::<PetPositionStore>() {
        if let Ok(mut expected) = store.expected_move.lock() {
            *expected = Some(position);
        }
    }
    window
        .set_position(LogicalPosition::new(position.x, position.y))
        .map_err(|error| error.to_string())
}

fn set_physical_position_expected(
    window: &WebviewWindow,
    app: &tauri::AppHandle,
    position: WindowPosition,
    scale_factor: f64,
) -> Result<(), String> {
    if let Some(store) = app.try_state::<PetPositionStore>() {
        if let Ok(mut expected) = store.expected_move.lock() {
            *expected = Some(position);
        }
    }
    window
        .set_position(tauri::PhysicalPosition::new(
            (position.x * scale_factor).round() as i32,
            (position.y * scale_factor).round() as i32,
        ))
        .map_err(|error| error.to_string())
}

fn consume_expected_move(
    app: &tauri::AppHandle,
    physical: tauri::PhysicalPosition<i32>,
    scale: f64,
) -> bool {
    let Some(store) = app.try_state::<PetPositionStore>() else {
        return false;
    };
    let Ok(mut expected) = store.expected_move.lock() else {
        return false;
    };
    let actual = WindowPosition {
        x: physical.x as f64 / scale,
        y: physical.y as f64 / scale,
    };
    if expected.is_some_and(|position| {
        (position.x - actual.x).abs() < 2.0 && (position.y - actual.y).abs() < 2.0
    }) {
        *expected = None;
        true
    } else {
        false
    }
}

fn record_window_position(
    app: &tauri::AppHandle,
    window: &WebviewWindow,
    physical_position: tauri::PhysicalPosition<i32>,
    force: bool,
) -> Result<WindowPosition, String> {
    let monitor = window
        .current_monitor()
        .map_err(|error| error.to_string())?
        .or(window
            .primary_monitor()
            .map_err(|error| error.to_string())?)
        .ok_or("Não foi possível localizar o monitor do pet.")?;
    let scale = window.scale_factor().map_err(|error| error.to_string())?;
    let work_area = monitor_work_area(&monitor);
    let window_size = window_size_logical(window)?;
    let position = pet_position::clamp_position(
        WindowPosition {
            x: physical_position.x as f64 / scale,
            y: physical_position.y as f64 / scale,
        },
        work_area,
        window_size,
    );
    let id = monitor_id(&monitor);
    let relative = pet_position::relative_position(position, work_area, window_size);
    store_monitor_position(app, id, relative, force);
    Ok(position)
}

fn store_monitor_position(
    app: &tauri::AppHandle,
    id: pet_position::MonitorId,
    relative: pet_position::RelativePosition,
    force: bool,
) {
    let mut saved = app
        .try_state::<PetPositionStore>()
        .and_then(|store| store.saved.lock().ok()?.clone())
        .unwrap_or_else(|| pet_position::PersistedPetPosition {
            active_monitor: id.clone(),
            monitors: Vec::new(),
        });
    saved.active_monitor = id.clone();
    if let Some(existing) = saved
        .monitors
        .iter_mut()
        .find(|entry| pet_position::monitor_matches(&entry.monitor, &id))
    {
        existing.monitor = id;
        existing.relative = relative;
    } else {
        saved.monitors.push(pet_position::MonitorPosition {
            monitor: id,
            relative,
        });
    }
    save_pet_position(app, saved, force);
}

fn restore_pet_position(app: &tauri::AppHandle, window: &WebviewWindow) -> Result<(), String> {
    let monitors = window
        .available_monitors()
        .map_err(|error| error.to_string())?;
    if monitors.is_empty() {
        return Ok(());
    }
    let ids: Vec<_> = monitors.iter().map(monitor_id).collect();
    let primary_id = window
        .primary_monitor()
        .map_err(|error| error.to_string())?
        .as_ref()
        .map(monitor_id);
    let primary_index = primary_id
        .as_ref()
        .and_then(|id| pet_position::matching_monitor_index(id, &ids));
    let saved = app
        .try_state::<PetPositionStore>()
        .and_then(|store| store.saved.lock().ok()?.clone());
    let active_index = saved
        .as_ref()
        .and_then(|saved| pet_position::selected_monitor_index(saved, &ids, primary_index))
        .or(primary_index)
        .unwrap_or(0);
    let monitor = &monitors[active_index];
    let id = &ids[active_index];
    let work_area = monitor_work_area(monitor);
    let window_size = window_size_logical(window)?;
    let current = window.outer_position().map_err(|error| error.to_string())?;
    let current_scale = window.scale_factor().map_err(|error| error.to_string())?;
    let current_position = WindowPosition {
        x: current.x as f64 / current_scale,
        y: current.y as f64 / current_scale,
    };
    let restored = saved
        .as_ref()
        .and_then(|saved| pet_position::saved_for_monitor(saved, id))
        .map(|entry| pet_position::position_from_relative(entry.relative, work_area, window_size))
        .unwrap_or_else(|| pet_position::clamp_position(current_position, work_area, window_size));
    set_physical_position_expected(window, app, restored, monitor.scale_factor())?;
    let relative = pet_position::relative_position(restored, work_area, window_size);
    store_monitor_position(app, id.clone(), relative, true);
    Ok(())
}

fn reconcile_current_monitor(app: &tauri::AppHandle, window: &WebviewWindow, scale_factor: f64) {
    let Ok(Some(monitor)) = window.current_monitor() else {
        return;
    };
    let id = monitor_id(&monitor);
    let work_area = monitor_work_area(&monitor);
    let Ok(size) = window.inner_size() else {
        return;
    };
    let window_size = WindowSize {
        width: size.width as f64 / scale_factor,
        height: size.height as f64 / scale_factor,
    };
    let Ok(position) = window.outer_position() else {
        return;
    };
    let relative = saved_position_for_monitor(app, &id);
    let target = relative
        .map(|relative| pet_position::position_from_relative(relative, work_area, window_size))
        .unwrap_or_else(|| {
            pet_position::clamp_position(
                WindowPosition {
                    x: position.x as f64 / scale_factor,
                    y: position.y as f64 / scale_factor,
                },
                work_area,
                window_size,
            )
        });
    if set_physical_position_expected(window, app, target, monitor.scale_factor()).is_ok() {
        let relative = pet_position::relative_position(target, work_area, window_size);
        store_monitor_position(app, id, relative, true);
    }
}

async fn ensure_pet_window(app: &tauri::AppHandle) -> Result<WebviewWindow, String> {
    if let Some(window) = app.get_webview_window("pet") {
        return Ok(window);
    }
    let window = WebviewWindowBuilder::new(app, "pet", WebviewUrl::App("index.html".into()))
        .title("Pet do OdeKrai")
        .inner_size(280.0, 104.0)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .focusable(PET_WINDOW_FOCUSABLE)
        .resizable(false)
        .skip_taskbar(true)
        .visible(false)
        .build()
        .map_err(|_| "Não foi possível preparar a janela do pet.".to_string())?;
    let handle = app.clone();
    let moved_window = window.clone();
    window.on_window_event(move |event| match event {
        tauri::WindowEvent::Moved(position) => {
            if !moved_window.is_visible().unwrap_or(false) {
                return;
            }
            if handle
                .try_state::<PetPositionStore>()
                .is_some_and(|store| store.transitioning.load(Ordering::SeqCst))
            {
                return;
            }
            let scale = moved_window.scale_factor().unwrap_or(1.0);
            if consume_expected_move(&handle, *position, scale) {
                return;
            }
            if let Ok(Some(monitor)) = moved_window.current_monitor() {
                let current_id = monitor_id(&monitor);
                let active_changed = handle
                    .try_state::<PetPositionStore>()
                    .and_then(|store| store.saved.lock().ok()?.clone())
                    .is_some_and(|saved| {
                        !pet_position::monitor_matches(&saved.active_monitor, &current_id)
                            && pet_position::saved_for_monitor(&saved, &current_id).is_some()
                    });
                if active_changed {
                    reconcile_current_monitor(&handle, &moved_window, scale);
                    return;
                }
            }
            if let Ok(clamped) = record_window_position(&handle, &moved_window, *position, false) {
                let x = position.x as f64 / scale;
                let y = position.y as f64 / scale;
                if (clamped.x - x).abs() > 1.0 || (clamped.y - y).abs() > 1.0 {
                    let _ = set_position_expected(&moved_window, &handle, clamped);
                }
                let _ = handle.emit("pet-moved", (clamped.x, clamped.y));
            }
        }
        tauri::WindowEvent::ScaleFactorChanged { scale_factor, .. }
            if moved_window.is_visible().unwrap_or(false) =>
        {
            reconcile_current_monitor(&handle, &moved_window, *scale_factor);
        }
        _ => {}
    });
    Ok(window)
}

async fn show_pet_quick_command(app: &tauri::AppHandle) -> Result<(), String> {
    let window = ensure_pet_window(app).await?;
    if !window.is_visible().unwrap_or(false) {
        restore_pet_position(app, &window)?;
        window
            .show()
            .map_err(|_| "Não foi possível mostrar a ilha para o comando rápido.".to_string())?;
        let _ = app.emit("pet-window-visibility", true);
    }
    let _ = app.emit("pet-quick-focus-request", ());
    Ok(())
}

pub(crate) async fn toggle_pet_from_tray(app: &tauri::AppHandle) {
    match app.get_webview_window("pet") {
        Some(window) if window.is_visible().unwrap_or(false) => {
            if let Ok(position) = window.outer_position() {
                let _ = record_window_position(app, &window, position, true);
            }
            let _ = window.hide();
            let _ = app.emit("pet-window-visibility", false);
        }
        _ => {
            if let Ok(window) = ensure_pet_window(app).await {
                let _ = restore_pet_position(app, &window);
                if window.show().is_ok() {
                    let _ = app.emit("pet-window-visibility", true);
                }
            }
        }
    }
}

#[tauri::command]
async fn set_pet_visible(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    visible: bool,
) -> Result<(), String> {
    require_caller(&caller, &["main", "pet"])?;
    let transition_lock = app.state::<PetPositionStore>();
    let _transition = transition_lock.visibility_transition.lock().await;
    let existing = app.get_webview_window("pet");
    match visibility_action(visible, existing.is_some()) {
        VisibilityAction::CreateOrShow => {
            let window = ensure_pet_window(&app).await?;
            if window.is_visible().unwrap_or(false) {
                window.show().map_err(|error| error.to_string())?;
            } else {
                restore_pet_position(&app, &window)?;
                let (target, outside, scale) = pet_window_transition_positions(&window)?;
                set_physical_position_expected(&window, &app, outside, scale)?;
                window.show().map_err(|error| error.to_string())?;
                let _ = app.emit("pet-window-visibility", true);
                animate_pet_window(&app, &window, outside, target, scale).await?;
            }
        }
        VisibilityAction::Close => {
            let window = existing.ok_or("Janela do pet indisponível para fechar.")?;
            if window.is_visible().unwrap_or(false) {
                if let Ok(position) = window.outer_position() {
                    let _ = record_window_position(&app, &window, position, true);
                }
                let (position, outside, scale) = pet_window_transition_positions(&window)?;
                animate_pet_window(&app, &window, position, outside, scale).await?;
            }
            window.close().map_err(|error| error.to_string())?;
        }
        VisibilityAction::Noop => {}
    }
    if !visible {
        let _ = app.emit("pet-window-visibility", false);
    }
    Ok(())
}

#[tauri::command]
async fn set_pet_keyboard_focus(caller: tauri::WebviewWindow, focused: bool) -> Result<(), String> {
    require_caller(&caller, &["pet"])?;
    if focused {
        caller
            .set_focusable(true)
            .map_err(|_| "Não foi possível atualizar o foco do pet.".to_string())?;
        caller.set_focus().map_err(|_| {
            let _ = caller.set_focusable(false);
            "Não foi possível focar o campo rápido do pet.".to_string()
        })?;
    } else {
        caller
            .set_focusable(false)
            .map_err(|_| "Não foi possível atualizar o foco do pet.".to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn set_pet_position(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    x: f64,
    y: f64,
    size: f64,
) -> Result<(), String> {
    require_caller(&caller, &["main", "pet"])?;
    if !x.is_finite()
        || !y.is_finite()
        || !(-100_000.0..=100_000.0).contains(&x)
        || !(-100_000.0..=100_000.0).contains(&y)
        || !(0.75..=1.5).contains(&size)
    {
        return Err("Posição ou escala inválida".into());
    }
    let window = ensure_pet_window(&app).await?;
    let monitor = window
        .current_monitor()
        .map_err(|error| error.to_string())?
        .or_else(|| window.primary_monitor().ok().flatten())
        .ok_or("Não foi possível localizar o monitor do pet.")?;
    let scale = window.scale_factor().map_err(|error| error.to_string())?;
    let work = monitor.work_area();
    let current = window.inner_size().map_err(|error| error.to_string())?;
    let width = current.width as f64 / scale;
    let height = current.height as f64 / scale;
    let position = pet_position::clamp_position(
        WindowPosition { x, y },
        pet_position::WorkArea {
            left: work.position.x as f64 / scale,
            top: work.position.y as f64 / scale,
            width: work.size.width as f64 / scale,
            height: work.size.height as f64 / scale,
        },
        WindowSize { width, height },
    );
    set_position_expected(&window, &app, position)?;
    let has_saved_position = app
        .try_state::<PetPositionStore>()
        .and_then(|store| store.saved.lock().ok().map(|saved| saved.is_some()))
        .unwrap_or(false);
    if window.is_visible().unwrap_or(false) || !has_saved_position {
        let id = monitor_id(&monitor);
        let relative = pet_position::relative_position(
            position,
            monitor_work_area(&monitor),
            WindowSize { width, height },
        );
        store_monitor_position(&app, id, relative, true);
    }
    Ok(())
}

#[tauri::command]
async fn set_pet_topmost(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    enabled: bool,
) -> Result<(), String> {
    require_caller(&caller, &["main", "pet"])?;
    if let Some(window) = app.get_webview_window("pet") {
        window
            .set_always_on_top(enabled)
            .map_err(|error| error.to_string())
    } else {
        Ok(())
    }
}

#[tauri::command]
async fn set_pet_size(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    width: f64,
    height: f64,
) -> Result<(), String> {
    require_caller(&caller, &["main", "pet"])?;
    if !width.is_finite()
        || !height.is_finite()
        || !(80.0..=420.0).contains(&width)
        || !(80.0..=360.0).contains(&height)
    {
        return Err("Dimensão inválida".into());
    }
    let window = app
        .get_webview_window("pet")
        .ok_or("Janela do pet indisponível")?;
    let monitor = window
        .current_monitor()
        .map_err(|error| error.to_string())?
        .or_else(|| window.primary_monitor().ok().flatten())
        .ok_or("Não foi possível localizar o monitor do pet.")?;
    let scale = window.scale_factor().map_err(|error| error.to_string())?;
    let position = window.outer_position().map_err(|error| error.to_string())?;
    let work = monitor.work_area();
    let work_area = pet_position::WorkArea {
        left: work.position.x as f64 / scale,
        top: work.position.y as f64 / scale,
        width: work.size.width as f64 / scale,
        height: work.size.height as f64 / scale,
    };
    let size = fit_size(WindowSize { width, height }, work_area);
    window
        .set_size(LogicalSize::new(size.width, size.height))
        .map_err(|error| error.to_string())?;
    let position = pet_position::clamp_position(
        WindowPosition {
            x: position.x as f64 / scale,
            y: position.y as f64 / scale,
        },
        work_area,
        size,
    );
    set_position_expected(&window, &app, position)
}
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() -> Result<(), tauri::Error> {
    tauri::Builder::default()
        .manage(MicrophoneOptIn::default())
        .on_permission_request(|webview, kind| {
            use tauri::webview::PermissionKind;
            let enabled = webview
                .app_handle()
                .try_state::<MicrophoneOptIn>()
                .is_some_and(|state| state.0.load(Ordering::SeqCst));
            microphone_permission_response(
                webview.label(),
                enabled,
                kind == PermissionKind::Microphone,
            )
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            tray::show_main(app);
        }))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        security::handle_global_shortcut(app, shortcut.id());
                    }
                })
                .build(),
        )
        .manage(providers::ProviderRequests::default())
        .manage(local_models::Downloads::default())
        .manage(PetPositionStore::default())
        .manage(security::SecurityState::new(true))
        .manage(shardcloud::ShardCloudState::default())
        .manage(tray::CloseBehaviorState::default())
        .manage(tray::ShutdownState::default())
        .manage(reminders::ReminderScheduler::default())
        .manage(pet_alerts::PetAlertQueue::default())
        .manage(mcp_registry::McpRuntime::default())
        .manage(google_oauth::GoogleOAuthState::default())
        .setup(|app| {
            reminders::start_scheduler(app.handle());
            load_pet_position(app.handle());
            let startup_state = diagnostics::setup(app.handle()).unwrap_or_else(|error| {
                log::error!("Não foi possível preparar o estado de inicialização: {error}");
                diagnostics::StartupState::safe_fallback()
            });
            app.manage(startup_state);
            if let Some(window) = app.get_webview_window("main") {
                window_sizing::fit_main_window(&window).map_err(std::io::Error::other)?;
                tray::install_main_close_handler(app.handle(), &window);
            }
            let halted = security::load_halted(app.handle());
            if let Some(state) = app.try_state::<security::SecurityState>() {
                state.set_initial_halted(halted);
            }
            security::init_shortcut(app.handle());
            diagnostics::log_setup_complete(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            set_microphone_opt_in,
            show_main_window,
            open_build_commit,
            open_logs_folder,
            code_delegate::get_code_tool_settings,
            code_delegate::set_code_tool,
            agent_runtime::agent_run,
            agent_runtime::agent_cancel,
            agent_runtime::agent_availability,
            agent_runtime::get_agent_limits,
            agent_runtime::set_agent_limits,
            get_build_info,
            external_links::open_external_link,
            external_links::open_cloud_link,
            external_links::open_update_release,
            updates::check_for_update,
            shardcloud::save_shardcloud_token,
            shardcloud::has_shardcloud_token,
            shardcloud::remove_shardcloud_token,
            shardcloud::get_shardcloud_monthly_budget,
            shardcloud::set_shardcloud_monthly_budget,
            shardcloud::shardcloud_test_connection,
            shardcloud::shardcloud_list_vms,
            shardcloud::shardcloud_get_vm_details,
            shardcloud::shardcloud_get_vm_metrics,
            shardcloud::shardcloud_list_distros,
            hardware::get_hardware_profile,
            local_models::download_local_model,
            local_models::cancel_local_model_download,
            local_models::delete_local_model,
            local_models::inspect_local_models,
            diagnostics::get_startup_state,
            diagnostics::complete_startup,
            diagnostics::record_diagnostic_event,
            set_pet_visible,
            set_pet_keyboard_focus,
            set_pet_position,
            set_pet_topmost,
            set_pet_size,
            pet_alerts::get_pet_alert_state,
            pet_alerts::enqueue_pet_activity,
            pet_alerts::dismiss_pet_alert,
            tray::set_close_behavior,
            providers::save_provider_credential,
            providers::has_provider_credential,
            providers::remove_provider_credential,
            providers::send_provider_message,
            providers::cancel_provider_request,
            providers::stream_provider_message,
            providers::validate_provider_endpoint,
            providers::list_provider_models,
            providers::discover_local_models,
            providers::test_provider_connection,
            providers::ping_provider,
            providers::get_provider_capabilities,
            providers::audio_transcription::transcribe_audio,
            mcp_registry::mcp_list_servers,
            mcp_registry::mcp_add_server,
            mcp_registry::mcp_set_secret,
            mcp_registry::mcp_set_enabled,
            mcp_registry::mcp_test_connection,
            mcp_registry::mcp_remove_server,
            google_oauth::google_oauth_connect,
            google_oauth::google_oauth_status,
            google_oauth::google_oauth_disconnect,
            providers::set_provider_tools_opt_in,
            profile::get_profile,
            profile::save_profile,
            profile::save_profile_avatar,
            profile::remove_profile_avatar,
            history::list_conversations,
            history::load_conversation,
            history::load_agent_action_cards,
            history::append_conversation_message,
            history::get_conversation_agent_mode,
            history::set_conversation_agent_mode,
            history::delete_conversation,
            history::rename_conversation,
            history::delete_all_conversations,
            history::get_provider_usage,
            history::get_selected_pet,
            history::get_pet_mode,
            history::set_pet_mode,
            history::set_selected_pet,
            calendar::calendar_list_events,
            calendar::calendar_save_event,
            calendar::calendar_delete_event,
            calendar::calendar_import_ics,
            calendar::calendar_export_ics,
            calendar::calendar_get_default,
            calendar::calendar_set_default,
            security::get_permission_settings,
            security::get_trusted_external_domains,
            security::trust_external_domain,
            security::forget_external_domain,
            security::set_permission_mode,
            security::set_tool_override,
            security::skills::get_skills,
            security::skills::get_skill_family_summaries,
            security::skills::get_home_reminder_count,
            reminders::get_upcoming_reminders,
            security::skills::get_public_catalog,
            security::skills::set_skill_enabled,
            security::skills::set_skill_group_enabled,
            security::skills::get_allowed_folders,
            security::skills::add_allowed_folder,
            security::skills::remove_allowed_folder,
            security::skills::get_allowed_apps,
            discord::discord_list,
            discord::discord_save,
            discord::discord_test_connection,
            discord::discord_remove,
            security::skills::register_allowed_app,
            security::skills::remove_allowed_app,
            security::set_kill_shortcut,
            security::get_island_quick_shortcut,
            security::set_island_quick_shortcut,
            security::take_pet_quick_focus_request,
            security::set_approval_ttl,
            security::halt_all,
            security::resume_all,
            security::list_audit,
            security::verify_audit,
            security::get_pending_approval_count,
            security::show_pending_approval_window,
            security::get_pending_approval,
            system_actions::open_agent_artifact,
            system_actions::reveal_agent_artifact,
            security::get_full_approval_preview,
            security::approval_respond,
            #[cfg(debug_assertions)]
            security::submit_simulated_action,
            security::undo_agent_action,
            agent_home::agent_home_status,
            agent_home::agent_home_init,
            agent_home::agent_home_open_folder,
            agent_home::agent_home_change_path,
            agent_home::agent_home_read_core,
            agent_home::agent_home_save_core,
            agent_home::agent_memory_list,
            agent_home::agent_memory_read,
            agent_home::agent_memory_save,
            agent_home::agent_memory_delete,
            agent_home::agent_memory_delete_all,
        ])
        .run(tauri::generate_context!())
}

#[cfg(test)]
mod microphone_permission_tests {
    use super::{microphone_permission_allowed, microphone_permission_response};
    use tauri::webview::PermissionResponse;

    #[test]
    fn microphone_requires_opt_in_main_window_and_audio_kind() {
        assert!(microphone_permission_allowed("main", true, true));
        assert!(!microphone_permission_allowed("main", false, true));
        assert!(!microphone_permission_allowed("approval", true, true));
        assert!(!microphone_permission_allowed("main", true, false));
    }

    #[test]
    fn opted_in_main_microphone_continues_the_webview_permission_flow() {
        assert_eq!(
            microphone_permission_response("main", true, true),
            PermissionResponse::Default
        );
        assert_eq!(
            microphone_permission_response("main", false, true),
            PermissionResponse::Deny
        );
        assert_eq!(
            microphone_permission_response("pet", true, true),
            PermissionResponse::Deny
        );
        assert_eq!(
            microphone_permission_response("main", true, false),
            PermissionResponse::Deny
        );
    }
}
