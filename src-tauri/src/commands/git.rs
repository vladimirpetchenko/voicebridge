//! Команды Git-изменений проекта.

use tauri::AppHandle;

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
