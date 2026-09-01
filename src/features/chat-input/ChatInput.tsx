import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Eye, Loader2, Mic, MicVocal, Send, Square } from "lucide-react";
import type { AppState } from "../../shared/types";
import { playRecordingStart, playRecordingStop } from "../../shared/lib/sounds";

const WAVE_BARS = 24;
/// Порог, после которого вставленный текст сворачивается в «вставлено ~N строк».
const PASTE_LINES_THRESHOLD = 3;
const PASTE_LEN_THRESHOLD = 300;
/// Маркер свёрнутой вставки в поле ввода.
const PASTE_MARKER_RE = /\[вставлено ~\d+[^\]]*\]/g;

/// Русская форма слова «строка» по числу.
function pluralLines(n: number): string {
  const m10 = n % 10;
  const m100 = n % 100;
  if (m10 === 1 && m100 !== 11) return "строка";
  if (m10 >= 2 && m10 <= 4 && (m100 < 12 || m100 > 14)) return "строки";
  return "строк";
}

/// Принадлежит ли текущая запись этому окну (по целевой сессии).
function ownsRecording(state: AppState, sessionId: string): boolean {
  const target = state.recordingSessionId ?? state.selectedSession?.sessionId ?? null;
  return target === sessionId;
}

/// Панель ввода в окне чата: текстовое поле + отправка + голосовая кнопка
/// (тап — запись, удержание — говорить, авто-остановка по тишине).
export default function ChatInput({ sessionId }: { sessionId: string }) {
  const [text, setText] = useState("");
  const [pastes, setPastes] = useState<{ lines: number; text: string }[]>([]);
  const [isRecording, setIsRecording] = useState(false);
  const [isProcessing, setIsProcessing] = useState(false);
  const [level, setLevel] = useState<number | null>(null);
  const [sendMode, setSendMode] = useState("direct");

  const recordingRef = useRef(false);
  const processingRef = useRef(false);
  const silenceTimeoutRef = useRef(3);
  const silenceSinceRef = useRef<number | null>(null);
  const pressedRef = useRef(false);
  const holdRef = useRef(false);
  const holdTimerRef = useRef<number | null>(null);
  const lastTranscriptRef = useRef("");
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    invoke<AppState>("get_app_state")
      .then((s) => {
        const mine = ownsRecording(s, sessionId);
        setIsRecording((s.recording || s.status === "recording") && mine);
        setIsProcessing(
          s.status === "processing" && mine && s.statusMessage.includes("Распознавание"),
        );
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
      setIsProcessing(
        e.payload.status === "processing" && mine && e.payload.statusMessage.includes("Распознавание"),
      );
      // В режиме предпроверки распознанный текст попадает в поле ввода —
      // но только в окно той сессии, которой адресована запись.
      // Существующий текст не затирается: новый дописывается в конец.
      if (e.payload.transcript && e.payload.sendMode === "confirm" && mine) {
        if (e.payload.transcript !== lastTranscriptRef.current) {
          lastTranscriptRef.current = e.payload.transcript;
          const t = e.payload.transcript;
          setText((prev) => {
            if (!prev) return t;
            const sep = prev.endsWith(" ") || prev.endsWith("\n") ? "" : " ";
            return prev + sep + t;
          });
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

  useEffect(() => {
    processingRef.current = isProcessing;
  }, [isProcessing]);

  // Звук начала/конца записи (только в окне, которому принадлежит запись).
  const prevRecordingRef = useRef(false);
  useEffect(() => {
    if (isRecording && !prevRecordingRef.current) playRecordingStart();
    else if (!isRecording && prevRecordingRef.current) playRecordingStop();
    prevRecordingRef.current = isRecording;
  }, [isRecording]);

  // Авто-рост textarea по содержимому (до максимума).
  useEffect(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, 160)}px`;
  }, [text]);

  const send = useCallback(() => {
    // Разворачиваем маркеры вставок в порядке их появления в тексте.
    let full = text;
    if (pastes.length > 0) {
      let i = 0;
      full = text.replace(PASTE_MARKER_RE, () => pastes[i++]?.text ?? "");
    }
    const final = full.trim();
    if (!final) return;
    invoke("send_text", { text: final }).catch(() => {});
    setText("");
    setPastes([]);
  }, [text, pastes]);

  const onPaste = useCallback(
    (e: React.ClipboardEvent) => {
      const pasted = e.clipboardData.getData("text");
      if (!pasted) return;
      const lines = pasted.split("\n").length;
      if (lines <= PASTE_LINES_THRESHOLD && pasted.length <= PASTE_LEN_THRESHOLD) return;
      // Большая вставка: ставим маркер в месте курсора.
      e.preventDefault();
      const el = textareaRef.current;
      const value = el ? el.value : text;
      const start = el?.selectionStart ?? value.length;
      const end = el?.selectionEnd ?? value.length;
      const marker = `[вставлено ~${lines} ${pluralLines(lines)}]`;
      const next = value.slice(0, start) + marker + value.slice(end);
      setText(next);
      setPastes((p) => [...p, { lines, text: pasted }]);
      const pos = start + marker.length;
      requestAnimationFrame(() => {
        const ta = textareaRef.current;
        if (ta) {
          ta.focus();
          ta.setSelectionRange(pos, pos);
        }
      });
    },
    [text],
  );

  const onPointerDown = useCallback((e: React.PointerEvent) => {
    if (processingRef.current) return;
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
          <textarea
            ref={textareaRef}
            className="chat-text"
            rows={1}
            value={text}
            onChange={(e) => setText(e.target.value)}
            onPaste={onPaste}
            placeholder={isRecording ? "Говорите…" : "Сообщение…"}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                send();
              }
            }}
          />
        )}
        {!isDirect && (
          <button className="send-btn" onClick={send} title="Отправить">
            <Send size={16} />
          </button>
        )}
        <button
          className={`voice-btn ${isRecording ? "recording" : ""} ${isProcessing ? "processing" : ""}`}
          onPointerDown={onPointerDown}
          onPointerUp={onPointerUp}
          onPointerCancel={onPointerUp}
          title={
            isProcessing
              ? "Распознавание речи…"
              : sendMode === "confirm"
                ? "Запись → в поле ввода"
                : "Тап — запись · удержание — говорить"
          }
          aria-label={
            isRecording
              ? "Остановить запись"
              : isProcessing
                ? "Распознавание речи…"
                : "Записать голосовое"
          }
        >
          {isRecording ? (
            <Square size={16} fill="currentColor" />
          ) : isProcessing ? (
            <Loader2 size={16} className="voice-spinner" />
          ) : (
            <Mic size={16} />
          )}
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
