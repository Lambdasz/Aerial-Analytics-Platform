import { useState, useRef, useEffect, type FC } from "react";
import type { Session } from "../sessionCommands";
import { shortId, formatSessionDateRange, getSessionProgressInfo } from "../sessionStats";

interface SessionCardProps {
  session: Session;
  isHighlighted: boolean;
  onRename: (session: Session) => void;
  onToggleStatus: (session: Session) => void;
  onAssignImage: (session: Session) => void;
  onDelete: (session: Session) => void;
  onCopyId: (id: string) => void;
  busy: boolean;
}

export const SessionCard: FC<SessionCardProps> = ({
  session,
  isHighlighted,
  onRename,
  onToggleStatus,
  onAssignImage,
  onDelete,
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

  const formattedDateRange = formatSessionDateRange(session.dateStart, session.dateEnd);
  const progressInfo = getSessionProgressInfo(session);

  return (
    <article
      id={`m1-session-card-${session.id}`}
      className={`m1-session-card ${isHighlighted ? "m1-session-card--highlighted" : ""}`}
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
              session.status === "active"
                ? "m1-session-card__tag--active"
                : "m1-session-card__tag--archived"
            }`}
          >
            {session.status === "active" ? "Aktif" : "Diarsipkan"}
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

        {/* Tanggal / Durasi */}
        <p className="m1-session-card__date-range m1-tabular">
          {formattedDateRange || "Waktu belum tercatat"}
        </p>

        {/* Progress Section (TaskFlow Dashboard Style) */}
        <div className="m1-session-card__progress-section">
          <div className="m1-session-card__progress-label">
            <span className="m1-progress-title">Progress</span>
            <span
              className={`m1-progress-percent m1-tabular ${
                session.status === "archived" ? "m1-progress-percent--archived" : ""
              }`}
            >
              {progressInfo.percent}%
            </span>
          </div>
          <div
            className="m1-progress-track"
            role="progressbar"
            aria-valuenow={progressInfo.percent}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-label="Progres kesiapan sesi"
          >
            <div
              className={`m1-progress-fill ${
                session.status === "archived" ? "m1-progress-fill--archived" : ""
              }`}
              style={{ width: `${progressInfo.percent}%` }}
            />
          </div>
          <div className="m1-progress-meta">
            <span className="m1-progress-meta__text">{progressInfo.label}</span>
          </div>
        </div>
      </div>

      {/* Card Footer: Info gambar + Tombol tambah */}
      <footer className="m1-session-card__footer">
        <span className="m1-session-card__images-count m1-tabular">Foto &amp; telemetri</span>

        <button
          type="button"
          className="m1-add-img-btn"
          onClick={() => onAssignImage(session)}
          disabled={busy}
        >
          <svg
            width="13"
            height="13"
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
          <span>Tambah gambar</span>
        </button>
      </footer>
    </article>
  );
};
