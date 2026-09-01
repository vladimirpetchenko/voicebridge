//! Токены/стоимость и история сессии OpenCode.

use super::{base_url, http_client, ModelRef};
use crate::state::{ConversationMessage, ConversationStore};
use serde::Deserialize;
use std::time::Duration;
use tauri::{AppHandle, Manager};

/// Приблизительный лимит контекста модели (в токенах). В API OpenCode точного
/// значения нет, поэтому определяем по семейству модели.
fn model_context_limit(model_id: &str) -> u64 {
    let m = model_id.to_lowercase();
    if m.contains("gemini") {
        1_000_000
    } else if m.contains("claude") || m.contains("sonnet") || m.contains("opus") || m.contains("haiku") {
        200_000
    } else {
        128_000
    }
}

#[derive(Deserialize)]
struct UsageTokens {
    #[serde(default)]
    input: u64,
    #[serde(default)]
    output: u64,
    #[serde(default)]
    reasoning: u64,
}

#[derive(Deserialize)]
struct UsageSessionDetail {
    #[serde(default)]
    cost: f64,
    tokens: UsageTokens,
    #[serde(default)]
    model: Option<ModelRef>,
}

/// Читает использование токенов/средств сессии с сервера OpenCode.
pub fn fetch_session_usage(app: &AppHandle, session_id: &str) -> Option<crate::state::SessionUsage> {
    let port = app
        .state::<ConversationStore>()
        .ports
        .lock()
        .unwrap()
        .get(session_id)
        .copied()?;
    let client = http_client(Duration::from_secs(5));
    let resp = client
        .get(format!("{}/session/{}", base_url(port), session_id))
        .send()
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let detail: UsageSessionDetail = resp.json().ok()?;
    let model = detail.model.map(|m| m.id).unwrap_or_default();
    let tokens_input = detail.tokens.input;
    let tokens_output = detail.tokens.output;
    let tokens_reasoning = detail.tokens.reasoning;
    let tokens_total = tokens_input + tokens_output + tokens_reasoning;
    Some(crate::state::SessionUsage {
        tokens_input,
        tokens_output,
        tokens_reasoning,
        tokens_total,
        cost: detail.cost,
        context_limit: model_context_limit(&model),
        model,
    })
}

#[derive(Deserialize)]
struct HistoryMessage {
    info: HistoryInfo,
    parts: Vec<HistoryPart>,
}

#[derive(Deserialize)]
struct HistoryInfo {
    #[serde(default)]
    role: String,
}

#[derive(Deserialize)]
struct HistoryPart {
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    text: String,
}

/// Подтягивает историю сессии из OpenCode (запросы и текстовые ответы).
pub fn fetch_session_history(port: u16, session_id: &str) -> Result<Vec<ConversationMessage>, String> {
    let client = http_client(Duration::from_secs(10));
    let resp = client
        .get(format!("{}/session/{}/message", base_url(port), session_id))
        .send()
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let entries: Vec<HistoryMessage> = resp.json().map_err(|e| e.to_string())?;

    let mut messages = Vec::new();
    // OpenCode разбивает один ответ ассистента на несколько сообщений:
    // сначала шаг с размышлениями (reasoning) и инструментами, затем шаг с
    // итоговым текстом. Доклеиваем размышления к следующему текстовому ответу,
    // чтобы при загрузке истории они не выносились в отдельные сообщения.
    let mut pending_reasoning = String::new();

    for entry in entries {
        let text = entry
            .parts
            .iter()
            .filter(|p| p.kind == "text" && !p.text.trim().is_empty())
            .map(|p| p.text.trim())
            .collect::<Vec<_>>()
            .join("\n\n");
        let reasoning = entry
            .parts
            .iter()
            .filter(|p| p.kind == "reasoning" && !p.text.trim().is_empty())
            .map(|p| p.text.trim())
            .collect::<Vec<_>>()
            .join("\n\n");
        let role = entry.info.role.clone();

        // Шаг ассистента только с размышлениями (без текста) — накапливаем.
        if role == "assistant" && text.is_empty() {
            if !reasoning.is_empty() {
                if !pending_reasoning.is_empty() {
                    pending_reasoning.push_str("\n\n");
                }
                pending_reasoning.push_str(&reasoning);
            }
            continue;
        }

        if text.is_empty() {
            continue; // пропускаем сообщения-инструменты без текста
        }

        let mut reasoning = reasoning;
        if role == "assistant" {
            if !pending_reasoning.is_empty() {
                reasoning = if reasoning.is_empty() {
                    std::mem::take(&mut pending_reasoning)
                } else {
                    format!("{}\n\n{}", std::mem::take(&mut pending_reasoning), reasoning)
                };
            }
        } else {
            pending_reasoning.clear();
        }

        messages.push(ConversationMessage {
            role,
            text,
            reasoning,
        });
    }
    Ok(messages)
}
