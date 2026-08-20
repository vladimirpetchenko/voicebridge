# OpenCode API (проверено на opencode 1.18.18)

Эта информация получена вручную (поднятием `opencode serve` и чтением
OpenAPI `/doc` + живого SSE-потока). **Не доверяй только OpenAPI-схеме —
реальные типы SSE-событий отличаются** (см. ниже).

## Запуск сервера

```bash
opencode serve --port 4096 --hostname 127.0.0.1   # headless
opencode web                                      # сервер + веб-интерфейс
opencode attach http://127.0.0.1:4096             # TUI поверх сервера
```

Обычный `opencode` (TUI) запускает сервер in-process и **не открывает
TCP-порт** — к нему подключиться нельзя.

## Endpoints

| Метод | Путь | Назначение |
|-------|------|-----------|
| GET | `/session` | список сессий (в рамках проекта сервера) |
| POST | `/session` | создать сессию (`{"title":"..."}`) |
| GET | `/session/{id}` | детали сессии |
| POST | `/session/{id}/message` | отправить промпт `{"parts":[{"type":"text","text":"..."}]}` |
| GET | `/session/{id}/message` | история сообщений |
| GET | `/event` | SSE-поток событий |
| GET | `/project/current` | `{id, worktree, vcs, time}` — корень проекта |
| GET | `/config` | конфигурация |
| GET | `/global/health` | `{healthy, version}` |

### Сессия (`Session`)

`{ id ("ses_…"), title, directory, projectID, agent, model, time:{created,updated}, … }`

### История (`GET /session/{id}/message`)

Массив `{ info: {id, role: "user"|"assistant", time, …}, parts: [Part] }`.
`Part` бывает `{type:"text", text}` (текст), `{type:"tool", tool, callID,
state:{status: pending|running|completed|error}}`, `{type:"step-start"}`,
`{type:"step-finish"}`, `{type:"reasoning"}`.

Важно: ответ ассистента может быть разбит на **несколько** сообщений:
промежуточные (только `tool`/`step-*` без текста) и финальное с `text`.
Текстовые сообщения — это `parts` с `type=="text"`.

## SSE `/event` — формат

Строки вида `data: {"id":"evt_…","type":"…","properties":{…}}` через `\n\n`.

### РЕАЛЬНЫЕ типы событий (не из OpenAPI-доки!)

| type | properties | смысл |
|------|-----------|-------|
| `server.connected` / `server.heartbeat` | `{}` | служебные |
| `message.part.delta` | `{sessionID, messageID, partID, field:"text", delta}` | **стрим текста** |
| `message.part.updated` | `{sessionID, part:{type:"tool",tool,callID,state:{status}}}` | инструмент |
| `message.updated` / `session.updated` | `{sessionID, …}` | обновления |
| `session.status` | `{sessionID, status:{type:"busy"\|"idle"}}` | статус |
| `session.idle` | `{sessionID}` | **завершение ответа** |
| `session.error` | `{sessionID, error}` | ошибка |

⚠️ OpenAPI-схема `/doc` описывает события `session.next.text.delta`,
`session.next.tool.called` и т.п. — **этих событий сервер НЕ шлёт**. Реальные:
`message.part.delta` (текст) и `message.part.updated` (инструменты).

### Текст ответа

Собирается из `message.part.delta` с `field=="text"` (поле `delta`).
Завершение — событие `session.idle`.

## Обнаружение проектов

- Список проектов (папок с сессиями): `opencode db "SELECT directory AS worktree,
  MAX(time_updated) AS updated FROM session WHERE directory != '' GROUP BY
  directory ORDER BY updated DESC" --format json`.
- Имя инстанса берётся из `GET /project/current` → `worktree` (basename), а НЕ
  из `directory` первой сессии (сессии могут лежать в подпапках).
