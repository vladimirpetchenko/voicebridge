//! Модуль распознавания речи (STT).
//!
//! Реализует локальное распознавание через `whisper.cpp` (обёртка `whisper-rs`):
//! - реестр моделей (Tiny/Base/Small/Medium + Qwen3-ASR как «скоро»);
//! - скачивание моделей с прогрессом (событие `model-download-progress`);
//! - загрузка модели в фоновом потоке;
//! - транскрипция аудио (16 кГц mono f32) в фоновом потоке.
//!
//! Все модели работают локально — аудио не покидает компьютер.

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

use crate::state::SharedState;

/// Описание модели в статическом реестре (не сериализуется напрямую).
#[derive(Clone, Copy)]
struct SttModelDef {
    id: &'static str,
    name: &'static str,
    size_mb: u64,
    url: &'static str,
    file_name: &'static str,
    engine: &'static str,
    supported: bool,
    description: &'static str,
}

/// Сериализуемая информация о модели (с флагом «скачана»).
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SttModelInfo {
    pub id: String,
    pub name: String,
    pub size_mb: u64,
    pub engine: String,
    pub supported: bool,
    pub description: String,
    pub downloaded: bool,
}

const MODELS: &[SttModelDef] = &[
    SttModelDef {
        id: "whisper-tiny",
        name: "Whisper Tiny",
        size_mb: 75,
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin",
        file_name: "ggml-tiny.bin",
        engine: "whisper",
        supported: true,
        description: "Самый быстрый, базовая точность",
    },
    SttModelDef {
        id: "whisper-base",
        name: "Whisper Base",
        size_mb: 142,
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin",
        file_name: "ggml-base.bin",
        engine: "whisper",
        supported: true,
        description: "Быстрый, хорошая точность",
    },
    SttModelDef {
        id: "whisper-small",
        name: "Whisper Small",
        size_mb: 466,
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin",
        file_name: "ggml-small.bin",
        engine: "whisper",
        supported: true,
        description: "Средний, отличная точность",
    },
    SttModelDef {
        id: "whisper-medium",
        name: "Whisper Medium",
        size_mb: 1500,
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin",
        file_name: "ggml-medium.bin",
        engine: "whisper",
        supported: true,
        description: "Высокая точность",
    },
    SttModelDef {
        id: "qwen3-asr-fast",
        name: "Qwen3-ASR Fast",
        size_mb: 1000,
        url: "",
        file_name: "",
        engine: "qwen3",
        supported: false,
        description: "Оптимизирован для Apple Silicon (скоро)",
    },
    SttModelDef {
        id: "qwen3-asr-accurate",
        name: "Qwen3-ASR Accurate",
        size_mb: 2500,
        url: "",
        file_name: "",
        engine: "qwen3",
        supported: false,
        description: "Максимальная точность (скоро)",
    },
];

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn models_dir(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("models")
}

fn model_path(app: &AppHandle, model_id: &str) -> Option<PathBuf> {
    let def = MODELS.iter().find(|m| m.id == model_id)?;
    if !def.supported || def.file_name.is_empty() {
        return None;
    }
    Some(models_dir(app).join(def.file_name))
}

/// Возвращает список моделей с информацией о том, какие уже скачаны.
pub fn list_models(app: &AppHandle) -> Vec<SttModelInfo> {
    let dir = models_dir(app);
    MODELS
        .iter()
        .map(|m| SttModelInfo {
            id: m.id.to_string(),
            name: m.name.to_string(),
            size_mb: m.size_mb,
            engine: m.engine.to_string(),
            supported: m.supported,
            description: m.description.to_string(),
            downloaded: !m.file_name.is_empty() && dir.join(m.file_name).exists(),
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Фоновый поток STT
// ---------------------------------------------------------------------------

enum SttJob {
    LoadModel { path: PathBuf },
    Transcribe {
        samples: Vec<f32>,
        language: Option<String>,
        model_path: PathBuf,
    },
}

pub struct SttEngine {
    tx: Mutex<mpsc::Sender<SttJob>>,
}

pub fn spawn(app: AppHandle) -> SttEngine {
    let (tx, rx) = mpsc::channel::<SttJob>();
    std::thread::Builder::new()
        .name("stt-worker".into())
        .spawn(move || worker_loop(app, rx))
        .expect("failed to spawn stt worker");
    SttEngine { tx: Mutex::new(tx) }
}

fn load_model(path: &Path) -> Result<whisper_rs::WhisperContext, String> {
    let mut params = whisper_rs::WhisperContextParameters::new();
    params.use_gpu(cfg!(target_os = "macos"));
    whisper_rs::WhisperContext::new_with_params(path, params)
        .map_err(|e| format!("не удалось загрузить модель: {e}"))
}

fn transcribe(
    ctx: &whisper_rs::WhisperContext,
    samples: &[f32],
    language: Option<&str>,
) -> Result<String, String> {
    use whisper_rs::{FullParams, SamplingStrategy};

    let mut state = ctx.create_state().map_err(|e| e.to_string())?;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_translate(false);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_suppress_blank(true);
    params.set_no_context(true);
    params.set_language(language);

    state
        .full(params, samples)
        .map_err(|e| format!("ошибка распознавания: {e}"))?;

    let n = state.full_n_segments();
    let mut text = String::new();
    for i in 0..n {
        if let Some(seg) = state.get_segment(i) {
            if let Ok(s) = seg.to_str_lossy() {
                let s = s.trim();
                if !s.is_empty() {
                    if !text.is_empty() {
                        text.push(' ');
                    }
                    text.push_str(s);
                }
            }
        }
    }
    Ok(text)
}

fn worker_loop(app: AppHandle, rx: mpsc::Receiver<SttJob>) {
    let mut ctx: Option<whisper_rs::WhisperContext> = None;
    let mut ctx_path: Option<PathBuf> = None;

    while let Ok(job) = rx.recv() {
        match job {
            SttJob::LoadModel { path } => {
                let _ = app.emit("model-loading", ());
                log::info!("loading whisper model: {}", path.display());
                match load_model(&path) {
                    Ok(c) => {
                        ctx = Some(c);
                        ctx_path = Some(path);
                        log::info!("whisper model loaded");
                        let _ = app.emit("model-loaded", ());
                    }
                    Err(e) => {
                        log::error!("whisper model load failed: {e}");
                        let _ = app.emit("model-load-error", e.clone());
                        crate::commands::fail_transcription(&app, e);
                    }
                }
            }
            SttJob::Transcribe {
                samples,
                language,
                model_path,
            } => {
                log::info!(
                    "transcribe start: {} samples, language={:?}",
                    samples.len(),
                    language
                );
                if ctx_path.as_deref() != Some(model_path.as_path()) {
                    match load_model(&model_path) {
                        Ok(c) => {
                            ctx = Some(c);
                            ctx_path = Some(model_path);
                        }
                        Err(e) => {
                            crate::commands::fail_transcription(&app, e);
                            continue;
                        }
                    }
                }

                let Some(ctx) = ctx.as_ref() else {
                    crate::commands::fail_transcription(&app, "модель не загружена".into());
                    continue;
                };

                match transcribe(ctx, &samples, language.as_deref()) {
                    Ok(text) => {
                        if text.is_empty() {
                            crate::commands::fail_transcription(&app, "речь не распознана".into());
                        } else {
                            log::info!("transcribe done: {} chars", text.len());
                            crate::commands::finish_transcription(&app, text);
                        }
                    }
                    Err(e) => crate::commands::fail_transcription(&app, e),
                }
            }
        }
    }
}

/// Запускает асинхронную транскрипцию накопленного аудио.
pub fn transcribe_async(app: &AppHandle, samples: Vec<f32>) {
    let state = app.state::<SharedState>();
    let (model_path, language) = {
        let s = state.0.lock().unwrap();
        let model_path = s
            .selected_model
            .as_deref()
            .and_then(|id| model_path(app, id))
            .filter(|p| p.exists());
        let language = match s.language.as_str() {
            "ru" => Some("ru".to_string()),
            "en" => Some("en".to_string()),
            _ => None,
        };
        (model_path, language)
    };

    let Some(model_path) = model_path else {
        crate::commands::fail_transcription(
            app,
            "модель не выбрана или не скачана. Скачайте модель в настройках.".into(),
        );
        return;
    };

    let engine = app.state::<SttEngine>();
    let job = SttJob::Transcribe {
        samples,
        language,
        model_path,
    };
    if engine.tx.lock().unwrap().send(job).is_err() {
        crate::commands::fail_transcription(app, "STT-движок недоступен".into());
    }
}

/// Отправляет команду загрузки модели в фоновый поток (если модель скачана).
pub fn request_model_load(app: &AppHandle, model_id: &str) -> bool {
    let Some(path) = model_path(app, model_id) else {
        return false;
    };
    if !path.exists() {
        return false;
    }
    let engine = app.state::<SttEngine>();
    let sent = engine
        .tx
        .lock()
        .unwrap()
        .send(SttJob::LoadModel { path })
        .is_ok();
    sent
}

// ---------------------------------------------------------------------------
// Скачивание моделей
// ---------------------------------------------------------------------------

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadProgress {
    model_id: String,
    downloaded_bytes: u64,
    total_bytes: u64,
    percent: f64,
}

fn download_to(app: &AppHandle, model: &SttModelDef, path: &Path) -> Result<(), String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(3600))
        .build()
        .map_err(|e| e.to_string())?;

    let mut resp = client.get(model.url).send().map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }

    let total = resp.content_length().unwrap_or(0);
    let mut file = std::fs::File::create(path).map_err(|e| e.to_string())?;

    use std::io::{Read, Write};
    let mut buf = vec![0u8; 128 * 1024];
    let mut downloaded = 0u64;
    let mut last_emit = 0u64;

    loop {
        let n = resp.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        downloaded += n as u64;

        let now = now_millis();
        if now.saturating_sub(last_emit) >= 100 {
            last_emit = now;
            let percent = if total > 0 {
                (downloaded as f64 / total as f64) * 100.0
            } else {
                0.0
            };
            let _ = app.emit(
                "model-download-progress",
                DownloadProgress {
                    model_id: model.id.to_string(),
                    downloaded_bytes: downloaded,
                    total_bytes: total,
                    percent,
                },
            );
        }
    }

    file.flush().map_err(|e| e.to_string())?;
    Ok(())
}

/// Скачивает модель в фоновом потоке с событиями прогресса.
pub fn download_model(app: AppHandle, model_id: String) {
    let Some(model) = MODELS.iter().find(|m| m.id == model_id).cloned() else {
        return;
    };
    if !model.supported || model.url.is_empty() {
        let _ = app.emit(
            "model-download-error",
            serde_json::json!({ "modelId": model_id, "error": "модель пока не поддерживается" }),
        );
        return;
    }

    std::thread::Builder::new()
        .name("model-download".into())
        .spawn(move || {
            log::info!("downloading model: {} ({})", model.id, model.url);
            let dir = models_dir(&app);
            if let Err(e) = std::fs::create_dir_all(&dir) {
                let _ = app.emit(
                    "model-download-error",
                    serde_json::json!({ "modelId": model_id, "error": e.to_string() }),
                );
                return;
            }
            let path = dir.join(model.file_name);
            match download_to(&app, &model, &path) {
                Ok(()) => {
                    log::info!("model downloaded: {}", model.id);
                    let _ = app.emit(
                        "model-download-done",
                        serde_json::json!({ "modelId": model_id }),
                    );
                    // Если скачана выбранная модель — сразу загружаем её.
                    let selected = {
                        let state = app.state::<SharedState>();
                        let s = state.0.lock().unwrap();
                        s.selected_model.clone()
                    };
                    if selected.as_deref() == Some(model.id) {
                        request_model_load(&app, model.id);
                    }
                }
                Err(e) => {
                    log::error!("model download failed ({}): {e}", model.id);
                    let _ = std::fs::remove_file(&path);
                    let _ = app.emit(
                        "model-download-error",
                        serde_json::json!({ "modelId": model_id, "error": e }),
                    );
                }
            }
        })
        .expect("failed to spawn download thread");
}

/// Гарантирует выбор модели по умолчанию (Whisper Base) и скачивает её,
/// если ни одной модели ещё нет на диске.
pub fn ensure_default_model(app: &AppHandle) {
    const DEFAULT: &str = "whisper-base";

    let selected = {
        let state = app.state::<SharedState>();
        let s = state.0.lock().unwrap();
        s.selected_model.clone()
    };

    if selected.is_none() {
        let state = app.state::<SharedState>();
        let mut s = state.0.lock().unwrap();
        s.selected_model = Some(DEFAULT.to_string());
        let snapshot = s.clone();
        drop(s);
        crate::commands::emit_state(app, &snapshot);
        crate::commands::save_state(app, &snapshot);
    }

    let any_downloaded = list_models(app).iter().any(|m| m.downloaded);
    let base_downloaded = model_path(app, DEFAULT).map(|p| p.exists()).unwrap_or(false);

    if !any_downloaded {
        download_model(app.clone(), DEFAULT.to_string());
    } else if base_downloaded {
        request_model_load(app, DEFAULT);
    }
}
