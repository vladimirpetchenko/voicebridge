//! Действия OpenCode: разрешения, вопросы, прерывание генерации.

use super::{base_url, http_client};
use std::time::Duration;

/// Отвечает на запрос разрешения OpenCode (once / always / reject).
pub fn reply_permission(port: u16, request_id: &str, reply: &str) -> Result<(), String> {
    let client = http_client(Duration::from_secs(10));
    let resp = client
        .post(format!("{}/permission/{}/reply", base_url(port), request_id))
        .json(&serde_json::json!({ "reply": reply }))
        .send()
        .map_err(|e| e.to_string())?;
    if resp.status().is_success() {
        Ok(())
    } else {
        Err(format!("HTTP {}", resp.status()))
    }
}

/// Отвечает на вопрос OpenCode (answers — массив массивов выбранных меток).
pub fn reply_question(
    port: u16,
    request_id: &str,
    answers: Vec<Vec<String>>,
) -> Result<(), String> {
    let client = http_client(Duration::from_secs(10));
    let resp = client
        .post(format!("{}/question/{}/reply", base_url(port), request_id))
        .json(&serde_json::json!({ "answers": answers }))
        .send()
        .map_err(|e| e.to_string())?;
    if resp.status().is_success() {
        Ok(())
    } else {
        Err(format!("HTTP {}", resp.status()))
    }
}

/// Отклоняет вопрос OpenCode.
pub fn reject_question(port: u16, request_id: &str) -> Result<(), String> {
    let client = http_client(Duration::from_secs(10));
    let resp = client
        .post(format!("{}/question/{}/reject", base_url(port), request_id))
        .send()
        .map_err(|e| e.to_string())?;
    if resp.status().is_success() {
        Ok(())
    } else {
        Err(format!("HTTP {}", resp.status()))
    }
}

/// Прерывает генерацию текущего ответа сессии.
pub fn abort_session(port: u16, session_id: &str) -> Result<(), String> {
    let client = http_client(Duration::from_secs(10));
    let resp = client
        .post(format!("{}/session/{}/abort", base_url(port), session_id))
        .send()
        .map_err(|e| e.to_string())?;
    if resp.status().is_success() {
        Ok(())
    } else {
        Err(format!("HTTP {}", resp.status()))
    }
}
