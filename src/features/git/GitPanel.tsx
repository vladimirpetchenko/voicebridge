import { ArrowLeft, GitBranch } from "lucide-react";
import type { GitDiff, GitFileChange } from "../../shared/types";
import { basename, dirname, gitStatusMeta } from "./gitFormat";
import { GitDiffView } from "./GitDiffView";

/// Панель изменений: список файлов + дифф по клику.
export function GitPanel({
  changes,
  selected,
  loadingDiff,
  onSelect,
  onBack,
}: {
  changes: GitFileChange[];
  selected: GitDiff | null;
  loadingDiff: boolean;
  onSelect: (path: string) => void;
  onBack: () => void;
}) {
  const totalAdd = changes.reduce((s, c) => s + c.additions, 0);
  const totalDel = changes.reduce((s, c) => s + c.deletions, 0);

  return (
    <div className="git-panel-inner">
      <div className="git-panel-header">
        {selected ? (
          <button className="icon-btn git-back" onClick={onBack} title="К списку файлов">
            <ArrowLeft size={14} />
          </button>
        ) : (
          <GitBranch size={15} className="git-panel-logo" />
        )}
        <div className="git-panel-title">
          {selected ? (
            <span className="git-file-name" title={selected.path}>
              {basename(selected.path)}
            </span>
          ) : (
            <>
              <span>Изменения</span>
              {changes.length > 0 && (
                <span className="git-summary">
                  <span className="git-adds">+{totalAdd}</span>
                  <span className="git-dels">−{totalDel}</span>
                </span>
              )}
            </>
          )}
        </div>
      </div>

      {selected ? (
        loadingDiff ? (
          <div className="git-loading">Загрузка диффа…</div>
        ) : (
          <GitDiffView diff={selected} />
        )
      ) : changes.length === 0 ? (
        <div className="git-empty">
          <GitBranch size={22} />
          <span>Нет изменений</span>
        </div>
      ) : (
        <div className="git-file-list">
          {changes.map((c) => {
            const meta = gitStatusMeta(c.status);
            const Icon = meta.Icon;
            return (
              <button
                key={c.path}
                className="git-file-row"
                onClick={() => onSelect(c.path)}
                title={c.path}
              >
                <Icon size={14} className={`git-file-icon ${meta.cls}`} />
                <span className="git-file-path">
                  <span className="git-file-name">{basename(c.path)}</span>
                  <span className="git-file-dir">{dirname(c.path)}</span>
                </span>
                <span className="git-file-stats">
                  {c.additions > 0 && <span className="git-adds">+{c.additions}</span>}
                  {c.deletions > 0 && <span className="git-dels">−{c.deletions}</span>}
                </span>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
