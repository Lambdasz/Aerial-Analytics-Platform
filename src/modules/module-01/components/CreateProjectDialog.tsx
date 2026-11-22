import { useRef, useState, type FC, type FormEvent } from "react";
import { Button, Dialog, FormGroup, InputGroup, Intent } from "@blueprintjs/core";
import { commandErrorMessage } from "../sessionCommands";
import { type Project, createProject } from "../projectCommands";

export interface CreateProjectDialogProps {
  isOpen: boolean;
  onClose: () => void;
  /** Runs after the project is stored and the dialog has been asked to close. */
  onCreated: (project: Project) => void | Promise<void>;
  onError: (message: string) => void;
}

/**
 * "Proyek baru" dialog. Extracted from ProjectPicker so the picker and the navbar's "+" menu
 * (through Module 01) share one implementation.
 */
export const CreateProjectDialog: FC<CreateProjectDialogProps> = ({
  isOpen,
  onClose,
  onCreated,
  onError,
}) => {
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [busy, setBusy] = useState(false);
  const nameInputRef = useRef<HTMLInputElement>(null);

  const requestClose = () => {
    if (!busy) onClose();
  };

  const handleSubmit = async (e?: FormEvent) => {
    e?.preventDefault();
    if (!name.trim() || busy) return;
    setBusy(true);
    try {
      const project = await createProject(name.trim(), description.trim() || null);
      onClose();
      await onCreated(project);
    } catch (err) {
      onError(commandErrorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      isOpen={isOpen}
      onClose={requestClose}
      onClosed={() => {
        setName("");
        setDescription("");
      }}
      onOpened={() => nameInputRef.current?.focus()}
      title="Proyek baru"
      className="m1-custom-dialog"
      portalClassName="m1-dialog-portal"
      canEscapeKeyClose={!busy}
      canOutsideClickClose={!busy}
    >
      <form onSubmit={(e) => void handleSubmit(e)}>
        <div className="m1-dialog-content">
          <FormGroup label="Nama proyek" labelFor="m1-pp-create-name">
            <InputGroup
              id="m1-pp-create-name"
              inputRef={nameInputRef}
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="cth: Blok A Perkebunan Sawit"
              disabled={busy}
              required
            />
          </FormGroup>
          <FormGroup label="Deskripsi" labelInfo="(opsional)" labelFor="m1-pp-create-desc">
            <InputGroup
              id="m1-pp-create-desc"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="Deskripsi singkat"
              disabled={busy}
            />
          </FormGroup>
        </div>
        <div className="m1-dialog-footer">
          <Button text="Batal" onClick={requestClose} disabled={busy} />
          <Button
            type="submit"
            intent={Intent.PRIMARY}
            text="Buat proyek"
            loading={busy}
            disabled={!name.trim() || busy}
          />
        </div>
      </form>
    </Dialog>
  );
};
