import { useCallback, useEffect, useId, useMemo, useRef, useState, type ReactNode } from "react";
import {
  LuArrowUpRight,
  LuCheck,
  LuChevronDown,
  LuChevronRight,
  LuFolder,
  LuFolderPlus,
  LuPlus,
  LuSearch,
} from "react-icons/lu";

import { ProjectBrowserModal } from "../modules/module-01/components/ProjectPicker";
import { type Project, listProjects } from "../modules/module-01/projectCommands";
import { type Session, getSessionsByProject } from "../modules/module-01/sessionCommands";
import { sendM1Intent } from "../modules/module-01/navIntent";
import { shortId } from "../modules/module-01/sessionStats";
import { useCurrentProject } from "./currentProject";

const IDLE_LIMIT = 5;
const MAX_SESSIONS = 5;
const MAX_SEARCH_PER_GROUP = 6;

type PickerStatus = "loading" | "error" | "ready";
type SessionState = Session[] | "loading" | "error" | undefined;

interface SessionHit {
  session: Session;
  project: Project;
}

type FlatOption = { kind: "project"; project: Project } | { kind: "session"; hit: SessionHit };

const byRecent = (a: { updatedAt: string }, b: { updatedAt: string }) =>
  Date.parse(b.updatedAt) - Date.parse(a.updatedAt);

function score(query: string, name: string, id: string): number | null {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (words.length === 0) return 0;
  const haystack = name.toLowerCase();
  let total = 0;
  for (const word of words) {
    if (haystack.startsWith(word)) total += 0;
    else if (haystack.includes(` ${word}`)) total += 1;
    else if (haystack.includes(word)) total += 2;
    else if (id.toLowerCase().startsWith(word)) total += 3;
    else return null;
  }
  return total;
}

function escapeRegExp(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function highlight(text: string, query: string): ReactNode {
  const words = query.trim().split(/\s+/).filter(Boolean);
  if (words.length === 0) return text;
  const pattern = new RegExp(`(${words.map(escapeRegExp).join("|")})`, "ig");
  return text.split(pattern).map((part, i) => (i % 2 === 1 ? <mark key={i}>{part}</mark> : part));
}

interface ProjectTreeListProps {
  items: readonly Project[];
  allSessions: readonly SessionHit[];
  status: PickerStatus;
  selectedId: string | null;
  expanded: ReadonlySet<string>;
  sessions: Record<string, SessionState>;
  onToggleExpand: (projectId: string) => void;
  onSelectProject: (project: Project) => void;
  onOpenProjectPage: (project: Project) => void;
  onOpenSession: (project: Project, session: Session) => void;
  onAddSession: (project: Project) => void;
  onRetry: () => void;
  limit?: number;
  focusOnMount?: boolean;
}

function ProjectTreeList({
  items,
  allSessions,
  status,
  selectedId,
  expanded,
  sessions,
  onToggleExpand,
  onSelectProject,
  onOpenProjectPage,
  onOpenSession,
  onAddSession,
  onRetry,
  limit = IDLE_LIMIT,
  focusOnMount = false,
}: ProjectTreeListProps) {
  const [query, setQuery] = useState("");
  const [activeIdx, setActiveIdx] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listId = useId();

  const trimmed = query.trim();

  useEffect(() => {
    if (focusOnMount) inputRef.current?.focus();
  }, [focusOnMount]);

  const { matchedProjects, matchedSessions } = useMemo(() => {
    if (!trimmed) {
      return {
        matchedProjects: items.slice(0, limit),
        matchedSessions: [] as SessionHit[],
      };
    }

    const projects = items
      .map((item, order) => ({ item, order, rank: score(trimmed, item.name, item.id) }))
      .filter((r): r is { item: Project; order: number; rank: number } => r.rank !== null)
      .sort((a, b) => a.rank - b.rank || a.order - b.order)
      .slice(0, MAX_SEARCH_PER_GROUP)
      .map((r) => r.item);

    const sessionHits = allSessions
      .map((hit) => ({ hit, rank: score(trimmed, hit.session.name, hit.session.id) }))
      .filter((r): r is { hit: SessionHit; rank: number } => r.rank !== null)
      .sort((a, b) => a.rank - b.rank || byRecent(a.hit.session, b.hit.session))
      .slice(0, MAX_SEARCH_PER_GROUP)
      .map((r) => r.hit);

    return { matchedProjects: projects, matchedSessions: sessionHits };
  }, [items, allSessions, trimmed, limit]);

  const flatOptions = useMemo<FlatOption[]>(
    () => [
      ...matchedProjects.map((project): FlatOption => ({ kind: "project", project })),
      ...matchedSessions.map((hit): FlatOption => ({ kind: "session", hit })),
    ],
    [matchedProjects, matchedSessions],
  );

  const safeActive = Math.min(activeIdx, Math.max(0, flatOptions.length - 1));
  const optionId = (i: number) => `${listId}-opt-${i}`;

  useEffect(() => {
    document.getElementById(`${listId}-opt-${safeActive}`)?.scrollIntoView?.({ block: "nearest" });
  }, [safeActive, listId, flatOptions]);

  const onKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Escape" && query) {
      e.preventDefault();
      e.stopPropagation();
      setQuery("");
      setActiveIdx(0);
      return;
    }
    if (flatOptions.length === 0) return;
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActiveIdx((safeActive + 1) % flatOptions.length);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActiveIdx((safeActive - 1 + flatOptions.length) % flatOptions.length);
    } else if (e.key === "Home") {
      e.preventDefault();
      setActiveIdx(0);
    } else if (e.key === "End") {
      e.preventDefault();
      setActiveIdx(flatOptions.length - 1);
    } else if (e.key === "Enter") {
      e.preventDefault();
      const chosen = flatOptions[safeActive];
      if (chosen.kind === "project") onSelectProject(chosen.project);
      else onOpenSession(chosen.hit.project, chosen.hit.session);
    }
  };

  const renderSessions = (project: Project) => {
    const state = sessions[project.id];
    const rows = Array.isArray(state) ? state : [];
    const shown = rows.slice(0, MAX_SESSIONS);
    const hidden = rows.length - shown.length;

    return (
      <div className="app-shell_pl-tree" role="group" aria-label={`Sesi ${project.name}`}>
        {state === "loading" && <p className="app-shell_pl-tree-note">Memuat sesi…</p>}
        {state === "error" && <p className="app-shell_pl-tree-note">Gagal memuat sesi</p>}
        {state !== "loading" && state !== "error" && shown.length === 0 && (
          <p className="app-shell_pl-tree-note">Belum ada sesi</p>
        )}
        {shown.map((session) => (
          <button
            key={session.id}
            type="button"
            className="app-shell_pl-tree-row"
            onClick={() => onOpenSession(project, session)}
          >
            <span className="app-shell_pl-tree-label">{session.name}</span>
            {session.status === "archived" && <span className="app-shell_pl-tree-tag">arsip</span>}
          </button>
        ))}
        {hidden > 0 && <p className="app-shell_pl-tree-note">… {hidden} sesi lain</p>}
        <button
          type="button"
          className="app-shell_pl-tree-row app-shell_pl-tree-row--add"
          onClick={() => onAddSession(project)}
        >
          <LuPlus size={13} aria-hidden="true" />
          <span className="app-shell_pl-tree-label">Tambah sesi</span>
        </button>
      </div>
    );
  };

  let body: ReactNode;
  if (status === "loading") {
    body = (
      <p className="app-shell_pl-state" role="status">
        Memuat proyek…
      </p>
    );
  } else if (status === "error") {
    body = (
      <div className="app-shell_pl-state" role="alert">
        <span>Gagal memuat proyek.</span>
        <button type="button" className="app-shell_pl-retry" onClick={onRetry}>
          Coba lagi
        </button>
      </div>
    );
  } else if (flatOptions.length === 0) {
    body = (
      <p className="app-shell_pl-state" role="status">
        {trimmed ? `Tidak ada hasil untuk “${trimmed}”.` : "Belum ada proyek."}
      </p>
    );
  } else {
    body = (
      <ul
        id={listId}
        role="listbox"
        aria-label="Hasil proyek dan sesi"
        tabIndex={-1}
        className="app-shell_pl-list"
      >
        {matchedProjects.length > 0 && (
          <li role="presentation" className="app-shell_pl-heading">
            {trimmed ? "Proyek" : "Terbaru"}
          </li>
        )}
        {matchedProjects.map((project, i) => {
          const active = i === safeActive;
          const selected = project.id === selectedId;
          const isExpanded = expanded.has(project.id);
          const Chevron = isExpanded ? LuChevronDown : LuChevronRight;

          return (
            <li
              key={`p-${project.id}`}
              id={optionId(i)}
              role="option"
              aria-selected={selected}
              className="app-shell_pl-node"
            >
              <div
                className={`app-shell_pl-item${active ? " app-shell_pl-item--active" : ""}${selected ? " app-shell_pl-item--selected" : ""}`}
                onMouseMove={() => setActiveIdx(i)}
              >
                <button
                  type="button"
                  className="app-shell_pl-expand"
                  aria-expanded={isExpanded}
                  aria-label={
                    isExpanded
                      ? `Tutup daftar sesi ${project.name}`
                      : `Tampilkan daftar sesi ${project.name}`
                  }
                  title={isExpanded ? "Tutup sesi" : "Lihat sesi"}
                  onMouseDown={(e) => e.preventDefault()}
                  onClick={(e) => {
                    e.stopPropagation();
                    onToggleExpand(project.id);
                  }}
                >
                  <Chevron size={14} aria-hidden="true" />
                </button>

                <button
                  type="button"
                  className="app-shell_pl-select"
                  onMouseDown={(e) => e.preventDefault()}
                  onClick={() => onSelectProject(project)}
                >
                  <span className="app-shell_pl-text">
                    <span className="app-shell_pl-name">{highlight(project.name, trimmed)}</span>
                    <span className="app-shell_pl-meta">ID {shortId(project.id)}</span>
                  </span>
                </button>

                <div className="app-shell_pl-actions">
                  {selected && (
                    <span className="app-shell_pl-check" aria-label="Proyek aktif">
                      <LuCheck size={13} aria-hidden="true" />
                    </span>
                  )}
                  <button
                    type="button"
                    className="app-shell_pl-open"
                    aria-label={`Buka halaman proyek ${project.name}`}
                    title="Buka di Image & Project Manager"
                    onMouseDown={(e) => e.preventDefault()}
                    onClick={(e) => {
                      e.stopPropagation();
                      onOpenProjectPage(project);
                    }}
                  >
                    <LuArrowUpRight size={15} aria-hidden="true" />
                  </button>
                </div>
              </div>

              {isExpanded && renderSessions(project)}
            </li>
          );
        })}

        {matchedSessions.length > 0 && (
          <li
            role="presentation"
            className={`app-shell_pl-heading${matchedProjects.length > 0 ? " app-shell_pl-heading--divided" : ""}`}
          >
            Sesi penerbangan
          </li>
        )}
        {matchedSessions.map((hit, i) => {
          const flatIdx = matchedProjects.length + i;
          const active = flatIdx === safeActive;
          return (
            <li
              key={`s-${hit.session.id}`}
              id={optionId(flatIdx)}
              role="option"
              aria-selected={active}
              className="app-shell_pl-node"
            >
              <button
                type="button"
                className={`app-shell_pl-session-hit${active ? " app-shell_pl-item--active" : ""}`}
                onMouseMove={() => setActiveIdx(flatIdx)}
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => onOpenSession(hit.project, hit.session)}
              >
                <span className="app-shell_pl-text">
                  <span className="app-shell_pl-name">{highlight(hit.session.name, trimmed)}</span>
                  <span className="app-shell_pl-meta">
                    di {hit.project.name}
                    {hit.session.status === "archived" ? " · diarsipkan" : ""}
                  </span>
                </span>
              </button>
            </li>
          );
        })}
      </ul>
    );
  }

  const showList = status === "ready" && flatOptions.length > 0;

  return (
    <div className="app-shell_pl">
      <div className="app-shell_pl-search">
        <LuSearch className="app-shell_pl-search-glyph" aria-hidden="true" />
        <input
          ref={inputRef}
          type="text"
          className="app-shell_pl-input"
          role="combobox"
          aria-label="Cari proyek atau sesi penerbangan"
          aria-expanded={showList}
          aria-controls={showList ? listId : undefined}
          aria-autocomplete="list"
          aria-activedescendant={showList ? optionId(safeActive) : undefined}
          placeholder="Cari proyek atau sesi…"
          autoComplete="off"
          spellCheck={false}
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setActiveIdx(0);
          }}
          onKeyDown={onKeyDown}
        />
      </div>
      {body}
    </div>
  );
}

export function ProjectSwitcher() {
  const { currentProject, setCurrentProject, clearCurrentProject } = useCurrentProject();
  const [open, setOpen] = useState(false);
  const [browserOpen, setBrowserOpen] = useState(false);
  const [projects, setProjects] = useState<Project[] | null>(null);
  const [failed, setFailed] = useState(false);
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(new Set());
  const [sessions, setSessions] = useState<Record<string, SessionState>>({});

  const wrapperRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const loadSeq = useRef(0);
  const panelId = useId();

  const load = useCallback(() => {
    const seq = ++loadSeq.current;
    setFailed(false);
    setProjects(null);
    setExpanded(new Set());
    setSessions({});
    listProjects()
      .then((list) => {
        if (seq !== loadSeq.current) return;
        const sorted = [...list].sort(byRecent);
        setProjects(sorted);
        // Preload sessions per project in the background so searching sessions is instant.
        void Promise.all(
          sorted.map((project) =>
            getSessionsByProject(project.id)
              .then((rows) => {
                if (seq === loadSeq.current) {
                  setSessions((prev) => ({ ...prev, [project.id]: [...rows].sort(byRecent) }));
                }
              })
              .catch(() => {
                if (seq === loadSeq.current) {
                  setSessions((prev) => ({ ...prev, [project.id]: "error" }));
                }
              }),
          ),
        );
      })
      .catch(() => {
        if (seq === loadSeq.current) setFailed(true);
      });
  }, []);

  const loadSessions = useCallback((projectId: string, seq: number) => {
    setSessions((prev) =>
      prev[projectId] === undefined ? { ...prev, [projectId]: "loading" } : prev,
    );
    getSessionsByProject(projectId)
      .then((rows) => {
        if (seq === loadSeq.current) {
          setSessions((prev) => ({ ...prev, [projectId]: [...rows].sort(byRecent) }));
        }
      })
      .catch(() => {
        if (seq === loadSeq.current) {
          setSessions((prev) => ({ ...prev, [projectId]: "error" }));
        }
      });
  }, []);

  const toggleExpand = useCallback(
    (projectId: string) => {
      let willExpand = false;
      setExpanded((prev) => {
        const next = new Set(prev);
        if (next.has(projectId)) {
          next.delete(projectId);
        } else {
          next.add(projectId);
          willExpand = true;
        }
        return next;
      });
      if (willExpand && sessions[projectId] === undefined) {
        loadSessions(projectId, loadSeq.current);
      }
    },
    [loadSessions, sessions],
  );

  const close = useCallback((returnFocus: boolean) => {
    setOpen(false);
    if (returnFocus) triggerRef.current?.focus();
  }, []);

  const toggle = () => {
    if (open) {
      close(false);
      return;
    }
    setOpen(true);
    load();
  };

  useEffect(() => {
    if (!open) return;
    const wrapper = wrapperRef.current;
    const onWrapperMouseDown = (e: MouseEvent) => {
      if (!(e.target instanceof HTMLInputElement)) e.preventDefault();
    };
    const onPointerDown = (e: MouseEvent) => {
      if (wrapper && !wrapper.contains(e.target as Node)) close(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close(true);
    };
    wrapper?.addEventListener("mousedown", onWrapperMouseDown);
    document.addEventListener("mousedown", onPointerDown);
    document.addEventListener("keydown", onKey);
    return () => {
      wrapper?.removeEventListener("mousedown", onWrapperMouseDown);
      document.removeEventListener("mousedown", onPointerDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open, close]);

  const items = useMemo(() => projects ?? [], [projects]);

  const allSessions = useMemo<SessionHit[]>(() => {
    const hits: SessionHit[] = [];
    for (const project of items) {
      const state = sessions[project.id];
      if (Array.isArray(state)) {
        for (const session of state) {
          hits.push({ session, project });
        }
      }
    }
    return hits;
  }, [items, sessions]);

  const status: PickerStatus = failed ? "error" : projects === null ? "loading" : "ready";

  const selectProject = (project: Project) => {
    setCurrentProject(project);
    close(false);
  };

  const openProjectPage = (project: Project) => {
    setCurrentProject(project);
    sendM1Intent({ kind: "select-project", project });
    close(false);
  };

  const openSession = (project: Project, session: Session) => {
    setCurrentProject(project);
    sendM1Intent({ kind: "open-session", project, sessionId: session.id });
    close(false);
  };

  const addSession = (project: Project) => {
    setCurrentProject(project);
    sendM1Intent({ kind: "create-session", project });
    close(false);
  };

  const addProject = () => {
    sendM1Intent({ kind: "create-project" });
    close(false);
  };

  return (
    <>
      <div
        className="app-shell_project"
        ref={wrapperRef}
        onBlur={(e) => {
          if (!e.currentTarget.contains(e.relatedTarget)) close(false);
        }}
      >
        <button
          ref={triggerRef}
          type="button"
          className="app-shell_project-trigger"
          aria-haspopup="dialog"
          aria-expanded={open}
          aria-controls={open ? panelId : undefined}
          aria-label={
            currentProject?.name ? `Proyek aktif: ${currentProject.name}` : "Pilih proyek"
          }
          title={currentProject?.name || "Pilih proyek"}
          onClick={toggle}
        >
          <LuFolder size={16} aria-hidden="true" />
          <span className="app-shell_project-name">{currentProject?.name || "Pilih proyek"}</span>
          <LuChevronDown size={12} aria-hidden="true" />
        </button>

        {open && (
          <div
            id={panelId}
            className="app-shell_project-menu"
            role="dialog"
            aria-label="Pilih proyek"
          >
            <ProjectTreeList
              items={items}
              allSessions={allSessions}
              status={status}
              selectedId={currentProject?.id ?? null}
              expanded={expanded}
              sessions={sessions}
              onToggleExpand={toggleExpand}
              onSelectProject={selectProject}
              onOpenProjectPage={openProjectPage}
              onOpenSession={openSession}
              onAddSession={addSession}
              onRetry={load}
              limit={IDLE_LIMIT}
              focusOnMount
            />
            <div className="app-shell_project-footer">
              <button
                type="button"
                className="app-shell_project-more"
                onClick={() => {
                  close(false);
                  setBrowserOpen(true);
                }}
              >
                Lihat semua proyek
              </button>
              <button type="button" className="app-shell_project-create" onClick={addProject}>
                <LuFolderPlus size={15} aria-hidden="true" />
                Proyek baru
              </button>
            </div>
          </div>
        )}
      </div>

      <ProjectBrowserModal
        isOpen={browserOpen}
        onClose={() => {
          setBrowserOpen(false);
          triggerRef.current?.focus();
        }}
        selectedProject={currentProject}
        onSelect={(project) => setCurrentProject(project)}
        onProjectDeleted={(deletedId) => {
          if (currentProject?.id === deletedId) clearCurrentProject();
        }}
        onProjectsChanged={() => {
          if (open) load();
        }}
      />
    </>
  );
}
