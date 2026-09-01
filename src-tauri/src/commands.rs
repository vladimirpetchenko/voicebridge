use crate::state::{AppState, AppStatus, SharedState};
use tauri::{AppHandle, Emitter, Manager};

fn current_state(app: &AppHandle) -> AppState {
    app.state::<SharedState>().0.lock().unwrap().clone()
}

pub fn emit_state(app: &AppHandle, state: &AppState) {
    let _ = app.emit("state-changed", state.clone());
    crate::modules::mobile::broadcast(
        app,
        "state-changed",
        serde_json::to_value(state).unwrap_or(serde_json::Value::Null),
    );
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

pub fn handle_toggle_recording(app: &AppHandle, session: Option<String>) {
    let is_recording = {
        let state = app.state::<SharedState>();
        let s = state.0.lock().unwrap();
        s.status == AppStatus::Recording
    };
    if is_recording {
        handle_stop_recording(app);
    } else {
        handle_start_recording(app, session);
    }
}

pub fn handle_start_recording(app: &AppHandle, session: Option<String>) {
    let state = app.state::<SharedState>();
    let mut s = state.0.lock().unwrap();
    if s.status == AppStatus::Recording {
        drop(s);
        return;
    }
    // Целевая сессия: из окна чата — явно, из хоткея/трея — выбранная сессия.
    let target = session.or_else(|| s.selected_session.as_ref().map(|t| t.session_id.clone()));
    s.status = AppStatus::Recording;
    s.recording = true;
    s.status_message = "Запись…".into();
    s.transcript.clear();
    s.response.clear();
    s.recording_session_id = target;
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
    let (send_mode, target_session) = {
        let state = app.state::<SharedState>();
        let s = state.0.lock().unwrap();
        let target = s
            .recording_session_id
            .clone()
            .or_else(|| s.selected_session.as_ref().map(|t| t.session_id.clone()));
        (s.send_mode.clone(), target)
    };

    let state = app.state::<SharedState>();
    let mut s = state.0.lock().unwrap();
    s.transcript = text.clone();
    s.recording_session_id = target_session.clone();
    s.response.clear();
    let snapshot = s.clone();
    drop(s);
    emit_state(app, &snapshot);
    save_state(app, &snapshot);

    // В режиме предпроверки текст остаётся в поле ввода — отправку делает пользователь.
    if send_mode == "direct" {
        crate::modules::opencode::send_prompt(app.clone(), text, target_session);
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

/// Извлекает id сессии из метки окна чата (`response-{sessionId}`).
fn session_id_from_window(window: &tauri::WebviewWindow) -> Option<String> {
    window
        .label()
        .strip_prefix("response-")
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
}

#[tauri::command]
pub fn toggle_recording(app: AppHandle, window: tauri::WebviewWindow) -> AppState {
    let session = session_id_from_window(&window);
    handle_toggle_recording(&app, session);
    current_state(&app)
}

#[tauri::command]
pub fn start_recording(app: AppHandle, window: tauri::WebviewWindow) -> AppState {
    let session = session_id_from_window(&window);
    handle_start_recording(&app, session);
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

/// Отправка набранного вручную текста в сессию окна чата (не зависит от режима).
#[tauri::command]
pub fn send_text(app: AppHandle, window: tauri::WebviewWindow, text: String) {
    let session = session_id_from_window(&window);
    crate::modules::opencode::send_prompt(app, text, session);
}

#[tauri::command]
pub async fn get_session_info(
    window: tauri::WebviewWindow,
    app: AppHandle,
) -> crate::state::SessionInfo {
    let session_id = window
        .label()
        .strip_prefix("response-")
        .unwrap_or_default()
        .to_string();
    let store = app.state::<crate::state::ConversationStore>();
    let title = store
        .titles
        .lock()
        .unwrap()
        .get(&session_id)
        .cloned()
        .unwrap_or_default();
    let project = store
        .projects
        .lock()
        .unwrap()
        .get(&session_id)
        .cloned()
        .unwrap_or_default();
    crate::state::SessionInfo { title, project }
}

#[tauri::command]
pub async fn get_session_usage(
    window: tauri::WebviewWindow,
    app: AppHandle,
) -> Option<crate::state::SessionUsage> {
    let session_id = window
        .label()
        .strip_prefix("response-")
        .unwrap_or_default()
        .to_string();
    if session_id.is_empty() {
        return None;
    }
    tauri::async_runtime::spawn_blocking(move || {
        crate::modules::opencode::fetch_session_usage(&app, &session_id)
    })
    .await
    .unwrap_or(None)
}

#[tauri::command]
pub fn abort_session(window: tauri::WebviewWindow, app: AppHandle) -> Result<(), String> {
    let session_id = window
        .label()
        .strip_prefix("response-")
        .unwrap_or_default();
    if session_id.is_empty() {
        return Err("сессия не определена".into());
    }
    let port = app
        .state::<crate::state::ConversationStore>()
        .ports
        .lock()
        .unwrap()
        .get(session_id)
        .copied()
        .ok_or("порт сессии не найден")?;
    crate::modules::opencode::abort_session(port, session_id)
}

#[tauri::command]
pub fn get_opencode_binary() -> String {
    crate::modules::opencode::opencode_binary()
}

#[tauri::command]
pub fn reply_permission(port: u16, request_id: String, reply: String) -> Result<(), String> {
    crate::modules::opencode::reply_permission(port, &request_id, &reply)
}

#[tauri::command]
pub fn reply_question(
    port: u16,
    request_id: String,
    answers: Vec<Vec<String>>,
) -> Result<(), String> {
    crate::modules::opencode::reply_question(port, &request_id, answers)
}

#[tauri::command]
pub fn reject_question(port: u16, request_id: String) -> Result<(), String> {
    crate::modules::opencode::reject_question(port, &request_id)
}

#[tauri::command]
pub async fn list_opencode_sessions() -> Vec<crate::state::OpenCodeInstance> {
    // Обнаружение экземпляров опрашивает порты и запускает подпроцессы —
    // выполняем в фоновом потоке, чтобы не блокировать UI.
    tauri::async_runtime::spawn_blocking(crate::modules::opencode::discover_instances)
        .await
        .unwrap_or_default()
}

#[tauri::command]
pub fn select_opencode_session(
    app: AppHandle,
    instance_id: String,
    port: u16,
    session_id: String,
    title: String,
    model: String,
) -> AppState {
    // instance_id — это папка проекта (см. discover_instances); имя проекта —
    // её basename.
    let project = std::path::Path::new(&instance_id)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let state = app.state::<SharedState>();
    let mut s = state.0.lock().unwrap();
    s.selected_session = Some(crate::state::OpenCodeTarget {
        instance_id: instance_id.clone(),
        port,
        session_id: session_id.clone(),
        title: title.clone(),
    });
    s.opencode_model = if model.is_empty() {
        None
    } else {
        Some(model)
    };
    crate::modules::opencode::remember_session_port(&app, &session_id, port);
    crate::modules::opencode::remember_session_title(&app, &session_id, &title);
    crate::modules::opencode::remember_session_project(&app, &session_id, &project);
    crate::modules::opencode::remember_session_directory(&app, &session_id, &instance_id);
    s.active_instance = Some(crate::state::OpenCodeInstanceRef {
        id: instance_id,
        port,
        name: String::new(),
    });
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
pub async fn list_projects() -> Vec<crate::modules::opencode::Project> {
    // Чтение БД opencode и проверка портов тяжёлые — не блокируем UI.
    tauri::async_runtime::spawn_blocking(crate::modules::opencode::list_projects)
        .await
        .unwrap_or_default()
}

/// Создаёт сессию и делает её выбранной. Синхронная часть (HTTP-вызов) —
/// вызывать из `spawn_blocking`. Общая для десктопа и мобильного моста.
pub fn create_session_inner(
    app: &AppHandle,
    port: u16,
    worktree: &str,
    title: &str,
) -> Result<AppState, String> {
    let session = crate::modules::opencode::create_session(port, title)?;

    let instance_id = worktree.to_string();
    let project = std::path::Path::new(worktree)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let state = app.state::<SharedState>();
    let mut s = state.0.lock().unwrap();
    s.selected_session = Some(crate::state::OpenCodeTarget {
        instance_id: instance_id.clone(),
        port,
        session_id: session.id.clone(),
        title: session.title.clone(),
    });
    s.opencode_model = if session.model.is_empty() {
        None
    } else {
        Some(session.model.clone())
    };
    s.active_instance = Some(crate::state::OpenCodeInstanceRef {
        id: instance_id,
        port,
        name: project.clone(),
    });
    crate::modules::opencode::remember_session_port(app, &session.id, port);
    crate::modules::opencode::remember_session_title(app, &session.id, &session.title);
    crate::modules::opencode::remember_session_project(app, &session.id, &project);
    crate::modules::opencode::remember_session_directory(app, &session.id, &worktree);
    s.response.clear();
    s.status = AppStatus::Idle;
    s.status_message = "Готов к работе".into();
    let snapshot = s.clone();
    drop(s);
    emit_state(app, &snapshot);
    save_state(app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub async fn create_session(
    app: AppHandle,
    port: u16,
    worktree: String,
    title: String,
) -> Result<AppState, String> {
    tauri::async_runtime::spawn_blocking(move || {
        create_session_inner(&app, port, &worktree, &title)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn start_project(worktree: String) -> Result<Vec<crate::modules::opencode::Project>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::modules::opencode::start_project(&worktree)?;
        Ok(crate::modules::opencode::list_projects())
    })
    .await
    .unwrap_or_else(|e| Err(e.to_string()))
}

#[tauri::command]
pub async fn stop_project(worktree: String) -> Result<Vec<crate::modules::opencode::Project>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::modules::opencode::stop_project(&worktree)?;
        Ok(crate::modules::opencode::list_projects())
    })
    .await
    .unwrap_or_else(|e| Err(e.to_string()))
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

fn broadcast_devices(app: &AppHandle, devices: &[crate::state::KnownDevice]) {
    let _ = app.emit("devices-changed", devices);
    crate::modules::mobile::broadcast(
        app,
        "devices-changed",
        serde_json::to_value(devices).unwrap_or(serde_json::Value::Array(vec![])),
    );
}

/// Регистрирует (или обновляет) мобильное устройство. Вызывается из мобильного
/// моста (WS) при подключении.
pub fn register_device(app: AppHandle, device_id: String, device_name: String) -> Vec<crate::state::KnownDevice> {
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
        s.known_devices.push(crate::state::KnownDevice {
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
pub fn list_devices(app: AppHandle) -> Vec<crate::state::KnownDevice> {
    current_state(&app).known_devices
}

/// Удаляет устройство из списка известных.
#[tauri::command]
pub fn forget_device(app: AppHandle, device_id: String) -> Vec<crate::state::KnownDevice> {
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

/// Проверяет наличие обновления. Возвращает версию, если доступна новая.
#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_updater::UpdaterExt;
    let updater = app
        .updater()
        .map_err(|e| format!("модуль обновлений недоступен: {e}"))?;
    match updater.check().await {
        Ok(Some(update)) => Ok(Some(update.version)),
        Ok(None) => Ok(None),
        Err(e) => Err(format!("не удалось проверить обновления: {e}")),
    }
}

#[tauri::command]
pub async fn open_response_window(app: AppHandle, session_id: String, title: String, port: u16) {
    crate::modules::opencode::remember_session_port(&app, &session_id, port);
    crate::modules::opencode::remember_session_title(&app, &session_id, &title);

    let label = format!("response-{session_id}");

    if let Some(window) = app.get_webview_window(&label) {
        let _ = window.show();
        let _ = window.set_focus();
    } else {
        use tauri::{WebviewUrl, WebviewWindowBuilder};
        // Создание окна должно быть асинхронным: `build()` на Windows шлёт
        // создание webview в главный поток и ждёт ответа. В синхронной команде
        // это приводит к deadlock (главный поток ждёт сам себя).
        let builder = WebviewWindowBuilder::new(
            &app,
            &label,
            WebviewUrl::App(std::path::PathBuf::from("index.html")),
        )
        .title(format!("OpenCode · {title}"))
        .inner_size(960.0, 820.0)
        .min_inner_size(480.0, 400.0);
        match builder.build() {
            Ok(_window) => {
                log::info!("opened response window {label}");
            }
            Err(e) => {
                log::error!("failed to open response window {label}: {e}");
            }
        }
    }

    crate::modules::opencode::mark_session_open(&app, &session_id);
}

#[tauri::command]
pub fn list_open_session_ids(app: AppHandle) -> Vec<String> {
    crate::modules::opencode::open_session_ids(&app)
}

#[tauri::command]
pub async fn get_conversation(
    window: tauri::WebviewWindow,
    app: AppHandle,
) -> Vec<crate::state::ConversationMessage> {
    let session_id = window
        .label()
        .strip_prefix("response-")
        .unwrap_or_default()
        .to_string();

    // Подтягивание истории делает HTTP-запрос — не блокируем главный поток.
    tauri::async_runtime::spawn_blocking(move || {
        let port = {
            let store = app.state::<crate::state::ConversationStore>();
            let ports = store.ports.lock().unwrap();
            ports.get(&session_id).copied()
        };

        if let Some(port) = port {
            if let Ok(history) = crate::modules::opencode::fetch_session_history(port, &session_id)
            {
                if !history.is_empty() {
                    return history;
                }
            }
        }

        crate::modules::opencode::conversation_for(&app, &session_id)
    })
    .await
    .unwrap_or_default()
}

#[tauri::command]
pub fn close_response_window(window: tauri::WebviewWindow) {
    let _ = window.close();
}

/// Список Git-изменений проекта текущей сессии (окно чата).
#[tauri::command]
pub async fn get_git_changes(
    window: tauri::WebviewWindow,
    app: AppHandle,
) -> Vec<crate::modules::git::GitFileChange> {
    let session_id = window
        .label()
        .strip_prefix("response-")
        .unwrap_or_default()
        .to_string();
    if session_id.is_empty() {
        return Vec::new();
    }
    // `git status`/`git diff` — подпроцессы, выполняем в фоне.
    tauri::async_runtime::spawn_blocking(move || {
        crate::modules::opencode::session_directory(&app, Some(&session_id))
            .map(|dir| crate::modules::git::changes(&dir))
            .unwrap_or_default()
    })
    .await
    .unwrap_or_default()
}

/// Дифф конкретного файла в проекте текущей сессии (окно чата).
#[tauri::command]
pub async fn get_git_diff(
    window: tauri::WebviewWindow,
    app: AppHandle,
    path: String,
) -> crate::modules::git::GitDiff {
    let session_id = window
        .label()
        .strip_prefix("response-")
        .unwrap_or_default()
        .to_string();
    if session_id.is_empty() {
        return crate::modules::git::GitDiff {
            path,
            status: "modified".into(),
            too_large: false,
            diff: String::new(),
        };
    }
    let path_for_fallback = path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        match crate::modules::opencode::session_directory(&app, Some(&session_id)) {
            Some(dir) => crate::modules::git::diff(&dir, &path),
            None => crate::modules::git::GitDiff {
                path,
                status: "modified".into(),
                too_large: false,
                diff: String::new(),
            },
        }
    })
    .await
    .unwrap_or_else(|_| crate::modules::git::GitDiff {
        path: path_for_fallback,
        status: "modified".into(),
        too_large: false,
        diff: String::new(),
    })
}
