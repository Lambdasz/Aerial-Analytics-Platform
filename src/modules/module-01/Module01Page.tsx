import {
  useState,
  useEffect,
  useMemo,
  useRef,
  useCallback,
  Component,
  type ReactNode,
  type ErrorInfo,
} from "react";
import { Button, Callout, Intent, NonIdealState } from "@blueprintjs/core";

import {
  type Session,
  type SessionStatus,
  commandErrorMessage,
  createSession,
  deleteSession,
  getSessionsByProject,
  isCommandError,
  updateSessionName,
  updateSessionStatus,
} from "./sessionCommands";

import { type Project, countImagesByProject } from "./projectCommands";

import { calculateSessionStats, isValidUuid, parseCameraDate } from "./sessionStats";

import { SummaryBoxes } from "./components/SummaryBoxes";
import { ProjectPicker } from "./components/ProjectPicker";
import { SessionCard } from "./components/SessionCard";
import { SessionDetail } from "./components/SessionDetail";
import {
  CreateSessionDialog,
  RenameSessionDialog,
  AssignImageDialog,
  DeleteSessionAlert,
} from "./components/SessionDialogs";

import "./Module01Page.css";

const PAGE_SIZE = 9;
const STORAGE_KEY = "m1_last_project_id";

type StatusFilter = "all" | "active" | "archived";
const DEFAULT_STATUS_FILTER: StatusFilter = "active";
type SortOrder = "newest" | "oldest" | "name";
type View = { kind: "list" } | { kind: "detail"; sessionId: string };

function Module01PageInner() {
  const [selectedProject, setSelectedProject] = useState<Project | null>(() => {
    try {
      const savedJson = localStorage.getItem(STORAGE_KEY);
      if (savedJson) {
        const parsed = JSON.parse(savedJson) as Project;
        if (parsed && parsed.id && isValidUuid(parsed.id)) return parsed;
      }
    } catch {
      // Ignore
    }
    return null;
  });

  const projectId = selectedProject?.id ?? "";

  const [sessions, setSessions] = useState<Session[]>([]);
  const [loading, setLoading] = useState<boolean>(() => {
    try {
      const savedJson = localStorage.getItem(STORAGE_KEY);
      if (savedJson) {
        const parsed = JSON.parse(savedJson) as Project;
        return Boolean(parsed && parsed.id && isValidUuid(parsed.id));
      }
    } catch {
      // Ignore
    }
    return false;
  });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [totalImages, setTotalImages] = useState<number | undefined>(undefined);

  // Search, filter, and sorting
  const [searchQuery, setSearchQuery] = useState("");
  const [statusFilter, setStatusFilter] = useState<StatusFilter>(DEFAULT_STATUS_FILTER);
  const [sortOrder, setSortOrder] = useState<SortOrder>("newest");
  const [isSortOpen, setIsSortOpen] = useState(false);
  const sortContainerRef = useRef<HTMLDivElement | null>(null);
  const [requestedPage, setCurrentPage] = useState(1);

  // Sliding indicator for filter capsule
  const activeBtnRef = useRef<HTMLButtonElement | null>(null);
  const archivedBtnRef = useRef<HTMLButtonElement | null>(null);
  const allBtnRef = useRef<HTMLButtonElement | null>(null);
  const [sliderStyle, setSliderStyle] = useState<{ left: number; width: number }>({
    left: 0,
    width: 0,
  });

  useEffect(() => {
    const target =
      statusFilter === "active"
        ? activeBtnRef.current
        : statusFilter === "archived"
          ? archivedBtnRef.current
          : allBtnRef.current;
    if (target) {
      setSliderStyle({
        left: target.offsetLeft,
        width: target.offsetWidth,
      });
    }
  }, [statusFilter, sessions.length]);

  // Close sort dropdown when clicked outside
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (sortContainerRef.current && !sortContainerRef.current.contains(event.target as Node)) {
        setIsSortOpen(false);
      }
    };

    if (isSortOpen) {
      document.addEventListener("mousedown", handleClickOutside);
    }
    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, [isSortOpen]);

  // In-page navigation (no router): list screen or a session's detail screen
  const [view, setView] = useState<View>({ kind: "list" });

  // Dialog states
  const [isCreateOpen, setIsCreateOpen] = useState(false);
  const [renameTarget, setRenameTarget] = useState<Session | null>(null);
  const [assignTarget, setAssignTarget] = useState<Session | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<Session | null>(null);

  // Pure React Toast notification state (with proper cleanup)
  const [toastMessage, setToastMessage] = useState<string | null>(null);
  const toastTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const showToast = useCallback((message: string) => {
    if (toastTimerRef.current) clearTimeout(toastTimerRef.current);
    setToastMessage(message);
    toastTimerRef.current = setTimeout(() => {
      setToastMessage(null);
      toastTimerRef.current = null;
    }, 2500);
  }, []);

  // Cleanup toast timer on unmount
  useEffect(() => {
    return () => {
      if (toastTimerRef.current) clearTimeout(toastTimerRef.current);
    };
  }, []);

  // Format friendly error message matching brief
  const formatError = useCallback((err: unknown): string => {
    if (isCommandError(err)) {
      if (err.code === "IMAGE_NOT_FOUND") {
        return "Gambar dengan ID itu belum terdaftar. Impor gambarnya dulu, lalu coba lagi.";
      }
      if (err.code === "SESSION_NOT_FOUND") {
        return "Sesi tidak ditemukan, mungkin sudah dihapus. Muat ulang daftar sesi.";
      }
      if (err.code === "PROJECT_NOT_FOUND") {
        return "Proyek tidak ditemukan. Pilih proyek lain atau buat proyek baru.";
      }
      if (err.code === "DB_ERROR") {
        return "Terjadi kesalahan pada basis data lokal. Periksa log atau coba lagi.";
      }
      return `${err.message} (${err.code})`;
    }
    return commandErrorMessage(err);
  }, []);

  // Latest selected project, so a slow refetch for a previous project can be discarded.
  const projectIdRef = useRef(projectId);
  useEffect(() => {
    projectIdRef.current = projectId;
  }, [projectId]);

  // Load sessions from backend
  const fetchSessions = useCallback(
    async (pid: string, isInitial = false) => {
      if (!pid.trim() || !isValidUuid(pid)) return;

      if (isInitial) {
        setLoading(true);
      } else {
        setBusy(true);
      }
      setError(null);

      try {
        const result = await getSessionsByProject(pid.trim());
        if (projectIdRef.current !== pid) return;
        setSessions(result);
        // Also fetch image count
        try {
          const count = await countImagesByProject(pid.trim());
          if (projectIdRef.current === pid) setTotalImages(count);
        } catch {
          if (projectIdRef.current === pid) setTotalImages(undefined);
        }
      } catch (err) {
        if (projectIdRef.current === pid) setError(formatError(err));
      } finally {
        setLoading(false);
        setBusy(false);
      }
    },
    [formatError],
  );

  // Load sessions when project changes
  useEffect(() => {
    let isMounted = true;
    if (projectId && isValidUuid(projectId)) {
      getSessionsByProject(projectId.trim())
        .then((result) => {
          if (isMounted) {
            setSessions(result);
            setLoading(false);
          }
        })
        .catch((err: unknown) => {
          if (isMounted) {
            setError(formatError(err));
            setLoading(false);
          }
        });
      // Also fetch image count
      countImagesByProject(projectId.trim())
        .then((count) => {
          if (isMounted) setTotalImages(count);
        })
        .catch(() => {
          if (isMounted) setTotalImages(undefined);
        });
    } else {
      // Defer state updates to avoid synchronous setState in effect body
      queueMicrotask(() => {
        setSessions([]);
        setTotalImages(undefined);
        setLoading(false);
      });
    }
    return () => {
      isMounted = false;
    };
  }, [projectId, formatError]);

  // Handle Project selection
  const handleSelectProject = useCallback(
    (project: Project) => {
      setSelectedProject(project);
      try {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(project));
      } catch {
        // Ignore localStorage errors
      }
      // Re-picking the open project does not re-run the load effect, so don't wait on it.
      setLoading(project.id !== projectId);
      setSearchQuery("");
      setStatusFilter(DEFAULT_STATUS_FILTER);
      setCurrentPage(1);
      setView({ kind: "list" });
    },
    [projectId],
  );

  // Project deleted from the picker: clear the selection if it was the active project
  const handleProjectDeleted = useCallback(
    (deletedId: string) => {
      if (selectedProject?.id !== deletedId) return;
      setSelectedProject(null);
      try {
        localStorage.removeItem(STORAGE_KEY);
      } catch {
        // Ignore localStorage errors
      }
      setSessions([]);
      setTotalImages(undefined);
      setLoading(false);
      setSearchQuery("");
      setStatusFilter(DEFAULT_STATUS_FILTER);
      setCurrentPage(1);
      setView({ kind: "list" });
    },
    [selectedProject],
  );

  // Manual load sessions button
  const handleLoadSessions = () => {
    void fetchSessions(projectId, false);
  };

  // Create session
  const handleCreateSession = async (name: string) => {
    setBusy(true);
    setError(null);
    try {
      await createSession(projectId, name.trim() || null);
      showToast("Sesi dibuat");
      setIsCreateOpen(false);
      await fetchSessions(projectId, false);
    } catch (err) {
      setError(formatError(err));
    } finally {
      setBusy(false);
    }
  };

  // Rename session
  const handleRenameSession = async (sessionId: string, newName: string) => {
    setBusy(true);
    setError(null);
    try {
      await updateSessionName(sessionId, newName);
      showToast("Nama disimpan");
      setRenameTarget(null);
      await fetchSessions(projectId, false);
    } catch (err) {
      setError(formatError(err));
    } finally {
      setBusy(false);
    }
  };

  // Toggle active/archived status
  const handleToggleStatus = async (session: Session) => {
    const next: SessionStatus = session.status === "active" ? "archived" : "active";
    setBusy(true);
    setError(null);
    try {
      await updateSessionStatus(session.id, next);
      showToast(next === "archived" ? "Sesi diarsipkan" : "Sesi diaktifkan");
      await fetchSessions(projectId, false);
    } catch (err) {
      setError(formatError(err));
    } finally {
      setBusy(false);
    }
  };

  const [imageRevision, setImageRevision] = useState(0);

  const handleImagesImported = async (message: string) => {
    showToast(message);
    setAssignTarget(null);
    setImageRevision((revision) => revision + 1);
    if (projectId) await fetchSessions(projectId, false);
  };

  // Delete session
  const handleDeleteSession = async () => {
    if (!deleteTarget) return;
    setBusy(true);
    setError(null);
    try {
      const outcome = await deleteSession(deleteTarget.id);
      showToast(
        outcome.filesLeft
          ? "Sesi dihapus, tapi beberapa salinan tersisa di disk dan tidak bisa dibersihkan"
          : "Sesi dihapus",
      );
      setDeleteTarget(null);
      await fetchSessions(projectId, false);
    } catch (err) {
      setError(formatError(err));
    } finally {
      setBusy(false);
    }
  };

  // Copy full ID to clipboard
  const handleCopyId = async (id: string) => {
    try {
      await navigator.clipboard.writeText(id);
      showToast("ID disalin");
    } catch {
      // Fallback
    }
  };

  // Search, filter, and sort handlers that reset pagination
  const handleSearchChange = (query: string) => {
    setSearchQuery(query);
    setCurrentPage(1);
  };

  const handleStatusFilterChange = (filter: StatusFilter) => {
    setStatusFilter(filter);
    setCurrentPage(1);
  };

  const handleSortOrderChange = (order: SortOrder) => {
    setSortOrder(order);
    setCurrentPage(1);
  };

  // Calculate statistics across ALL sessions
  const stats = useMemo(
    () => ({ ...calculateSessionStats(sessions), totalImages }),
    [sessions, totalImages],
  );

  // Filter sessions
  const filteredSessions = useMemo(() => {
    let result = [...sessions];

    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase();
      result = result.filter((s) => s.name.toLowerCase().includes(q));
    }

    if (statusFilter !== "all") {
      result = result.filter((s) => s.status === statusFilter);
    }

    result.sort((a, b) => {
      if (sortOrder === "newest") {
        const diff = parseCameraDate(b.dateEnd).getTime() - parseCameraDate(a.dateEnd).getTime();
        // Fallback to createdAt for stable sort when dateEnd is identical
        return diff !== 0
          ? diff
          : new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime();
      }
      if (sortOrder === "oldest") {
        const diff =
          parseCameraDate(a.dateStart).getTime() - parseCameraDate(b.dateStart).getTime();
        return diff !== 0
          ? diff
          : new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime();
      }
      if (sortOrder === "name") {
        return a.name.localeCompare(b.name, "id-ID");
      }
      return 0;
    });

    return result;
  }, [sessions, searchQuery, statusFilter, sortOrder]);

  // Detail view: derived from `view`, so it falls back to the list when the session is gone
  const detailSession =
    view.kind === "detail" ? (sessions.find((s) => s.id === view.sessionId) ?? null) : null;

  // Pagination calculation
  const totalPages = Math.max(1, Math.ceil(filteredSessions.length / PAGE_SIZE));
  // The list can shrink under the requested page (archive, delete, refetch): clamp, don't go blank.
  const currentPage = Math.min(requestedPage, totalPages);
  const paginatedSessions = useMemo(() => {
    const startIdx = (currentPage - 1) * PAGE_SIZE;
    return filteredSessions.slice(startIdx, startIdx + PAGE_SIZE);
  }, [filteredSessions, currentPage]);

  return (
    <main className="m1-page" aria-labelledby={detailSession ? undefined : "m1-page-heading"}>
      {/* Pure React Toast notification for feedback */}
      {toastMessage && (
        <div className="m1-custom-toast" role="status" aria-live="polite">
          <svg
            width="15"
            height="15"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2.5"
            strokeLinecap="round"
            strokeLinejoin="round"
            aria-hidden="true"
          >
            <polyline points="20 6 9 17 4 12" />
          </svg>
          <span>{toastMessage}</span>
        </div>
      )}

      {/* Accessible live status region */}
      <div className="visually-hidden" aria-live="polite">
        {busy ? "Sedang memproses…" : ""}
      </div>

      {/* Error banner: shared by the list and detail views */}
      {error && (
        <Callout intent={Intent.DANGER} icon="error" title="Galat" style={{ borderRadius: 8 }}>
          <div
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "space-between",
              gap: 12,
            }}
          >
            <span>{error}</span>
            <Button small text="Coba lagi" onClick={handleLoadSessions} disabled={busy} />
          </div>
        </Callout>
      )}

      {detailSession ? (
        <SessionDetail
          key={`${detailSession.id}-${imageRevision}`}
          session={detailSession}
          projectName={selectedProject?.name ?? ""}
          busy={busy}
          otherSessions={sessions.filter((s) => s.id !== detailSession.id)}
          onBack={() => setView({ kind: "list" })}
          onToggleStatus={(s) => {
            void handleToggleStatus(s);
          }}
          onAssignImage={(s) => setAssignTarget(s)}
          onCopyId={(id) => {
            void handleCopyId(id);
          }}
          onImagesChanged={handleImagesImported}
        />
      ) : (
        <>
          {/* ── 1. Page Header (White Rounded Panel) ──────────────────── */}
          <header className="m1-white-panel m1-header-panel">
            <div className="m1-header__info">
              <div className="m1-header__tag-row">
                <span className="m1-header__tag">Modul 1</span>
              </div>
              <h1 id="m1-page-heading" className="m1-header__title">
                Sesi Penerbangan
              </h1>
              <p className="m1-header__subtitle">
                Kelola sesi pengambilan data citra udara drone dan pengelompokan gambar dalam proyek
                pemetaan.
              </p>
            </div>

            <div className="m1-header__actions">
              <ProjectPicker
                selectedProject={selectedProject}
                onSelect={handleSelectProject}
                onProjectDeleted={handleProjectDeleted}
                onError={setError}
                onToast={showToast}
              />
              <button
                type="button"
                className="m1-primary-pill-btn"
                onClick={() => setIsCreateOpen(true)}
                disabled={!selectedProject || busy}
                title={!selectedProject ? "Pilih proyek terlebih dahulu" : "Buat sesi baru"}
              >
                <svg
                  width="14"
                  height="14"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2.5"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  aria-hidden="true"
                >
                  <line x1="12" y1="5" x2="12" y2="19" />
                  <line x1="5" y1="12" x2="19" y2="12" />
                </svg>
                <span>Sesi baru</span>
              </button>
            </div>
          </header>

          {/* ── 2. Empat Card Berwarna di dalam Kontainer Putih ──────── */}
          {selectedProject && (
            <section className="m1-white-panel m1-summary-panel">
              <SummaryBoxes
                projectName={selectedProject.name}
                projectDescription={selectedProject.description}
                projectCreatedAt={selectedProject.createdAt}
                stats={stats}
              />
            </section>
          )}

          {/* ── 4. Kontainer Putih Besar: Semua Sesi + Grid Card + Pagination ── */}
          <section className="m1-white-panel m1-sessions-container">
            {/* Filter, Search & Controls Bar */}
            <div className="m1-controls-bar" aria-label="Kontrol filter dan pencarian">
              <div className="m1-controls-bar__left">
                <h2 className="m1-controls-bar__heading">
                  Daftar Sesi
                  {sessions.length > 0 && (
                    <span className="m1-controls-bar__badge m1-tabular">
                      {filteredSessions.length}{" "}
                      {filteredSessions.length !== sessions.length ? `dari ${sessions.length}` : ""}
                    </span>
                  )}
                </h2>
              </div>

              <div className="m1-controls-bar__right">
                {/* Professional Search Bar */}
                <div className="m1-search-box">
                  <svg
                    className="m1-search-icon"
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
                    <circle cx="11" cy="11" r="8" />
                    <line x1="21" y1="21" x2="16.65" y2="16.65" />
                  </svg>
                  <input
                    type="text"
                    className="m1-search-input"
                    placeholder="Cari sesi penerbangan…"
                    value={searchQuery}
                    onChange={(e) => handleSearchChange(e.target.value)}
                    disabled={loading || sessions.length === 0}
                    aria-label="Cari nama sesi penerbangan"
                  />
                  {searchQuery && (
                    <button
                      type="button"
                      className="m1-search-clear"
                      onClick={() => handleSearchChange("")}
                      title="Hapus pencarian"
                      aria-label="Hapus teks pencarian"
                    >
                      <svg
                        width="12"
                        height="12"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        strokeWidth="2.5"
                        strokeLinecap="round"
                        strokeLinejoin="round"
                      >
                        <line x1="18" y1="6" x2="6" y2="18" />
                        <line x1="6" y1="6" x2="18" y2="18" />
                      </svg>
                    </button>
                  )}
                </div>

                {/* Professional Capsule Status Filters (dengan animasi sliding indicator) */}
                <div className="m1-filter-capsule" role="tablist" aria-label="Filter status sesi">
                  {sliderStyle.width > 0 && (
                    <div
                      className="m1-filter-capsule__slider"
                      style={{
                        transform: `translateX(${sliderStyle.left}px)`,
                        width: `${sliderStyle.width}px`,
                      }}
                      aria-hidden="true"
                    />
                  )}
                  <button
                    ref={activeBtnRef}
                    type="button"
                    role="tab"
                    aria-selected={statusFilter === "active"}
                    className={`m1-filter-pill ${statusFilter === "active" ? "m1-filter-pill--active" : ""}`}
                    onClick={() => handleStatusFilterChange("active")}
                    disabled={loading || sessions.length === 0}
                  >
                    <span>Aktif</span>
                    <span className="m1-filter-pill__count m1-tabular">{stats.activeCount}</span>
                  </button>

                  <button
                    ref={archivedBtnRef}
                    type="button"
                    role="tab"
                    aria-selected={statusFilter === "archived"}
                    className={`m1-filter-pill ${statusFilter === "archived" ? "m1-filter-pill--active" : ""}`}
                    onClick={() => handleStatusFilterChange("archived")}
                    disabled={loading || sessions.length === 0}
                  >
                    <span>Diarsipkan</span>
                    <span className="m1-filter-pill__count m1-tabular">{stats.archivedCount}</span>
                  </button>

                  <button
                    ref={allBtnRef}
                    type="button"
                    role="tab"
                    aria-selected={statusFilter === "all"}
                    className={`m1-filter-pill ${statusFilter === "all" ? "m1-filter-pill--active" : ""}`}
                    onClick={() => handleStatusFilterChange("all")}
                    disabled={loading || sessions.length === 0}
                  >
                    <span>Semua</span>
                    <span className="m1-filter-pill__count m1-tabular">{sessions.length}</span>
                  </button>
                </div>

                {/* Professional Sort Dropdown (Zero-jump pure dropdown) */}
                <div className="m1-sort-container" ref={sortContainerRef}>
                  <button
                    type="button"
                    className={`m1-sort-btn ${isSortOpen ? "m1-sort-btn--active" : ""}`}
                    onClick={() => setIsSortOpen((prev) => !prev)}
                    disabled={loading || sessions.length === 0}
                    title="Urutkan daftar sesi"
                    aria-label="Urutkan daftar sesi"
                    aria-haspopup="true"
                    aria-expanded={isSortOpen}
                  >
                    <svg
                      className="m1-sort-icon"
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
                      <line x1="4" y1="6" x2="20" y2="6" />
                      <line x1="7" y1="12" x2="17" y2="12" />
                      <line x1="10" y1="18" x2="14" y2="18" />
                    </svg>
                    <span className="m1-sort-label">
                      {sortOrder === "newest"
                        ? "Terbaru"
                        : sortOrder === "oldest"
                          ? "Terlama"
                          : "Nama A–Z"}
                    </span>
                    <svg
                      className={`m1-sort-chevron ${isSortOpen ? "m1-sort-chevron--open" : ""}`}
                      width="12"
                      height="12"
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
                  </button>

                  {isSortOpen && (
                    <div className="m1-sort-dropdown-menu" role="menu">
                      <button
                        type="button"
                        role="menuitem"
                        className={`m1-sort-item ${sortOrder === "newest" ? "m1-sort-item--active" : ""}`}
                        onClick={() => {
                          handleSortOrderChange("newest");
                          setIsSortOpen(false);
                        }}
                      >
                        <span className="m1-sort-check">
                          {sortOrder === "newest" ? (
                            <svg
                              width="12"
                              height="12"
                              viewBox="0 0 24 24"
                              fill="none"
                              stroke="currentColor"
                              strokeWidth="2.5"
                              strokeLinecap="round"
                              strokeLinejoin="round"
                            >
                              <polyline points="20 6 9 17 4 12" />
                            </svg>
                          ) : null}
                        </span>
                        <span>Terbaru</span>
                      </button>

                      <button
                        type="button"
                        role="menuitem"
                        className={`m1-sort-item ${sortOrder === "oldest" ? "m1-sort-item--active" : ""}`}
                        onClick={() => {
                          handleSortOrderChange("oldest");
                          setIsSortOpen(false);
                        }}
                      >
                        <span className="m1-sort-check">
                          {sortOrder === "oldest" ? (
                            <svg
                              width="12"
                              height="12"
                              viewBox="0 0 24 24"
                              fill="none"
                              stroke="currentColor"
                              strokeWidth="2.5"
                              strokeLinecap="round"
                              strokeLinejoin="round"
                            >
                              <polyline points="20 6 9 17 4 12" />
                            </svg>
                          ) : null}
                        </span>
                        <span>Terlama</span>
                      </button>

                      <button
                        type="button"
                        role="menuitem"
                        className={`m1-sort-item ${sortOrder === "name" ? "m1-sort-item--active" : ""}`}
                        onClick={() => {
                          handleSortOrderChange("name");
                          setIsSortOpen(false);
                        }}
                      >
                        <span className="m1-sort-check">
                          {sortOrder === "name" ? (
                            <svg
                              width="12"
                              height="12"
                              viewBox="0 0 24 24"
                              fill="none"
                              stroke="currentColor"
                              strokeWidth="2.5"
                              strokeLinecap="round"
                              strokeLinejoin="round"
                            >
                              <polyline points="20 6 9 17 4 12" />
                            </svg>
                          ) : null}
                        </span>
                        <span>Nama A–Z</span>
                      </button>
                    </div>
                  )}
                </div>
              </div>
            </div>

            {/* ── Main Content / Cards List / States ─────────────────────── */}
            {loading ? (
              <div className="m1-session-grid" aria-busy="true">
                {Array.from({ length: 6 }).map((_, i) => (
                  <div key={i} className="m1-skeleton-card" />
                ))}
              </div>
            ) : !selectedProject ? (
              <div className="m1-state-container">
                <NonIdealState
                  icon="folder-open"
                  title="Pilih atau Buat Proyek"
                  description="Pilih proyek dari daftar di atas, atau buat proyek baru untuk mulai mengelola sesi penerbangan."
                />
              </div>
            ) : sessions.length === 0 ? (
              <div className="m1-state-container">
                <NonIdealState
                  icon="inbox"
                  title="Belum ada sesi penerbangan"
                  description="Proyek ini belum memiliki sesi. Buat sesi baru untuk mulai mengorganisasi penerbangan drone."
                  action={
                    <button
                      type="button"
                      className="m1-state-primary-btn"
                      onClick={() => setIsCreateOpen(true)}
                    >
                      <svg
                        width="15"
                        height="15"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        strokeWidth="2.5"
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        aria-hidden="true"
                      >
                        <line x1="12" y1="5" x2="12" y2="19" />
                        <line x1="5" y1="12" x2="19" y2="12" />
                      </svg>
                      <span>Buat sesi pertama</span>
                    </button>
                  }
                />
              </div>
            ) : filteredSessions.length === 0 ? (
              <div className="m1-state-container">
                <NonIdealState
                  icon="search"
                  title="Tidak ada sesi yang cocok"
                  description="Tidak ditemukan sesi penerbangan yang memenuhi kriteria pencarian atau filter status."
                  action={
                    <Button
                      text="Reset filter"
                      onClick={() => {
                        handleSearchChange("");
                        handleStatusFilterChange(DEFAULT_STATUS_FILTER);
                      }}
                    />
                  }
                />
              </div>
            ) : (
              <>
                <div
                  className="m1-session-grid"
                  role="region"
                  aria-label="Daftar kartu sesi penerbangan"
                >
                  {paginatedSessions.map((session) => (
                    <SessionCard
                      key={session.id}
                      session={session}
                      onRename={(s) => setRenameTarget(s)}
                      onToggleStatus={(s) => {
                        void handleToggleStatus(s);
                      }}
                      onAssignImage={(s) => setAssignTarget(s)}
                      onDelete={(s) => setDeleteTarget(s)}
                      onOpenDetail={(s) => setView({ kind: "detail", sessionId: s.id })}
                      onCopyId={(id) => {
                        void handleCopyId(id);
                      }}
                      busy={busy}
                    />
                  ))}
                  {currentPage === totalPages && (
                    <button
                      type="button"
                      className="m1-new-session-tile"
                      onClick={() => setIsCreateOpen(true)}
                      disabled={busy}
                    >
                      <span className="m1-new-session-tile__icon" aria-hidden="true">
                        <svg
                          width="20"
                          height="20"
                          viewBox="0 0 24 24"
                          fill="none"
                          stroke="currentColor"
                          strokeWidth="2.5"
                          strokeLinecap="round"
                          strokeLinejoin="round"
                        >
                          <line x1="12" y1="5" x2="12" y2="19" />
                          <line x1="5" y1="12" x2="19" y2="12" />
                        </svg>
                      </span>
                      <span>Buat sesi baru</span>
                    </button>
                  )}
                </div>

                {/* ── Pagination (Only if > 9 sessions) ──────────────────── */}
                {filteredSessions.length > PAGE_SIZE && (
                  <nav className="m1-pagination-bar" aria-label="Navigasi halaman sesi">
                    <span className="m1-pagination-bar__info m1-tabular">
                      Menampilkan {(currentPage - 1) * PAGE_SIZE + 1}–
                      {Math.min(currentPage * PAGE_SIZE, filteredSessions.length)} dari{" "}
                      {filteredSessions.length} sesi
                    </span>

                    <div className="m1-pagination-bar__buttons">
                      <button
                        type="button"
                        className="m1-page-num-btn"
                        disabled={currentPage <= 1 || busy}
                        onClick={() => setCurrentPage((p) => Math.max(1, p - 1))}
                        aria-label="Halaman sebelumnya"
                      >
                        <span className="bp5-icon bp5-icon-chevron-left" />
                      </button>

                      {Array.from({ length: totalPages }).map((_, idx) => {
                        const pageNum = idx + 1;
                        return (
                          <button
                            key={pageNum}
                            type="button"
                            className={`m1-page-num-btn ${currentPage === pageNum ? "m1-page-num-btn--active" : ""}`}
                            onClick={() => setCurrentPage(pageNum)}
                            disabled={busy}
                          >
                            {pageNum}
                          </button>
                        );
                      })}

                      <button
                        type="button"
                        className="m1-page-num-btn"
                        disabled={currentPage >= totalPages || busy}
                        onClick={() => setCurrentPage((p) => Math.min(totalPages, p + 1))}
                        aria-label="Halaman berikutnya"
                      >
                        <span className="bp5-icon bp5-icon-chevron-right" />
                      </button>
                    </div>
                  </nav>
                )}
              </>
            )}
          </section>
        </>
      )}

      {/* ── Dialogs ────────────────────────────────────────────────── */}
      <CreateSessionDialog
        isOpen={isCreateOpen}
        onClose={() => setIsCreateOpen(false)}
        onSubmit={handleCreateSession}
        busy={busy}
      />

      <RenameSessionDialog
        key={`rename-${renameTarget?.id ?? "none"}`}
        session={renameTarget}
        isOpen={renameTarget !== null}
        onClose={() => setRenameTarget(null)}
        onSubmit={handleRenameSession}
        busy={busy}
      />

      <AssignImageDialog
        key={`assign-${assignTarget?.id ?? "none"}`}
        session={assignTarget}
        isOpen={assignTarget !== null}
        onClose={() => setAssignTarget(null)}
        onImported={handleImagesImported}
        busy={busy}
      />

      <DeleteSessionAlert
        session={deleteTarget}
        isOpen={deleteTarget !== null}
        onClose={() => setDeleteTarget(null)}
        onConfirm={handleDeleteSession}
        busy={busy}
      />
    </main>
  );
}

interface ErrorBoundaryProps {
  children: ReactNode;
}

interface ErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
}

class Module01ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  constructor(props: ErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error("Module 01 Error:", error, errorInfo);
  }

  render() {
    if (this.state.hasError) {
      return (
        <main className="m1-page" style={{ padding: 32 }}>
          <div
            style={{
              background: "#ffffff",
              border: "1px solid #fee2e2",
              borderRadius: 16,
              padding: 28,
              boxShadow: "0 10px 25px -5px rgba(0,0,0,0.05)",
            }}
          >
            <h2 style={{ color: "#dc2626", margin: "0 0 12px 0", fontSize: 18, fontWeight: 700 }}>
              Terjadi Kendala Tampilan pada Modul Sesi Penerbangan
            </h2>
            <p style={{ color: "#475569", fontSize: 14, margin: "0 0 16px 0" }}>
              Silakan klik tombol di bawah untuk memuat ulang modul:
            </p>
            <pre
              style={{
                background: "#fef2f2",
                color: "#991b1b",
                padding: 14,
                borderRadius: 8,
                fontSize: 12,
                overflowX: "auto",
                margin: "0 0 16px 0",
              }}
            >
              {this.state.error?.message}
            </pre>
            <button
              type="button"
              onClick={() => {
                this.setState({ hasError: false, error: null });
                window.location.reload();
              }}
              style={{
                background: "#2563eb",
                color: "#ffffff",
                border: "none",
                borderRadius: 8,
                padding: "10px 20px",
                fontWeight: 600,
                cursor: "pointer",
              }}
            >
              Muat Ulang Halaman
            </button>
          </div>
        </main>
      );
    }
    return this.props.children;
  }
}

export function Module01Page() {
  return (
    <Module01ErrorBoundary>
      <Module01PageInner />
    </Module01ErrorBoundary>
  );
}
