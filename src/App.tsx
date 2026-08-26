import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { open } from "@tauri-apps/plugin-dialog";
import {
  Bot,
  Check,
  Copy,
  FileText,
  Globe,
  Mic,
  MicVocal,
  MousePointer2,
  Pencil,
  Play,
  Plus,
  RefreshCw,
  Search,
  Settings,
  Square,
  Terminal,
  Wrench,
  type LucideIcon,
} from "lucide-react";
import Markdown from "./Markdown";
import ChatInput from "./ChatInput";
import { playReceive, playSend } from "./sounds";
import type {
  AppState,
  OpenCodeInstance,
  OpenCodeSession,
  Project,
  AppMode,
  SttModelInfo,
  DownloadProgress,
  ToolAction,
  ConversationMessage,
  PermissionRequest,
  QuestionRequest,
  SessionInfo,
  SessionUsage,
} from "./types";

const DEFAULT_STATE: AppState = {
  mode: "opencode",
  status: "idle",
  statusMessage: "Готов к работе",
  recording: false,
  sensitivity: 1,
  silenceTimeout: 3,
  pasteMethod: "clipboard",
  pasteDelayMs: 500,
  sendMode: "direct",
  language: "auto",
  selectedModel: null,
  transcript: "",
  response: "",
  recordingSessionId: null,
  selectedMicrophone: null,
  selectedSession: null,
  opencodeModel: null,
  activeInstance: null,
  selectedWindow: null,
};

const MODE_LABELS: Record<AppMode, string> = {
  opencode: "OpenCode",
  gui: "GUI",
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

function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return `${n}`;
}

function formatCost(c: number): string {
  if (!c || c <= 0) return "$0.00";
  return `$${c.toFixed(c < 0.01 ? 4 : 2)}`;
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

function toolIcon(name: string): LucideIcon {
  const t = name.toLowerCase();
  if (t.includes("edit") || t.includes("write") || t.includes("patch")) return Pencil;
  if (t.includes("grep") || t.includes("search") || t.includes("glob")) return Search;
  if (t.includes("bash") || t.includes("shell") || t.includes("exec")) return Terminal;
  if (t.includes("read")) return FileText;
  if (t.includes("web") || t.includes("fetch")) return Globe;
  return Wrench;
}

function MainApp() {
  const [state, setState] = useState<AppState>(DEFAULT_STATE);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const [settingsTab, setSettingsTab] = useState<string>("Модели");
  const [microphones, setMicrophones] = useState<string[]>([]);
  const [models, setModels] = useState<SttModelInfo[]>([]);
  const [downloads, setDownloads] = useState<Record<string, number>>({});
  const [instances, setInstances] = useState<OpenCodeInstance[]>([]);
  const [projects, setProjects] = useState<Project[]>([]);
  const [openSessionIds, setOpenSessionIds] = useState<string[]>([]);
  const [modelLoading, setModelLoading] = useState(false);
  const [opencodeBinary, setOpencodeBinary] = useState("");
  const [updateStatus, setUpdateStatus] = useState<string | null>(null);
  const [updateChecking, setUpdateChecking] = useState(false);

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

    invoke<string[]>("list_open_session_ids")
      .then(setOpenSessionIds)
      .catch(() => {});

    invoke<string>("get_opencode_binary")
      .then(setOpencodeBinary)
      .catch(() => {});

    const unlistenState = listen<AppState>("state-changed", (event) => {
      setState(event.payload);
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
    const unlistenOcErr = listen<{ sessionId: string; error: string }>(
      "opencode-error",
      (event) => {
        setError(event.payload.error);
      },
    );
    const unlistenOpen = listen<string[]>("sessions-open-changed", (event) => {
      setOpenSessionIds(event.payload);
    });

    return () => {
      unlistenState.then((f) => f());
      unlistenProgress.then((f) => f());
      unlistenDone.then((f) => f());
      unlistenDlErr.then((f) => f());
      unlistenLoading.then((f) => f());
      unlistenLoaded.then((f) => f());
      unlistenLoadErr.then((f) => f());
      unlistenOcErr.then((f) => f());
      unlistenOpen.then((f) => f());
    };
  }, []);

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
      invoke<string[]>("list_open_session_ids")
        .then(setOpenSessionIds)
        .catch(() => {});
    }, 5000);
    return () => clearInterval(t);
  }, []);

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
      model: session.model,
    }).catch((e) => setError(String(e)));
    invoke("open_response_window", {
      sessionId: session.id,
      title: session.title,
      port: inst.port,
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
        model: latest.model,
      }).catch((e) => setError(String(e)));
      invoke("open_response_window", {
        sessionId: latest.id,
        title: latest.title,
        port: inst.port,
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

  const newInstance = useCallback(async () => {
    const dir = await open({ directory: true, multiple: false, title: "Выберите рабочую папку проекта" });
    const path = typeof dir === "string" ? dir : null;
    if (!path) return;
    invoke<Project[]>("start_project", { worktree: path })
      .then((list) => {
        setProjects(list);
        invoke<OpenCodeInstance[]>("list_opencode_sessions")
          .then(setInstances)
          .catch(() => {});
      })
      .catch((e) => setError(String(e)));
   }, []);

  const setPasteMethod = useCallback((method: string) => {
    invoke("set_paste_method", { method }).catch((e) => setError(String(e)));
  }, []);

  const setPasteDelay = useCallback((ms: number) => {
    invoke("set_paste_delay", { ms }).catch((e) => setError(String(e)));
  }, []);

  const setSendMode = useCallback((mode: string) => {
    invoke("set_send_mode", { mode }).catch((e) => setError(String(e)));
  }, []);

  const refreshBinary = useCallback(() => {
    invoke<string>("get_opencode_binary")
      .then(setOpencodeBinary)
      .catch(() => {});
  }, []);

  const checkUpdate = useCallback(async () => {
    setUpdateChecking(true);
    setUpdateStatus("Проверяю обновления…");
    try {
      const version = await invoke<string | null>("check_update");
      setUpdateStatus(
        version ? `Доступна новая версия: ${version}` : "У вас последняя версия",
      );
    } catch (e) {
      setUpdateStatus(`Не удалось проверить обновления: ${String(e)}`);
    } finally {
      setUpdateChecking(false);
    }
  }, []);

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

  const anyDownloaded = models.some((m) => m.downloaded);

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
          <MicVocal className="logo" size={22} />
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
            <Settings size={18} />
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
            {mode === "opencode" ? <Bot size={16} /> : <MousePointer2 size={16} />}
            <span>{mode === "opencode" ? "OpenCode" : "GUI"}</span>
          </button>
        ))}
      </section>

      {state.mode === "opencode" && (
        <section className="projects-panel">
          <div className="panel-header">
            <span>Проекты</span>
            <div className="panel-header-actions">
              <button className="link-btn" onClick={newInstance} title="Новый инстанс (выбрать папку)">
                <Plus size={13} /> Новый
              </button>
              <button className="link-btn" onClick={refreshProjects} title="Обновить проекты">
                <RefreshCw size={13} />
              </button>
            </div>
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
                    <Square size={12} fill="currentColor" />
                  </button>
                ) : (
                  <button
                    className="btn small play"
                    onClick={() => startProject(p.worktree)}
                    title="Запустить сервер OpenCode"
                  >
                    <Play size={12} fill="currentColor" />
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
              <RefreshCw size={13} /> Обновить
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
                    const isOpen = openSessionIds.includes(session.id);
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
                        {isOpen && (
                          <span className="session-open-badge">открыта</span>
                        )}
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

      <footer className="app-footer">
        <span className="shortcut-hint" title="Глобальная горячая клавиша записи">
          ⌘⇧V
        </span>
        <button
          className="mic-hint"
          onClick={() => {
            setSettingsTab("Микрофон");
            setShowSettings(true);
          }}
          title={`Микрофон: ${state.selectedMicrophone ?? "по умолчанию"}`}
        >
          <Mic size={13} /> {state.selectedMicrophone ?? "По умолчанию"}
        </button>
        <span
          className="target-hint"
          title={
            state.mode === "opencode"
              ? state.selectedSession
                ? state.opencodeModel
                  ? `Модель: ${state.opencodeModel}`
                  : `Сессия: ${state.selectedSession.title}`
                : state.activeInstance
                  ? `Экземпляр: ${state.activeInstance.name}`
                  : "Чат не выбран"
              : "GUI-режим"
          }
        >
          {state.mode === "opencode"
            ? state.selectedSession
              ? state.opencodeModel ?? state.selectedSession.title
              : state.activeInstance
                ? state.activeInstance.name
                : "чат не выбран"
            : state.selectedWindow?.appName ?? "GUI"}
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
                            <span className="badge-done"><Check size={12} /> Скачана</span>
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
                  <RefreshCw size={14} /> Обновить
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
                      <RefreshCw size={15} />
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

            {settingsTab === "OpenCode" && (
              <div className="settings-content">
                <label className="field">
                  <span className="field-label">Режим отправки распознанного текста</span>
                  <select
                    className="select"
                    value={state.sendMode}
                    onChange={(e) => setSendMode(e.target.value)}
                  >
                    <option value="direct">Сразу в чат (разговор)</option>
                    <option value="confirm">Предпросмотр перед отправкой</option>
                  </select>
                </label>

                <label className="field">
                  <span className="field-label">Исполняемый файл opencode</span>
                  <div className="field-row">
                    <input className="select" value={opencodeBinary} readOnly />
                    <button className="icon-btn" onClick={refreshBinary} title="Пересканировать">
                      <RefreshCw size={15} />
                    </button>
                  </div>
                </label>
                <div className="hint-banner">
                  Запущенные серверы OpenCode находятся автоматически (по процессам
                  и портам 4096/12000/3000/17000) и обновляются каждые 5 секунд.
                </div>
              </div>
            )}

            {settingsTab === "Вставка" && (
              <div className="settings-content">
                <label className="field">
                  <span className="field-label">Способ вставки</span>
                  <select
                    className="select"
                    value={state.pasteMethod}
                    onChange={(e) => setPasteMethod(e.target.value)}
                  >
                    <option value="clipboard">Буфер обмена + Ctrl/Cmd+V (быстро)</option>
                    <option value="keys">Симуляция нажатий клавиш (медленно, везде)</option>
                    <option value="accessibility">Accessibility API (экспериментально)</option>
                  </select>
                </label>
                <label className="field">
                  <span className="field-label">
                    Задержка перед вставкой: {state.pasteDelayMs} мс
                  </span>
                  <input
                    type="range"
                    min={0}
                    max={3000}
                    step={100}
                    value={state.pasteDelayMs}
                    onChange={(e) => setPasteDelay(parseInt(e.target.value, 10))}
                  />
                </label>
              </div>
            )}

            {settingsTab === "Горячие клавиши" && (
              <div className="settings-content">
                <div className="field">
                  <span className="field-label">Запись (глобально)</span>
                  <span className="shortcut-hint">⌘⇧V / Ctrl+Shift+V</span>
                </div>
                <div className="hint-banner">
                  Настройка произвольных комбинаций появится позже.
                </div>
              </div>
            )}

            {settingsTab === "О программе" && (
              <div className="settings-content">
                <p>VoiceBridge — голосовой ассистент для разработчиков.</p>
                <p>Локальное распознавание речи (whisper.cpp), управление OpenCode.</p>
                <p className="about-version">Версия 0.1.0 · Rust + Tauri · React</p>
                <p>
                  <a
                    className="link"
                    href="https://github.com/vladimirpetchenko/voicebridge"
                    target="_blank"
                    rel="noreferrer"
                  >
                    github.com/vladimirpetchenko/voicebridge
                  </a>
                </p>
                <button
                  className="btn"
                  onClick={checkUpdate}
                  disabled={updateChecking}
                >
                  <RefreshCw size={14} /> Проверить обновления
                </button>
                {updateStatus && <p className="about-version">{updateStatus}</p>}
              </div>
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
  const [tools, setTools] = useState<ToolAction[]>([]);
  const [permissions, setPermissions] = useState<PermissionRequest[]>([]);
  const [questions, setQuestions] = useState<QuestionRequest[]>([]);
  const [copiedIdx, setCopiedIdx] = useState<number | null>(null);
  const [info, setInfo] = useState<SessionInfo>({ title: "", project: "" });
  const [usage, setUsage] = useState<SessionUsage | null>(null);
  const bodyRef = useRef<HTMLDivElement>(null);

  const sessionId = useMemo(() => {
    try {
      return getCurrentWebviewWindow().label.replace(/^response-/, "");
    } catch {
      return "";
    }
  }, []);

  const refreshUsage = useCallback(() => {
    invoke<SessionUsage | null>("get_session_usage")
      .then(setUsage)
      .catch(() => {});
  }, []);

  useEffect(() => {
    invoke<ConversationMessage[]>("get_conversation")
      .then(setMessages)
      .catch(() => {});

    invoke<SessionInfo>("get_session_info")
      .then(setInfo)
      .catch(() => {});

    refreshUsage();

    const unlistenUser = listen<{ sessionId: string; text: string }>(
      "opencode-user",
      (e) => {
        if (e.payload.sessionId !== sessionId) return;
        playSend();
        setTools([]);
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
    const unlistenDone = listen<{ sessionId: string }>("opencode-done", (e) => {
      if (e.payload.sessionId !== sessionId) return;
      playReceive();
      // Обновляем счётчики после завершения ответа (БД может чуть отставать).
      setTimeout(refreshUsage, 500);
    });
    const unlistenTool = listen<{ sessionId: string } & ToolAction>(
      "opencode-tool",
      (e) => {
        if (e.payload.sessionId !== sessionId) return;
        const a: ToolAction = {
          callId: e.payload.callId,
          name: e.payload.name,
          state: e.payload.state,
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
    const unlistenPermission = listen<PermissionRequest>(
      "opencode-permission",
      (e) => {
        if (e.payload.sessionId !== sessionId) return;
        setPermissions((list) => [...list, e.payload]);
      },
    );
    const unlistenQuestion = listen<QuestionRequest>(
      "opencode-question",
      (e) => {
        if (e.payload.sessionId !== sessionId) return;
        setQuestions((list) => [...list, e.payload]);
      },
    );

    return () => {
      unlistenUser.then((f) => f());
      unlistenDelta.then((f) => f());
      unlistenDone.then((f) => f());
      unlistenTool.then((f) => f());
      unlistenPermission.then((f) => f());
      unlistenQuestion.then((f) => f());
    };
  }, [sessionId, refreshUsage]);

  useEffect(() => {
    bodyRef.current?.scrollTo({ top: bodyRef.current.scrollHeight });
  }, [messages, tools, permissions, questions]);

  const copyMessage = useCallback(async (text: string, idx: number) => {
    try {
      await writeText(text);
      setCopiedIdx(idx);
      setTimeout(() => setCopiedIdx(null), 1500);
    } catch (e) {
      console.error("copy failed", e);
    }
  }, []);

  const close = useCallback(() => {
    invoke("close_response_window").catch(() => {});
  }, []);

  const replyPermission = useCallback((req: PermissionRequest, reply: string) => {
    invoke("reply_permission", { port: req.port, requestId: req.requestId, reply }).catch(
      () => {},
    );
    setPermissions((list) => list.filter((p) => p.requestId !== req.requestId));
  }, []);

  const answerQuestion = useCallback((req: QuestionRequest, answers: string[][]) => {
    invoke("reply_question", { port: req.port, requestId: req.requestId, answers }).catch(
      () => {},
    );
    setQuestions((list) => list.filter((q) => q.requestId !== req.requestId));
  }, []);

  const rejectQuestion = useCallback((req: QuestionRequest) => {
    invoke("reject_question", { port: req.port, requestId: req.requestId }).catch(() => {});
    setQuestions((list) => list.filter((q) => q.requestId !== req.requestId));
  }, []);

  return (
    <main className="response-view">
      <header className="response-view-header">
        <div className="chat-title">
          <h1>{info.title || "OpenCode"}</h1>
          {info.project && <span className="chat-project">{info.project}</span>}
        </div>
        <div className="header-actions">
          <button className="btn" onClick={close}>
            Закрыть
          </button>
        </div>
      </header>
      <div className="response-view-body" ref={bodyRef}>
        {permissions.map((p) => (
          <div key={p.requestId} className="action-card">
            <div className="action-card-title">OpenCode запрашивает разрешение</div>
            <div className="action-desc">
              Инструмент: <code>{p.permission || "?"}</code>
            </div>
            {p.patterns.length > 0 && (
              <pre className="action-patterns">{p.patterns.join("\n")}</pre>
            )}
            <div className="action-buttons">
              <button className="btn play" onClick={() => replyPermission(p, "once")}>
                Разрешить
              </button>
              <button className="btn" onClick={() => replyPermission(p, "always")}>
                Всегда
              </button>
              <button className="btn stop" onClick={() => replyPermission(p, "reject")}>
                Запретить
              </button>
            </div>
          </div>
        ))}

        {questions.map((q) => (
          <div key={q.requestId} className="action-card">
            <div className="action-card-title">{q.questions[0]?.header || "Вопрос OpenCode"}</div>
            <div className="action-desc">{q.questions[0]?.question || ""}</div>
            <div className="action-options">
              {(q.questions[0]?.options ?? []).map((opt) => (
                <button
                  key={opt.label}
                  className="btn"
                  onClick={() => answerQuestion(q, [[opt.label]])}
                  title={opt.description}
                >
                  {opt.label}
                </button>
              ))}
            </div>
            <div className="action-buttons">
              <button className="btn stop" onClick={() => rejectQuestion(q)}>
                Отклонить
              </button>
            </div>
          </div>
        ))}

        {tools.length > 0 && (
          <div className="tool-list response-tools">
            {tools.map((t) => (
              <span key={t.callId} className={`tool-chip ${t.state}`}>
                {(() => {
                  const Icon = toolIcon(t.name);
                  return <Icon size={13} />;
                })()}{" "}
                {t.name || "инструмент"}
              </span>
            ))}
          </div>
        )}

        {messages.length === 0 ? (
          <p className="response-empty">
            Скажите фразу в VoiceBridge — ответ OpenCode появится здесь в реальном
            времени, вместе с инструментами и запросами действий.
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
                <div className="chat-role-row">
                  <span className="chat-role">OpenCode</span>
                  {m.text && (
                    <button
                      className="msg-copy-btn"
                      onClick={() => copyMessage(m.text, i)}
                      title="Копировать сообщение"
                    >
                      {copiedIdx === i ? <Check size={13} /> : <Copy size={13} />}
                    </button>
                  )}
                </div>
                {m.text ? (
                  <Markdown>{m.text}</Markdown>
                ) : (
                  <span className="thinking-dots" aria-label="OpenCode думает">
                    <span />
                    <span />
                    <span />
                  </span>
                )}
              </div>
            ),
          )
        )}
      </div>
      <ChatInput sessionId={sessionId} />
      <footer className="chat-status-bar">
        {usage && (
          <>
            <span
              className="status-metric"
              title={`Ввод ${formatTokens(usage.tokensInput)} · вывод ${formatTokens(usage.tokensOutput)} · reasoning ${formatTokens(usage.tokensReasoning)}`}
            >
              {formatTokens(usage.tokensTotal)} токенов
            </span>
            <span className="status-sep">·</span>
            <span className="status-metric" title={`Модель: ${usage.model}`}>
              {formatCost(usage.cost)}
            </span>
          </>
        )}
      </footer>
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
