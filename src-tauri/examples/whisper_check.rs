//! Проверка работы whisper: загружает модель и транскрибирует тестовый сигнал.
//! Использование: cargo run --example whisper_check -- <путь к ggml-модели>

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

fn main() {
    let path = std::env::args().nth(1).expect("укажите путь к модели");
    let mut params = WhisperContextParameters::new();
    params.use_gpu(cfg!(target_os = "macos"));
    println!("загрузка модели…");
    let ctx = WhisperContext::new_with_params(std::path::Path::new(&path), params)
        .expect("не удалось загрузить модель");

    let mut state = ctx.create_state().expect("create_state");

    let rate = 16000usize;
    let samples: Vec<f32> = (0..rate)
        .map(|i| (i as f32 * 440.0 * std::f32::consts::TAU / rate as f32).sin() * 0.4)
        .collect();

    let mut fp = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    fp.set_language(Some("en"));
    fp.set_translate(false);
    fp.set_print_special(false);
    fp.set_print_progress(false);
    fp.set_print_realtime(false);
    fp.set_print_timestamps(false);
    fp.set_no_context(true);

    println!("транскрипция…");
    state.full(fp, &samples).expect("full");

    let n = state.full_n_segments();
    println!("сегментов: {n}");
    for i in 0..n {
        if let Some(seg) = state.get_segment(i) {
            println!("[{i}] {:?}", seg.to_str_lossy());
        }
    }
    println!("OK");
}
