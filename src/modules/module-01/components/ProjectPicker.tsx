import { memo, useCallback, useEffect, useMemo, useRef, useState, type FC } from "react";
import { Alert, Callout, Dialog, Intent } from "@blueprintjs/core";
import { commandErrorMessage } from "../sessionCommands";
import { type Project, listProjects, deleteProject } from "../projectCommands";
import { shortId } from "../sessionStats";
import { CreateProjectDialog } from "./CreateProjectDialog";
import "./ProjectPicker.css";

export interface ProjectBrowserModalProps {
  isOpen: boolean;
  onClose: () => void;
  selectedProject: Project | null;
  onSelect: (project: Project) => void;
  onProjectDeleted: (projectId: string) => void;
  onProjectsChanged?: () => void;
}

type PickerTab = "recent" | "starred" | "all";
type SortKey = "name" | "createdAt";

const STARRED_KEY = "m1_starred_projects";

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

const ProjectBrowserModalBase: FC<ProjectBrowserModalProps> = ({
  isOpen,
  onClose,
  selectedProject,
  onSelect,
  onProjectDeleted,
  onProjectsChanged,
}) => {
  const [projects, setProjects] = useState<Project[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [starred, setStarred] = useState<string[]>(() => readStarred());

  const [tab, setTab] = useState<PickerTab>("recent");
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<{ key: SortKey; dir: "asc" | "desc" }>({
    key: "createdAt",
    dir: "desc",
  });
  const [deleteTarget, setDeleteTarget] = useState<Project | null>(null);
  const [deleteBusy, setDeleteBusy] = useState(false);
  const [createOpen, setCreateOpen] = useState(false);

  const searchRef = useRef<HTMLInputElement>(null);

  const fetchProjects = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const result = await listProjects();
      setProjects(result);
    } catch (err) {
      setError(commandErrorMessage(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (!isOpen) return;
    queueMicrotask(() => {
      void fetchProjects();
    });
  }, [isOpen, fetchProjects]);

  const toggleStar = (projectId: string) => {
    setStarred((prev) => {
      const next = prev.includes(projectId)
        ? prev.filter((id) => id !== projectId)
        : [...prev, projectId];
      writeStarred(next);
      return next;
    });
  };

  const handleCreated = async (project: Project) => {
    await fetchProjects();
    onSelect(project);
    onProjectsChanged?.();
  };

  const handleDelete = async (project: Project) => {
    setDeleteBusy(true);
    setError(null);
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
      onProjectsChanged?.();
    } catch (err) {
      setError(commandErrorMessage(err));
    } finally {
      setDeleteBusy(false);
    }
  };

  const handleSelect = (project: Project) => {
    onSelect(project);
    onClose();
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

  return (
    <>
      <Dialog
        isOpen={isOpen}
        onClose={onClose}
        onOpened={() => searchRef.current?.focus()}
        title="Pilih proyek"
        className="m1-custom-dialog m1-pp-dialog--wide"
        portalClassName="m1-dialog-portal"
      >
        <div className="m1-dialog-content">
          {error && (
            <Callout intent={Intent.DANGER} icon="error" style={{ marginBottom: 12 }}>
              {error}
            </Callout>
          )}
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
                  onClose();
                  setCreateOpen(true);
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
                    <th
                      scope="col"
                      aria-sort={tab === "recent" ? undefined : ariaSort("createdAt")}
                    >
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
                        <td className="m1-pp-actions">
                          <button
                            type="button"
                            className="m1-pp-icon-btn m1-pp-icon-btn--danger"
                            onClick={() => setDeleteTarget(project)}
                            aria-label={`Hapus proyek ${project.name}`}
                            title="Hapus proyek"
                          >
                            <IconTrash />
                          </button>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            )}
          </div>
        </div>

        <div className="m1-dialog-footer">
          <button type="button" className="m1-pp-btn m1-pp-btn--outline" onClick={onClose}>
            Batal
          </button>
        </div>
      </Dialog>

      <CreateProjectDialog
        isOpen={createOpen}
        onClose={() => setCreateOpen(false)}
        onCreated={handleCreated}
        onError={setError}
      />

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
            Tindakan ini permanen dan tidak dapat dibatalkan. Semua sesi dan citra di dalam proyek
            ini ikut terhapus dari basis data lokal.
          </p>
        </div>
      </Alert>
    </>
  );
};

export const ProjectBrowserModal = memo(ProjectBrowserModalBase);
