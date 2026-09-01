use tauri::{
    menu::{Menu, MenuItem, Submenu},
    tray::{TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager,
};

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let show_hide =
        MenuItem::with_id(app, "show_hide", "Показать/Скрыть", true, None::<&str>)?;

    let models = MenuItem::with_id(app, "settings_models", "Модели", true, None::<&str>)?;
    let microphone =
        MenuItem::with_id(app, "settings_microphone", "Микрофон", true, None::<&str>)?;
    let opencode = MenuItem::with_id(app, "settings_opencode", "OpenCode", true, None::<&str>)?;
    let hotkeys =
        MenuItem::with_id(app, "settings_hotkeys", "Горячие клавиши", true, None::<&str>)?;
    let mobile =
        MenuItem::with_id(app, "settings_mobile", "Мобильный доступ", true, None::<&str>)?;
    let about = MenuItem::with_id(app, "settings_about", "О программе", true, None::<&str>)?;

    let settings_submenu = Submenu::with_items(
        app,
        "Настройки",
        true,
        &[&models, &microphone, &opencode, &hotkeys, &mobile, &about],
    )?;

    let quit = MenuItem::with_id(app, "quit", "Выход", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&show_hide, &settings_submenu, &quit])?;

    let mut builder = TrayIconBuilder::with_id("main-tray")
        .tooltip("VoiceBridge — ожидание")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { .. } = event {
                show_and_focus(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    builder.build(app)?;
    Ok(())
}

/// Показывает и фокусирует главное окно, если оно скрыто или не в фокусе.
fn show_and_focus(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let visible = window.is_visible().unwrap_or(false);
        let focused = window.is_focused().unwrap_or(false);
        if !visible || !focused {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}

fn toggle_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}

/// Имя вкладки настроек по id пункта меню.
fn settings_tab(id: &str) -> Option<&'static str> {
    match id {
        "settings_models" => Some("Модели"),
        "settings_microphone" => Some("Микрофон"),
        "settings_opencode" => Some("OpenCode"),
        "settings_hotkeys" => Some("Горячие клавиши"),
        "settings_mobile" => Some("Мобильный доступ"),
        "settings_about" => Some("О программе"),
        _ => None,
    }
}

pub fn handle_menu_event(app: &AppHandle, event: tauri::menu::MenuEvent) {
    let id = event.id().as_ref();

    if let Some(tab) = settings_tab(id) {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.show();
            let _ = window.set_focus();
        }
        // Фронтенд открывает нужную вкладку настроек.
        let _ = app.emit("open-settings", tab);
        return;
    }

    match id {
        "show_hide" => toggle_window(app),
        "quit" => app.exit(0),
        _ => {}
    }
}
