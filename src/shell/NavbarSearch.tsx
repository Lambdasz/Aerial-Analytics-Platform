import { useCallback, useEffect, useId, useMemo, useRef, useState, type ReactNode } from "react";
import { LuFolder, LuSearch } from "react-icons/lu";

import { type Project, listProjects } from "../modules/module-01/projectCommands";
import { type Session, getSessionsByProject } from "../modules/module-01/sessionCommands";
import { sendM1Intent } from "../modules/module-01/navIntent";
import { shortId } from "../modules/module-01/sessionStats";

const MAX_PER_GROUP = 6;

interface SessionHit {
  session: Session;
  project: Project;
}

type ResultItem =
  | { kind: "project"; key: string; project: Project }
  | { kind: "session"; key: string; hit: SessionHit };

interface SearchIndex {
  projects: Project[];
  sessions: SessionHit[];
}

/** Projects plus every project's sessions. Local database, so the fan-out is cheap. */
async function loadIndex(): Promise<SearchIndex> {
  const projects = await listProjects();
  const perProject = await Promise.all(
    projects.map((project) =>
      getSessionsByProject(project.id)
        .then((sessions) => sessions.map((session) => ({ session, project })))
        // One broken project must not blank the whole search.
        .catch((): SessionHit[] => []),
    ),
  );
  return { projects, sessions: perProject.flat() };
}

const byRecent = (a: { updatedAt: string }, b: { updatedAt: string }) =>
  Date.parse(b.updatedAt) - Date.parse(a.updatedAt);

/** 0 = best. null = no match. Every word of the query must appear. */
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

function isTypingTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName);
}

export function NavbarSearch() {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [index, setIndex] = useState<SearchIndex | null>(null);
  const [loading, setLoading] = useState(false);
  const [failed, setFailed] = useState(false);
  const [activeIdx, setActiveIdx] = useState(0);

  const wrapperRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const loadSeq = useRef(0);
  const listId = useId();

  // Refresh the index every time the box opens, so sessions made since last time show up.
  const refresh = useCallback(() => {
    const seq = ++loadSeq.current;
    setLoading(true);
    setFailed(false);
    loadIndex()
      .then((next) => {
        if (seq === loadSeq.current) setIndex(next);
      })
      .catch(() => {
        if (seq === loadSeq.current) setFailed(true);
      })
      .finally(() => {
        if (seq === loadSeq.current) setLoading(false);
      });
  }, []);

  const close = useCallback(() => {
    setOpen(false);
    setQuery("");
    setActiveIdx(0);
  }, []);

  const openBox = useCallback(() => {
    if (open) return;
    setOpen(true);
    refresh();
  }, [open, refresh]);

  // "/" and Ctrl/Cmd+K focus the search, like GitHub.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const slash = e.key === "/" && !e.ctrlKey && !e.metaKey && !e.altKey;
      const palette = e.key.toLowerCase() === "k" && (e.ctrlKey || e.metaKey);
      if (!slash && !palette) return;
      if (slash && isTypingTarget(e.target)) return;
      e.preventDefault();
      inputRef.current?.focus();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (e: MouseEvent) => {
      if (wrapperRef.current && !wrapperRef.current.contains(e.target as Node)) close();
    };
    document.addEventListener("mousedown", onPointerDown);
    return () => document.removeEventListener("mousedown", onPointerDown);
  }, [open, close]);

  const { projectItems, sessionItems } = useMemo(() => {
    if (!index) return { projectItems: [], sessionItems: [] };
    const q = query.trim();

    const projects = index.projects
      .map((project) => ({ project, rank: score(q, project.name, project.id) }))
      .filter((r): r is { project: Project; rank: number } => r.rank !== null)
      .sort((a, b) => a.rank - b.rank || byRecent(a.project, b.project))
      .slice(0, MAX_PER_GROUP)
      .map((r): ResultItem => ({ kind: "project", key: `p-${r.project.id}`, project: r.project }));

    const sessions = index.sessions
      .map((hit) => ({ hit, rank: score(q, hit.session.name, hit.session.id) }))
      .filter((r): r is { hit: SessionHit; rank: number } => r.rank !== null)
      .sort((a, b) => a.rank - b.rank || byRecent(a.hit.session, b.hit.session))
      .slice(0, MAX_PER_GROUP)
      .map((r): ResultItem => ({ kind: "session", key: `s-${r.hit.session.id}`, hit: r.hit }));

    return { projectItems: projects, sessionItems: sessions };
  }, [index, query]);

  const items = useMemo(() => [...projectItems, ...sessionItems], [projectItems, sessionItems]);
  const safeActive = Math.min(activeIdx, Math.max(0, items.length - 1));

  const choose = (item: ResultItem) => {
    if (item.kind === "project") {
      sendM1Intent({ kind: "select-project", project: item.project });
    } else {
      sendM1Intent({
        kind: "open-session",
        project: item.hit.project,
        sessionId: item.hit.session.id,
      });
    }
    close();
    inputRef.current?.blur();
  };

  const onInputKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Escape") {
      e.preventDefault();
      if (query) setQuery("");
      else {
        close();
        inputRef.current?.blur();
      }
      return;
    }
    if (!open) return;
    if (e.key === "ArrowDown" && items.length) {
      e.preventDefault();
      setActiveIdx((safeActive + 1) % items.length);
    } else if (e.key === "ArrowUp" && items.length) {
      e.preventDefault();
      setActiveIdx((safeActive - 1 + items.length) % items.length);
    } else if (e.key === "Enter" && items[safeActive]) {
      e.preventDefault();
      choose(items[safeActive]);
    }
  };

  const renderItem = (item: ResultItem, flatIndex: number) => {
    const active = flatIndex === safeActive;
    const common = {
      id: `${listId}-opt-${flatIndex}`,
      role: "option" as const,
      "aria-selected": active,
      // Not in the tab order: the input keeps focus and points here via aria-activedescendant.
      tabIndex: -1,
      className: `app-shell_search-item${active ? " app-shell_search-item--active" : ""}`,
      onMouseMove: () => setActiveIdx(flatIndex),
      // mousedown keeps the input focused; the click then fires the choice.
      onMouseDown: (e: React.MouseEvent) => e.preventDefault(),
      onClick: () => choose(item),
    };

    if (item.kind === "project") {
      return (
        <li key={item.key} {...common}>
          <LuFolder className="app-shell_search-icon" aria-hidden="true" />
          <span className="app-shell_search-text">
            <span className="app-shell_search-name">{highlight(item.project.name, query)}</span>
            <span className="app-shell_search-meta">ID {shortId(item.project.id)}</span>
          </span>
        </li>
      );
    }

    const { session, project } = item.hit;
    return (
      <li key={item.key} {...common}>
        <span className="app-shell_search-icon" aria-hidden="true" />
        <span className="app-shell_search-text">
          <span className="app-shell_search-name">{highlight(session.name, query)}</span>
          <span className="app-shell_search-meta">
            di {project.name}
            {session.status === "archived" ? " · diarsipkan" : ""}
          </span>
        </span>
      </li>
    );
  };

  const hasQuery = query.trim().length > 0;
  let body: ReactNode;
  if (!index && loading) {
    body = <p className="app-shell_search-state">Memuat…</p>;
  } else if (failed && !index) {
    body = <p className="app-shell_search-state">Gagal memuat data pencarian.</p>;
  } else if (items.length === 0) {
    body = (
      <p className="app-shell_search-state">
        {hasQuery ? `Tidak ada hasil untuk “${query.trim()}”.` : "Belum ada proyek."}
      </p>
    );
  } else {
    body = (
      <ul id={listId} role="listbox" aria-label="Hasil pencarian" className="app-shell_search-list">
        {projectItems.length > 0 && (
          <li role="presentation" className="app-shell_search-group">
            Proyek
          </li>
        )}
        {projectItems.map((item, i) => renderItem(item, i))}
        {sessionItems.length > 0 && (
          <li role="presentation" className="app-shell_search-group">
            Sesi penerbangan
          </li>
        )}
        {sessionItems.map((item, i) => renderItem(item, projectItems.length + i))}
      </ul>
    );
  }

  return (
    <div
      className={`app-shell_search${open ? " app-shell_search--open" : ""}`}
      ref={wrapperRef}
      role="search"
    >
      <LuSearch className="app-shell_search-glyph" aria-hidden="true" />
      <input
        ref={inputRef}
        type="text"
        className="app-shell_search-input"
        placeholder="Cari proyek atau sesi…"
        aria-label="Cari proyek atau sesi penerbangan"
        role="combobox"
        aria-expanded={open}
        aria-controls={listId}
        aria-autocomplete="list"
        aria-activedescendant={open && items.length ? `${listId}-opt-${safeActive}` : undefined}
        autoComplete="off"
        spellCheck={false}
        value={query}
        onFocus={openBox}
        onChange={(e) => {
          setQuery(e.target.value);
          setActiveIdx(0);
        }}
        onKeyDown={onInputKeyDown}
      />
      {!open && <kbd className="app-shell_kbd">/</kbd>}

      {open && (
        <div className="app-shell_search-panel">
          {body}
          <div className="app-shell_search-footer" aria-hidden="true">
            <span>
              <kbd className="app-shell_kbd">↑</kbd>
              <kbd className="app-shell_kbd">↓</kbd> pilih
            </span>
            <span>
              <kbd className="app-shell_kbd">↵</kbd> buka
            </span>
            <span>
              <kbd className="app-shell_kbd">esc</kbd> tutup
            </span>
          </div>
        </div>
      )}
    </div>
  );
}
