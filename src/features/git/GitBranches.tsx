import { Check, GitBranch as GitBranchIcon } from "lucide-react";
import type { GitBranchInfo } from "../../shared/types";

/// Список локальных веток: текущая ветка выделена.
export function GitBranchList({
  branches,
  loading,
}: {
  branches: GitBranchInfo[];
  loading: boolean;
}) {
  if (loading && branches.length === 0) {
    return <div className="git-loading">Загрузка веток…</div>;
  }
  if (branches.length === 0) {
    return (
      <div className="git-empty">
        <GitBranchIcon size={22} />
        <span>Нет веток</span>
      </div>
    );
  }
  return (
    <div className="git-branch-list">
      {branches.map((b) => (
        <div
          key={b.name}
          className={`git-branch-row${b.current ? " current" : ""}`}
          title={b.name}
        >
          <GitBranchIcon
            size={14}
            className={`git-branch-row-icon${b.current ? " current" : ""}`}
          />
          <span className="git-branch-row-name">{b.name}</span>
          {b.current && (
            <span className="git-branch-current">
              <Check size={11} /> текущая
            </span>
          )}
        </div>
      ))}
    </div>
  );
}
