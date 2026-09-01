import type { ToolAction } from "../../../shared/types";
import { toolIcon } from "../toolIcon";

/// Чипы запущенных инструментов OpenCode.
export function ToolChips({ tools }: { tools: ToolAction[] }) {
  if (tools.length === 0) return null;
  return (
    <div className="tool-list response-tools">
      {tools.map((t) => {
        const Icon = toolIcon(t.name);
        return (
          <span key={t.callId} className={`tool-chip ${t.state}`}>
            <Icon size={13} /> {t.name || "инструмент"}
          </span>
        );
      })}
    </div>
  );
}
