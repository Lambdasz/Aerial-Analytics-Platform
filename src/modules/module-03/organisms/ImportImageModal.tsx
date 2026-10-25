import React, { useState, useMemo } from "react";
import type * as L from "leaflet";
import {
  Dialog,
  DialogBody,
  DialogFooter,
  Button,
  FormGroup,
  NumericInput,
  Callout,
  Intent,
  ProgressBar,
  Tag,
} from "@blueprintjs/core";
import { open } from "@tauri-apps/plugin-dialog";
import { importOrthoImage } from "../services/api";
import type { StoredImageMetadata } from "../types/storage";

export interface ImportImageModalProps {
  isOpen: boolean;
  onClose: () => void;
  onImportSuccess: (image: StoredImageMetadata) => void;
  map?: L.Map | null;
}

interface SelectedFileInfo {
  path: string;
  name: string;
  size: number;
  ext: string;
}

export const ImportImageModal: React.FC<ImportImageModalProps> = ({
  isOpen,
  onClose,
  onImportSuccess,
  map,
}) => {
  const [selectedFile, setSelectedFile] = useState<SelectedFileInfo | null>(null);
  const [west, setWest] = useState<number | undefined>(undefined);
  const [south, setSouth] = useState<number | undefined>(undefined);
  const [east, setEast] = useState<number | undefined>(undefined);
  const [north, setNorth] = useState<number | undefined>(undefined);
  const [imgDimensions, setImgDimensions] = useState<[number, number] | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const resetForm = () => {
    setSelectedFile(null);
    setWest(undefined);
    setSouth(undefined);
    setEast(undefined);
    setNorth(undefined);
    setImgDimensions(null);
    setIsSubmitting(false);
    setErrorMessage(null);
  };

  const handleClose = () => {
    if (!isSubmitting) {
      resetForm();
      onClose();
    }
  };

  // ── Handler Pemilihan Berkas ─────────────────────────────────────────────
  const handleSelectFile = async () => {
    setErrorMessage(null);
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        filters: [
          {
            name: "Citra Udara (JPG, PNG)",
            extensions: ["jpg", "jpeg", "png"],
          },
        ],
      });

      if (!selected || typeof selected !== "string") {
        return;
      }

      const rawPath = selected;
      const filename = rawPath.split(/[/\\]/).pop() || rawPath;
      const ext = filename.split(".").pop()?.toLowerCase() || "";

      if (ext !== "jpg" && ext !== "jpeg" && ext !== "png") {
        setErrorMessage("Format berkas tidak valid. Harap pilih berkas JPG atau PNG.");
        return;
      }

      // Deteksi dimensi via Image element browser (fast preview)
      const previewImg = new Image();
      previewImg.src = `asset://${rawPath}`;

      previewImg.onload = () => {
        setImgDimensions([previewImg.naturalWidth, previewImg.naturalHeight]);
      };

      // Simpan info berkas terpilih
      setSelectedFile({
        path: rawPath,
        name: filename,
        size: 0,
        ext,
      });
    } catch (err) {
      console.error("Gagal membuka file picker:", err);
      setErrorMessage("Gagal membuka dialog pemilihan berkas.");
    }
  };

  // ── Shortcut: Isi dari Viewport Peta ─────────────────────────────────────
  const handleFillFromMap = () => {
    if (!map) return;
    const b = map.getBounds();
    setWest(Number(b.getWest().toFixed(6)));
    setSouth(Number(b.getSouth().toFixed(6)));
    setEast(Number(b.getEast().toFixed(6)));
    setNorth(Number(b.getNorth().toFixed(6)));
  };

  // ── Validasi Per-Field ───────────────────────────────────────────────────
  const southError = useMemo(() => {
    if (south === undefined) return null;
    if (isNaN(south)) return "Wajib berupa angka desimal.";
    if (south < -90 || south > 90) return "Harus berada dalam rentang -90° s/d 90°.";
    if (north !== undefined && south >= north) return "Harus lebih kecil dari North (Lat Max).";
    return null;
  }, [south, north]);

  const northError = useMemo(() => {
    if (north === undefined) return null;
    if (isNaN(north)) return "Wajib berupa angka desimal.";
    if (north < -90 || north > 90) return "Harus berada dalam rentang -90° s/d 90°.";
    if (south !== undefined && north <= south) return "Harus lebih besar dari South (Lat Min).";
    return null;
  }, [south, north]);

  const westError = useMemo(() => {
    if (west === undefined) return null;
    if (isNaN(west)) return "Wajib berupa angka desimal.";
    if (west < -180 || west > 180) return "Harus berada dalam rentang -180° s/d 180°.";
    if (east !== undefined && west >= east) return "Harus lebih kecil dari East (Lon Max).";
    return null;
  }, [west, east]);

  const eastError = useMemo(() => {
    if (east === undefined) return null;
    if (isNaN(east)) return "Wajib berupa angka desimal.";
    if (east < -180 || east > 180) return "Harus berada dalam rentang -180° s/d 180°.";
    if (west !== undefined && east <= west) return "Harus lebih besar dari West (Lon Min).";
    return null;
  }, [west, east]);

  // Cek apakah seluruh form lengkap & valid
  const isFormValid = useMemo(() => {
    if (!selectedFile) return false;
    if (west === undefined || south === undefined || east === undefined || north === undefined) {
      return false;
    }
    return !southError && !northError && !westError && !eastError;
  }, [selectedFile, west, south, east, north, southError, northError, westError, eastError]);

  // ── Peringatan Rasio Aspek (Aspect Ratio Warning) ────────────────────────
  const aspectRatioWarning = useMemo(() => {
    if (
      !imgDimensions ||
      west === undefined ||
      south === undefined ||
      east === undefined ||
      north === undefined ||
      !isFormValid
    ) {
      return null;
    }

    const [pxW, pxH] = imgDimensions;
    if (pxW <= 0 || pxH <= 0) return null;
    const pxRatio = pxW / pxH;

    const latMidRad = (((south + north) / 2) * Math.PI) / 180;
    const deltaLonDeg = Math.abs(east - west);
    const deltaLatDeg = Math.abs(north - south);
    if (deltaLatDeg <= 0) return null;

    const geoRatio = (deltaLonDeg * Math.cos(latMidRad)) / deltaLatDeg;
    const diff = Math.abs(geoRatio / pxRatio - 1.0);

    if (diff > 0.35) {
      const pct = (diff * 100).toFixed(0);
      return `Peringatan: Proporsi koordinat area berbeda ~${pct}% dari rasio piksel gambar (${pxW}x${pxH}). Citra mungkin tampak terdistorsi pada peta.`;
    }

    return null;
  }, [imgDimensions, west, south, east, north, isFormValid]);

  // ── Eksekusi Submit Impor ─────────────────────────────────────────────────
  const handleSubmit = async () => {
    if (
      !isFormValid ||
      !selectedFile ||
      west === undefined ||
      south === undefined ||
      east === undefined ||
      north === undefined
    ) {
      return;
    }

    setIsSubmitting(true);
    setErrorMessage(null);

    try {
      const imported = await importOrthoImage({
        filePath: selectedFile.path,
        west,
        south,
        east,
        north,
      });

      window.dispatchEvent(new CustomEvent("ortho_image:saved", { detail: imported }));
      onImportSuccess(imported);
      resetForm();
      onClose();
    } catch (err: unknown) {
      console.error("Gagal mengimpor citra orthomosaic:", err);
      const msg =
        typeof err === "string" ? err : (err as Error)?.message || "Gagal mengimpor citra.";
      setErrorMessage(msg);
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <Dialog
      isOpen={isOpen}
      onClose={handleClose}
      title="Impor Citra Orthomosaic (JPG / PNG)"
      icon="import"
      style={{ width: "520px" }}
      canOutsideClickClose={!isSubmitting}
    >
      <DialogBody>
        <div style={{ display: "flex", flexDirection: "column", gap: "14px" }}>
          {/* BAGIAN 1: PEMILIHAN FILE */}
          <div>
            <div
              style={{
                fontSize: "12px",
                fontWeight: 600,
                color: "var(--color-slate)",
                marginBottom: "6px",
              }}
            >
              1. Berkas Citra Udara (Maks 50 MB, 16.384 px)
            </div>
            {selectedFile ? (
              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "space-between",
                  padding: "10px 14px",
                  background: "var(--color-gallery-white)",
                  border: "1px solid var(--color-hairline)",
                  borderRadius: "var(--radius-cards)",
                }}
              >
                <div>
                  <div style={{ fontWeight: 600, fontSize: "13px", color: "var(--color-ink)" }}>
                    {selectedFile.name}
                  </div>
                  <div style={{ display: "flex", gap: "8px", marginTop: "4px" }}>
                    <Tag minimal intent="primary" round style={{ fontSize: "10px" }}>
                      {selectedFile.ext.toUpperCase()}
                    </Tag>
                    {imgDimensions && (
                      <span style={{ fontSize: "11px", color: "var(--color-slate)" }}>
                        {imgDimensions[0]} × {imgDimensions[1]} px
                      </span>
                    )}
                  </div>
                </div>
                <Button
                  small
                  minimal
                  icon="cross"
                  disabled={isSubmitting}
                  onClick={() => setSelectedFile(null)}
                  title="Ganti berkas"
                />
              </div>
            ) : (
              <Button
                icon="folder-open"
                text="Pilih Berkas Citra (JPG / PNG)..."
                intent="primary"
                outlined
                fill
                onClick={() => void handleSelectFile()}
              />
            )}
          </div>

          {/* BAGIAN 2: KOORDINAT AREA (WGS84) */}
          <div style={{ borderTop: "1px solid var(--color-hairline)", paddingTop: "12px" }}>
            <div
              style={{
                display: "flex",
                justifyContent: "space-between",
                alignItems: "center",
                marginBottom: "8px",
              }}
            >
              <div style={{ fontSize: "12px", fontWeight: 600, color: "var(--color-slate)" }}>
                2. Koordinat Batas Area (WGS84 Derajat Desimal)
              </div>
              {map && (
                <Button
                  small
                  minimal
                  intent="primary"
                  icon="locate"
                  text="Gunakan Viewport Peta"
                  disabled={isSubmitting}
                  onClick={handleFillFromMap}
                />
              )}
            </div>

            <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "10px" }}>
              {/* SOUTH */}
              <FormGroup
                label="South (Lat Min °)"
                helperText={southError}
                intent={southError ? Intent.DANGER : Intent.NONE}
                style={{ marginBottom: 0 }}
              >
                <NumericInput
                  fill
                  placeholder="-1.225000"
                  value={south ?? ""}
                  onValueChange={(val) => setSouth(isNaN(val) ? undefined : val)}
                  stepSize={0.0001}
                  minorStepSize={0.00001}
                  intent={southError ? Intent.DANGER : Intent.NONE}
                  disabled={isSubmitting}
                />
              </FormGroup>

              {/* NORTH */}
              <FormGroup
                label="North (Lat Max °)"
                helperText={northError}
                intent={northError ? Intent.DANGER : Intent.NONE}
                style={{ marginBottom: 0 }}
              >
                <NumericInput
                  fill
                  placeholder="-1.218000"
                  value={north ?? ""}
                  onValueChange={(val) => setNorth(isNaN(val) ? undefined : val)}
                  stepSize={0.0001}
                  minorStepSize={0.00001}
                  intent={northError ? Intent.DANGER : Intent.NONE}
                  disabled={isSubmitting}
                />
              </FormGroup>

              {/* WEST */}
              <FormGroup
                label="West (Lon Min °)"
                helperText={westError}
                intent={westError ? Intent.DANGER : Intent.NONE}
                style={{ marginBottom: 0 }}
              >
                <NumericInput
                  fill
                  placeholder="116.856000"
                  value={west ?? ""}
                  onValueChange={(val) => setWest(isNaN(val) ? undefined : val)}
                  stepSize={0.0001}
                  minorStepSize={0.00001}
                  intent={westError ? Intent.DANGER : Intent.NONE}
                  disabled={isSubmitting}
                />
              </FormGroup>

              {/* EAST */}
              <FormGroup
                label="East (Lon Max °)"
                helperText={eastError}
                intent={eastError ? Intent.DANGER : Intent.NONE}
                style={{ marginBottom: 0 }}
              >
                <NumericInput
                  fill
                  placeholder="116.865000"
                  value={east ?? ""}
                  onValueChange={(val) => setEast(isNaN(val) ? undefined : val)}
                  stepSize={0.0001}
                  minorStepSize={0.00001}
                  intent={eastError ? Intent.DANGER : Intent.NONE}
                  disabled={isSubmitting}
                />
              </FormGroup>
            </div>
          </div>

          {/* PERINGATAN RASIO ASPEK */}
          {aspectRatioWarning && (
            <Callout
              intent="warning"
              icon="warning-sign"
              style={{ fontSize: "12px", padding: "8px 12px" }}
            >
              {aspectRatioWarning}
            </Callout>
          )}

          {/* PESAN ERROR SUBMIT */}
          {errorMessage && (
            <Callout intent="danger" icon="error" style={{ fontSize: "12px", padding: "8px 12px" }}>
              {errorMessage}
            </Callout>
          )}

          {isSubmitting && <ProgressBar intent="primary" />}
        </div>
      </DialogBody>

      <DialogFooter
        actions={
          <div style={{ display: "flex", gap: "8px", justifyContent: "flex-end" }}>
            <Button text="Batal" minimal disabled={isSubmitting} onClick={handleClose} />
            <Button
              intent="primary"
              icon="cloud-upload"
              text={isSubmitting ? "Mengimpor..." : "Impor Citra"}
              disabled={!isFormValid || isSubmitting}
              loading={isSubmitting}
              onClick={() => void handleSubmit()}
            />
          </div>
        }
      />
    </Dialog>
  );
};
