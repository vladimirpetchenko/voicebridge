import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { AlertTriangle, Bot, Check, Clock, Copy, GitBranch, Square, User, X } from "lucide-react";
import Markdown from "../../shared/ui/Markdown";
import ChatInput from "../../features/chat-input/ChatInput";
import { GitPanel } from "../../features/git/GitPanel";
import { useIsWide } from "../../shared/lib/hooks";
import { formatCost, formatDuration, formatTokens } from "../../shared/lib/format";
import { playReceive, playSend } from "../../shared/lib/sounds";
import type {
  ConversationMessage,
  GitDiff,
  GitFileChange,
  GitInfo,
  PermissionRequest,
  QuestionRequest,
  SessionInfo,
  SessionUsage,
  ToolAction,
} from "../../shared/types";
import { ReasoningBlock } from "./components/ReasoningBlock";
import { PermissionCard, QuestionCard } from "./components/ActionCards";
import { ToolChips } from "./components/ToolChips";

type ChatMessage = ConversationMessage & { durationMs?: number };

/// Порог «зависшего» ответа: если событий нет дольше этого времени.
const STALL_THRESHOLD_MS = 90_000;

export default function ChatPage() {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [tools, setTools] = useState<ToolAction[]>([]);
  const [permissions, setPermissions] = useState<PermissionRequest[]>([]);
  const [questions, setQuestions] = useState<QuestionRequest[]>([]);
  const [copiedIdx, setCopiedIdx] = useState<number | null>(null);
  const [info, setInfo] = useState<SessionInfo>({ title: "", project: "" });
  const [usage, setUsage] = useState<SessionUsage | null>(null);
  const [busy, setBusy] = useState(false);
  const bodyRef = useRef<HTMLDivElement>(null);
  const busyStartRef = useRef<number | null>(null);
  const lastActivityRef = useRef(0);
  const [elapsed, setElapsed] = useState(0);
  const [stalled, setStalled] = useState(false);
  const [gitChanges, setGitChanges] = useState<GitFileChange[]>([]);
  const [gitBranch, setGitBranch] = useState("");
  const [gitDiff, setGitDiff] = useState<GitDiff | null>(null);
  const [gitLoadingDiff, setGitLoadingDiff] = useState(false);
  const [gitPanelOpen, setGitPanelOpen] = useState(false);
  const [gitWidth, setGitWidth] = useState(340);
  const [gitResizing, setGitResizing] = useState(false);
  const wide = useIsWide(760);

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

  const refreshGitChanges = useCallback(() => {
    invoke<GitInfo>("get_git_changes")
      .then((info) => {
        setGitBranch(info.branch);
        setGitChanges(info.changes);
      })
      .catch(() => {});
  }, []);

  const selectGitFile = useCallback((path: string) => {
    setGitLoadingDiff(true);
    setGitDiff(null);
    invoke<GitDiff>("get_git_diff", { path })
      .then(setGitDiff)
      .catch(() => {})
      .finally(() => setGitLoadingDiff(false));
  }, []);

  const backToGitList = useCallback(() => {
    setGitDiff(null);
  }, []);

  const startGitResize = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      setGitResizing(true);
      const startX = e.clientX;
      const startW = gitWidth;
      const onMove = (ev: MouseEvent) => {
        const w = startW + (startX - ev.clientX);
        // Предел: панель не должна съедать чат — оставляем чату минимум 360px.
        const maxW = Math.min(820, window.innerWidth - 360);
        setGitWidth(Math.min(maxW, Math.max(260, w)));
      };
      const onUp = () => {
        setGitResizing(false);
        window.removeEventListener("mousemove", onMove);
        window.removeEventListener("mouseup", onUp);
      };
      window.addEventListener("mousemove", onMove);
      window.addEventListener("mouseup", onUp);
    },
    [gitWidth],
  );

  useEffect(() => {
    refreshGitChanges();
    const unlistenGit = listen<{ sessionId: string; branch: string; changes: GitFileChange[] }>(
      "git-changes",
      (e) => {
        if (e.payload.sessionId !== sessionId) return;
        setGitBranch(e.payload.branch ?? "");
        setGitChanges(e.payload.changes ?? []);
      },
    );
    const t = setInterval(refreshGitChanges, 4000);
    return () => {
      clearInterval(t);
      unlistenGit.then((f) => f());
    };
  }, [sessionId, refreshGitChanges]);

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
        busyStartRef.current = Date.now();
        lastActivityRef.current = Date.now();
        setElapsed(0);
        setStalled(false);
        setBusy(true);
        setTools([]);
        setMessages((m) => [
          ...m,
          { role: "user", text: e.payload.text },
          { role: "assistant", text: "", reasoning: "" },
        ]);
      },
    );
    const unlistenDelta = listen<{ sessionId: string; text: string }>(
      "opencode-delta",
      (e) => {
        if (e.payload.sessionId !== sessionId) return;
        lastActivityRef.current = Date.now();
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
    const unlistenReasoning = listen<{ sessionId: string; text: string }>(
      "opencode-reasoning-delta",
      (e) => {
        if (e.payload.sessionId !== sessionId) return;
        lastActivityRef.current = Date.now();
        setMessages((m) => {
          const next = [...m];
          const last = next[next.length - 1];
          if (!last || last.role !== "assistant") {
            next.push({ role: "assistant", text: "", reasoning: e.payload.text });
          } else {
            next[next.length - 1] = {
              ...last,
              reasoning: (last.reasoning ?? "") + e.payload.text,
            };
          }
          return next;
        });
      },
    );
    const unlistenDone = listen<{ sessionId: string }>("opencode-done", (e) => {
      if (e.payload.sessionId !== sessionId) return;
      playReceive();
      const dur = busyStartRef.current !== null ? Date.now() - busyStartRef.current : 0;
      busyStartRef.current = null;
      setBusy(false);
      setStalled(false);
      setMessages((m) => {
        const next = [...m];
        const last = next[next.length - 1];
        if (last && last.role === "assistant") {
          next[next.length - 1] = { ...last, durationMs: dur };
        }
        return next;
      });
      // Обновляем счётчики после завершения ответа (БД может чуть отставать).
      setTimeout(refreshUsage, 500);
    });
    const unlistenOcError = listen<{ sessionId: string; error: string }>(
      "opencode-error",
      (e) => {
        if (e.payload.sessionId !== sessionId) return;
        busyStartRef.current = null;
        setBusy(false);
        setStalled(false);
      },
    );
    const unlistenTool = listen<{ sessionId: string } & ToolAction>(
      "opencode-tool",
      (e) => {
        if (e.payload.sessionId !== sessionId) return;
        lastActivityRef.current = Date.now();
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
      unlistenReasoning.then((f) => f());
      unlistenDone.then((f) => f());
      unlistenOcError.then((f) => f());
      unlistenTool.then((f) => f());
      unlistenPermission.then((f) => f());
      unlistenQuestion.then((f) => f());
    };
  }, [sessionId, refreshUsage]);

  useEffect(() => {
    bodyRef.current?.scrollTo({ top: bodyRef.current.scrollHeight });
  }, [messages, tools, permissions, questions]);

  // Тикающий таймер выполнения ответа (останавливается по завершении).
  useEffect(() => {
    if (!busy) {
      setElapsed(0);
      return;
    }
    const tick = () => {
      if (busyStartRef.current !== null) {
        setElapsed(Date.now() - busyStartRef.current);
      }
    };
    tick();
    const t = setInterval(tick, 1000);
    return () => clearInterval(t);
  }, [busy]);

  // Детект «зависшего» ответа (нет событий дольше порога).
  useEffect(() => {
    if (!busy) {
      setStalled(false);
      return;
    }
    const check = () => {
      setStalled(
        lastActivityRef.current > 0 &&
          Date.now() - lastActivityRef.current > STALL_THRESHOLD_MS,
      );
    };
    check();
    const t = setInterval(check, 5000);
    return () => clearInterval(t);
  }, [busy]);

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

  const abort = useCallback(() => {
    invoke("abort_session").catch(() => {});
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

  const runningTool = tools.find((t) => t.state === "running")?.name;

  return (
    <main className="response-view">
      <header className="response-view-header">
        <div className="chat-title">
          <span className="chat-logo"><Bot size={16} /></span>
          <div className="chat-title-text">
            <div className="chat-title-row">
              <h1>{info.title || "OpenCode"}</h1>
              {busy && <span className="chat-busy-dot" title="OpenCode отвечает" />}
            </div>
            {info.project && <span className="chat-project">{info.project}</span>}
          </div>
        </div>
        <div className="header-actions">
          {busy && (
            <button className="btn stop" onClick={abort} title="Прервать генерацию">
              <Square size={14} fill="currentColor" /> Стоп
            </button>
          )}
          {!wide && (
            <button
              className="icon-btn git-toggle"
              onClick={() => setGitPanelOpen((o) => !o)}
              title="Изменения в проекте"
            >
              <GitBranch size={16} />
              {gitChanges.length > 0 && <span className="git-badge">{gitChanges.length}</span>}
            </button>
          )}
          <button className="icon-btn" onClick={close} title="Закрыть">
            <X size={16} />
          </button>
        </div>
      </header>
      <div className="response-view-main">
        <div className="response-view-chat">
          <div className="response-view-body" ref={bodyRef}>
            <ToolChips tools={tools} />

            {messages.length === 0 ? (
              <p className="response-empty">
                Скажите фразу в VoiceBridge — ответ OpenCode появится здесь в реальном
                времени, вместе с инструментами и запросами действий.
              </p>
            ) : (
              messages.map((m, i) =>
                m.role === "user" ? (
                  <div key={i} className="chat-msg user">
                    <div className="chat-avatar user">
                      <User size={14} />
                    </div>
                    <div className="chat-msg-body">
                      <div className="chat-bubble user-bubble">{m.text}</div>
                    </div>
                  </div>
                ) : (
                  <div key={i} className="chat-msg assistant">
                    <div className="chat-avatar assistant">
                      <Bot size={14} />
                    </div>
                    <div className="chat-msg-body">
                      <div className="chat-bubble assistant-bubble">
                        {m.reasoning ? (
                          <ReasoningBlock
                            text={m.reasoning}
                            streaming={busy && !m.text}
                          />
                        ) : null}
                        {m.text ? (
                          <Markdown>{m.text}</Markdown>
                        ) : !m.reasoning ? (
                          <span className="thinking-dots" aria-label="OpenCode думает">
                            <span />
                            <span />
                            <span />
                          </span>
                        ) : null}
                      </div>
                      <div className="chat-msg-meta">
                        {m.text && (
                          <button
                            className="msg-copy-btn"
                            onClick={() => copyMessage(m.text, i)}
                            title="Копировать сообщение"
                          >
                            {copiedIdx === i ? <Check size={13} /> : <Copy size={13} />}
                          </button>
                        )}
                        {(() => {
                          const isLast = i === messages.length - 1;
                          const dur = m.durationMs ?? (isLast && busy ? elapsed : undefined);
                          return dur !== undefined ? (
                            <span className="msg-duration" title="Время выполнения">
                              <Clock size={11} /> {formatDuration(dur)}
                            </span>
                          ) : null;
                        })()}
                      </div>
                    </div>
                  </div>
                ),
              )
            )}
          </div>
          {(permissions.length > 0 || questions.length > 0 || (stalled && busy)) && (
            <div className="action-dock">
              {permissions.map((p) => (
                <PermissionCard key={p.requestId} request={p} onReply={replyPermission} />
              ))}
              {questions.map((q) => (
                <QuestionCard
                  key={q.requestId}
                  request={q}
                  onAnswer={answerQuestion}
                  onReject={rejectQuestion}
                />
              ))}
              {stalled && busy && (
                <div className="stall-warning">
                  <div className="stall-warning-body">
                    <AlertTriangle size={16} className="stall-warning-icon" />
                    <div className="stall-warning-text">
                      <span className="stall-warning-title">OpenCode, возможно, завис</span>
                      <span className="stall-warning-msg">
                        Нет ответа уже {formatDuration(elapsed)}
                      </span>
                    </div>
                  </div>
                  <button className="btn small stop" onClick={abort}>
                    <Square size={13} fill="currentColor" /> Прервать
                  </button>
                </div>
              )}
            </div>
          )}
          <div className="chat-activity-bar">
            {busy ? (
              <>
                <span className="chat-activity-spinner" />
                <span className="chat-activity-text">
                  {runningTool ? (
                    <>
                      выполняет <code>{runningTool}</code>…
                    </>
                  ) : (
                    "OpenCode работает…"
                  )}
                </span>
              </>
            ) : (
              <span className="chat-activity-text idle">Готов к работе</span>
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
        </div>
        {wide && (
          <>
            <div
              className={`git-resize-handle ${gitResizing ? "dragging" : ""}`}
              onMouseDown={startGitResize}
              title="Изменить ширину панели"
            />
            <aside className="git-panel" style={{ width: gitWidth }}>
              <GitPanel
                branch={gitBranch}
                changes={gitChanges}
                selected={gitDiff}
                loadingDiff={gitLoadingDiff}
                onSelect={selectGitFile}
                onBack={backToGitList}
              />
            </aside>
          </>
        )}
      </div>
      {!wide && gitPanelOpen && (
        <div className="git-overlay" onClick={() => setGitPanelOpen(false)}>
          <aside className="git-drawer" onClick={(e) => e.stopPropagation()}>
            <GitPanel
              branch={gitBranch}
              changes={gitChanges}
              selected={gitDiff}
              loadingDiff={gitLoadingDiff}
              onSelect={selectGitFile}
              onBack={backToGitList}
            />
          </aside>
        </div>
      )}
    </main>
  );
}
