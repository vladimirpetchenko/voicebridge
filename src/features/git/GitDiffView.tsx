import { useMemo } from "react";
import { GitBranch } from "lucide-react";
import type { GitDiff } from "../../shared/types";
import { parseDiff } from "./gitFormat";

/// Unified diff файла с подсветкой добавленных/удалённых строк и номерами строк.
export function GitDiffView({ diff }: { diff: GitDiff }) {
  const rows = useMemo(() => parseDiff(diff.diff), [diff.diff]);
  if (!diff.diff) {
    return (
      <div className="git-empty">
        <GitBranch size={20} />
        <span>Дифф недоступен (возможно, бинарный файл)</span>
      </div>
    );
  }
  return (
    <div className="git-diff">
      {diff.tooLarge && <div className="git-diff-too-large">Файл большой — показана часть.</div>}
      {rows.map((r, i) => (
        <div key={i} className={`git-diff-line ${r.cls}`}>
          <span className="git-diff-num old">{r.old}</span>
          <span className="git-diff-num new">{r.neu}</span>
          <span className="git-diff-text">{r.text || " "}</span>
        </div>
      ))}
    </div>
  );
}
