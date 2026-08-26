use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

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
    pub model: String,
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
pub struct KnownDevice {
    pub id: String,
    pub name: String,
    pub last_seen: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppState {
    pub status: AppStatus,
    pub status_message: String,
    pub recording: bool,
    pub sensitivity: f32,
    pub silence_timeout: f32,
    pub send_mode: String,
    pub hotkey: String,
    pub language: String,
    pub mobile_enabled: bool,
    pub mobile_port: u16,
    pub mobile_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_model: Option<String>,
    pub transcript: String,
    pub response: String,
    /// Сессия, в которую адресована текущая запись/распознанный текст.
    /// None — запись не из окна чата (хоткей/трей), берём selected_session.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recording_session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_microphone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_session: Option<OpenCodeTarget>,
    /// Модель OpenCode выбранной сессии (для строки состояния лаунчера).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opencode_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_instance: Option<OpenCodeInstanceRef>,
    /// Известные (сохранённые) мобильные устройства.
    #[serde(default)]
    pub known_devices: Vec<KnownDevice>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            status: AppStatus::Idle,
            status_message: "Готов к работе".into(),
            recording: false,
            sensitivity: 1.0,
            silence_timeout: 3.0,
            send_mode: "direct".into(),
            hotkey: if cfg!(target_os = "macos") {
                "Cmd+Shift+V".into()
            } else {
                "Ctrl+Shift+V".into()
            },
            language: "auto".into(),
            mobile_enabled: false,
            mobile_port: 47800,
            mobile_token: String::new(),
            selected_model: None,
            transcript: String::new(),
            response: String::new(),
            recording_session_id: None,
            selected_microphone: None,
            selected_session: None,
            opencode_model: None,
            active_instance: None,
            known_devices: Vec::new(),
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

/// Информация о сессии для шапки окна чата.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub title: String,
    pub project: String,
}

/// Использование токенов/средств сессии (строка состояния в чате).
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionUsage {
    pub tokens_input: u64,
    pub tokens_output: u64,
    pub tokens_reasoning: u64,
    pub tokens_total: u64,
    pub cost: f64,
    pub context_limit: u64,
    pub model: String,
}

/// Диалоги по сессиям OpenCode (ключ — session_id). Хранится в памяти.
pub struct ConversationStore {
    pub conversations: Mutex<HashMap<String, Vec<ConversationMessage>>>,
    pub ports: Mutex<HashMap<String, u16>>,
    pub titles: Mutex<HashMap<String, String>>,
    /// Имя проекта (папки) для сессии — для шапки окна чата.
    pub projects: Mutex<HashMap<String, String>>,
    /// Сессии, для которых сейчас открыто окно чата.
    pub open_sessions: Mutex<HashSet<String>>,
}

impl Default for ConversationStore {
    fn default() -> Self {
        Self {
            conversations: Mutex::new(HashMap::new()),
            ports: Mutex::new(HashMap::new()),
            titles: Mutex::new(HashMap::new()),
            projects: Mutex::new(HashMap::new()),
            open_sessions: Mutex::new(HashSet::new()),
        }
    }
}
