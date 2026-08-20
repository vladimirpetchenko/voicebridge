use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AppMode {
    #[default]
    OpenCode,
    Gui,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AppStatus {
    #[default]
    Idle,
    Recording,
    Processing,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenCodeSession {
    pub id: String,
    pub title: String,
    pub directory: String,
    pub updated_at: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenCodeInstance {
    pub id: String,
    pub name: String,
    pub port: u16,
    pub sessions: Vec<OpenCodeSession>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenCodeTarget {
    pub instance_id: String,
    pub port: u16,
    pub session_id: String,
    pub title: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenCodeInstanceRef {
    pub id: String,
    pub port: u16,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowInfo {
    pub id: String,
    pub title: String,
    pub app_name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppState {
    pub mode: AppMode,
    pub status: AppStatus,
    pub status_message: String,
    pub recording: bool,
    pub sensitivity: f32,
    pub silence_timeout: f32,
    pub language: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_model: Option<String>,
    pub transcript: String,
    pub response: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_microphone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_session: Option<OpenCodeTarget>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_instance: Option<OpenCodeInstanceRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_window: Option<WindowInfo>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            mode: AppMode::OpenCode,
            status: AppStatus::Idle,
            status_message: "Готов к работе".into(),
            recording: false,
            sensitivity: 1.0,
            silence_timeout: 3.0,
            language: "auto".into(),
            selected_model: None,
            transcript: String::new(),
            response: String::new(),
            selected_microphone: None,
            selected_session: None,
            active_instance: None,
            selected_window: None,
        }
    }
}
pub struct SharedState(pub Mutex<AppState>);

impl Default for SharedState {
    fn default() -> Self {
        Self(Mutex::new(AppState::default()))
    }
}

/// Сообщение диалога сессии OpenCode (запрос пользователя или ответ ассистента).
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationMessage {
    pub role: String,
    pub text: String,
}

/// Диалоги по сессиям OpenCode (ключ — session_id). Хранится в памяти.
pub struct ConversationStore {
    pub conversations: Mutex<HashMap<String, Vec<ConversationMessage>>>,
    pub ports: Mutex<HashMap<String, u16>>,
}

impl Default for ConversationStore {
    fn default() -> Self {
        Self {
            conversations: Mutex::new(HashMap::new()),
            ports: Mutex::new(HashMap::new()),
        }
    }
}
