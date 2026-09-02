import type { ToolAction } from "../../../shared/types";
import { toolIcon } from "../toolIcon";

/// Короткое однострочное превью для inline-показа в чипе.
function inlinePreview(s: string, n: number): string {
  const oneLine = s.replace(/\s+/g, " ").trim();
  return oneLine.length > n ? oneLine.slice(0, n) + "…" : oneLine;
}

/// Чипы запущенных инструментов OpenCode. На hover показываются вход/выход.
export function ToolChips({ tools }: { tools: ToolAction[] }) {
  if (tools.length === 0) return null;
  return (
    <div className="tool-list response-tools">
      {tools.map((t) => {
        const Icon = toolIcon(t.name);
        const hasDetail = Boolean(t.input || t.output);
        return (
          <span key={t.callId} className={`tool-chip ${t.state}`}>
            <Icon size={13} /> {t.name || "инструмент"}
            {t.state === "done" && t.output && (
              <span className="tool-chip-output">{inlinePreview(t.output, 60)}</span>
            )}
            {hasDetail && (
              <span className="tool-chip-tip">
                {t.input && (
                  <div className="tip-row">
                    <b>Вход</b>
                    <pre>{t.input}</pre>
                  </div>
                )}
                {t.output && (
                  <div className="tip-row">
                    <b>Выход</b>
                    <pre>{t.output}</pre>
                  </div>
                )}
              </span>
            )}
          </span>
        );
      })}
    </div>
  );
}
