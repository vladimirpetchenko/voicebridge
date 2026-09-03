//! Модуль интеграции с OpenCode.
//!
//! Обнаруживает запущенные экземпляры OpenCode (HTTP-серверы на локальных
//! портах), читает список сессий и отправляет промпты с потоковым приёмом
//! ответа через SSE (`GET /event`).
//!
//! Разбит на подмодули по ответственности:
//! - [`discovery`] — обнаружение экземпляров и создание сессий;
//! - [`projects`] — управление проектами (запуск/остановка серверов);
//! - [`sessions`] — выбор цели и отправка промптов;
//! - [`streaming`] — SSE-стрим ответа и диалоги;
//! - [`store`] — память сессий (`ConversationStore`) и broadcast Git-изменений;
//! - [`usage`] — токены/стоимость и история сессии;
//! - [`actions`] — разрешения, вопросы, прерывание.

mod actions;
mod discovery;
mod projects;
mod sessions;
mod store;
mod streaming;
mod usage;

pub use actions::{abort_session, reject_question, reply_permission, reply_question};
pub use discovery::{create_session, delete_session, discover_instances, update_session_title};
pub use projects::{
    list_projects, list_projects_with_extra, opencode_binary, start_project, stop_project, Project,
};
pub(crate) use projects::normalize_worktree;
pub use sessions::send_prompt;
pub use store::{
    broadcast_git_changes, mark_session_closed, mark_session_open, open_session_ids,
    remember_session_directory, remember_session_port, remember_session_project,
    remember_session_title, session_directory,
};
pub use streaming::{conversation_for, latest_assistant_response};
pub use usage::{fetch_session_history, fetch_session_usage};

use crate::state::OpenCodeSession;
use base64::Engine;
use serde::Deserialize;
use std::time::Duration;

pub(crate) fn base_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}")
}

/// Значение заголовка `Authorization` для Basic-авторизации сервера OpenCode.
///
/// OpenCode защищает сервер HTTP Basic-авторизацией, если в окружении задан
/// `OPENCODE_SERVER_PASSWORD` (username — `OPENCODE_SERVER_USERNAME`, по
/// умолчанию `opencode`). Приложение запускает `opencode serve` с тем же
/// окружением, поэтому читает тот же пароль и передаёт его во всех запросах.
pub(crate) fn server_basic_auth() -> Option<String> {
    let password = std::env::var("OPENCODE_SERVER_PASSWORD").ok()?;
    if password.is_empty() {
        return None;
    }
    let username = std::env::var("OPENCODE_SERVER_USERNAME")
        .ok()
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| "opencode".to_string());
    let creds = format!("{username}:{password}");
    Some(format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode(creds.as_bytes())
    ))
}

pub(crate) fn http_client(timeout: Duration) -> reqwest::blocking::Client {
    let mut builder = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(timeout);
    if let Some(auth) = server_basic_auth() {
        if let Ok(value) = reqwest::header::HeaderValue::from_str(&auth) {
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(reqwest::header::AUTHORIZATION, value);
            builder = builder.default_headers(headers);
        }
    }
    builder.build().unwrap_or_else(|_| reqwest::blocking::Client::new())
}

#[derive(Deserialize)]
pub(crate) struct ModelRef {
    id: String,
}

#[derive(Deserialize)]
pub(crate) struct SessionInfo {
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    directory: String,
    #[serde(default)]
    time: Option<TimeInfo>,
    #[serde(default)]
    model: Option<ModelRef>,
}

#[derive(Deserialize)]
pub(crate) struct TimeInfo {
    #[serde(default)]
    created: u64,
    #[serde(default)]
    updated: u64,
}

impl From<SessionInfo> for OpenCodeSession {
    fn from(s: SessionInfo) -> Self {
        let updated_at = s
            .time
            .map(|t| if t.updated > 0 { t.updated } else { t.created })
            .unwrap_or(0);
        OpenCodeSession {
            id: s.id,
            title: s.title,
            directory: s.directory,
            updated_at,
            model: s.model.map(|m| m.id).unwrap_or_default(),
        }
    }
}
