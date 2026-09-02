import { MessagesSquare, User } from "lucide-react";
import type { ConversationMessage } from "../../shared/types";

/// Сворачивает текст сообщения в однострочное превью для списка.
function preview(m: ConversationMessage): string {
  const t = (m.text || "").trim().replace(/\s+/g, " ");
  return t.length > 120 ? t.slice(0, 120) + "…" : t;
}

/// Панель сообщений: список запросов пользователя с переходом к любому из них.
export function MessagePanel({
  messages,
  activeIdx,
  onSelect,
}: {
  messages: ConversationMessage[];
  activeIdx: number | null;
  onSelect: (idx: number) => void;
}) {
  const userCount = messages.filter((m) => m.role === "user").length;

  return (
    <div className="msg-panel-inner">
      <div className="msg-panel-header">
        <MessagesSquare size={15} className="msg-panel-logo" />
        <span className="msg-panel-title">Сообщения</span>
        <span className="msg-panel-count">{userCount}</span>
      </div>
      {userCount === 0 ? (
        <div className="msg-panel-empty">
          <MessagesSquare size={22} />
          <span>Сообщений пока нет</span>
        </div>
      ) : (
        <div className="msg-panel-list">
          {messages.map((m, i) =>
            m.role === "user" ? (
              <button
                key={i}
                className={`msg-row${activeIdx === i ? " active" : ""}`}
                onClick={() => onSelect(i)}
                title={preview(m)}
              >
                <span className="msg-row-icon user">
                  <User size={13} />
                </span>
                <span className="msg-row-body">
                  <span className="msg-row-preview">{preview(m) || "…"}</span>
                </span>
              </button>
            ) : null,
          )}
        </div>
      )}
    </div>
  );
}
