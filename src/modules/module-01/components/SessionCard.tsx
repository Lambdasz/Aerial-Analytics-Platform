import { useState, useRef, useEffect, type FC } from "react";
import type { Session } from "../sessionCommands";
import {
  shortId,
  formatSingleDate,
  formatTimestamp,
  getSessionStage,
  hasImageRange,
  type SessionStage,
} from "../sessionStats";

const STEP_LABELS = ["Sesi dibuat", "Citra ditambahkan", "Selesai & diarsipkan"] as const;

interface SessionCardProps {
  session: Session;
  /** Stored image rows in this session, when the page has counted them. */
  imageCount?: number;
  onRename: (session: Session) => void;
  onToggleStatus: (session: Session) => void;
  onAssignImage: (session: Session) => void;
  onDelete: (session: Session) => void;
  onOpenDetail: (session: Session) => void;
  onCopyId: (id: string) => void;
  busy: boolean;
}

export const SessionCard: FC<SessionCardProps> = ({
  session,
  imageCount,
  onRename,
  onToggleStatus,
  onAssignImage,
  onDelete,
  onOpenDetail,
  onCopyId,
  busy,
}) => {
  const [isMenuOpen, setIsMenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement | null>(null);

  // Close menu on outside click
  useEffect(() => {
    const handleOutsideClick = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setIsMenuOpen(false);
      }
    };
    if (isMenuOpen) {
      document.addEventListener("mousedown", handleOutsideClick);
    }
    return () => {
      document.removeEventListener("mousedown", handleOutsideClick);
    };
  }, [isMenuOpen]);

  // The stored count is the truth; the date-range guess is only the fallback
  // while the count has not arrived (or failed to).
  const hasCaptureRange = hasImageRange(session);
  const hasImages = imageCount === undefined ? hasCaptureRange : imageCount > 0;
  const stage = getSessionStage(session, hasImages);
  const isActive = session.status === "active";

  return (
    <article
      id={`m1-session-card-${session.id}`}
      className={`m1-session-card ${isActive ? "m1-session-card--active" : ""}`}
      style={{
        zIndex: isMenuOpen ? 50 : 1,
        position: "relative",
      }}
      aria-label={`Sesi ${session.name}`}
    >
      <div className="m1-session-card__body">
        {/* Header Row: Status & 3-Dots Action */}
        <div className="m1-session-card__header">
          <span
            className={`m1-session-card__tag ${
              isActive ? "m1-session-card__tag--active" : "m1-session-card__tag--archived"
            }`}
          >
            {isActive ? "Aktif" : "Diarsipkan"}
          </span>

          <div className="m1-card-menu-wrapper" ref={menuRef}>
            <button
              type="button"
              className={`m1-card-menu-btn ${isMenuOpen ? "m1-card-menu-btn--active" : ""}`}
              onClick={() => setIsMenuOpen((prev) => !prev)}
              aria-label={`Menu opsi untuk sesi ${session.name}`}
              aria-expanded={isMenuOpen}
              disabled={busy}
            >
              <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
                <circle cx="5" cy="12" r="2.2" />
                <circle cx="12" cy="12" r="2.2" />
                <circle cx="19" cy="12" r="2.2" />
              </svg>
            </button>

            {isMenuOpen && (
              <div className="m1-card-dropdown-menu" role="menu">
                <button
                  type="button"
                  role="menuitem"
                  className="m1-card-menu-item"
                  onClick={() => {
                    setIsMenuOpen(false);
                    onRename(session);
                  }}
                  disabled={busy}
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
                    <path d="M12 20h9" />
                    <path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z" />
                  </svg>
                  <span>Ganti nama</span>
                </button>

                <button
                  type="button"
                  role="menuitem"
                  className="m1-card-menu-item"
                  onClick={() => {
                    setIsMenuOpen(false);
                    onToggleStatus(session);
                  }}
                  disabled={busy}
                >
                  {session.status === "active" ? (
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
                      <polyline points="21 8 21 21 3 21 3 8" />
                      <rect x="1" y="3" width="22" height="5" />
                      <line x1="10" y1="12" x2="14" y2="12" />
                    </svg>
                  ) : (
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
                      <polyline points="21 8 21 21 3 21 3 8" />
                      <rect x="1" y="3" width="22" height="5" />
                      <polyline points="10 12 12 10 14 12" />
                    </svg>
                  )}
                  <span>{session.status === "active" ? "Arsipkan" : "Aktifkan"}</span>
                </button>

                <button
                  type="button"
                  role="menuitem"
                  className="m1-card-menu-item"
                  onClick={() => {
                    setIsMenuOpen(false);
                    onAssignImage(session);
                  }}
                  disabled={busy}
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
                    <path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71" />
                    <path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71" />
                  </svg>
                  <span>Tambah gambar</span>
                </button>

                <div className="m1-card-menu-divider" />

                <button
                  type="button"
                  role="menuitem"
                  className="m1-card-menu-item m1-card-menu-item--danger"
                  onClick={() => {
                    setIsMenuOpen(false);
                    onDelete(session);
                  }}
                  disabled={busy}
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
                  <span>Hapus sesi</span>
                </button>
              </div>
            )}
          </div>
        </div>

        {/* Title */}
        <h3 className="m1-session-card__name" title={session.name}>
          {session.name}
        </h3>

        {/* Short ID + Salin ID */}
        <div className="m1-session-card__id-row">
          <span className="m1-session-card__id-badge m1-tabular">{shortId(session.id)}</span>
          <button
            type="button"
            className="m1-copy-btn"
            onClick={() => onCopyId(session.id)}
            title={`Salin ID lengkap: ${session.id}`}
          >
            <svg
              width="13"
              height="13"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
              aria-hidden="true"
            >
              <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
              <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
            </svg>
            <span>Salin ID</span>
          </button>
        </div>

        {/* Facts */}
        <dl className="m1-session-card__facts m1-tabular">
          <div className="m1-session-card__fact">
            <dt>Dibuat pada</dt>
            <dd>{formatTimestamp(session.createdAt, true)}</dd>
          </div>
          <div className="m1-session-card__fact">
            <dt>Diperbarui pada</dt>
            <dd>{formatTimestamp(session.updatedAt, true)}</dd>
          </div>
          <div className="m1-session-card__fact">
            <dt>Tanggal mulai</dt>
            {hasImages ? (
              hasCaptureRange ? (
                <dd>{formatSingleDate(session.dateStart, true)}</dd>
              ) : (
                <dd className="m1-session-card__fact-empty">waktu tidak ada</dd>
              )
            ) : (
              <dd className="m1-session-card__fact-empty">Belum ada citra</dd>
            )}
          </div>
          <div className="m1-session-card__fact">
            <dt>Tanggal selesai</dt>
            {hasImages ? (
              hasCaptureRange ? (
                <dd>{formatSingleDate(session.dateEnd, true)}</dd>
              ) : (
                <dd className="m1-session-card__fact-empty">waktu tidak ada</dd>
              )
            ) : (
              <dd className="m1-session-card__fact-empty">Belum ada citra</dd>
            )}
          </div>
        </dl>

        {/* Image strip */}
        <div className="m1-session-card__images">
          <span className="m1-session-card__images-label">
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
              <rect x="3" y="4" width="18" height="16" rx="2" />
              <circle cx="9" cy="10" r="1.6" />
              <path d="m21 16-5-5-8 8" />
            </svg>
            <span>Citra</span>
          </span>
          <span className="m1-session-card__images-count m1-tabular">
            {imageCount === undefined ? "…" : `${imageCount} berkas`}
          </span>
        </div>

        {/* Stepper: Tahap sesi */}
        <div className="m1-session-card__stepper">
          <div className="m1-session-card__stepper-head">
            <span>Tahap sesi</span>
            <span className="m1-tabular">{stage} dari 3</span>
          </div>
          <ol className="m1-session-card__steps" aria-label="Tahap sesi">
            {STEP_LABELS.map((label, index) => {
              const step = (index + 1) as SessionStage;
              // An archived session has finished every step, including the last one.
              const state =
                step < stage || (step === stage && session.status === "archived")
                  ? "done"
                  : step === stage
                    ? "current"
                    : "upcoming";
              return (
                <li
                  key={label}
                  className={`m1-session-card__step m1-session-card__step--${state}`}
                  aria-current={state === "current" ? "step" : undefined}
                >
                  <span className="m1-session-card__step-dot">
                    {state === "done" && (
                      <svg
                        width="14"
                        height="14"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        strokeWidth="3"
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        role="img"
                        aria-label="Selesai"
                      >
                        <path d="m5 12 5 5L20 7" />
                      </svg>
                    )}
                  </span>
                  <span className="m1-session-card__step-label">{label}</span>
                </li>
              );
            })}
          </ol>
        </div>
      </div>

      {/* Footer actions */}
      <footer className="m1-session-card__footer">
        <button
          type="button"
          className="m1-session-card__detail-btn"
          onClick={() => onOpenDetail(session)}
        >
          <span>Lihat detail</span>
          <svg
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2.2"
            strokeLinecap="round"
            strokeLinejoin="round"
            aria-hidden="true"
          >
            <path d="M5 12h14m0 0-6-6m6 6-6 6" />
          </svg>
        </button>

        <button
          type="button"
          className="m1-add-img-btn"
          onClick={() => onAssignImage(session)}
          disabled={busy}
        >
          <svg
            width="18"
            height="18"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2.2"
            strokeLinecap="round"
            strokeLinejoin="round"
            aria-hidden="true"
          >
            <path d="M12 15V4m0 0L8 8m4-4 4 4" />
            <path d="M4 15v3a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-3" />
          </svg>
          <span>Tambah citra</span>
        </button>
      </footer>
    </article>
  );
};
