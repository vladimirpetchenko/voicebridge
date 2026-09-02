import { GitCommitHorizontal, User } from "lucide-react";
import type { GitCommit, GitCommitDetail } from "../../shared/types";
import {
  basename,
  formatCommitDate,
  gitStatusMeta,
  relativeTime,
  shortHash,
} from "./gitFormat";
import { GitDiffView } from "./GitDiffView";

/// Список коммитов: хэш, сообщение, автор и относительное время.
export function GitCommitList({
  commits,
  loading,
  onSelect,
}: {
  commits: GitCommit[];
  loading: boolean;
  onSelect: (hash: string) => void;
}) {
  if (loading && commits.length === 0) {
    return <div className="git-loading">Загрузка коммитов…</div>;
  }
  if (commits.length === 0) {
    return (
      <div className="git-empty">
        <GitCommitHorizontal size={22} />
        <span>Нет коммитов</span>
      </div>
    );
  }
  return (
    <div className="git-commit-list">
      {commits.map((c) => (
        <button
          key={c.hash}
          className="git-commit-row"
          onClick={() => onSelect(c.hash)}
          title={c.message}
        >
          <span className="git-commit-row-top">
            <span className="git-commit-hash">{shortHash(c.hash)}</span>
            <span className="git-commit-msg">{c.message}</span>
          </span>
          <span className="git-commit-row-meta">
            <span className="git-commit-author">
              <User size={11} /> {c.author}
            </span>
            <span className="git-commit-date">{relativeTime(c.date)}</span>
          </span>
        </button>
      ))}
    </div>
  );
}

/// Детали коммита: метаданные, список файлов и дифф.
export function GitCommitView({
  detail,
  loading,
}: {
  detail: GitCommitDetail | null;
  loading: boolean;
}) {
  if (loading) {
    return <div className="git-loading">Загрузка коммита…</div>;
  }
  if (!detail) {
    return (
      <div className="git-empty">
        <GitCommitHorizontal size={22} />
        <span>Коммит недоступен</span>
      </div>
    );
  }
  const diff = {
    path: "",
    status: "modified",
    tooLarge: detail.tooLarge,
    diff: detail.diff,
  };
  return (
    <div className="git-commit-view">
      <div className="git-commit-view-head">
        <div className="git-commit-view-title">
          <span className="git-commit-hash">{shortHash(detail.hash)}</span>
          <span className="git-commit-msg">{detail.message}</span>
        </div>
        <div className="git-commit-view-meta">
          <span className="git-commit-author">
            <User size={12} /> {detail.author}
          </span>
          <span className="git-commit-date">{formatCommitDate(detail.date)}</span>
        </div>
      </div>
      {detail.files.length > 0 && (
        <div className="git-commit-files">
          {detail.files.map((f) => {
            const meta = gitStatusMeta(f.status);
            const Icon = meta.Icon;
            return (
              <div key={f.path} className="git-commit-file-row" title={f.path}>
                <Icon size={13} className={`git-file-icon ${meta.cls}`} />
                <span className="git-commit-file-path">
                  <span className="git-file-name">{basename(f.path)}</span>
                  <span className="git-file-dir">
                    {f.path !== basename(f.path) ? f.path : ""}
                  </span>
                </span>
                <span className="git-file-stats">
                  {f.additions > 0 && <span className="git-adds">+{f.additions}</span>}
                  {f.deletions > 0 && <span className="git-dels">−{f.deletions}</span>}
                </span>
              </div>
            );
          })}
        </div>
      )}
      <div className="git-commit-diff">
        {detail.diff ? (
          <GitDiffView diff={diff} />
        ) : (
          <div className="git-empty">
            <GitCommitHorizontal size={20} />
            <span>Нет диффа (возможно, merge-коммит)</span>
          </div>
        )}
      </div>
    </div>
  );
}
