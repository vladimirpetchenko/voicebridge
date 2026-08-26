//! Модуль интеграции с OpenCode.
//!
//! Обнаруживает запущенные экземпляры OpenCode (HTTP-серверы на локальных
//! портах), читает список сессий и отправляет промпты с потоковым приёмом
//! ответа через SSE (`GET /event`).

use crate::state::{
    AppStatus, ConversationMessage, ConversationStore, OpenCodeInstance, OpenCodeSession,
    OpenCodeTarget, SharedState,
};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

/// Порты, на которых ищем экземпляры OpenCode.
const DEFAULT_PORTS: &[u16] = &[4096, 12000, 3000, 17000];

fn base_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}")
}

fn http_client(timeout: Duration) -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(timeout)
        .build()
        .unwrap_or_else(|_| reqwest::blocking::Client::new())
}

#[derive(Deserialize)]
struct SessionInfo {
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
struct ModelRef {
    id: String,
}

#[derive(Deserialize)]
struct TimeInfo {
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

fn extract_port(name: &str) -> Option<u16> {
    name.rsplit(':')
        .next()?
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .ok()
}

/// PID процессов opencode на Windows (через `tasklist`).
#[cfg(target_os = "windows")]
fn opencode_pids_windows() -> Vec<u32> {
    let out = match Command::new("tasklist")
        .args(["/FO", "CSV", "/NH"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut pids = Vec::new();
    for line in text.lines() {
        // CSV-формат: "opencode.exe","1234","Console","1","123 456 K"
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 2 {
            continue;
        }
        if !parts[0].trim_matches('"').to_lowercase().contains("opencode") {
            continue;
        }
        if let Ok(pid) = parts[1].trim_matches('"').parse::<u32>() {
            pids.push(pid);
        }
    }
    pids
}

/// Находит порты, на которых слушают процессы OpenCode (через `lsof`).
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn opencode_process_ports() -> Vec<u16> {
    let out = match Command::new("lsof")
        .args(["-nP", "-iTCP", "-sTCP:LISTEN"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut ports = Vec::new();
    for line in text.lines().skip(1) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 9 {
            continue;
        }
        if !fields[0].to_lowercase().contains("opencode") {
            continue;
        }
        if let Some(port) = extract_port(fields[8]) {
            ports.push(port);
        }
    }
    ports
}

/// Находит порты, на которых слушают процессы OpenCode (через `netstat` + `tasklist`).
#[cfg(target_os = "windows")]
fn opencode_process_ports() -> Vec<u16> {
    let pids = opencode_pids_windows();
    if pids.is_empty() {
        return Vec::new();
    }
    let out = match Command::new("netstat")
        .args(["-ano", "-p", "TCP"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut ports = Vec::new();
    for line in text.lines() {
        // Формат: Proto Local_Address Foreign_Address State PID
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 5 {
            continue;
        }
        if !fields[3].eq_ignore_ascii_case("LISTENING") {
            continue;
        }
        let pid: u32 = match fields[4].parse() {
            Ok(p) => p,
            Err(_) => continue,
        };
        if !pids.contains(&pid) {
            continue;
        }
        if let Some(port) = extract_port(fields[1]) {
            ports.push(port);
        }
    }
    ports
}

/// Собирает порты-кандидаты: процессы OpenCode + порты по умолчанию.
fn candidate_ports() -> Vec<u16> {
    let mut ports = opencode_process_ports();
    for &p in DEFAULT_PORTS {
        if !ports.contains(&p) {
            ports.push(p);
        }
    }
    ports
}

/// Опрашивает порты и возвращает обнаруженные проекты OpenCode с их сессиями.
///
/// `opencode serve` отдаёт глобальный список сессий (все проекты сразу), а
/// `/project/current` — глобальный проект (`/`), поэтому группируем сессии по
/// их `directory` (папке проекта), а не по серверу/порту.
pub fn discover_instances() -> Vec<OpenCodeInstance> {
    let client = http_client(Duration::from_millis(800));
    let mut running_ports: Vec<u16> = Vec::new();
    let mut by_dir: std::collections::BTreeMap<String, Vec<OpenCodeSession>> =
        std::collections::BTreeMap::new();

    for port in candidate_ports() {
        let url = format!("{}/session", base_url(port));
        let resp = match client.get(&url).send() {
            Ok(r) => r,
            Err(_) => continue,
        };
        if !resp.status().is_success() {
            continue;
        }
        let sessions: Vec<OpenCodeSession> = match resp.json::<Vec<SessionInfo>>() {
            Ok(list) => list.into_iter().map(OpenCodeSession::from).collect(),
            Err(_) => continue,
        };
        running_ports.push(port);
        for s in sessions {
            if s.directory.is_empty() {
                continue;
            }
            let entry = by_dir.entry(s.directory.clone()).or_default();
            if !entry.iter().any(|x| x.id == s.id) {
                entry.push(s);
            }
        }
    }

    if running_ports.is_empty() {
        return Vec::new();
    }

    // Ручной глобальный сервер (opencode serve на порту по умолчанию) отдаёт
    // сессии всех проектов — в этом случае показываем все проекты.
    let has_default_port = running_ports.iter().any(|p| DEFAULT_PORTS.contains(p));

    let mut instances: Vec<OpenCodeInstance> = by_dir
        .into_iter()
        .filter_map(|(directory, mut sessions)| {
            // Показываем проект только если запущен его «собственный» сервер
            // (порт project_port) или есть ручной глобальный сервер.
            let own_port = project_port(&directory);
            let port = if running_ports.contains(&own_port) {
                own_port
            } else if has_default_port {
                running_ports
                    .iter()
                    .copied()
                    .find(|p| DEFAULT_PORTS.contains(p))
                    .unwrap_or(running_ports[0])
            } else {
                return None;
            };
            sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
            let name = Path::new(&directory)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| directory.clone());
            log::info!(
                "discovered opencode project: {name} (port {port}, {} sessions)",
                sessions.len()
            );
            Some(OpenCodeInstance {
                id: directory.clone(),
                name,
                port,
                sessions,
            })
        })
        .collect();

    instances.sort_by(|a, b| {
        let au = a.sessions.iter().map(|s| s.updated_at).max().unwrap_or(0);
        let bu = b.sessions.iter().map(|s| s.updated_at).max().unwrap_or(0);
        bu.cmp(&au)
    });
    instances
}

// ---------------------------------------------------------------------------
// Отправка промпта и потоковый приём ответа
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct RawEvent {
    #[serde(rename = "type")]
    event_type: String,
    properties: serde_json::Value,
}

fn prop_str(properties: &serde_json::Value, key: &str) -> Option<String> {
    properties.get(key)?.as_str().map(|s| s.to_string())
}

/// Читает SSE-поток событий и транслирует их во фронтенд.
fn read_sse(
    resp: reqwest::blocking::Response,
    app: &AppHandle,
    session_id: &str,
    port: u16,
) {
    use std::io::{BufRead, BufReader};
    let reader = BufReader::new(resp);
    let mut text = String::new();

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
                        text.push_str(&delta);
                        append_assistant_delta(app, session_id, &delta);
                        let _ = app.emit(
                            "opencode-delta",
                            serde_json::json!({ "sessionId": session_id, "text": delta }),
                        );
                    }
                }
            }
            "message.part.updated" => {
                if let Some(part) = ev.properties.get("part") {
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
                        let _ = app.emit(
                            "opencode-tool",
                            serde_json::json!({ "sessionId": session_id, "callId": call_id, "name": tool, "state": state }),
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
                let _ = app.emit(
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
                let _ = app.emit(
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

fn set_status(app: &AppHandle, status: AppStatus, message: &str) {
    let state = app.state::<SharedState>();
    let mut s = state.0.lock().unwrap();
    s.status = status;
    s.status_message = message.to_string();
    let snapshot = s.clone();
    drop(s);
    crate::commands::emit_state(app, &snapshot);
    crate::commands::save_state(app, &snapshot);
}

fn fail(app: &AppHandle, session_id: &str, message: String) {
    log::error!("opencode error (session={session_id}): {message}");
    set_status(app, AppStatus::Error, &message);
    let _ = app.emit(
        "opencode-error",
        serde_json::json!({ "sessionId": session_id, "error": message }),
    );
}

/// Завершает приём ответа: фиксирует итог и статус (только если сессия активна).
fn finish_response(app: &AppHandle, session_id: &str, text: String) {
    let _ = app.emit(
        "opencode-done",
        serde_json::json!({ "sessionId": session_id }),
    );

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
fn record_user_message(app: &AppHandle, session_id: &str, text: &str) {
    let store = app.state::<ConversationStore>();
    let mut conv = store.conversations.lock().unwrap();
    let entry = conv.entry(session_id.to_string()).or_default();
    entry.push(ConversationMessage {
        role: "user".into(),
        text: text.to_string(),
    });
    entry.push(ConversationMessage {
        role: "assistant".into(),
        text: String::new(),
    });
    drop(conv);

    let _ = app.emit(
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
    for entry in entries {
        let text = entry
            .parts
            .iter()
            .filter(|p| p.kind == "text" && !p.text.trim().is_empty())
            .map(|p| p.text.trim())
            .collect::<Vec<_>>()
            .join("\n\n");
        if text.is_empty() {
            continue; // пропускаем сообщения-инструменты без текста
        }
        messages.push(ConversationMessage {
            role: entry.info.role,
            text,
        });
    }
    Ok(messages)
}

/// Возвращает выбранную сессию, а если её нет (или сервер упал) — создаёт новую
/// в первом доступном экземпляре OpenCode.
fn ensure_selected_session(app: &AppHandle, text: &str) -> Result<OpenCodeTarget, String> {
    let (selected, active) = {
        let state = app.state::<SharedState>();
        let s = state.0.lock().unwrap();
        (s.selected_session.clone(), s.active_instance.clone())
    };
    if let Some(t) = selected {
        if is_server_running(t.port) {
            return Ok(t);
        }
    }

    let instances = discover_instances();
    if instances.is_empty() {
        return Err("нет запущенных серверов OpenCode — запустите проект в панели «Проекты»".into());
    }
    // Предпочитаем активный экземпляр, иначе — первый доступный.
    let instance = active
        .and_then(|a| instances.iter().find(|i| i.port == a.port).cloned())
        .or_else(|| instances.first().cloned())
        .unwrap();

    let title: String = text.chars().take(60).collect();
    let client = http_client(Duration::from_secs(10));
    let resp = client
        .post(format!("{}/session", base_url(instance.port)))
        .json(&serde_json::json!({ "title": title }))
        .send()
        .map_err(|e| format!("не удалось создать сессию: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("не удалось создать сессию: HTTP {}", resp.status()));
    }
    let session: SessionInfo = resp
        .json()
        .map_err(|e| format!("не удалось создать сессию: {e}"))?;

    let target = OpenCodeTarget {
        instance_id: instance.id.clone(),
        port: instance.port,
        session_id: session.id,
        title: if session.title.is_empty() {
            title
        } else {
            session.title
        },
    };

    let state = app.state::<SharedState>();
    let mut s = state.0.lock().unwrap();
    s.selected_session = Some(target.clone());
    let snapshot = s.clone();
    drop(s);
    crate::commands::emit_state(app, &snapshot);
    crate::commands::save_state(app, &snapshot);

    Ok(target)
}

/// Возвращает цель (порт + заголовок) для известной сессии по её id.
fn target_for_session(app: &AppHandle, session_id: &str) -> Option<OpenCodeTarget> {
    let store = app.state::<ConversationStore>();
    let port = store.ports.lock().unwrap().get(session_id).copied()?;
    let title = store
        .titles
        .lock()
        .unwrap()
        .get(session_id)
        .cloned()
        .unwrap_or_default();
    Some(OpenCodeTarget {
        instance_id: format!("port-{port}"),
        port,
        session_id: session_id.to_string(),
        title,
    })
}

/// Возвращает выбранную сессию, а если её нет (или сервер упал) — создаёт новую
/// в первом доступном экземпляре OpenCode. Если указана конкретная сессия
/// (например, из окна чата) — использует её.
fn ensure_target_session(
    app: &AppHandle,
    text: &str,
    prefer_session: Option<&str>,
) -> Result<OpenCodeTarget, String> {
    if let Some(sid) = prefer_session {
        if let Some(target) = target_for_session(app, sid) {
            if is_server_running(target.port) {
                return Ok(target);
            }
        }
    }
    ensure_selected_session(app, text)
}

/// Отправляет текст в сессию OpenCode (потоковый приём ответа).
/// Если сессия не выбрана — создаёт новую.
pub fn send_prompt(app: AppHandle, text: String, prefer_session: Option<String>) {
    set_status(&app, AppStatus::Processing, "OpenCode думает…");

    std::thread::Builder::new()
        .name("opencode-send".into())
        .spawn(move || {
            let target = match ensure_target_session(&app, &text, prefer_session.as_deref()) {
                Ok(t) => t,
                Err(e) => {
                    fail(&app, "", e);
                    return;
                }
            };

            let session_id = target.session_id;
            let port = target.port;

            log::info!("sending prompt to session {session_id} (port {port})");
            // Фиксируем запрос пользователя в диалоге сессии.
            record_user_message(&app, &session_id, &text);

            let client = http_client(Duration::from_secs(600));
            let base = base_url(port);

            // 1. Подписываемся на SSE-события до отправки промпта.
            let sse = match client.get(format!("{base}/event")).send() {
                Ok(r) => r,
                Err(e) => {
                    fail(&app, &session_id, format!("не удалось подключиться к OpenCode: {e}"));
                    return;
                }
            };

            // 2. Отправляем промпт в отдельном потоке (не блокируем чтение SSE).
            let c2 = client.clone();
            let url = format!("{base}/session/{session_id}/message");
            let t2 = text.clone();
            let app2 = app.clone();
            let sid2 = session_id.clone();
            std::thread::spawn(move || {
                match c2
                    .post(&url)
                    .json(&serde_json::json!({ "parts": [{ "type": "text", "text": t2 }] }))
                    .send()
                {
                    Ok(resp) if resp.status().is_success() => {}
                    Ok(resp) => fail(&app2, &sid2, format!("OpenCode: HTTP {}", resp.status())),
                    Err(e) => fail(&app2, &sid2, format!("OpenCode: {e}")),
                }
            });

            // 3. Читаем события до завершения.
            read_sse(sse, &app, &session_id, port);
        })
        .expect("failed to spawn opencode thread");
}

// ---------------------------------------------------------------------------
// Управление проектами (запуск/остановка серверов OpenCode)
// ---------------------------------------------------------------------------

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub worktree: String,
    pub name: String,
    pub updated: u64,
    pub running: bool,
    pub port: u16,
}

#[derive(Deserialize)]
struct DirRow {
    worktree: String,
    #[serde(default)]
    updated: u64,
}

/// Находит исполняемый файл opencode (PATH у GUI-приложений ограничен).
pub fn opencode_binary() -> String {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        for p in ["/opt/homebrew/bin/opencode", "/usr/local/bin/opencode"] {
            if Path::new(p).exists() {
                return p.to_string();
            }
        }
        if let Ok(out) = Command::new("sh")
            .args(["-lc", "command -v opencode"])
            .output()
        {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return s;
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        // `where opencode` отдаёт пути в OEM-кодировке (CP866/CP1251), поэтому
        // для путей с кириллицей при декодировании в UTF-8 получается «мусор»
        // и файл не находится. Ищем бинарник напрямую по каталогам из PATH
        // (env::var возвращает корректную строку). npm-шим содержит `opencode`
        // (bash-скрипт), `opencode.cmd` и `opencode.ps1`; запускать напрямую
        // можно только .exe и .cmd/.bat, поэтому предпочитаем их.
        if let Ok(path_var) = std::env::var("PATH") {
            let mut shim: Option<std::path::PathBuf> = None;
            for dir in std::env::split_paths(&path_var) {
                let exe = dir.join("opencode.exe");
                if exe.is_file() {
                    return exe.to_string_lossy().into_owned();
                }
                if shim.is_none() {
                    for name in ["opencode.cmd", "opencode.bat"] {
                        let candidate = dir.join(name);
                        if candidate.is_file() {
                            shim = Some(candidate);
                            break;
                        }
                    }
                }
            }
            if let Some(p) = shim {
                return p.to_string_lossy().into_owned();
            }
        }
    }
    "opencode".to_string()
}

/// Собирает команду запуска opencode. На Windows npm-шим (`.cmd`/`.bat`)
/// нельзя запускать напрямую — оборачиваем в `cmd /C`.
fn opencode_command(bin: &str) -> Command {
    #[cfg(target_os = "windows")]
    {
        if bin.ends_with(".cmd") || bin.ends_with(".bat") {
            let mut c = Command::new("cmd");
            c.arg("/C").arg(bin);
            return c;
        }
    }
    Command::new(bin)
}

/// Стабильный порт для проекта (по хэшу пути, диапазон 4100–4199).
fn project_port(worktree: &str) -> u16 {
    // Нормализуем разделители: `opencode db` отдаёт пути с `/`, а `GET /session`
    // и диалог выбора папки — с `\`. Без нормализации один и тот же проект
    // получал бы разные порты.
    let normalized = worktree.replace('\\', "/");
    let mut h: u32 = 2_166_136_261;
    for b in normalized.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(16_777_619);
    }
    (4100 + h % 100) as u16
}

fn is_server_running(port: u16) -> bool {
    let client = http_client(Duration::from_millis(300));
    client
        .get(format!("{}/session", base_url(port)))
        .send()
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

/// PID процесса opencode, слушающего заданный порт (через lsof).
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn opencode_pid_on_port(port: u16) -> Option<u32> {
    let out = Command::new("lsof")
        .args(["-nP", &format!("-iTCP:{port}"), "-sTCP:LISTEN"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines().skip(1) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() >= 2 && fields[0].to_lowercase().contains("opencode") {
            return fields[1].parse().ok();
        }
    }
    None
}

/// PID процесса opencode, слушающего заданный порт (через `netstat` + `tasklist`).
#[cfg(target_os = "windows")]
fn opencode_pid_on_port(port: u16) -> Option<u32> {
    let pids = opencode_pids_windows();
    if pids.is_empty() {
        return None;
    }
    let out = Command::new("netstat")
        .args(["-ano", "-p", "TCP"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 5 {
            continue;
        }
        if !fields[3].eq_ignore_ascii_case("LISTENING") {
            continue;
        }
        let pid: u32 = fields[4].parse().ok()?;
        if pids.contains(&pid) && extract_port(fields[1]) == Some(port) {
            return Some(pid);
        }
    }
    None
}

/// Список известных проектов (папок, где были сессии opencode).
pub fn list_projects() -> Vec<Project> {
    let bin = opencode_binary();
    const QUERY: &str = "SELECT directory AS worktree, MAX(time_updated) AS updated FROM session WHERE directory != '' GROUP BY directory ORDER BY updated DESC";
    let out = match opencode_command(&bin)
        .args(["db", QUERY, "--format", "json"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    if !out.status.success() {
        return Vec::new();
    }
    let rows: Vec<DirRow> = match serde_json::from_slice(&out.stdout) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let mut projects: Vec<Project> = rows
        .into_iter()
        .filter(|r| !r.worktree.is_empty() && Path::new(&r.worktree).parent().is_some())
        .map(|r| {
            let name = Path::new(&r.worktree)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| r.worktree.clone());
            let port = project_port(&r.worktree);
            Project {
                id: r.worktree.clone(),
                worktree: r.worktree,
                name,
                updated: r.updated,
                running: is_server_running(port),
                port,
            }
        })
        .collect();

    projects.sort_by(|a, b| b.updated.cmp(&a.updated));
    projects
}

/// Запускает headless-сервер OpenCode для проекта (в его папке).
pub fn start_project(worktree: &str) -> Result<(), String> {
    let port = project_port(worktree);
    if is_server_running(port) {
        return Ok(());
    }
    if !Path::new(worktree).is_dir() {
        return Err("папка проекта не существует".into());
    }
    let bin = opencode_binary();
    opencode_command(&bin)
        .args(["serve", "--port", &port.to_string(), "--hostname", "127.0.0.1"])
        .current_dir(worktree)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("не удалось запустить opencode: {e}"))?;
    Ok(())
}

/// Останавливает headless-сервер OpenCode для проекта.
pub fn stop_project(worktree: &str) -> Result<(), String> {
    let port = project_port(worktree);
    let pid = opencode_pid_on_port(port).ok_or("сервер для проекта не запущен")?;

    #[cfg(target_os = "windows")]
    let status = {
        let pid_str = pid.to_string();
        Command::new("taskkill")
            .args(["/PID", pid_str.as_str(), "/F"])
            .status()
            .map_err(|e| e.to_string())?
    };
    #[cfg(not(target_os = "windows"))]
    let status = Command::new("kill")
        .arg(pid.to_string())
        .status()
        .map_err(|e| e.to_string())?;

    if status.success() {
        Ok(())
    } else {
        Err("не удалось остановить сервер".into())
    }
}

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
