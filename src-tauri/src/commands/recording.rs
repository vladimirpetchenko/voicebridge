//! Запись и распознавание речи.

use super::{current_state, emit_state, save_state, session_id_from_window};
use crate::state::{AppState, AppStatus, SharedState};
use tauri::{AppHandle, Manager};

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
            // Диагностика: пик амплитуды. Если ~0 — микрофон ловит тишину
            // (права macOS / не тот вход), а не проблему whisper.
            let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            log::info!(
                "audio captured: {} samples, rate={rate}, channels={channels}, peak={peak:.5}",
                samples.len()
            );
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
    // whisper отдаёт спец-токен `[BLANK_AUDIO]`, когда речи не было — не считаем
    // это текстом и не шлём его в OpenCode.
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed == "[BLANK_AUDIO]" {
        let state = app.state::<SharedState>();
        let mut s = state.0.lock().unwrap();
        s.status = AppStatus::Idle;
        s.status_message = "Ничего не распознано — проверьте микрофон".into();
        s.transcript.clear();
        s.response.clear();
        let snapshot = s.clone();
        drop(s);
        emit_state(app, &snapshot);
        save_state(app, &snapshot);
        return;
    }

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
    // В режиме предпроверки текст остаётся в поле ввода — отправку делает
    // пользователь; статус возвращаем в idle, иначе «Распознавание…» зависнет.
    if send_mode != "direct" {
        s.status = AppStatus::Idle;
        s.status_message = "Готов к работе".into();
    }
    let snapshot = s.clone();
    drop(s);
    emit_state(app, &snapshot);
    save_state(app, &snapshot);

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
