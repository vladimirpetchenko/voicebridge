//! Управление проектами: список, запуск/остановка headless-серверов OpenCode.

use super::{base_url, http_client};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub worktree: String,
    pub name: String,
    pub updated: u64,
    pub running: bool,
    pub port: u16,
}

#[derive(Deserialize)]
struct DirRow {
    worktree: String,
    #[serde(default)]
    updated: u64,
}

/// Находит исполняемый файл opencode (PATH у GUI-приложений ограничен).
pub fn opencode_binary() -> String {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        for p in ["/opt/homebrew/bin/opencode", "/usr/local/bin/opencode"] {
            if Path::new(p).exists() {
                return p.to_string();
            }
        }
        if let Ok(out) = Command::new("sh")
            .args(["-lc", "command -v opencode"])
            .output()
        {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return s;
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        // `where opencode` отдаёт пути в OEM-кодировке (CP866/CP1251), поэтому
        // для путей с кириллицей при декодировании в UTF-8 получается «мусор»
        // и файл не находится. Ищем бинарник напрямую по каталогам из PATH
        // (env::var возвращает корректную строку). npm-шим содержит `opencode`
        // (bash-скрипт), `opencode.cmd` и `opencode.ps1`; запускать напрямую
        // можно только .exe и .cmd/.bat, поэтому предпочитаем их.
        if let Ok(path_var) = std::env::var("PATH") {
            let mut shim: Option<std::path::PathBuf> = None;
            for dir in std::env::split_paths(&path_var) {
                let exe = dir.join("opencode.exe");
                if exe.is_file() {
                    return exe.to_string_lossy().into_owned();
                }
                if shim.is_none() {
                    for name in ["opencode.cmd", "opencode.bat"] {
                        let candidate = dir.join(name);
                        if candidate.is_file() {
                            shim = Some(candidate);
                            break;
                        }
                    }
                }
            }
            if let Some(p) = shim {
                return p.to_string_lossy().into_owned();
            }
        }
    }
    "opencode".to_string()
}

/// Собирает команду запуска opencode. На Windows npm-шим (`.cmd`/`.bat`)
/// нельзя запускать напрямую — оборачиваем в `cmd /C`.
fn opencode_command(bin: &str) -> Command {
    #[cfg(target_os = "windows")]
    {
        if bin.ends_with(".cmd") || bin.ends_with(".bat") {
            let mut c = Command::new("cmd");
            c.arg("/C").arg(bin);
            return c;
        }
    }
    Command::new(bin)
}

/// Стабильный порт для проекта (по хэшу пути, диапазон 4100–4199).
pub(crate) fn project_port(worktree: &str) -> u16 {
    // Нормализуем разделители: `opencode db` отдаёт пути с `/`, а `GET /session`
    // и диалог выбора папки — с `\`. Без нормализации один и тот же проект
    // получал бы разные порты.
    let normalized = worktree.replace('\\', "/");
    let mut h: u32 = 2_166_136_261;
    for b in normalized.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(16_777_619);
    }
    (4100 + h % 100) as u16
}

pub(crate) fn is_server_running(port: u16) -> bool {
    let client = http_client(Duration::from_millis(300));
    client
        .get(format!("{}/session", base_url(port)))
        .send()
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

/// PID процесса opencode, слушающего заданный порт (через lsof).
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn opencode_pid_on_port(port: u16) -> Option<u32> {
    let out = Command::new("lsof")
        .args(["-nP", &format!("-iTCP:{port}"), "-sTCP:LISTEN"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines().skip(1) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() >= 2 && fields[0].to_lowercase().contains("opencode") {
            return fields[1].parse().ok();
        }
    }
    None
}

/// PID процесса opencode, слушающего заданный порт (через `netstat` + `tasklist`).
#[cfg(target_os = "windows")]
fn opencode_pid_on_port(port: u16) -> Option<u32> {
    let pids = super::discovery::opencode_pids_windows();
    if pids.is_empty() {
        return None;
    }
    let out = Command::new("netstat")
        .args(["-ano", "-p", "TCP"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 5 {
            continue;
        }
        if !fields[3].eq_ignore_ascii_case("LISTENING") {
            continue;
        }
        let pid: u32 = fields[4].parse().ok()?;
        if pids.contains(&pid) && super::discovery::extract_port(fields[1]) == Some(port) {
            return Some(pid);
        }
    }
    None
}

/// Список известных проектов (папок, где были сессии opencode).
pub fn list_projects() -> Vec<Project> {
    let bin = opencode_binary();
    const QUERY: &str = "SELECT directory AS worktree, MAX(time_updated) AS updated FROM session WHERE directory != '' GROUP BY directory ORDER BY updated DESC";
    let out = match opencode_command(&bin)
        .args(["db", QUERY, "--format", "json"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    if !out.status.success() {
        return Vec::new();
    }
    let rows: Vec<DirRow> = match serde_json::from_slice(&out.stdout) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let mut projects: Vec<Project> = rows
        .into_iter()
        .filter(|r| !r.worktree.is_empty() && Path::new(&r.worktree).parent().is_some())
        .map(|r| {
            let name = Path::new(&r.worktree)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| r.worktree.clone());
            let port = project_port(&r.worktree);
            Project {
                id: r.worktree.clone(),
                worktree: r.worktree,
                name,
                updated: r.updated,
                running: is_server_running(port),
                port,
            }
        })
        .collect();

    projects.sort_by(|a, b| b.updated.cmp(&a.updated));
    projects
}

/// Запускает headless-сервер OpenCode для проекта (в его папке).
pub fn start_project(worktree: &str) -> Result<(), String> {
    let port = project_port(worktree);
    if is_server_running(port) {
        return Ok(());
    }
    if !Path::new(worktree).is_dir() {
        return Err("папка проекта не существует".into());
    }
    let bin = opencode_binary();
    opencode_command(&bin)
        .args(["serve", "--port", &port.to_string(), "--hostname", "127.0.0.1"])
        .current_dir(worktree)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("не удалось запустить opencode: {e}"))?;
    Ok(())
}

/// Останавливает headless-сервер OpenCode для проекта.
pub fn stop_project(worktree: &str) -> Result<(), String> {
    let port = project_port(worktree);
    let pid = opencode_pid_on_port(port).ok_or("сервер для проекта не запущен")?;

    #[cfg(target_os = "windows")]
    let status = {
        let pid_str = pid.to_string();
        Command::new("taskkill")
            .args(["/PID", pid_str.as_str(), "/F"])
            .status()
            .map_err(|e| e.to_string())?
    };
    #[cfg(not(target_os = "windows"))]
    let status = Command::new("kill")
        .arg(pid.to_string())
        .status()
        .map_err(|e| e.to_string())?;

    if status.success() {
        Ok(())
    } else {
        Err("не удалось остановить сервер".into())
    }
}
