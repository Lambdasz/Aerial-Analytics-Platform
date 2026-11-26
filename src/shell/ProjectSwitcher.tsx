import { useCallback, useEffect, useId, useMemo, useRef, useState, type ReactNode } from "react";
import { LuCheck, LuChevronDown, LuFolder, LuFolderPlus, LuSearch } from "react-icons/lu";

import { type Project, listProjects } from "../modules/module-01/projectCommands";
import { sendM1Intent } from "../modules/module-01/navIntent";
import { shortId } from "../modules/module-01/sessionStats";
import { useCurrentProject } from "./currentProject";

const IDLE_LIMIT = 5;

type PickerStatus = "loading" | "error" | "ready";

interface PickerItem {
  id: string;
  name: string;
}

interface ProjectSearchListProps<T extends PickerItem> {
  items: readonly T[];
  status: PickerStatus;
  onSelect: (item: T) => void;
  selectedId?: string | null;
  query?: string;
  onQueryChange?: (query: string) => void;
  limit?: number;
  label?: string;
  placeholder?: string;
  idleHeading?: string;
  resultsHeading?: string;
  loadingText?: string;
  errorText?: string;
  emptyText?: string;
  noResultsText?: (query: string) => string;
  onRetry?: () => void;
  renderMeta?: (item: T) => ReactNode;
  focusOnMount?: boolean;
}

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

function ProjectSearchList<T extends PickerItem>({
  items,
  status,
  onSelect,
  selectedId = null,
  query: controlledQuery,
  onQueryChange,
  limit = 5,
  label = "Cari proyek",
  placeholder = "Cari nama atau ID proyek",
  idleHeading = "Terbaru",
  resultsHeading = "Hasil",
  loadingText = "Memuat proyek…",
  errorText = "Gagal memuat proyek.",
  emptyText = "Belum ada proyek.",
  noResultsText = (q) => `Tidak ada proyek untuk “${q}”.`,
  onRetry,
  renderMeta,
  focusOnMount = false,
}: ProjectSearchListProps<T>) {
  const [innerQuery, setInnerQuery] = useState("");
  const [activeIdx, setActiveIdx] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listId = useId();

  const query = controlledQuery ?? innerQuery;
  const trimmed = query.trim();

  const setQuery = (next: string) => {
    if (controlledQuery === undefined) setInnerQuery(next);
    onQueryChange?.(next);
    setActiveIdx(0);
  };

  useEffect(() => {
    if (focusOnMount) inputRef.current?.focus();
  }, [focusOnMount]);

  const visible = useMemo(() => {
    if (!trimmed) return items.slice(0, limit);
    return items
      .map((item, order) => ({ item, order, rank: score(trimmed, item.name, item.id) }))
      .filter((r): r is { item: T; order: number; rank: number } => r.rank !== null)
      .sort((a, b) => a.rank - b.rank || a.order - b.order)
      .map((r) => r.item);
  }, [items, trimmed, limit]);

  const safeActive = Math.min(activeIdx, Math.max(0, visible.length - 1));
  const optionId = (i: number) => `${listId}-opt-${i}`;

  useEffect(() => {
    document.getElementById(`${listId}-opt-${safeActive}`)?.scrollIntoView?.({ block: "nearest" });
  }, [safeActive, listId, visible]);

  const onKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Escape" && query) {
      e.preventDefault();
      e.stopPropagation();
      setQuery("");
      return;
    }
    if (visible.length === 0) return;
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActiveIdx((safeActive + 1) % visible.length);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActiveIdx((safeActive - 1 + visible.length) % visible.length);
    } else if (e.key === "Home") {
      e.preventDefault();
      setActiveIdx(0);
    } else if (e.key === "End") {
      e.preventDefault();
      setActiveIdx(visible.length - 1);
    } else if (e.key === "Enter") {
      e.preventDefault();
      onSelect(visible[safeActive]);
    }
  };

  let body: ReactNode;
  if (status === "loading") {
    body = (
      <p className="app-shell_pl-state" role="status">
        {loadingText}
      </p>
    );
  } else if (status === "error") {
    body = (
      <div className="app-shell_pl-state" role="alert">
        <span>{errorText}</span>
        {onRetry && (
          <button type="button" className="app-shell_pl-retry" onClick={onRetry}>
            Coba lagi
          </button>
        )}
      </div>
    );
  } else if (visible.length === 0) {
    body = (
      <p className="app-shell_pl-state" role="status">
        {trimmed ? noResultsText(trimmed) : emptyText}
      </p>
    );
  } else {
    body = (
      <>
        <p className="app-shell_pl-heading" role="presentation">
          {trimmed ? resultsHeading : idleHeading}
        </p>
        <ul
          id={listId}
          role="listbox"
          aria-label={label}
          tabIndex={-1}
          className="app-shell_pl-list"
        >
          {visible.map((item, i) => {
            const active = i === safeActive;
            const selected = item.id === selectedId;
            return (
              <li
                key={item.id}
                id={optionId(i)}
                role="option"
                aria-selected={selected}
                tabIndex={-1}
                className={`app-shell_pl-item${active ? " app-shell_pl-item--active" : ""}${selected ? " app-shell_pl-item--selected" : ""}`}
                onMouseMove={() => setActiveIdx(i)}
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => onSelect(item)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    onSelect(item);
                  }
                }}
              >
                <span className="app-shell_pl-text">
                  <span className="app-shell_pl-name">{highlight(item.name, trimmed)}</span>
                  {renderMeta && <span className="app-shell_pl-meta">{renderMeta(item)}</span>}
                </span>
                {selected && (
                  <span className="app-shell_pl-check" aria-label="Proyek aktif">
                    <LuCheck size={14} aria-hidden="true" />
                  </span>
                )}
              </li>
            );
          })}
        </ul>
      </>
    );
  }

  const showList = status === "ready" && visible.length > 0;

  return (
    <div className="app-shell_pl">
      <div className="app-shell_pl-search">
        <LuSearch className="app-shell_pl-search-glyph" aria-hidden="true" />
        <input
          ref={inputRef}
          type="text"
          className="app-shell_pl-input"
          role="combobox"
          aria-label={label}
          aria-expanded={showList}
          aria-controls={showList ? listId : undefined}
          aria-autocomplete="list"
          aria-activedescendant={showList ? optionId(safeActive) : undefined}
          placeholder={placeholder}
          autoComplete="off"
          spellCheck={false}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={onKeyDown}
        />
      </div>
      {body}
    </div>
  );
}

export function ProjectSwitcher() {
  const { currentProject, setCurrentProject } = useCurrentProject();
  const [open, setOpen] = useState(false);
  const [projects, setProjects] = useState<Project[] | null>(null);
  const [failed, setFailed] = useState(false);

  const wrapperRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const loadSeq = useRef(0);
  const panelId = useId();

  const load = useCallback(() => {
    const seq = ++loadSeq.current;
    setFailed(false);
    setProjects(null);
    listProjects()
      .then((list) => {
        if (seq === loadSeq.current) {
          setProjects([...list].sort((a, b) => Date.parse(b.updatedAt) - Date.parse(a.updatedAt)));
        }
      })
      .catch(() => {
        if (seq === loadSeq.current) setFailed(true);
      });
  }, []);

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
    const onPointerDown = (e: MouseEvent) => {
      if (wrapperRef.current && !wrapperRef.current.contains(e.target as Node)) close(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close(true);
    };
    document.addEventListener("mousedown", onPointerDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onPointerDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open, close]);

  const items = useMemo(() => projects ?? [], [projects]);
  const status: PickerStatus = failed ? "error" : projects === null ? "loading" : "ready";

  const selectProject = (project: Project) => {
    setCurrentProject(project);
    close(false);
  };

  const addProject = () => {
    sendM1Intent({ kind: "create-project" });
    close(false);
  };

  return (
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
        aria-label={currentProject?.name ? `Proyek aktif: ${currentProject.name}` : "Pilih proyek"}
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
          <ProjectSearchList
            items={items}
            status={status}
            selectedId={currentProject?.id ?? null}
            limit={IDLE_LIMIT}
            onSelect={selectProject}
            onRetry={load}
            renderMeta={(project) => `ID ${shortId(project.id)}`}
            focusOnMount
          />
          <div className="app-shell_project-footer">
            <button type="button" className="app-shell_project-create" onClick={addProject}>
              <LuFolderPlus size={16} aria-hidden="true" />
              Proyek baru
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
