import { useState, useEffect, useMemo, useRef, useCallback } from "react";
import {
  Button,
  Callout,
  Intent,
  NonIdealState,
  OverlayToaster,
  Position,
  type Toaster,
} from "@blueprintjs/core";

import {
  type Session,
  type SessionStatus,
  assignImageToSession,
  commandErrorMessage,
  createSession,
  deleteSession,
  getSessionsByProject,
  isCommandError,
  updateSessionName,
  updateSessionStatus,
} from "./sessionCommands";

import { calculateSessionStats, isValidUuid, parseCameraDate } from "./sessionStats";

import { SummaryBoxes } from "./components/SummaryBoxes";
import { ProjectBar } from "./components/ProjectBar";
import { SessionCard } from "./components/SessionCard";
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
type SortOrder = "newest" | "oldest" | "name";

export function Module01Page() {
  const [projectId, setProjectId] = useState<string>(() => {
    try {
      const saved = localStorage.getItem(STORAGE_KEY);
      return saved && isValidUuid(saved) ? saved : "";
    } catch {
      return "";
    }
  });

  const [sessions, setSessions] = useState<Session[]>([]);
  const [loading, setLoading] = useState<boolean>(() => {
    try {
      const saved = localStorage.getItem(STORAGE_KEY);
      return Boolean(saved && isValidUuid(saved));
    } catch {
      return false;
    }
  });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Search, filter, and sorting
  const [searchQuery, setSearchQuery] = useState("");
  const [statusFilter, setStatusFilter] = useState<StatusFilter>("all");
  const [sortOrder, setSortOrder] = useState<SortOrder>("newest");
  const [isSortOpen, setIsSortOpen] = useState(false);
  const sortContainerRef = useRef<HTMLDivElement | null>(null);
  const [currentPage, setCurrentPage] = useState(1);

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

  // Interaction: Card highlight
  const [highlightedSessionId, setHighlightedSessionId] = useState<string | null>(null);

  // Dialog states
  const [isCreateOpen, setIsCreateOpen] = useState(false);
  const [renameTarget, setRenameTarget] = useState<Session | null>(null);
  const [assignTarget, setAssignTarget] = useState<Session | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<Session | null>(null);

  // Blueprint Toaster reference
  const toasterRef = useRef<Toaster | null>(null);

  const showToast = useCallback((message: string, intent: Intent = Intent.SUCCESS) => {
    toasterRef.current?.show({
      message,
      intent,
      timeout: 2500,
    });
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
      if (err.code === "DB_ERROR") {
        return "Terjadi kesalahan pada basis data lokal. Periksa log atau coba lagi.";
      }
      return `${err.message} (${err.code})`;
    }
    return commandErrorMessage(err);
  }, []);

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
        setSessions(result);
        try {
          localStorage.setItem(STORAGE_KEY, pid.trim());
        } catch {
          // Ignore localStorage errors
        }
      } catch (err) {
        setError(formatError(err));
      } finally {
        setLoading(false);
        setBusy(false);
      }
    },
    [formatError],
  );

  // Load sessions on mount if projectId was restored from localStorage
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
    }
    return () => {
      isMounted = false;
    };
  }, [projectId, formatError]);

  // Handle Project ID input change
  const handleProjectIdChange = (newId: string) => {
    setProjectId(newId);
    try {
      localStorage.setItem(STORAGE_KEY, newId);
    } catch {
      // Ignore localStorage errors
    }
  };

  // Generate random UUID
  const handleGenerateProjectId = () => {
    const uuid = crypto.randomUUID();
    setProjectId(uuid);
    void fetchSessions(uuid, true);
  };

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

  // Assign image
  const handleAssignImage = async (sessionId: string, imageId: string) => {
    setBusy(true);
    setError(null);
    try {
      await assignImageToSession(sessionId, imageId);
      showToast("Gambar ditambahkan");
      setAssignTarget(null);
      await fetchSessions(projectId, false);
    } catch (err) {
      setError(formatError(err));
    } finally {
      setBusy(false);
    }
  };

  // Delete session
  const handleDeleteSession = async () => {
    if (!deleteTarget) return;
    setBusy(true);
    setError(null);
    try {
      await deleteSession(deleteTarget.id);
      showToast("Sesi dihapus");
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
  const stats = useMemo(() => calculateSessionStats(sessions), [sessions]);

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
        return parseCameraDate(b.dateEnd).getTime() - parseCameraDate(a.dateEnd).getTime();
      }
      if (sortOrder === "oldest") {
        return parseCameraDate(a.dateStart).getTime() - parseCameraDate(b.dateStart).getTime();
      }
      if (sortOrder === "name") {
        return a.name.localeCompare(b.name, "id-ID");
      }
      return 0;
    });

    return result;
  }, [sessions, searchQuery, statusFilter, sortOrder]);

  // Pagination calculation
  const totalPages = Math.max(1, Math.ceil(filteredSessions.length / PAGE_SIZE));
  const paginatedSessions = useMemo(() => {
    const startIdx = (currentPage - 1) * PAGE_SIZE;
    return filteredSessions.slice(startIdx, startIdx + PAGE_SIZE);
  }, [filteredSessions, currentPage]);

  // Focus & highlight latest session card
  const handleFocusLatestSession = (sessionId: string) => {
    // Ensure session is visible under filter
    setStatusFilter("all");
    setSearchQuery("");

    // Find which page contains this session
    const index = sessions.findIndex((s) => s.id === sessionId);
    if (index !== -1) {
      const page = Math.floor(index / PAGE_SIZE) + 1;
      setCurrentPage(page);
    }

    setHighlightedSessionId(sessionId);

    // Scroll into view
    setTimeout(() => {
      const el = document.getElementById(`m1-session-card-${sessionId}`);
      if (el) {
        el.scrollIntoView({ behavior: "smooth", block: "center" });
      }
    }, 100);

    setTimeout(() => {
      setHighlightedSessionId(null);
    }, 2500);
  };

  return (
    <main className="m1-page" aria-labelledby="m1-page-heading">
      {/* Blueprint OverlayToaster for feedback */}
      <OverlayToaster
        ref={(instance) => {
          toasterRef.current = instance;
        }}
        position={Position.TOP_RIGHT}
      />

      {/* Accessible live status region */}
      <div className="visually-hidden" aria-live="polite">
        {busy ? "Sedang memproses…" : ""}
      </div>

      {/* ── Page Header ────────────────────────────────────────────── */}
      <header className="m1-header">
        <div className="m1-header__info">
          <div className="m1-header__tag-row">
            <span className="m1-header__tag bp5-tag bp5-minimal">Modul 1</span>
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
          <Button
            icon="plus"
            intent={Intent.PRIMARY}
            text="Sesi baru"
            onClick={() => setIsCreateOpen(true)}
            disabled={!projectId || !isValidUuid(projectId) || busy}
          />
        </div>
      </header>

      {/* ── Error Banner ───────────────────────────────────────────── */}
      {error && (
        <Callout
          intent={Intent.DANGER}
          icon="error"
          title="Galat"
          style={{ marginBottom: 20, borderRadius: 8 }}
        >
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

      {/* ── 2. Empat Card Berwarna di dalam Kontainer Putih ──────── */}
      <section className="m1-white-panel m1-summary-panel">
        <SummaryBoxes stats={stats} onFocusLatestSession={handleFocusLatestSession} />
      </section>

      {/* ── 3. ID Proyek dan Barnya (White Rounded Panel) ───────────── */}
      <section className="m1-white-panel m1-project-panel">
        <ProjectBar
          projectId={projectId}
          onProjectIdChange={handleProjectIdChange}
          onGenerateProjectId={handleGenerateProjectId}
          onLoadSessions={handleLoadSessions}
          busy={busy || loading}
        />
      </section>

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

            {/* Professional Capsule Status Filters (tanpa titik kecil) */}
            <div className="m1-filter-capsule" role="tablist" aria-label="Filter status sesi">
              <button
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

              <button
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
        ) : !projectId || !isValidUuid(projectId) ? (
          <div className="m1-state-container">
            <NonIdealState
              icon="folder-open"
              title="Pilih atau Buat ID Proyek"
              description="Masukkan UUID proyek atau klik 'Buat ID proyek baru' di atas untuk memuat daftar sesi penerbangan."
              action={
                <Button
                  intent={Intent.PRIMARY}
                  icon="add"
                  text="Buat ID proyek baru"
                  onClick={handleGenerateProjectId}
                />
              }
            />
          </div>
        ) : sessions.length === 0 ? (
          <div className="m1-state-container">
            <NonIdealState
              icon="inbox"
              title="Belum ada sesi penerbangan"
              description="Proyek ini belum memiliki sesi. Buat sesi baru untuk mulai mengorganisasi penerbangan drone."
              action={
                <Button
                  intent={Intent.PRIMARY}
                  icon="plus"
                  text="Sesi baru"
                  onClick={() => setIsCreateOpen(true)}
                />
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
                    handleStatusFilterChange("all");
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
                  isHighlighted={highlightedSessionId === session.id}
                  onRename={(s) => setRenameTarget(s)}
                  onToggleStatus={(s) => {
                    void handleToggleStatus(s);
                  }}
                  onAssignImage={(s) => setAssignTarget(s)}
                  onDelete={(s) => setDeleteTarget(s)}
                  onCopyId={(id) => {
                    void handleCopyId(id);
                  }}
                  busy={busy}
                />
              ))}
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

      {/* ── Dialogs ────────────────────────────────────────────────── */}
      <CreateSessionDialog
        isOpen={isCreateOpen}
        onClose={() => setIsCreateOpen(false)}
        onSubmit={handleCreateSession}
        busy={busy}
      />

      <RenameSessionDialog
        key={renameTarget?.id ?? "none"}
        session={renameTarget}
        isOpen={renameTarget !== null}
        onClose={() => setRenameTarget(null)}
        onSubmit={handleRenameSession}
        busy={busy}
      />

      <AssignImageDialog
        key={assignTarget?.id ?? "none"}
        session={assignTarget}
        isOpen={assignTarget !== null}
        onClose={() => setAssignTarget(null)}
        onSubmit={handleAssignImage}
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
