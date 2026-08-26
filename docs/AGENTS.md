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

- Фронтенд:
  - `src/App.tsx` — главное окно `MainApp` (лаунчер: проекты + сессии) и окно
    чата `ResponseView` (шапка с названием сессии/проекта, диалог, инструменты,
    действия).
  - `src/ChatInput.tsx` — панель ввода в чате: текстовое поле + отправка +
    голосовая кнопка (тап/удержание/тишина), режимы отправки, индикатор записи.
  - `src/Markdown.tsx` — рендер markdown (подсветка синтаксиса).
  - `src/sounds.ts` — звуки интерфейса (Web Audio API, синтез без файлов).
- Бэкенд:
  - `src-tauri/src/commands.rs` — все `#[tauri::command]` (мост в фронтенд).
  - `src-tauri/src/state.rs` — `AppState`, `ConversationStore`, типы.
  - `src-tauri/src/logging.rs` — файловый логгер + перехват паник.
  - `src-tauri/src/modules/audio.rs` — cpal, буфер, ресемплинг.
  - `src-tauri/src/modules/stt.rs` — whisper, модели, скачивание.
  - `src-tauri/src/modules/opencode.rs` — OpenCode (обнаружение, сессии,
    проекты, стриминг SSE, диалоги, permission/question).
  - `src-tauri/src/modules/mobile.rs` — мобильный доступ: встроенный
    WebSocket-сервер (axum), токен/QR, команды и broadcast событий.
- Проверочные бинарники — `src-tauri/examples/*.rs` (запуск:
  `cargo run --example opencode_check`).

## Команды и события (мост Rust ↔ фронтенд)

- Команды (`invoke`): `get_app_state`, `toggle_recording`,
  `start_recording`, `stop_recording`, `list_microphones`, `select_microphone`,
  `set_sensitivity`, `set_silence_timeout`, `set_send_mode`, `set_hotkey`,
  `send_text`, `list_opencode_sessions`,
  `select_opencode_session`, `select_opencode_instance`, `list_projects`,
  `create_session`, `hide_project`, `unhide_project`,
  `start_project`, `stop_project`, `get_models`, `download_model`,
  `select_stt_model`, `set_language`, `open_response_window`,
  `get_conversation`, `get_session_info`, `get_session_usage`,
  `abort_session`, `close_response_window`,
  `list_open_session_ids`, `reply_permission`, `reply_question`,
  `reject_question`, `get_opencode_binary`, `check_update`,
  `get_mobile_info`, `set_mobile_enabled`, `regenerate_mobile_token`,
  `quit_app`.
- События (`listen`): `state-changed` (AppState), `audio-level` (number),
  `model-download-progress/-done/-error`, `model-loading/-loaded/-load-error`,
  `sessions-open-changed` (Vec<String> — id сессий с открытым окном чата),
  `opencode-user {sessionId,text}`, `opencode-delta {sessionId,text}`,
  `opencode-reasoning-delta {sessionId,text}`,
  `opencode-tool {sessionId,callId,name,state}`, `opencode-error {sessionId,error}`,
  `opencode-done {sessionId}`, `opencode-permission {sessionId,requestId,port,permission,patterns}`,
  `opencode-question {sessionId,requestId,port,questions}`.

Tauri сам мапит camelCase (JS) ↔ snake_case (Rust) в аргументах команд.

## Критичные грабли (не наступай снова)

1. **SDKROOT + MACOSX_DEPLOYMENT_TARGET**: в `src-tauri/.cargo/config.toml`
   принудительно `SDKROOT=""` (иначе при `SDKROOT=iPhoneOS` FindBLAS падает) и
   `MACOSX_DEPLOYMENT_TARGET=10.15` (иначе в новых Xcode whisper.cpp падает на
   `std::filesystem`, см. CI). Не убирай эти настройки.
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

Реализовано: трей, хоткей, STT (whisper + модели), OpenCode (обнаружение,
сессии, проекты, новый инстанс с выбором папки, стриминг, история, действия
permission/question). Главное окно — лаунчер, окно чата — основной интерфейс:
шапка (название сессии + проект), диалог с markdown, панель ввода (текст +
отправка + голос с тап/удержание/авто-стоп), режимы отправки (сразу в чат /
предпросмотр), индикатор текущего режима в чате (клик — переключение). Окна
чата изолированы по сессиям: запись/индикатор/превью привязаны к сессии окна
(`recording_session_id`); открытые сессии трекаются в лаунчере
(`ConversationStore.open_sessions`, команда `list_open_session_ids`, событие
`sessions-open-changed`), при закрытии окна выбор сбрасывается. Обнаружение/
остановка OpenCode работает и на Windows (`netstat`+`tasklist` для портов/PID,
`taskkill` для остановки, `where` для поиска бинаря, `cmd /C` для npm-шимов
`.cmd`/`.bat`), подсветка синтаксиса в markdown (`rehype-highlight` + тёмная
тема hljs), иконки `lucide-react` вместо emoji, звуки записи/отправки/ответа и
анимация «думает» в чате. Внизу окна чата — строка состояния (токены сессии,
стоимость) из `GET /session/{id}`. В футере лаунчера показывается модель
OpenCode выбранной сессии (`opencode_model` из `/session`). Горячая клавиша
записи настраивается («Настройки → Горячие клавиши», команда `set_hotkey`).
Подключён `tauri-plugin-updater` (базовый конфиг в
`tauri.conf.json`: endpoint — GitHub Releases `latest.json`, публичный ключ
вшит; команда `check_update` + кнопка в «О программе»). Ключ подписи лежит в
`src-tauri/voicebridge-signing.key` (gitignored); в CI передаётся через
`TAURI_SIGNING_PRIVATE_KEY`.

Не сделано: автообновления подключены частично: `createUpdaterArtifacts`
выключен (иначе сборка требует ключ), не хватает секрета `TAURI_SIGNING_PRIVATE_KEY`
в CI, подписи/нотаризации `.app` под macOS, сборки `.msi`/NSIS под Windows и
публикации артефактов + манифеста `latest.json` в релиз.

Мобильное приложение (Flutter, управление десктопом по LAN через WebSocket) —
спроектировано и расписано в `docs/MOBILE.md` (ветка `feature/mobile-app`).
Десктопная часть готова: встроенный WS-сервер в `modules/mobile.rs` (axum,
порт 47800, токен/QR, команды `ping/list_sessions/list_projects/start_project/
stop_project/create_session/hide_project/unhide_project/select_session/
send_prompt/abort/get_conversation/get_state/get_session_usage/reply_permission/
reply_question/reject_question/register_device`, трансляция `state-changed`,
`devices-changed` и `opencode-*`).
Flutter-клиент в `mobile/` (этапы 2–3 готовы): пейринг по QR/ручному вводу,
лаунчер проектов с сессиями («Новая сессия», скрытие/возврат проектов,
3 последних сессии + раскрывашка), чат с markdown (подсветка кода), стримом и
инструментами, карточки разрешений/вопросов, строка токенов/стоимости,
полная история из OpenCode, авто-переподключение. Брендинг как у десктопа:
иконка-«волна», шрифт Fira Code, циановая палитра. Осталось — этап 4 (вне LAN:
Tailscale/ZeroTier или облачный relay, пуши).

## Планы (отложено на самый конец)

GUI-автоматизация — вставка распознанного текста в выбранное окно. Объёмная
задача, отложена на самый конец: системные API для списка окон (macOS
`CGWindowListCopyWindowInfo`, Windows `EnumWindows`), вставка текста (симуляция
нажатий клавиш / буфер обмена + Ctrl/Cmd+V / Accessibility API). В текущей
сборке не реализована (модуль `automation.rs` и режим «GUI» удалены).
