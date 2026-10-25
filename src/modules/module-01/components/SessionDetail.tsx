import { useEffect, useMemo, useState, type FC } from "react";
import { Alert, Intent } from "@blueprintjs/core";
import {
  commandErrorMessage,
  copyImageToSession,
  deleteImage,
  getImagesBySession,
  moveImageToSession,
  type Session,
  type StoredImage,
} from "../sessionCommands";
import {
  formatSingleDate,
  formatTimestamp,
  getSessionStage,
  parseCameraDate,
  shortId,
  type SessionStage,
} from "../sessionStats";
import { loadImageThumbnail } from "../../../lib/imagePreview";
import { MetadataExtractorPanel } from "./MetadataExtractorPanel";
import { MoveCopyImageDialog } from "./SessionDialogs";
import "./SessionDetail.css";

interface SessionDetailProps {
  session: Session;
  projectName: string;
  busy: boolean;
  /** Other sessions of the same project — the move/copy targets. */
  otherSessions: Session[];
  onBack: () => void;
  onToggleStatus: (session: Session) => void;
  onAssignImage: (session: Session) => void;
  onCopyId: (id: string) => void;
  /** Toast + refresh, shared with the import dialog's success path. */
  onImagesChanged: (message: string) => Promise<void>;
}

type SortKey = "name" | "taken";
type SortDir = "asc" | "desc";

const PAGE_STEP = 8;
const STEP_LABELS = ["Sesi dibuat", "Citra ditambahkan", "Selesai & diarsipkan"] as const;

function fileNameOf(path: string): string {
  const parts = path.split(/[/\\]/);
  return parts[parts.length - 1] || path;
}

function formatBytes(size: number | null): string {
  if (size === null) return "—";
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(0)} KB`;
  return `${(size / (1024 * 1024)).toFixed(1)} MB`;
}

function formatGps(lat: number | null, lon: number | null): string {
  if (lat === null || lon === null) return "GPS tidak ada";
  return `${lat.toFixed(5)}, ${lon.toFixed(5)}`;
}

function cameraLabel(image: StoredImage): string {
  const parts = [image.make, image.cameraModel].filter(Boolean);
  return parts.length > 0 ? parts.join(" ") : "Kamera tidak diketahui";
}

const ImageThumb: FC<{ path: string; format: string | null }> = ({ path, format }) => {
  const [url, setUrl] = useState<string | null>(null);
  const [missing, setMissing] = useState(false);

  useEffect(() => {
    let alive = true;
    loadImageThumbnail(path)
      .then((next) => {
        if (alive) setUrl(next);
      })
      .catch(() => {
        if (alive) setMissing(true);
      });
    return () => {
      alive = false;
      // The URL is owned by the session-wide thumbnail cache and shared with
      // other views, so it is deliberately not revoked here.
    };
  }, [path]);

  if (url) return <img className="m1-meta__thumb" src={url} alt="" />;
  return (
    <span className="m1-meta__thumb m1-meta__thumb--empty">
      {missing ? "—" : (format ?? "…").slice(0, 3).toUpperCase()}
    </span>
  );
};

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
  otherSessions,
  onBack,
  onToggleStatus,
  onAssignImage,
  onCopyId,
  onImagesChanged,
}) => {
  const [sortKey, setSortKey] = useState<SortKey>("taken");
  const [sortDir, setSortDir] = useState<SortDir>("asc");
  const [visibleCount, setVisibleCount] = useState(PAGE_STEP);
  const [images, setImages] = useState<StoredImage[]>([]);
  const [imagesError, setImagesError] = useState<string | null>(null);
  const [imagesLoading, setImagesLoading] = useState(true);
  const [selectedImage, setSelectedImage] = useState<StoredImage | null>(null);
  const [menuImageId, setMenuImageId] = useState<string | null>(null);
  const [actionTarget, setActionTarget] = useState<{
    image: StoredImage;
    mode: "move" | "copy";
  } | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<StoredImage | null>(null);
  const [actionBusy, setActionBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);

  const handleMoveOrCopy = async (
    mode: "move" | "copy",
    image: StoredImage,
    targetSessionId: string,
  ) => {
    if (mode === "move") {
      await moveImageToSession(image.id, targetSessionId);
    } else {
      await copyImageToSession(image.id, targetSessionId);
    }
    setActionTarget(null);
    await onImagesChanged(
      mode === "move" ? "Citra dipindahkan ke sesi tujuan" : "Citra disalin ke sesi tujuan",
    );
  };

  const handleDeleteImage = async () => {
    if (!deleteTarget) return;
    setActionBusy(true);
    setActionError(null);
    try {
      const outcome = await deleteImage(deleteTarget.id);
      setDeleteTarget(null);
      await onImagesChanged(
        outcome.filesLeft ? "Citra dihapus, tapi salinannya tersisa di disk" : "Citra dihapus",
      );
    } catch (err) {
      setActionError(commandErrorMessage(err));
    } finally {
      setActionBusy(false);
    }
  };

  // The row menu is hand-rolled (a portal-free dropdown like the project
  // picker's) because Blueprint's Popover does not anchor in the Tauri
  // webview. That means the dismissal behaviour is ours too: closing happens
  // on Escape, and on the first pointer outside the open menu.
  useEffect(() => {
    if (menuImageId === null) return;
    const dismiss = (event: MouseEvent | KeyboardEvent) => {
      if (event instanceof KeyboardEvent) {
        if (event.key === "Escape") setMenuImageId(null);
        return;
      }
      const target = event.target as HTMLElement | null;
      if (!target?.closest(".m1-meta__menu-anchor")) setMenuImageId(null);
    };
    document.addEventListener("pointerdown", dismiss);
    document.addEventListener("keydown", dismiss);
    return () => {
      document.removeEventListener("pointerdown", dismiss);
      document.removeEventListener("keydown", dismiss);
    };
  }, [menuImageId]);

  useEffect(() => {
    let alive = true;
    getImagesBySession(session.id)
      .then((rows) => {
        if (alive) setImages(rows);
      })
      .catch((err) => {
        if (alive) setImagesError(commandErrorMessage(err));
      })
      .finally(() => {
        if (alive) setImagesLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [session.id]);

  const hasImages = images.length > 0;
  const stage = getSessionStage(session);
  const isActive = session.status === "active";

  const sortedRows = useMemo(() => {
    const dir = sortDir === "asc" ? 1 : -1;
    return [...images].sort((a, b) => {
      if (sortKey === "name") {
        return fileNameOf(a.filePath).localeCompare(fileNameOf(b.filePath), "id-ID") * dir;
      }
      const aMs = a.capturedAt ? parseCameraDate(a.capturedAt).getTime() : 0;
      const bMs = b.capturedAt ? parseCameraDate(b.capturedAt).getTime() : 0;
      return (aMs - bMs) * dir;
    });
  }, [images, sortKey, sortDir]);

  const shownRows = sortedRows.slice(0, visibleCount);

  const handleSort = (key: SortKey) => {
    if (key === sortKey) {
      setSortDir((prev) => (prev === "asc" ? "desc" : "asc"));
    } else {
      setSortKey(key);
      setSortDir("asc");
    }
  };

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
                <span className="m1-sd-count-badge m1-tabular">
                  {imagesLoading ? "…" : images.length}
                </span>
              </div>
              <div className="m1-sd-sort">
                <button type="button" className="m1-sd-th-btn" onClick={() => handleSort("name")}>
                  Nama {sortIcon("name")}
                </button>
                <button type="button" className="m1-sd-th-btn" onClick={() => handleSort("taken")}>
                  Waktu {sortIcon("taken")}
                </button>
              </div>
            </div>

            {imagesError ? (
              <p className="m1-sd-note">{imagesError}</p>
            ) : imagesLoading ? (
              <p className="m1-sd-note">Memuat citra…</p>
            ) : hasImages ? (
              <>
                <ul className="m1-meta-list">
                  {shownRows.map((image) => (
                    <li key={image.id} className="m1-meta">
                      <ImageThumb path={image.filePath} format={image.format} />
                      <div className="m1-meta__body">
                        <button
                          type="button"
                          className="m1-meta__name"
                          title={image.filePath}
                          onClick={() => setSelectedImage(image)}
                        >
                          {fileNameOf(image.filePath)}
                        </button>
                        <p className="m1-meta__line">
                          {(image.format ?? "format tidak diketahui").toUpperCase()}
                          {" · "}
                          {image.width && image.height
                            ? `${image.width}×${image.height}`
                            : "ukuran tidak ada"}
                          {" · "}
                          {formatBytes(image.fileSize)}
                        </p>
                        <p className="m1-meta__line">
                          {cameraLabel(image)}
                          {" · "}
                          {image.capturedAt
                            ? formatSingleDate(image.capturedAt, true)
                            : "waktu tidak ada"}
                        </p>
                        <p className="m1-meta__line">
                          {formatGps(image.locationLat, image.locationLon)}
                          {image.gpsAltitudeM !== null
                            ? ` · ${image.gpsAltitudeM.toFixed(0)} m`
                            : ""}
                          {image.analysisReady === false ? " · belum siap analisa" : ""}
                        </p>
                      </div>
                      <div className="m1-meta__menu-anchor">
                        <button
                          type="button"
                          className="m1-meta__more"
                          aria-haspopup="menu"
                          aria-expanded={menuImageId === image.id}
                          aria-label={`Aksi untuk ${fileNameOf(image.filePath)}`}
                          onClick={() =>
                            setMenuImageId((current) => (current === image.id ? null : image.id))
                          }
                        >
                          ⋯
                        </button>
                        {menuImageId === image.id && (
                          <div className="m1-meta__menu" role="menu">
                            <button
                              type="button"
                              role="menuitem"
                              className="m1-meta__menu-item"
                              onClick={() => {
                                setDeleteTarget(image);
                                setMenuImageId(null);
                              }}
                            >
                              Hapus
                            </button>
                            <button
                              type="button"
                              role="menuitem"
                              className="m1-meta__menu-item"
                              disabled={otherSessions.length === 0}
                              onClick={() => {
                                setActionTarget({ image, mode: "move" });
                                setMenuImageId(null);
                              }}
                            >
                              Pindah ke sesi lain
                            </button>
                            <button
                              type="button"
                              role="menuitem"
                              className="m1-meta__menu-item"
                              disabled={otherSessions.length === 0}
                              onClick={() => {
                                setActionTarget({ image, mode: "copy" });
                                setMenuImageId(null);
                              }}
                            >
                              Salin ke sesi lain
                            </button>
                          </div>
                        )}
                      </div>
                    </li>
                  ))}
                </ul>
                <div className="m1-sd-table-footer">
                  <span className="m1-tabular">
                    Menampilkan {shownRows.length} dari {images.length} berkas
                  </span>
                  {visibleCount < images.length && (
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
                <dd>{imagesLoading ? "…" : images.length}</dd>
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

      <MetadataExtractorPanel image={selectedImage} onClose={() => setSelectedImage(null)} />

      {actionTarget && (
        <MoveCopyImageDialog
          key={`${actionTarget.image.id}-${actionTarget.mode}`}
          image={actionTarget.image}
          mode={actionTarget.mode}
          sessions={otherSessions}
          onClose={() => setActionTarget(null)}
          onConfirm={handleMoveOrCopy}
        />
      )}

      <Alert
        isOpen={deleteTarget !== null}
        onClose={() => {
          if (!actionBusy) {
            setDeleteTarget(null);
            setActionError(null);
          }
        }}
        onConfirm={() => void handleDeleteImage()}
        intent={Intent.DANGER}
        className="m1-custom-dialog m1-custom-alert"
        cancelButtonText="Batal"
        confirmButtonText="Hapus citra"
        icon="trash"
        canEscapeKeyCancel={!actionBusy}
        canOutsideClickCancel={!actionBusy}
        loading={actionBusy}
      >
        <div className="m1-alert-body">
          <p className="m1-alert-message">
            Hapus <strong>{deleteTarget ? fileNameOf(deleteTarget.filePath) : ""}</strong> dari sesi
            ini?
          </p>
          <p className="m1-alert-subtext">
            Salinan di library dipindahkan ke <code>.trash</code>, jadi masih bisa dipulihkan.
            Pasangan JPEG/DNG yang tersisa kehilangan tautannya.
          </p>
          {actionError && <p className="m1-alert-subtext">{actionError}</p>}
        </div>
      </Alert>
    </div>
  );
};
