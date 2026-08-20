use crate::state::{AppMode, AppState, AppStatus, SharedState};
use tauri::{AppHandle, Emitter, Manager};

fn current_state(app: &AppHandle) -> AppState {
    app.state::<SharedState>().0.lock().unwrap().clone()
}

pub fn emit_state(app: &AppHandle, state: &AppState) {
    let _ = app.emit("state-changed", state.clone());
    if let Some(tray) = app.tray_by_id("main-tray") {
        let text = match state.status {
            AppStatus::Idle => "VoiceBridge — ожидание",
            AppStatus::Recording => "VoiceBridge — запись…",
            AppStatus::Processing => "VoiceBridge — обработка…",
            AppStatus::Error => "VoiceBridge — ошибка",
        };
        let _ = tray.set_tooltip(Some(text));
    }
}

pub fn save_state(app: &AppHandle, state: &AppState) {
    let Ok(dir) = app.path().app_data_dir() else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("state.json");
    if let Ok(json) = serde_json::to_string_pretty(state) {
        let _ = std::fs::write(path, json);
    }
}

pub fn load_state(app: &AppHandle) -> Option<AppState> {
    let dir = app.path().app_data_dir().ok()?;
    let path = dir.join("state.json");
    let data = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

pub fn handle_toggle_recording(app: &AppHandle) {
    let is_recording = {
        let state = app.state::<SharedState>();
        let s = state.0.lock().unwrap();
        s.status == AppStatus::Recording
    };
    if is_recording {
        handle_stop_recording(app);
    } else {
        handle_start_recording(app);
    }
}

pub fn handle_start_recording(app: &AppHandle) {
    let state = app.state::<SharedState>();
    let mut s = state.0.lock().unwrap();
    if s.status == AppStatus::Recording {
        drop(s);
        return;
    }
    s.status = AppStatus::Recording;
    s.recording = true;
    s.status_message = "Запись…".into();
    s.transcript.clear();
    s.response.clear();
    let snapshot = s.clone();
    drop(s);
    emit_state(app, &snapshot);

    if let Err(e) = crate::modules::audio::start_capture(app) {
        let st = app.state::<SharedState>();
        let mut s = st.0.lock().unwrap();
        s.status = AppStatus::Error;
        s.recording = false;
        s.status_message = format!("Ошибка микрофона: {e}");
        let snapshot = s.clone();
        drop(s);
        emit_state(app, &snapshot);
        crate::modules::audio::stop_capture(app);
    }

    let snapshot = app.state::<SharedState>().0.lock().unwrap().clone();
    save_state(app, &snapshot);
}

pub fn handle_stop_recording(app: &AppHandle) {
    crate::modules::audio::stop_capture(app);
    let audio = crate::modules::audio::take_audio(app);

    let state = app.state::<SharedState>();
    let mut s = state.0.lock().unwrap();
    if s.status != AppStatus::Recording {
        drop(s);
        return;
    }
    s.status = AppStatus::Processing;
    s.recording = false;
    s.status_message = "Распознавание речи…".into();
    let snapshot = s.clone();
    drop(s);
    emit_state(app, &snapshot);
    save_state(app, &snapshot);

    match audio {
        Some((samples, rate, channels)) => {
            let mono = crate::modules::audio::resample_to_16k_mono(&samples, rate, channels);
            let duration = mono.len() as f32 / 16000.0;
            if duration < 0.4 {
                fail_transcription(app, "фраза слишком короткая".into());
                return;
            }
            crate::modules::stt::transcribe_async(app, mono);
        }
        None => fail_transcription(app, "нет аудио для распознавания".into()),
    }
}

/// Устанавливает распознанный текст и запускает отправку в OpenCode.
/// Если сессия не выбрана — она будет создана автоматически.
pub fn finish_transcription(app: &AppHandle, text: String) {
    let mode = {
        let state = app.state::<SharedState>();
        let s = state.0.lock().unwrap();
        s.mode
    };

    let state = app.state::<SharedState>();
    let mut s = state.0.lock().unwrap();
    s.transcript = text.clone();
    s.response.clear();
    let snapshot = s.clone();
    drop(s);
    emit_state(app, &snapshot);
    save_state(app, &snapshot);

    if mode == AppMode::OpenCode {
        crate::modules::opencode::send_prompt(app.clone(), text);
    }
}

/// Устанавливает статус ошибки распознавания.
pub fn fail_transcription(app: &AppHandle, error: String) {
    log::error!("transcription failed: {error}");
    let state = app.state::<SharedState>();
    let mut s = state.0.lock().unwrap();
    s.status = AppStatus::Error;
    s.status_message = format!("Ошибка распознавания: {error}");
    let snapshot = s.clone();
    drop(s);
    emit_state(app, &snapshot);
    save_state(app, &snapshot);
}

#[tauri::command]
pub fn get_app_state(app: AppHandle) -> AppState {
    current_state(&app)
}

#[tauri::command]
pub fn set_mode(app: AppHandle, mode: String) -> AppState {
    let mode = match mode.as_str() {
        "gui" => AppMode::Gui,
        _ => AppMode::OpenCode,
    };
    let state = app.state::<SharedState>();
    let mut s = state.0.lock().unwrap();
    if s.mode != mode {
        s.mode = mode;
        let snapshot = s.clone();
        drop(s);
        emit_state(&app, &snapshot);
        save_state(&app, &snapshot);
    }
    current_state(&app)
}

#[tauri::command]
pub fn toggle_recording(app: AppHandle) -> AppState {
    handle_toggle_recording(&app);
    current_state(&app)
}

#[tauri::command]
pub fn start_recording(app: AppHandle) -> AppState {
    handle_start_recording(&app);
    current_state(&app)
}

#[tauri::command]
pub fn stop_recording(app: AppHandle) -> AppState {
    handle_stop_recording(&app);
    current_state(&app)
}

#[tauri::command]
pub fn list_microphones() -> Vec<String> {
    crate::modules::audio::list_microphones()
}

#[tauri::command]
pub fn select_microphone(app: AppHandle, name: String) -> AppState {
    let state = app.state::<SharedState>();
    let mut s = state.0.lock().unwrap();
    s.selected_microphone = Some(name);
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
pub fn list_opencode_sessions() -> Vec<crate::state::OpenCodeInstance> {
    crate::modules::opencode::discover_instances()
}

#[tauri::command]
pub fn select_opencode_session(
    app: AppHandle,
    instance_id: String,
    port: u16,
    session_id: String,
    title: String,
) -> AppState {
    let state = app.state::<SharedState>();
    let mut s = state.0.lock().unwrap();
    s.selected_session = Some(crate::state::OpenCodeTarget {
        instance_id: instance_id.clone(),
        port,
        session_id: session_id.clone(),
        title,
    });
    s.active_instance = Some(crate::state::OpenCodeInstanceRef {
        id: instance_id,
        port,
        name: String::new(),
    });
    crate::modules::opencode::remember_session_port(&app, &session_id, port);
    // Показываем последний ответ этой сессии и сбрасываем статус.
    s.response = crate::modules::opencode::latest_assistant_response(&app, &session_id);
    s.status = AppStatus::Idle;
    s.status_message = "Готов к работе".into();
    let snapshot = s.clone();
    drop(s);
    emit_state(&app, &snapshot);
    save_state(&app, &snapshot);
    current_state(&app)
}

#[tauri::command]
pub fn select_opencode_instance(app: AppHandle, id: String, port: u16, name: String) -> AppState {
    let state = app.state::<SharedState>();
    let mut s = state.0.lock().unwrap();
    s.active_instance = Some(crate::state::OpenCodeInstanceRef {
        id,
        port,
        name,
    });
    // При переключении на другой инстанс сбрасываем сессию, ответ и статус.
    if s.selected_session.as_ref().map(|t| t.port) != Some(port) {
        s.selected_session = None;
        s.response.clear();
        s.status = AppStatus::Idle;
        s.status_message = "Готов к работе".into();
    }
    let snapshot = s.clone();
    drop(s);
    emit_state(&app, &snapshot);
    save_state(&app, &snapshot);
    current_state(&app)
}

#[tauri::command]
pub fn list_projects() -> Vec<crate::modules::opencode::Project> {
    crate::modules::opencode::list_projects()
}

#[tauri::command]
pub fn start_project(worktree: String) -> Result<Vec<crate::modules::opencode::Project>, String> {
    crate::modules::opencode::start_project(&worktree)?;
    Ok(crate::modules::opencode::list_projects())
}

#[tauri::command]
pub fn stop_project(worktree: String) -> Result<Vec<crate::modules::opencode::Project>, String> {
    crate::modules::opencode::stop_project(&worktree)?;
    Ok(crate::modules::opencode::list_projects())
}

#[tauri::command]
pub fn list_windows() -> Vec<crate::state::WindowInfo> {
    crate::modules::automation::list_windows()
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

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
pub fn open_response_window(app: AppHandle, session_id: String, title: String, port: u16) {
    crate::modules::opencode::remember_session_port(&app, &session_id, port);

    let label = format!("response-{session_id}");

    if let Some(window) = app.get_webview_window(&label) {
        let _ = window.show();
        let _ = window.set_focus();
    } else {
        use tauri::{WebviewUrl, WebviewWindowBuilder};
        let _ = WebviewWindowBuilder::new(
            &app,
            &label,
            WebviewUrl::App(std::path::PathBuf::from("index.html")),
        )
        .title(format!("OpenCode · {title}"))
        .inner_size(960.0, 820.0)
        .min_inner_size(480.0, 400.0)
        .build();
    }
}

#[tauri::command]
pub fn get_conversation(
    window: tauri::WebviewWindow,
    app: AppHandle,
) -> Vec<crate::state::ConversationMessage> {
    let session_id = window
        .label()
        .strip_prefix("response-")
        .unwrap_or_default()
        .to_string();

    let port = {
        let store = app.state::<crate::state::ConversationStore>();
        let ports = store.ports.lock().unwrap();
        ports.get(&session_id).copied()
    };

    if let Some(port) = port {
        if let Ok(history) = crate::modules::opencode::fetch_session_history(port, &session_id) {
            if !history.is_empty() {
                return history;
            }
        }
    }

    crate::modules::opencode::conversation_for(&app, &session_id)
}

#[tauri::command]
pub fn close_response_window(window: tauri::WebviewWindow) {
    let _ = window.close();
}
