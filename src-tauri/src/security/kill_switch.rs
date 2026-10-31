use super::*;

const DEFAULT_SHORTCUT: &str = "Ctrl+Shift+F12";

fn set_halted(
    app: &tauri::AppHandle,
    state: &SecurityState,
    halted: bool,
    reason: &'static str,
) -> Result<(), String> {
    if halted {
        state.halted.store(true, Ordering::SeqCst);
        let persistence = set_db_setting(app, "halted", "true");
        crate::providers::cancel_all(app);
        let handle = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Some(state) = handle.try_state::<SecurityState>() {
                state.approval_broker.stop_all().await;
                let mut runs = state.agent_runs.lock().await;
                for token in runs.values() {
                    token.cancel();
                }
                runs.clear();
            }
        });
        let mut pending = state
            .pending
            .lock()
            .map_err(|_| "Fila de aprovação indisponível.".to_string())?;
        pending.clear();
        drop(pending);
        let _ = app.emit("approval-updated", ());
        if let Some(window) = app.get_webview_window("approval") {
            let _ = window.close();
        }
        let _ = app.emit("companion-state",serde_json::json!({"type":"activity","state":"paused","message":"Parado pelo kill switch"}));
        let _ = crate::pet_alerts::enqueue_agent_activity(app, "paused");
        let _ = app.emit("security-halted", ());
        let request_id = id();
        let origin = Origin::User;
        let audit = record(
            app,
            state,
            &request_id,
            "security.kill_switch",
            Risk::Leitura,
            &origin,
            "halt",
            reason,
            "not_required",
            0,
            "{}".into(),
            "halted",
            None,
        );
        return persistence.and(audit);
    }
    if !state.is_halted() {
        return Ok(());
    }
    let request_id = id();
    let origin = Origin::User;
    record(
        app,
        state,
        &request_id,
        "security.kill_switch",
        Risk::Leitura,
        &origin,
        "resume",
        reason,
        "not_required",
        0,
        "{}".into(),
        "resumed",
        None,
    )?;
    set_db_setting(app, "halted", "false")?;
    state.halted.store(false, Ordering::SeqCst);
    let _ = app.emit("companion-state", serde_json::json!({"type":"reveal"}));
    let _ = app.emit("security-resumed", ());
    Ok(())
}

pub(super) fn halt_all_inner(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    state: State<'_, SecurityState>,
) -> Result<(), String> {
    crate::require_main(&caller)?;
    set_halted(&app, &state, true, "user_halt")
}

pub(super) fn resume_all_inner(
    app: tauri::AppHandle,
    caller: tauri::WebviewWindow,
    state: State<'_, SecurityState>,
) -> Result<(), String> {
    crate::require_main(&caller)?;
    set_halted(&app, &state, false, "user_resume")
}

pub fn on_global_shortcut(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<SecurityState>() {
        if let Err(error) = set_halted(app, &state, true, "global_shortcut") {
            let _ = app.emit("security-error", error);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShortcutAction {
    KillSwitch,
    FocusQuickCommand,
    Ignore,
}

trait QuickShortcutRegistry {
    fn register(&mut self, shortcut: &str) -> Result<u32, String>;
    fn unregister(&mut self, shortcut: &str) -> Result<(), String>;
}

struct TauriShortcutRegistry<'a>(&'a tauri::AppHandle);

impl QuickShortcutRegistry for TauriShortcutRegistry<'_> {
    fn register(&mut self, value: &str) -> Result<u32, String> {
        let shortcut = value
            .parse::<tauri_plugin_global_shortcut::Shortcut>()
            .map_err(|error| error.to_string())?;
        let id = shortcut.id();
        self.0
            .global_shortcut()
            .register(shortcut)
            .map_err(|error| error.to_string())?;
        Ok(id)
    }

    fn unregister(&mut self, value: &str) -> Result<(), String> {
        self.0
            .global_shortcut()
            .unregister(value)
            .map_err(|error| error.to_string())
    }
}

fn transition_quick_shortcut<R: QuickShortcutRegistry>(
    registry: &mut R,
    previous: &str,
    was_enabled: bool,
    next: &str,
    enabled: bool,
) -> Result<Option<u32>, String> {
    if was_enabled && (previous != next || !enabled) {
        registry.unregister(previous)?;
    }
    if enabled && (!was_enabled || previous != next) {
        return registry.register(next).map(Some);
    }
    Ok(None)
}

fn register_optional_at_startup<R: QuickShortcutRegistry>(
    registry: &mut R,
    shortcut: &str,
) -> (Option<u32>, Option<String>) {
    match registry.register(shortcut) {
        Ok(id) => (Some(id), None),
        Err(error) => (None, Some(error)),
    }
}

fn route_shortcut(id: u32, kill_id: Option<u32>, quick_id: Option<u32>) -> ShortcutAction {
    if Some(id) == kill_id {
        ShortcutAction::KillSwitch
    } else if Some(id) == quick_id {
        ShortcutAction::FocusQuickCommand
    } else {
        ShortcutAction::Ignore
    }
}

pub fn handle_global_shortcut(app: &tauri::AppHandle, id: u32) {
    let Some(state) = app.try_state::<SecurityState>() else {
        return;
    };
    let kill_id = state.active_kill_shortcut_id.lock().ok().and_then(|id| *id);
    let quick_id = state
        .active_quick_shortcut_id
        .lock()
        .ok()
        .and_then(|id| *id);
    match route_shortcut(id, kill_id, quick_id) {
        ShortcutAction::KillSwitch => on_global_shortcut(app),
        ShortcutAction::FocusQuickCommand => {
            state.quick_focus_requested.store(true, Ordering::SeqCst);
            let handle = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(error) = crate::show_pet_quick_command(&handle).await {
                    if let Some(state) = handle.try_state::<SecurityState>() {
                        state.quick_focus_requested.store(false, Ordering::SeqCst);
                    }
                    let _ = handle.emit("pet-quick-focus-error", error);
                }
            });
        }
        ShortcutAction::Ignore => {}
    }
}

pub fn init_shortcut(app: &tauri::AppHandle) {
    let shortcut = db_setting(app, "shortcut").unwrap_or_else(|_| DEFAULT_SHORTCUT.into());
    match shortcut.parse::<tauri_plugin_global_shortcut::Shortcut>() {
        Ok(parsed) => {
            let parsed_id = parsed.id();
            if let Err(error) = app.global_shortcut().register(parsed) {
                if let Some(state) = app.try_state::<SecurityState>() {
                    if let Ok(mut status) = state.shortcut_error.lock() {
                        *status = Some(format!("Atalho não registrado: {error}"));
                    }
                }
            } else if let Some(state) = app.try_state::<SecurityState>() {
                if let Ok(mut id) = state.active_kill_shortcut_id.lock() {
                    *id = Some(parsed_id);
                }
            }
        }
        Err(error) => {
            if let Some(state) = app.try_state::<SecurityState>() {
                if let Ok(mut status) = state.shortcut_error.lock() {
                    *status = Some(format!("Atalho não registrado: {error}"));
                }
            }
        }
    }
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        init_quick_shortcut(&handle).await;
    });
}

async fn init_quick_shortcut(app: &tauri::AppHandle) {
    let enabled =
        db_setting(app, "island_quick_shortcut_enabled").is_ok_and(|value| value == "true");
    if !enabled {
        return;
    }
    let value =
        db_setting(app, "island_quick_shortcut").unwrap_or_else(|_| "Ctrl+Alt+Space".into());
    if db_setting(app, "shortcut").is_ok_and(|kill| kill == value) {
        let _ = set_db_setting(app, "island_quick_shortcut_enabled", "false");
        if let Some(state) = app.try_state::<SecurityState>() {
            if let Ok(mut status) = state.quick_shortcut_error.lock() {
                *status = Some(
                    "Esta combinação já está reservada para Parar tudo; escolha outra.".into(),
                );
            }
        }
        let _ = app.emit("island-quick-shortcut-updated", ());
        return;
    }
    let mut adapter = TauriShortcutRegistry(app);
    let (shortcut_id, registration_error) = register_optional_at_startup(&mut adapter, &value);
    if let Some(shortcut_id) = shortcut_id {
        if let Some(state) = app.try_state::<SecurityState>() {
            if let Ok(mut id) = state.active_quick_shortcut_id.lock() {
                *id = Some(shortcut_id);
            }
        }
    } else if let Some(error) = registration_error {
        let _ = set_db_setting(app, "island_quick_shortcut_enabled", "false");
        if let Some(state) = app.try_state::<SecurityState>() {
            let message = if error.to_ascii_lowercase().contains("already registered") {
                quick_shortcut_error(&error)
            } else {
                format!("A combinação salva para o comando rápido não pôde ser registrada: {error}")
            };
            if let Ok(mut status) = state.quick_shortcut_error.lock() {
                *status = Some(message);
            }
        }
    }
    let _ = app.emit("island-quick-shortcut-updated", ());
}

fn quick_shortcut_error(error: &str) -> String {
    let normalized = error.to_ascii_lowercase();
    if normalized.contains("already registered") || normalized.contains("hotkey already registered")
    {
        "Esta combinação já é usada por outro programa; escolha outra.".into()
    } else {
        format!("Não foi possível registrar o atalho do comando rápido: {error}")
    }
}

pub(crate) async fn update_quick_shortcut(
    app: &tauri::AppHandle,
    shortcut_value: &str,
    enabled: bool,
) -> Result<(), String> {
    let previous =
        db_setting(app, "island_quick_shortcut").unwrap_or_else(|_| "Ctrl+Alt+Space".into());
    let was_enabled =
        db_setting(app, "island_quick_shortcut_enabled").is_ok_and(|value| value == "true");
    let kill = db_setting(app, "shortcut").unwrap_or_else(|_| DEFAULT_SHORTCUT.into());
    if !enabled {
        if was_enabled {
            let mut adapter = TauriShortcutRegistry(app);
            transition_quick_shortcut(&mut adapter, &previous, true, &previous, false).map_err(
                |error| format!("Não foi possível desativar o atalho anterior: {error}"),
            )?;
        }
        let draft_is_valid = shortcut_value.trim() == shortcut_value
            && shortcut_value.len() <= 80
            && shortcut_value.contains('+')
            && shortcut_value
                .parse::<tauri_plugin_global_shortcut::Shortcut>()
                .is_ok()
            && shortcut_value != kill;
        let saved = if draft_is_valid {
            shortcut_value
        } else {
            &previous
        };
        set_db_setting(app, "island_quick_shortcut", saved)?;
        set_db_setting(app, "island_quick_shortcut_enabled", "false")?;
        if let Some(state) = app.try_state::<SecurityState>() {
            if let Ok(mut id) = state.active_quick_shortcut_id.lock() {
                *id = None;
            }
            if let Ok(mut status) = state.quick_shortcut_error.lock() {
                *status = None;
            }
        }
        return Ok(());
    }
    if shortcut_value.trim() != shortcut_value
        || shortcut_value.len() > 80
        || !shortcut_value.contains('+')
    {
        return Err("Formato de atalho inválido. Use uma combinação como Ctrl+Alt+Espaço.".into());
    }
    if enabled && shortcut_value == kill {
        if was_enabled {
            let mut adapter = TauriShortcutRegistry(app);
            transition_quick_shortcut(&mut adapter, &previous, true, &previous, false)?;
            let _ = set_db_setting(app, "island_quick_shortcut_enabled", "false");
            if let Some(state) = app.try_state::<SecurityState>() {
                if let Ok(mut id) = state.active_quick_shortcut_id.lock() {
                    *id = None;
                }
            }
        }
        return Err("Esta combinação já está reservada para Parar tudo; escolha outra.".into());
    }
    let _parsed = shortcut_value
        .parse::<tauri_plugin_global_shortcut::Shortcut>()
        .map_err(|_| {
            "Formato de atalho inválido. Use uma combinação como Ctrl+Alt+Espaço.".to_string()
        })?;
    let unchanged = was_enabled && previous == shortcut_value;
    let registration = if unchanged {
        None
    } else {
        let mut adapter = TauriShortcutRegistry(app);
        match transition_quick_shortcut(&mut adapter, &previous, was_enabled, shortcut_value, true)
        {
            Ok(id) => id,
            Err(error) => {
                let message = quick_shortcut_error(&error);
                let _ = set_db_setting(app, "island_quick_shortcut", shortcut_value);
                let _ = set_db_setting(app, "island_quick_shortcut_enabled", "false");
                if let Some(state) = app.try_state::<SecurityState>() {
                    if let Ok(mut id) = state.active_quick_shortcut_id.lock() {
                        *id = None;
                    }
                    if let Ok(mut status) = state.quick_shortcut_error.lock() {
                        *status = Some(message.clone());
                    }
                }
                return Err(message);
            }
        }
    };
    if let Some(state) = app.try_state::<SecurityState>() {
        if let Ok(mut id) = state.active_quick_shortcut_id.lock() {
            if let Some(registration_id) = registration {
                *id = Some(registration_id);
            }
        }
    }
    set_db_setting(app, "island_quick_shortcut", shortcut_value)?;
    set_db_setting(app, "island_quick_shortcut_enabled", "true")?;
    if let Some(state) = app.try_state::<SecurityState>() {
        if let Ok(mut status) = state.quick_shortcut_error.lock() {
            *status = None;
        }
    }
    Ok(())
}

pub(crate) fn consume_quick_focus_request(state: &SecurityState) -> bool {
    state.quick_focus_requested.swap(false, Ordering::SeqCst)
}

pub fn load_halted(app: &tauri::AppHandle) -> bool {
    db_setting(app, "halted")
        .map(|value| value == "true")
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct SimRegistry {
        events: Vec<String>,
        registered: Vec<String>,
        conflict: Option<String>,
        next_id: u32,
    }

    impl QuickShortcutRegistry for SimRegistry {
        fn register(&mut self, shortcut: &str) -> Result<u32, String> {
            self.events.push(format!("register:{shortcut}"));
            if self.conflict.as_deref() == Some(shortcut) {
                return Err("HotKey already registered".into());
            }
            self.registered.push(shortcut.into());
            self.next_id += 1;
            Ok(self.next_id)
        }

        fn unregister(&mut self, shortcut: &str) -> Result<(), String> {
            self.events.push(format!("unregister:{shortcut}"));
            self.registered.retain(|registered| registered != shortcut);
            Ok(())
        }
    }

    #[test]
    fn enabling_registers_shortcut_for_focus_only() {
        let mut registry = SimRegistry::default();
        let id =
            transition_quick_shortcut(&mut registry, "Ctrl+Alt+Space", false, "Ctrl+Alt+J", true)
                .unwrap();
        assert_eq!(id, Some(1));
        assert_eq!(
            route_shortcut(1, Some(2), Some(1)),
            ShortcutAction::FocusQuickCommand
        );
        assert_eq!(registry.registered, ["Ctrl+Alt+J"]);
    }

    #[test]
    fn conflict_returns_localized_error_and_leaves_shortcut_unregistered() {
        let mut registry = SimRegistry {
            conflict: Some("Ctrl+Alt+J".into()),
            ..Default::default()
        };
        let error =
            transition_quick_shortcut(&mut registry, "Ctrl+Alt+Space", false, "Ctrl+Alt+J", true)
                .unwrap_err();
        assert_eq!(
            quick_shortcut_error(&error),
            "Esta combinação já é usada por outro programa; escolha outra."
        );
        assert!(registry.registered.is_empty());
    }

    #[test]
    fn replacing_unregisters_before_registering_and_disabling_unregisters() {
        let mut registry = SimRegistry {
            registered: vec!["Ctrl+Alt+Space".into()],
            ..Default::default()
        };
        let id =
            transition_quick_shortcut(&mut registry, "Ctrl+Alt+Space", true, "Ctrl+Alt+J", true)
                .unwrap();
        assert_eq!(id, Some(1));
        assert_eq!(
            registry.events,
            ["unregister:Ctrl+Alt+Space", "register:Ctrl+Alt+J"]
        );
        registry.events.clear();
        transition_quick_shortcut(&mut registry, "Ctrl+Alt+J", true, "Ctrl+Alt+J", false).unwrap();
        assert_eq!(registry.events, ["unregister:Ctrl+Alt+J"]);
        assert!(registry.registered.is_empty());
    }

    #[test]
    fn startup_registration_failure_is_non_fatal_and_quick_action_never_sends() {
        let mut registry = SimRegistry {
            conflict: Some("Ctrl+Alt+Space".into()),
            ..Default::default()
        };
        let (id, error) = register_optional_at_startup(&mut registry, "Ctrl+Alt+Space");
        assert_eq!(id, None);
        assert_eq!(error.as_deref(), Some("HotKey already registered"));
        assert!(!registry.registered.contains(&"Ctrl+Alt+Space".into()));
        assert_eq!(
            route_shortcut(8, Some(1), Some(8)),
            ShortcutAction::FocusQuickCommand
        );
        assert_eq!(route_shortcut(9, Some(1), Some(8)), ShortcutAction::Ignore);
        assert_eq!(
            route_shortcut(1, Some(1), Some(8)),
            ShortcutAction::KillSwitch
        );
    }
}
