//! Обнаружение экземпляров OpenCode и создание сессий.

use super::{base_url, http_client, SessionInfo};
use crate::state::{OpenCodeInstance, OpenCodeSession};
use std::path::Path;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::process::Command;
use std::time::Duration;

/// Порты, на которых ищем экземпляры OpenCode.
const DEFAULT_PORTS: &[u16] = &[4096, 12000, 3000, 17000];

/// Извлекает порт из адреса вида `host:port`.
pub(crate) fn extract_port(name: &str) -> Option<u16> {
    name.rsplit(':')
        .next()?
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .ok()
}

/// PID процессов opencode на Windows (через `tasklist`).
#[cfg(target_os = "windows")]
pub(crate) fn opencode_pids_windows() -> Vec<u32> {
    let out = match crate::no_console_command("tasklist")
        .args(["/FO", "CSV", "/NH"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut pids = Vec::new();
    for line in text.lines() {
        // CSV-формат: "opencode.exe","1234","Console","1","123 456 K"
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 2 {
            continue;
        }
        if !parts[0].trim_matches('"').to_lowercase().contains("opencode") {
            continue;
        }
        if let Ok(pid) = parts[1].trim_matches('"').parse::<u32>() {
            pids.push(pid);
        }
    }
    pids
}

/// Находит порты, на которых слушают процессы OpenCode (через `lsof`).
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn opencode_process_ports() -> Vec<u16> {
    let out = match Command::new("lsof")
        .args(["-nP", "-iTCP", "-sTCP:LISTEN"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut ports = Vec::new();
    for line in text.lines().skip(1) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 9 {
            continue;
        }
        if !fields[0].to_lowercase().contains("opencode") {
            continue;
        }
        if let Some(port) = extract_port(fields[8]) {
            ports.push(port);
        }
    }
    ports
}

/// Находит порты, на которых слушают процессы OpenCode (через `netstat` + `tasklist`).
#[cfg(target_os = "windows")]
fn opencode_process_ports() -> Vec<u16> {
    let pids = opencode_pids_windows();
    if pids.is_empty() {
        return Vec::new();
    }
    let out = match crate::no_console_command("netstat")
        .args(["-ano", "-p", "TCP"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut ports = Vec::new();
    for line in text.lines() {
        // Формат: Proto Local_Address Foreign_Address State PID
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 5 {
            continue;
        }
        if !fields[3].eq_ignore_ascii_case("LISTENING") {
            continue;
        }
        let pid: u32 = match fields[4].parse() {
            Ok(p) => p,
            Err(_) => continue,
        };
        if !pids.contains(&pid) {
            continue;
        }
        if let Some(port) = extract_port(fields[1]) {
            ports.push(port);
        }
    }
    ports
}

/// Собирает порты-кандидаты: процессы OpenCode + порты по умолчанию.
fn candidate_ports() -> Vec<u16> {
    let mut ports = opencode_process_ports();
    for &p in DEFAULT_PORTS {
        if !ports.contains(&p) {
            ports.push(p);
        }
    }
    ports
}

/// Опрашивает порты и возвращает обнаруженные проекты OpenCode с их сессиями.
///
/// `opencode serve` отдаёт глобальный список сессий (все проекты сразу), а
/// `/project/current` — глобальный проект (`/`), поэтому группируем сессии по
/// их `directory` (папке проекта), а не по серверу/порту.
pub fn discover_instances() -> Vec<OpenCodeInstance> {
    let client = http_client(Duration::from_millis(800));
    let mut running_ports: Vec<u16> = Vec::new();
    let mut by_dir: std::collections::BTreeMap<String, Vec<OpenCodeSession>> =
        std::collections::BTreeMap::new();

    for port in candidate_ports() {
        let url = format!("{}/session", base_url(port));
        let resp = match client.get(&url).send() {
            Ok(r) => r,
            Err(_) => continue,
        };
        if !resp.status().is_success() {
            continue;
        }
        let sessions: Vec<OpenCodeSession> = match resp.json::<Vec<SessionInfo>>() {
            Ok(list) => list.into_iter().map(OpenCodeSession::from).collect(),
            Err(_) => continue,
        };
        running_ports.push(port);
        for s in sessions {
            if s.directory.is_empty() {
                continue;
            }
            let entry = by_dir.entry(s.directory.clone()).or_default();
            if !entry.iter().any(|x| x.id == s.id) {
                entry.push(s);
            }
        }
    }

    if running_ports.is_empty() {
        return Vec::new();
    }

    // Ручной глобальный сервер (opencode serve на порту по умолчанию) отдаёт
    // сессии всех проектов — в этом случае показываем все проекты.
    let has_default_port = running_ports.iter().any(|p| DEFAULT_PORTS.contains(p));

    let mut instances: Vec<OpenCodeInstance> = by_dir
        .into_iter()
        .filter_map(|(directory, mut sessions)| {
            // Показываем проект только если запущен его «собственный» сервер
            // (порт project_port) или есть ручной глобальный сервер.
            let own_port = super::projects::project_port(&directory);
            let port = if running_ports.contains(&own_port) {
                own_port
            } else if has_default_port {
                running_ports
                    .iter()
                    .copied()
                    .find(|p| DEFAULT_PORTS.contains(p))
                    .unwrap_or(running_ports[0])
            } else {
                return None;
            };
            sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
            let name = Path::new(&directory)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| directory.clone());
            log::info!(
                "discovered opencode project: {name} (port {port}, {} sessions)",
                sessions.len()
            );
            Some(OpenCodeInstance {
                id: directory.clone(),
                name,
                port,
                sessions,
            })
        })
        .collect();

    instances.sort_by(|a, b| {
        let au = a.sessions.iter().map(|s| s.updated_at).max().unwrap_or(0);
        let bu = b.sessions.iter().map(|s| s.updated_at).max().unwrap_or(0);
        bu.cmp(&au)
    });
    instances
}

/// Создаёт новую сессию в экземпляре OpenCode (по порту) и возвращает её.
pub fn create_session(port: u16, title: &str) -> Result<OpenCodeSession, String> {
    let client = http_client(Duration::from_secs(10));
    let resp = client
        .post(format!("{}/session", base_url(port)))
        .json(&serde_json::json!({ "title": title }))
        .send()
        .map_err(|e| format!("не удалось создать сессию: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("не удалось создать сессию: HTTP {}", resp.status()));
    }
    let session: SessionInfo = resp
        .json()
        .map_err(|e| format!("не удалось создать сессию: {e}"))?;
    Ok(OpenCodeSession::from(session))
}
