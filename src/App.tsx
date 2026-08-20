import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import Markdown from "./Markdown";
import type {
  AppState,
  OpenCodeInstance,
  OpenCodeSession,
  Project,
  WindowInfo,
  AppMode,
  AppStatus,
  SttModelInfo,
  DownloadProgress,
  ToolAction,
  ConversationMessage,
} from "./types";

const DEFAULT_STATE: AppState = {
  mode: "opencode",
  status: "idle",
  statusMessage: "Готов к работе",
  recording: false,
  sensitivity: 1,
  silenceTimeout: 3,
  language: "auto",
  selectedModel: null,
  transcript: "",
  response: "",
  selectedMicrophone: null,
  selectedSession: null,
  activeInstance: null,
  selectedWindow: null,
};

const MODE_LABELS: Record<AppMode, string> = {
  opencode: "OpenCode",
  gui: "GUI",
};

const STATUS_LABELS: Record<AppStatus, string> = {
  idle: "Ожидание",
  recording: "Запись…",
  processing: "Обработка…",
  error: "Ошибка",
};

const SETTINGS_TABS = [
  "Модели",
  "Микрофон",
  "OpenCode",
  "Вставка",
  "Горячие клавиши",
  "О программе",
] as const;

function formatMb(mb: number): string {
  if (mb >= 1000) return `${(mb / 1000).toFixed(1)} ГБ`;
  return `${mb} МБ`;
}

function relTime(ms: number): string {
  if (!ms) return "";
  const diff = Date.now() - ms;
  if (diff < 60000) return "только что";
  const m = Math.floor(diff / 60000);
  if (m < 60) return `${m} мин назад`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h} ч назад`;
  return `${Math.floor(h / 24)} дн назад`;
}

function toolIcon(name: string): string {
  const t = name.toLowerCase();
  if (t.includes("edit") || t.includes("write") || t.includes("patch")) return "✏️";
  if (t.includes("grep") || t.includes("search") || t.includes("glob")) return "🔍";
  if (t.includes("bash") || t.includes("shell") || t.includes("exec")) return "💻";
  if (t.includes("read")) return "📄";
  if (t.includes("web") || t.includes("fetch")) return "🌐";
  return "🔧";
}

function Waveform({ active, level }: { active: boolean; level: number | null }) {
  const bars = 28;
  const live = active && level !== null;
  return (
    <div
      className={`waveform ${active ? "active" : ""} ${live ? "live" : ""}`}
      aria-hidden
    >
      {Array.from({ length: bars }).map((_, i) => {
        let height = 6;
        if (live && level != null) {
          const amp = 0.35 + 0.65 * Math.abs(Math.sin(i * 0.7 + 0.5));
          height = Math.max(6, Math.min(30, 6 + level * amp * 34));
        }
        return (
          <span
            key={i}
            className="wave-bar"
            style={{ height: `${height}px`, animationDelay: `${(i % 7) * 0.09}s` }}
          />
        );
      })}
    </div>
  );
}

function StatusDot({ status }: { status: AppStatus }) {
  return <span className={`status-dot ${status}`} />;
}

function MainApp() {
  const [state, setState] = useState<AppState>(DEFAULT_STATE);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const [settingsTab, setSettingsTab] = useState<string>("Модели");
  const [level, setLevel] = useState<number | null>(null);
  const [microphones, setMicrophones] = useState<string[]>([]);
  const [models, setModels] = useState<SttModelInfo[]>([]);
  const [downloads, setDownloads] = useState<Record<string, number>>({});
  const [instances, setInstances] = useState<OpenCodeInstance[]>([]);
  const [projects, setProjects] = useState<Project[]>([]);
  const [liveText, setLiveText] = useState("");
  const [tools, setTools] = useState<ToolAction[]>([]);
  const [modelLoading, setModelLoading] = useState(false);
  const prevStatus = useRef<AppStatus>("idle");
  const selectedSessionIdRef = useRef<string | null>(null);
  const recordingRef = useRef(false);
  const silenceTimeoutRef = useRef(3);
  const silenceSinceRef = useRef<number | null>(null);
  const pressedRef = useRef(false);
  const holdRef = useRef(false);
  const holdTimerRef = useRef<number | null>(null);

  useEffect(() => {
    invoke<AppState>("get_app_state")
      .then(setState)
      .catch((e) => setError(String(e)));

    invoke<string[]>("list_microphones")
      .then(setMicrophones)
      .catch((e) => setError(String(e)));

    invoke<SttModelInfo[]>("get_models")
      .then(setModels)
      .catch((e) => setError(String(e)));

    invoke<OpenCodeInstance[]>("list_opencode_sessions")
      .then(setInstances)
      .catch(() => {});

    invoke<Project[]>("list_projects")
      .then(setProjects)
      .catch(() => {});

    const unlistenState = listen<AppState>("state-changed", (event) => {
      prevStatus.current = event.payload.status;
      const newSession = event.payload.selectedSession?.sessionId ?? null;
      if (newSession !== selectedSessionIdRef.current) {
        selectedSessionIdRef.current = newSession;
        setLiveText("");
        setTools([]);
      }
      setState(event.payload);
    });
    const unlistenLevel = listen<number>("audio-level", (event) => {
      const level = event.payload;
      setLevel(level);
      // Авто-остановка по тишине.
      const timeout = silenceTimeoutRef.current;
      if (timeout > 0 && recordingRef.current) {
        if (level < 0.03) {
          if (silenceSinceRef.current === null) {
            silenceSinceRef.current = Date.now();
          } else if (Date.now() - silenceSinceRef.current >= timeout * 1000) {
            silenceSinceRef.current = null;
            invoke("stop_recording").catch(() => {});
          }
        } else {
          silenceSinceRef.current = null;
        }
      }
    });
    const unlistenProgress = listen<DownloadProgress>("model-download-progress", (event) => {
      setDownloads((d) => ({ ...d, [event.payload.modelId]: event.payload.percent }));
    });
    const unlistenDone = listen<{ modelId: string }>("model-download-done", () => {
      setDownloads({});
      invoke<SttModelInfo[]>("get_models").then(setModels).catch(() => {});
      setNotice("Модель скачана");
    });
    const unlistenDlErr = listen<{ modelId: string; error: string }>(
      "model-download-error",
      (event) => {
        setDownloads((d) => {
          const next = { ...d };
          delete next[event.payload.modelId];
          return next;
        });
        setError(`Ошибка скачивания: ${event.payload.error}`);
      },
    );
    const unlistenLoading = listen("model-loading", () => {
      setModelLoading(true);
    });
    const unlistenLoaded = listen("model-loaded", () => {
      setModelLoading(false);
      setNotice("Модель загружена");
    });
    const unlistenLoadErr = listen<string>("model-load-error", (event) => {
      setModelLoading(false);
      setError(`Ошибка загрузки модели: ${event.payload}`);
    });
    const unlistenDelta = listen<{ sessionId: string; text: string }>(
      "opencode-delta",
      (event) => {
        if (event.payload.sessionId === selectedSessionIdRef.current) {
          setLiveText((t) => t + event.payload.text);
        }
      },
    );
    const unlistenTool = listen<{ sessionId: string } & ToolAction>(
      "opencode-tool",
      (event) => {
        if (event.payload.sessionId !== selectedSessionIdRef.current) return;
        const a: ToolAction = {
          callId: event.payload.callId,
          name: event.payload.name,
          state: event.payload.state,
        };
        setTools((list) => {
          const idx = list.findIndex((t) => t.callId === a.callId);
          if (idx >= 0) {
            const next = [...list];
            next[idx] = a;
            return next;
          }
          return [...list, a];
        });
      },
    );
    const unlistenOcErr = listen<{ sessionId: string; error: string }>(
      "opencode-error",
      (event) => {
        setError(event.payload.error);
      },
    );

    return () => {
      unlistenState.then((f) => f());
      unlistenLevel.then((f) => f());
      unlistenProgress.then((f) => f());
      unlistenDone.then((f) => f());
      unlistenDlErr.then((f) => f());
      unlistenLoading.then((f) => f());
      unlistenLoaded.then((f) => f());
      unlistenLoadErr.then((f) => f());
      unlistenDelta.then((f) => f());
      unlistenTool.then((f) => f());
      unlistenOcErr.then((f) => f());
    };
  }, []);

  const isRecording = state.status === "recording" || state.recording;

  useEffect(() => {
    recordingRef.current = isRecording;
    silenceTimeoutRef.current = state.silenceTimeout;
    if (!isRecording) {
      setLevel(null);
      silenceSinceRef.current = null;
    }
    if (isRecording) {
      setLiveText("");
      setTools([]);
      silenceSinceRef.current = null;
    }
  }, [isRecording, state.silenceTimeout]);

  useEffect(() => {
    if (!notice) return;
    const t = setTimeout(() => setNotice(null), 3000);
    return () => clearTimeout(t);
  }, [notice]);

  useEffect(() => {
    const t = setInterval(() => {
      invoke<OpenCodeInstance[]>("list_opencode_sessions")
        .then(setInstances)
        .catch(() => {});
      invoke<Project[]>("list_projects")
        .then(setProjects)
        .catch(() => {});
    }, 5000);
    return () => clearInterval(t);
  }, []);

  const toggleRecording = useCallback(() => {
    invoke("toggle_recording").catch((e) => setError(String(e)));
  }, []);

  const startRecording = useCallback(() => {
    invoke("start_recording").catch((e) => setError(String(e)));
  }, []);

  const stopRecording = useCallback(() => {
    invoke("stop_recording").catch((e) => setError(String(e)));
  }, []);

  const onRecordPointerDown = useCallback(
    (e: React.PointerEvent) => {
      try {
        e.currentTarget.setPointerCapture(e.pointerId);
      } catch {
        /* ignore */
      }
      pressedRef.current = true;
      holdRef.current = false;
      if (holdTimerRef.current !== null) {
        clearTimeout(holdTimerRef.current);
      }
      holdTimerRef.current = window.setTimeout(() => {
        if (pressedRef.current && !recordingRef.current) {
          holdRef.current = true;
          startRecording();
        }
      }, 300);
    },
    [startRecording],
  );

  const onRecordPointerUp = useCallback(() => {
    if (!pressedRef.current) return;
    pressedRef.current = false;
    if (holdTimerRef.current !== null) {
      clearTimeout(holdTimerRef.current);
      holdTimerRef.current = null;
    }
    if (holdRef.current) {
      holdRef.current = false;
      stopRecording();
    } else {
      toggleRecording();
    }
  }, [stopRecording, toggleRecording]);

  const setMode = useCallback((mode: AppMode) => {
    invoke("set_mode", { mode }).catch((e) => setError(String(e)));
  }, []);

  const selectMicrophone = useCallback((name: string) => {
    invoke("select_microphone", { name }).catch((e) => setError(String(e)));
  }, []);

  const setSensitivity = useCallback((value: number) => {
    invoke("set_sensitivity", { level: value }).catch((e) => setError(String(e)));
  }, []);

  const setSilenceTimeout = useCallback((seconds: number) => {
    invoke("set_silence_timeout", { seconds }).catch((e) => setError(String(e)));
  }, []);

  const selectModel = useCallback((modelId: string) => {
    invoke("select_stt_model", { modelId }).catch((e) => setError(String(e)));
  }, []);

  const setLanguage = useCallback((language: string) => {
    invoke("set_language", { language }).catch((e) => setError(String(e)));
  }, []);

  const downloadModel = useCallback((modelId: string) => {
    invoke("download_model", { modelId }).catch((e) => setError(String(e)));
  }, []);

  const selectSession = useCallback((inst: OpenCodeInstance, session: OpenCodeSession) => {
    invoke("select_opencode_session", {
      instanceId: inst.id,
      port: inst.port,
      sessionId: session.id,
      title: session.title,
    }).catch((e) => setError(String(e)));
  }, []);

  const selectInstance = useCallback((inst: OpenCodeInstance) => {
    const latest = inst.sessions.reduce<OpenCodeSession | null>(
      (a, b) => (a && a.updatedAt >= b.updatedAt ? a : b),
      null,
    );
    if (latest) {
      invoke("select_opencode_session", {
        instanceId: inst.id,
        port: inst.port,
        sessionId: latest.id,
        title: latest.title,
      }).catch((e) => setError(String(e)));
    } else {
      invoke("select_opencode_instance", {
        id: inst.id,
        port: inst.port,
        name: inst.name,
      }).catch((e) => setError(String(e)));
    }
  }, []);

  const refreshInstances = useCallback(() => {
    invoke<OpenCodeInstance[]>("list_opencode_sessions")
      .then(setInstances)
      .catch((e) => setError(String(e)));
  }, []);

  const refreshProjects = useCallback(() => {
    invoke<Project[]>("list_projects")
      .then(setProjects)
      .catch((e) => setError(String(e)));
  }, []);

  const startProject = useCallback((worktree: string) => {
    invoke<Project[]>("start_project", { worktree })
      .then((list) => {
        setProjects(list);
        invoke<OpenCodeInstance[]>("list_opencode_sessions")
          .then(setInstances)
          .catch(() => {});
      })
      .catch((e) => setError(String(e)));
  }, []);

  const stopProject = useCallback((worktree: string) => {
    invoke<Project[]>("stop_project", { worktree })
      .then((list) => {
        setProjects(list);
        invoke<OpenCodeInstance[]>("list_opencode_sessions")
          .then(setInstances)
          .catch(() => {});
      })
      .catch((e) => setError(String(e)));
  }, []);

  const openResponseWindow = useCallback(() => {
    if (!state.selectedSession) return;
    invoke("open_response_window", {
      sessionId: state.selectedSession.sessionId,
      title: state.selectedSession.title,
      port: state.selectedSession.port,
    }).catch((e) => setError(String(e)));
  }, [state.selectedSession]);

  const refreshMicrophones = useCallback(() => {
    invoke<string[]>("list_microphones")
      .then(setMicrophones)
      .catch((e) => setError(String(e)));
  }, []);

  const refreshModels = useCallback(() => {
    invoke<SttModelInfo[]>("get_models")
      .then(setModels)
      .catch((e) => setError(String(e)));
  }, []);

  const refreshWindows = useCallback(() => {
    invoke<WindowInfo[]>("list_windows")
      .then(() => {})
      .catch((e) => setError(String(e)));
  }, []);

  const anyDownloaded = models.some((m) => m.downloaded);
  const responseText = state.status === "processing" ? liveText : state.response;

  const selectedModelInfo = models.find((m) => m.id === state.selectedModel);
  const modelName = selectedModelInfo?.name ?? state.selectedModel ?? "Модель";
  let modelStatusLabel = "Не выбрана";
  let modelStatusCls = "none";
  if (state.selectedModel) {
    const dl = downloads[state.selectedModel];
    if (dl !== undefined) {
      modelStatusLabel = `Скачивание ${Math.round(dl)}%`;
      modelStatusCls = "loading";
    } else if (selectedModelInfo && !selectedModelInfo.downloaded) {
      modelStatusLabel = "Не скачана";
      modelStatusCls = "error";
    } else if (modelLoading) {
      modelStatusLabel = "Загрузка…";
      modelStatusCls = "loading";
    } else {
      modelStatusLabel = "Готова";
      modelStatusCls = "ok";
    }
  }

  return (
    <main className="app">
      <header className="app-header">
        <div className="app-title">
          <span className="logo">🎙️</span>
          <h1>VoiceBridge</h1>
        </div>
        <div className="header-actions">
          <button
            className="model-badge"
            title="Выбранная модель — нажмите для настройки"
            onClick={() => {
              setSettingsTab("Модели");
              setShowSettings(true);
            }}
          >
            <span className={`model-dot ${modelStatusCls}`} />
            <span className="model-badge-name">{modelName}</span>
            <span className={`model-badge-status ${modelStatusCls}`}>{modelStatusLabel}</span>
          </button>
          <button
            className="icon-btn"
            title="Настройки"
            onClick={() => setShowSettings((s) => !s)}
          >
            ⚙️
          </button>
        </div>
      </header>

      <section className="mode-switch" role="tablist">
        {(Object.keys(MODE_LABELS) as AppMode[]).map((mode) => (
          <button
            key={mode}
            role="tab"
            aria-selected={state.mode === mode}
            className={`mode-btn ${state.mode === mode ? "active" : ""}`}
            onClick={() => setMode(mode)}
          >
            {mode === "opencode" ? "🤖 OpenCode" : "🖱️ GUI"}
          </button>
        ))}
      </section>

      <section className="recorder">
        <button
          className={`record-btn ${isRecording ? "recording" : ""}`}
          onPointerDown={onRecordPointerDown}
          onPointerUp={onRecordPointerUp}
          onPointerCancel={onRecordPointerUp}
          aria-label={isRecording ? "Остановить запись" : "Начать запись"}
        >
          <span className="record-btn-icon">{isRecording ? "⏹" : "🎤"}</span>
        </button>
        <div className="record-hint">
          {isRecording
            ? "Говорите… (тап — стоп, или держите)"
            : "Тап — запись, удержание — говорить"}
        </div>
        <Waveform active={isRecording} level={level} />
      </section>

      <section className="status-bar">
        <StatusDot status={state.status} />
        <span className="status-text">
          {STATUS_LABELS[state.status]}
          {state.statusMessage ? ` — ${state.statusMessage}` : ""}
        </span>
      </section>

      {state.mode === "opencode" && (
        <section className="projects-panel">
          <div className="panel-header">
            <span>Проекты</span>
            <button className="link-btn" onClick={refreshProjects} title="Обновить проекты">
              ⟳
            </button>
          </div>
          <div className="projects-list">
            {projects.length === 0 && (
              <div className="sessions-empty">
                Проекты не найдены. Откройте opencode в папке проекта.
              </div>
            )}
            {projects.map((p) => (
              <div key={p.id} className={`project-row ${p.running ? "running" : ""}`}>
                <span className={`session-dot ${p.running ? "on" : ""}`} />
                <span className="project-name" title={p.worktree}>
                  {p.name}
                </span>
                <span className="project-time">{relTime(p.updated)}</span>
                {p.running ? (
                  <button
                    className="btn small stop"
                    onClick={() => stopProject(p.worktree)}
                    title="Остановить сервер"
                  >
                    ■
                  </button>
                ) : (
                  <button
                    className="btn small play"
                    onClick={() => startProject(p.worktree)}
                    title="Запустить сервер OpenCode"
                  >
                    ▶
                  </button>
                )}
              </div>
            ))}
          </div>
        </section>
      )}

      {state.mode === "opencode" && (
        <section className="sessions-panel">
          <div className="panel-header">
            <span>Сессии OpenCode</span>
            <button className="link-btn" onClick={refreshInstances} title="Обновить сессии">
              ⟳ Обновить
            </button>
          </div>
          <div className="sessions-list">
            {instances.length === 0 && (
              <div className="sessions-empty">
                Запустите проект в панели «Проекты» — его сессии появятся здесь.
              </div>
            )}
            {instances.map((inst) => {
              const isActiveInstance = state.activeInstance?.port === inst.port;
              return (
                <div
                  className={`instance-card ${isActiveInstance ? "active" : ""}`}
                  key={inst.id}
                >
                  <button
                    className="instance-header"
                    onClick={() => selectInstance(inst)}
                    title="Выбрать этот экземпляр OpenCode"
                  >
                    <span className="instance-name">{inst.name}</span>
                    <span className="instance-port">:{inst.port}</span>
                    {isActiveInstance && (
                      <span className="instance-active-badge">активный</span>
                    )}
                  </button>
                  {inst.sessions.length === 0 && (
                    <div className="session-row empty">Нет сессий</div>
                  )}
                  {inst.sessions.map((session) => {
                    const active =
                      state.selectedSession?.sessionId === session.id &&
                      state.selectedSession?.instanceId === inst.id;
                    return (
                      <button
                        key={session.id}
                        className={`session-row ${active ? "active" : ""}`}
                        onClick={() => selectSession(inst, session)}
                      >
                        <span className={`session-dot ${active ? "on" : ""}`} />
                        <span className="session-title" title={session.title}>
                          {session.title || "Без названия"}
                        </span>
                        <span className="session-time">{relTime(session.updatedAt)}</span>
                      </button>
                    );
                  })}
                </div>
              );
            })}
          </div>
        </section>
      )}

      <section className="panels">
        <div className="panel">
          <div className="panel-header">
            <span>Распознанный текст</span>
            <span className="panel-badge">{state.mode === "opencode" ? "→ OpenCode" : "→ вставка"}</span>
          </div>
          <div className="panel-body transcript">
            {state.transcript || "Здесь появится распознанный текст…"}
          </div>
        </div>

        <div className="panel">
          <div className="panel-header">
            <span>
              {state.mode === "opencode" ? "Ответ OpenCode" : "Статус вставки"}
            </span>
            <div className="panel-header-actions">
              {state.mode === "opencode" && state.selectedSession && (
                <button
                  className="link-btn"
                  onClick={openResponseWindow}
                  title="Развернуть в отдельном окне (стриминг в реальном времени)"
                >
                  ⛶ Развернуть
                </button>
              )}
              {state.mode === "gui" && (
                <button className="link-btn" onClick={refreshWindows} title="Обновить окна">
                  ⟳ Окна
                </button>
              )}
            </div>
          </div>
          <div className="panel-body response">
            {tools.length > 0 && (
              <div className="tool-list">
                {tools.map((t) => (
                  <span key={t.callId} className={`tool-chip ${t.state}`}>
                    {toolIcon(t.name)} {t.name || "инструмент"}
                  </span>
                ))}
              </div>
            )}
            {responseText ? (
              <Markdown>{responseText}</Markdown>
            ) : (
              <span className="response-placeholder">
                {state.status === "processing"
                  ? "OpenCode думает…"
                  : "Ответ появится здесь…"}
              </span>
            )}
          </div>
        </div>
      </section>

      <footer className="app-footer">
        <div className="footer-left">
          <span className="shortcut-hint">⌘⇧V — запись</span>
          <button
            className="mic-hint"
            onClick={() => {
              setSettingsTab("Микрофон");
              setShowSettings(true);
            }}
            title="Выбрать микрофон"
          >
            🎤 {state.selectedMicrophone ?? "Микрофон по умолчанию"}
          </button>
        </div>
        <span className="target-hint">
          {state.mode === "opencode"
            ? state.selectedSession
              ? `Сессия: ${state.selectedSession.title}`
              : state.activeInstance
                ? `Экземпляр: ${state.activeInstance.name}`
                : "Экземпляр не выбран"
            : state.selectedWindow
              ? `Окно: ${state.selectedWindow.appName}`
              : "Окно: не выбрано"}
        </span>
      </footer>

      {showSettings && (
        <div className="settings-overlay" onClick={() => setShowSettings(false)}>
          <div className="settings-panel" onClick={(e) => e.stopPropagation()}>
            <h2>Настройки</h2>
            <div className="settings-tabs">
              {SETTINGS_TABS.map((tab) => (
                <button
                  key={tab}
                  className={`settings-tab ${settingsTab === tab ? "active" : ""}`}
                  onClick={() => setSettingsTab(tab)}
                >
                  {tab}
                </button>
              ))}
            </div>

            {settingsTab === "Модели" && (
              <div className="settings-content">
                <label className="field">
                  <span className="field-label">Язык распознавания</span>
                  <select
                    className="select"
                    value={state.language}
                    onChange={(e) => setLanguage(e.target.value)}
                  >
                    <option value="auto">Автоопределение</option>
                    <option value="ru">Русский</option>
                    <option value="en">Английский</option>
                  </select>
                </label>

                {!anyDownloaded && (
                  <div className="hint-banner">
                    Скачайте модель, чтобы начать. Рекомендуем Whisper Base.
                  </div>
                )}

                <div className="model-list">
                  {models.map((m) => {
                    const downloading = downloads[m.id] !== undefined;
                    return (
                      <div
                        key={m.id}
                        className={`model-item ${!m.supported ? "disabled" : ""}`}
                      >
                        <div className="model-info">
                          <label className="model-name">
                            <input
                              type="radio"
                              name="model"
                              checked={state.selectedModel === m.id}
                              disabled={!m.supported || !m.downloaded}
                              onChange={() => selectModel(m.id)}
                            />
                            <span>{m.name}</span>
                            <span className="model-size">{formatMb(m.sizeMb)}</span>
                          </label>
                          <span className="model-desc">{m.description}</span>
                        </div>
                        <div className="model-action">
                          {!m.supported ? (
                            <span className="badge-soon">Скоро</span>
                          ) : m.downloaded ? (
                            <span className="badge-done">✓ Скачана</span>
                          ) : downloading ? (
                            <div className="progress">
                              <div
                                className="progress-fill"
                                style={{ width: `${Math.min(100, downloads[m.id])}%` }}
                              />
                              <span className="progress-text">
                                {Math.round(downloads[m.id])}%
                              </span>
                            </div>
                          ) : (
                            <button
                              className="btn small"
                              onClick={() => downloadModel(m.id)}
                            >
                              Скачать
                            </button>
                          )}
                        </div>
                      </div>
                    );
                  })}
                </div>

                <button className="btn" onClick={refreshModels}>
                  ⟳ Обновить
                </button>
              </div>
            )}

            {settingsTab === "Микрофон" && (
              <div className="settings-content">
                <label className="field">
                  <span className="field-label">Устройство ввода</span>
                  <div className="field-row">
                    <select
                      className="select"
                      value={state.selectedMicrophone ?? ""}
                      onChange={(e) => selectMicrophone(e.target.value)}
                    >
                      <option value="">По умолчанию</option>
                      {microphones.map((m) => (
                        <option key={m} value={m}>
                          {m}
                        </option>
                      ))}
                    </select>
                    <button className="icon-btn" onClick={refreshMicrophones} title="Обновить список">
                      ⟳
                    </button>
                  </div>
                </label>

                <label className="field">
                  <span className="field-label">
                    Чувствительность: {state.sensitivity.toFixed(1)}×
                  </span>
                  <input
                    type="range"
                    min={0.1}
                    max={5}
                    step={0.1}
                    value={state.sensitivity}
                    onChange={(e) => setSensitivity(parseFloat(e.target.value))}
                  />
                </label>

                <label className="field">
                  <span className="field-label">Авто-остановка при тишине</span>
                  <select
                    className="select"
                    value={state.silenceTimeout}
                    onChange={(e) => setSilenceTimeout(parseFloat(e.target.value))}
                  >
                    <option value={0}>Выключена</option>
                    <option value={2}>2 секунды</option>
                    <option value={3}>3 секунды</option>
                    <option value={5}>5 секунд</option>
                  </select>
                </label>
              </div>
            )}

            {settingsTab !== "Микрофон" && settingsTab !== "Модели" && (
              <p className="settings-placeholder">
                Раздел «{settingsTab}» появится на следующих этапах разработки.
              </p>
            )}

            <button className="btn" onClick={() => setShowSettings(false)}>
              Закрыть
            </button>
          </div>
        </div>
      )}

      {notice && <div className="notice-toast">{notice}</div>}

      {error && (
        <div className="error-toast" onClick={() => setError(null)}>
          {error}
        </div>
      )}
    </main>
  );
}

function ResponseView() {
  const [messages, setMessages] = useState<ConversationMessage[]>([]);
  const [copied, setCopied] = useState(false);

  const sessionId = useMemo(() => {
    try {
      return getCurrentWebviewWindow().label.replace(/^response-/, "");
    } catch {
      return "";
    }
  }, []);

  useEffect(() => {
    invoke<ConversationMessage[]>("get_conversation")
      .then(setMessages)
      .catch(() => {});

    const unlistenUser = listen<{ sessionId: string; text: string }>(
      "opencode-user",
      (e) => {
        if (e.payload.sessionId !== sessionId) return;
        setMessages((m) => [
          ...m,
          { role: "user", text: e.payload.text },
          { role: "assistant", text: "" },
        ]);
      },
    );
    const unlistenDelta = listen<{ sessionId: string; text: string }>(
      "opencode-delta",
      (e) => {
        if (e.payload.sessionId !== sessionId) return;
        setMessages((m) => {
          const next = [...m];
          const last = next[next.length - 1];
          if (!last || last.role !== "assistant") {
            next.push({ role: "assistant", text: e.payload.text });
          } else {
            next[next.length - 1] = { ...last, text: last.text + e.payload.text };
          }
          return next;
        });
      },
    );

    return () => {
      unlistenUser.then((f) => f());
      unlistenDelta.then((f) => f());
    };
  }, [sessionId]);

  const copy = useCallback(async () => {
    const text = messages
      .filter((m) => m.role === "assistant")
      .map((m) => m.text)
      .join("\n\n");
    try {
      await writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch (e) {
      console.error("copy failed", e);
    }
  }, [messages]);

  const close = useCallback(() => {
    invoke("close_response_window").catch(() => {});
  }, []);

  return (
    <main className="response-view">
      <header className="response-view-header">
        <h1>Ответ OpenCode</h1>
        <div className="header-actions">
          <button className="btn" onClick={copy}>
            {copied ? "Скопировано ✓" : "Копировать"}
          </button>
          <button className="btn" onClick={close}>
            Закрыть
          </button>
        </div>
      </header>
      <div className="response-view-body">
        {messages.length === 0 ? (
          <p className="response-empty">
            Скажите фразу в VoiceBridge — ответ OpenCode появится здесь в реальном
            времени.
          </p>
        ) : (
          messages.map((m, i) =>
            m.role === "user" ? (
              <div key={i} className="chat-user">
                <span className="chat-role">Вы</span>
                <div className="chat-user-text">{m.text}</div>
              </div>
            ) : (
              <div key={i} className="chat-assistant">
                <span className="chat-role">OpenCode</span>
                <Markdown>{m.text || "*…*"}</Markdown>
              </div>
            ),
          )
        )}
      </div>
    </main>
  );
}

function App() {
  const [view] = useState(() => {
    try {
      return getCurrentWebviewWindow().label;
    } catch {
      return "main";
    }
  });

  if (view.startsWith("response-")) {
    return <ResponseView />;
  }
  return <MainApp />;
}

export default App;
