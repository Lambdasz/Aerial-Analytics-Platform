import { useEffect, useRef, useState, type FC, type FormEvent } from "react";
import {
  Alert,
  Button,
  Dialog,
  FormGroup,
  HTMLSelect,
  InputGroup,
  Intent,
} from "@blueprintjs/core";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import type { Session, StoredImage } from "../sessionCommands";
import { commandErrorMessage } from "../sessionCommands";
import { shortId } from "../sessionStats";
import { importErrorCode, importImages, type ImportReport } from "../../../lib/imageImport";
import { loadImagePreview } from "../../../lib/imagePreview";

/* ── Create Session Dialog ────────────────────────────────────────────── */

interface CreateDialogProps {
  isOpen: boolean;
  onClose: () => void;
  onSubmit: (name: string) => Promise<void>;
  busy: boolean;
}

export const CreateSessionDialog: FC<CreateDialogProps> = ({ isOpen, onClose, onSubmit, busy }) => {
  const [name, setName] = useState("");

  const handleSubmit = (e: FormEvent) => {
    e.preventDefault();
    void onSubmit(name);
  };

  const handleClose = () => {
    setName("");
    onClose();
  };

  return (
    <Dialog
      isOpen={isOpen}
      onClose={handleClose}
      onClosed={() => setName("")}
      title="Buat Sesi Baru"
      className="m1-custom-dialog"
      portalClassName="m1-dialog-portal"
      canEscapeKeyClose={!busy}
      canOutsideClickClose={!busy}
    >
      <form onSubmit={handleSubmit}>
        <div className="m1-dialog-content">
          <FormGroup
            label="Nama sesi penerbangan"
            labelInfo="(opsional)"
            labelFor="m1-create-name-input"
            helperText="Jika dikosongkan, nama otomatis dibuat berdasarkan tanggal & waktu sekarang."
          >
            <InputGroup
              id="m1-create-name-input"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="Contoh: Blok A - Penerbangan Pagi"
              disabled={busy}
            />
          </FormGroup>
        </div>
        <div className="m1-dialog-footer">
          <Button text="Batal" onClick={handleClose} disabled={busy} />
          <Button
            type="submit"
            intent={Intent.PRIMARY}
            text="Buat sesi"
            loading={busy}
            disabled={busy}
          />
        </div>
      </form>
    </Dialog>
  );
};

/* ── Rename Session Dialog ────────────────────────────────────────────── */

interface RenameDialogProps {
  session: Session | null;
  isOpen: boolean;
  onClose: () => void;
  onSubmit: (sessionId: string, newName: string) => Promise<void>;
  busy: boolean;
}

export const RenameSessionDialog: FC<RenameDialogProps> = ({
  session,
  isOpen,
  onClose,
  onSubmit,
  busy,
}) => {
  const [name, setName] = useState(session?.name ?? "");

  const handleSubmit = (e: FormEvent) => {
    e.preventDefault();
    if (!session || !name.trim()) return;
    void onSubmit(session.id, name.trim());
  };

  return (
    <Dialog
      isOpen={isOpen}
      onClose={onClose}
      title="Ganti Nama Sesi"
      className="m1-custom-dialog"
      portalClassName="m1-dialog-portal"
      canEscapeKeyClose={!busy}
      canOutsideClickClose={!busy}
    >
      <form onSubmit={handleSubmit}>
        <div className="m1-dialog-content">
          <FormGroup label="Nama baru sesi" labelFor="m1-rename-input">
            <InputGroup
              id="m1-rename-input"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="Masukkan nama baru sesi"
              disabled={busy}
            />
          </FormGroup>
        </div>
        <div className="m1-dialog-footer">
          <Button text="Batal" onClick={onClose} disabled={busy} />
          <Button
            type="submit"
            intent={Intent.PRIMARY}
            text="Simpan nama"
            disabled={!name.trim() || busy}
            loading={busy}
          />
        </div>
      </form>
    </Dialog>
  );
};

/* ── Import images dialog ─────────────────────────────────────────────── */

const IMAGE_EXT = new Set(["jpg", "jpeg", "dng"]);

interface PickedSource {
  path: string;
  kind: "file" | "folder";
  preview: string | null;
}

function fileNameOf(path: string): string {
  const parts = path.split(/[/\\]/);
  return parts[parts.length - 1] || path;
}

function extensionOf(path: string): string {
  const name = fileNameOf(path);
  const dot = name.lastIndexOf(".");
  return dot < 0 ? "" : name.slice(dot + 1).toLowerCase();
}

function kindOf(path: string): "file" | "folder" {
  return IMAGE_EXT.has(extensionOf(path)) ? "file" : "folder";
}

function asPaths(value: string | string[] | null): string[] {
  if (value === null) return [];
  return Array.isArray(value) ? value : [value];
}

function emptyReport(sessionId: string): ImportReport {
  return {
    session_id: sessionId,
    imported_image_ids: [],
    duplicates: [],
    rejected: [],
    not_analysis_ready: [],
  };
}

function mergeReports(left: ImportReport, right: ImportReport): ImportReport {
  return {
    session_id: right.session_id || left.session_id,
    imported_image_ids: [...left.imported_image_ids, ...right.imported_image_ids],
    duplicates: [...left.duplicates, ...right.duplicates],
    rejected: [...left.rejected, ...right.rejected],
    not_analysis_ready: [...left.not_analysis_ready, ...right.not_analysis_ready],
  };
}

function reportSummary(report: ImportReport): string {
  const parts = [`${report.imported_image_ids.length} citra ditambahkan`];
  if (report.duplicates.length > 0) parts.push(`${report.duplicates.length} duplikat`);
  if (report.rejected.length > 0) parts.push(`${report.rejected.length} ditolak`);
  if (report.not_analysis_ready.length > 0) {
    parts.push(`${report.not_analysis_ready.length} belum siap analisa`);
  }
  return parts.join(", ");
}

interface AssignImageDialogProps {
  session: Session | null;
  isOpen: boolean;
  onClose: () => void;
  onImported: (message: string) => Promise<void>;
  busy: boolean;
}

export const AssignImageDialog: FC<AssignImageDialogProps> = ({
  session,
  isOpen,
  onClose,
  onImported,
  busy: parentBusy,
}) => {
  const [picked, setPicked] = useState<PickedSource[]>([]);
  const [dragging, setDragging] = useState(false);
  const [importing, setImporting] = useState(false);
  const [progress, setProgress] = useState<{ done: number; total: number } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const pickedRef = useRef<PickedSource[]>([]);
  const busy = parentBusy || importing;

  const addPaths = async (paths: string[], force?: "file" | "folder") => {
    const known = new Set(pickedRef.current.map((item) => item.path));
    const next: PickedSource[] = [];
    for (const path of paths) {
      if (!path || known.has(path)) continue;
      known.add(path);
      next.push({ path, kind: force ?? kindOf(path), preview: null });
    }
    if (next.length === 0) return;
    pickedRef.current = [...pickedRef.current, ...next];
    setPicked(pickedRef.current);
    setError(null);
    await Promise.all(
      next
        .filter((item) => item.kind === "file")
        .map(async (item) => {
          try {
            const preview = await loadImagePreview(item.path);
            pickedRef.current = pickedRef.current.map((row) =>
              row.path === item.path ? { ...row, preview } : row,
            );
            setPicked(pickedRef.current);
          } catch {
            /* A DNG without an embedded JPEG still imports. */
          }
        }),
    );
  };

  const addPathsRef = useRef(addPaths);
  useEffect(() => {
    addPathsRef.current = addPaths;
  });

  useEffect(() => {
    if (!isOpen) return;
    let unlisten: (() => void) | undefined;
    let alive = true;
    getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type === "enter" || event.payload.type === "over") {
          setDragging(true);
        }
        if (event.payload.type === "leave") setDragging(false);
        if (event.payload.type === "drop") {
          setDragging(false);
          void addPathsRef.current(event.payload.paths);
        }
      })
      .then((stop) => {
        if (!alive) stop();
        else unlisten = stop;
      })
      .catch(() => {});
    return () => {
      alive = false;
      unlisten?.();
    };
  }, [isOpen]);

  const removePath = (path: string) => {
    // The preview URL belongs to the session-wide thumbnail cache; it is
    // shared with other views, so it is deliberately not revoked here.
    pickedRef.current = pickedRef.current.filter((row) => row.path !== path);
    setPicked(pickedRef.current);
  };

  const pickFiles = async () => {
    const selected = await open({
      multiple: true,
      title: "Pilih gambar",
      filters: [{ name: "Citra", extensions: ["jpg", "jpeg", "dng", "JPG", "JPEG", "DNG"] }],
    });
    await addPaths(asPaths(selected), "file");
  };

  const pickFolder = async () => {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "Pilih folder gambar",
    });
    await addPaths(asPaths(selected), "folder");
  };

  const handleSubmit = async (event: FormEvent) => {
    event.preventDefault();
    if (!session || picked.length === 0 || busy) return;
    setImporting(true);
    setError(null);
    setProgress({ done: 0, total: picked.length });
    // Declared before the try so a failure in a later batch can still report
    // the batches that already succeeded.
    let report = emptyReport(session.id);
    try {
      const files = picked.filter((item) => item.kind === "file").map((item) => item.path);
      const folders = picked.filter((item) => item.kind === "folder").map((item) => item.path);
      const onProgress = (step: { done: number; total: number }) => setProgress(step);
      if (files.length > 0) {
        report = mergeReports(
          report,
          await importImages(session.id, files, { fromFolder: false, onProgress }),
        );
      }
      for (const folder of folders) {
        report = mergeReports(
          report,
          await importImages(session.id, [folder], { fromFolder: true, onProgress }),
        );
      }
      if (report.imported_image_ids.length === 0 && report.rejected.length > 0) {
        const first = report.rejected[0];
        setError(
          `${fileNameOf(first.source_path)} ditolak (${importErrorCode(first.reason)}). Tidak ada citra yang ditambahkan.`,
        );
        return;
      }
      await onImported(reportSummary(report));
    } catch (err) {
      // A saved batch is on disk and in the database even when a later one
      // fails, so refresh through the same path as a full success — carrying
      // the failure into the summary, because that path closes the dialog.
      if (report.imported_image_ids.length > 0) {
        try {
          await onImported(`${reportSummary(report)} — gagal: ${commandErrorMessage(err)}`);
        } catch {
          /* The summary toast is best-effort; the import already happened. */
        }
      } else {
        setError(commandErrorMessage(err));
      }
    } finally {
      setImporting(false);
      setProgress(null);
    }
  };

  return (
    <Dialog
      isOpen={isOpen}
      onClose={onClose}
      title="Tambah Gambar ke Sesi"
      className="m1-custom-dialog m1-import-dialog"
      portalClassName="m1-dialog-portal"
      canEscapeKeyClose={!busy}
      canOutsideClickClose={!busy}
    >
      <form onSubmit={(event) => void handleSubmit(event)}>
        <div className="m1-dialog-content">
          <p className="m1-import-target">
            Ke sesi <strong>{session?.name ?? "—"}</strong>
          </p>
          <button
            type="button"
            className={`m1-drop${dragging ? " m1-drop--over" : ""}`}
            onClick={() => void pickFiles().catch((err) => setError(commandErrorMessage(err)))}
            disabled={busy}
          >
            <span className="m1-drop__arrow" aria-hidden="true">
              ↑
            </span>
            <span className="m1-drop__label">Upload gambar / folder</span>
            <span className="m1-drop__hint">
              Seret ke sini, atau klik untuk membuka file manager. JPEG dan DNG; folder dipindai
              termasuk subfolder.
            </span>
          </button>
          <button
            type="button"
            className="m1-drop-folder"
            onClick={() => void pickFolder().catch((err) => setError(commandErrorMessage(err)))}
            disabled={busy}
          >
            atau pilih folder
          </button>
          {picked.length > 0 && (
            <ul className="m1-import-previews">
              {picked.map((item) => (
                <li key={item.path} className="m1-import-preview">
                  {item.preview ? (
                    <img src={item.preview} alt="" className="m1-import-preview__img" />
                  ) : (
                    <span className="m1-import-preview__fallback">
                      {item.kind === "folder"
                        ? "DIR"
                        : extensionOf(item.path).toUpperCase() || "FILE"}
                    </span>
                  )}
                  <span className="m1-import-preview__name" title={item.path}>
                    {fileNameOf(item.path)}
                  </span>
                  <button
                    type="button"
                    className="m1-import-preview__remove"
                    onClick={() => removePath(item.path)}
                    disabled={busy}
                    aria-label={`Hapus ${fileNameOf(item.path)} dari pilihan`}
                  >
                    ×
                  </button>
                </li>
              ))}
            </ul>
          )}
          {progress && (
            <p className="m1-import-progress">
              Mengimpor {progress.done} dari {progress.total}
            </p>
          )}
          {error && <p className="m1-import-error">{error}</p>}
        </div>
        <div className="m1-dialog-footer">
          <Button text="Batal" onClick={onClose} disabled={busy} />
          <Button
            type="submit"
            intent={Intent.PRIMARY}
            text="Tambah gambar"
            disabled={picked.length === 0 || busy}
            loading={importing}
          />
        </div>
      </form>
    </Dialog>
  );
};

/* ── Delete Session Alert ─────────────────────────────────────────────── */

interface DeleteAlertProps {
  session: Session | null;
  isOpen: boolean;
  onClose: () => void;
  onConfirm: () => Promise<void>;
  busy: boolean;
}

export const DeleteSessionAlert: FC<DeleteAlertProps> = ({
  session,
  isOpen,
  onClose,
  onConfirm,
  busy,
}) => {
  return (
    <Alert
      isOpen={isOpen}
      onClose={onClose}
      onConfirm={() => {
        void onConfirm();
      }}
      intent={Intent.DANGER}
      className="m1-custom-dialog m1-custom-alert"
      cancelButtonText="Batal"
      confirmButtonText="Hapus sesi"
      icon="trash"
      canEscapeKeyCancel={!busy}
      canOutsideClickCancel={!busy}
      loading={busy}
    >
      <div className="m1-alert-body">
        <p className="m1-alert-message">
          Apakah Anda yakin ingin menghapus sesi <strong>{session?.name ?? ""}</strong>?
        </p>
        <p className="m1-alert-subtext">
          Tindakan ini permanen dan tidak dapat dibatalkan. Data sesi ini akan dihapus dari basis
          data lokal.
        </p>
      </div>
    </Alert>
  );
};

/* ── Move / copy image dialog ─────────────────────────────────────────── */

interface MoveCopyImageDialogProps {
  image: StoredImage;
  mode: "move" | "copy";
  /** Other sessions in the same project (never the image's own session). */
  sessions: Session[];
  onClose: () => void;
  onConfirm: (mode: "move" | "copy", image: StoredImage, targetSessionId: string) => Promise<void>;
}

/**
 * Picks the target session for moving or copying one image.
 *
 * The parent keys this by image and mode, so switching target starts from a
 * fresh selection; the first other session is preselected.
 */
export const MoveCopyImageDialog: FC<MoveCopyImageDialogProps> = ({
  image,
  mode,
  sessions,
  onClose,
  onConfirm,
}) => {
  const [selectedId, setSelectedId] = useState(() => sessions[0]?.id ?? "");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const isMove = mode === "move";

  const handleConfirm = async () => {
    if (!selectedId || busy) return;
    setBusy(true);
    setError(null);
    try {
      await onConfirm(mode, image, selectedId);
    } catch (err) {
      setError(commandErrorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      isOpen
      onClose={onClose}
      title={isMove ? "Pindahkan Citra" : "Salin Citra"}
      className="m1-custom-dialog"
      portalClassName="m1-dialog-portal"
      canEscapeKeyClose={!busy}
      canOutsideClickClose={!busy}
    >
      <div className="m1-dialog-content">
        <p className="m1-import-target">
          Berkas <strong>{image.filePath.split(/[/\\]/).pop() ?? image.filePath}</strong>
        </p>
        <FormGroup label="Sesi tujuan" labelFor="m1-movecopy-session">
          <HTMLSelect
            id="m1-movecopy-session"
            value={selectedId}
            disabled={busy || sessions.length === 0}
            onChange={(event) => setSelectedId(event.target.value)}
            options={sessions.map((session) => ({
              value: session.id,
              label: `${session.name} (${shortId(session.id)})`,
            }))}
          />
        </FormGroup>
        <p className="m1-import-hint">
          {isMove
            ? "Baris dan berkasnya dipindah ke sesi tujuan; tautan pasangan JPEG/DNG dilepas."
            : "Berkasnya digandakan; salinan tidak dipasangkan otomatis dengan JPEG/DNG lain."}
        </p>
        {sessions.length === 0 && (
          <p className="m1-import-error">Belum ada sesi lain di proyek ini.</p>
        )}
        {error && <p className="m1-import-error">{error}</p>}
      </div>
      <div className="m1-dialog-footer">
        <Button text="Batal" onClick={onClose} disabled={busy} />
        <Button
          intent={Intent.PRIMARY}
          text={isMove ? "Pindahkan" : "Salin"}
          disabled={!selectedId || busy}
          loading={busy}
          onClick={() => void handleConfirm()}
        />
      </div>
    </Dialog>
  );
};
