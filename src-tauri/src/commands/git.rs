//! Команды Git-изменений проекта.

use tauri::AppHandle;

/// Сводка Git проекта текущей сессии (ветка + изменённые файлы) для окна чата.
#[tauri::command]
pub async fn get_git_changes(
    window: tauri::WebviewWindow,
    app: AppHandle,
) -> crate::modules::git::GitInfo {
    let session_id = window
        .label()
        .strip_prefix("response-")
        .unwrap_or_default()
        .to_string();
    if session_id.is_empty() {
        return crate::modules::git::GitInfo {
            branch: String::new(),
            changes: Vec::new(),
        };
    }
    // `git status`/`git diff` — подпроцессы, выполняем в фоне.
    tauri::async_runtime::spawn_blocking(move || {
        crate::modules::opencode::session_directory(&app, Some(&session_id))
            .map(|dir| crate::modules::git::info(&dir))
            .unwrap_or(crate::modules::git::GitInfo {
                branch: String::new(),
                changes: Vec::new(),
            })
    })
    .await
    .unwrap_or(crate::modules::git::GitInfo {
        branch: String::new(),
        changes: Vec::new(),
    })
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

/// История коммитов проекта текущей сессии (окно чата).
#[tauri::command]
pub async fn get_git_commits(
    window: tauri::WebviewWindow,
    app: AppHandle,
) -> Vec<crate::modules::git::GitCommit> {
    let session_id = window
        .label()
        .strip_prefix("response-")
        .unwrap_or_default()
        .to_string();
    if session_id.is_empty() {
        return Vec::new();
    }
    // `git log` — подпроцесс, выполняем в фоне.
    tauri::async_runtime::spawn_blocking(move || {
        crate::modules::opencode::session_directory(&app, Some(&session_id))
            .map(|dir| crate::modules::git::recent_commits(&dir))
            .unwrap_or_default()
    })
    .await
    .unwrap_or_default()
}

/// Детали коммита (метаданные + файлы + дифф) проекта текущей сессии.
#[tauri::command]
pub async fn get_git_commit(
    window: tauri::WebviewWindow,
    app: AppHandle,
    hash: String,
) -> crate::modules::git::GitCommitDetail {
    let session_id = window
        .label()
        .strip_prefix("response-")
        .unwrap_or_default()
        .to_string();
    let hash_for_fallback = hash.clone();
    if session_id.is_empty() || hash.trim().is_empty() {
        return crate::modules::git::GitCommitDetail {
            hash,
            author: String::new(),
            date: 0,
            message: String::new(),
            files: Vec::new(),
            diff: String::new(),
            too_large: false,
        };
    }
    tauri::async_runtime::spawn_blocking(move || {
        match crate::modules::opencode::session_directory(&app, Some(&session_id)) {
            Some(dir) => crate::modules::git::commit(&dir, &hash),
            None => crate::modules::git::GitCommitDetail {
                hash,
                author: String::new(),
                date: 0,
                message: String::new(),
                files: Vec::new(),
                diff: String::new(),
                too_large: false,
            },
        }
    })
    .await
    .unwrap_or_else(|_| crate::modules::git::GitCommitDetail {
        hash: hash_for_fallback,
        author: String::new(),
        date: 0,
        message: String::new(),
        files: Vec::new(),
        diff: String::new(),
        too_large: false,
    })
}

/// Список локальных веток проекта текущей сессии (окно чата).
#[tauri::command]
pub async fn get_git_branches(
    window: tauri::WebviewWindow,
    app: AppHandle,
) -> Vec<crate::modules::git::GitBranch> {
    let session_id = window
        .label()
        .strip_prefix("response-")
        .unwrap_or_default()
        .to_string();
    if session_id.is_empty() {
        return Vec::new();
    }
    tauri::async_runtime::spawn_blocking(move || {
        crate::modules::opencode::session_directory(&app, Some(&session_id))
            .map(|dir| crate::modules::git::branches(&dir))
            .unwrap_or_default()
    })
    .await
    .unwrap_or_default()
}
