mod commands;
mod hotkeys;
mod logging;
pub mod modules;
mod state;
mod tray;

use tauri::Manager;

/// Создаёт `Command` для запуска консольной утилиты без всплывающего окна
/// консоли. Приложение — GUI-подсистема Windows, поэтому каждый запуск
/// `cmd`/`git`/`netstat`/`tasklist`/`taskkill`/`opencode.cmd` без
/// `CREATE_NO_WINDOW` открывает видимое окно терминала (раз в пару секунд —
/// при опросе проектов/сессий/Git). На других ОС — обычный `Command::new`.
#[cfg(target_os = "windows")]
pub(crate) fn no_console_command(program: &str) -> std::process::Command {
    use std::os::windows::process::CommandExt;
    let mut cmd = std::process::Command::new(program);
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    cmd
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn no_console_command(program: &str) -> std::process::Command {
    std::process::Command::new(program)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let handle = app.handle().clone();

            // Логирование в файл (важно на Windows — там нет консоли).
            let log_dir = handle
                .path()
                .app_log_dir()
                .or_else(|_| handle.path().app_data_dir())
                .unwrap_or_else(|_| std::env::temp_dir());
            let _ = std::fs::create_dir_all(&log_dir);
            logging::init(log_dir.join("voicebridge.log"));
            log::info!("VoiceBridge starting (log: {:?})", log_dir);

            handle.manage(state::SharedState::default());
            handle.manage(state::ConversationStore::default());
            handle.manage(state::PendingUpdate::default());
            handle.manage(modules::audio::AudioEngine::default());
            handle.manage(modules::stt::spawn(handle.clone()));
            handle.manage(modules::mobile::MobileServer::default());

            // Восстанавливаем сохранённое состояние после перезапуска.
            // Рантайм-поля (статус, запись, текст) сбрасываются.
            if let Some(loaded) = commands::load_state(&handle) {
                let st = handle.state::<state::SharedState>();
                let mut s = st.0.lock().unwrap();
                s.sensitivity = loaded.sensitivity;
                s.silence_timeout = loaded.silence_timeout;
                s.send_mode = loaded.send_mode;
                s.hotkey = loaded.hotkey;
                s.language = loaded.language;
                s.selected_model = loaded.selected_model;
                s.selected_microphone = loaded.selected_microphone;
                s.selected_session = loaded.selected_session;
                s.opencode_model = loaded.opencode_model;
                s.mobile_enabled = loaded.mobile_enabled;
                s.mobile_port = loaded.mobile_port;
                s.mobile_token = loaded.mobile_token;
                s.known_devices = loaded.known_devices;
                s.hidden_projects = loaded.hidden_projects;
                s.known_worktrees = loaded.known_worktrees;
            }

            // Токен мобильного доступа генерируется один раз и сохраняется.
            {
                let st = handle.state::<state::SharedState>();
                let mut s = st.0.lock().unwrap();
                if s.mobile_token.is_empty() {
                    s.mobile_token = modules::mobile::generate_token();
                }
                let snapshot = s.clone();
                drop(s);
                commands::save_state(&handle, &snapshot);
            }

            tray::setup(&handle)?;
            hotkeys::setup(&handle)?;

            // Встроенный WebSocket-сервер мобильного доступа.
            tauri::async_runtime::spawn(modules::mobile::serve(handle.clone()));

            let snapshot = handle
                .state::<state::SharedState>()
                .0
                .lock()
                .unwrap()
                .clone();
            commands::emit_state(&handle, &snapshot);

            // Модель по умолчанию (Base) + автоскачивание при первом запуске.
            modules::stt::ensure_default_model(&handle);

            Ok(())
        })
        .on_menu_event(tray::handle_menu_event)
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
            if let tauri::WindowEvent::Destroyed = event {
                if let Some(session_id) = window.label().strip_prefix("response-") {
                    crate::modules::opencode::mark_session_closed(
                        window.app_handle(),
                        session_id,
                    );
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_state,
            commands::toggle_recording,
            commands::start_recording,
            commands::stop_recording,
            commands::list_microphones,
            commands::select_microphone,
            commands::set_sensitivity,
            commands::set_silence_timeout,
            commands::set_send_mode,
            commands::set_hotkey,
            commands::send_text,
            commands::get_session_info,
            commands::get_opencode_binary,
            commands::reply_permission,
            commands::reply_question,
            commands::reject_question,
            commands::list_opencode_sessions,
            commands::select_opencode_session,
            commands::select_opencode_instance,
            commands::list_projects,
            commands::create_session,
            commands::delete_session,
            commands::start_project,
            commands::stop_project,
            commands::get_models,
            commands::download_model,
            commands::select_stt_model,
            commands::set_language,
            commands::open_response_window,
            commands::get_conversation,
            commands::get_session_info,
            commands::get_session_usage,
            commands::abort_session,
            commands::close_response_window,
            commands::list_open_session_ids,
            commands::check_update,
            commands::install_update,
            commands::get_mobile_info,
            commands::set_mobile_enabled,
            commands::regenerate_mobile_token,
            commands::list_devices,
            commands::forget_device,
            commands::hide_project,
            commands::unhide_project,
            commands::get_git_changes,
            commands::get_git_diff,
            commands::get_git_commits,
            commands::get_git_commit,
            commands::get_git_branches,
            commands::quit_app
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
