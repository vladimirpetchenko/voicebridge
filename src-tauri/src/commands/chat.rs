//! Команды окна чата и диалогов.

use super::session_id_from_window;
use crate::state::ConversationStore;
use tauri::{AppHandle, Manager};

/// Отправка набранного вручную текста в сессию окна чата (не зависит от режима).
#[tauri::command]
pub fn send_text(app: AppHandle, window: tauri::WebviewWindow, text: String) {
    let session = session_id_from_window(&window);
    crate::modules::opencode::send_prompt(app, text, session);
}

#[tauri::command]
pub async fn get_session_info(
    window: tauri::WebviewWindow,
    app: AppHandle,
) -> crate::state::SessionInfo {
    let session_id = window
        .label()
        .strip_prefix("response-")
        .unwrap_or_default()
        .to_string();
    let store = app.state::<ConversationStore>();
    let title = store
        .titles
        .lock()
        .unwrap()
        .get(&session_id)
        .cloned()
        .unwrap_or_default();
    let project = store
        .projects
        .lock()
        .unwrap()
        .get(&session_id)
        .cloned()
        .unwrap_or_default();
    crate::state::SessionInfo { title, project }
}

#[tauri::command]
pub async fn get_session_usage(
    window: tauri::WebviewWindow,
    app: AppHandle,
) -> Option<crate::state::SessionUsage> {
    let session_id = window
        .label()
        .strip_prefix("response-")
        .unwrap_or_default()
        .to_string();
    if session_id.is_empty() {
        return None;
    }
    tauri::async_runtime::spawn_blocking(move || {
        crate::modules::opencode::fetch_session_usage(&app, &session_id)
    })
    .await
    .unwrap_or(None)
}

#[tauri::command]
pub fn abort_session(window: tauri::WebviewWindow, app: AppHandle) -> Result<(), String> {
    let session_id = window
        .label()
        .strip_prefix("response-")
        .unwrap_or_default();
    if session_id.is_empty() {
        return Err("сессия не определена".into());
    }
    let port = app
        .state::<ConversationStore>()
        .ports
        .lock()
        .unwrap()
        .get(session_id)
        .copied()
        .ok_or("порт сессии не найден")?;
    crate::modules::opencode::abort_session(port, session_id)
}

#[tauri::command]
pub fn get_opencode_binary() -> String {
    crate::modules::opencode::opencode_binary()
}

#[tauri::command]
pub fn reply_permission(port: u16, request_id: String, reply: String) -> Result<(), String> {
    crate::modules::opencode::reply_permission(port, &request_id, &reply)
}

#[tauri::command]
pub fn reply_question(
    port: u16,
    request_id: String,
    answers: Vec<Vec<String>>,
) -> Result<(), String> {
    crate::modules::opencode::reply_question(port, &request_id, answers)
}

#[tauri::command]
pub fn reject_question(port: u16, request_id: String) -> Result<(), String> {
    crate::modules::opencode::reject_question(port, &request_id)
}

#[tauri::command]
pub async fn open_response_window(app: AppHandle, session_id: String, title: String, port: u16) {
    crate::modules::opencode::remember_session_port(&app, &session_id, port);
    crate::modules::opencode::remember_session_title(&app, &session_id, &title);

    let label = format!("response-{session_id}");

    if let Some(window) = app.get_webview_window(&label) {
        let _ = window.show();
        let _ = window.set_focus();
    } else {
        use tauri::{WebviewUrl, WebviewWindowBuilder};
        // Создание окна должно быть асинхронным: `build()` на Windows шлёт
        // создание webview в главный поток и ждёт ответа. В синхронной команде
        // это приводит к deadlock (главный поток ждёт сам себя).
        let builder = WebviewWindowBuilder::new(
            &app,
            &label,
            WebviewUrl::App(std::path::PathBuf::from("index.html")),
        )
        .title(format!("OpenCode · {title}"))
        .inner_size(960.0, 820.0)
        .min_inner_size(480.0, 400.0);
        match builder.build() {
            Ok(_window) => {
                log::info!("opened response window {label}");
            }
            Err(e) => {
                log::error!("failed to open response window {label}: {e}");
            }
        }
    }

    crate::modules::opencode::mark_session_open(&app, &session_id);
}

#[tauri::command]
pub fn list_open_session_ids(app: AppHandle) -> Vec<String> {
    crate::modules::opencode::open_session_ids(&app)
}

#[tauri::command]
pub async fn get_conversation(
    window: tauri::WebviewWindow,
    app: AppHandle,
) -> Vec<crate::state::ConversationMessage> {
    let session_id = window
        .label()
        .strip_prefix("response-")
        .unwrap_or_default()
        .to_string();

    // Подтягивание истории делает HTTP-запрос — не блокируем главный поток.
    tauri::async_runtime::spawn_blocking(move || {
        let port = {
            let store = app.state::<ConversationStore>();
            let ports = store.ports.lock().unwrap();
            ports.get(&session_id).copied()
        };

        if let Some(port) = port {
            if let Ok(history) = crate::modules::opencode::fetch_session_history(port, &session_id)
            {
                if !history.is_empty() {
                    return history;
                }
            }
        }

        crate::modules::opencode::conversation_for(&app, &session_id)
    })
    .await
    .unwrap_or_default()
}

#[tauri::command]
pub fn close_response_window(window: tauri::WebviewWindow) {
    let _ = window.close();
}
