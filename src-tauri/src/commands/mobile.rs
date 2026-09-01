//! Команды мобильного доступа, устройств и скрытых проектов.

use super::{current_state, emit_state, save_state};
use crate::state::{AppState, KnownDevice, SharedState};
use tauri::{AppHandle, Emitter, Manager};

#[tauri::command]
pub fn get_mobile_info(app: AppHandle) -> crate::modules::mobile::MobileInfo {
    crate::modules::mobile::mobile_info(&app)
}

#[tauri::command]
pub fn set_mobile_enabled(app: AppHandle, enabled: bool) -> AppState {
    let state = app.state::<SharedState>();
    let mut s = state.0.lock().unwrap();
    s.mobile_enabled = enabled;
    let snapshot = s.clone();
    drop(s);
    emit_state(&app, &snapshot);
    save_state(&app, &snapshot);
    current_state(&app)
}

#[tauri::command]
pub fn regenerate_mobile_token(app: AppHandle) -> AppState {
    let state = app.state::<SharedState>();
    let mut s = state.0.lock().unwrap();
    s.mobile_token = crate::modules::mobile::generate_token();
    let snapshot = s.clone();
    drop(s);
    emit_state(&app, &snapshot);
    save_state(&app, &snapshot);
    current_state(&app)
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn broadcast_devices(app: &AppHandle, devices: &[KnownDevice]) {
    let _ = app.emit("devices-changed", devices);
    crate::modules::mobile::broadcast(
        app,
        "devices-changed",
        serde_json::to_value(devices).unwrap_or(serde_json::Value::Array(vec![])),
    );
}

/// Регистрирует (или обновляет) мобильное устройство. Вызывается из мобильного
/// моста (WS) при подключении.
pub fn register_device(
    app: AppHandle,
    device_id: String,
    device_name: String,
) -> Vec<KnownDevice> {
    let state = app.state::<SharedState>();
    let mut s = state.0.lock().unwrap();
    let name = if device_name.trim().is_empty() {
        "Мобильное устройство".to_string()
    } else {
        device_name.trim().to_string()
    };
    let now = now_secs();
    if let Some(d) = s.known_devices.iter_mut().find(|d| d.id == device_id) {
        d.name = name;
        d.last_seen = now;
    } else {
        s.known_devices.push(KnownDevice {
            id: device_id,
            name,
            last_seen: now,
        });
    }
    let snapshot = s.clone();
    drop(s);
    save_state(&app, &snapshot);
    broadcast_devices(&app, &snapshot.known_devices);
    snapshot.known_devices
}

/// Список известных (сохранённых) мобильных устройств.
#[tauri::command]
pub fn list_devices(app: AppHandle) -> Vec<KnownDevice> {
    current_state(&app).known_devices
}

/// Удаляет устройство из списка известных.
#[tauri::command]
pub fn forget_device(app: AppHandle, device_id: String) -> Vec<KnownDevice> {
    let state = app.state::<SharedState>();
    let mut s = state.0.lock().unwrap();
    s.known_devices.retain(|d| d.id != device_id);
    let snapshot = s.clone();
    drop(s);
    save_state(&app, &snapshot);
    broadcast_devices(&app, &snapshot.known_devices);
    snapshot.known_devices
}

/// Скрывает проект из лаунчера (не удаляя папку).
#[tauri::command]
pub fn hide_project(app: AppHandle, worktree: String) -> AppState {
    let state = app.state::<SharedState>();
    let mut s = state.0.lock().unwrap();
    if !s.hidden_projects.contains(&worktree) {
        s.hidden_projects.push(worktree);
    }
    let snapshot = s.clone();
    drop(s);
    emit_state(&app, &snapshot);
    save_state(&app, &snapshot);
    snapshot
}

/// Возвращает скрытый проект обратно в лаунчер.
#[tauri::command]
pub fn unhide_project(app: AppHandle, worktree: String) -> AppState {
    let state = app.state::<SharedState>();
    let mut s = state.0.lock().unwrap();
    s.hidden_projects.retain(|w| w != &worktree);
    let snapshot = s.clone();
    drop(s);
    emit_state(&app, &snapshot);
    save_state(&app, &snapshot);
    snapshot
}
