//! Модуль захвата и распознавания звука (STT).
//!
//! Реализовано на данном этапе:
//! - перечисление устройств ввода через `cpal`;
//! - выбор микрофона и чувствительность (усиление уровня);
//! - живой захват звука при записи с расчётом RMS-уровня
//!   и отправкой события `audio-level` во фронтенд (визуализация волны).
//!
//! Планируется далее:
//! - локальное распознавание через обёртку над `whisper.cpp`
//!   (модели Tiny/Base/Small/Medium/Large v3 Turbo/Large v3);
//! - скачивание моделей с прогрессом.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

use crate::state::SharedState;

/// Активный поток захвата. Хранится в управляемом состоянии,
/// чтобы держать его живым, пока идёт запись, и останавливать при её завершении.
pub struct AudioEngine {
    pub stream: Mutex<Option<cpal::Stream>>,
    pub buffer: Arc<Mutex<Vec<f32>>>,
    pub sample_rate: Arc<Mutex<u32>>,
    pub channels: Arc<Mutex<u16>>,
}

impl Default for AudioEngine {
    fn default() -> Self {
        Self {
            stream: Mutex::new(None),
            buffer: Arc::new(Mutex::new(Vec::new())),
            sample_rate: Arc::new(Mutex::new(16000)),
            channels: Arc::new(Mutex::new(1)),
        }
    }
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Список доступных устройств ввода (человекочитаемые имена).
pub fn list_microphones() -> Vec<String> {
    let host = cpal::default_host();
    match host.input_devices() {
        Ok(devices) => devices.map(|d| d.to_string()).collect(),
        Err(_) => Vec::new(),
    }
}

fn find_device(name: Option<&str>) -> Result<cpal::Device, String> {
    let host = cpal::default_host();

    match name {
        Some(name) if !name.trim().is_empty() => {
            let devices = host.input_devices().map_err(|e| e.to_string())?;
            for device in devices {
                if device.to_string() == name {
                    return Ok(device);
                }
            }
            Err("микрофон не найден".into())
        }
        _ => host
            .default_input_device()
            .ok_or_else(|| "нет устройства ввода".into()),
    }
}

/// Собирает поток захвата для заданного формата сэмплов.
fn build_stream<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    app: &AppHandle,
    sensitivity: f32,
    last_emit: Arc<AtomicU64>,
    buffer: Arc<Mutex<Vec<f32>>>,
    max_samples: usize,
) -> Result<cpal::Stream, cpal::Error>
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    let app = app.clone();
    let err_fn = move |err| log::error!("audio stream error: {err}");

    device.build_input_stream(
        config,
        move |data: &[T], _| {
            let mut sum = 0.0f32;
            let mut push: Vec<f32> = Vec::with_capacity(data.len());
            for sample in data {
                let v: f32 = sample.to_sample();
                sum += v * v;
                push.push(v);
            }

            {
                let mut buf = buffer.lock().unwrap();
                let remaining = max_samples.saturating_sub(buf.len());
                if remaining > 0 {
                    buf.extend_from_slice(&push[..push.len().min(remaining)]);
                }
            }

            if data.is_empty() {
                return;
            }
            let rms = (sum / data.len() as f32).sqrt();
            let level = (rms * sensitivity).clamp(0.0, 1.0);

            // Ограничиваем частоту отправки примерно 30 Гц.
            let now = now_millis();
            let prev = last_emit.load(Ordering::Relaxed);
            if now.saturating_sub(prev) >= 33 {
                if last_emit
                    .compare_exchange(prev, now, Ordering::Relaxed, Ordering::Relaxed)
                    .is_ok()
                {
                    let _ = app.emit("audio-level", level);
                }
            }
        },
        err_fn,
        None,
    )
}

/// Запускает захват звука (использует выбранный микрофон и чувствительность из состояния).
pub fn start_capture(app: &AppHandle) -> Result<(), String> {
    // Останавливаем предыдущий поток, если он был.
    stop_capture(app);

    let state = app.state::<SharedState>();
    let (device_name, sensitivity) = {
        let s = state.0.lock().unwrap();
        (s.selected_microphone.clone(), s.sensitivity)
    };

    let device = find_device(device_name.as_deref())?;
    let supported = device.default_input_config().map_err(|e| e.to_string())?;

    let config: cpal::StreamConfig = supported.into();
    let sample_rate = config.sample_rate;
    let channels = config.channels;

    // Готовим буфер накопления и запоминаем параметры потока.
    let engine = app.state::<AudioEngine>();
    *engine.sample_rate.lock().unwrap() = sample_rate;
    *engine.channels.lock().unwrap() = channels;
    engine.buffer.lock().unwrap().clear();
    let buffer = engine.buffer.clone();
    let max_samples = (sample_rate as usize) * (channels as usize) * 30;

    let last_emit = Arc::new(AtomicU64::new(0));

    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => {
            build_stream::<f32>(&device, config, app, sensitivity, last_emit, buffer, max_samples)
        }
        cpal::SampleFormat::I16 => {
            build_stream::<i16>(&device, config, app, sensitivity, last_emit, buffer, max_samples)
        }
        cpal::SampleFormat::U16 => {
            build_stream::<u16>(&device, config, app, sensitivity, last_emit, buffer, max_samples)
        }
        cpal::SampleFormat::I32 => {
            build_stream::<i32>(&device, config, app, sensitivity, last_emit, buffer, max_samples)
        }
        cpal::SampleFormat::U32 => {
            build_stream::<u32>(&device, config, app, sensitivity, last_emit, buffer, max_samples)
        }
        cpal::SampleFormat::I64 => {
            build_stream::<i64>(&device, config, app, sensitivity, last_emit, buffer, max_samples)
        }
        cpal::SampleFormat::U64 => {
            build_stream::<u64>(&device, config, app, sensitivity, last_emit, buffer, max_samples)
        }
        cpal::SampleFormat::F64 => {
            build_stream::<f64>(&device, config, app, sensitivity, last_emit, buffer, max_samples)
        }
        cpal::SampleFormat::I8 => {
            build_stream::<i8>(&device, config, app, sensitivity, last_emit, buffer, max_samples)
        }
        cpal::SampleFormat::U8 => {
            build_stream::<u8>(&device, config, app, sensitivity, last_emit, buffer, max_samples)
        }
        cpal::SampleFormat::I24 => {
            build_stream::<cpal::I24>(&device, config, app, sensitivity, last_emit, buffer, max_samples)
        }
        cpal::SampleFormat::U24 => {
            build_stream::<cpal::U24>(&device, config, app, sensitivity, last_emit, buffer, max_samples)
        }
        other => return Err(format!("неподдерживаемый формат сэмплов: {other:?}")),
    }
    .map_err(|e| e.to_string())?;

    stream.play().map_err(|e| e.to_string())?;

    *app.state::<AudioEngine>().stream.lock().unwrap() = Some(stream);
    Ok(())
}

/// Забирает накопленные сэмплы (и сбрасывает буфер).
/// Возвращает (сэмплы interleaved f32, частота, число каналов).
pub fn take_audio(app: &AppHandle) -> Option<(Vec<f32>, u32, u16)> {
    let engine = app.try_state::<AudioEngine>()?;
    let mut buf = engine.buffer.lock().unwrap();
    if buf.is_empty() {
        return None;
    }
    let samples = std::mem::take(&mut *buf);
    let rate = *engine.sample_rate.lock().unwrap();
    let channels = *engine.channels.lock().unwrap();
    Some((samples, rate, channels))
}

/// Передискретизирует interleaved-аудио в 16 кГц mono f32 (формат whisper).
pub fn resample_to_16k_mono(samples: &[f32], src_rate: u32, channels: u16) -> Vec<f32> {
    const TARGET: u32 = 16000;
    let channels = (channels as usize).max(1);
    let src_rate = (src_rate as f64).max(1.0);
    let mono_len = (samples.len() + channels - 1) / channels;
    let ratio = TARGET as f64 / src_rate;
    let out_len = (mono_len as f64 * ratio).floor() as usize;

    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src = i as f64 / ratio;
        let idx = src.floor() as usize;
        let frac = (src - idx as f64) as f32;
        let s0 = samples.get(idx * channels).copied().unwrap_or(0.0);
        let s1 = samples.get((idx + 1) * channels).copied().unwrap_or(s0);
        out.push(s0 + (s1 - s0) * frac);
    }
    out
}

/// Останавливает захват (освобождает микрофон).
pub fn stop_capture(app: &AppHandle) {
    if let Some(engine) = app.try_state::<AudioEngine>() {
        *engine.stream.lock().unwrap() = None;
    }
}
