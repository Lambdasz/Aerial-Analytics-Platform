import { useCallback, useEffect, useMemo, useRef, useState, type FC, type FormEvent } from "react";
import { Alert, Button, Dialog, FormGroup, InputGroup, Intent } from "@blueprintjs/core";
import { getSessionsByProject, commandErrorMessage } from "../sessionCommands";
import { type Project, listProjects, createProject, deleteProject } from "../projectCommands";
import { shortId } from "../sessionStats";
import "./ProjectPicker.css";

export interface ProjectPickerProps {
  selectedProject: Project | null;
  onSelect: (project: Project) => void;
  onProjectDeleted: (projectId: string) => void;
  onError: (message: string) => void;
  onToast: (message: string) => void;
}

type PickerTab = "recent" | "starred" | "all";
type SortKey = "name" | "createdAt";

const STARRED_KEY = "m1_starred_projects";
const RECENT_COUNT = 3;

const dateFormatter = new Intl.DateTimeFormat("id-ID", {
  day: "numeric",
  month: "short",
  year: "numeric",
});

function formatDate(iso: string): string {
  const date = new Date(iso);
  return Number.isNaN(date.getTime()) ? "-" : dateFormatter.format(date);
}

function readStarred(): string[] {
  try {
    const raw = window.localStorage.getItem(STARRED_KEY);
    if (!raw) return [];
    const parsed: unknown = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed.filter((v): v is string => typeof v === "string") : [];
  } catch {
    return [];
  }
}

function writeStarred(ids: string[]): void {
  try {
    window.localStorage.setItem(STARRED_KEY, JSON.stringify(ids));
  } catch {
    // Storage unavailable (private mode, blocked): stars simply do not persist.
  }
}

const IconFolder: FC = () => (
  <svg
    width="16"
    height="16"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2"
    strokeLinecap="round"
    strokeLinejoin="round"
    aria-hidden="true"
  >
    <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
  </svg>
);

const IconChevron: FC = () => (
  <svg
    width="14"
    height="14"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2.2"
    strokeLinecap="round"
    strokeLinejoin="round"
    aria-hidden="true"
  >
    <polyline points="6 9 12 15 18 9" />
  </svg>
);

const IconPlus: FC = () => (
  <svg
    width="14"
    height="14"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2.2"
    strokeLinecap="round"
    strokeLinejoin="round"
    aria-hidden="true"
  >
    <line x1="12" y1="5" x2="12" y2="19" />
    <line x1="5" y1="12" x2="19" y2="12" />
  </svg>
);

const IconCheck: FC = () => (
  <svg
    width="14"
    height="14"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2.4"
    strokeLinecap="round"
    strokeLinejoin="round"
    aria-hidden="true"
  >
    <polyline points="20 6 9 17 4 12" />
  </svg>
);

const IconStar: FC<{ filled: boolean }> = ({ filled }) => (
  <svg
    width="16"
    height="16"
    viewBox="0 0 24 24"
    fill={filled ? "currentColor" : "none"}
    stroke="currentColor"
    strokeWidth="2"
    strokeLinejoin="round"
    aria-hidden="true"
  >
    <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
  </svg>
);

const IconTrash: FC = () => (
  <svg
    width="14"
    height="14"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2"
    strokeLinecap="round"
    strokeLinejoin="round"
    aria-hidden="true"
  >
    <polyline points="3 6 5 6 21 6" />
    <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
  </svg>
);

export const ProjectPicker: FC<ProjectPickerProps> = ({
  selectedProject,
  onSelect,
  onProjectDeleted,
  onError,
  onToast,
}) => {
  const [projects, setProjects] = useState<Project[]>([]);
  const [loading, setLoading] = useState(true);
  const [starred, setStarred] = useState<string[]>(() => readStarred());

  const [popoverOpen, setPopoverOpen] = useState(false);
  const [sessionCounts, setSessionCounts] = useState<Record<string, number>>({});

  const [modalOpen, setModalOpen] = useState(false);
  const [tab, setTab] = useState<PickerTab>("recent");
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<{ key: SortKey; dir: "asc" | "desc" }>({
    key: "createdAt",
    dir: "desc",
  });
  const [deleteTarget, setDeleteTarget] = useState<Project | null>(null);
  const [deleteBusy, setDeleteBusy] = useState(false);

  const [createOpen, setCreateOpen] = useState(false);
  const [newName, setNewName] = useState("");
  const [newDesc, setNewDesc] = useState("");
  const [createBusy, setCreateBusy] = useState(false);

  const wrapperRef = useRef<HTMLDivElement>(null);
  const pillRef = useRef<HTMLButtonElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const nameInputRef = useRef<HTMLInputElement>(null);
  const attemptedCountsRef = useRef<Set<string>>(new Set());

  const fetchProjects = useCallback(async () => {
    try {
      setLoading(true);
      const result = await listProjects();
      setProjects(result);
    } catch (err) {
      onError(commandErrorMessage(err));
    } finally {
      setLoading(false);
    }
  }, [onError]);

  useEffect(() => {
    // Defer to avoid synchronous setState in effect body (same pattern as ProjectBar).
    queueMicrotask(() => {
      void fetchProjects();
    });
  }, [fetchProjects]);

  // Auto-select the first project when nothing is selected (ported from ProjectBar).
  useEffect(() => {
    if (!selectedProject && projects.length > 0) {
      onSelect(projects[0]);
    }
  }, [projects, selectedProject, onSelect]);

  const recentProjects = useMemo(
    () =>
      [...projects]
        .sort((a, b) => Date.parse(b.updatedAt) - Date.parse(a.updatedAt))
        .slice(0, RECENT_COUNT),
    [projects],
  );

  const recentIdsKey = recentProjects.map((p) => p.id).join(",");

  // Lazily load session counts for the popover list. Failures are tolerated (count omitted).
  useEffect(() => {
    if (!popoverOpen) return;
    attemptedCountsRef.current = new Set();
    const ids = recentIdsKey ? recentIdsKey.split(",") : [];
    for (const id of ids) {
      if (attemptedCountsRef.current.has(id)) continue;
      attemptedCountsRef.current.add(id);
      getSessionsByProject(id)
        .then((sessions) => {
          setSessionCounts((prev) => ({ ...prev, [id]: sessions.length }));
        })
        .catch(() => {
          // Count stays unknown; the row still renders without it.
        });
    }
  }, [popoverOpen, recentIdsKey]);

  const toggleStar = (projectId: string) => {
    setStarred((prev) => {
      const next = prev.includes(projectId)
        ? prev.filter((id) => id !== projectId)
        : [...prev, projectId];
      writeStarred(next);
      return next;
    });
  };

  const closePopover = useCallback(() => setPopoverOpen(false), []);

  const closeModal = useCallback(() => {
    setModalOpen(false);
    pillRef.current?.focus();
  }, []);

  const closeCreate = useCallback(() => {
    setCreateOpen(false);
  }, []);

  // Escape closes the popover. Dialogs handle Escape and focus through Blueprint's overlay stack.
  useEffect(() => {
    if (!popoverOpen) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      closePopover();
      pillRef.current?.focus();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [popoverOpen, closePopover]);

  // Click outside the popover closes it.
  useEffect(() => {
    if (!popoverOpen) return;
    const onPointerDown = (e: MouseEvent) => {
      if (wrapperRef.current && !wrapperRef.current.contains(e.target as Node)) {
        setPopoverOpen(false);
      }
    };
    document.addEventListener("mousedown", onPointerDown);
    return () => document.removeEventListener("mousedown", onPointerDown);
  }, [popoverOpen]);

  const openCreate = () => {
    setPopoverOpen(false);
    setCreateOpen(true);
  };

  const requestCloseCreate = () => {
    if (!createBusy) closeCreate();
  };

  const handleCreate = async (e?: FormEvent) => {
    e?.preventDefault();
    if (!newName.trim() || createBusy) return;
    setCreateBusy(true);
    try {
      const project = await createProject(newName.trim(), newDesc.trim() || null);
      closeCreate();
      await fetchProjects();
      onSelect(project);
      onToast(`Proyek "${project.name}" dibuat.`);
    } catch (err) {
      onError(commandErrorMessage(err));
    } finally {
      setCreateBusy(false);
    }
  };

  const handleDelete = async (project: Project) => {
    setDeleteBusy(true);
    try {
      await deleteProject(project.id);
      setDeleteTarget(null);
      setStarred((prev) => {
        if (!prev.includes(project.id)) return prev;
        const next = prev.filter((id) => id !== project.id);
        writeStarred(next);
        return next;
      });
      await fetchProjects();
      onProjectDeleted(project.id);
      onToast(`Proyek "${project.name}" dihapus beserta seluruh sesi dan citranya.`);
    } catch (err) {
      onError(commandErrorMessage(err));
    } finally {
      setDeleteBusy(false);
    }
  };

  const handleSelect = (project: Project) => {
    onSelect(project);
    setPopoverOpen(false);
    setModalOpen(false);
  };

  const visibleProjects = useMemo(() => {
    const needle = query.trim().toLowerCase();
    let list = projects;
    if (tab === "starred") list = list.filter((p) => starred.includes(p.id));
    if (needle) {
      list = list.filter(
        (p) => p.name.toLowerCase().includes(needle) || p.id.toLowerCase().includes(needle),
      );
    }
    if (tab === "recent") {
      return [...list].sort((a, b) => Date.parse(b.updatedAt) - Date.parse(a.updatedAt));
    }
    const dir = sort.dir === "asc" ? 1 : -1;
    return [...list].sort((a, b) => {
      if (sort.key === "name") return a.name.localeCompare(b.name, "id") * dir;
      return (Date.parse(a.createdAt) - Date.parse(b.createdAt)) * dir;
    });
  }, [projects, query, tab, starred, sort]);

  const toggleSort = (key: SortKey) => {
    setSort((prev) =>
      prev.key === key
        ? { key, dir: prev.dir === "asc" ? "desc" : "asc" }
        : { key, dir: key === "name" ? "asc" : "desc" },
    );
  };

  const ariaSort = (key: SortKey): "ascending" | "descending" | "none" => {
    if (sort.key !== key) return "none";
    return sort.dir === "asc" ? "ascending" : "descending";
  };

  const renderDeleteControl = (project: Project) => (
    <button
      type="button"
      className="m1-pp-icon-btn m1-pp-icon-btn--danger"
      onClick={() => setDeleteTarget(project)}
      aria-label={`Hapus proyek ${project.name}`}
      title="Hapus proyek"
    >
      <IconTrash />
    </button>
  );

  const deleteAlert = (
    <Alert
      isOpen={deleteTarget !== null}
      onClose={() => setDeleteTarget(null)}
      onConfirm={() => {
        if (deleteTarget) void handleDelete(deleteTarget);
      }}
      intent={Intent.DANGER}
      className="m1-custom-dialog m1-custom-alert"
      cancelButtonText="Batal"
      confirmButtonText="Hapus proyek"
      icon="trash"
      canEscapeKeyCancel={!deleteBusy}
      canOutsideClickCancel={!deleteBusy}
      loading={deleteBusy}
    >
      <div className="m1-alert-body">
        <p className="m1-alert-message">
          Apakah Anda yakin ingin menghapus proyek <strong>{deleteTarget?.name ?? ""}</strong>?
        </p>
        <p className="m1-alert-subtext">
          Tindakan ini permanen dan tidak dapat dibatalkan. Semua sesi dan citra di dalam proyek ini
          ikut terhapus dari basis data lokal.
        </p>
      </div>
    </Alert>
  );

  const createDialog = (
    <Dialog
      isOpen={createOpen}
      onClose={requestCloseCreate}
      onClosed={() => {
        setNewName("");
        setNewDesc("");
      }}
      onOpened={() => nameInputRef.current?.focus()}
      title="Proyek baru"
      className="m1-custom-dialog"
      portalClassName="m1-dialog-portal"
      canEscapeKeyClose={!createBusy}
      canOutsideClickClose={!createBusy}
    >
      <form onSubmit={(e) => void handleCreate(e)}>
        <div className="m1-dialog-content">
          <FormGroup label="Nama proyek" labelFor="m1-pp-create-name">
            <InputGroup
              id="m1-pp-create-name"
              inputRef={nameInputRef}
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              placeholder="cth: Blok A Perkebunan Sawit"
              disabled={createBusy}
              required
            />
          </FormGroup>
          <FormGroup label="Deskripsi" labelInfo="(opsional)" labelFor="m1-pp-create-desc">
            <InputGroup
              id="m1-pp-create-desc"
              value={newDesc}
              onChange={(e) => setNewDesc(e.target.value)}
              placeholder="Deskripsi singkat"
              disabled={createBusy}
            />
          </FormGroup>
        </div>
        <div className="m1-dialog-footer">
          <Button text="Batal" onClick={requestCloseCreate} disabled={createBusy} />
          <Button
            type="submit"
            intent={Intent.PRIMARY}
            text="Buat proyek"
            loading={createBusy}
            disabled={!newName.trim() || createBusy}
          />
        </div>
      </form>
    </Dialog>
  );

  const modal = (
    <Dialog
      isOpen={modalOpen}
      onClose={closeModal}
      onOpened={() => searchRef.current?.focus()}
      title="Pilih proyek"
      className="m1-custom-dialog m1-pp-dialog--wide"
      portalClassName="m1-dialog-portal"
    >
      <div className="m1-dialog-content">
        <div className="m1-pp-toolbar">
          <input
            ref={searchRef}
            type="search"
            className="m1-pp-input m1-pp-input--search"
            placeholder="Cari nama atau ID proyek"
            aria-label="Cari proyek berdasarkan nama atau ID"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
          <div className="m1-pp-toolbar__row">
            <div className="m1-pp-tabs" role="tablist" aria-label="Filter proyek">
              {(
                [
                  ["recent", "Terbaru"],
                  ["starred", "Berbintang"],
                  ["all", "Semua"],
                ] as const
              ).map(([value, label]) => (
                <button
                  key={value}
                  type="button"
                  role="tab"
                  id={`m1-pp-tab-${value}`}
                  aria-selected={tab === value}
                  aria-controls="m1-pp-tabpanel"
                  tabIndex={tab === value ? 0 : -1}
                  className={`m1-pp-tab ${tab === value ? "m1-pp-tab--active" : ""}`}
                  onClick={() => setTab(value)}
                >
                  {label}
                </button>
              ))}
            </div>
            <button
              type="button"
              className="m1-pp-btn m1-pp-btn--primary m1-pp-btn--sm"
              onClick={() => {
                setModalOpen(false);
                openCreate();
              }}
            >
              <IconPlus />
              Proyek baru
            </button>
          </div>
        </div>

        <div
          className="m1-pp-table-wrap"
          role="tabpanel"
          id="m1-pp-tabpanel"
          aria-labelledby={`m1-pp-tab-${tab}`}
        >
          {loading ? (
            <p className="m1-pp-state">Memuat proyek…</p>
          ) : visibleProjects.length === 0 ? (
            <p className="m1-pp-state">
              {projects.length === 0
                ? "Belum ada proyek."
                : tab === "starred" && !query.trim()
                  ? "Belum ada proyek berbintang. Tandai bintang pada daftar proyek."
                  : "Tidak ada proyek yang cocok."}
            </p>
          ) : (
            <table className="m1-pp-table">
              <thead>
                <tr>
                  <th scope="col" className="m1-pp-table__star">
                    <span className="m1-pp-sr-only">Berbintang</span>
                  </th>
                  <th scope="col" aria-sort={tab === "recent" ? undefined : ariaSort("name")}>
                    {tab === "recent" ? (
                      "Nama"
                    ) : (
                      <button
                        type="button"
                        className="m1-pp-sort"
                        onClick={() => toggleSort("name")}
                      >
                        Nama
                      </button>
                    )}
                  </th>
                  <th scope="col" aria-sort={tab === "recent" ? undefined : ariaSort("createdAt")}>
                    {tab === "recent" ? (
                      "Dibuat pada"
                    ) : (
                      <button
                        type="button"
                        className="m1-pp-sort"
                        onClick={() => toggleSort("createdAt")}
                      >
                        Dibuat pada
                      </button>
                    )}
                  </th>
                  <th scope="col">
                    <span className="m1-pp-sr-only">Aksi</span>
                  </th>
                </tr>
              </thead>
              <tbody>
                {visibleProjects.map((project) => {
                  const isSelected = selectedProject?.id === project.id;
                  const isStarred = starred.includes(project.id);
                  return (
                    <tr key={project.id} className={isSelected ? "m1-pp-row--selected" : ""}>
                      <td className="m1-pp-table__star">
                        <button
                          type="button"
                          className={`m1-pp-icon-btn m1-pp-star ${isStarred ? "m1-pp-star--on" : ""}`}
                          aria-pressed={isStarred}
                          aria-label={
                            isStarred
                              ? `Hapus ${project.name} dari berbintang`
                              : `Tandai ${project.name} sebagai berbintang`
                          }
                          title={isStarred ? "Hapus dari berbintang" : "Tandai berbintang"}
                          onClick={() => toggleStar(project.id)}
                        >
                          <IconStar filled={isStarred} />
                        </button>
                      </td>
                      <td>
                        <button
                          type="button"
                          className="m1-pp-name"
                          onClick={() => handleSelect(project)}
                          aria-current={isSelected ? "true" : undefined}
                        >
                          <span className="m1-pp-name__text">{project.name}</span>
                          <span className="m1-pp-name__id">ID {shortId(project.id)}</span>
                          {isSelected ? (
                            <span className="m1-pp-check" aria-label="Proyek aktif">
                              <IconCheck />
                            </span>
                          ) : null}
                        </button>
                      </td>
                      <td className="m1-pp-date">{formatDate(project.createdAt)}</td>
                      <td className="m1-pp-actions">{renderDeleteControl(project)}</td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          )}
        </div>
      </div>

      <div className="m1-dialog-footer">
        <button type="button" className="m1-pp-btn m1-pp-btn--outline" onClick={closeModal}>
          Batal
        </button>
      </div>
    </Dialog>
  );

  const popoverRecent = recentProjects;

  return (
    <div className="m1-pp" ref={wrapperRef}>
      <button
        ref={pillRef}
        type="button"
        className="m1-pp-pill"
        aria-haspopup="dialog"
        aria-expanded={popoverOpen}
        aria-controls="m1-pp-popover"
        onClick={() => setPopoverOpen((open) => !open)}
      >
        <span className="m1-pp-pill__icon">
          <IconFolder />
        </span>
        <span className="m1-pp-pill__label">
          {selectedProject ? `Proyek ${selectedProject.name}` : "Pilih proyek"}
        </span>
        <IconChevron />
      </button>

      {popoverOpen ? (
        <div
          id="m1-pp-popover"
          className="m1-pp-popover"
          role="dialog"
          aria-label="Pilih proyek cepat"
        >
          <p className="m1-pp-popover__heading">Terbaru</p>
          {loading ? (
            <p className="m1-pp-state">Memuat proyek…</p>
          ) : popoverRecent.length === 0 ? (
            <p className="m1-pp-state">Belum ada proyek.</p>
          ) : (
            <ul className="m1-pp-popover__list">
              {popoverRecent.map((project) => {
                const isSelected = selectedProject?.id === project.id;
                const count = sessionCounts[project.id];
                return (
                  <li key={project.id}>
                    <button
                      type="button"
                      className={`m1-pp-popover__item ${isSelected ? "m1-pp-popover__item--selected" : ""}`}
                      onClick={() => handleSelect(project)}
                      aria-current={isSelected ? "true" : undefined}
                    >
                      <span className="m1-pp-popover__item-text">
                        <span className="m1-pp-popover__name">{project.name}</span>
                        <span className="m1-pp-popover__meta">
                          ID {shortId(project.id)}
                          {count !== undefined ? ` · ${count} sesi` : ""}
                        </span>
                      </span>
                      {isSelected ? (
                        <span className="m1-pp-check" aria-label="Proyek aktif">
                          <IconCheck />
                        </span>
                      ) : null}
                    </button>
                  </li>
                );
              })}
            </ul>
          )}
          <div className="m1-pp-popover__footer">
            <button
              type="button"
              className="m1-pp-btn m1-pp-btn--ghost"
              onClick={() => {
                setPopoverOpen(false);
                setModalOpen(true);
              }}
            >
              Lihat semua proyek
            </button>
            <button
              type="button"
              className="m1-pp-btn m1-pp-btn--primary m1-pp-btn--sm"
              onClick={openCreate}
            >
              <IconPlus />
              Proyek baru
            </button>
          </div>
        </div>
      ) : null}

      {modal}
      {createDialog}
      {deleteAlert}
    </div>
  );
};
