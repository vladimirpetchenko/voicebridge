import { useMemo, useState } from "react";
import { ArrowLeft, ChevronDown, ChevronRight, Folder, GitBranch } from "lucide-react";
import type { GitDiff, GitFileChange } from "../../shared/types";
import { basename, gitStatusMeta, groupChangesByDir } from "./gitFormat";
import { GitDiffView } from "./GitDiffView";

/// Панель изменений: файлы сгруппированы по папкам + дифф по клику.
export function GitPanel({
  branch,
  changes,
  selected,
  loadingDiff,
  onSelect,
  onBack,
}: {
  branch: string;
  changes: GitFileChange[];
  selected: GitDiff | null;
  loadingDiff: boolean;
  onSelect: (path: string) => void;
  onBack: () => void;
}) {
  const totalAdd = changes.reduce((s, c) => s + c.additions, 0);
  const totalDel = changes.reduce((s, c) => s + c.deletions, 0);
  const groups = useMemo(() => groupChangesByDir(changes), [changes]);
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());

  const toggleDir = (dir: string) => {
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(dir)) {
        next.delete(dir);
      } else {
        next.add(dir);
      }
      return next;
    });
  };

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
            <div className="git-panel-heading">
              <span className="git-branch-name" title={branch}>
                {branch || "Изменения"}
              </span>
              <span className="git-summary">
                <span>{changes.length} файлов</span>
                {changes.length > 0 && (
                  <>
                    <span className="git-adds">+{totalAdd}</span>
                    <span className="git-dels">−{totalDel}</span>
                  </>
                )}
              </span>
            </div>
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
          {groups.map((g) => {
            const isCollapsed = collapsed.has(g.dir);
            return (
              <div key={g.dir || "__root__"} className="git-group">
                {g.dir !== "" && (
                  <button
                    className="git-folder-header"
                    onClick={() => toggleDir(g.dir)}
                    title={g.dir}
                  >
                    {isCollapsed ? <ChevronRight size={13} /> : <ChevronDown size={13} />}
                    <Folder size={13} className="git-folder-icon" />
                    <span className="git-folder-name">{g.dir}</span>
                    <span className="git-folder-stats">
                      {g.additions > 0 && <span className="git-adds">+{g.additions}</span>}
                      {g.deletions > 0 && <span className="git-dels">−{g.deletions}</span>}
                    </span>
                  </button>
                )}
                {!isCollapsed && (
                  <div className="git-group-files">
                    {g.changes.map((c) => {
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
          })}
        </div>
      )}
    </div>
  );
}
