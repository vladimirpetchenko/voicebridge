use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let raw = if cfg!(target_os = "macos") {
        "Cmd+Shift+V"
    } else {
        "Ctrl+Shift+V"
    };
    let shortcut: Shortcut = raw.parse().expect("invalid shortcut");

    app.global_shortcut()
        .on_shortcut(shortcut, move |app, _sc, event| {
            if event.state() == ShortcutState::Pressed {
                crate::commands::handle_toggle_recording(app, None);
            }
        })
        .map_err(|e| std::io::Error::other(e.to_string()))?;

    Ok(())
}
