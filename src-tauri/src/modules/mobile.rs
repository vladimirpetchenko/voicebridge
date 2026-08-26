//! Модуль мобильного доступа.
//!
//! Встроенный WebSocket-сервер, который принимает команды с мобильного
//! приложения и транслирует события (`state-changed`, `opencode-*`) обратно.
//! Авторизация — по токену, который десктоп показывает в виде QR-кода.

use std::collections::HashMap;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use futures_util::{SinkExt, StreamExt};
use qr_code::QrCode;
use tauri::{AppHandle, Emitter, Manager};

use crate::state::{ConversationStore, SharedState};

/// Состояние мобильного сервера: канал рассылки событий подключённым клиентам.
pub struct MobileServer {
    pub tx: tokio::sync::broadcast::Sender<String>,
}

impl Default for MobileServer {
    fn default() -> Self {
        let (tx, _rx) = tokio::sync::broadcast::channel(256);
        Self { tx }
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MobileInfo {
    pub enabled: bool,
    pub port: u16,
    pub ip: String,
    pub token: String,
    pub uri: String,
    pub qr_svg: String,
}

pub fn generate_token() -> String {
    use rand::Rng;
    const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut rng = rand::thread_rng();
    (0..32)
        .map(|_| CHARS[rng.gen_range(0..CHARS.len())] as char)
        .collect()
}

fn qr_svg(data: &str) -> String {
    let code = match QrCode::new(data.as_bytes()) {
        Ok(c) => c,
        Err(_) => return String::new(),
    };
    let width = code.width();
    let bits = code.to_vec();
    let scale = 4;
    let size = width * scale;
    let mut svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{size}" height="{size}" shape-rendering="crispEdges" viewBox="0 0 {size} {size}"><rect width="100%" height="100%" fill="#ffffff"/>"##
    );
    for y in 0..width {
        for x in 0..width {
            if bits[y * width + x] {
                svg.push_str(&format!(
                    r##"<rect x="{}" y="{}" width="{scale}" height="{scale}" fill="#0f1218"/>"##,
                    x * scale,
                    y * scale
                ));
            }
        }
    }
    svg.push_str("</svg>");
    svg
}

fn event_msg(name: &str, data: serde_json::Value) -> String {
    serde_json::json!({ "type": "event", "name": name, "data": data }).to_string()
}

/// Рассылает событие подключённым мобильным клиентам.
pub fn broadcast(app: &AppHandle, name: &str, data: serde_json::Value) {
    if let Some(server) = app.try_state::<MobileServer>() {
        let _ = server.tx.send(event_msg(name, data));
    }
}

/// Транслирует событие и в окна Tauri, и на мобильных клиентов.
pub fn emit_and_broadcast(app: &AppHandle, name: &str, data: serde_json::Value) {
    let _ = app.emit(name, data.clone());
    broadcast(app, name, data);
}

/// Запускает WebSocket-сервер (вызывается один раз при старте приложения).
pub async fn serve(app: AppHandle) {
    let port = {
        let state = app.state::<SharedState>();
        let s = state.0.lock().unwrap();
        let p = s.mobile_port;
        drop(s);
        p
    };

    let router = Router::new()
        .route("/", get(ws_handler))
        .route("/health", get(health))
        .with_state(app);

    let addr = format!("0.0.0.0:{port}");
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            log::error!("mobile server: не удалось занять {addr}: {e}");
            return;
        }
    };
    log::info!("mobile server listening on {addr}");
    if let Err(e) = axum::serve(listener, router).await {
        log::error!("mobile server error: {e}");
    }
}

async fn health() -> &'static str {
    "ok"
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(params): Query<HashMap<String, String>>,
    State(app): State<AppHandle>,
) -> impl IntoResponse {
    let (enabled, token) = {
        let state = app.state::<SharedState>();
        let s = state.0.lock().unwrap();
        (s.mobile_enabled, s.mobile_token.clone())
    };
    if !enabled {
        return (
            axum::http::StatusCode::FORBIDDEN,
            "mobile access disabled",
        )
            .into_response();
    }
    let provided = params.get("token").cloned().unwrap_or_default();
    if token.is_empty() || provided != token {
        return (axum::http::StatusCode::UNAUTHORIZED, "invalid token").into_response();
    }
    ws.on_upgrade(move |socket| handle_socket(socket, app))
}

async fn handle_socket(socket: WebSocket, app: AppHandle) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = app.state::<MobileServer>().tx.subscribe();

    loop {
        tokio::select! {
            ev = rx.recv() => {
                match ev {
                    Ok(msg) => {
                        if sender.send(Message::Text(msg.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            incoming = receiver.next() => {
                match incoming {
                    Some(Ok(msg)) => {
                        match msg {
                            Message::Text(text) => {
                                if let Some(reply) = handle_command(&app, &text).await {
                                    if sender.send(Message::Text(reply.into())).await.is_err() {
                                        break;
                                    }
                                }
                            }
                            Message::Close(_) => break,
                            // Binary/Ping/Pong — не используются, игнорируем.
                            _ => {}
                        }
                    }
                    Some(Err(_)) => break,
                    None => break,
                }
            }
        }
    }
}

/// Обрабатывает входящее сообщение (блокирующие вызовы — в spawn_blocking).
async fn handle_command(app: &AppHandle, text: &str) -> Option<String> {
    let app = app.clone();
    let text = text.to_string();
    tokio::task::spawn_blocking(move || dispatch(&app, &text))
        .await
        .ok()?
}

fn dispatch(app: &AppHandle, text: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    let id = v.get("id").and_then(|x| x.as_str()).unwrap_or("");
    let name = v.get("name").and_then(|x| x.as_str()).unwrap_or("");

    let result = run_command(app, name, &v);
    let resp = match result {
        Ok(data) => serde_json::json!({ "type": "response", "id": id, "ok": true, "data": data }),
        Err(e) => serde_json::json!({ "type": "response", "id": id, "ok": false, "error": e }),
    };
    Some(resp.to_string())
}

fn get_str(args: &serde_json::Value, key: &str) -> String {
    args.get(key)
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string()
}

fn run_command(
    app: &AppHandle,
    name: &str,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    match name {
        "ping" => Ok(serde_json::json!({ "pong": true })),
        "list_sessions" => Ok(serde_json::to_value(crate::modules::opencode::discover_instances())
            .unwrap_or(serde_json::Value::Array(vec![]))),
        "get_state" => Ok(serde_json::to_value(crate::commands::get_app_state(app.clone()))
            .unwrap_or(serde_json::Value::Null)),
        "select_session" => {
            let instance_id = get_str(args, "instanceId");
            let port = args.get("port").and_then(|x| x.as_u64()).unwrap_or(0) as u16;
            let session_id = get_str(args, "sessionId");
            let title = get_str(args, "title");
            let model = get_str(args, "model");
            if session_id.is_empty() {
                return Err("sessionId обязателен".into());
            }
            let state = crate::commands::select_opencode_session(
                app.clone(),
                instance_id,
                port,
                session_id,
                title,
                model,
            );
            Ok(serde_json::to_value(state).unwrap_or(serde_json::Value::Null))
        }
        "send_prompt" => {
            let text = get_str(args, "text");
            if text.trim().is_empty() {
                return Err("text пустой".into());
            }
            let session = args
                .get("sessionId")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string());
            crate::modules::opencode::send_prompt(app.clone(), text, session);
            Ok(serde_json::json!({ "accepted": true }))
        }
        "abort" => {
            let session_id = get_str(args, "sessionId");
            if session_id.is_empty() {
                let selected = {
                    let state = app.state::<SharedState>();
                    let s = state.0.lock().unwrap();
                    s.selected_session.clone()
                };
                match selected {
                    Some(t) => crate::modules::opencode::abort_session(t.port, &t.session_id)?,
                    None => return Err("сессия не выбрана".into()),
                }
            } else {
                let port = app
                    .state::<ConversationStore>()
                    .ports
                    .lock()
                    .unwrap()
                    .get(&session_id)
                    .copied()
                    .ok_or("порт сессии не найден")?;
                crate::modules::opencode::abort_session(port, &session_id)?;
            }
            Ok(serde_json::json!({ "aborted": true }))
        }
        "get_conversation" => {
            let session_id = get_str(args, "sessionId");
            if session_id.is_empty() {
                return Err("sessionId обязателен".into());
            }
            let port = args
                .get("port")
                .and_then(|x| x.as_u64())
                .map(|p| p as u16)
                .or_else(|| {
                    app.state::<ConversationStore>()
                        .ports
                        .lock()
                        .unwrap()
                        .get(&session_id)
                        .copied()
                });
            // Полная история из OpenCode (как в десктопе), иначе — в памяти.
            let msgs = match port {
                Some(p) => match crate::modules::opencode::fetch_session_history(p, &session_id) {
                    Ok(h) if !h.is_empty() => h,
                    _ => crate::modules::opencode::conversation_for(app, &session_id),
                },
                None => crate::modules::opencode::conversation_for(app, &session_id),
            };
            Ok(serde_json::to_value(msgs).unwrap_or(serde_json::Value::Array(vec![])))
        }
        "list_projects" => Ok(serde_json::to_value(crate::modules::opencode::list_projects())
            .unwrap_or(serde_json::Value::Array(vec![]))),
        "start_project" => {
            let worktree = get_str(args, "worktree");
            if worktree.is_empty() {
                return Err("worktree обязателен".into());
            }
            crate::modules::opencode::start_project(&worktree)?;
            Ok(serde_json::to_value(crate::modules::opencode::list_projects())
                .unwrap_or(serde_json::Value::Array(vec![])))
        }
        "stop_project" => {
            let worktree = get_str(args, "worktree");
            if worktree.is_empty() {
                return Err("worktree обязателен".into());
            }
            crate::modules::opencode::stop_project(&worktree)?;
            Ok(serde_json::to_value(crate::modules::opencode::list_projects())
                .unwrap_or(serde_json::Value::Array(vec![])))
        }
        "get_session_usage" => {
            let session_id = get_str(args, "sessionId");
            Ok(serde_json::to_value(
                crate::modules::opencode::fetch_session_usage(app, &session_id),
            )
            .unwrap_or(serde_json::Value::Null))
        }
        "reply_permission" => {
            let port = args.get("port").and_then(|x| x.as_u64()).unwrap_or(0) as u16;
            let request_id = get_str(args, "requestId");
            let reply = get_str(args, "reply");
            crate::modules::opencode::reply_permission(port, &request_id, &reply)?;
            Ok(serde_json::json!({ "ok": true }))
        }
        "reply_question" => {
            let port = args.get("port").and_then(|x| x.as_u64()).unwrap_or(0) as u16;
            let request_id = get_str(args, "requestId");
            let answers = args
                .get("answers")
                .and_then(|x| x.as_array())
                .map(|arr| {
                    arr.iter()
                        .map(|a| {
                            a.as_array()
                                .map(|inner| {
                                    inner
                                        .iter()
                                        .filter_map(|s| s.as_str().map(|x| x.to_string()))
                                        .collect::<Vec<String>>()
                                })
                                .unwrap_or_default()
                        })
                        .collect::<Vec<Vec<String>>>()
                })
                .unwrap_or_default();
            crate::modules::opencode::reply_question(port, &request_id, answers)?;
            Ok(serde_json::json!({ "ok": true }))
        }
        "reject_question" => {
            let port = args.get("port").and_then(|x| x.as_u64()).unwrap_or(0) as u16;
            let request_id = get_str(args, "requestId");
            crate::modules::opencode::reject_question(port, &request_id)?;
            Ok(serde_json::json!({ "ok": true }))
        }
        "register_device" => {
            let device_id = get_str(args, "deviceId");
            let device_name = get_str(args, "deviceName");
            if device_id.is_empty() {
                return Err("deviceId обязателен".into());
            }
            let devices =
                crate::commands::register_device(app.clone(), device_id, device_name);
            Ok(serde_json::to_value(devices).unwrap_or(serde_json::Value::Array(vec![])))
        }
        _ => Err(format!("неизвестная команда: {name}")),
    }
}

/// Информация для QR-кода (адрес + токен).
pub fn mobile_info(app: &AppHandle) -> MobileInfo {
    let (enabled, port, token) = {
        let state = app.state::<SharedState>();
        let s = state.0.lock().unwrap();
        (s.mobile_enabled, s.mobile_port, s.mobile_token.clone())
    };
    let ip = local_ip_address::local_ip()
        .map(|i| i.to_string())
        .unwrap_or_else(|_| "127.0.0.1".into());
    let uri = format!("ws://{ip}:{port}/?token={token}");
    let qr_svg = qr_svg(&uri);
    MobileInfo {
        enabled,
        port,
        ip,
        token,
        uri,
        qr_svg,
    }
}
