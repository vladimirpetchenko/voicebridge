//! Потоковый приём ответа через SSE и ведение диалога сессии.

use crate::state::{AppStatus, ConversationMessage, ConversationStore, SharedState};
use serde::Deserialize;
use std::collections::HashSet;
use tauri::{AppHandle, Manager};

#[derive(Deserialize)]
struct RawEvent {
    #[serde(rename = "type")]
    event_type: String,
    properties: serde_json::Value,
}

fn prop_str(properties: &serde_json::Value, key: &str) -> Option<String> {
    properties.get(key)?.as_str().map(|s| s.to_string())
}

/// Максимальный размер входных/выходных данных инструмента, передаваемый в UI.
const TOOL_FIELD_MAX: usize = 2000;

/// Превращает JSON-значение в строку (строки как есть, остальное — компактный JSON).
fn value_to_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => String::new(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

/// Обрезает длинную строку до `TOOL_FIELD_MAX` символов.
fn truncate_field(s: String) -> String {
    if s.chars().count() > TOOL_FIELD_MAX {
        let mut t: String = s.chars().take(TOOL_FIELD_MAX).collect();
        t.push('…');
        t
    } else {
        s
    }
}

/// Читает SSE-поток событий и транслирует их во фронтенд.
pub(crate) fn read_sse(
    resp: reqwest::blocking::Response,
    app: &AppHandle,
    session_id: &str,
    port: u16,
) {
    use std::io::{BufRead, BufReader};
    let reader = BufReader::new(resp);
    let mut text = String::new();
    // PartID → признак «это размышления (reasoning)». Заполняется из
    // `message.part.updated`, используется в `message.part.delta` для
    // разделения потока на размышления и итоговый ответ.
    let mut reasoning_parts: HashSet<String> = HashSet::new();

    for line in reader.lines().map_while(Result::ok) {
        let line = line.trim();
        let Some(data) = line.strip_prefix("data:") else {
            continue;
        };
        let data = data.trim();
        if data.is_empty() {
            continue;
        }
        let Ok(ev) = serde_json::from_str::<RawEvent>(data) else {
            continue;
        };

        // Фильтруем события, не относящиеся к нашей сессии.
        if let Some(sid) = prop_str(&ev.properties, "sessionID") {
            if sid != session_id && !ev.event_type.starts_with("server.") {
                continue;
            }
        }

        match ev.event_type.as_str() {
            "message.part.delta" => {
                if prop_str(&ev.properties, "field").as_deref() == Some("text") {
                    if let Some(delta) = prop_str(&ev.properties, "delta") {
                        let part_id = prop_str(&ev.properties, "partID");
                        let is_reasoning = part_id
                            .as_ref()
                            .map(|id| reasoning_parts.contains(id))
                            .unwrap_or(false);
                        if is_reasoning {
                            append_assistant_reasoning_delta(app, session_id, &delta);
                            crate::modules::mobile::emit_and_broadcast(
                                app,
                                "opencode-reasoning-delta",
                                serde_json::json!({ "sessionId": session_id, "text": delta }),
                            );
                        } else {
                            text.push_str(&delta);
                            append_assistant_delta(app, session_id, &delta);
                            crate::modules::mobile::emit_and_broadcast(
                                app,
                                "opencode-delta",
                                serde_json::json!({ "sessionId": session_id, "text": delta }),
                            );
                        }
                    }
                }
            }
            "message.part.updated" => {
                if let Some(part) = ev.properties.get("part") {
                    // Запоминаем тип парта, чтобы отличать размышления от текста.
                    if let (Some(id), Some(kind)) = (
                        part.get("id").and_then(|x| x.as_str()),
                        part.get("type").and_then(|x| x.as_str()),
                    ) {
                        if kind == "reasoning" {
                            reasoning_parts.insert(id.to_string());
                        } else if kind == "text" {
                            reasoning_parts.remove(id);
                        }
                    }
                    if part.get("type").and_then(|t| t.as_str()) == Some("tool") {
                        let tool = part
                            .get("tool")
                            .and_then(|t| t.as_str())
                            .unwrap_or("")
                            .to_string();
                        let call_id = part
                            .get("callID")
                            .and_then(|c| c.as_str())
                            .unwrap_or("")
                            .to_string();
                        let status = part
                            .get("state")
                            .and_then(|s| s.get("status"))
                            .and_then(|s| s.as_str())
                            .unwrap_or("running");
                        let state = match status {
                            "completed" => "done",
                            "error" | "cancelled" => "failed",
                            _ => "running",
                        };
                        let input = part
                            .get("state")
                            .and_then(|s| s.get("input"))
                            .map(value_to_string)
                            .map(truncate_field)
                            .unwrap_or_default();
                        let output = part
                            .get("state")
                            .and_then(|s| s.get("output"))
                            .map(value_to_string)
                            .map(truncate_field)
                            .unwrap_or_default();
                        crate::modules::mobile::emit_and_broadcast(
                            app,
                            "opencode-tool",
                            serde_json::json!({
                                "sessionId": session_id,
                                "callId": call_id,
                                "name": tool,
                                "state": state,
                                "input": input,
                                "output": output,
                            }),
                        );
                    }
                }
            }
            "session.idle" => {
                log::info!("opencode response finished ({} chars)", text.chars().count());
                finish_response(app, session_id, text);
                return;
            }
            "permission.asked" => {
                let request_id = prop_str(&ev.properties, "id").unwrap_or_default();
                let permission = prop_str(&ev.properties, "permission").unwrap_or_default();
                let patterns: Vec<String> = ev
                    .properties
                    .get("patterns")
                    .and_then(|p| p.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                log::info!(
                    "permission.asked: id={request_id} permission={permission} patterns={:?}",
                    patterns
                );
                crate::modules::mobile::emit_and_broadcast(
                    app,
                    "opencode-permission",
                    serde_json::json!({
                        "sessionId": session_id,
                        "requestId": request_id,
                        "port": port,
                        "permission": permission,
                        "patterns": patterns,
                    }),
                );
            }
            "question.asked" => {
                let request_id = prop_str(&ev.properties, "id").unwrap_or_default();
                let questions = ev.properties.get("questions").cloned().unwrap_or_default();
                log::info!("question.asked: id={request_id}");
                crate::modules::mobile::emit_and_broadcast(
                    app,
                    "opencode-question",
                    serde_json::json!({
                        "sessionId": session_id,
                        "requestId": request_id,
                        "port": port,
                        "questions": questions,
                    }),
                );
            }
            "session.error" => {
                let err = ev
                    .properties
                    .get("error")
                    .map(|e| e.to_string())
                    .unwrap_or_else(|| "неизвестная ошибка".into());
                fail(app, session_id, format!("OpenCode: {err}"));
                return;
            }
            _ => {}
        }
    }

    // Поток завершился (соединение закрыто) — фиксируем накопленный текст.
    log::info!("opencode SSE stream ended ({} chars)", text.chars().count());
    finish_response(app, session_id, text);
}

pub(crate) fn set_status(app: &AppHandle, status: AppStatus, message: &str) {
    let state = app.state::<SharedState>();
    let mut s = state.0.lock().unwrap();
    s.status = status;
    s.status_message = message.to_string();
    let snapshot = s.clone();
    drop(s);
    crate::commands::emit_state(app, &snapshot);
    crate::commands::save_state(app, &snapshot);
}

pub(crate) fn fail(app: &AppHandle, session_id: &str, message: String) {
    log::error!("opencode error (session={session_id}): {message}");
    set_status(app, AppStatus::Error, &message);
    crate::modules::mobile::emit_and_broadcast(
        app,
        "opencode-error",
        serde_json::json!({ "sessionId": session_id, "error": message }),
    );
}

/// Завершает приём ответа: фиксирует итог и статус (только если сессия активна).
fn finish_response(app: &AppHandle, session_id: &str, text: String) {
    crate::modules::mobile::emit_and_broadcast(
        app,
        "opencode-done",
        serde_json::json!({ "sessionId": session_id }),
    );

    // После ответа подтягиваем свежие Git-изменения проекта (могли появиться
    // новые файлы/правки от инструментов).
    super::store::broadcast_git_changes(app, session_id.to_string());

    let is_selected = {
        let state = app.state::<SharedState>();
        let s = state.0.lock().unwrap();
        s.selected_session
            .as_ref()
            .map(|t| t.session_id == session_id)
            .unwrap_or(false)
    };

    if is_selected {
        let state = app.state::<SharedState>();
        let mut s = state.0.lock().unwrap();
        s.response = text;
        s.status = AppStatus::Idle;
        s.status_message = "Готов к работе".into();
        let snapshot = s.clone();
        drop(s);
        crate::commands::emit_state(app, &snapshot);
        crate::commands::save_state(app, &snapshot);
    }
}

/// Добавляет запрос пользователя в диалог сессии и создаёт пустой ответ ассистента.
pub(crate) fn record_user_message(app: &AppHandle, session_id: &str, text: &str) {
    let store = app.state::<ConversationStore>();
    let mut conv = store.conversations.lock().unwrap();
    let entry = conv.entry(session_id.to_string()).or_default();
    entry.push(ConversationMessage {
        role: "user".into(),
        text: text.to_string(),
        reasoning: String::new(),
    });
    entry.push(ConversationMessage {
        role: "assistant".into(),
        text: String::new(),
        reasoning: String::new(),
    });
    drop(conv);

    crate::modules::mobile::emit_and_broadcast(
        app,
        "opencode-user",
        serde_json::json!({ "sessionId": session_id, "text": text }),
    );
}

/// Дописывает фрагмент текста в последний ответ ассистента.
fn append_assistant_delta(app: &AppHandle, session_id: &str, delta: &str) {
    let store = app.state::<ConversationStore>();
    let mut conv = store.conversations.lock().unwrap();
    if let Some(msgs) = conv.get_mut(session_id) {
        if let Some(last) = msgs.last_mut() {
            if last.role == "assistant" {
                last.text.push_str(delta);
            }
        }
    }
}

/// Дописывает фрагмент размышлений (reasoning) в последний ответ ассистента.
fn append_assistant_reasoning_delta(app: &AppHandle, session_id: &str, delta: &str) {
    let store = app.state::<ConversationStore>();
    let mut conv = store.conversations.lock().unwrap();
    if let Some(msgs) = conv.get_mut(session_id) {
        if let Some(last) = msgs.last_mut() {
            if last.role == "assistant" {
                last.reasoning.push_str(delta);
            }
        }
    }
}

/// Последний ответ ассистента в диалоге сессии.
pub fn latest_assistant_response(app: &AppHandle, session_id: &str) -> String {
    let store = app.state::<ConversationStore>();
    let conv = store.conversations.lock().unwrap();
    conv.get(session_id)
        .and_then(|msgs| msgs.iter().rev().find(|m| m.role == "assistant"))
        .map(|m| m.text.clone())
        .unwrap_or_default()
}

/// Диалог сессии (запросы и ответы).
pub fn conversation_for(app: &AppHandle, session_id: &str) -> Vec<ConversationMessage> {
    let store = app.state::<ConversationStore>();
    let conv = store.conversations.lock().unwrap();
    conv.get(session_id).cloned().unwrap_or_default()
}
