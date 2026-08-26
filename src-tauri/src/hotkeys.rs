use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// Регистрирует глобальную горячую клавишу записи.
fn register(app: &AppHandle, raw: &str) -> Result<(), String> {
    let shortcut: Shortcut = raw
        .parse()
        .map_err(|e| format!("неверная комбинация «{raw}»: {e}"))?;
    app.global_shortcut()
        .on_shortcut(shortcut, |app, _sc, event| {
            if event.state() == ShortcutState::Pressed {
                crate::commands::handle_toggle_recording(app, None);
            }
        })
        .map_err(|e| format!("не удалось зарегистрировать «{raw}»: {e}"))
}

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let hotkey = {
        let state = app.state::<crate::state::SharedState>();
        let h = state.0.lock().unwrap().hotkey.clone();
        h
    };
    register(app, &hotkey).map_err(std::io::Error::other)?;
    Ok(())
}

/// Перепривязывает глобальную горячую клавишу записи на новую комбинацию.
pub fn apply_hotkey(app: &AppHandle, raw: &str) -> Result<(), String> {
    let old = {
        let state = app.state::<crate::state::SharedState>();
        let h = state.0.lock().unwrap().hotkey.clone();
        h
    };
    if old == raw {
        return Ok(());
    }
    // Сначала регистрируем новую (fail-safe), затем снимаем старую.
    register(app, raw)?;
    if let Ok(old_sc) = old.parse::<Shortcut>() {
        let _ = app.global_shortcut().unregister(old_sc);
    }
    Ok(())
}
