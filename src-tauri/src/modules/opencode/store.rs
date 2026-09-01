//! Память сессий (`ConversationStore`) и трансляция Git-изменений.

use crate::state::{AppStatus, ConversationStore, SharedState};
use tauri::{AppHandle, Emitter, Manager};

/// Запоминает порт экземпляра, на котором живёт сессия.
pub fn remember_session_port(app: &AppHandle, session_id: &str, port: u16) {
    let store = app.state::<ConversationStore>();
    let mut ports = store.ports.lock().unwrap();
    ports.insert(session_id.to_string(), port);
}

/// Запоминает заголовок сессии (для шапки окна чата).
pub fn remember_session_title(app: &AppHandle, session_id: &str, title: &str) {
    let store = app.state::<ConversationStore>();
    let mut titles = store.titles.lock().unwrap();
    titles.insert(session_id.to_string(), title.to_string());
}

/// Запоминает имя проекта (папки) сессии (для шапки окна чата).
pub fn remember_session_project(app: &AppHandle, session_id: &str, project: &str) {
    let store = app.state::<ConversationStore>();
    let mut projects = store.projects.lock().unwrap();
    projects.insert(session_id.to_string(), project.to_string());
}

/// Запоминает полный путь рабочей папки сессии (для Git-панели).
pub fn remember_session_directory(app: &AppHandle, session_id: &str, directory: &str) {
    if directory.is_empty() {
        return;
    }
    let store = app.state::<ConversationStore>();
    let mut dirs = store.directories.lock().unwrap();
    dirs.insert(session_id.to_string(), directory.to_string());
}

/// Путь рабочей папки сессии (если известен). Для мобилки — по выбранной сессии.
pub fn session_directory(app: &AppHandle, session_id: Option<&str>) -> Option<String> {
    if let Some(sid) = session_id {
        let store = app.state::<ConversationStore>();
        let dir = store.directories.lock().unwrap().get(sid).cloned();
        if let Some(dir) = dir {
            if !dir.is_empty() {
                return Some(dir);
            }
        }
    }
    let state = app.state::<SharedState>();
    let s = state.0.lock().unwrap();
    s.selected_session.as_ref().map(|t| t.instance_id.clone())
}

/// Считает Git-изменения проекта сессии и транслирует их во фронтенд и на мобилку.
/// Вызывается после завершения ответа и при открытии окна чата.
pub fn broadcast_git_changes(app: &AppHandle, session_id: String) {
    let app = app.clone();
    std::thread::spawn(move || {
        let Some(dir) = session_directory(&app, Some(&session_id)) else {
            return;
        };
        let changes = crate::modules::git::changes(&dir);
        crate::modules::mobile::emit_and_broadcast(
            &app,
            "git-changes",
            serde_json::json!({ "sessionId": session_id, "changes": changes }),
        );
    });
}

/// Список id сессий, для которых сейчас открыто окно чата.
pub fn open_session_ids(app: &AppHandle) -> Vec<String> {
    let store = app.state::<ConversationStore>();
    let open = store.open_sessions.lock().unwrap();
    let mut ids: Vec<String> = open.iter().cloned().collect();
    ids.sort();
    ids
}

/// Помечает сессию как открытую (есть окно чата) и оповещает фронтенд.
pub fn mark_session_open(app: &AppHandle, session_id: &str) {
    let store = app.state::<ConversationStore>();
    let mut open = store.open_sessions.lock().unwrap();
    open.insert(session_id.to_string());
    drop(open);
    let _ = app.emit("sessions-open-changed", open_session_ids(app));
    broadcast_git_changes(app, session_id.to_string());
}

/// Помечает сессию как закрытую (окно чата уничтожено) и оповещает фронтенд.
pub fn mark_session_closed(app: &AppHandle, session_id: &str) {
    let store = app.state::<ConversationStore>();
    let mut open = store.open_sessions.lock().unwrap();
    open.remove(session_id);
    drop(open);

    // Если закрытая сессия была выбранной — сбрасываем выбор, чтобы лаунчер
    // не показывал закрытую сессию как активную.
    {
        let state = app.state::<SharedState>();
        let mut s = state.0.lock().unwrap();
        if s.selected_session
            .as_ref()
            .map(|t| t.session_id.as_str())
            == Some(session_id)
        {
            s.selected_session = None;
            s.response.clear();
            s.status = AppStatus::Idle;
            s.status_message = "Готов к работе".into();
            let snapshot = s.clone();
            drop(s);
            crate::commands::emit_state(app, &snapshot);
            crate::commands::save_state(app, &snapshot);
        }
    }

    let _ = app.emit("sessions-open-changed", open_session_ids(app));
}
