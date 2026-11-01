import React, { useState } from "react";
import { Button, Callout, Classes, Dialog, Icon } from "@blueprintjs/core";
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
  const [southStr, setSouthStr] = useState<string>(
    initialBounds ? String(initialBounds[0][0]) : "",
  );
  const [westStr, setWestStr] = useState<string>(initialBounds ? String(initialBounds[0][1]) : "");
  const [northStr, setNorthStr] = useState<string>(
    initialBounds ? String(initialBounds[1][0]) : "",
  );
  const [eastStr, setEastStr] = useState<string>(initialBounds ? String(initialBounds[1][1]) : "");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

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
    if (!filename.trim()) {
      return "Nama citra wajib diisi.";
    }
    const parseNum = (s: string): number | undefined => {
      const t = s.trim();
      if (t === "") return undefined;
      return Number(t);
    };
    const west = parseNum(westStr);
    const south = parseNum(southStr);
    const east = parseNum(eastStr);
    const north = parseNum(northStr);

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
      return "Seluruh 4 koordinat bounding box (West, South, East, North) wajib diisi angka valid.";
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
      west: Number(westStr.trim()),
      south: Number(southStr.trim()),
      east: Number(eastStr.trim()),
      north: Number(northStr.trim()),
    }).catch((err: unknown) => {
      const msg = err instanceof Error ? err.message : String(err);
      setErrorMessage(msg);
    });
  };

  return (
    <form onSubmit={handleSubmit}>
      <div className={Classes.DIALOG_BODY} style={{ padding: "16px 20px" }}>
        {errorMessage && (
          <Callout intent="danger" style={{ marginBottom: "16px", borderRadius: "10px" }}>
            {errorMessage}
          </Callout>
        )}

        {/* Field Statis / Immutable */}
        <div
          style={{
            padding: "12px 14px",
            background: "#f8fafc",
            borderRadius: "12px",
            marginBottom: "16px",
            fontSize: "12px",
            color: "#64748b",
            display: "flex",
            justifyContent: "space-between",
          }}
        >
          <div>
            <span style={{ color: "#64748b" }}>ID:</span>{" "}
            <code style={{ fontSize: "11px", color: "#0f172a" }}>{image.id}</code>
          </div>
          <div>
            <span style={{ color: "#64748b" }}>Diimpor:</span>{" "}
            <strong style={{ color: "#0f172a" }}>
              {image.imported_at ? new Date(image.imported_at).toLocaleString() : "-"}
            </strong>
          </div>
        </div>

        <div>
          <div
            style={{
              fontSize: "12px",
              fontWeight: 600,
              color: "#64748b",
              marginBottom: "6px",
            }}
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
          <div style={{ fontSize: "11px", color: "#64748b", marginTop: "4px" }}>
            Nama identifikasi citra pada galeri dan daftar layer peta
          </div>
        </div>

        <div style={{ marginTop: "16px" }}>
          <div
            style={{
              display: "flex",
              justifyContent: "space-between",
              alignItems: "center",
              marginBottom: "8px",
            }}
          >
            <div style={{ fontSize: "12px", fontWeight: 600, color: "#64748b" }}>
              Koordinat Bounding Box (WGS84)
            </div>
            {getCurrentMapBounds && (
              <button
                type="button"
                disabled={isSubmitting}
                onClick={handleUseCurrentView}
                style={{
                  padding: "4px 10px",
                  fontSize: "11px",
                  fontWeight: 600,
                  borderRadius: "999px",
                  border: "none",
                  background: "#f1f5f9",
                  color: "#0f172a",
                  cursor: "pointer",
                  display: "flex",
                  alignItems: "center",
                  gap: "4px",
                }}
              >
                <Icon icon="locate" size={12} color="#0f172a" />
                <span>Gunakan Area Peta Saat Ini</span>
              </button>
            )}
          </div>

          <div
            style={{
              display: "grid",
              gridTemplateColumns: "1fr 1fr",
              gap: "10px",
            }}
          >
            <div>
              <div
                style={{
                  fontSize: "11px",
                  fontWeight: 600,
                  color: "#64748b",
                  marginBottom: "4px",
                }}
              >
                West (Bujur Barat) [-180..180]
              </div>
              <input
                id="bbox-west"
                type="text"
                inputMode="decimal"
                value={westStr}
                disabled={isSubmitting}
                onChange={(e) => setWestStr(e.target.value)}
                style={{
                  width: "100%",
                  padding: "8px 12px",
                  fontSize: "12px",
                  background: "#f8fafc",
                  border: "none",
                  borderRadius: "10px",
                  outline: "none",
                  color: "#0f172a",
                  boxSizing: "border-box",
                }}
              />
            </div>

            <div>
              <div
                style={{
                  fontSize: "11px",
                  fontWeight: 600,
                  color: "#64748b",
                  marginBottom: "4px",
                }}
              >
                South (Lintang Selatan) [-90..90]
              </div>
              <input
                id="bbox-south"
                type="text"
                inputMode="decimal"
                value={southStr}
                disabled={isSubmitting}
                onChange={(e) => setSouthStr(e.target.value)}
                style={{
                  width: "100%",
                  padding: "8px 12px",
                  fontSize: "12px",
                  background: "#f8fafc",
                  border: "none",
                  borderRadius: "10px",
                  outline: "none",
                  color: "#0f172a",
                  boxSizing: "border-box",
                }}
              />
            </div>

            <div>
              <div
                style={{
                  fontSize: "11px",
                  fontWeight: 600,
                  color: "#64748b",
                  marginBottom: "4px",
                }}
              >
                East (Bujur Timur) [-180..180]
              </div>
              <input
                id="bbox-east"
                type="text"
                inputMode="decimal"
                value={eastStr}
                disabled={isSubmitting}
                onChange={(e) => setEastStr(e.target.value)}
                style={{
                  width: "100%",
                  padding: "8px 12px",
                  fontSize: "12px",
                  background: "#f8fafc",
                  border: "none",
                  borderRadius: "10px",
                  outline: "none",
                  color: "#0f172a",
                  boxSizing: "border-box",
                }}
              />
            </div>

            <div>
              <div
                style={{
                  fontSize: "11px",
                  fontWeight: 600,
                  color: "#64748b",
                  marginBottom: "4px",
                }}
              >
                North (Lintang Utara) [-90..90]
              </div>
              <input
                id="bbox-north"
                type="text"
                inputMode="decimal"
                value={northStr}
                disabled={isSubmitting}
                onChange={(e) => setNorthStr(e.target.value)}
                style={{
                  width: "100%",
                  padding: "8px 12px",
                  fontSize: "12px",
                  background: "#f8fafc",
                  border: "none",
                  borderRadius: "10px",
                  outline: "none",
                  color: "#0f172a",
                  boxSizing: "border-box",
                }}
              />
            </div>
          </div>
        </div>
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
              boxShadow: "none",
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
      style={{ width: "480px", borderRadius: "16px" }}
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
