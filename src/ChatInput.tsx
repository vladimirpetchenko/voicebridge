import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Eye, Mic, MicVocal, Send, Square } from "lucide-react";
import type { AppState } from "./types";
import { playRecordingStart, playRecordingStop } from "./sounds";

const WAVE_BARS = 24;

/// Принадлежит ли текущая запись этому окну (по целевой сессии).
function ownsRecording(state: AppState, sessionId: string): boolean {
  const target = state.recordingSessionId ?? state.selectedSession?.sessionId ?? null;
  return target === sessionId;
}

/// Панель ввода в окне чата: текстовое поле + отправка + голосовая кнопка
/// (тап — запись, удержание — говорить, авто-остановка по тишине).
export default function ChatInput({ sessionId }: { sessionId: string }) {
  const [text, setText] = useState("");
  const [isRecording, setIsRecording] = useState(false);
  const [level, setLevel] = useState<number | null>(null);
  const [sendMode, setSendMode] = useState("direct");

  const recordingRef = useRef(false);
  const silenceTimeoutRef = useRef(3);
  const silenceSinceRef = useRef<number | null>(null);
  const pressedRef = useRef(false);
  const holdRef = useRef(false);
  const holdTimerRef = useRef<number | null>(null);
  const lastTranscriptRef = useRef("");

  useEffect(() => {
    invoke<AppState>("get_app_state")
      .then((s) => {
        const mine = ownsRecording(s, sessionId);
        setIsRecording((s.recording || s.status === "recording") && mine);
        silenceTimeoutRef.current = s.silenceTimeout;
        setSendMode(s.sendMode);
        lastTranscriptRef.current = s.transcript;
      })
      .catch(() => {});

    const unlistenState = listen<AppState>("state-changed", (e) => {
      silenceTimeoutRef.current = e.payload.silenceTimeout;
      setSendMode(e.payload.sendMode);
      const mine = ownsRecording(e.payload, sessionId);
      setIsRecording((e.payload.recording || e.payload.status === "recording") && mine);
      // В режиме предпроверки распознанный текст попадает в поле ввода —
      // но только в окно той сессии, которой адресована запись.
      if (e.payload.transcript && e.payload.sendMode === "confirm" && mine) {
        if (e.payload.transcript !== lastTranscriptRef.current) {
          lastTranscriptRef.current = e.payload.transcript;
          setText(e.payload.transcript);
        }
      }
    });
    const unlistenLevel = listen<number>("audio-level", (e) => {
      const lv = e.payload;
      setLevel(lv);
      const timeout = silenceTimeoutRef.current;
      if (timeout > 0 && recordingRef.current) {
        if (lv < 0.03) {
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

    return () => {
      unlistenState.then((f) => f());
      unlistenLevel.then((f) => f());
    };
  }, []);

  useEffect(() => {
    recordingRef.current = isRecording;
    if (!isRecording) setLevel(null);
    if (isRecording) silenceSinceRef.current = null;
  }, [isRecording]);

  // Звук начала/конца записи (только в окне, которому принадлежит запись).
  const prevRecordingRef = useRef(false);
  useEffect(() => {
    if (isRecording && !prevRecordingRef.current) playRecordingStart();
    else if (!isRecording && prevRecordingRef.current) playRecordingStop();
    prevRecordingRef.current = isRecording;
  }, [isRecording]);

  const send = useCallback(() => {
    const t = text.trim();
    if (!t) return;
    invoke("send_text", { text: t }).catch(() => {});
    setText("");
  }, [text]);

  const onPointerDown = useCallback((e: React.PointerEvent) => {
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
        invoke("start_recording").catch(() => {});
      }
    }, 300);
  }, []);

  const onPointerUp = useCallback(() => {
    if (!pressedRef.current) return;
    pressedRef.current = false;
    if (holdTimerRef.current !== null) {
      clearTimeout(holdTimerRef.current);
      holdTimerRef.current = null;
    }
    if (holdRef.current) {
      holdRef.current = false;
      invoke("stop_recording").catch(() => {});
    } else {
      invoke("toggle_recording").catch(() => {});
    }
  }, []);

  const isDirect = sendMode === "direct";

  const toggleSendMode = useCallback(() => {
    const next = sendMode === "direct" ? "confirm" : "direct";
    invoke("set_send_mode", { mode: next }).catch(() => {});
  }, [sendMode]);

  return (
    <div className="chat-input">
      {isRecording && (
        <div className="voice-wave" aria-hidden>
          {Array.from({ length: WAVE_BARS }).map((_, i) => {
            let h = 4;
            if (level != null) {
              const amp = 0.3 + 0.7 * Math.abs(Math.sin(i * 0.8 + 0.4));
              h = Math.max(4, Math.min(26, 4 + level * amp * 30));
            }
            return <span key={i} className="voice-wave-bar" style={{ height: `${h}px` }} />;
          })}
        </div>
      )}
      <div className={`chat-input-row ${isDirect ? "direct" : ""}`}>
        {!isDirect && (
          <input
            className="chat-text"
            value={text}
            onChange={(e) => setText(e.target.value)}
            placeholder={isRecording ? "Говорите…" : "Сообщение…"}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                send();
              }
            }}
          />
        )}
        {!isDirect && text.trim() && (
          <button className="send-btn" onClick={send} title="Отправить">
            <Send size={16} />
          </button>
        )}
        <button
          className={`voice-btn ${isRecording ? "recording" : ""}`}
          onPointerDown={onPointerDown}
          onPointerUp={onPointerUp}
          onPointerCancel={onPointerUp}
          title={sendMode === "confirm" ? "Запись → в поле ввода" : "Тап — запись · удержание — говорить"}
          aria-label={isRecording ? "Остановить запись" : "Записать голосовое"}
        >
          {isRecording ? <Square size={20} fill="currentColor" /> : <Mic size={20} />}
        </button>
      </div>
      <div className="chat-input-footer">
        <button
          className="send-mode-badge"
          onClick={toggleSendMode}
          title="Нажмите, чтобы переключить режим отправки"
        >
          {isDirect ? (
            <>
              <MicVocal size={13} /> Голос сразу в чат
            </>
          ) : (
            <>
              <Eye size={13} /> Предпросмотр перед отправкой
            </>
          )}
        </button>
      </div>
    </div>
  );
}
