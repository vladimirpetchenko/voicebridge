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
    <div className="action-card">
      <div className="action-card-title">OpenCode запрашивает разрешение</div>
      <div className="action-desc">
        Инструмент: <code>{request.permission || "?"}</code>
      </div>
      {request.patterns.length > 0 && (
        <pre className="action-patterns">{request.patterns.join("\n")}</pre>
      )}
      <div className="action-buttons">
        <button className="btn play" onClick={() => onReply(request, "once")}>
          Разрешить
        </button>
        <button className="btn" onClick={() => onReply(request, "always")}>
          Всегда
        </button>
        <button className="btn stop" onClick={() => onReply(request, "reject")}>
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
    <div className="action-card">
      <div className="action-card-title">{first?.header || "Вопрос OpenCode"}</div>
      <div className="action-desc">{first?.question || ""}</div>
      <div className="action-options">
        {(first?.options ?? []).map((opt) => (
          <button
            key={opt.label}
            className="btn"
            onClick={() => onAnswer(request, [[opt.label]])}
            title={opt.description}
          >
            {opt.label}
          </button>
        ))}
      </div>
      <div className="action-buttons">
        <button className="btn stop" onClick={() => onReject(request)}>
          Отклонить
        </button>
      </div>
    </div>
  );
}
