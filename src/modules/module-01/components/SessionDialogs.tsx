import { useState, type FC, type FormEvent } from "react";
import { Dialog, Button, InputGroup, FormGroup, Alert, Intent } from "@blueprintjs/core";
import type { Session } from "../sessionCommands";
import { isValidUuid } from "../sessionStats";

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
      title="Buat Sesi Baru"
      icon="add"
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
      icon="edit"
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

/* ── Assign Image Dialog ──────────────────────────────────────────────── */

interface AssignImageDialogProps {
  session: Session | null;
  isOpen: boolean;
  onClose: () => void;
  onSubmit: (sessionId: string, imageId: string) => Promise<void>;
  busy: boolean;
}

export const AssignImageDialog: FC<AssignImageDialogProps> = ({
  session,
  isOpen,
  onClose,
  onSubmit,
  busy,
}) => {
  const [imageId, setImageId] = useState("");
  const [touched, setTouched] = useState(false);

  const isInvalid = touched && imageId.trim() !== "" && !isValidUuid(imageId);

  const handleSubmit = (e: FormEvent) => {
    e.preventDefault();
    setTouched(true);
    if (!session || !isValidUuid(imageId)) return;
    void onSubmit(session.id, imageId.trim());
  };

  return (
    <Dialog
      isOpen={isOpen}
      onClose={onClose}
      title="Tambah Gambar ke Sesi"
      icon="media"
      className="m1-custom-dialog"
      portalClassName="m1-dialog-portal"
      canEscapeKeyClose={!busy}
      canOutsideClickClose={!busy}
    >
      <form onSubmit={handleSubmit}>
        <div className="m1-dialog-content">
          <FormGroup label="ID Sesi Target" labelFor="m1-assign-session-id">
            <InputGroup id="m1-assign-session-id" value={session?.id ?? ""} readOnly disabled />
          </FormGroup>

          <FormGroup
            label="ID Gambar (UUID)"
            labelFor="m1-assign-image-id"
            intent={isInvalid ? Intent.DANGER : Intent.NONE}
            helperText={
              isInvalid
                ? "ID harus berformat UUID, contoh: 550e8400-e29b-41d4-a716-446655440000."
                : "Masukkan ID UUID gambar yang sudah terdaftar dalam sistem."
            }
          >
            <InputGroup
              id="m1-assign-image-id"
              value={imageId}
              onChange={(e) => {
                setImageId(e.target.value);
                setTouched(true);
              }}
              placeholder="Contoh: 550e8400-e29b-41d4-a716-446655440000"
              intent={isInvalid ? Intent.DANGER : Intent.NONE}
              disabled={busy}
            />
          </FormGroup>
        </div>
        <div className="m1-dialog-footer">
          <Button text="Batal" onClick={onClose} disabled={busy} />
          <Button
            type="submit"
            intent={Intent.PRIMARY}
            text="Tambah gambar"
            disabled={!isValidUuid(imageId) || busy}
            loading={busy}
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
