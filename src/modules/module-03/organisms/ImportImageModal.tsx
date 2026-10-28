import React, { useState, useMemo } from "react";
import type * as L from "leaflet";
import {
  Dialog,
  DialogBody,
  DialogFooter,
  Button,
  FormGroup,
  NumericInput,
  InputGroup,
  Callout,
  Intent,
  Tag,
  Tabs,
  Tab,
  Checkbox,
  Spinner,
} from "@blueprintjs/core";
import { open } from "@tauri-apps/plugin-dialog";
import { startImportOrthoJob, getOrthoImage, inspectGeotiffHeader } from "../services/api";
import type { StoredImageMetadata, GeotiffInspectionResult } from "../types/storage";
import { ImportProgressModal } from "./ImportProgressModal";
import { showToast } from "../lib/toaster";

export interface ImportImageModalProps {
  isOpen: boolean;
  onClose: () => void;
  onImportSuccess: (image: StoredImageMetadata) => void;
  map?: L.Map | null;
}

interface SelectedFileInfo {
  path: string;
  name: string;
  ext: string;
}

export const ImportImageModal: React.FC<ImportImageModalProps> = ({
  isOpen,
  onClose,
  onImportSuccess,
  map,
}) => {
  const [activeTab, setActiveTab] = useState<"A" | "B">("A");

  // ── State Tab A (Citra Ringan JPG / PNG) ──────────────────────────────────
  const [selectedFile, setSelectedFile] = useState<SelectedFileInfo | null>(null);
  const [customName, setCustomName] = useState<string>("");
  const [saveOriginal, setSaveOriginal] = useState<boolean>(true);
  const [useManualBounds, setUseManualBounds] = useState<boolean>(false);
  const [west, setWest] = useState<number | undefined>(undefined);
  const [south, setSouth] = useState<number | undefined>(undefined);
  const [east, setEast] = useState<number | undefined>(undefined);
  const [north, setNorth] = useState<number | undefined>(undefined);
  const [imgDimensions, setImgDimensions] = useState<[number, number] | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  // ── State Tab B (GeoTIFF Presisi Tinggi) ──────────────────────────────────
  const [selectedTiffFile, setSelectedTiffFile] = useState<SelectedFileInfo | null>(null);
  const [customNameTiff, setCustomNameTiff] = useState<string>("");
  // Checkbox "Simpan file TIFF asli" default MATI (false) dengan peringatan disk
  const [saveOriginalTiff, setSaveOriginalTiff] = useState<boolean>(false);
  const [isInspectingTiff, setIsInspectingTiff] = useState<boolean>(false);
  const [tiffInspection, setTiffInspection] = useState<GeotiffInspectionResult | null>(null);
  const [tiffWest, setTiffWest] = useState<number | undefined>(undefined);
  const [tiffSouth, setTiffSouth] = useState<number | undefined>(undefined);
  const [tiffEast, setTiffEast] = useState<number | undefined>(undefined);
  const [tiffNorth, setTiffNorth] = useState<number | undefined>(undefined);
  const [isBboxEdited, setIsBboxEdited] = useState<boolean>(false);
  const [tiffError, setTiffError] = useState<string | null>(null);

  // ── State Job Asinkron ───────────────────────────────────────────────────
  const [activeJobId, setActiveJobId] = useState<string | null>(null);
  const [isStartingJob, setIsStartingJob] = useState(false);

  const resetForm = () => {
    // Reset Tab A
    setSelectedFile(null);
    setCustomName("");
    setSaveOriginal(true);
    setUseManualBounds(false);
    setWest(undefined);
    setSouth(undefined);
    setEast(undefined);
    setNorth(undefined);
    setImgDimensions(null);
    setErrorMessage(null);

    // Reset Tab B
    setSelectedTiffFile(null);
    setCustomNameTiff("");
    setSaveOriginalTiff(false); // Tetap default false (mati)
    setIsInspectingTiff(false);
    setTiffInspection(null);
    setTiffWest(undefined);
    setTiffSouth(undefined);
    setTiffEast(undefined);
    setTiffNorth(undefined);
    setIsBboxEdited(false);
    setTiffError(null);

    setIsStartingJob(false);
  };

  const handleClose = () => {
    if (!isStartingJob) {
      resetForm();
      onClose();
    }
  };

  // ── Handler Pemilihan Berkas Tab A ───────────────────────────────────────
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

      const previewImg = new Image();
      previewImg.src = `asset://${rawPath}`;
      previewImg.onload = () => {
        setImgDimensions([previewImg.naturalWidth, previewImg.naturalHeight]);
      };

      setSelectedFile({
        path: rawPath,
        name: filename,
        ext,
      });

      if (!customName) {
        const baseName = filename.replace(/\.[^/.]+$/, "");
        setCustomName(baseName);
      }
    } catch (err) {
      console.error("Gagal membuka file picker Tab A:", err);
      setErrorMessage("Gagal membuka dialog pemilihan berkas.");
    }
  };

  // ── Handler Pemilihan Berkas Tab B (GeoTIFF) ─────────────────────────────
  const handleSelectTiffFile = async () => {
    setTiffError(null);
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        filters: [
          {
            name: "Citra GeoTIFF (TIF, TIFF, GEOTIFF)",
            extensions: ["tif", "tiff", "geotiff"],
          },
        ],
      });

      if (!selected || typeof selected !== "string") {
        return;
      }

      const rawPath = selected;
      const filename = rawPath.split(/[/\\]/).pop() || rawPath;
      const ext = filename.split(".").pop()?.toLowerCase() || "";

      if (ext !== "tif" && ext !== "tiff" && ext !== "geotiff") {
        setTiffError(
          "Format berkas tidak valid. Harap pilih berkas GeoTIFF (.tif, .tiff, .geotiff).",
        );
        return;
      }

      setSelectedTiffFile({
        path: rawPath,
        name: filename,
        ext,
      });

      if (!customNameTiff) {
        const baseName = filename.replace(/\.[^/.]+$/, "");
        setCustomNameTiff(baseName);
      }

      // Jalankan inspeksi header GeoTIFF secara asinkron
      setIsInspectingTiff(true);
      try {
        const inspection = await inspectGeotiffHeader(rawPath);
        setTiffInspection(inspection);

        if (inspection.is_crs_supported && inspection.detected_bounds) {
          setTiffWest(Number(inspection.detected_bounds.west.toFixed(6)));
          setTiffSouth(Number(inspection.detected_bounds.south.toFixed(6)));
          setTiffEast(Number(inspection.detected_bounds.east.toFixed(6)));
          setTiffNorth(Number(inspection.detected_bounds.north.toFixed(6)));
          setIsBboxEdited(false);
        } else {
          // Fallback input manual saat CrsTidakDidukung atau georeferensi hilang
          setTiffWest(undefined);
          setTiffSouth(undefined);
          setTiffEast(undefined);
          setTiffNorth(undefined);
          setIsBboxEdited(false);
        }
      } catch (err: unknown) {
        const msg =
          typeof err === "string"
            ? err
            : (err as Error)?.message || "Gagal memeriksa header GeoTIFF.";
        setTiffError(msg);
        setTiffInspection(null);
      } finally {
        setIsInspectingTiff(false);
      }
    } catch (err) {
      console.error("Gagal membuka file picker GeoTIFF:", err);
      setTiffError("Gagal membuka dialog pemilihan berkas GeoTIFF.");
    }
  };

  // ── Kembalikan ke Deteksi Bounding Box Awal (Tab B) ───────────────────────
  const handleResetToDetectedBbox = () => {
    if (tiffInspection?.detected_bounds) {
      setTiffWest(Number(tiffInspection.detected_bounds.west.toFixed(6)));
      setTiffSouth(Number(tiffInspection.detected_bounds.south.toFixed(6)));
      setTiffEast(Number(tiffInspection.detected_bounds.east.toFixed(6)));
      setTiffNorth(Number(tiffInspection.detected_bounds.north.toFixed(6)));
      setIsBboxEdited(false);
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
    setUseManualBounds(true);
  };

  const handleFillTiffFromMap = () => {
    if (!map) return;
    const b = map.getBounds();
    setTiffWest(Number(b.getWest().toFixed(6)));
    setTiffSouth(Number(b.getSouth().toFixed(6)));
    setTiffEast(Number(b.getEast().toFixed(6)));
    setTiffNorth(Number(b.getNorth().toFixed(6)));
    setIsBboxEdited(true);
  };

  // ── Validasi Koordinat Tab A ─────────────────────────────────────────────
  const southError = useMemo(() => {
    if (!useManualBounds || south === undefined) return null;
    if (isNaN(south)) return "Wajib berupa angka desimal.";
    if (south < -90 || south > 90) return "Harus berada dalam rentang -90° s/d 90°.";
    if (north !== undefined && south >= north) return "Harus lebih kecil dari North (Lat Max).";
    return null;
  }, [useManualBounds, south, north]);

  const northError = useMemo(() => {
    if (!useManualBounds || north === undefined) return null;
    if (isNaN(north)) return "Wajib berupa angka desimal.";
    if (north < -90 || north > 90) return "Harus berada dalam rentang -90° s/d 90°.";
    if (south !== undefined && north <= south) return "Harus lebih besar dari South (Lat Min).";
    return null;
  }, [useManualBounds, south, north]);

  const westError = useMemo(() => {
    if (!useManualBounds || west === undefined) return null;
    if (isNaN(west)) return "Wajib berupa angka desimal.";
    if (west < -180 || west > 180) return "Harus berada dalam rentang -180° s/d 180°.";
    if (east !== undefined && west >= east) return "Harus lebih kecil dari East (Lon Max).";
    return null;
  }, [useManualBounds, west, east]);

  const eastError = useMemo(() => {
    if (!useManualBounds || east === undefined) return null;
    if (isNaN(east)) return "Wajib berupa angka desimal.";
    if (east < -180 || east > 180) return "Harus berada dalam rentang -180° s/d 180°.";
    if (west !== undefined && east <= west) return "Harus lebih besar dari West (Lon Min).";
    return null;
  }, [useManualBounds, west, east]);

  const aspectRatioWarning = useMemo(() => {
    if (
      !useManualBounds ||
      !imgDimensions ||
      west === undefined ||
      south === undefined ||
      east === undefined ||
      north === undefined
    ) {
      return null;
    }
    if (south >= north || west >= east) return null;

    const [imgW, imgH] = imgDimensions;
    if (imgW <= 0 || imgH <= 0) return null;
    const imgAspect = imgW / imgH;

    const latMidRad = (((south + north) / 2) * Math.PI) / 180;
    const cosLat = Math.cos(latMidRad);
    const deltaLon = east - west;
    const deltaLat = north - south;

    if (deltaLat <= 0 || deltaLon <= 0) return null;

    const bboxAspect = (deltaLon * cosLat) / deltaLat;
    const discrepancy = Math.abs(bboxAspect - imgAspect) / imgAspect;

    if (discrepancy > 0.05) {
      const pctDiff = (discrepancy * 100).toFixed(1);
      return `Peringatan: Rasio aspek batas koordinat (${bboxAspect.toFixed(2)}) berbeda ${pctDiff}% (>5%) dari rasio aspek citra (${imgAspect.toFixed(2)}). Citra mungkin tampak terdistorsi atau terenggang.`;
    }
    return null;
  }, [useManualBounds, imgDimensions, west, south, east, north]);

  const isFormValid = useMemo(() => {
    if (!selectedFile) return false;
    if (useManualBounds) {
      if (west === undefined || south === undefined || east === undefined || north === undefined) {
        return false;
      }
      return !southError && !northError && !westError && !eastError;
    }
    return true;
  }, [
    selectedFile,
    useManualBounds,
    west,
    south,
    east,
    north,
    southError,
    northError,
    westError,
    eastError,
  ]);

  // ── Validasi Koordinat Tab B ─────────────────────────────────────────────
  const tiffSouthError = useMemo(() => {
    if (tiffSouth === undefined) return "Wajib diisi.";
    if (isNaN(tiffSouth)) return "Wajib berupa angka desimal.";
    if (tiffSouth < -90 || tiffSouth > 90) return "Harus berada dalam rentang -90° s/d 90°.";
    if (tiffNorth !== undefined && tiffSouth >= tiffNorth) return "Harus lebih kecil dari North.";
    return null;
  }, [tiffSouth, tiffNorth]);

  const tiffNorthError = useMemo(() => {
    if (tiffNorth === undefined) return "Wajib diisi.";
    if (isNaN(tiffNorth)) return "Wajib berupa angka desimal.";
    if (tiffNorth < -90 || tiffNorth > 90) return "Harus berada dalam rentang -90° s/d 90°.";
    if (tiffSouth !== undefined && tiffNorth <= tiffSouth) return "Harus lebih besar dari South.";
    return null;
  }, [tiffSouth, tiffNorth]);

  const tiffWestError = useMemo(() => {
    if (tiffWest === undefined) return "Wajib diisi.";
    if (isNaN(tiffWest)) return "Wajib berupa angka desimal.";
    if (tiffWest < -180 || tiffWest > 180) return "Harus berada dalam rentang -180° s/d 180°.";
    if (tiffEast !== undefined && tiffWest >= tiffEast) return "Harus lebih kecil dari East.";
    return null;
  }, [tiffWest, tiffEast]);

  const tiffEastError = useMemo(() => {
    if (tiffEast === undefined) return "Wajib diisi.";
    if (isNaN(tiffEast)) return "Wajib berupa angka desimal.";
    if (tiffEast < -180 || tiffEast > 180) return "Harus berada dalam rentang -180° s/d 180°.";
    if (tiffWest !== undefined && tiffEast <= tiffWest) return "Harus lebih besar dari West.";
    return null;
  }, [tiffWest, tiffEast]);

  const isTiffFormValid = useMemo(() => {
    if (!selectedTiffFile || isInspectingTiff) return false;
    if (
      tiffSouth === undefined ||
      tiffNorth === undefined ||
      tiffWest === undefined ||
      tiffEast === undefined
    ) {
      return false;
    }
    return !tiffSouthError && !tiffNorthError && !tiffWestError && !tiffEastError;
  }, [
    selectedTiffFile,
    isInspectingTiff,
    tiffSouth,
    tiffNorth,
    tiffWest,
    tiffEast,
    tiffSouthError,
    tiffNorthError,
    tiffWestError,
    tiffEastError,
  ]);

  // ── Eksekusi Mulai Impor Asinkron Tab A ───────────────────────────────────
  const handleStartImport = async () => {
    if (!isFormValid || !selectedFile) return;

    setIsStartingJob(true);
    setErrorMessage(null);

    try {
      const jobId = await startImportOrthoJob({
        mode: "A",
        filePath: selectedFile.path,
        saveOriginal,
        manualBounds:
          useManualBounds &&
          west !== undefined &&
          south !== undefined &&
          east !== undefined &&
          north !== undefined
            ? { west, south, east, north }
            : undefined,
        customFilename: customName.trim() || undefined,
      });

      setActiveJobId(jobId);
    } catch (err: unknown) {
      console.error("Gagal memulai job impor Tab A:", err);
      const msg =
        typeof err === "string" ? err : (err as Error)?.message || "Gagal memulai job impor.";
      setErrorMessage(msg);
    } finally {
      setIsStartingJob(false);
    }
  };

  // ── Eksekusi Mulai Impor Asinkron Tab B (GeoTIFF) ─────────────────────────
  const handleStartImportTiff = async () => {
    if (!isTiffFormValid || !selectedTiffFile) return;

    setIsStartingJob(true);
    setTiffError(null);

    try {
      const jobId = await startImportOrthoJob({
        mode: "B",
        filePath: selectedTiffFile.path,
        saveOriginal: saveOriginalTiff,
        manualBounds: {
          west: Number(tiffWest),
          south: Number(tiffSouth),
          east: Number(tiffEast),
          north: Number(tiffNorth),
        },
        customFilename: customNameTiff.trim() || undefined,
      });

      setActiveJobId(jobId);
    } catch (err: unknown) {
      console.error("Gagal memulai job impor GeoTIFF Tab B:", err);
      const msg =
        typeof err === "string"
          ? err
          : (err as Error)?.message || "Gagal memulai job impor GeoTIFF.";
      setTiffError(msg);
    } finally {
      setIsStartingJob(false);
    }
  };

  // ── Penanganan Selesai Job & Banner Peringatan Resolusi <2.048 px ─────────
  const handleJobSuccess = async (resultImageId: string) => {
    try {
      const meta = await getOrthoImage(resultImageId);
      window.dispatchEvent(new CustomEvent("ortho_image:saved", { detail: meta }));
      onImportSuccess(meta);

      // Banner peringatan bila resolusi display dibatasi < 2.048 px
      if (meta.display_dimensions) {
        const [w, h] = meta.display_dimensions;
        if (Math.max(w, h) < 2048) {
          void showToast(
            `Peringatan: Resolusi display dibatasi ke ${w}×${h} px (< 2.048 px) untuk mematuhi batas ukuran memori (≤ 10 MiB). Detail pada zoom tinggi mungkin berkurang.`,
            "warning",
          );
        }
      }
    } catch (e) {
      console.error("Gagal mengambil metadata citra hasil impor:", e);
    }
    setActiveJobId(null);
    resetForm();
    onClose();
  };

  return (
    <>
      <Dialog
        isOpen={isOpen && !activeJobId}
        onClose={handleClose}
        title="Impor Citra Orthomosaic"
        icon="import"
        style={{ width: "580px" }}
        canOutsideClickClose={!isStartingJob}
      >
        <DialogBody>
          <Tabs
            id="ortho-import-tabs"
            selectedTabId={activeTab}
            onChange={(id) => setActiveTab(id as "A" | "B")}
          >
            {/* ══════════════════════════════════════════════════════════════════
                TAB A: CITRA RINGAN (JPG / PNG)
               ══════════════════════════════════════════════════════════════════ */}
            <Tab
              id="A"
              title="Opsi A: Citra Ringan (JPG / PNG)"
              panel={
                <div
                  style={{
                    display: "flex",
                    flexDirection: "column",
                    gap: "14px",
                    marginTop: "12px",
                  }}
                >
                  {/* PILIH BERKAS */}
                  <div>
                    <div
                      style={{
                        fontSize: "12px",
                        fontWeight: 600,
                        color: "#5c7080",
                        marginBottom: "6px",
                      }}
                    >
                      1. Berkas Citra (Maks 50 MB, IDCT Downscaling Otomatis)
                    </div>
                    {selectedFile ? (
                      <div
                        style={{
                          display: "flex",
                          alignItems: "center",
                          justifyContent: "space-between",
                          padding: "10px 14px",
                          background: "#f5f8fa",
                          border: "1px solid #d8e1e8",
                          borderRadius: "4px",
                        }}
                      >
                        <div>
                          <div style={{ fontWeight: 600, fontSize: "13px", color: "#182026" }}>
                            {selectedFile.name}
                          </div>
                          <div style={{ display: "flex", gap: "8px", marginTop: "4px" }}>
                            <Tag minimal intent="primary" round style={{ fontSize: "10px" }}>
                              {selectedFile.ext.toUpperCase()}
                            </Tag>
                            {imgDimensions && (
                              <span style={{ fontSize: "11px", color: "#5c7080" }}>
                                {imgDimensions[0]} × {imgDimensions[1]} px
                              </span>
                            )}
                          </div>
                        </div>
                        <Button
                          small
                          minimal
                          icon="cross"
                          disabled={isStartingJob}
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

                  {/* NAMA LAYER KUSTOM */}
                  <FormGroup label="Nama Layer" style={{ marginBottom: 0 }}>
                    <InputGroup
                      value={customName}
                      placeholder="Masukkan nama layer..."
                      onChange={(e) => setCustomName(e.target.value)}
                      disabled={isStartingJob}
                    />
                  </FormGroup>

                  {/* CHECKBOX SIMPAN ORIGINAL */}
                  <div
                    style={{
                      background: "#f5f8fa",
                      padding: "10px 12px",
                      borderRadius: "4px",
                      border: "1px solid #d8e1e8",
                    }}
                  >
                    <Checkbox
                      checked={saveOriginal}
                      onChange={(e) => setSaveOriginal(e.currentTarget.checked)}
                      label="Simpan berkas citra asli di penyimpanan aplikasi"
                      disabled={isStartingJob}
                      style={{ marginBottom: "2px", fontWeight: 600 }}
                    />
                    <div style={{ fontSize: "11px", color: "#5c7080", marginLeft: "24px" }}>
                      {saveOriginal
                        ? "Berkas asli akan disimpan sehingga dapat di-render ulang dengan resolusi lebih tinggi di masa mendatang."
                        : "Perhatian: Tanpa file asli, citra tidak dapat di-render ulang dengan resolusi lebih tinggi di masa mendatang."}
                    </div>
                  </div>

                  {/* DETEKSI KOORDINAT & BOUNDS MANUAL */}
                  <div style={{ borderTop: "1px solid #d8e1e8", paddingTop: "12px" }}>
                    <div
                      style={{
                        display: "flex",
                        justifyContent: "space-between",
                        alignItems: "center",
                        marginBottom: "8px",
                      }}
                    >
                      <div style={{ fontSize: "12px", fontWeight: 600, color: "#5c7080" }}>
                        2. Koordinat Georeferensi (WGS84)
                      </div>
                      {map && (
                        <Button
                          type="button"
                          small
                          minimal
                          intent="primary"
                          icon="locate"
                          text="Isi dari Viewport Peta"
                          disabled={isStartingJob}
                          onClick={handleFillFromMap}
                        />
                      )}
                    </div>

                    <Checkbox
                      checked={useManualBounds}
                      onChange={(e) => setUseManualBounds(e.currentTarget.checked)}
                      label="Masukkan batas koordinat WGS84 secara manual"
                      disabled={isStartingJob}
                      style={{ fontSize: "12px", marginBottom: useManualBounds ? "10px" : "4px" }}
                    />

                    {!useManualBounds ? (
                      <div
                        style={{
                          fontSize: "11px",
                          color: "#5c7080",
                          fontStyle: "italic",
                          background: "#f5f8fa",
                          padding: "8px 10px",
                          borderRadius: "4px",
                        }}
                      >
                        Sistem akan otomatis mendeteksi berkas World File (.jgw, .pgw, .wld) atau
                        Sidecar JSON (.json) di folder yang sama dengan citra. Centang kotak di atas
                        jika ingin memasukkan koordinat manual.
                      </div>
                    ) : (
                      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "10px" }}>
                        <FormGroup
                          label="South (Lat Min °)"
                          helperText={southError}
                          intent={southError ? Intent.DANGER : Intent.NONE}
                        >
                          <NumericInput
                            fill
                            placeholder="-1.225000"
                            value={south ?? ""}
                            onValueChange={(val) => setSouth(isNaN(val) ? undefined : val)}
                            stepSize={0.0001}
                            minorStepSize={0.00001}
                            disabled={isStartingJob}
                          />
                        </FormGroup>

                        <FormGroup
                          label="North (Lat Max °)"
                          helperText={northError}
                          intent={northError ? Intent.DANGER : Intent.NONE}
                        >
                          <NumericInput
                            fill
                            placeholder="-1.218000"
                            value={north ?? ""}
                            onValueChange={(val) => setNorth(isNaN(val) ? undefined : val)}
                            stepSize={0.0001}
                            minorStepSize={0.00001}
                            disabled={isStartingJob}
                          />
                        </FormGroup>

                        <FormGroup
                          label="West (Lon Min °)"
                          helperText={westError}
                          intent={westError ? Intent.DANGER : Intent.NONE}
                        >
                          <NumericInput
                            fill
                            placeholder="116.856000"
                            value={west ?? ""}
                            onValueChange={(val) => setWest(isNaN(val) ? undefined : val)}
                            stepSize={0.0001}
                            minorStepSize={0.00001}
                            disabled={isStartingJob}
                          />
                        </FormGroup>

                        <FormGroup
                          label="East (Lon Max °)"
                          helperText={eastError}
                          intent={eastError ? Intent.DANGER : Intent.NONE}
                        >
                          <NumericInput
                            fill
                            placeholder="116.865000"
                            value={east ?? ""}
                            onValueChange={(val) => setEast(isNaN(val) ? undefined : val)}
                            stepSize={0.0001}
                            minorStepSize={0.00001}
                            disabled={isStartingJob}
                          />
                        </FormGroup>

                        {aspectRatioWarning && (
                          <div style={{ gridColumn: "1 / -1", marginTop: "2px" }}>
                            <Callout
                              intent={Intent.WARNING}
                              icon="warning-sign"
                              style={{ fontSize: "11px", padding: "8px 12px" }}
                            >
                              {aspectRatioWarning}
                            </Callout>
                          </div>
                        )}
                      </div>
                    )}
                  </div>

                  {errorMessage && (
                    <Callout
                      intent={Intent.DANGER}
                      icon="error"
                      style={{ fontSize: "12px", padding: "8px 12px" }}
                    >
                      {errorMessage}
                    </Callout>
                  )}
                </div>
              }
            />

            {/* ══════════════════════════════════════════════════════════════════
                TAB B: GEOTIFF PRESISI TINGGI (STREAMING & REPROYEKSI PURE RUST)
               ══════════════════════════════════════════════════════════════════ */}
            <Tab
              id="B"
              title="Opsi B: GeoTIFF (.tif, .tiff)"
              panel={
                <div
                  style={{
                    display: "flex",
                    flexDirection: "column",
                    gap: "14px",
                    marginTop: "12px",
                  }}
                >
                  {/* PILIH BERKAS GEOTIFF */}
                  <div>
                    <div
                      style={{
                        fontSize: "12px",
                        fontWeight: 600,
                        color: "#5c7080",
                        marginBottom: "6px",
                      }}
                    >
                      1. Berkas GeoTIFF (Hingga 250 MB / 500 Megapiksel)
                    </div>

                    {isInspectingTiff ? (
                      <div
                        style={{
                          display: "flex",
                          alignItems: "center",
                          gap: "12px",
                          padding: "14px",
                          background: "#f5f8fa",
                          border: "1px solid #d8e1e8",
                          borderRadius: "4px",
                        }}
                      >
                        <Spinner size={20} intent="primary" />
                        <span style={{ fontSize: "12px", color: "#5c7080" }}>
                          Membaca header GeoTIFF dan mengekstrak georeferensi...
                        </span>
                      </div>
                    ) : selectedTiffFile ? (
                      <div
                        style={{
                          padding: "10px 14px",
                          background: "#f5f8fa",
                          border: "1px solid #d8e1e8",
                          borderRadius: "4px",
                        }}
                      >
                        <div
                          style={{
                            display: "flex",
                            alignItems: "center",
                            justifyContent: "space-between",
                          }}
                        >
                          <div>
                            <div style={{ fontWeight: 600, fontSize: "13px", color: "#182026" }}>
                              {selectedTiffFile.name}
                            </div>
                            <div
                              style={{
                                display: "flex",
                                gap: "6px",
                                marginTop: "6px",
                                flexWrap: "wrap",
                              }}
                            >
                              <Tag minimal intent="primary" round style={{ fontSize: "10px" }}>
                                GEOTIFF
                              </Tag>
                              {tiffInspection && (
                                <>
                                  <Tag minimal intent="none" round style={{ fontSize: "10px" }}>
                                    {tiffInspection.width} × {tiffInspection.height} px
                                  </Tag>
                                  <Tag minimal intent="none" round style={{ fontSize: "10px" }}>
                                    {(tiffInspection.file_size_bytes / (1024 * 1024)).toFixed(1)} MB
                                  </Tag>
                                  <Tag minimal intent="none" round style={{ fontSize: "10px" }}>
                                    {tiffInspection.bit_depth}-bit
                                  </Tag>
                                  <Tag
                                    minimal
                                    intent={tiffInspection.has_alpha ? "warning" : "none"}
                                    round
                                    style={{ fontSize: "10px" }}
                                  >
                                    {tiffInspection.has_alpha ? "RGBA (Alpha)" : "RGB"}
                                  </Tag>
                                  <Tag minimal intent="none" round style={{ fontSize: "10px" }}>
                                    {tiffInspection.compression}
                                  </Tag>
                                </>
                              )}
                            </div>
                          </div>
                          <Button
                            small
                            minimal
                            icon="cross"
                            disabled={isStartingJob}
                            onClick={() => {
                              setSelectedTiffFile(null);
                              setTiffInspection(null);
                              setTiffWest(undefined);
                              setTiffSouth(undefined);
                              setTiffEast(undefined);
                              setTiffNorth(undefined);
                              setIsBboxEdited(false);
                            }}
                            title="Ganti berkas"
                          />
                        </div>
                      </div>
                    ) : (
                      <Button
                        icon="folder-open"
                        text="Pilih Berkas GeoTIFF (.tif, .tiff, .geotiff)..."
                        intent="primary"
                        outlined
                        fill
                        onClick={() => void handleSelectTiffFile()}
                      />
                    )}
                  </div>

                  {/* NAMA LAYER KUSTOM TAB B */}
                  <FormGroup label="Nama Layer" style={{ marginBottom: 0 }}>
                    <InputGroup
                      value={customNameTiff}
                      placeholder="Masukkan nama layer..."
                      onChange={(e) => setCustomNameTiff(e.target.value)}
                      disabled={isStartingJob}
                    />
                  </FormGroup>

                  {/* CHECKBOX "Simpan file TIFF asli" (DEFAULT MATI DENGAN PERINGATAN DISK) */}
                  <div
                    style={{
                      background: saveOriginalTiff ? "#fef8e7" : "#f5f8fa",
                      padding: "10px 12px",
                      borderRadius: "4px",
                      border: saveOriginalTiff ? "1px solid #fae1a0" : "1px solid #d8e1e8",
                    }}
                  >
                    <Checkbox
                      checked={saveOriginalTiff}
                      onChange={(e) => setSaveOriginalTiff(e.currentTarget.checked)}
                      label="Simpan file TIFF asli di penyimpanan aplikasi"
                      disabled={isStartingJob}
                      style={{ marginBottom: "2px", fontWeight: 600 }}
                    />
                    <div
                      style={{
                        fontSize: "11px",
                        color: saveOriginalTiff ? "#7f5f00" : "#5c7080",
                        marginLeft: "24px",
                        lineHeight: 1.4,
                      }}
                    >
                      {saveOriginalTiff ? (
                        <span>
                          ⚠️ <strong>Peringatan Ruang Disk:</strong> Berkas TIFF asli berukuran
                          penuh (dapat mencapai puluhan hingga ratusan MB) akan disalin ke direktori
                          penyimpanan. Gunakan hanya jika Anda memerlukan re-ekspor atau analisis
                          piksel asli.
                        </span>
                      ) : (
                        <span>
                          <strong>Rekomendasi (Default Nonaktif):</strong> Berkas TIFF asli tidak
                          disimpan di storage aplikasi untuk menghemat ruang disk. Sistem hanya
                          menyimpan pratinjau display teroptimasi (≤ 10 MiB / ≤ 5 MiB) yang cukup
                          untuk navigasi visual peta.
                        </span>
                      )}
                    </div>
                  </div>

                  {/* GEOREFERENSI, KOREKSI BBOX HASIL DETEKSI, & FALLBACK MANUAL */}
                  {selectedTiffFile && !isInspectingTiff && (
                    <div style={{ borderTop: "1px solid #d8e1e8", paddingTop: "12px" }}>
                      <div
                        style={{
                          display: "flex",
                          justifyContent: "space-between",
                          alignItems: "center",
                          marginBottom: "8px",
                        }}
                      >
                        <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
                          <span style={{ fontSize: "12px", fontWeight: 600, color: "#5c7080" }}>
                            2. Batas Koordinat WGS84
                          </span>
                          {tiffInspection?.is_crs_supported && (
                            <Tag minimal intent="success" round style={{ fontSize: "10px" }}>
                              {tiffInspection.crs_name || "CRS Terdeteksi"}
                            </Tag>
                          )}
                          {isBboxEdited && (
                            <Tag minimal intent="warning" round style={{ fontSize: "10px" }}>
                              Telah Disesuaikan
                            </Tag>
                          )}
                        </div>

                        <div style={{ display: "flex", gap: "6px" }}>
                          {isBboxEdited && tiffInspection?.detected_bounds && (
                            <Button
                              type="button"
                              small
                              minimal
                              intent="none"
                              icon="reset"
                              text="Deteksi Awal"
                              disabled={isStartingJob}
                              onClick={handleResetToDetectedBbox}
                              title="Kembalikan koordinat ke hasil ekstraksi GeoTIFF awal"
                            />
                          )}
                          {map && (
                            <Button
                              type="button"
                              small
                              minimal
                              intent="primary"
                              icon="locate"
                              text="Isi dari Viewport Peta"
                              disabled={isStartingJob}
                              onClick={handleFillTiffFromMap}
                            />
                          )}
                        </div>
                      </div>

                      {/* FALLBACK INPUT MANUAL SAAT CRS TIDAK DIDUKUNG / GEOREFERENSI HILANG */}
                      {tiffInspection && !tiffInspection.is_crs_supported && (
                        <div style={{ marginBottom: "10px" }}>
                          <Callout
                            intent={Intent.WARNING}
                            icon="warning-sign"
                            title="Sistem Koordinat Citra Belum Didukung Otomatis"
                            style={{ fontSize: "11px", padding: "8px 12px" }}
                          >
                            {tiffInspection.error_message ||
                              "Sistem koordinat citra belum didukung otomatis oleh converter pure Rust."}{" "}
                            Silakan masukkan batas koordinat WGS84 secara manual di bawah ini (atau
                            gunakan tombol <strong>Isi dari Viewport Peta</strong>) agar citra dapat
                            ditampilkan pada posisi yang tepat.
                          </Callout>
                        </div>
                      )}

                      {/* KOREKSI BBOX HASIL DETEKSI */}
                      {tiffInspection?.is_crs_supported && !isBboxEdited && (
                        <div
                          style={{
                            fontSize: "11px",
                            color: "#5c7080",
                            marginBottom: "8px",
                            lineHeight: 1.35,
                          }}
                        >
                          Koordinat di bawah ini diekstrak otomatis dari GeoTIFF. Anda dapat
                          mengoreksi atau menyesuaikan batas koordinat jika diperlukan.
                        </div>
                      )}

                      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "10px" }}>
                        <FormGroup
                          label="South (Lat Min °)"
                          helperText={tiffSouthError}
                          intent={tiffSouthError ? Intent.DANGER : Intent.NONE}
                        >
                          <NumericInput
                            fill
                            placeholder="-1.225000"
                            value={tiffSouth ?? ""}
                            onValueChange={(val) => {
                              setTiffSouth(isNaN(val) ? undefined : val);
                              setIsBboxEdited(true);
                            }}
                            stepSize={0.0001}
                            minorStepSize={0.00001}
                            disabled={isStartingJob}
                          />
                        </FormGroup>

                        <FormGroup
                          label="North (Lat Max °)"
                          helperText={tiffNorthError}
                          intent={tiffNorthError ? Intent.DANGER : Intent.NONE}
                        >
                          <NumericInput
                            fill
                            placeholder="-1.218000"
                            value={tiffNorth ?? ""}
                            onValueChange={(val) => {
                              setTiffNorth(isNaN(val) ? undefined : val);
                              setIsBboxEdited(true);
                            }}
                            stepSize={0.0001}
                            minorStepSize={0.00001}
                            disabled={isStartingJob}
                          />
                        </FormGroup>

                        <FormGroup
                          label="West (Lon Min °)"
                          helperText={tiffWestError}
                          intent={tiffWestError ? Intent.DANGER : Intent.NONE}
                        >
                          <NumericInput
                            fill
                            placeholder="116.856000"
                            value={tiffWest ?? ""}
                            onValueChange={(val) => {
                              setTiffWest(isNaN(val) ? undefined : val);
                              setIsBboxEdited(true);
                            }}
                            stepSize={0.0001}
                            minorStepSize={0.00001}
                            disabled={isStartingJob}
                          />
                        </FormGroup>

                        <FormGroup
                          label="East (Lon Max °)"
                          helperText={tiffEastError}
                          intent={tiffEastError ? Intent.DANGER : Intent.NONE}
                        >
                          <NumericInput
                            fill
                            placeholder="116.865000"
                            value={tiffEast ?? ""}
                            onValueChange={(val) => {
                              setTiffEast(isNaN(val) ? undefined : val);
                              setIsBboxEdited(true);
                            }}
                            stepSize={0.0001}
                            minorStepSize={0.00001}
                            disabled={isStartingJob}
                          />
                        </FormGroup>
                      </div>
                    </div>
                  )}

                  {tiffError && (
                    <Callout
                      intent={Intent.DANGER}
                      icon="error"
                      style={{ fontSize: "12px", padding: "8px 12px" }}
                    >
                      {tiffError}
                    </Callout>
                  )}
                </div>
              }
            />
          </Tabs>
        </DialogBody>

        <DialogFooter
          actions={
            <div style={{ display: "flex", gap: "8px", justifyContent: "flex-end" }}>
              <Button text="Batal" minimal disabled={isStartingJob} onClick={handleClose} />
              {activeTab === "A" && (
                <Button
                  intent={Intent.PRIMARY}
                  icon="cloud-upload"
                  text={isStartingJob ? "Memulai..." : "Mulai Impor"}
                  disabled={!isFormValid || isStartingJob}
                  loading={isStartingJob}
                  onClick={() => void handleStartImport()}
                />
              )}
              {activeTab === "B" && (
                <Button
                  intent={Intent.PRIMARY}
                  icon="cloud-upload"
                  text={isStartingJob ? "Memulai..." : "Mulai Impor GeoTIFF"}
                  disabled={!isTiffFormValid || isStartingJob || isInspectingTiff}
                  loading={isStartingJob}
                  onClick={() => void handleStartImportTiff()}
                />
              )}
            </div>
          }
        />
      </Dialog>

      {/* MODAL PROGRES & STATUS JOB */}
      <ImportProgressModal
        isOpen={Boolean(activeJobId)}
        jobId={activeJobId}
        onClose={() => setActiveJobId(null)}
        onSuccess={(id) => {
          void handleJobSuccess(id);
        }}
      />
    </>
  );
};
