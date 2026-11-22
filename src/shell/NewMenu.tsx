import { useCallback, useEffect, useId, useRef, useState } from "react";
import {
  LuChevronDown,
  LuChevronRight,
  LuFolder,
  LuFolderOpen,
  LuFolderPlus,
  LuPlus,
} from "react-icons/lu";

import { type Project, listProjects } from "../modules/module-01/projectCommands";
import { type Session, getSessionsByProject } from "../modules/module-01/sessionCommands";
import { sendM1Intent } from "../modules/module-01/navIntent";

const DEFAULT_EXPANDED = 2;
const MAX_SESSIONS = 5;
const ACTIVE_PROJECT_KEY = "m1_last_project_id";

type SessionState = Session[] | "loading" | "error" | undefined;

function readActiveProjectId(): string | null {
  try {
    const raw = window.localStorage.getItem(ACTIVE_PROJECT_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as { id?: unknown };
    return typeof parsed.id === "string" ? parsed.id : null;
  } catch {
    return null;
  }
}

const byRecent = (a: { updatedAt: string }, b: { updatedAt: string }) =>
  Date.parse(b.updatedAt) - Date.parse(a.updatedAt);

export function NewMenu() {
  const [open, setOpen] = useState(false);
  const [projects, setProjects] = useState<Project[] | null>(null);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(new Set());
  const [sessions, setSessions] = useState<Record<string, SessionState>>({});

  const wrapperRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const openSeq = useRef(0);
  const menuId = useId();

  const close = useCallback((returnFocus: boolean) => {
    setOpen(false);
    if (returnFocus) triggerRef.current?.focus();
  }, []);

  const loadSessions = useCallback((projectId: string, seq: number) => {
    setSessions((prev) =>
      prev[projectId] === undefined ? { ...prev, [projectId]: "loading" } : prev,
    );
    getSessionsByProject(projectId)
      .then((rows) => {
        if (seq === openSeq.current) {
          setSessions((prev) => ({ ...prev, [projectId]: [...rows].sort(byRecent) }));
        }
      })
      .catch(() => {
        if (seq === openSeq.current) setSessions((prev) => ({ ...prev, [projectId]: "error" }));
      });
  }, []);

  const toggle = () => {
    if (open) {
      close(false);
      return;
    }
    const seq = ++openSeq.current;
    const active = readActiveProjectId();
    setOpen(true);
    setActiveId(active);
    setFailed(false);
    setProjects(null);
    setSessions({});
    setExpanded(new Set());
    listProjects()
      .then((list) => {
        if (seq !== openSeq.current) return;
        const ordered = [...list].sort((a, b) => {
          if (a.id === active) return -1;
          if (b.id === active) return 1;
          return byRecent(a, b);
        });
        setProjects(ordered);
        const initial = ordered.slice(0, DEFAULT_EXPANDED).map((p) => p.id);
        setExpanded(new Set(initial));
        initial.forEach((id) => loadSessions(id, seq));
      })
      .catch(() => {
        if (seq === openSeq.current) setFailed(true);
      });
  };

  const setFolder = (projectId: string, shouldOpen: boolean) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (shouldOpen) next.add(projectId);
      else next.delete(projectId);
      return next;
    });
    if (shouldOpen) loadSessions(projectId, openSeq.current);
  };

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (e: MouseEvent) => {
      if (wrapperRef.current && !wrapperRef.current.contains(e.target as Node)) close(false);
    };
    document.addEventListener("mousedown", onPointerDown);
    return () => document.removeEventListener("mousedown", onPointerDown);
  }, [open, close]);

  useEffect(() => {
    if (!open) return;
    const target =
      menuRef.current?.querySelector<HTMLElement>("[data-folder]") ??
      menuRef.current?.querySelector<HTMLElement>('[role="menuitem"]');
    target?.focus();
  }, [open, projects]);

  const onMenuKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      e.preventDefault();
      close(true);
      return;
    }
    const entries = Array.from(
      menuRef.current?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? [],
    );
    if (entries.length === 0) return;
    const current = document.activeElement as HTMLElement | null;
    const at = current ? entries.indexOf(current) : -1;

    let next: number | null = null;
    if (e.key === "ArrowDown") next = (at + 1) % entries.length;
    else if (e.key === "ArrowUp") next = (at - 1 + entries.length) % entries.length;
    else if (e.key === "Home") next = 0;
    else if (e.key === "End") next = entries.length - 1;
    else if (e.key === "ArrowRight" && current?.dataset.folder) {
      e.preventDefault();
      const id = current.dataset.folder;
      if (!expanded.has(id)) setFolder(id, true);
      else if (at + 1 < entries.length) entries[at + 1].focus();
      return;
    } else if (e.key === "ArrowLeft" && current) {
      const folderId = current.dataset.folder;
      const parentId = current.dataset.parent;
      if (folderId && expanded.has(folderId)) {
        e.preventDefault();
        setFolder(folderId, false);
      } else if (parentId) {
        e.preventDefault();
        menuRef.current?.querySelector<HTMLElement>(`[data-folder="${parentId}"]`)?.focus();
      }
      return;
    }
    if (next !== null) {
      e.preventDefault();
      entries[next].focus();
    }
  };

  const addProject = () => {
    sendM1Intent({ kind: "create-project" });
    close(false);
  };

  const addSession = (project: Project) => {
    sendM1Intent({ kind: "create-session", project });
    close(false);
  };

  const activateProject = (project: Project) => {
    sendM1Intent({ kind: "select-project", project });
    setActiveId(project.id);
    menuRef.current?.querySelector<HTMLElement>(`[data-folder="${project.id}"]`)?.focus();
  };

  const openSession = (project: Project, session: Session) => {
    sendM1Intent({ kind: "open-session", project, sessionId: session.id });
    close(false);
  };

  const renderChildren = (project: Project) => {
    const state = sessions[project.id];
    const rows = Array.isArray(state) ? state : [];
    const shown = rows.slice(0, MAX_SESSIONS);
    const hidden = rows.length - shown.length;
    return (
      <div className="app-shell_ft-children" role="group" aria-label={`Sesi ${project.name}`}>
        {state === "loading" && (
          <p className="app-shell_ft-note" role="presentation">
            Memuat sesi…
          </p>
        )}
        {state === "error" && (
          <p className="app-shell_ft-note" role="presentation">
            Gagal memuat sesi
          </p>
        )}
        {shown.map((session) => (
          <button
            key={session.id}
            type="button"
            role="menuitem"
            data-parent={project.id}
            className="app-shell_ft-row app-shell_ft-row--file"
            onClick={() => openSession(project, session)}
          >
            <span className="app-shell_ft-label">{session.name}</span>
            {session.status === "archived" && <span className="app-shell_ft-tag">arsip</span>}
          </button>
        ))}
        {hidden > 0 && (
          <p className="app-shell_ft-note" role="presentation">
            … {hidden} sesi lain, pakai pencarian
          </p>
        )}
        <button
          type="button"
          role="menuitem"
          data-parent={project.id}
          className="app-shell_ft-row app-shell_ft-row--add"
          onClick={() => addSession(project)}
        >
          <span className="app-shell_ft-label">Tambah sesi</span>
          <LuPlus className="app-shell_ft-plus" aria-hidden="true" />
        </button>
      </div>
    );
  };

  return (
    <div className="app-shell_new" ref={wrapperRef}>
      <button
        ref={triggerRef}
        type="button"
        className="app-shell_new-trigger"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? menuId : undefined}
        aria-label="Buat baru"
        title="Buat baru"
        onClick={toggle}
      >
        <LuPlus size={16} aria-hidden="true" />
        <LuChevronDown size={12} aria-hidden="true" />
      </button>

      {open && (
        <div
          id={menuId}
          ref={menuRef}
          className="app-shell_new-menu"
          role="menu"
          aria-label="Buat baru"
          tabIndex={-1}
          onKeyDown={onMenuKeyDown}
        >
          <div className="app-shell_ft-head">
            <p className="app-shell_ft-title" role="presentation">
              Sesi baru di proyek
            </p>
            <button
              type="button"
              role="menuitem"
              className="app-shell_ft-addproject"
              aria-label="Proyek baru"
              title="Proyek baru"
              onClick={addProject}
            >
              <LuFolderPlus size={16} aria-hidden="true" />
            </button>
          </div>

          <div className="app-shell_ft-scroll">
            {projects === null && !failed && (
              <p className="app-shell_ft-note app-shell_ft-note--top" role="presentation">
                Memuat proyek…
              </p>
            )}
            {failed && (
              <p className="app-shell_ft-note app-shell_ft-note--top" role="presentation">
                Gagal memuat proyek
              </p>
            )}
            {projects?.length === 0 && (
              <p className="app-shell_ft-note app-shell_ft-note--top" role="presentation">
                Belum ada proyek. Tekan tombol di kanan atas untuk membuat satu.
              </p>
            )}

            {projects?.map((project) => {
              const isOpen = expanded.has(project.id);
              const isActive = project.id === activeId;
              const Chevron = isOpen ? LuChevronDown : LuChevronRight;
              const Folder = isOpen ? LuFolderOpen : LuFolder;
              return (
                <div key={project.id} className="app-shell_ft-node">
                  <div className="app-shell_ft-folder">
                    <button
                      type="button"
                      role="menuitem"
                      data-folder={project.id}
                      aria-expanded={isOpen}
                      className="app-shell_ft-row app-shell_ft-row--folder"
                      onClick={() => setFolder(project.id, !isOpen)}
                    >
                      <Chevron className="app-shell_ft-chevron" aria-hidden="true" />
                      <Folder
                        className={`app-shell_ft-icon${isActive ? " app-shell_ft-icon--active" : ""}`}
                        aria-hidden="true"
                      />
                      <span className="app-shell_ft-label">{project.name}</span>
                    </button>
                    {isActive ? (
                      <span className="app-shell_ft-tag app-shell_ft-tag--blue">aktif</span>
                    ) : (
                      <button
                        type="button"
                        role="menuitem"
                        data-parent={project.id}
                        className="app-shell_ft-activate"
                        aria-label={`Aktifkan proyek ${project.name}`}
                        onClick={() => activateProject(project)}
                      >
                        aktifkan
                      </button>
                    )}
                  </div>
                  {isOpen && renderChildren(project)}
                </div>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}
