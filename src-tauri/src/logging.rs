//! Минимальный файловый логгер (без внешних зависимостей, кроме `log`).
//! Нужен, чтобы диагностировать проблемы на Windows (у GUI-приложения нет консоли,
//! `eprintln!` там никуда не выводится). Пишет в файл + перехватывает паники.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;

static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();

struct FileLogger;

impl log::Log for FileLogger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        let Some(path) = LOG_PATH.get() else {
            return;
        };
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(
                file,
                "[{}] [{}] [{}] {}",
                record.level(),
                record.target(),
                chrono_now(),
                record.args()
            );
        }
    }

    fn flush(&self) {}
}

/// Возвращает текущее время в формате RFC3339 (или миллисекунды, если ошибка).
fn chrono_now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| format!("{}ms", d.as_millis()))
        .unwrap_or_default()
}

/// Инициализирует файловый логгер и перехват паник.
/// Вызывать один раз при старте приложения.
pub fn init(path: PathBuf) {
    let _ = LOG_PATH.set(path);

    // Игнорируем ошибку, если логгер уже установлен.
    let _ = log::set_logger(&FileLogger);
    log::set_max_level(log::LevelFilter::Info);

    std::panic::set_hook(Box::new(|info| {
        log::error!("panic: {info}");
    }));
}
