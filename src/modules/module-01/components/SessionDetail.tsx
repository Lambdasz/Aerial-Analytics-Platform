import { useMemo, useState, type FC } from "react";
import type { Session } from "../sessionCommands";
import {
  formatSingleDate,
  formatTimestamp,
  getSessionStage,
  hasImageRange,
  parseCameraDate,
  shortId,
  type SessionStage,
} from "../sessionStats";
import "./SessionDetail.css";

interface SessionDetailProps {
  session: Session;
  projectName: string;
  busy: boolean;
  onBack: () => void;
  onToggleStatus: (session: Session) => void;
  onAssignImage: (session: Session) => void;
  onCopyId: (id: string) => void;
}

interface ImageRow {
  id: string;
  name: string;
  /** Capture time in camera-local ISO form, parseable by `parseCameraDate`. */
  takenIso: string;
  takenMs: number;
}

type SortKey = "name" | "taken";
type SortDir = "asc" | "desc";

const PAGE_STEP = 5;
const MOCK_IMAGE_COUNT = 12;
const STEP_LABELS = ["Sesi dibuat", "Citra ditambahkan", "Selesai & diarsipkan"] as const;

const pad = (n: number) => String(n).padStart(2, "0");

/** Formats a local Date as a camera-local ISO string (no timezone suffix). */
function toCameraIso(d: Date): string {
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(
    d.getHours(),
  )}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

// TODO: replace with a real list_images_by_session command once the backend exists.
// Mock rows are spread evenly across the session's date range, and only exist when the
// session has an image range (see `hasImageRange`). Otherwise the list is empty.
function buildMockImageRows(session: Session): ImageRow[] {
  if (!hasImageRange(session)) return [];
  const startMs = parseCameraDate(session.dateStart).getTime();
  const endMs = parseCameraDate(session.dateEnd).getTime();
  const step = (endMs - startMs) / (MOCK_IMAGE_COUNT - 1);
  return Array.from({ length: MOCK_IMAGE_COUNT }, (_, i) => {
    const takenMs = Math.round(startMs + step * i);
    return {
      id: `mock-${session.id}-${i + 1}`,
      name: `IMG_${String(i + 1).padStart(4, "0")}.JPG`,
      takenIso: toCameraIso(new Date(takenMs)),
      takenMs,
    };
  });
}

const StageStepper: FC<{ stage: SessionStage; completed: boolean }> = ({ stage, completed }) => (
  <ol className="m1-sd-steps" aria-label="Tahap sesi">
    {STEP_LABELS.map((label, index) => {
      const step = (index + 1) as SessionStage;
      const state =
        step < stage || (completed && step === stage)
          ? "done"
          : step === stage
            ? "current"
            : "upcoming";
      return (
        <li
          key={label}
          className={`m1-sd-step m1-sd-step--${state}`}
          aria-current={state === "current" ? "step" : undefined}
        >
          <span className="m1-sd-step__dot" aria-hidden="true">
            {state === "done" ? "✓" : step}
          </span>
          <span className="m1-sd-step__label">{label}</span>
        </li>
      );
    })}
  </ol>
);

export const SessionDetail: FC<SessionDetailProps> = ({
  session,
  projectName,
  busy,
  onBack,
  onToggleStatus,
  onAssignImage,
  onCopyId,
}) => {
  const [sortKey, setSortKey] = useState<SortKey>("taken");
  const [sortDir, setSortDir] = useState<SortDir>("asc");
  const [visibleCount, setVisibleCount] = useState(PAGE_STEP);

  const rows = useMemo(() => buildMockImageRows(session), [session]);
  const hasImages = rows.length > 0;
  const stage = getSessionStage(session);
  const isActive = session.status === "active";

  const sortedRows = useMemo(() => {
    const dir = sortDir === "asc" ? 1 : -1;
    return [...rows].sort((a, b) =>
      sortKey === "name"
        ? a.name.localeCompare(b.name, "id-ID") * dir
        : (a.takenMs - b.takenMs) * dir,
    );
  }, [rows, sortKey, sortDir]);

  const shownRows = sortedRows.slice(0, visibleCount);

  const handleSort = (key: SortKey) => {
    if (key === sortKey) {
      setSortDir((prev) => (prev === "asc" ? "desc" : "asc"));
    } else {
      setSortKey(key);
      setSortDir("asc");
    }
  };

  const ariaSort = (key: SortKey): "ascending" | "descending" | "none" =>
    sortKey !== key ? "none" : sortDir === "asc" ? "ascending" : "descending";

  const sortIcon = (key: SortKey) => (sortKey === key && sortDir === "desc" ? "↓" : "↑");

  return (
    <div className="m1-sd">
      {/* Breadcrumb */}
      <nav className="m1-sd-breadcrumb" aria-label="Breadcrumb">
        <button type="button" className="m1-sd-breadcrumb__back" onClick={onBack}>
          Sesi Penerbangan
        </button>
        <span className="m1-sd-breadcrumb__sep" aria-hidden="true">
          ›
        </span>
        <span className="m1-sd-breadcrumb__item">{projectName}</span>
        <span className="m1-sd-breadcrumb__sep" aria-hidden="true">
          ›
        </span>
        <span className="m1-sd-breadcrumb__current" aria-current="page">
          {session.name}
        </span>
      </nav>

      {/* Header */}
      <header className="m1-sd-header">
        <div className="m1-sd-header__info">
          <div className="m1-sd-header__meta">
            <span
              className={`m1-sd-status ${isActive ? "m1-sd-status--active" : "m1-sd-status--archived"}`}
            >
              {isActive ? "Aktif" : "Diarsipkan"}
            </span>
            <code className="m1-sd-id-chip m1-tabular" title={session.id}>
              {shortId(session.id)}
            </code>
            <button
              type="button"
              className="m1-sd-copy-btn"
              onClick={() => onCopyId(session.id)}
              aria-label="Salin ID sesi"
            >
              Salin ID
            </button>
          </div>
          <h1 className="m1-sd-header__title">{session.name}</h1>
          <p className="m1-sd-header__subtitle">
            Sesi pengambilan data citra udara drone pada proyek {projectName}.
          </p>
        </div>

        <div className="m1-sd-header__actions">
          <button
            type="button"
            className="m1-sd-btn m1-sd-btn--outline"
            onClick={() => onToggleStatus(session)}
            disabled={busy}
          >
            {isActive ? "Arsipkan sesi" : "Aktifkan sesi"}
          </button>
          <button
            type="button"
            className="m1-sd-btn m1-sd-btn--primary"
            onClick={() => onAssignImage(session)}
            disabled={busy}
          >
            Tambah citra
          </button>
        </div>
      </header>

      <div className="m1-sd-body">
        <div className="m1-sd-main">
          {/* Stage panel */}
          <section className="m1-sd-panel" aria-labelledby="m1-sd-stage-heading">
            <div className="m1-sd-panel__head">
              <h2 id="m1-sd-stage-heading" className="m1-sd-panel__title">
                Tahap sesi
              </h2>
              <span className="m1-sd-panel__meta m1-tabular">Tahap {stage} dari 3</span>
            </div>
            <StageStepper stage={stage} completed={session.status === "archived"} />
          </section>

          {/* Images panel */}
          <section className="m1-sd-panel" aria-labelledby="m1-sd-images-heading">
            <div className="m1-sd-panel__head">
              <div className="m1-sd-panel__title-row">
                <h2 id="m1-sd-images-heading" className="m1-sd-panel__title">
                  Citra &amp; telemetri
                </h2>
                <span className="m1-sd-count-badge m1-tabular">—</span>
              </div>
            </div>

            {hasImages ? (
              <>
                <p className="m1-sd-note">
                  Data contoh: daftar citra asli belum tersedia, jadi nama berkas dan waktu di bawah
                  hanya ilustrasi.
                </p>
                <div className="m1-sd-table-wrap">
                  <table className="m1-sd-table m1-tabular">
                    <thead>
                      <tr>
                        <th scope="col" aria-sort={ariaSort("name")}>
                          <button
                            type="button"
                            className="m1-sd-th-btn"
                            onClick={() => handleSort("name")}
                          >
                            Nama berkas
                            <span aria-hidden="true">{sortIcon("name")}</span>
                          </button>
                        </th>
                        <th scope="col" aria-sort={ariaSort("taken")}>
                          <button
                            type="button"
                            className="m1-sd-th-btn"
                            onClick={() => handleSort("taken")}
                          >
                            Waktu pengambilan
                            <span aria-hidden="true">{sortIcon("taken")}</span>
                          </button>
                        </th>
                      </tr>
                    </thead>
                    <tbody>
                      {shownRows.map((row) => (
                        <tr key={row.id}>
                          <td>{row.name}</td>
                          <td>{formatSingleDate(row.takenIso, true)}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>

                <div className="m1-sd-table-footer">
                  <span className="m1-tabular">
                    Menampilkan {shownRows.length} dari {rows.length} berkas
                  </span>
                  {visibleCount < rows.length && (
                    <button
                      type="button"
                      className="m1-sd-link-btn"
                      onClick={() => setVisibleCount((n) => n + PAGE_STEP)}
                    >
                      Tampilkan lebih banyak
                    </button>
                  )}
                </div>
              </>
            ) : (
              <div className="m1-sd-empty">
                <h3 className="m1-sd-empty__title">Belum ada citra</h3>
                <p className="m1-sd-empty__text">
                  Tambahkan minimal 1 citra agar sesi ini lanjut ke tahap berikutnya.
                </p>
                <button
                  type="button"
                  className="m1-sd-btn m1-sd-btn--primary"
                  onClick={() => onAssignImage(session)}
                  disabled={busy}
                >
                  Tambah citra
                </button>
              </div>
            )}
          </section>
        </div>

        {/* Side column: session facts */}
        <aside className="m1-sd-side" aria-labelledby="m1-sd-info-heading">
          <section className="m1-sd-panel">
            <h2 id="m1-sd-info-heading" className="m1-sd-panel__title">
              Informasi sesi
            </h2>
            <dl className="m1-sd-facts m1-tabular">
              <div className="m1-sd-fact">
                <dt>Proyek</dt>
                <dd>{projectName}</dd>
              </div>
              <div className="m1-sd-fact">
                <dt>Total citra</dt>
                <dd>—</dd>
              </div>
              <div className="m1-sd-fact">
                <dt>Dibuat pada</dt>
                <dd>{formatTimestamp(session.createdAt, true)}</dd>
              </div>
              <div className="m1-sd-fact">
                <dt>Diperbarui pada</dt>
                <dd>{formatTimestamp(session.updatedAt, true)}</dd>
              </div>
              <div className="m1-sd-fact">
                <dt>Tanggal mulai</dt>
                {hasImages ? (
                  <dd>{formatSingleDate(session.dateStart, true)}</dd>
                ) : (
                  <dd className="m1-sd-fact__empty">Belum ada citra</dd>
                )}
              </div>
              <div className="m1-sd-fact">
                <dt>Tanggal selesai</dt>
                {hasImages ? (
                  <dd>{formatSingleDate(session.dateEnd, true)}</dd>
                ) : (
                  <dd className="m1-sd-fact__empty">Belum ada citra</dd>
                )}
              </div>
            </dl>
          </section>
        </aside>
      </div>
    </div>
  );
};
