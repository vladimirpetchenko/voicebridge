import { HelpCircle, ShieldAlert } from "lucide-react";
import type { PermissionRequest, QuestionRequest } from "../../../shared/types";

/// Карточка запроса разрешения от OpenCode.
export function PermissionCard({
  request,
  onReply,
}: {
  request: PermissionRequest;
  onReply: (req: PermissionRequest, reply: string) => void;
}) {
  return (
    <div className="action-card permission">
      <div className="action-card-title">
        <ShieldAlert size={16} className="action-card-icon" />
        <span>OpenCode запрашивает разрешение</span>
      </div>
      <div className="action-desc">
        Инструмент: <code>{request.permission || "?"}</code>
      </div>
      {request.patterns.length > 0 && (
        <pre className="action-patterns">{request.patterns.join("\n")}</pre>
      )}
      <div className="action-buttons">
        <button className="btn small play" onClick={() => onReply(request, "once")}>
          Разрешить
        </button>
        <button className="btn small" onClick={() => onReply(request, "always")}>
          Всегда
        </button>
        <button className="btn small stop" onClick={() => onReply(request, "reject")}>
          Запретить
        </button>
      </div>
    </div>
  );
}

/// Карточка вопроса OpenCode (с вариантами ответа).
export function QuestionCard({
  request,
  onAnswer,
  onReject,
}: {
  request: QuestionRequest;
  onAnswer: (req: QuestionRequest, answers: string[][]) => void;
  onReject: (req: QuestionRequest) => void;
}) {
  const first = request.questions[0];
  return (
    <div className="action-card question">
      <div className="action-card-title">
        <HelpCircle size={16} className="action-card-icon" />
        <span>{first?.header || "Вопрос OpenCode"}</span>
      </div>
      <div className="action-desc">{first?.question || ""}</div>
      <div className="action-options">
        {(first?.options ?? []).map((opt) => (
          <button
            key={opt.label}
            className="btn small"
            onClick={() => onAnswer(request, [[opt.label]])}
            title={opt.description}
          >
            {opt.label}
          </button>
        ))}
      </div>
      <div className="action-buttons">
        <button className="btn small stop" onClick={() => onReject(request)}>
          Отклонить
        </button>
      </div>
    </div>
  );
}
