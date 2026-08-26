# VoiceBridge Mobile — мобильное приложение (план и протокол)

Мобильное приложение для управления VoiceBridge с телефона. Десктоп-приложение
(Tauri/Rust) работает как **мост**: мобилка шлёт команды на него, он передаёт их
в OpenCode и транслирует ответ (стрим, инструменты, действия) обратно на мобилку.

Стек мобилки: **Flutter** (Dart). Связь: **локальная сеть (LAN)** по WebSocket,
без облака. Позже, для работы вне одной сети, рассмотрим Tailscale/ZeroTier.

## Статус

- ✅ **Этап 0–1 (десктоп) готов** — встроенный WebSocket-сервер, токен/QR,
  диспетчеризация команд и трансляция событий. Протестировано вручную.
- ⏳ **Этап 2 (Flutter-клиент) — текущая задача.** Всё на ветке
  `feature/mobile-app`.

## Архитектура

```
[Flutter мобилка] ⇄ WebSocket (LAN) ⇄ [Tauri/Rust десктоп] ⇄ [OpenCode сервер]
     клиент                            мост + WS-сервер          (как сейчас)
```

Десктоп уже делает всю тяжёлую работу (обнаружение сессий, отправка промптов,
стриминг SSE, инструменты, разрешения/вопросы, abort, стоимость). Нужно только
«вывернуть» существующие `invoke`-команды и `listen`-события наружу по сети.

## Транспорт

- **Протокол**: один WebSocket на команды (мобилка → десктоп) и события
  (десктоп → мобилка).
- **Адрес**: `ws://<ip-десктопа>:47800/?token=<token>`.
- **Healthcheck**: `GET /health` → `ok` (без авторизации).
- **Авторизация**: токен из query-параметра `token`. Неверный токен → `401`,
  мобильный доступ выключен → `403`.

## Пейринг (QR)

1. На десктопе (настройки → «Мобильный доступ») — тумблер «Принимать команды».
2. Десктоп генерирует случайный токен и показывает QR:
   `ws://192.168.1.10:47800/?token=<token>`.
3. Мобилка сканирует QR, сохраняет адрес + токен и подключается.
4. Десктоп принимает только соединения с верным токеном (можно добавить явное
   подтверждение на десктопе).

Токен хранится на мобилке (secure storage), при следующем запуске подключается
сразу. Регенерация токена — кнопка на десктопе.

## Протокол (JSON поверх WebSocket)

Все сообщения — JSON-объекты. Команды идут от мобилки, ответы/события — от
десктопа. Имена полей camelCase (как в существующем мосте Tauri).

### Команды (мобилка → десктоп)

Команда — JSON-объект с `type: "command"`, уникальным `id` (строка, для
сопоставления с ответом) и `name`. Остальные поля — аргументы **плоско** в этом же
объекте (не вложены в `args`).

```jsonc
{ "type": "command", "id": "1", "name": "ping" }
{ "type": "command", "id": "2", "name": "list_sessions" }
{ "type": "command", "id": "3", "name": "get_state" }
{ "type": "command", "id": "4", "name": "select_session", "instanceId": "port-4149", "port": 4149, "sessionId": "ses_…", "title": "…", "model": "…" }
{ "type": "command", "id": "5", "name": "send_prompt", "text": "сделай …" }
{ "type": "command", "id": "6", "name": "send_prompt", "text": "…", "sessionId": "ses_…" }
{ "type": "command", "id": "7", "name": "abort" }
{ "type": "command", "id": "8", "name": "abort", "sessionId": "ses_…" }
{ "type": "command", "id": "9", "name": "get_conversation", "sessionId": "ses_…" }
```

- `list_sessions` → массив `OpenCodeInstance` (инстансы с `port` и `sessions`,
  у сессии есть `id/title/model/updatedAt`).
- `select_session` — обязателен `sessionId`; `port`/`instanceId`/`title`/`model`
  берутся из данных `list_sessions`.
- `send_prompt` — без `sessionId` шлёт в выбранную сессию; с `sessionId` — в неё.
- `abort` — прерывает генерацию (без `sessionId` — выбранную сессию).

### Ответы на команды (десктоп → мобилка)

```jsonc
{ "type": "response", "id": "1", "ok": true, "data": { /* … */ } }
{ "type": "response", "id": "1", "ok": false, "error": "текст ошибки" }
```

### События (десктоп → мобилка, аналог текущих `listen`)

```jsonc
{ "type": "event", "name": "state-changed", "data": { /* AppState */ } }
{ "type": "event", "name": "sessions-open-changed", "data": ["ses_…"] }
{ "type": "event", "name": "opencode-user",     "data": { "sessionId": "…", "text": "…" } }
{ "type": "event", "name": "opencode-delta",    "data": { "sessionId": "…", "text": "…" } }
{ "type": "event", "name": "opencode-tool",     "data": { "sessionId": "…", "callId": "…", "name": "…", "state": "…" } }
{ "type": "event", "name": "opencode-done",     "data": { "sessionId": "…" } }
{ "type": "event", "name": "opencode-error",    "data": { "sessionId": "…", "error": "…" } }
{ "type": "event", "name": "opencode-permission","data": { "sessionId": "…", "requestId": "…", "port": 0, "permission": "…", "patterns": [] } }
{ "type": "event", "name": "opencode-question", "data": { "sessionId": "…", "requestId": "…", "port": 0, "questions": [] } }
```

Мобилка не обязана понимать все события сразу — на первом этапе достаточно
`opencode-user/-delta/-done/-error` для чата.

## Что уже реализовано в десктопе (этапы 0–1)

- **`src-tauri/src/modules/mobile.rs`** — встроенный WS-сервер на axum, слушает
  `0.0.0.0:47800`, роуты `/` (ws) и `/health`. Авторизация по токену, диспетчеризация
  команд (вызывает функции из `commands.rs`/`opencode.rs`), broadcast-канал событий.
- **Трансляция событий**: `emit_state` (`state-changed`) и все `opencode-*`
  (`user/delta/tool/done/error/permission/question`) шлются и в окна Tauri, и
  мобильным клиентам (через `emit_and_broadcast`).
- **Состояние**: `AppState` — `mobile_enabled`, `mobile_port`, `mobile_token`
  (токен генерируется один раз, сохраняется в `state.json`).
- **Команды Tauri**: `get_mobile_info` (ip/port/token/uri/qrSvg),
  `set_mobile_enabled`, `regenerate_mobile_token`.
- **UI**: вкладка «Настройки → Мобильный доступ» — тумблер + QR-код + адрес.

Зависимости (Cargo.toml): `axum` (ws), `tokio`, `futures-util`, `rand`,
`qr_code`, `local-ip-address`.

## Как протестировать десктоп без мобилки

Сервер поднимается вместе с десктопом (`npm run tauri dev`). Проверить можно так:

```bash
# healthcheck
curl http://127.0.0.1:47800/health          # -> ok

# рукопожатие с токеном (без токена — 401, при выключенном доступе — 403)
# токен лежит в state.json (поле mobileToken), либо в UI «Мобильный доступ»
curl -i -H "Connection: Upgrade" -H "Upgrade: websocket" \
  -H "Sec-WebSocket-Version: 13" -H "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==" \
  "http://127.0.0.1:47800/?token=TOKEN"
```

Для полноценного обмена по WS без установки клиента можно написать мини-клиент
(Python `socket` + ручной фрейминг) — в сессии разработки это уже делалось и
работало: `ping`/`list_sessions`/`select_session` отвечают, после `select_session`
приходит событие `state-changed`.

## Что построить на мобилке (Flutter) — этап 2

- Проект в `mobile/` (пакет `voicebridge_mobile`).
- Зависимости: `web_socket_channel`, `mobile_scanner` (QR), `flutter_secure_storage`
  (токен), `provider`/`riverpod` (состояние).
- Экраны:
  - **Пейринг** — скан QR, сохранение адреса/токена, подключение.
  - **Сессии** (лаунчер) — список экземпляров/сессий, выбор сессии.
  - **Чат** — сообщения, стрим, инструменты, кнопка «Стоп», строка состояния
    (токены/стоимость).
- Dart-модели повторяют типы из `src/types.ts`.

### Советы для Flutter-клиента

- QR-код на десктопе кодирует строку `ws://<ip>:47800/?token=<token>` — её можно
  парсить как `Uri`, извлекая `host`, `port` и `queryParameters['token']`.
- `web_socket_channel` подключается к `Uri` напрямую (`WebSocketChannel.connect(uri)`).
- Один WS-канал на приложение; команды шлются с возрастающим `id`, ответы
  сопоставляются по `id`. Входящие `type: "event"` раздаются подписчикам по `name`.
- Поток чата: `send_prompt` → события `opencode-user`, `opencode-delta` (доклеивать
  в последний ответ), `opencode-tool`, `opencode-done`. Стоимость/токены — команда
  `get_state` (поле `response` — последний ответ, но полная история — `get_conversation`).

## Безопасность

- Токен при каждом подключении; на десктопе — только явно включённый режим.
- Не пробрасывать порт в интернет; для удалённого доступа использовать
  Tailscale/ZeroTier (шифрованный оверлей) вместо порт-форвардинга.
- Удалённо доступен ограниченный набор команд (без `quit`, без смены настроек
  безопасности).

## Поэтапный план

1. ✅ **Этап 0 — сервер и протокол (десктоп)**: WS-сервер, токен, QR, healthcheck,
   рассылка событий.
2. ✅ **Этап 1 — мост команд**: `list_sessions`, `select_session`, `send_prompt`,
   `abort`; трансляция `opencode-*` событий на мобилку.
3. ⏳ **Этап 2 — мобилка (MVP)**: пейринг по QR, лаунчер сессий, чат со стримом.
4. **Этап 3 — действия**: разрешения/вопросы, стоимость, retry/reconnect,
   сохранение пары «устройство».
5. **Этап 4 — вне LAN**: Tailscale/ZeroTier (или облачный relay) для доступа
   из любой сети, пуши.

## Заметки

- Протокол намеренно повторяет существующие команды/события — так десктопный
  мост получается тонким (обёртка над уже готовой логикой).
- Порт/токен держим в `AppState` и сохраняем в `state.json`, чтобы пара
  «мобилка ↔ десктоп» переживала перезапуск.
