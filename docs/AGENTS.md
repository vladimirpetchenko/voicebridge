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

Требования: Rust (rustup), Node 20+, cmake (whisper.cpp). macOS — Xcode CLT (clang,
Apple Silicon желателен для Metal-ускорения whisper). Windows — MSVC Build Tools
(workload `VCTools`, нужен C++ для whisper.cpp) и LLVM (`libclang.dll` для bindgen
whisper-rs: `LIBCLANG_PATH` на `<LLVM>\bin`). Первая сборка компилирует whisper.cpp
из исходников и занимает несколько минут.

## Где что лежит

Структура — Feature-Sliced Design (фронтенд) и модули по доменам (бэкенд,
мобилка). Подробности — `docs/ARCHITECTURE.md`.

- Фронтенд (`src/`, FSD):
  - `src/app/App.tsx` — корень: по метке окна выбирает лаунчер или чат.
  - `src/pages/launcher/` — `LauncherPage.tsx` (лаунчер), `SettingsOverlay.tsx`.
  - `src/pages/chat/` — `ChatPage.tsx` (окно чата) + `components/` (пузыри,
    размышления, карточки действий, чипы инструментов).
  - `src/features/chat-input/ChatInput.tsx` — панель ввода в чате.
  - `src/features/git/` — `GitPanel.tsx`, `GitDiffView.tsx`, `GitCommits.tsx`,
    `GitBranches.tsx`, `gitFormat.ts`.
  - `src/features/messages/` — `MessagePanel.tsx` (панель сообщений, навигация
    по чату).
  - `src/shared/` — `types.ts`, `lib/` (format, hooks, sounds), `ui/Markdown.tsx`.
- Бэкенд (`src-tauri/src/`):
  - `commands/` — Tauri-команды (мост в фронтенд), по доменам: `recording`,
    `settings`, `sessions`, `chat`, `mobile`, `git`, `system`; `mod.rs` — re-exports
    + `emit_state`/`save_state`/`load_state`.
  - `state.rs` — `AppState`, `ConversationStore`, типы.
  - `logging.rs` — файловый логгер + перехват паник.
  - `modules/audio.rs` — cpal, буфер, ресемплинг.
  - `modules/stt.rs` — whisper, модели, скачивание.
  - `modules/git.rs` — git status/diff/log/show/for-each-ref.
  - `modules/opencode/` — OpenCode (по подмодулям: `discovery`, `projects`,
    `sessions`, `streaming`, `store`, `usage`, `actions`).
  - `modules/mobile.rs` — мобильный доступ: WebSocket-сервер (axum), токен/QR,
    команды и broadcast событий.
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
  `get_git_changes`, `get_git_diff`, `get_git_commits`, `get_git_commit`,
  `get_git_branches`,
  `quit_app`.
- События (`listen`): `state-changed` (AppState), `audio-level` (number),
  `model-download-progress/-done/-error`, `model-loading/-loaded/-load-error`,
  `sessions-open-changed` (Vec<String> — id сессий с открытым окном чата),
  `opencode-user {sessionId,text}`, `opencode-delta {sessionId,text}`,
  `opencode-reasoning-delta {sessionId,text}`,
  `opencode-tool {sessionId,callId,name,state,input,output}`, `opencode-error {sessionId,error}`,
  `opencode-done {sessionId}`, `opencode-permission {sessionId,requestId,port,permission,patterns}`,
  `opencode-question {sessionId,requestId,port,questions}`,
  `git-changes {sessionId,branch,changes}`, `open-settings` (string — имя вкладки настроек).

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
8. **Windows: Basic-auth сервера OpenCode**. `opencode serve` требует HTTP
   Basic-авторизацию, если в окружении задан `OPENCODE_SERVER_PASSWORD`
   (username `OPENCODE_SERVER_USERNAME`, по умолчанию `opencode`). Десктопное
   приложение OpenCode задаёт эти переменные — поэтому на Windows сервер отвечает
   `401`, и без авторизации приложение считает его незапущенным («проект не
   стартует», «нет сессий»). `http_client()` в `modules/opencode/mod.rs` читает
   эти env и добавляет заголовок `Authorization: Basic …` во все запросы.
   На macOS переменных обычно нет — заголовок не добавляется, поведение прежнее.
9. **Windows: нормализация путей проектов**. `opencode db` отдаёт пути с `/`,
   а диалог выбора папки и `known_worktrees` — с `\`. Без нормализации один проект
   считался разными (дубли в лаунчере, дубли сессий). Все пути прогоняются через
   `normalize_worktree()` (`\` → `/`) в `modules/opencode/projects.rs`; регистрация
   в `commands/mod.rs` тоже нормализует. Не сравнивай пути строково без неё.
10. **Windows: кириллица в Git**. `core.quotepath` на Windows по умолчанию
    `true`, поэтому не-ASCII имена файлов в `git status`/`diff` выходят как
    octal-escape (`\321\202…`), файл не находится на диске (счётчик строк = 0).
    `run_git()` в `modules/git.rs` всегда передаёт `-c core.quotepath=false` —
    иначе кириллические имена «мусорятся»/теряются. На macOS настройка и так
    выключена.

## Конвенции

- Комментарии и строки UI — на русском.
- Статусы/режимы сериализуются через `#[serde(rename_all = "lowercase")]`.
- Долгие операции (HTTP, whisper, скачивание) — в `std::thread::spawn`, не в
  основном потоке. Результат/прогресс — через `app.emit(...)`.
- Иконка генерируется `python3 scripts/gen_icon.py` → `npx tauri icon`.

## Текущее состояние / TODO

Реализовано: трей (клик по иконке показывает/фокусирует окно, если оно скрыто
или не в фокусе; в меню — «Показать/Скрыть», подменю «Настройки» с вкладками и
«Выход»), хоткей, STT (whisper + модели), OpenCode (обнаружение,
сессии, проекты, новый инстанс с выбором папки, стриминг, история, действия
permission/question). Папки проектов, запущенные через «Новый проект» (ещё без
сессий в БД), запоминаются в `AppState.known_worktrees` и подмешиваются в
`list_projects` через `list_projects_with_extra` (чтобы не пропадали при
обновлении списка). Главное окно — лаунчер, окно чата — основной интерфейс:
шапка (название сессии + проект), диалог с markdown, панель ввода (текст +
отправка + голос с тап/удержание/авто-стоп), режимы отправки (сразу в чат /
предпросмотр), индикатор текущего режима в чате (клик — переключение). Окна
чата изолированы по сессиям: запись/индикатор/превью привязаны к сессии окна
(`recording_session_id`); открытые сессии трекаются в лаунчере
(`ConversationStore.open_sessions`, команда `list_open_session_ids`, событие
`sessions-open-changed`), при закрытии окна выбор сбрасывается. В чате —
запросы действий (разрешения/вопросы) в «доке» над полем ввода (всегда на виду),
время выполнения ответа (реалтайм + замирает по завершении, `formatDuration`),
строка статуса над инпутом («работает…» / «выполняет `tool`…») и предупреждение
«возможно, завис» (нет событий 90 с, `STALL_THRESHOLD_MS`). Git-панель с тремя
вкладками: «Изменения» (файлы, сгруппированные по папкам `groupChangesByDir`,
дифф по клику), «История» (список коммитов `git log` — хэш/автор/дата/сообщение;
клик по коммиту — изменённые файлы и дифф коммита `git show`, компоненты
 `GitCommits.tsx`) и «Ветки» (локальные ветки `git for-each-ref`, текущая с
бейджем «текущая», компонент `GitBranches.tsx`).
`git status --untracked-files=all` разворачивает неотслеживаемые папки в файлы.
Слева — скрываемая панель сообщений (`features/messages/MessagePanel.tsx`):
список запросов пользователя с превью, клик — переход к сообщению (скролл +
подсветка, `data-msg-idx`/`scrollIntoView`); на узком окне — шторка слева. Ширина
панели тянется за край (как Git-панель). Git-панель и панель сообщений
переключаются кнопками в шапке (рядом справа); Git-панель скрыта/показана
состоянием `gitPanelVisible` (широкое окно) / `gitPanelOpen` (узкое).
История из OpenCode склеивает размышления (reasoning) с итоговым текстом одного
ответа. Голос: `[BLANK_AUDIO]` фильтруется (показ «Ничего не распознано»), пустое
имя микрофона = «по умолчанию» (None), статус не зависает в «Распознавание…» в
режиме предпросмотра; на голосовой кнопке — спиннер во время распознавания
(`isProcessing` в `ChatInput`); в лог пишется `peak` амплитуды захваченного звука
для диагностики прав микрофона. В мобилке — паритет: док запросов действий над
полем ввода, размышления в рамках сообщения, Git-изменения по папкам, история
коммитов и список веток (переключатель «Изменения»/«История»/«Ветки» на экране
Git, отдельный экран коммита). Обнаружение/
остановка OpenCode работает и на Windows (`netstat`+`tasklist` для портов/PID,
`taskkill` для остановки, поиск бинаря через `env::var("PATH")` + `Path::is_file()`
— НЕ через `where`, т.к. он отдаёт пути в OEM-кодировке и ломает кириллицу;
`cmd /C` для npm-шимов `.cmd`/`.bat`), подсветка синтаксиса в markdown
(`rehype-highlight` + тёмная
тема hljs), иконки `lucide-react` вместо emoji, звуки записи/отправки/ответа и
анимация «думает» в чате. Чипы инструментов показывают вход/выход вызова
(`state.input`/`state.output` из `message.part.updated`, обрезаются до 2000
символов на бэкенде): на десктопе — hover-попап + inline-превью вывода, на
мобилке — tooltip по долгому нажатию. Внизу окна чата — строка состояния (токены сессии,
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
reply_question/reject_question/register_device/get_git_changes/get_git_diff/
get_git_commits/get_git_commit/get_git_branches`, трансляция `state-changed`,
`devices-changed` и `opencode-*`).
Flutter-клиент в `mobile/` (этапы 2–3 готовы): пейринг по QR/ручному вводу,
лаунчер проектов с сессиями («Новая сессия», скрытие/возврат проектов,
3 последних сессии + раскрывашка), чат с markdown (подсветка кода), стримом и
инструментами, карточки разрешений/вопросов, строка токенов/стоимости,
полная история из OpenCode, авто-переподключение. Брендинг как у десктопа:
иконка-«волна», шрифт Fira Code, циановая палитра. Осталось — этап 4 (вне LAN:
Tailscale/ZeroTier или облачный relay, пуши).

CI (`.github/workflows/build.yml`): сборка десктопа (`.exe`/`.dmg`) и Android
APK (Flutter, job `android` на `ubuntu-latest`, debug-подпись) по тегу `v*` или
вручную.

## Планы

Выполнено:

- ✅ **Рефакторинг под современную архитектуру** — Feature-Sliced Design на
  фронтенде (`app/`, `pages/`, `features/`, `shared/`), модули по доменам на
  бэкенде (`commands/`, `modules/opencode/`), виджеты мобилки, CSS по FSD.
  Ветка `refactoring`.
- ✅ **Git-панель изменений** (десктоп + мобилка) — список изменённых файлов
  (сгруппированы по папкам), дифф по клику, ветка, responsive (панель/оверлей),
  отдельный экран в мобилке. Модуль `src-tauri/src/modules/git.rs`; команды
  `get_git_changes`/`get_git_diff` (десктоп) и WS-команды (мобилка), событие
  `git-changes`.
- ✅ **Список коммитов в Git-панели** (десктоп + мобилка) — вкладка «История»
  в Git-панели: список коммитов (`git log` — хэш/автор/дата/сообщение), клик по
  коммиту — изменённые файлы и дифф (`git show`). Модуль
  `src-tauri/src/modules/git.rs` (типы `GitCommit`/`GitCommitDetail`,
  `recent_commits`/`commit`); команды `get_git_commits`/`get_git_commit`
  (десктоп) и WS-команды (мобилка). Фронтенд — `GitCommits.tsx` + вкладки в
  `GitPanel`; мобилка — переключатель в `git_screen.dart` + экран `GitCommitScreen`.
- ✅ **Просмотр веток в Git-панели** (десктоп + мобилка) — вкладка «Ветки»:
  список локальных веток (`git for-each-ref refs/heads`), текущая помечена
  бейджем «текущая». Модуль `modules/git.rs` (тип `GitBranch`, `branches()`);
  команда `get_git_branches` (десктоп) и WS-команда (мобилка). Фронтенд —
  `GitBranches.tsx`; мобилка — вкладка в `git_screen.dart` (`_BranchTile`).
- ✅ **Панель сообщений (десктоп)** — скрываемая левая панель со списком
  запросов пользователя (превью), клик — переход к сообщению (скролл +
  подсветка); на узком окне — шторка слева. Компонент
  `features/messages/MessagePanel.tsx`, переключение из шапки (кнопка слева),
  ширина тянется за край.

Далее:

1. **Баг: теряется ответ в мобилке** — если отправить запрос, свернуть
   приложение и снова открыть, стриминговый ответ теряется (соединение/подписка
   на `opencode-*` обрывается при сворачивании). Нужно восстановить ответ из
   истории OpenCode при возврате в приложение.
2. **Скролл чата** — не автоскроллить вниз, пока ответ OpenCode продолжает
   стримиться: позиция «замирает» (можно читать историю); к низу — по явному
   действию/кнопке.
3. **Управление сессиями** — удаление сессии из лаунчера (десктоп + мобилка) и
   автоматическое название: если сессия была пустой (без заголовка), после
   первого сообщения подставлять осмысленное имя.
4. **GUI-автоматизация** — вставка распознанного текста в выбранное окно.
   Объёмная задача, отложена на самый конец: системные API для списка окон
   (macOS `CGWindowListCopyWindowInfo`, Windows `EnumWindows`), вставка текста
   (симуляция нажатий клавиш / буфер обмена + Ctrl/Cmd+V / Accessibility API).
   В текущей сборке не реализована (модуль `automation.rs` и режим «GUI» удалены).
