import { useCallback, useEffect, useId, useRef, useState } from "react";
import { LuChevronDown, LuPlus } from "react-icons/lu";

import { type Project, listProjects } from "../modules/module-01/projectCommands";
import { sendM1Intent } from "../modules/module-01/navIntent";

const MAX_PROJECTS = 5;
const ACTIVE_PROJECT_KEY = "m1_last_project_id";

/** The project Module 01 has open right now, if any (it stores the whole project object). */
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

/**
 * GitHub-style "+" menu. The tree reads like a directory listing:
 *
 *   Sesi baru di…
 *   ├─ Blok A
 *   ├─ Blok B
 *   └─ + Proyek baru
 */
export function NewMenu() {
  const [open, setOpen] = useState(false);
  const [projects, setProjects] = useState<Project[] | null>(null);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  const wrapperRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const loadSeq = useRef(0);
  const menuId = useId();

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
    setActiveId(readActiveProjectId());
    setFailed(false);
    const seq = ++loadSeq.current;
    listProjects()
      .then((list) => {
        if (seq === loadSeq.current) setProjects(list);
      })
      .catch(() => {
        if (seq === loadSeq.current) setFailed(true);
      });
  };

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (e: MouseEvent) => {
      if (wrapperRef.current && !wrapperRef.current.contains(e.target as Node)) close(false);
    };
    document.addEventListener("mousedown", onPointerDown);
    return () => document.removeEventListener("mousedown", onPointerDown);
  }, [open, close]);

  // Move focus into the menu once it has items (and again when the project list arrives).
  useEffect(() => {
    if (!open) return;
    const first = menuRef.current?.querySelector<HTMLElement>('[role="menuitem"]');
    first?.focus();
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
    const at = entries.indexOf(document.activeElement as HTMLElement);
    let next: number | null = null;
    if (e.key === "ArrowDown") next = (at + 1) % entries.length;
    else if (e.key === "ArrowUp") next = (at - 1 + entries.length) % entries.length;
    else if (e.key === "Home") next = 0;
    else if (e.key === "End") next = entries.length - 1;
    if (next !== null) {
      e.preventDefault();
      entries[next].focus();
    }
  };

  const createSessionIn = (project: Project) => {
    sendM1Intent({ kind: "create-session", project });
    close(false);
  };

  const createProject = () => {
    sendM1Intent({ kind: "create-project" });
    close(false);
  };

  // The project Module 01 has open goes first, then the most recently touched.
  const shown = (projects ?? [])
    .slice()
    .sort((a, b) => {
      if (a.id === activeId) return -1;
      if (b.id === activeId) return 1;
      return Date.parse(b.updatedAt) - Date.parse(a.updatedAt);
    })
    .slice(0, MAX_PROJECTS);
  const hiddenCount = (projects?.length ?? 0) - shown.length;

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
          <p className="app-shell_tree-root" role="presentation">
            Sesi baru di proyek…
          </p>

          {projects === null && !failed && (
            <p className="app-shell_tree-note" role="presentation">
              <span className="app-shell_tree-glyph" aria-hidden="true" />
              <span className="app-shell_tree-label">memuat…</span>
            </p>
          )}
          {failed && (
            <p className="app-shell_tree-note" role="presentation">
              <span className="app-shell_tree-glyph" aria-hidden="true" />
              <span className="app-shell_tree-label">gagal memuat proyek</span>
            </p>
          )}

          {shown.map((project) => (
            <button
              key={project.id}
              type="button"
              role="menuitem"
              className="app-shell_tree-row"
              onClick={() => createSessionIn(project)}
            >
              <span className="app-shell_tree-glyph" aria-hidden="true" />
              <span className="app-shell_tree-label">{project.name}</span>
              {project.id === activeId && <span className="app-shell_tree-badge">aktif</span>}
            </button>
          ))}

          {hiddenCount > 0 && (
            <p className="app-shell_tree-note" role="presentation">
              <span className="app-shell_tree-glyph" aria-hidden="true" />
              <span className="app-shell_tree-label">
                … {hiddenCount} proyek lain, pakai pencarian
              </span>
            </p>
          )}

          <button
            type="button"
            role="menuitem"
            className="app-shell_tree-row app-shell_tree-row--add"
            onClick={createProject}
          >
            <span className="app-shell_tree-glyph app-shell_tree-glyph--last" aria-hidden="true" />
            <span className="app-shell_tree-label">+ Proyek baru</span>
          </button>
        </div>
      )}
    </div>
  );
}
