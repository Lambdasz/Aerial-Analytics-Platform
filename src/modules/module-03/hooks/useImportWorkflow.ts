import { useState, useMemo } from "react";
import type * as L from "leaflet";
import { startImportOrthoJob, getOrthoImage, inspectGeotiffHeader } from "../services/api";
import type { StoredImageMetadata, GeotiffInspectionResult } from "../types/storage";
import type { FileSelectionResult } from "../molecules/import/ImportDropzone";
import { showToast } from "../lib/toaster";

export interface UseImportWorkflowOptions {
  map?: L.Map | null;
  onImportSuccess: (image: StoredImageMetadata) => void;
  onClose: () => void;
}

export function useImportWorkflow({ map, onImportSuccess, onClose }: UseImportWorkflowOptions) {
  const [activeTab, setActiveTab] = useState<"A" | "B">("A");

  // ── State Tab A (Citra Ringan JPG / PNG) ──────────────────────────────────
  const [selectedFile, setSelectedFile] = useState<FileSelectionResult | null>(null);
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
  const [selectedTiffFile, setSelectedTiffFile] = useState<FileSelectionResult | null>(null);
  const [customNameTiff, setCustomNameTiff] = useState<string>("");
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

    setSelectedTiffFile(null);
    setCustomNameTiff("");
    setSaveOriginalTiff(false);
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

  const handleSelectFile = (file: FileSelectionResult) => {
    setErrorMessage(null);
    setSelectedFile(file);

    if (!customName) {
      const baseName = file.name.replace(/\.[^/.]+$/, "");
      setCustomName(baseName);
    }

    const previewImg = new Image();
    previewImg.src = `asset://${file.path}`;
    previewImg.onload = () => {
      setImgDimensions([previewImg.naturalWidth, previewImg.naturalHeight]);
    };
  };

  const handleRemoveFile = () => {
    setSelectedFile(null);
    setImgDimensions(null);
  };

  const handleSelectTiffFile = async (file: FileSelectionResult) => {
    setTiffError(null);
    setSelectedTiffFile(file);

    if (!customNameTiff) {
      const baseName = file.name.replace(/\.[^/.]+$/, "");
      setCustomNameTiff(baseName);
    }

    setIsInspectingTiff(true);
    try {
      const inspection = await inspectGeotiffHeader(file.path);
      setTiffInspection(inspection);

      if (inspection.is_crs_supported && inspection.detected_bounds) {
        setTiffWest(inspection.detected_bounds.west.toFixed(6));
        setTiffSouth(inspection.detected_bounds.south.toFixed(6));
        setTiffEast(inspection.detected_bounds.east.toFixed(6));
        setTiffNorth(inspection.detected_bounds.north.toFixed(6));
        setIsBboxEdited(false);
      } else {
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
  };

  const handleRemoveTiffFile = () => {
    setSelectedTiffFile(null);
    setTiffInspection(null);
    setTiffWest("");
    setTiffSouth("");
    setTiffEast("");
    setTiffNorth("");
    setIsBboxEdited(false);
  };

  const handleResetToDetectedBbox = () => {
    if (tiffInspection?.detected_bounds) {
      setTiffWest(tiffInspection.detected_bounds.west.toFixed(6));
      setTiffSouth(tiffInspection.detected_bounds.south.toFixed(6));
      setTiffEast(tiffInspection.detected_bounds.east.toFixed(6));
      setTiffNorth(tiffInspection.detected_bounds.north.toFixed(6));
      setIsBboxEdited(false);
    }
  };

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

  // ── Penanganan Selesai Job ────────────────────────────────────────────────
  const handleJobSuccess = async (resultImageId: string) => {
    try {
      const meta = await getOrthoImage(resultImageId);
      window.dispatchEvent(new CustomEvent("ortho_image:saved", { detail: meta }));
      onImportSuccess(meta);

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

  return {
    activeTab,
    setActiveTab,
    // Tab A state & actions
    selectedFile,
    handleSelectFile,
    handleRemoveFile,
    imgDimensions,
    customName,
    setCustomName,
    saveOriginal,
    setSaveOriginal,
    useManualBounds,
    setUseManualBounds,
    south,
    setSouth,
    north,
    setNorth,
    west,
    setWest,
    east,
    setEast,
    southError,
    northError,
    westError,
    eastError,
    aspectRatioWarning,
    handleFillFromMap,
    errorMessage,
    setErrorMessage,
    isFormValid,
    handleStartImport,
    // Tab B state & actions
    selectedTiffFile,
    handleSelectTiffFile,
    handleRemoveTiffFile,
    isInspectingTiff,
    tiffInspection,
    customNameTiff,
    setCustomNameTiff,
    saveOriginalTiff,
    setSaveOriginalTiff,
    tiffWest,
    setTiffWest,
    tiffSouth,
    setTiffSouth,
    tiffEast,
    setTiffEast,
    tiffNorth,
    setTiffNorth,
    tiffSouthError,
    tiffNorthError,
    tiffWestError,
    tiffEastError,
    isBboxEdited,
    handleResetToDetectedBbox,
    handleFillTiffFromMap,
    tiffError,
    setTiffError,
    isTiffFormValid,
    handleStartImportTiff,
    // Async job & common
    activeJobId,
    setActiveJobId,
    isStartingJob,
    handleJobSuccess,
    handleClose,
  };
}
