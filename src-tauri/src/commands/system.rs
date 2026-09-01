//! Команды выхода и обновлений.

use tauri::AppHandle;

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
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
