import { useState, useEffect, type FC } from "react";
import type { Project } from "../projectCommands";
import { listProjects, createProject, deleteProject } from "../projectCommands";
import { shortId } from "../sessionStats";

interface ProjectBarProps {
  selectedProject: Project | null;
  onSelectProject: (project: Project) => void;
  busy: boolean;
}

export const ProjectBar: FC<ProjectBarProps> = ({ selectedProject, onSelectProject, busy }) => {
  const [projects, setProjects] = useState<Project[]>([]);
  const [loading, setLoading] = useState(true);
  const [isCreateOpen, setIsCreateOpen] = useState(false);
  const [newName, setNewName] = useState("");
  const [newDesc, setNewDesc] = useState("");
  const [createBusy, setCreateBusy] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);

  const fetchProjects = async () => {
    try {
      setLoading(true);
      const result = await listProjects();
      setProjects(result);
    } catch {
      // Silently handle — list may be empty on first run
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    // Defer to avoid synchronous setState in effect body
    queueMicrotask(() => {
      void fetchProjects();
    });
  }, []);

  // Auto-select first project if none selected and projects exist
  useEffect(() => {
    if (!selectedProject && projects.length > 0) {
      onSelectProject(projects[0]);
    }
  }, [projects, selectedProject, onSelectProject]);

  const handleCreate = async () => {
    if (!newName.trim()) return;
    setCreateBusy(true);
    try {
      const project = await createProject(newName.trim(), newDesc.trim() || null);
      setNewName("");
      setNewDesc("");
      setIsCreateOpen(false);
      await fetchProjects();
      onSelectProject(project);
    } catch {
      // Error handled silently
    } finally {
      setCreateBusy(false);
    }
  };

  const handleDelete = async (projectId: string) => {
    try {
      await deleteProject(projectId);
      setConfirmDelete(null);
      await fetchProjects();
      if (selectedProject?.id === projectId) {
        // Will auto-select via useEffect
      }
    } catch {
      // Error handled silently
    }
  };

  return (
    <section className="m1-project-bar" aria-label="Manajemen Proyek">
      <div className="m1-project-bar__header">
        <div className="m1-project-bar__identity">
          <div className="m1-project-bar__icon-wrap" aria-hidden="true">
            <svg
              width="18"
              height="18"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
            </svg>
          </div>
          <div>
            <h3 className="m1-project-bar__title">Proyek Pemetaan</h3>
            <p className="m1-project-bar__subtitle">
              Pilih atau buat proyek untuk mengelola sesi penerbangan drone.
            </p>
          </div>
        </div>

        <div className="m1-project-bar__actions">
          <button
            type="button"
            className="m1-btn-new-project"
            onClick={() => setIsCreateOpen(!isCreateOpen)}
            disabled={busy || createBusy}
            title="Buat proyek pemetaan baru"
          >
            <svg
              width="15"
              height="15"
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
            <span>Proyek baru</span>
          </button>
        </div>
      </div>

      {/* Create Project Inline Form */}
      {isCreateOpen && (
        <div className="m1-project-create-form">
          <div className="m1-project-create-form__row">
            <input
              type="text"
              className="m1-project-create-input"
              placeholder="Nama proyek, cth: Blok A Perkebunan Sawit"
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && newName.trim()) void handleCreate();
              }}
              disabled={createBusy}
            />
            <input
              type="text"
              className="m1-project-create-input m1-project-create-input--desc"
              placeholder="Deskripsi singkat (opsional)"
              value={newDesc}
              onChange={(e) => setNewDesc(e.target.value)}
              disabled={createBusy}
            />
          </div>
          <div className="m1-project-create-form__actions">
            <button
              type="button"
              className="m1-btn-cancel"
              onClick={() => {
                setIsCreateOpen(false);
                setNewName("");
                setNewDesc("");
              }}
              disabled={createBusy}
            >
              Batal
            </button>
            <button
              type="button"
              className="m1-btn-create"
              onClick={() => void handleCreate()}
              disabled={!newName.trim() || createBusy}
            >
              {createBusy ? <span className="m1-spinner-mini" aria-hidden="true" /> : null}
              Buat proyek
            </button>
          </div>
        </div>
      )}

      {/* Project List / Selector */}
      {loading ? (
        <div className="m1-project-list-loading">
          <span className="m1-spinner-mini" aria-hidden="true" />
          <span>Memuat proyek…</span>
        </div>
      ) : projects.length === 0 ? (
        <div className="m1-project-empty">
          <svg
            width="24"
            height="24"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
            aria-hidden="true"
          >
            <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
          </svg>
          <p>Belum ada proyek. Klik &quot;Proyek baru&quot; untuk memulai.</p>
        </div>
      ) : (
        <div className="m1-project-list" role="listbox" aria-label="Daftar proyek pemetaan">
          {projects.map((project) => (
            <div
              key={project.id}
              role="option"
              aria-selected={selectedProject?.id === project.id}
              className={`m1-project-item ${selectedProject?.id === project.id ? "m1-project-item--selected" : ""}`}
              onClick={() => onSelectProject(project)}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  onSelectProject(project);
                }
              }}
              tabIndex={0}
            >
              <div className="m1-project-item__info">
                <div className="m1-project-item__name-row">
                  <span className="m1-project-item__name">{project.name}</span>
                  <span className="m1-project-item__id m1-tabular">{shortId(project.id)}</span>
                </div>
                {project.description && (
                  <span className="m1-project-item__desc">{project.description}</span>
                )}
              </div>

              {/* Delete button (only visible on hover) */}
              {confirmDelete === project.id ? (
                <div className="m1-project-item__confirm-delete">
                  <button
                    type="button"
                    className="m1-btn-confirm-delete"
                    onClick={(e) => {
                      e.stopPropagation();
                      void handleDelete(project.id);
                    }}
                  >
                    Hapus
                  </button>
                  <button
                    type="button"
                    className="m1-btn-cancel-delete"
                    onClick={(e) => {
                      e.stopPropagation();
                      setConfirmDelete(null);
                    }}
                  >
                    Batal
                  </button>
                </div>
              ) : (
                <button
                  type="button"
                  className="m1-project-item__delete-btn"
                  onClick={(e) => {
                    e.stopPropagation();
                    setConfirmDelete(project.id);
                  }}
                  title="Hapus proyek"
                  aria-label={`Hapus proyek ${project.name}`}
                >
                  <svg
                    width="14"
                    height="14"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="2"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  >
                    <polyline points="3 6 5 6 21 6" />
                    <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
                  </svg>
                </button>
              )}
            </div>
          ))}
        </div>
      )}
    </section>
  );
};
