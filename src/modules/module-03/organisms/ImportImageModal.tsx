import React, { useState, useMemo } from "react";
import type * as L from "leaflet";
import {
  Dialog,
  DialogBody,
  DialogFooter,
  Button,
  Callout,
  Intent,
  Tag,
  Checkbox,
  Spinner,
  Icon,
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
  const [west, setWest] = useState<string>("");
  const [south, setSouth] = useState<string>("");
  const [east, setEast] = useState<string>("");
  const [north, setNorth] = useState<string>("");
  const [imgDimensions, setImgDimensions] = useState<[number, number] | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  // ── State Tab B (GeoTIFF Presisi Tinggi) ──────────────────────────────────
  const [selectedTiffFile, setSelectedTiffFile] = useState<SelectedFileInfo | null>(null);
  const [customNameTiff, setCustomNameTiff] = useState<string>("");
  // Checkbox "Simpan file TIFF asli" default MATI (false) dengan peringatan disk
  const [saveOriginalTiff, setSaveOriginalTiff] = useState<boolean>(false);
  const [isInspectingTiff, setIsInspectingTiff] = useState<boolean>(false);
  const [tiffInspection, setTiffInspection] = useState<GeotiffInspectionResult | null>(null);
  const [tiffWest, setTiffWest] = useState<string>("");
  const [tiffSouth, setTiffSouth] = useState<string>("");
  const [tiffEast, setTiffEast] = useState<string>("");
  const [tiffNorth, setTiffNorth] = useState<string>("");
  const [isBboxEdited, setIsBboxEdited] = useState<boolean>(false);
  const [tiffError, setTiffError] = useState<string | null>(null);

  // ── State Job Asinkron ───────────────────────────────────────────────────
  const [activeJobId, setActiveJobId] = useState<string | null>(null);
  const [isStartingJob, setIsStartingJob] = useState(false);

  const parseCoord = (s: string): number | undefined => {
    const t = s.trim();
    if (t === "") return undefined;
    return Number(t);
  };

  const numWest = useMemo(() => parseCoord(west), [west]);
  const numSouth = useMemo(() => parseCoord(south), [south]);
  const numEast = useMemo(() => parseCoord(east), [east]);
  const numNorth = useMemo(() => parseCoord(north), [north]);

  const numTiffWest = useMemo(() => parseCoord(tiffWest), [tiffWest]);
  const numTiffSouth = useMemo(() => parseCoord(tiffSouth), [tiffSouth]);
  const numTiffEast = useMemo(() => parseCoord(tiffEast), [tiffEast]);
  const numTiffNorth = useMemo(() => parseCoord(tiffNorth), [tiffNorth]);

  const resetForm = () => {
    // Reset Tab A
    setSelectedFile(null);
    setCustomName("");
    setSaveOriginal(true);
    setUseManualBounds(false);
    setWest("");
    setSouth("");
    setEast("");
    setNorth("");
    setImgDimensions(null);
    setErrorMessage(null);

    // Reset Tab B
    setSelectedTiffFile(null);
    setCustomNameTiff("");
    setSaveOriginalTiff(false); // Tetap default false (mati)
    setIsInspectingTiff(false);
    setTiffInspection(null);
    setTiffWest("");
    setTiffSouth("");
    setTiffEast("");
    setTiffNorth("");
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
          setTiffWest(inspection.detected_bounds.west.toFixed(6));
          setTiffSouth(inspection.detected_bounds.south.toFixed(6));
          setTiffEast(inspection.detected_bounds.east.toFixed(6));
          setTiffNorth(inspection.detected_bounds.north.toFixed(6));
          setIsBboxEdited(false);
        } else {
          // Fallback input manual saat CrsTidakDidukung atau georeferensi hilang
          setTiffWest("");
          setTiffSouth("");
          setTiffEast("");
          setTiffNorth("");
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
      setTiffWest(tiffInspection.detected_bounds.west.toFixed(6));
      setTiffSouth(tiffInspection.detected_bounds.south.toFixed(6));
      setTiffEast(tiffInspection.detected_bounds.east.toFixed(6));
      setTiffNorth(tiffInspection.detected_bounds.north.toFixed(6));
      setIsBboxEdited(false);
    }
  };

  // ── Shortcut: Isi dari Viewport Peta ─────────────────────────────────────
  const handleFillFromMap = () => {
    if (!map) return;
    const b = map.getBounds();
    setWest(b.getWest().toFixed(6));
    setSouth(b.getSouth().toFixed(6));
    setEast(b.getEast().toFixed(6));
    setNorth(b.getNorth().toFixed(6));
    setUseManualBounds(true);
  };

  const handleFillTiffFromMap = () => {
    if (!map) return;
    const b = map.getBounds();
    setTiffWest(b.getWest().toFixed(6));
    setTiffSouth(b.getSouth().toFixed(6));
    setTiffEast(b.getEast().toFixed(6));
    setTiffNorth(b.getNorth().toFixed(6));
    setIsBboxEdited(true);
  };

  // ── Validasi Koordinat Tab A ─────────────────────────────────────────────
  const southError = useMemo(() => {
    if (!useManualBounds || south.trim() === "") return null;
    if (numSouth === undefined || isNaN(numSouth)) return "Wajib berupa angka desimal.";
    if (numSouth < -90 || numSouth > 90) return "Harus berada dalam rentang -90° s/d 90°.";
    if (numNorth !== undefined && !isNaN(numNorth) && numSouth >= numNorth) {
      return "Harus lebih kecil dari North (Lat Max).";
    }
    return null;
  }, [useManualBounds, south, numSouth, numNorth]);

  const northError = useMemo(() => {
    if (!useManualBounds || north.trim() === "") return null;
    if (numNorth === undefined || isNaN(numNorth)) return "Wajib berupa angka desimal.";
    if (numNorth < -90 || numNorth > 90) return "Harus berada dalam rentang -90° s/d 90°.";
    if (numSouth !== undefined && !isNaN(numSouth) && numNorth <= numSouth) {
      return "Harus lebih besar dari South (Lat Min).";
    }
    return null;
  }, [useManualBounds, north, numSouth, numNorth]);

  const westError = useMemo(() => {
    if (!useManualBounds || west.trim() === "") return null;
    if (numWest === undefined || isNaN(numWest)) return "Wajib berupa angka desimal.";
    if (numWest < -180 || numWest > 180) return "Harus berada dalam rentang -180° s/d 180°.";
    if (numEast !== undefined && !isNaN(numEast) && numWest >= numEast) {
      return "Harus lebih kecil dari East (Lon Max).";
    }
    return null;
  }, [useManualBounds, west, numWest, numEast]);

  const eastError = useMemo(() => {
    if (!useManualBounds || east.trim() === "") return null;
    if (numEast === undefined || isNaN(numEast)) return "Wajib berupa angka desimal.";
    if (numEast < -180 || numEast > 180) return "Harus berada dalam rentang -180° s/d 180°.";
    if (numWest !== undefined && !isNaN(numWest) && numEast <= numWest) {
      return "Harus lebih besar dari West (Lon Min).";
    }
    return null;
  }, [useManualBounds, east, numWest, numEast]);

  const aspectRatioWarning = useMemo(() => {
    if (
      !useManualBounds ||
      !imgDimensions ||
      numWest === undefined ||
      numSouth === undefined ||
      numEast === undefined ||
      numNorth === undefined ||
      isNaN(numWest) ||
      isNaN(numSouth) ||
      isNaN(numEast) ||
      isNaN(numNorth)
    ) {
      return null;
    }
    if (numSouth >= numNorth || numWest >= numEast) return null;

    const [imgW, imgH] = imgDimensions;
    if (imgW <= 0 || imgH <= 0) return null;
    const imgAspect = imgW / imgH;

    const latMidRad = (((numSouth + numNorth) / 2) * Math.PI) / 180;
    const cosLat = Math.cos(latMidRad);
    const deltaLon = numEast - numWest;
    const deltaLat = numNorth - numSouth;

    if (deltaLat <= 0 || deltaLon <= 0) return null;

    const bboxAspect = (deltaLon * cosLat) / deltaLat;
    const discrepancy = Math.abs(bboxAspect - imgAspect) / imgAspect;

    if (discrepancy > 0.05) {
      const pctDiff = (discrepancy * 100).toFixed(1);
      return `Peringatan: Rasio aspek batas koordinat (${bboxAspect.toFixed(2)}) berbeda ${pctDiff}% (>5%) dari rasio aspek citra (${imgAspect.toFixed(2)}). Citra mungkin tampak terdistorsi atau terenggang.`;
    }
    return null;
  }, [useManualBounds, imgDimensions, numWest, numSouth, numEast, numNorth]);

  const isFormValid = useMemo(() => {
    if (!selectedFile) return false;
    if (useManualBounds) {
      if (
        numWest === undefined ||
        numSouth === undefined ||
        numEast === undefined ||
        numNorth === undefined ||
        isNaN(numWest) ||
        isNaN(numSouth) ||
        isNaN(numEast) ||
        isNaN(numNorth)
      ) {
        return false;
      }
      return !southError && !northError && !westError && !eastError;
    }
    return true;
  }, [
    selectedFile,
    useManualBounds,
    numWest,
    numSouth,
    numEast,
    numNorth,
    southError,
    northError,
    westError,
    eastError,
  ]);

  // ── Validasi Koordinat Tab B ─────────────────────────────────────────────
  const tiffSouthError = useMemo(() => {
    if (tiffSouth.trim() === "") return "Wajib diisi.";
    if (numTiffSouth === undefined || isNaN(numTiffSouth)) return "Wajib berupa angka desimal.";
    if (numTiffSouth < -90 || numTiffSouth > 90) return "Harus berada dalam rentang -90° s/d 90°.";
    if (numTiffNorth !== undefined && !isNaN(numTiffNorth) && numTiffSouth >= numTiffNorth) {
      return "Harus lebih kecil dari North.";
    }
    return null;
  }, [tiffSouth, numTiffSouth, numTiffNorth]);

  const tiffNorthError = useMemo(() => {
    if (tiffNorth.trim() === "") return "Wajib diisi.";
    if (numTiffNorth === undefined || isNaN(numTiffNorth)) return "Wajib berupa angka desimal.";
    if (numTiffNorth < -90 || numTiffNorth > 90) return "Harus berada dalam rentang -90° s/d 90°.";
    if (numTiffSouth !== undefined && !isNaN(numTiffSouth) && numTiffNorth <= numTiffSouth) {
      return "Harus lebih besar dari South.";
    }
    return null;
  }, [tiffNorth, numTiffSouth, numTiffNorth]);

  const tiffWestError = useMemo(() => {
    if (tiffWest.trim() === "") return "Wajib diisi.";
    if (numTiffWest === undefined || isNaN(numTiffWest)) return "Wajib berupa angka desimal.";
    if (numTiffWest < -180 || numTiffWest > 180)
      return "Harus berada dalam rentang -180° s/d 180°.";
    if (numTiffEast !== undefined && !isNaN(numTiffEast) && numTiffWest >= numTiffEast) {
      return "Harus lebih kecil dari East.";
    }
    return null;
  }, [tiffWest, numTiffWest, numTiffEast]);

  const tiffEastError = useMemo(() => {
    if (tiffEast.trim() === "") return "Wajib diisi.";
    if (numTiffEast === undefined || isNaN(numTiffEast)) return "Wajib berupa angka desimal.";
    if (numTiffEast < -180 || numTiffEast > 180)
      return "Harus berada dalam rentang -180° s/d 180°.";
    if (numTiffWest !== undefined && !isNaN(numTiffWest) && numTiffEast <= numTiffWest) {
      return "Harus lebih besar dari West.";
    }
    return null;
  }, [tiffEast, numTiffWest, numTiffEast]);

  const isTiffFormValid = useMemo(() => {
    if (!selectedTiffFile || isInspectingTiff) return false;
    if (
      numTiffSouth === undefined ||
      numTiffNorth === undefined ||
      numTiffWest === undefined ||
      numTiffEast === undefined ||
      isNaN(numTiffSouth) ||
      isNaN(numTiffNorth) ||
      isNaN(numTiffWest) ||
      isNaN(numTiffEast)
    ) {
      return false;
    }
    return !tiffSouthError && !tiffNorthError && !tiffWestError && !tiffEastError;
  }, [
    selectedTiffFile,
    isInspectingTiff,
    numTiffSouth,
    numTiffNorth,
    numTiffWest,
    numTiffEast,
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
          numWest !== undefined &&
          numSouth !== undefined &&
          numEast !== undefined &&
          numNorth !== undefined
            ? { west: numWest, south: numSouth, east: numEast, north: numNorth }
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
          west: Number(numTiffWest),
          south: Number(numTiffSouth),
          east: Number(numTiffEast),
          north: Number(numTiffNorth),
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
        style={{ width: "580px", borderRadius: "16px" }}
        canOutsideClickClose={!isStartingJob}
      >
        <DialogBody>
          {/* Segmented Control Pill Bar */}
          <div
            style={{
              display: "flex",
              background: "#f1f5f9",
              padding: "3px",
              borderRadius: "10px",
              gap: "3px",
              marginBottom: "16px",
            }}
          >
            <button
              type="button"
              onClick={() => setActiveTab("A")}
              style={{
                flex: 1,
                padding: "8px 12px",
                fontSize: "12px",
                fontWeight: activeTab === "A" ? 600 : 500,
                color: activeTab === "A" ? "#0f172a" : "#64748b",
                background: activeTab === "A" ? "#ffffff" : "transparent",
                border: "none",
                borderRadius: "8px",
                boxShadow: activeTab === "A" ? "0 1px 3px rgba(0,0,0,0.08)" : "none",
                cursor: "pointer",
                transition: "all 0.15s ease",
              }}
            >
              Opsi A: Citra Ringan (JPG / PNG)
            </button>
            <button
              type="button"
              onClick={() => setActiveTab("B")}
              style={{
                flex: 1,
                padding: "8px 12px",
                fontSize: "12px",
                fontWeight: activeTab === "B" ? 600 : 500,
                color: activeTab === "B" ? "#0f172a" : "#64748b",
                background: activeTab === "B" ? "#ffffff" : "transparent",
                border: "none",
                borderRadius: "8px",
                boxShadow: activeTab === "B" ? "0 1px 3px rgba(0,0,0,0.08)" : "none",
                cursor: "pointer",
                transition: "all 0.15s ease",
              }}
            >
              Opsi B: GeoTIFF (.tif, .tiff)
            </button>
          </div>

          {activeTab === "A" && (
            <div
              style={{
                display: "flex",
                flexDirection: "column",
                gap: "14px",
              }}
            >
              {/* PILIH BERKAS */}
              <div>
                <div
                  style={{
                    fontSize: "12px",
                    fontWeight: 600,
                    color: "#64748b",
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
                      padding: "12px 14px",
                      background: "#f8fafc",
                      borderRadius: "12px",
                    }}
                  >
                    <div>
                      <div style={{ fontWeight: 600, fontSize: "13px", color: "#0f172a" }}>
                        {selectedFile.name}
                      </div>
                      <div
                        style={{
                          display: "flex",
                          gap: "8px",
                          marginTop: "4px",
                          alignItems: "center",
                        }}
                      >
                        <Tag minimal intent="primary" round style={{ fontSize: "10px" }}>
                          {selectedFile.ext.toUpperCase()}
                        </Tag>
                        {imgDimensions && (
                          <span style={{ fontSize: "11px", color: "#64748b" }}>
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
                  <button
                    type="button"
                    onClick={() => void handleSelectFile()}
                    style={{
                      width: "100%",
                      padding: "16px",
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "center",
                      gap: "8px",
                      background: "#f8fafc",
                      border: "1px dashed #cbd5e1",
                      borderRadius: "12px",
                      cursor: "pointer",
                      color: "#0f172a",
                      fontSize: "13px",
                      fontWeight: 600,
                      transition: "all 0.15s ease",
                    }}
                  >
                    <Icon icon="folder-open" size={16} color="#64748b" />
                    <span>Pilih Berkas Citra (JPG / PNG)...</span>
                  </button>
                )}
              </div>

              {/* NAMA LAYER KUSTOM */}
              <div>
                <div
                  style={{
                    fontSize: "12px",
                    fontWeight: 600,
                    color: "#64748b",
                    marginBottom: "6px",
                  }}
                >
                  Nama Layer
                </div>
                <input
                  type="text"
                  value={customName}
                  placeholder="Masukkan nama layer..."
                  onChange={(e) => setCustomName(e.target.value)}
                  disabled={isStartingJob}
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

              {/* CHECKBOX SIMPAN ORIGINAL */}
              <div
                style={{
                  background: "#f8fafc",
                  padding: "12px",
                  borderRadius: "12px",
                }}
              >
                <Checkbox
                  checked={saveOriginal}
                  onChange={(e) => setSaveOriginal(e.currentTarget.checked)}
                  label="Simpan berkas citra asli di penyimpanan aplikasi"
                  disabled={isStartingJob}
                  style={{ marginBottom: "2px", fontWeight: 600 }}
                />
                <div style={{ fontSize: "11px", color: "#64748b", marginLeft: "24px" }}>
                  {saveOriginal
                    ? "Berkas asli akan disimpan sehingga dapat di-render ulang dengan resolusi lebih tinggi di masa mendatang."
                    : "Perhatian: Tanpa file asli, citra tidak dapat di-render ulang dengan resolusi lebih tinggi di masa mendatang."}
                </div>
              </div>

              {/* DETEKSI KOORDINAT & BOUNDS MANUAL */}
              <div style={{ borderTop: "1px solid #f1f5f9", paddingTop: "12px" }}>
                <div
                  style={{
                    display: "flex",
                    justifyContent: "space-between",
                    alignItems: "center",
                    marginBottom: "8px",
                  }}
                >
                  <div style={{ fontSize: "12px", fontWeight: 600, color: "#64748b" }}>
                    2. Koordinat Georeferensi (WGS84)
                  </div>
                  {map && (
                    <button
                      type="button"
                      disabled={isStartingJob}
                      onClick={handleFillFromMap}
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
                      <span>Isi dari Viewport Peta</span>
                    </button>
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
                      color: "#64748b",
                      fontStyle: "italic",
                      background: "#f8fafc",
                      padding: "10px 12px",
                      borderRadius: "10px",
                    }}
                  >
                    Sistem akan otomatis mendeteksi berkas World File (.jgw, .pgw, .wld) atau
                    Sidecar JSON (.json) di folder yang sama dengan citra. Centang kotak di atas
                    jika ingin memasukkan koordinat manual.
                  </div>
                ) : (
                  <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "10px" }}>
                    <div>
                      <div
                        style={{
                          fontSize: "11px",
                          fontWeight: 600,
                          color: southError ? "#ef4444" : "#64748b",
                          marginBottom: "4px",
                        }}
                      >
                        South (Lat Min °)
                      </div>
                      <input
                        type="text"
                        inputMode="decimal"
                        placeholder="-1.225000"
                        value={south}
                        onChange={(e) => setSouth(e.target.value)}
                        disabled={isStartingJob}
                        style={{
                          width: "100%",
                          padding: "8px 12px",
                          fontSize: "12px",
                          background: "#f8fafc",
                          border: southError ? "1px solid #fca5a5" : "none",
                          borderRadius: "10px",
                          outline: "none",
                          color: "#0f172a",
                          boxSizing: "border-box",
                        }}
                      />
                      {southError && (
                        <div style={{ fontSize: "10px", color: "#ef4444", marginTop: "3px" }}>
                          {southError}
                        </div>
                      )}
                    </div>

                    <div>
                      <div
                        style={{
                          fontSize: "11px",
                          fontWeight: 600,
                          color: northError ? "#ef4444" : "#64748b",
                          marginBottom: "4px",
                        }}
                      >
                        North (Lat Max °)
                      </div>
                      <input
                        type="text"
                        inputMode="decimal"
                        placeholder="-1.218000"
                        value={north}
                        onChange={(e) => setNorth(e.target.value)}
                        disabled={isStartingJob}
                        style={{
                          width: "100%",
                          padding: "8px 12px",
                          fontSize: "12px",
                          background: "#f8fafc",
                          border: northError ? "1px solid #fca5a5" : "none",
                          borderRadius: "10px",
                          outline: "none",
                          color: "#0f172a",
                          boxSizing: "border-box",
                        }}
                      />
                      {northError && (
                        <div style={{ fontSize: "10px", color: "#ef4444", marginTop: "3px" }}>
                          {northError}
                        </div>
                      )}
                    </div>

                    <div>
                      <div
                        style={{
                          fontSize: "11px",
                          fontWeight: 600,
                          color: westError ? "#ef4444" : "#64748b",
                          marginBottom: "4px",
                        }}
                      >
                        West (Lon Min °)
                      </div>
                      <input
                        type="text"
                        inputMode="decimal"
                        placeholder="116.856000"
                        value={west}
                        onChange={(e) => setWest(e.target.value)}
                        disabled={isStartingJob}
                        style={{
                          width: "100%",
                          padding: "8px 12px",
                          fontSize: "12px",
                          background: "#f8fafc",
                          border: westError ? "1px solid #fca5a5" : "none",
                          borderRadius: "10px",
                          outline: "none",
                          color: "#0f172a",
                          boxSizing: "border-box",
                        }}
                      />
                      {westError && (
                        <div style={{ fontSize: "10px", color: "#ef4444", marginTop: "3px" }}>
                          {westError}
                        </div>
                      )}
                    </div>

                    <div>
                      <div
                        style={{
                          fontSize: "11px",
                          fontWeight: 600,
                          color: eastError ? "#ef4444" : "#64748b",
                          marginBottom: "4px",
                        }}
                      >
                        East (Lon Max °)
                      </div>
                      <input
                        type="text"
                        inputMode="decimal"
                        placeholder="116.865000"
                        value={east}
                        onChange={(e) => setEast(e.target.value)}
                        disabled={isStartingJob}
                        style={{
                          width: "100%",
                          padding: "8px 12px",
                          fontSize: "12px",
                          background: "#f8fafc",
                          border: eastError ? "1px solid #fca5a5" : "none",
                          borderRadius: "10px",
                          outline: "none",
                          color: "#0f172a",
                          boxSizing: "border-box",
                        }}
                      />
                      {eastError && (
                        <div style={{ fontSize: "10px", color: "#ef4444", marginTop: "3px" }}>
                          {eastError}
                        </div>
                      )}
                    </div>

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
          )}

          {activeTab === "B" && (
            <div
              style={{
                display: "flex",
                flexDirection: "column",
                gap: "14px",
              }}
            >
              {/* PILIH BERKAS GEOTIFF */}
              <div>
                <div
                  style={{
                    fontSize: "12px",
                    fontWeight: 600,
                    color: "#64748b",
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
                      background: "#f8fafc",
                      borderRadius: "12px",
                    }}
                  >
                    <Spinner size={20} intent="primary" />
                    <span style={{ fontSize: "12px", color: "#64748b" }}>
                      Membaca header GeoTIFF dan mengekstrak georeferensi...
                    </span>
                  </div>
                ) : selectedTiffFile ? (
                  <div
                    style={{
                      padding: "12px 14px",
                      background: "#f8fafc",
                      borderRadius: "12px",
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
                        <div style={{ fontWeight: 600, fontSize: "13px", color: "#0f172a" }}>
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
                          setTiffWest("");
                          setTiffSouth("");
                          setTiffEast("");
                          setTiffNorth("");
                          setIsBboxEdited(false);
                        }}
                        title="Ganti berkas"
                      />
                    </div>
                  </div>
                ) : (
                  <button
                    type="button"
                    onClick={() => void handleSelectTiffFile()}
                    style={{
                      width: "100%",
                      padding: "16px",
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "center",
                      gap: "8px",
                      background: "#f8fafc",
                      border: "1px dashed #cbd5e1",
                      borderRadius: "12px",
                      cursor: "pointer",
                      color: "#0f172a",
                      fontSize: "13px",
                      fontWeight: 600,
                      transition: "all 0.15s ease",
                    }}
                  >
                    <Icon icon="folder-open" size={16} color="#64748b" />
                    <span>Pilih Berkas GeoTIFF (.tif, .tiff, .geotiff)...</span>
                  </button>
                )}
              </div>

              {/* NAMA LAYER KUSTOM TAB B */}
              <div>
                <div
                  style={{
                    fontSize: "12px",
                    fontWeight: 600,
                    color: "#64748b",
                    marginBottom: "6px",
                  }}
                >
                  Nama Layer
                </div>
                <input
                  type="text"
                  value={customNameTiff}
                  placeholder="Masukkan nama layer..."
                  onChange={(e) => setCustomNameTiff(e.target.value)}
                  disabled={isStartingJob}
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

              {/* CHECKBOX "Simpan file TIFF asli" */}
              <div
                style={{
                  background: saveOriginalTiff ? "#fef3c7" : "#f8fafc",
                  padding: "12px",
                  borderRadius: "12px",
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
                    color: saveOriginalTiff ? "#92400e" : "#64748b",
                    marginLeft: "24px",
                    lineHeight: 1.4,
                  }}
                >
                  {saveOriginalTiff ? (
                    <span>
                      ⚠️ <strong>Peringatan Ruang Disk:</strong> Berkas TIFF asli berukuran penuh
                      (dapat mencapai puluhan hingga ratusan MB) akan disalin ke direktori
                      penyimpanan. Gunakan hanya jika Anda memerlukan re-ekspor atau analisis piksel
                      asli.
                    </span>
                  ) : (
                    <span>
                      <strong>Rekomendasi (Default Nonaktif):</strong> Berkas TIFF asli tidak
                      disimpan di storage aplikasi untuk menghemat ruang disk. Sistem hanya
                      menyimpan pratinjau display teroptimasi (≤ 10 MiB / ≤ 5 MiB) yang cukup untuk
                      navigasi visual peta.
                    </span>
                  )}
                </div>
              </div>

              {/* GEOREFERENSI, KOREKSI BBOX HASIL DETEKSI, & FALLBACK MANUAL */}
              {selectedTiffFile && !isInspectingTiff && (
                <div style={{ borderTop: "1px solid #f1f5f9", paddingTop: "12px" }}>
                  <div
                    style={{
                      display: "flex",
                      justifyContent: "space-between",
                      alignItems: "center",
                      marginBottom: "8px",
                    }}
                  >
                    <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
                      <span style={{ fontSize: "12px", fontWeight: 600, color: "#64748b" }}>
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
                        <button
                          type="button"
                          disabled={isStartingJob}
                          onClick={handleResetToDetectedBbox}
                          title="Kembalikan koordinat ke hasil ekstraksi GeoTIFF awal"
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
                          <Icon icon="reset" size={12} color="#0f172a" />
                          <span>Deteksi Awal</span>
                        </button>
                      )}
                      {map && (
                        <button
                          type="button"
                          disabled={isStartingJob}
                          onClick={handleFillTiffFromMap}
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
                          <span>Isi dari Viewport Peta</span>
                        </button>
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
                        color: "#64748b",
                        marginBottom: "8px",
                        lineHeight: 1.35,
                      }}
                    >
                      Koordinat di bawah ini diekstrak otomatis dari GeoTIFF. Anda dapat mengoreksi
                      atau menyesuaikan batas koordinat jika diperlukan.
                    </div>
                  )}

                  <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "10px" }}>
                    <div>
                      <div
                        style={{
                          fontSize: "11px",
                          fontWeight: 600,
                          color: tiffSouthError ? "#ef4444" : "#64748b",
                          marginBottom: "4px",
                        }}
                      >
                        South (Lat Min °)
                      </div>
                      <input
                        type="text"
                        inputMode="decimal"
                        placeholder="-1.225000"
                        value={tiffSouth}
                        onChange={(e) => {
                          setTiffSouth(e.target.value);
                          setIsBboxEdited(true);
                        }}
                        disabled={isStartingJob}
                        style={{
                          width: "100%",
                          padding: "8px 12px",
                          fontSize: "12px",
                          background: "#f8fafc",
                          border: tiffSouthError ? "1px solid #fca5a5" : "none",
                          borderRadius: "10px",
                          outline: "none",
                          color: "#0f172a",
                          boxSizing: "border-box",
                        }}
                      />
                      {tiffSouthError && (
                        <div style={{ fontSize: "10px", color: "#ef4444", marginTop: "3px" }}>
                          {tiffSouthError}
                        </div>
                      )}
                    </div>

                    <div>
                      <div
                        style={{
                          fontSize: "11px",
                          fontWeight: 600,
                          color: tiffNorthError ? "#ef4444" : "#64748b",
                          marginBottom: "4px",
                        }}
                      >
                        North (Lat Max °)
                      </div>
                      <input
                        type="text"
                        inputMode="decimal"
                        placeholder="-1.218000"
                        value={tiffNorth}
                        onChange={(e) => {
                          setTiffNorth(e.target.value);
                          setIsBboxEdited(true);
                        }}
                        disabled={isStartingJob}
                        style={{
                          width: "100%",
                          padding: "8px 12px",
                          fontSize: "12px",
                          background: "#f8fafc",
                          border: tiffNorthError ? "1px solid #fca5a5" : "none",
                          borderRadius: "10px",
                          outline: "none",
                          color: "#0f172a",
                          boxSizing: "border-box",
                        }}
                      />
                      {tiffNorthError && (
                        <div style={{ fontSize: "10px", color: "#ef4444", marginTop: "3px" }}>
                          {tiffNorthError}
                        </div>
                      )}
                    </div>

                    <div>
                      <div
                        style={{
                          fontSize: "11px",
                          fontWeight: 600,
                          color: tiffWestError ? "#ef4444" : "#64748b",
                          marginBottom: "4px",
                        }}
                      >
                        West (Lon Min °)
                      </div>
                      <input
                        type="text"
                        inputMode="decimal"
                        placeholder="116.856000"
                        value={tiffWest}
                        onChange={(e) => {
                          setTiffWest(e.target.value);
                          setIsBboxEdited(true);
                        }}
                        disabled={isStartingJob}
                        style={{
                          width: "100%",
                          padding: "8px 12px",
                          fontSize: "12px",
                          background: "#f8fafc",
                          border: tiffWestError ? "1px solid #fca5a5" : "none",
                          borderRadius: "10px",
                          outline: "none",
                          color: "#0f172a",
                          boxSizing: "border-box",
                        }}
                      />
                      {tiffWestError && (
                        <div style={{ fontSize: "10px", color: "#ef4444", marginTop: "3px" }}>
                          {tiffWestError}
                        </div>
                      )}
                    </div>

                    <div>
                      <div
                        style={{
                          fontSize: "11px",
                          fontWeight: 600,
                          color: tiffEastError ? "#ef4444" : "#64748b",
                          marginBottom: "4px",
                        }}
                      >
                        East (Lon Max °)
                      </div>
                      <input
                        type="text"
                        inputMode="decimal"
                        placeholder="116.865000"
                        value={tiffEast}
                        onChange={(e) => {
                          setTiffEast(e.target.value);
                          setIsBboxEdited(true);
                        }}
                        disabled={isStartingJob}
                        style={{
                          width: "100%",
                          padding: "8px 12px",
                          fontSize: "12px",
                          background: "#f8fafc",
                          border: tiffEastError ? "1px solid #fca5a5" : "none",
                          borderRadius: "10px",
                          outline: "none",
                          color: "#0f172a",
                          boxSizing: "border-box",
                        }}
                      />
                      {tiffEastError && (
                        <div style={{ fontSize: "10px", color: "#ef4444", marginTop: "3px" }}>
                          {tiffEastError}
                        </div>
                      )}
                    </div>
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
          )}
        </DialogBody>

        <DialogFooter
          actions={
            <div style={{ display: "flex", gap: "8px", justifyContent: "flex-end" }}>
              <Button
                text="Batal"
                minimal
                disabled={isStartingJob}
                onClick={handleClose}
                style={{
                  borderRadius: "999px",
                  background: "#f1f5f9",
                  color: "#475569",
                  fontWeight: 600,
                  padding: "6px 16px",
                  boxShadow: "none",
                }}
              />
              {activeTab === "A" && (
                <Button
                  minimal
                  icon="cloud-upload"
                  text={isStartingJob ? "Memulai..." : "Mulai Impor"}
                  disabled={!isFormValid || isStartingJob}
                  loading={isStartingJob}
                  onClick={() => void handleStartImport()}
                  style={{
                    borderRadius: "999px",
                    background: !isFormValid || isStartingJob ? "#f1f5f9" : "#0f172a",
                    color: !isFormValid || isStartingJob ? "#94a3b8" : "#ffffff",
                    fontWeight: 600,
                    padding: "6px 18px",
                    boxShadow:
                      !isFormValid || isStartingJob ? "none" : "0 2px 8px rgba(15,23,42,0.15)",
                  }}
                />
              )}
              {activeTab === "B" && (
                <Button
                  minimal
                  icon="cloud-upload"
                  text={isStartingJob ? "Memulai..." : "Mulai Impor GeoTIFF"}
                  disabled={!isTiffFormValid || isStartingJob || isInspectingTiff}
                  loading={isStartingJob}
                  onClick={() => void handleStartImportTiff()}
                  style={{
                    borderRadius: "999px",
                    background:
                      !isTiffFormValid || isStartingJob || isInspectingTiff ? "#f1f5f9" : "#0f172a",
                    color:
                      !isTiffFormValid || isStartingJob || isInspectingTiff ? "#94a3b8" : "#ffffff",
                    fontWeight: 600,
                    padding: "6px 18px",
                    boxShadow:
                      !isTiffFormValid || isStartingJob || isInspectingTiff
                        ? "none"
                        : "0 2px 8px rgba(15,23,42,0.15)",
                  }}
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
