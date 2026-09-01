# VoiceBridge — архитектура

Проект состоит из трёх частей: десктоп-приложение (Rust + Tauri + React),
мобильное приложение (Flutter) и их связь по WebSocket (LAN).

```
[React (Tauri webview)]          [Rust/Tauri бэкенд]            [Flutter мобилка]
  src/ (FSD)                      src-tauri/src/                 mobile/lib/
      │                               │  ⇄ HTTP                     │
      │  invoke / listen              │  OpenCode server            │  ⇄ WebSocket
      └──────────────► commands ◄─────┘                             └────► mobile.rs (WS-сервер)
```

## Принцип

Код делится по **ответственности**, а не по типу файла. Общая цель — лёгкие
(до ~300 строк) и понятные модули. Крупные файлы, разросшиеся в процессе
разработки, выносятся в модули по доменам (рефакторинг — ветка `refactoring`).

## Десктоп — фронтенд (`src/`, React + TypeScript)

Используется **Feature-Sliced Design** (FSD). Слои сверху вниз: `app` →
`pages` → `features` → `shared`. Слои ниже не импортируют слои выше.

```
src/
  app/                        # инициализация и композиция
    App.tsx                   # по метке окна выбирает лаунчер или чат
  pages/                      # «страницы» = окна приложения
    launcher/                 # главное окно (лаунчер проектов/сессий)
      LauncherPage.tsx        # состояние + разводка
      SettingsOverlay.tsx     # модальное окно настроек
    chat/                     # окно чата (response-{sessionId})
      ChatPage.tsx            # состояние + диалог
      components/             # MessageBubble, ReasoningBlock, ActionCards, ToolChips
      toolIcon.ts
  features/                   # переиспользуемые фичи
    chat-input/ChatInput.tsx  # панель ввода
    git/                      # Git-панель изменений
      GitPanel.tsx
      GitDiffView.tsx
      gitFormat.ts
  shared/                     # общий слой (без бизнес-логики)
    types.ts                  # доменные типы (мост с бэкендом)
    lib/                      # format, hooks, sounds
    ui/                       # Markdown
```

Правила:
- `shared` не знает о `pages`/`features`; `features` не знает о `pages`.
- Презентационные компоненты (`components/`) получают данные через props.
- Вся связь с бэкендом — через `invoke` (команды) и `listen` (события).

## Десктоп — бэкенд (`src-tauri/src/`, Rust)

Модульная структура по доменам:

```
src-tauri/src/
  lib.rs                      # сборка: плагины, состояние, трей, хоткеи, handler
  commands/                   # Tauri-команды (мост в фронтенд)
    mod.rs                    # re-exports + shared (emit/save/load state)
    recording.rs              # запись/распознавание
    settings.rs               # модели, микрофон, язык, горячие клавиши
    sessions.rs               # сессии и проекты OpenCode
    chat.rs                   # окно чата, диалоги
    mobile.rs                 # мобильный доступ, устройства, скрытые проекты
    git.rs                    # Git-изменения
    system.rs                 # выход, обновления
  modules/                    # доменные модули
    audio.rs                  # cpal
    stt.rs                    # whisper
    git.rs                    # git status/diff
    mobile.rs                 # WS-сервер (axum)
    opencode/                 # интеграция с OpenCode
      mod.rs                  # shared (base_url, http_client, типы)
      discovery.rs            # обнаружение инстансов, создание сессий
      projects.rs             # проекты (запуск/остановка)
      sessions.rs             # выбор цели, send_prompt
      streaming.rs            # SSE-стрим, диалоги
      store.rs                # память сессий, broadcast git-changes
      usage.rs                # токены/стоимость, история
      actions.rs              # permission/question/abort
  state.rs                    # AppState, ConversationStore, типы
  logging.rs, hotkeys.rs, tray.rs
```

Правила:
- `commands` — тонкий слой: парсинг аргументов, блокировки, вызов `modules`.
- `modules` — логика без знания о Tauri-командах (кроме `AppHandle` для
  состояния/эмитов).
- Долгие операции (HTTP, whisper, git) — в `std::thread::spawn` /
  `tauri::async_runtime::spawn_blocking`, не в основном потоке.

## Мобилка (`mobile/lib/`, Flutter)

Повторяет слоистый подход: экраны (`screens/`), переиспользуемые виджеты
(`widgets/`), состояние (`app_state.dart`), модели (`models.dart`), тема
(`theme.dart`).

```
mobile/lib/
  app_state.dart              # AppController (ChangeNotifier, provider)
  models.dart                 # модели (повторяют src/shared/types.ts)
  theme.dart                  # тёмная тема (палитра десктопа)
  ws_client.dart              # WebSocket-клиент (команды/события)
  screens/                    # экраны навигации
    pairing_screen.dart
    sessions_screen.dart      # лаунчер
    chat_screen.dart
    git_screen.dart           # отдельный экран Git-изменений
  widgets/                    # переиспользуемые виджеты
    markdown_text.dart
    voicebridge_logo.dart
    project_card.dart         # карточка проекта/сессии лаунчера
    chat_widgets.dart         # пузыри, действия, инструменты, строка стоимости
```

## Связь мобилка ↔ десктоп

Протокол JSON поверх WebSocket (см. `docs/MOBILE.md`). Мобилка шлёт команды
(`type: "command"`), десктоп отвечает (`type: "response"`) и шлёт события
(`type: "event"`). Имена команд/событий повторяют Tauri-мост.
