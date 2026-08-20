# AGENTS.md — инструкция для агентов (AI)

Ты работаешь над **VoiceBridge** — голосовым ассистентом для разработчиков
(Rust + Tauri). Это руководство — что и где находится, как собирать, чего
избегать.

## Как собрать и запустить

```bash
npm install
npm run tauri dev        # dev-режим (vite + cargo run)
npm run tauri build      # релиз

# Rust отдельно (в src-tauri/):
cargo check              # быстрая проверка
cargo build              # полная сборка
# Фронтенд отдельно (в корне):
npm run build            # tsc + vite build
```

Требования: Rust (rustup), Node 20+, cmake (whisper.cpp), Xcode CLT (macOS).

## Где что лежит

- Фронтенд целиком — `src/App.tsx` (это один большой файл: главное окно
  `MainApp`, окно ответа `ResponseView`). Компонент markdown — `src/Markdown.tsx`.
- Бэкенд:
  - `src-tauri/src/commands.rs` — все `#[tauri::command]` (мост в фронтенд).
  - `src-tauri/src/state.rs` — `AppState`, `ConversationStore`, типы.
  - `src-tauri/src/modules/audio.rs` — cpal, буфер, ресемплинг.
  - `src-tauri/src/modules/stt.rs` — whisper, модели, скачивание.
  - `src-tauri/src/modules/opencode.rs` — OpenCode (обнаружение, сессии,
    проекты, стриминг SSE, диалоги).
- Проверочные бинарники — `src-tauri/examples/*.rs` (запуск:
  `cargo run --example opencode_check`).

## Команды и события (мост Rust ↔ фронтенд)

- Команды (`invoke`): `get_app_state`, `set_mode`, `toggle_recording`,
  `start_recording`, `stop_recording`, `list_microphones`, `select_microphone`,
  `set_sensitivity`, `set_silence_timeout`, `list_opencode_sessions`,
  `select_opencode_session`, `select_opencode_instance`, `list_projects`,
  `start_project`, `stop_project`, `get_models`, `download_model`,
  `select_stt_model`, `set_language`, `open_response_window`,
  `get_conversation`, `close_response_window`, `quit_app`.
- События (`listen`): `state-changed` (AppState), `audio-level` (number),
  `model-download-progress/-done/-error`, `model-loading/-loaded/-load-error`,
  `opencode-user {sessionId,text}`, `opencode-delta {sessionId,text}`,
  `opencode-tool {sessionId,callId,name,state}`, `opencode-error {sessionId,error}`,
  `opencode-done {sessionId}`.

Tauri сам мапит camelCase (JS) ↔ snake_case (Rust) в аргументах команд.

## Критичные грабли (не наступай снова)

1. **SDKROOT**: в `src-tauri/.cargo/config.toml` принудительно `SDKROOT=""`.
   На машине автора глобальный `SDKROOT` указывал на iPhoneOS SDK, из-за чего
   сборка whisper.cpp падала на `FindBLAS` (`BLAS not found`). Не убирай эту
   настройку.
2. **SSE OpenCode**: реальные типы событий — `message.part.delta`,
   `message.part.updated`, `session.idle` (см. `docs/OPENCODE.md`), а НЕ
   `session.next.*` из OpenAPI-доки.
3. **TUI opencode не слушает TCP** — только `opencode serve`/`web` доступны по
   HTTP.
4. **Capability**: `capabilities/default.json` использует `"windows": ["*"]`,
   чтобы динамические окна `response-{sessionId}` могли `listen` события.
   Не сужай без надобности.
5. **cpal 0.18**: `default_host()` возвращает `Host` (не `Result`), имя устройства
   — `device.to_string()` (метода `.name()` нет), `SampleFormat` — `#[non_exhaustive]`
   (есть I24/U24/DSD), `config.sample_rate` — это `u32`.
6. **whisper-rs**: `WhisperContext` не `Send`/`Sync` — держи его только в потоке
   `stt-worker` (см. `modules/stt.rs`).
7. **Блокировки**: при работе с `SharedState` сначала бери `let state =
   app.state::<SharedState>();` затем `state.0.lock()` (иначе E0716 — временный
   `State` дропается). Не держи `Mutex<AppState>` при вызове функций, которые
   берут другие блокировки.

## Конвенции

- Комментарии и строки UI — на русском.
- Статусы/режимы сериализуются через `#[serde(rename_all = "lowercase")]`.
- Долгие операции (HTTP, whisper, скачивание) — в `std::thread::spawn`, не в
  основном потоке. Результат/прогресс — через `app.emit(...)`.
- Иконка генерируется `python3 scripts/gen_icon.py` → `npx tauri icon`.

## Текущее состояние / TODO

Реализовано: трей, хоткей, запись (тап/удержание + авто-стоп по тишине), STT
(whisper + модели), OpenCode (обнаружение, сессии, проекты, стриминг, история,
развёрнутое окно с markdown).

Не сделано: GUI-автоматизация (`modules/automation.rs` — заглушка, нужны
системные API: macOS CGWindowList, Windows EnumWindows), Qwen3-ASR (whisper.cpp
не поддерживает), подсветка синтаксиса в markdown, автообновления,
Windows-обнаружение OpenCode (сейчас `lsof` — только macOS/Linux).
