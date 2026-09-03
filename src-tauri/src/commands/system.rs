//! Команды выхода и обновлений.

use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::UpdaterExt;

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

/// Проверяет наличие обновления. Если доступна новая версия — сохраняет её в
/// `PendingUpdate` (для последующей установки) и возвращает версию.
#[tauri::command]
pub async fn check_update(
    app: AppHandle,
    pending: tauri::State<'_, crate::state::PendingUpdate>,
) -> Result<Option<String>, String> {
    let updater = app
        .updater()
        .map_err(|e| format!("модуль обновлений недоступен: {e}"))?;
    match updater.check().await {
        Ok(Some(update)) => {
            let version = update.version.clone();
            *pending.0.lock().unwrap() = Some(update);
            Ok(Some(version))
        }
        Ok(None) => Ok(None),
        Err(e) => Err(format!("не удалось проверить обновления: {e}")),
    }
}

/// Скачивает и устанавливает найденное обновление, затем перезапускает
/// приложение. Прогресс загрузки шлётся событием `update-download-progress`,
/// завершение — `update-download-finished`.
#[tauri::command]
pub async fn install_update(
    app: AppHandle,
    pending: tauri::State<'_, crate::state::PendingUpdate>,
) -> Result<(), String> {
    let update = pending
        .0
        .lock()
        .unwrap()
        .take()
        .ok_or("нет доступного обновления")?;

    let progress_app = app.clone();
    let finish_app = app.clone();
    update
        .download_and_install(
            move |chunk_length, content_length| {
                let _ = progress_app.emit(
                    "update-download-progress",
                    serde_json::json!({ "downloaded": chunk_length, "total": content_length }),
                );
            },
            move || {
                let _ = finish_app.emit("update-download-finished", ());
            },
        )
        .await
        .map_err(|e| format!("не удалось установить обновление: {e}"))?;

    app.restart()
}
