import { useState, type FC, type KeyboardEvent } from "react";
import { isValidUuid } from "../sessionStats";

interface ProjectBarProps {
  projectId: string;
  onProjectIdChange: (id: string) => void;
  onGenerateProjectId: () => void;
  onLoadSessions: () => void;
  busy: boolean;
}

export const ProjectBar: FC<ProjectBarProps> = ({
  projectId,
  onProjectIdChange,
  onGenerateProjectId,
  onLoadSessions,
  busy,
}) => {
  const [touched, setTouched] = useState(false);
  const [copied, setCopied] = useState(false);

  const isValid = isValidUuid(projectId);
  const isInvalid = touched && projectId.trim() !== "" && !isValid;

  const handleBlur = () => {
    setTouched(true);
  };

  const handleKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter" && projectId.trim() && isValid && !busy) {
      onLoadSessions();
    }
  };

  const handleCopy = async () => {
    if (!projectId.trim()) return;
    try {
      await navigator.clipboard.writeText(projectId.trim());
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // Ignore clipboard error
    }
  };

  return (
    <section className="m1-project-bar" aria-label="Pengaturan ID Proyek">
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
            <h3 className="m1-project-bar__title">Workspace & ID Proyek</h3>
            <p className="m1-project-bar__subtitle">
              Sesi penerbangan dikelompokkan berdasarkan UUID proyek lokal.
            </p>
          </div>
        </div>

        {/* Action Buttons */}
        <div className="m1-project-bar__actions">
          <button
            type="button"
            className="m1-btn-load"
            onClick={onLoadSessions}
            disabled={!projectId.trim() || !isValid || busy}
            title="Tarik dan muat sesi dari basis data untuk ID proyek ini"
          >
            {busy ? (
              <span className="m1-spinner-mini" aria-hidden="true" />
            ) : (
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
                <polyline points="23 4 23 10 17 10" />
                <polyline points="1 20 1 14 7 14" />
                <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15" />
              </svg>
            )}
            <span>Muat sesi</span>
          </button>

          <button
            type="button"
            className="m1-btn-new-project"
            onClick={onGenerateProjectId}
            disabled={busy}
            title="Generate UUID baru untuk membuat proyek pemetaan baru"
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
            <span>Buat ID proyek baru</span>
          </button>
        </div>
      </div>

      {/* Input Row */}
      <div className="m1-project-bar__input-wrapper">
        <div
          className={`m1-project-input-box ${isInvalid ? "m1-project-input-box--invalid" : ""} ${isValid ? "m1-project-input-box--valid" : ""}`}
        >
          <span className="m1-project-input-prefix">ID:</span>
          <input
            id="m1-project-id-input"
            type="text"
            className="m1-project-input"
            value={projectId}
            onChange={(e) => {
              onProjectIdChange(e.target.value);
              setTouched(true);
            }}
            onBlur={handleBlur}
            onKeyDown={handleKeyDown}
            placeholder="Contoh: 550e8400-e29b-41d4-a716-446655440000"
            disabled={busy}
            spellCheck={false}
            autoComplete="off"
          />

          {projectId.trim() && (
            <button
              type="button"
              className="m1-input-action-btn"
              onClick={() => {
                void handleCopy();
              }}
              title={copied ? "ID tersalin ke clipboard!" : "Salin ID Proyek"}
              aria-label="Salin ID Proyek"
            >
              {copied ? (
                <span className="m1-copy-feedback">
                  <svg
                    width="13"
                    height="13"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="#16a34a"
                    strokeWidth="2.5"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  >
                    <polyline points="20 6 9 17 4 12" />
                  </svg>
                  Tersalin
                </span>
              ) : (
                <span className="m1-copy-idle">
                  <svg
                    width="13"
                    height="13"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="2"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  >
                    <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                    <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
                  </svg>
                  Salin
                </span>
              )}
            </button>
          )}
        </div>
      </div>

      {/* Validation or Info text */}
      <div className="m1-project-bar__meta">
        {isInvalid ? (
          <p className="m1-project-bar__hint m1-project-bar__hint--error" role="alert">
            <svg
              width="13"
              height="13"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2.2"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <circle cx="12" cy="12" r="10" />
              <line x1="12" y1="8" x2="12" y2="12" />
              <line x1="12" y1="16" x2="12.01" y2="16" />
            </svg>
            ID harus berformat UUID yang valid, contoh: 550e8400-e29b-41d4-a716-446655440000.
          </p>
        ) : (
          <p className="m1-project-bar__hint">
            <span>
              Tekan <strong>Enter</strong> atau klik <strong>Muat sesi</strong> untuk menyinkronkan
              data. ID proyek terakhir tersimpan otomatis.
            </span>
          </p>
        )}
      </div>
    </section>
  );
};
