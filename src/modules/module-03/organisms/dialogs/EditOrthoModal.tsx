import React, { useState } from "react";
import { Button, Callout, Classes, Dialog } from "@blueprintjs/core";
import type { StoredImageMetadata, UpdateOrthoImageParams } from "../../types/storage";
import { OrthoBoundingBoxInputs } from "../../molecules/ortho/OrthoBoundingBoxInputs";

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

export const EditOrthoModal: React.FC<EditOrthoModalProps> = ({
  isOpen,
  onClose,
  image,
  getCurrentMapBounds,
  onSave,
  isSubmitting = false,
}) => {
  const [filename, setFilename] = useState("");
  const [southStr, setSouthStr] = useState("");
  const [westStr, setWestStr] = useState("");
  const [northStr, setNorthStr] = useState("");
  const [eastStr, setEastStr] = useState("");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [lastImageId, setLastImageId] = useState<string | null>(null);

  if (image && image.id !== lastImageId) {
    setLastImageId(image.id ?? null);
    setFilename(image.filename || "");
    const b = image.bounds && image.bounds.length === 2 ? image.bounds : null;
    setSouthStr(b ? String(b[0][0]) : "");
    setWestStr(b ? String(b[0][1]) : "");
    setNorthStr(b ? String(b[1][0]) : "");
    setEastStr(b ? String(b[1][1]) : "");
    setErrorMessage(null);
  }

  const handleUseCurrentView = () => {
    if (!getCurrentMapBounds) return;
    const b = getCurrentMapBounds();
    if (!b) return;
    setWestStr(String(b.west));
    setSouthStr(String(b.south));
    setEastStr(String(b.east));
    setNorthStr(String(b.north));
    setErrorMessage(null);
  };

  const validate = (): string | null => {
    if (!filename.trim()) return "Nama citra wajib diisi.";
    const [w, s, e, n] = [Number(westStr), Number(southStr), Number(eastStr), Number(northStr)];
    if ([w, s, e, n].some((v) => Number.isNaN(v)))
      return "Seluruh 4 koordinat bounding box wajib diisi angka valid.";
    if (w < -180 || w > 180 || e < -180 || e > 180)
      return "Bujur (West/East) harus dalam rentang [-180, 180].";
    if (s < -90 || s > 90 || n < -90 || n > 90)
      return "Lintang (South/North) harus dalam rentang [-90, 90].";
    if (w >= e) return "Koordinat West harus lebih kecil daripada East.";
    if (s >= n) return "Koordinat South harus lebih kecil daripada North.";
    return null;
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!image?.id) return;
    const validationErr = validate();
    if (validationErr) {
      setErrorMessage(validationErr);
      return;
    }
    setErrorMessage(null);
    void onSave({
      id: image.id,
      filename: filename.trim(),
      west: Number(westStr.trim()),
      south: Number(southStr.trim()),
      east: Number(eastStr.trim()),
      north: Number(northStr.trim()),
    }).catch((err: unknown) => {
      setErrorMessage(err instanceof Error ? err.message : String(err));
    });
  };

  return (
    <Dialog
      isOpen={isOpen && Boolean(image)}
      onClose={isSubmitting ? undefined : onClose}
      title="Edit Citra Orthomosaic"
      icon="edit"
      style={{ width: "480px", borderRadius: "16px" }}
    >
      {image && (
        <form onSubmit={handleSubmit}>
          <div className={Classes.DIALOG_BODY} style={{ padding: "16px 20px" }}>
            {errorMessage && (
              <Callout intent="danger" style={{ marginBottom: "16px", borderRadius: "10px" }}>
                {errorMessage}
              </Callout>
            )}
            <div
              style={{
                padding: "10px 14px",
                background: "#f8fafc",
                borderRadius: "10px",
                marginBottom: "14px",
                fontSize: "12px",
                color: "#64748b",
                display: "flex",
                justifyContent: "space-between",
              }}
            >
              <div>
                ID: <code style={{ fontSize: "11px", color: "#0f172a" }}>{image.id}</code>
              </div>
              <div>
                Diimpor:{" "}
                <strong style={{ color: "#0f172a" }}>
                  {image.imported_at ? new Date(image.imported_at).toLocaleString() : "-"}
                </strong>
              </div>
            </div>
            <div>
              <div
                style={{ fontSize: "12px", fontWeight: 600, color: "#64748b", marginBottom: "6px" }}
              >
                Nama Tampilan Citra <span style={{ color: "#ef4444" }}>*</span>
              </div>
              <input
                id="ortho-filename"
                type="text"
                placeholder="Contoh: Kariangau_Inspection_Area.jpg"
                value={filename}
                disabled={isSubmitting}
                onChange={(e) => setFilename(e.target.value)}
                style={{
                  width: "100%",
                  padding: "9px 12px",
                  fontSize: "13px",
                  background: "#f8fafc",
                  border: "none",
                  borderRadius: "10px",
                  outline: "none",
                  color: "#0f172a",
                  boxSizing: "border-box",
                }}
              />
            </div>
            <OrthoBoundingBoxInputs
              westStr={westStr}
              setWestStr={setWestStr}
              southStr={southStr}
              setSouthStr={setSouthStr}
              eastStr={eastStr}
              setEastStr={setEastStr}
              northStr={northStr}
              setNorthStr={setNorthStr}
              disabled={isSubmitting}
              onUseCurrentView={getCurrentMapBounds ? handleUseCurrentView : undefined}
            />
          </div>
          <div className={Classes.DIALOG_FOOTER}>
            <div className={Classes.DIALOG_FOOTER_ACTIONS}>
              <Button
                text="Batal"
                minimal
                disabled={isSubmitting}
                onClick={onClose}
                style={{
                  borderRadius: "999px",
                  background: "#f1f5f9",
                  color: "#475569",
                  fontWeight: 600,
                  padding: "6px 16px",
                }}
              />
              <Button
                minimal
                type="submit"
                icon="saved"
                text="Simpan Perubahan"
                loading={isSubmitting}
                style={{
                  borderRadius: "999px",
                  background: "#0f172a",
                  color: "#ffffff",
                  fontWeight: 600,
                  padding: "6px 18px",
                  boxShadow: "0 2px 8px rgba(15,23,42,0.15)",
                }}
              />
            </div>
          </div>
        </form>
      )}
    </Dialog>
  );
};
