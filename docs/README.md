# VoiceBridge — документация проекта

Голосовой ассистент для разработчиков. Десктоп-приложение (macOS + Windows) на
стеке Rust + Tauri, живёт в системном трее. Распознаёт речь локально (без
интернета) и управляет OpenCode или вставляет текст в приложение.

## Режимы работы

1. **OpenCode** — наговоренный текст отправляется как промпт в выбранную сессию
   OpenCode, ответ стримится в интерфейс.
2. **GUI-автоматизация** — текст вставляется в выбранное окно (пока заглушка).

## Стек

- **Бэкенд**: Rust, Tauri 2 (`src-tauri/`).
- **Фронтенд**: React 19 + TypeScript + Vite 7 (`src/`).
- **Захват звука**: `cpal` 0.18.
- **Распознавание речи**: `whisper.cpp` через `whisper-rs` 0.16 (Metal на Apple
  Silicon).
- **HTTP**: `reqwest` (blocking + json).
- **Markdown**: `react-markdown` + `remark-gfm`.

## Структура

```
src/                        фронтенд (React + TS)
  App.tsx                   всё приложение (главное окно + окно ответа)
  Markdown.tsx              рендер markdown
  types.ts                  типы (общие с бэкендом)
  styles.css                стили (тёмная тема)
src-tauri/src/              бэкенд
  lib.rs                    сборка: плагины, состояние, трей, хоткеи, команды
  commands.rs               Tauri-команды (мост фронтенд ↔ модули)
  state.rs                  AppState, ConversationStore, типы
  modules/audio.rs          захват cpal, буфер, ресемплинг в 16 кГц
  modules/stt.rs            whisper, реестр моделей, скачивание, поток STT
  modules/opencode.rs       интеграция с OpenCode (обнаружение, сессии,
                            проекты, стриминг, диалоги)
  modules/automation.rs     GUI-автоматизация (заглушка)
src-tauri/examples/         whisper_check, opencode_check, projects_check
scripts/gen_icon.py         генерация иконки
```

## Поток данных

1. Запись: `cpal` пишет сэмплы в буфер + считает RMS-уровень → событие
   `audio-level` (волна в UI).
2. Остановка → буфер ресемплится в 16 кГц mono → задание в STT-поток.
3. STT-поток (владеет `WhisperContext`) транскрибирует → `finish_transcription`.
4. В режиме OpenCode → `send_prompt` → `POST /session/{id}/message` + чтение
   SSE `GET /event` → стриминг ответа.

## Запись

Кнопка записи: **тап** = переключение (старт/стоп), **удержание** = запись пока
держишь (отпустил — стоп). Есть авто-остановка по тишине (настройка в
«Настройки → Микрофон»).

## OpenCode

Подробности API и типы событий — см. `docs/OPENCODE.md`.
Коротко:

- Обнаружение: процессы `opencode` через `lsof` + порты 4096/12000/3000/17000.
- Сессии: `GET /session`; история: `GET /session/{id}/message`.
- Отправка: `POST /session/{id}/message`; стрим — SSE `GET /event`.
- Проекты: список из БД (`opencode db`), запуск/остановка `opencode serve`
  из приложения (стабильный порт по хэшу пути).

## Сборка и запуск

```bash
npm install
npm run tauri dev     # dev
npm run tauri build   # релиз (.dmg / .exe)
```

Требования: Rust (rustup), Node 20+, cmake (для whisper.cpp), Xcode CLT
(macOS) или MSVC Build Tools (Windows). Первая сборка компилирует whisper.cpp
(несколько минут).

## Известные особенности / грабли

- `src-tauri/.cargo/config.toml` принудительно ставит `SDKROOT=""` — иначе при
  `SDKROOT=iPhoneOS` сборка whisper.cpp падает на FindBLAS.
- Типы событий SSE OpenCode **не совпадают** с OpenAPI-схемой (см.
  `docs/OPENCODE.md`).
- Обычный TUI `opencode` не открывает TCP-порт — нужно `opencode serve` или
  `opencode web`.
- Capability `windows: ["*"]` обязательна, чтобы динамические окна
  `response-{sessionId}` получали события.
