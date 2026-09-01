//! Tauri-команды (мост фронтенд ↔ модули).
//!
//! Разбит на подмодули по доменам:
//! - [`recording`] — запись/распознавание речи;
//! - [`settings`] — настройки (модели, микрофон, язык, горячие клавиши);
//! - [`chat`] — окно чата и диалоги;
//! - [`sessions`] — сессии и проекты OpenCode;
//! - [`mobile`] — мобильный доступ и устройства;
//! - [`git`] — Git-изменения проекта;
//! - [`system`] — выход и обновления.

mod chat;
mod git;
mod mobile;
mod recording;
mod sessions;
mod settings;
mod system;

pub use chat::*;
pub use git::*;
pub use mobile::*;
pub use recording::*;
pub use sessions::*;
pub use settings::*;
pub use system::*;

use crate::state::{AppState, AppStatus, SharedState};
use tauri::{AppHandle, Emitter, Manager};

pub(crate) fn current_state(app: &AppHandle) -> AppState {
    app.state::<SharedState>().0.lock().unwrap().clone()
}

pub fn emit_state(app: &AppHandle, state: &AppState) {
    let _ = app.emit("state-changed", state.clone());
    crate::modules::mobile::broadcast(
        app,
        "state-changed",
        serde_json::to_value(state).unwrap_or(serde_json::Value::Null),
    );
    if let Some(tray) = app.tray_by_id("main-tray") {
        let text = match state.status {
            AppStatus::Idle => "VoiceBridge — ожидание",
            AppStatus::Recording => "VoiceBridge — запись…",
            AppStatus::Processing => "VoiceBridge — обработка…",
            AppStatus::Error => "VoiceBridge — ошибка",
        };
        let _ = tray.set_tooltip(Some(text));
    }
}

pub fn save_state(app: &AppHandle, state: &AppState) {
    let Ok(dir) = app.path().app_data_dir() else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("state.json");
    if let Ok(json) = serde_json::to_string_pretty(state) {
        let _ = std::fs::write(path, json);
    }
}

pub fn load_state(app: &AppHandle) -> Option<AppState> {
    let dir = app.path().app_data_dir().ok()?;
    let path = dir.join("state.json");
    let data = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

/// Папки проектов, запущенные через приложение (даже без сессий в БД).
pub(crate) fn known_worktrees(app: &AppHandle) -> Vec<String> {
    app.state::<SharedState>()
        .0
        .lock()
        .unwrap()
        .known_worktrees
        .clone()
}

/// Запоминает папку запущенного проекта (для показа в лаунчере).
pub(crate) fn register_worktree(app: &AppHandle, worktree: &str) {
    let normalized = crate::modules::opencode::normalize_worktree(worktree);
    let state = app.state::<SharedState>();
    let mut s = state.0.lock().unwrap();
    if !s
        .known_worktrees
        .iter()
        .any(|w| crate::modules::opencode::normalize_worktree(w) == normalized)
    {
        s.known_worktrees.push(normalized);
        let snapshot = s.clone();
        drop(s);
        save_state(app, &snapshot);
    }
}

/// Извлекает id сессии из метки окна чата (`response-{sessionId}`).
pub(crate) fn session_id_from_window(window: &tauri::WebviewWindow) -> Option<String> {
    window
        .label()
        .strip_prefix("response-")
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
}
