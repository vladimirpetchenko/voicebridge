import { useMemo, useState } from "react";
import { ArrowLeft, ChevronDown, ChevronRight, Folder, GitBranch } from "lucide-react";
import type { GitBranchInfo, GitCommit, GitCommitDetail, GitDiff, GitFileChange } from "../../shared/types";
import { basename, gitStatusMeta, groupChangesByDir, shortHash } from "./gitFormat";
import { GitDiffView } from "./GitDiffView";
import { GitCommitList, GitCommitView } from "./GitCommits";
import { GitBranchList } from "./GitBranches";

/// Вкладка Git-панели: изменения рабочего дерева, история коммитов или ветки.
export type GitTab = "changes" | "commits" | "branches";

/// Панель Git: вкладки «Изменения»/«История»/«Ветки», файлы + дифф, коммиты, ветки.
export function GitPanel({
  branch,
  changes,
  commits,
  branches,
  selected,
  loadingDiff,
  commitDetail,
  loadingCommit,
  loadingCommits,
  loadingBranches,
  tab,
  onTab,
  onSelect,
  onBack,
  onSelectCommit,
  onBackCommit,
}: {
  branch: string;
  changes: GitFileChange[];
  commits: GitCommit[];
  branches: GitBranchInfo[];
  selected: GitDiff | null;
  loadingDiff: boolean;
  commitDetail: GitCommitDetail | null;
  loadingCommit: boolean;
  loadingCommits: boolean;
  loadingBranches: boolean;
  tab: GitTab;
  onTab: (tab: GitTab) => void;
  onSelect: (path: string) => void;
  onBack: () => void;
  onSelectCommit: (hash: string) => void;
  onBackCommit: () => void;
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

  const showingFileDiff = tab === "changes" && selected !== null;
  const showingCommit = tab === "commits" && commitDetail !== null;

  return (
    <div className="git-panel-inner">
      <div className="git-tabs">
        <button
          className={`git-tab ${tab === "changes" ? "active" : ""}`}
          onClick={() => onTab("changes")}
        >
          Изменения
        </button>
        <button
          className={`git-tab ${tab === "commits" ? "active" : ""}`}
          onClick={() => onTab("commits")}
        >
          История
        </button>
        <button
          className={`git-tab ${tab === "branches" ? "active" : ""}`}
          onClick={() => onTab("branches")}
        >
          Ветки
        </button>
      </div>
      <div className="git-panel-header">
        {showingFileDiff ? (
          <button className="icon-btn git-back" onClick={onBack} title="К списку файлов">
            <ArrowLeft size={14} />
          </button>
        ) : showingCommit ? (
          <button
            className="icon-btn git-back"
            onClick={onBackCommit}
            title="К списку коммитов"
          >
            <ArrowLeft size={14} />
          </button>
        ) : (
          <GitBranch size={15} className="git-panel-logo" />
        )}
        <div className="git-panel-title">
          {showingFileDiff ? (
            <span className="git-file-name" title={selected.path}>
              {basename(selected.path)}
            </span>
          ) : showingCommit ? (
            <span className="git-branch-name" title={commitDetail.hash}>
              Коммит {shortHash(commitDetail.hash)}
            </span>
          ) : (
            <div className="git-panel-heading">
              <span className="git-branch-name" title={branch}>
                {branch || "Изменения"}
              </span>
              <span className="git-summary">
                {tab === "changes" ? (
                  <>
                    <span>{changes.length} файлов</span>
                    {changes.length > 0 && (
                      <>
                        <span className="git-adds">+{totalAdd}</span>
                        <span className="git-dels">−{totalDel}</span>
                      </>
                    )}
                  </>
                ) : tab === "commits" ? (
                  <span>{commits.length} коммитов</span>
                ) : (
                  <span>{branches.length} веток</span>
                )}
              </span>
            </div>
          )}
        </div>
      </div>

      {tab === "changes" ? (
        showingFileDiff ? (
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
        )
      ) : tab === "commits" ? (
        showingCommit ? (
          <GitCommitView detail={commitDetail} loading={loadingCommit} />
        ) : (
          <GitCommitList commits={commits} loading={loadingCommits} onSelect={onSelectCommit} />
        )
      ) : (
        <GitBranchList branches={branches} loading={loadingBranches} />
      )}
    </div>
  );
}
