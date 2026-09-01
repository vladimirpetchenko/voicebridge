import { ChevronDown, ChevronUp, Play, Plus, RefreshCw, RotateCcw, Square, X } from "lucide-react";
import { relTime } from "../../shared/lib/format";
import type { OpenCodeInstance, OpenCodeSession, Project } from "../../shared/types";

export interface ProjectsPanelProps {
  visibleProjects: Project[];
  orphanInstances: OpenCodeInstance[];
  sessionsByPort: Map<number, OpenCodeSession[]>;
  instanceByPort: Map<number, OpenCodeInstance>;
  expandedProjects: Set<string>;
  selectedSessionId: string | undefined;
  openSessionIds: string[];
  hiddenProjects: string[];
  projects: Project[];
  onNewInstance: () => void;
  onRefresh: () => void;
  onCreateSession: (port: number, worktree: string) => void;
  onStartProject: (worktree: string) => void;
  onStopProject: (worktree: string) => void;
  onHideProject: (worktree: string) => void;
  onUnhideProject: (worktree: string) => void;
  onSelectSession: (inst: OpenCodeInstance, session: OpenCodeSession) => void;
  onToggleSessions: (id: string) => void;
}

/// Панель «Проекты» лаунчера: список проектов, их сессий и скрытых проектов.
export function ProjectsPanel(props: ProjectsPanelProps) {
  const {
    visibleProjects,
    orphanInstances,
    sessionsByPort,
    instanceByPort,
    expandedProjects,
    selectedSessionId,
    openSessionIds,
    hiddenProjects,
    projects,
    onNewInstance,
    onRefresh,
    onCreateSession,
    onStartProject,
    onStopProject,
    onHideProject,
    onUnhideProject,
    onSelectSession,
    onToggleSessions,
  } = props;

  return (
    <section className="projects-panel">
      <div className="panel-header">
        <span>Проекты</span>
        <div className="panel-header-actions">
          <button className="link-btn" onClick={onNewInstance} title="Новый проект (выбрать папку)">
            <Plus size={13} /> Новый
          </button>
          <button className="link-btn" onClick={onRefresh} title="Обновить">
            <RefreshCw size={13} />
          </button>
        </div>
      </div>
      <div className="projects-list">
        {visibleProjects.length === 0 && orphanInstances.length === 0 && (
          <div className="sessions-empty">
            Проекты не найдены. Откройте opencode в папке проекта.
          </div>
        )}
        {visibleProjects.map((p) => {
          const sessions = sessionsByPort.get(p.port) ?? [];
          const inst = instanceByPort.get(p.port);
          const expanded = expandedProjects.has(p.id);
          const visible = expanded ? sessions : sessions.slice(0, 3);
          const hidden = sessions.length - visible.length;
          return (
            <div key={p.id} className={`project-card ${p.running ? "running" : ""}`}>
              <div className="project-row">
                <span className={`session-dot ${p.running ? "on" : ""}`} />
                <div className="project-meta">
                  <span className="project-name" title={p.worktree}>{p.name}</span>
                  <span className="project-time">{relTime(p.updated)}</span>
                </div>
                {p.running && (
                  <button className="btn small new-session" onClick={() => onCreateSession(p.port, p.worktree)}>
                    <Plus size={13} /> Новая сессия
                  </button>
                )}
                {p.running ? (
                  <button className="btn small stop" onClick={() => onStopProject(p.worktree)} title="Остановить сервер">
                    <Square size={12} fill="currentColor" />
                  </button>
                ) : (
                  <button className="btn small play" onClick={() => onStartProject(p.worktree)} title="Запустить сервер OpenCode">
                    <Play size={12} fill="currentColor" />
                  </button>
                )}
                <button className="project-hide" onClick={() => onHideProject(p.worktree)} title="Скрыть проект из списка">
                  <X size={13} />
                </button>
              </div>
              {p.running && sessions.length === 0 && (
                <div className="session-row empty">Нет сессий</div>
              )}
              {visible.map((session) => {
                const active = selectedSessionId === session.id;
                const isOpen = openSessionIds.includes(session.id);
                return (
                  <button
                    key={session.id}
                    className={`session-row ${active ? "active" : ""}`}
                    onClick={() => inst && onSelectSession(inst, session)}
                  >
                    <span className={`session-dot ${active ? "on" : ""}`} />
                    <span className="session-title" title={session.title}>
                      {session.title || "Без названия"}
                    </span>
                    {isOpen && <span className="session-open-badge">открыта</span>}
                    <span className="session-time">{relTime(session.updatedAt)}</span>
                  </button>
                );
              })}
              {hidden > 0 && (
                <button className="session-toggle" onClick={() => onToggleSessions(p.id)}>
                  {expanded ? <ChevronUp size={13} /> : <ChevronDown size={13} />}
                  {expanded ? "Свернуть" : `Ещё ${hidden}`}
                </button>
              )}
            </div>
          );
        })}
        {orphanInstances.map((inst) => {
          const expanded = expandedProjects.has(inst.id);
          const visible = expanded ? inst.sessions : inst.sessions.slice(0, 3);
          const hidden = inst.sessions.length - visible.length;
          return (
            <div key={inst.id} className="project-card running">
              <div className="project-row">
                <span className="session-dot on" />
                <div className="project-meta">
                  <span className="project-name" title={inst.name}>{inst.name}</span>
                  <span className="project-time">:{inst.port}</span>
                </div>
                <button className="btn small new-session" onClick={() => onCreateSession(inst.port, inst.id)}>
                  <Plus size={13} /> Новая сессия
                </button>
              </div>
              {inst.sessions.length === 0 && <div className="session-row empty">Нет сессий</div>}
              {visible.map((session) => {
                const active = selectedSessionId === session.id;
                const isOpen = openSessionIds.includes(session.id);
                return (
                  <button key={session.id} className={`session-row ${active ? "active" : ""}`} onClick={() => onSelectSession(inst, session)}>
                    <span className={`session-dot ${active ? "on" : ""}`} />
                    <span className="session-title" title={session.title}>{session.title || "Без названия"}</span>
                    {isOpen && <span className="session-open-badge">открыта</span>}
                    <span className="session-time">{relTime(session.updatedAt)}</span>
                  </button>
                );
              })}
              {hidden > 0 && (
                <button className="session-toggle" onClick={() => onToggleSessions(inst.id)}>
                  {expanded ? <ChevronUp size={13} /> : <ChevronDown size={13} />}
                  {expanded ? "Свернуть" : `Ещё ${hidden}`}
                </button>
              )}
            </div>
          );
        })}
        {hiddenProjects.length > 0 && (
          <div className="hidden-projects">
            <div className="hidden-projects-header">Скрытые проекты</div>
            {hiddenProjects.map((w) => {
              const p = projects.find((pr) => pr.worktree === w);
              return (
                <div key={w} className="hidden-project-row">
                  <span className="project-name" title={w}>{p?.name ?? w}</span>
                  <button className="btn small" onClick={() => onUnhideProject(w)} title="Вернуть проект в список">
                    <RotateCcw size={13} /> Вернуть
                  </button>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </section>
  );
}
