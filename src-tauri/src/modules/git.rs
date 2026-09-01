//! Модуль Git-изменений проекта.
//!
//! Читает локальный репозиторий проекта OpenCode (`git status`, `git diff`) и
//! возвращает список изменённых файлов с диффом (до/после) для показа в панели
//! окна чата (десктоп) и на отдельном экране (мобилка).

use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

/// Статус изменения файла.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitFileChange {
    /// Путь к файлу относительно корня репозитория.
    pub path: String,
    /// Один из: `modified`, `added`, `deleted`, `untracked`, `renamed`.
    pub status: String,
    /// Количество добавленных строк.
    pub additions: u32,
    /// Количество удалённых строк.
    pub deletions: u32,
}

/// Дифф одного файла (текст в формате unified diff).
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitDiff {
    pub path: String,
    pub status: String,
    /// Размер диффа (в байтах) — для защиты от огромных файлов.
    pub too_large: bool,
    /// Текст unified diff.
    pub diff: String,
}

/// Максимальный размер читаемого диффа (байты).
const MAX_DIFF_BYTES: usize = 256 * 1024;

/// Запускает `git` в указанной папке и возвращает stdout (или ошибку).
fn run_git(directory: &str, args: &[&str]) -> Result<String, String> {
    // `-c core.quotepath=false` заставляет git выводить не-ASCII пути как UTF-8
    // (не octal-escape вида `\321\202...`). На Windows `core.quotepath` по
    // умолчанию включён, из-за чего кириллические имена файлов превращались в
    // «мусор» и не находились на диске. На macOS настройка и так выключена.
    let out = Command::new("git")
        .arg("-c")
        .arg("core.quotepath=false")
        .args(args)
        .current_dir(directory)
        .output()
        .map_err(|e| {
            let msg = format!("git не запустился в {directory:?}: {e}");
            log::error!("{msg}");
            msg
        })?;
    // `git diff` возвращает код 1, когда есть изменения — это не ошибка.
    if out.status.success() || !out.stdout.is_empty() {
        return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
    }
    let msg = String::from_utf8_lossy(&out.stderr).into_owned();
    log::debug!("git {args:?} в {directory:?}: {msg}");
    Err(msg)
}

/// Превращает двухсимвольный статус `git status --porcelain` в строку статуса.
fn status_label(x: char, y: char) -> &'static str {
    let code = if x != ' ' { x } else { y };
    match code {
        'A' => "added",
        'D' => "deleted",
        'R' => "renamed",
        'C' => "copied",
        '?' => "untracked",
        _ => "modified",
    }
}

/// Список изменённых файлов проекта (рабочее дерево + индекс).
pub fn changes(directory: &str) -> Vec<GitFileChange> {
    // `--untracked-files=all` разворачивает неотслеживаемые папки в отдельные
    // файлы — иначе папка попадает в список одной строкой и клик по ней пуст.
    let Ok(status) = run_git(
        directory,
        &["status", "--porcelain=v1", "--untracked-files=all"],
    ) else {
        return Vec::new();
    };

    // Числа добавленных/удалённых строк для отслеживаемых изменений.
    let mut numstat: HashMap<String, (u32, u32)> = HashMap::new();
    if let Ok(ns) = run_git(directory, &["diff", "--numstat"]) {
        for line in ns.lines() {
            let mut it = line.splitn(3, '\t');
            let added = it.next().and_then(|s| s.parse::<u32>().ok());
            let deleted = it.next().and_then(|s| s.parse::<u32>().ok());
            let path = it.next().unwrap_or("").to_string();
            if !path.is_empty() {
                numstat.insert(path, (added.unwrap_or(0), deleted.unwrap_or(0)));
            }
        }
    }

    let mut result = Vec::new();
    for line in status.lines() {
        if line.len() < 4 {
            continue;
        }
        let mut chars = line.chars();
        let x = chars.next().unwrap_or(' ');
        let y = chars.next().unwrap_or(' ');
        let rest = &line[3..];

        // Переименование: "R  old -> new".
        let (path, status) = if rest.contains(" -> ") {
            let new_path = rest.split(" -> ").nth(1).unwrap_or(rest);
            (new_path.to_string(), "renamed".to_string())
        } else {
            (rest.trim_matches('"').to_string(), status_label(x, y).to_string())
        };
        if path.is_empty() {
            continue;
        }

        let (additions, deletions) = if status == "untracked" {
            let path_buf = Path::new(directory).join(&path);
            let count = std::fs::read_to_string(&path_buf)
                .map(|s| s.lines().count() as u32)
                .unwrap_or(0);
            (count, 0)
        } else if status == "added" {
            // Добавленные в индекс — считаем по числу строк файла.
            let path_buf = Path::new(directory).join(&path);
            let count = std::fs::read_to_string(&path_buf)
                .map(|s| s.lines().count() as u32)
                .unwrap_or(0);
            (count, 0)
        } else {
            numstat.get(&path).copied().unwrap_or((0, 0))
        };

        result.push(GitFileChange {
            path,
            status,
            additions,
            deletions,
        });
    }

    result.sort_by(|a, b| a.path.cmp(&b.path));
    result
}

/// Дифф файла (unified diff). Для untracked/новых файлов строим дифф вручную.
pub fn diff(directory: &str, path: &str) -> GitDiff {
    let status = changes(directory)
        .into_iter()
        .find(|c| c.path == path)
        .map(|c| c.status)
        .unwrap_or_else(|| "modified".to_string());

    let is_untracked = status == "untracked" || status == "added";

    let raw = if is_untracked {
        new_file_diff(directory, path)
    } else {
        run_git(directory, &["diff", "--", path]).unwrap_or_default()
    };

    let too_large = raw.len() > MAX_DIFF_BYTES;
    let diff_text = if too_large {
        raw.chars().take(MAX_DIFF_BYTES).collect()
    } else {
        raw
    };

    GitDiff {
        path: path.to_string(),
        status,
        too_large,
        diff: diff_text,
    }
}

/// Строит unified diff для нового (untracked/added) файла: все строки — добавленные.
fn new_file_diff(directory: &str, path: &str) -> String {
    let full = Path::new(directory).join(path);
    let content = match std::fs::read_to_string(&full) {
        Ok(c) => c,
        Err(_) => return String::new(),
    };
    let lines = content.lines().count();
    let mut out = String::new();
    out.push_str(&format!("diff --git a/{path} b/{path}\n"));
    out.push_str("new file mode 100644\n");
    out.push_str("--- /dev/null\n");
    out.push_str(&format!("+++ b/{path}\n"));
    out.push_str(&format!("@@ -0,0 +1,{lines} @@\n"));
    for line in content.lines() {
        out.push('+');
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Имя репозитория (basename) — для отображения в панели.
pub fn repo_name(directory: &str) -> String {
    Path::new(directory)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| directory.to_string())
}

/// Текущая ветка репозитория (пустая строка, если не git-репозиторий).
pub fn current_branch(directory: &str) -> String {
    run_git(directory, &["rev-parse", "--abbrev-ref", "HEAD"])
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// Сводка: текущая ветка + изменённые файлы.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitInfo {
    pub branch: String,
    pub changes: Vec<GitFileChange>,
}

pub fn info(directory: &str) -> GitInfo {
    GitInfo {
        branch: current_branch(directory),
        changes: changes(directory),
    }
}
