//! Выбор целевой сессии и отправка промптов.

use super::{base_url, http_client, SessionInfo};
use crate::state::{AppStatus, OpenCodeTarget, SharedState};
use std::time::Duration;
use tauri::{AppHandle, Manager};

/// Возвращает выбранную сессию, а если её нет (или сервер упал) — создаёт новую
/// в первом доступном экземпляре OpenCode.
fn ensure_selected_session(app: &AppHandle, text: &str) -> Result<OpenCodeTarget, String> {
    let (selected, active) = {
        let state = app.state::<SharedState>();
        let s = state.0.lock().unwrap();
        (s.selected_session.clone(), s.active_instance.clone())
    };
    if let Some(t) = selected {
        if super::projects::is_server_running(t.port) {
            return Ok(t);
        }
    }

    let instances = super::discovery::discover_instances();
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
    let store = app.state::<crate::state::ConversationStore>();
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
            if super::projects::is_server_running(target.port) {
                return Ok(target);
            }
        }
    }
    ensure_selected_session(app, text)
}

/// Отправляет текст в сессию OpenCode (потоковый приём ответа).
/// Если сессия не выбрана — создаёт новую.
pub fn send_prompt(app: AppHandle, text: String, prefer_session: Option<String>) {
    super::streaming::set_status(&app, AppStatus::Processing, "OpenCode думает…");

    std::thread::Builder::new()
        .name("opencode-send".into())
        .spawn(move || {
            let target = match ensure_target_session(&app, &text, prefer_session.as_deref()) {
                Ok(t) => t,
                Err(e) => {
                    super::streaming::fail(&app, "", e);
                    return;
                }
            };

            let session_id = target.session_id;
            let port = target.port;
            let title_was_empty = target.title.trim().is_empty();

            log::info!("sending prompt to session {session_id} (port {port})");
            // Фиксируем запрос пользователя в диалоге сессии.
            super::streaming::record_user_message(&app, &session_id, &text);

            // Автоназвание: если сессия была пустой (без заголовка), после первого
            // сообщения подставляем осмысленное имя.
            if title_was_empty {
                let title: String = text.chars().take(60).collect();
                match super::update_session_title(port, &session_id, &title) {
                    Ok(()) => {
                        super::remember_session_title(&app, &session_id, &title);
                        let state = app.state::<SharedState>();
                        let mut s = state.0.lock().unwrap();
                        let mut changed = false;
                        if let Some(t) = s.selected_session.as_mut() {
                            if t.session_id == session_id && t.title.trim().is_empty() {
                                t.title = title.clone();
                                changed = true;
                            }
                        }
                        let snapshot = s.clone();
                        drop(s);
                        if changed {
                            crate::commands::emit_state(&app, &snapshot);
                            crate::commands::save_state(&app, &snapshot);
                        }
                    }
                    Err(e) => log::warn!("не удалось задать название сессии {session_id}: {e}"),
                }
            }

            let client = http_client(Duration::from_secs(600));
            let base = base_url(port);

            // 1. Подписываемся на SSE-события до отправки промпта.
            let sse = match client.get(format!("{base}/event")).send() {
                Ok(r) => r,
                Err(e) => {
                    super::streaming::fail(&app, &session_id, format!("не удалось подключиться к OpenCode: {e}"));
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
                    Ok(resp) => super::streaming::fail(&app2, &sid2, format!("OpenCode: HTTP {}", resp.status())),
                    Err(e) => super::streaming::fail(&app2, &sid2, format!("OpenCode: {e}")),
                }
            });

            // 3. Читаем события до завершения.
            super::streaming::read_sse(sse, &app, &session_id, port);
        })
        .expect("failed to spawn opencode thread");
}
