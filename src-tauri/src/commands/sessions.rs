//! Команды сессий и проектов OpenCode.

use super::{current_state, emit_state, save_state};
use crate::state::{AppState, AppStatus, OpenCodeInstanceRef, OpenCodeTarget, SharedState};
use tauri::{AppHandle, Manager};

#[tauri::command]
pub async fn list_opencode_sessions() -> Vec<crate::state::OpenCodeInstance> {
    // Обнаружение экземпляров опрашивает порты и запускает подпроцессы —
    // выполняем в фоновом потоке, чтобы не блокировать UI.
    tauri::async_runtime::spawn_blocking(crate::modules::opencode::discover_instances)
        .await
        .unwrap_or_default()
}

#[tauri::command]
pub fn select_opencode_session(
    app: AppHandle,
    instance_id: String,
    port: u16,
    session_id: String,
    title: String,
    model: String,
) -> AppState {
    // instance_id — это папка проекта (см. discover_instances); имя проекта —
    // её basename.
    let project = std::path::Path::new(&instance_id)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let state = app.state::<SharedState>();
    let mut s = state.0.lock().unwrap();
    s.selected_session = Some(OpenCodeTarget {
        instance_id: instance_id.clone(),
        port,
        session_id: session_id.clone(),
        title: title.clone(),
    });
    s.opencode_model = if model.is_empty() {
        None
    } else {
        Some(model)
    };
    crate::modules::opencode::remember_session_port(&app, &session_id, port);
    crate::modules::opencode::remember_session_title(&app, &session_id, &title);
    crate::modules::opencode::remember_session_project(&app, &session_id, &project);
    crate::modules::opencode::remember_session_directory(&app, &session_id, &instance_id);
    s.active_instance = Some(OpenCodeInstanceRef {
        id: instance_id,
        port,
        name: String::new(),
    });
    // Показываем последний ответ этой сессии и сбрасываем статус.
    s.response = crate::modules::opencode::latest_assistant_response(&app, &session_id);
    s.status = AppStatus::Idle;
    s.status_message = "Готов к работе".into();
    let snapshot = s.clone();
    drop(s);
    emit_state(&app, &snapshot);
    save_state(&app, &snapshot);
    current_state(&app)
}

#[tauri::command]
pub fn select_opencode_instance(app: AppHandle, id: String, port: u16, name: String) -> AppState {
    let state = app.state::<SharedState>();
    let mut s = state.0.lock().unwrap();
    s.active_instance = Some(OpenCodeInstanceRef { id, port, name });
    // При переключении на другой инстанс сбрасываем сессию, ответ и статус.
    if s.selected_session.as_ref().map(|t| t.port) != Some(port) {
        s.selected_session = None;
        s.response.clear();
        s.status = AppStatus::Idle;
        s.status_message = "Готов к работе".into();
    }
    let snapshot = s.clone();
    drop(s);
    emit_state(&app, &snapshot);
    save_state(&app, &snapshot);
    current_state(&app)
}

#[tauri::command]
pub async fn list_projects(app: AppHandle) -> Vec<crate::modules::opencode::Project> {
    let known = super::known_worktrees(&app);
    // Чтение БД opencode и проверка портов тяжёлые — не блокируем UI.
    tauri::async_runtime::spawn_blocking(move || {
        crate::modules::opencode::list_projects_with_extra(&known)
    })
    .await
    .unwrap_or_default()
}

/// Создаёт сессию и делает её выбранной. Синхронная часть (HTTP-вызов) —
/// вызывать из `spawn_blocking`. Общая для десктопа и мобильного моста.
pub fn create_session_inner(
    app: &AppHandle,
    port: u16,
    worktree: &str,
    title: &str,
) -> Result<AppState, String> {
    let session = crate::modules::opencode::create_session(port, title)?;

    let instance_id = worktree.to_string();
    let project = std::path::Path::new(worktree)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let state = app.state::<SharedState>();
    let mut s = state.0.lock().unwrap();
    s.selected_session = Some(OpenCodeTarget {
        instance_id: instance_id.clone(),
        port,
        session_id: session.id.clone(),
        title: session.title.clone(),
    });
    s.opencode_model = if session.model.is_empty() {
        None
    } else {
        Some(session.model.clone())
    };
    s.active_instance = Some(OpenCodeInstanceRef {
        id: instance_id,
        port,
        name: project.clone(),
    });
    crate::modules::opencode::remember_session_port(app, &session.id, port);
    crate::modules::opencode::remember_session_title(app, &session.id, &session.title);
    crate::modules::opencode::remember_session_project(app, &session.id, &project);
    crate::modules::opencode::remember_session_directory(app, &session.id, &worktree);
    s.response.clear();
    s.status = AppStatus::Idle;
    s.status_message = "Готов к работе".into();
    let snapshot = s.clone();
    drop(s);
    emit_state(app, &snapshot);
    save_state(app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub async fn create_session(
    app: AppHandle,
    port: u16,
    worktree: String,
    title: String,
) -> Result<AppState, String> {
    tauri::async_runtime::spawn_blocking(move || {
        create_session_inner(&app, port, &worktree, &title)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn start_project(
    app: AppHandle,
    worktree: String,
) -> Result<Vec<crate::modules::opencode::Project>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::modules::opencode::start_project(&worktree)?;
        super::register_worktree(&app, &worktree);
        let known = super::known_worktrees(&app);
        Ok(crate::modules::opencode::list_projects_with_extra(&known))
    })
    .await
    .unwrap_or_else(|e| Err(e.to_string()))
}

/// Удаляет сессию OpenCode и чистит связанное состояние. Общая для десктопа
/// и мобильного моста (HTTP-вызов — вызывать из `spawn_blocking`).
pub fn delete_session_inner(app: &AppHandle, session_id: &str) -> Result<AppState, String> {
    let port = app
        .state::<crate::state::ConversationStore>()
        .ports
        .lock()
        .unwrap()
        .get(session_id)
        .copied()
        .ok_or("порт сессии не найден")?;

    crate::modules::opencode::delete_session(port, session_id)?;

    // Убираем сессию из памяти.
    {
        let store = app.state::<crate::state::ConversationStore>();
        store.conversations.lock().unwrap().remove(session_id);
        store.ports.lock().unwrap().remove(session_id);
        store.titles.lock().unwrap().remove(session_id);
        store.projects.lock().unwrap().remove(session_id);
        store.directories.lock().unwrap().remove(session_id);
        store.open_sessions.lock().unwrap().remove(session_id);
    }

    // Закрываем окно чата, если оно было открыто.
    if let Some(window) = app.get_webview_window(&format!("response-{session_id}")) {
        let _ = window.close();
    }

    // Сбрасываем выбор, если удалили выбранную сессию.
    let state = app.state::<SharedState>();
    let mut s = state.0.lock().unwrap();
    if s.selected_session.as_ref().map(|t| t.session_id.as_str()) == Some(session_id) {
        s.selected_session = None;
        s.response.clear();
        s.status = AppStatus::Idle;
        s.status_message = "Готов к работе".into();
    }
    let snapshot = s.clone();
    drop(s);
    emit_state(app, &snapshot);
    save_state(app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub async fn delete_session(app: AppHandle, session_id: String) -> Result<AppState, String> {
    tauri::async_runtime::spawn_blocking(move || delete_session_inner(&app, &session_id))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn stop_project(worktree: String) -> Result<Vec<crate::modules::opencode::Project>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::modules::opencode::stop_project(&worktree)?;
        Ok(crate::modules::opencode::list_projects())
    })
    .await
    .unwrap_or_else(|e| Err(e.to_string()))
}
