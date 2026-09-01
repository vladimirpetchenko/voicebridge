//! Команды настроек: модели, микрофон, язык, горячие клавиши, режим отправки.

use super::{current_state, emit_state, save_state};
use crate::state::{AppState, AppStatus, SharedState};
use tauri::{AppHandle, Manager};

#[tauri::command]
pub fn list_microphones() -> Vec<String> {
    crate::modules::audio::list_microphones()
}

#[tauri::command]
pub fn select_microphone(app: AppHandle, name: String) -> AppState {
    let state = app.state::<SharedState>();
    let mut s = state.0.lock().unwrap();
    // Пустая строка = «по умолчанию» — храним как None, иначе find_device
    // ищет устройство с пустым именем и падает с «микрофон не найден».
    s.selected_microphone = if name.is_empty() { None } else { Some(name) };
    let recording = s.status == AppStatus::Recording;
    let snapshot = s.clone();
    drop(s);
    emit_state(&app, &snapshot);
    save_state(&app, &snapshot);

    if recording {
        if let Err(e) = crate::modules::audio::start_capture(&app) {
            let st = app.state::<SharedState>();
            let mut s = st.0.lock().unwrap();
            s.status = AppStatus::Error;
            s.recording = false;
            s.status_message = format!("Ошибка микрофона: {e}");
            let snapshot = s.clone();
            drop(s);
            emit_state(&app, &snapshot);
        }
    }

    current_state(&app)
}

#[tauri::command]
pub fn set_sensitivity(app: AppHandle, level: f32) -> AppState {
    let state = app.state::<SharedState>();
    let mut s = state.0.lock().unwrap();
    s.sensitivity = level.clamp(0.1, 5.0);
    let recording = s.status == AppStatus::Recording;
    let snapshot = s.clone();
    drop(s);
    emit_state(&app, &snapshot);
    save_state(&app, &snapshot);

    if recording {
        let _ = crate::modules::audio::start_capture(&app);
    }

    current_state(&app)
}

#[tauri::command]
pub fn set_silence_timeout(app: AppHandle, seconds: f32) -> AppState {
    let state = app.state::<SharedState>();
    let mut s = state.0.lock().unwrap();
    s.silence_timeout = if seconds <= 0.0 { 0.0 } else { seconds.clamp(1.0, 30.0) };
    let snapshot = s.clone();
    drop(s);
    emit_state(&app, &snapshot);
    save_state(&app, &snapshot);
    current_state(&app)
}

#[tauri::command]
pub fn set_send_mode(app: AppHandle, mode: String) -> AppState {
    let state = app.state::<SharedState>();
    let mut s = state.0.lock().unwrap();
    s.send_mode = if mode == "confirm" { "confirm".into() } else { "direct".into() };
    let snapshot = s.clone();
    drop(s);
    emit_state(&app, &snapshot);
    save_state(&app, &snapshot);
    current_state(&app)
}

#[tauri::command]
pub fn set_hotkey(app: AppHandle, hotkey: String) -> Result<AppState, String> {
    let hotkey = hotkey.trim().to_string();
    if hotkey.is_empty() {
        return Err("комбинация не может быть пустой".into());
    }
    crate::hotkeys::apply_hotkey(&app, &hotkey)?;
    let state = app.state::<SharedState>();
    let mut s = state.0.lock().unwrap();
    s.hotkey = hotkey;
    let snapshot = s.clone();
    drop(s);
    emit_state(&app, &snapshot);
    save_state(&app, &snapshot);
    Ok(current_state(&app))
}

#[tauri::command]
pub fn get_models(app: AppHandle) -> Vec<crate::modules::stt::SttModelInfo> {
    crate::modules::stt::list_models(&app)
}

#[tauri::command]
pub fn download_model(app: AppHandle, model_id: String) {
    crate::modules::stt::download_model(app, model_id);
}

#[tauri::command]
pub fn select_stt_model(app: AppHandle, model_id: String) -> AppState {
    let state = app.state::<SharedState>();
    let mut s = state.0.lock().unwrap();
    s.selected_model = Some(model_id.clone());
    let snapshot = s.clone();
    drop(s);
    emit_state(&app, &snapshot);
    save_state(&app, &snapshot);

    crate::modules::stt::request_model_load(&app, &model_id);

    current_state(&app)
}

#[tauri::command]
pub fn set_language(app: AppHandle, language: String) -> AppState {
    let lang = match language.as_str() {
        "ru" | "en" | "auto" => language,
        _ => "auto".to_string(),
    };
    let state = app.state::<SharedState>();
    let mut s = state.0.lock().unwrap();
    s.language = lang;
    let snapshot = s.clone();
    drop(s);
    emit_state(&app, &snapshot);
    save_state(&app, &snapshot);
    current_state(&app)
}
