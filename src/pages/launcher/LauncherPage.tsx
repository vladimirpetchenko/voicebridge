import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { Mic, MicVocal, Settings } from "lucide-react";
import { prettifyModel } from "../../shared/lib/format";
import { ProjectsPanel } from "./ProjectsPanel";
import type {
  AppState,
  DownloadProgress,
  KnownDevice,
  MobileInfo,
  OpenCodeInstance,
  OpenCodeSession,
  Project,
  SttModelInfo,
} from "../../shared/types";
import { SettingsOverlay } from "./SettingsOverlay";

const DEFAULT_STATE: AppState = {
  status: "idle",
  statusMessage: "Готов к работе",
  recording: false,
  sensitivity: 1,
  silenceTimeout: 3,
  sendMode: "direct",
  hotkey: "Cmd+Shift+V",
  mobileEnabled: false,
  mobilePort: 47800,
  mobileToken: "",
  language: "auto",
  selectedModel: null,
  transcript: "",
  response: "",
  recordingSessionId: null,
  selectedMicrophone: null,
  selectedSession: null,
  opencodeModel: null,
  activeInstance: null,
  hiddenProjects: [],
};

/// Главное окно — лаунчер: проекты + сессии + настройки.
export default function LauncherPage() {
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
  const [hotkeyDraft, setHotkeyDraft] = useState(DEFAULT_STATE.hotkey);
  const [mobileInfo, setMobileInfo] = useState<MobileInfo | null>(null);
  const [devices, setDevices] = useState<KnownDevice[]>([]);
  const [expandedProjects, setExpandedProjects] = useState<Set<string>>(new Set());

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

    invoke<MobileInfo>("get_mobile_info")
      .then(setMobileInfo)
      .catch(() => {});

    invoke<KnownDevice[]>("list_devices")
      .then(setDevices)
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
    const unlistenDevices = listen<KnownDevice[]>("devices-changed", (event) => {
      setDevices(event.payload);
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
      unlistenDevices.then((f) => f());
    };
  }, []);

  useEffect(() => {
    if (!notice) return;
    const t = setTimeout(() => setNotice(null), 3000);
    return () => clearTimeout(t);
  }, [notice]);

  // Синхронизируем черновик горячей клавиши с актуальным значением.
  useEffect(() => {
    setHotkeyDraft(state.hotkey);
  }, [state.hotkey]);

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

  const createSession = useCallback(
    (port: number, worktree: string) => {
      invoke<AppState>("create_session", { port, worktree, title: "" })
        .then((st) => {
          const sel = st.selectedSession;
          if (sel) {
            invoke("open_response_window", {
              sessionId: sel.sessionId,
              title: sel.title,
              port,
            }).catch(() => {});
          }
          refreshInstances();
          refreshProjects();
        })
        .catch((e) => setError(String(e)));
    },
    [refreshInstances, refreshProjects],
  );

  const toggleProjectSessions = useCallback((id: string) => {
    setExpandedProjects((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  }, []);

  const hideProject = useCallback(
    (worktree: string) => {
      invoke("hide_project", { worktree }).catch((e) => setError(String(e)));
      refreshInstances();
    },
    [refreshInstances],
  );

  const unhideProject = useCallback((worktree: string) => {
    invoke("unhide_project", { worktree }).catch((e) => setError(String(e)));
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

  const setSendMode = useCallback((mode: string) => {
    invoke("set_send_mode", { mode }).catch((e) => setError(String(e)));
  }, []);

  const saveHotkey = useCallback(() => {
    invoke("set_hotkey", { hotkey: hotkeyDraft })
      .then(() => setNotice("Горячая клавиша обновлена"))
      .catch((e) => setError(String(e)));
  }, [hotkeyDraft]);

  const refreshMobileInfo = useCallback(() => {
    invoke<MobileInfo>("get_mobile_info").then(setMobileInfo).catch(() => {});
  }, []);

  const setMobileEnabled = useCallback(
    (enabled: boolean) => {
      invoke("set_mobile_enabled", { enabled })
        .then(() => refreshMobileInfo())
        .catch((e) => setError(String(e)));
    },
    [refreshMobileInfo],
  );

  const regenerateToken = useCallback(() => {
    invoke("regenerate_mobile_token")
      .then(() => refreshMobileInfo())
      .catch((e) => setError(String(e)));
  }, [refreshMobileInfo]);

  const forgetDevice = useCallback((deviceId: string) => {
    invoke<KnownDevice[]>("forget_device", { deviceId })
      .then(setDevices)
      .catch((e) => setError(String(e)));
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

  const refreshSettings = useCallback(() => {
    invoke<SttModelInfo[]>("get_models").then(setModels).catch(() => {});
    invoke<string[]>("list_microphones").then(setMicrophones).catch(() => {});
    invoke<MobileInfo>("get_mobile_info").then(setMobileInfo).catch(() => {});
    invoke<string>("get_opencode_binary").then(setOpencodeBinary).catch(() => {});
    setNotice("Данные обновлены");
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

  // Сессии по порту и инстансы по порту — для показа сессий под проектом.
  const sessionsByPort = useMemo(() => {
    const map = new Map<number, OpenCodeSession[]>();
    for (const inst of instances) map.set(inst.port, inst.sessions);
    return map;
  }, [instances]);

  const instanceByPort = useMemo(() => {
    const map = new Map<number, OpenCodeInstance>();
    for (const inst of instances) map.set(inst.port, inst);
    return map;
  }, [instances]);

  // Инстансы, не привязанные к проектам (ручной запуск сервера).
  const projectPorts = useMemo(() => new Set(projects.map((p) => p.port)), [projects]);
  const hiddenSet = useMemo(() => new Set(state.hiddenProjects ?? []), [state.hiddenProjects]);
  const visibleProjects = projects.filter((p) => !hiddenSet.has(p.worktree));
  const orphanInstances = instances.filter(
    (i) => !projectPorts.has(i.port) && !hiddenSet.has(i.id),
  );

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

      <ProjectsPanel
        visibleProjects={visibleProjects}
        orphanInstances={orphanInstances}
        sessionsByPort={sessionsByPort}
        instanceByPort={instanceByPort}
        expandedProjects={expandedProjects}
        selectedSessionId={state.selectedSession?.sessionId}
        openSessionIds={openSessionIds}
        hiddenProjects={state.hiddenProjects}
        projects={projects}
        onNewInstance={newInstance}
        onRefresh={() => {
          refreshProjects();
          refreshInstances();
        }}
        onCreateSession={createSession}
        onStartProject={startProject}
        onStopProject={stopProject}
        onHideProject={hideProject}
        onUnhideProject={unhideProject}
        onSelectSession={selectSession}
        onToggleSessions={toggleProjectSessions}
      />

      <footer className="app-footer">
        <span className="shortcut-hint" title="Глобальная горячая клавиша записи">
          {state.hotkey}
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
            state.selectedSession
              ? state.opencodeModel
                ? `Модель: ${state.opencodeModel}`
                : `Сессия: ${state.selectedSession.title}`
              : state.activeInstance
                ? `Экземпляр: ${state.activeInstance.name}`
                : "Чат не выбран"
          }
        >
          {state.selectedSession
            ? state.opencodeModel
              ? prettifyModel(state.opencodeModel)
              : state.selectedSession.title
            : state.activeInstance
              ? state.activeInstance.name
              : "чат не выбран"}
        </span>
      </footer>

      {showSettings && (
        <SettingsOverlay
          state={state}
          settingsTab={settingsTab}
          setSettingsTab={setSettingsTab}
          onClose={() => setShowSettings(false)}
          onRefreshSettings={refreshSettings}
          models={models}
          downloads={downloads}
          anyDownloaded={anyDownloaded}
          onSelectModel={selectModel}
          onDownloadModel={downloadModel}
          onRefreshModels={refreshModels}
          onSetLanguage={setLanguage}
          microphones={microphones}
          onSelectMicrophone={selectMicrophone}
          onRefreshMicrophones={refreshMicrophones}
          onSetSensitivity={setSensitivity}
          onSetSilenceTimeout={setSilenceTimeout}
          opencodeBinary={opencodeBinary}
          onSetSendMode={setSendMode}
          onRefreshBinary={refreshBinary}
          hotkeyDraft={hotkeyDraft}
          setHotkeyDraft={setHotkeyDraft}
          onSaveHotkey={saveHotkey}
          mobileInfo={mobileInfo}
          devices={devices}
          onSetMobileEnabled={setMobileEnabled}
          onRegenerateToken={regenerateToken}
          onForgetDevice={forgetDevice}
          updateStatus={updateStatus}
          updateChecking={updateChecking}
          onCheckUpdate={checkUpdate}
        />
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
