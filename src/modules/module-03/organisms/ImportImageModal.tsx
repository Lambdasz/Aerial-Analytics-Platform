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
} from "@blueprintjs/core";
import { open } from "@tauri-apps/plugin-dialog";
import { startImportOrthoJob, getOrthoImage } from "../services/api";
import type { StoredImageMetadata } from "../types/storage";
import { ImportProgressModal } from "./ImportProgressModal";

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
  const [selectedFile, setSelectedFile] = useState<SelectedFileInfo | null>(null);
  const [customName, setCustomName] = useState<string>("");
  const [saveOriginal, setSaveOriginal] = useState<boolean>(true);

  // Manual Coordinates (opsional bila ada world file / sidecar JSON)
  const [useManualBounds, setUseManualBounds] = useState<boolean>(false);
  const [west, setWest] = useState<number | undefined>(undefined);
  const [south, setSouth] = useState<number | undefined>(undefined);
  const [east, setEast] = useState<number | undefined>(undefined);
  const [north, setNorth] = useState<number | undefined>(undefined);
  const [imgDimensions, setImgDimensions] = useState<[number, number] | null>(null);

  const [activeJobId, setActiveJobId] = useState<string | null>(null);
  const [isStartingJob, setIsStartingJob] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const resetForm = () => {
    setSelectedFile(null);
    setCustomName("");
    setSaveOriginal(true);
    setUseManualBounds(false);
    setWest(undefined);
    setSouth(undefined);
    setEast(undefined);
    setNorth(undefined);
    setImgDimensions(null);
    setIsStartingJob(false);
    setErrorMessage(null);
  };

  const handleClose = () => {
    if (!isStartingJob) {
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
    setUseManualBounds(true);
  };

  // ── Validasi Koordinat Manual ────────────────────────────────────────────
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

  // ── Eksekusi Mulai Impor Asinkron ─────────────────────────────────────────
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
      console.error("Gagal memulai job impor:", err);
      const msg =
        typeof err === "string" ? err : (err as Error)?.message || "Gagal memulai job impor.";
      setErrorMessage(msg);
    } finally {
      setIsStartingJob(false);
    }
  };

  const handleJobSuccess = async (resultImageId: string) => {
    try {
      const meta = await getOrthoImage(resultImageId);
      window.dispatchEvent(new CustomEvent("ortho_image:saved", { detail: meta }));
      onImportSuccess(meta);
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
        style={{ width: "560px" }}
        canOutsideClickClose={!isStartingJob}
      >
        <DialogBody>
          <Tabs
            id="ortho-import-tabs"
            selectedTabId={activeTab}
            onChange={(id) => setActiveTab(id as "A" | "B")}
          >
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

            <Tab
              id="B"
              title="Opsi B: GeoTIFF (Fase 3)"
              panel={
                <div style={{ marginTop: "16px" }}>
                  <Callout
                    intent={Intent.PRIMARY}
                    icon="info-sign"
                    title="Opsi B: GeoTIFF Presisi Tinggi (Mendatang)"
                  >
                    Fitur impor GeoTIFF presisi tinggi (hingga 250 MB / 500 Megapiksel) dengan
                    streaming tiled reader, resolusi 16-bit adaptif, penanganan alpha/nodata, dan
                    konversi pure Rust akan aktif pada <strong>Fase 3</strong>.
                  </Callout>
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
