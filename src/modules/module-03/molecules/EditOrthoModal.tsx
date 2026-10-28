import React, { useState } from "react";
import {
  Button,
  Callout,
  Classes,
  Dialog,
  FormGroup,
  InputGroup,
  NumericInput,
} from "@blueprintjs/core";
import type { StoredImageMetadata, UpdateOrthoImageParams } from "../types/storage";

export interface CurrentMapBounds {
  west: number;
  south: number;
  east: number;
  north: number;
}

export interface EditOrthoModalProps {
  isOpen: boolean;
  onClose: () => void;
  image: StoredImageMetadata | null;
  getCurrentMapBounds?: () => CurrentMapBounds | null;
  onSave: (params: UpdateOrthoImageParams) => Promise<void>;
  isSubmitting?: boolean;
}

interface EditOrthoFormProps {
  image: StoredImageMetadata;
  getCurrentMapBounds?: () => CurrentMapBounds | null;
  onSave: (params: UpdateOrthoImageParams) => Promise<void>;
  onClose: () => void;
  isSubmitting?: boolean;
}

const EditOrthoForm: React.FC<EditOrthoFormProps> = ({
  image,
  getCurrentMapBounds,
  onSave,
  onClose,
  isSubmitting = false,
}) => {
  const initialBounds = image.bounds && image.bounds.length === 2 ? image.bounds : null;
  const [filename, setFilename] = useState(image.filename || "");
  const [south, setSouth] = useState<number | undefined>(
    initialBounds ? initialBounds[0][0] : undefined,
  );
  const [west, setWest] = useState<number | undefined>(
    initialBounds ? initialBounds[0][1] : undefined,
  );
  const [north, setNorth] = useState<number | undefined>(
    initialBounds ? initialBounds[1][0] : undefined,
  );
  const [east, setEast] = useState<number | undefined>(
    initialBounds ? initialBounds[1][1] : undefined,
  );
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const handleUseCurrentView = () => {
    if (!getCurrentMapBounds) return;
    const b = getCurrentMapBounds();
    if (!b) return;
    setWest(b.west);
    setSouth(b.south);
    setEast(b.east);
    setNorth(b.north);
    setErrorMessage(null);
  };

  const validate = (): string | null => {
    if (!filename.trim()) {
      return "Nama citra wajib diisi.";
    }
    if (
      west === undefined ||
      south === undefined ||
      east === undefined ||
      north === undefined ||
      isNaN(west) ||
      isNaN(south) ||
      isNaN(east) ||
      isNaN(north)
    ) {
      return "Seluruh 4 koordinat bounding box (West, South, East, North) wajib diisi.";
    }
    if (west < -180 || west > 180 || east < -180 || east > 180) {
      return "Bujur (West/East) harus berada dalam rentang [-180, 180].";
    }
    if (south < -90 || south > 90 || north < -90 || north > 90) {
      return "Lintang (South/North) harus berada dalam rentang [-90, 90].";
    }
    if (west >= east) {
      return "Koordinat West harus lebih kecil daripada East.";
    }
    if (south >= north) {
      return "Koordinat South harus lebih kecil daripada North.";
    }
    return null;
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!image.id) return;

    const validationErr = validate();
    if (validationErr) {
      setErrorMessage(validationErr);
      return;
    }

    setErrorMessage(null);
    void onSave({
      id: image.id,
      filename: filename.trim(),
      west,
      south,
      east,
      north,
    }).catch((err: unknown) => {
      const msg = err instanceof Error ? err.message : String(err);
      setErrorMessage(msg);
    });
  };

  return (
    <form onSubmit={handleSubmit}>
      <div className={Classes.DIALOG_BODY}>
        {errorMessage && (
          <Callout intent="danger" style={{ marginBottom: "16px" }}>
            {errorMessage}
          </Callout>
        )}

        {/* Field Statis / Immutable */}
        <div
          style={{
            padding: "10px 12px",
            background: "rgba(0, 0, 0, 0.03)",
            borderRadius: "8px",
            marginBottom: "16px",
            fontSize: "12px",
            color: "var(--color-slate, #64748b)",
            display: "flex",
            justifyContent: "space-between",
          }}
        >
          <div>
            <strong>ID:</strong> {image.id}
          </div>
          <div>
            <strong>Diimpor:</strong>{" "}
            {image.imported_at ? new Date(image.imported_at).toLocaleString() : "-"}
          </div>
        </div>

        <FormGroup
          label="Nama Tampilan Citra"
          labelFor="ortho-filename"
          labelInfo="(wajib)"
          helperText="Nama identifikasi citra pada galeri dan daftar layer peta"
        >
          <InputGroup
            id="ortho-filename"
            placeholder="Contoh: Kariangau_Inspection_Area.jpg"
            value={filename}
            disabled={isSubmitting}
            onChange={(e) => setFilename(e.target.value)}
          />
        </FormGroup>

        <div style={{ marginTop: "16px" }}>
          <div
            style={{
              display: "flex",
              justifyContent: "space-between",
              alignItems: "center",
              marginBottom: "8px",
            }}
          >
            <div style={{ fontSize: "12px", fontWeight: 600, color: "var(--color-ink)" }}>
              Koordinat Bounding Box (WGS84)
            </div>
            {getCurrentMapBounds && (
              <Button
                small
                minimal
                icon="locate"
                intent="primary"
                text="Gunakan Area Peta Saat Ini"
                disabled={isSubmitting}
                onClick={handleUseCurrentView}
              />
            )}
          </div>

          <div
            style={{
              display: "grid",
              gridTemplateColumns: "1fr 1fr",
              gap: "12px",
            }}
          >
            <FormGroup label="West (Bujur Barat)" labelFor="bbox-west" labelInfo="[-180..180]">
              <NumericInput
                id="bbox-west"
                fill
                stepSize={0.0001}
                minorStepSize={0.00001}
                majorStepSize={0.001}
                value={west ?? ""}
                disabled={isSubmitting}
                onValueChange={(val) => setWest(isNaN(val) ? undefined : val)}
              />
            </FormGroup>

            <FormGroup label="South (Lintang Selatan)" labelFor="bbox-south" labelInfo="[-90..90]">
              <NumericInput
                id="bbox-south"
                fill
                stepSize={0.0001}
                minorStepSize={0.00001}
                majorStepSize={0.001}
                value={south ?? ""}
                disabled={isSubmitting}
                onValueChange={(val) => setSouth(isNaN(val) ? undefined : val)}
              />
            </FormGroup>

            <FormGroup label="East (Bujur Timur)" labelFor="bbox-east" labelInfo="[-180..180]">
              <NumericInput
                id="bbox-east"
                fill
                stepSize={0.0001}
                minorStepSize={0.00001}
                majorStepSize={0.001}
                value={east ?? ""}
                disabled={isSubmitting}
                onValueChange={(val) => setEast(isNaN(val) ? undefined : val)}
              />
            </FormGroup>

            <FormGroup label="North (Lintang Utara)" labelFor="bbox-north" labelInfo="[-90..90]">
              <NumericInput
                id="bbox-north"
                fill
                stepSize={0.0001}
                minorStepSize={0.00001}
                majorStepSize={0.001}
                value={north ?? ""}
                disabled={isSubmitting}
                onValueChange={(val) => setNorth(isNaN(val) ? undefined : val)}
              />
            </FormGroup>
          </div>
        </div>
      </div>

      <div className={Classes.DIALOG_FOOTER}>
        <div className={Classes.DIALOG_FOOTER_ACTIONS}>
          <Button text="Batal" disabled={isSubmitting} onClick={onClose} />
          <Button
            intent="primary"
            type="submit"
            icon="saved"
            text="Simpan Perubahan"
            loading={isSubmitting}
          />
        </div>
      </div>
    </form>
  );
};

export const EditOrthoModal: React.FC<EditOrthoModalProps> = ({
  isOpen,
  onClose,
  image,
  getCurrentMapBounds,
  onSave,
  isSubmitting = false,
}) => {
  return (
    <Dialog
      isOpen={isOpen}
      onClose={isSubmitting ? undefined : onClose}
      title="Edit Citra Orthomosaic"
      icon="edit"
      style={{ width: "480px" }}
    >
      {isOpen && image && (
        <EditOrthoForm
          key={image.id}
          image={image}
          getCurrentMapBounds={getCurrentMapBounds}
          onSave={onSave}
          onClose={onClose}
          isSubmitting={isSubmitting}
        />
      )}
    </Dialog>
  );
};
