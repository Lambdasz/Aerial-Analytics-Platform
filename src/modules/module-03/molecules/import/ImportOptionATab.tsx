import React from "react";
import { Checkbox, Callout, Intent, Icon } from "@blueprintjs/core";
import { ImportDropzone, type FileSelectionResult } from "./ImportDropzone";
import { ImportMetadataCard } from "./ImportMetadataCard";
import { OrthoBoundingBoxInputs } from "../ortho/OrthoBoundingBoxInputs";

export interface ImportOptionATabProps {
  selectedFile: FileSelectionResult | null;
  onSelectFile: (file: FileSelectionResult) => void;
  onRemoveFile: () => void;
  imgDimensions: [number, number] | null;
  customName: string;
  onCustomNameChange: (val: string) => void;
  saveOriginal: boolean;
  onSaveOriginalChange: (val: boolean) => void;
  useManualBounds: boolean;
  onUseManualBoundsChange: (val: boolean) => void;
  south: string;
  onSouthChange: (val: string) => void;
  north: string;
  onNorthChange: (val: string) => void;
  west: string;
  onWestChange: (val: string) => void;
  east: string;
  onEastChange: (val: string) => void;
  southError: string | null;
  northError: string | null;
  westError: string | null;
  eastError: string | null;
  aspectRatioWarning: string | null;
  hasMap: boolean;
  onFillFromMap: () => void;
  errorMessage: string | null;
  onError: (err: string) => void;
  idctQuality?: number;
  onIdctQualityChange?: (val: number) => void;
  disabled?: boolean;
}

export const ImportOptionATab: React.FC<ImportOptionATabProps> = ({
  selectedFile,
  onSelectFile,
  onRemoveFile,
  imgDimensions,
  customName,
  onCustomNameChange,
  saveOriginal,
  onSaveOriginalChange,
  useManualBounds,
  onUseManualBoundsChange,
  south,
  onSouthChange,
  north,
  onNorthChange,
  west,
  onWestChange,
  east,
  onEastChange,
  southError,
  northError,
  westError,
  eastError,
  aspectRatioWarning,
  hasMap,
  onFillFromMap,
  errorMessage,
  onError,
  idctQuality = 80,
  onIdctQualityChange,
  disabled = false,
}) => {
  const [internalQuality, setInternalQuality] = React.useState(idctQuality);
  const currentQuality = onIdctQualityChange ? idctQuality : internalQuality;
  const handleQualityChange = (val: number) => {
    setInternalQuality(val);
    onIdctQualityChange?.(val);
  };

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        gap: "14px",
      }}
    >
      {/* 1. PILIH BERKAS CITRA */}
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
          <ImportMetadataCard
            fileName={selectedFile.name}
            formatTag={selectedFile.ext}
            mimeType={selectedFile.mimeType}
            dimensions={imgDimensions}
            fileSizeBytes={selectedFile.size}
            onRemove={onRemoveFile}
            disabled={disabled}
          />
        ) : (
          <ImportDropzone
            label="Pilih Berkas Citra (JPG / PNG)"
            sublabel="Drag & drop berkas atau klik untuk membuka dialog pemilihan berkas"
            allowedExtensions={["jpg", "jpeg", "png"]}
            filterName="Citra Udara (JPG, PNG)"
            onFileSelect={onSelectFile}
            onError={onError}
            disabled={disabled}
          />
        )}
      </div>

      {/* SLIDER KOMPRESI IDCT */}
      <div
        style={{
          background: "#f8fafc",
          padding: "12px 14px",
          borderRadius: "12px",
        }}
      >
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
            marginBottom: "6px",
          }}
        >
          <span style={{ fontSize: "12px", fontWeight: 600, color: "#475569" }}>
            Target Kualitas Kompresi IDCT
          </span>
          <span
            style={{
              fontSize: "12px",
              fontWeight: 700,
              color: "#0f172a",
              background: "#e2e8f0",
              padding: "2px 8px",
              borderRadius: "6px",
            }}
          >
            {currentQuality}%
          </span>
        </div>
        <input
          type="range"
          min={50}
          max={95}
          step={5}
          value={currentQuality}
          onChange={(e) => handleQualityChange(Number(e.target.value))}
          disabled={disabled}
          style={{
            width: "100%",
            accentColor: "#0f172a",
            cursor: disabled ? "not-allowed" : "pointer",
          }}
        />
        <div style={{ fontSize: "11px", color: "#64748b", marginTop: "4px" }}>
          Target pratinjau cepat (≤ 5 MB). Algoritma IDCT adaptif mempertahankan detail tekstur
          aerial.
        </div>
      </div>

      {/* 2. NAMA LAYER KUSTOM */}
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
          onChange={(e) => onCustomNameChange(e.target.value)}
          disabled={disabled}
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

      {/* 3. CHECKBOX SIMPAN ORIGINAL */}
      <div
        style={{
          background: "#f8fafc",
          padding: "12px",
          borderRadius: "12px",
        }}
      >
        <Checkbox
          checked={saveOriginal}
          onChange={(e) => onSaveOriginalChange(e.currentTarget.checked)}
          label="Simpan berkas citra asli di penyimpanan aplikasi"
          disabled={disabled}
          style={{ marginBottom: "2px", fontWeight: 600 }}
        />
        <div style={{ fontSize: "11px", color: "#64748b", marginLeft: "24px" }}>
          {saveOriginal
            ? "Berkas asli akan disimpan sehingga dapat di-render ulang dengan resolusi lebih tinggi di masa mendatang."
            : "Perhatian: Tanpa file asli, citra tidak dapat di-render ulang dengan resolusi lebih tinggi di masa mendatang."}
        </div>
      </div>

      {/* 4. DETEKSI KOORDINAT & BOUNDS MANUAL */}
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
          {hasMap && (
            <button
              type="button"
              disabled={disabled}
              onClick={onFillFromMap}
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
          onChange={(e) => onUseManualBoundsChange(e.currentTarget.checked)}
          label="Masukkan batas koordinat WGS84 secara manual"
          disabled={disabled}
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
            Sistem akan otomatis mendeteksi berkas World File (.jgw, .pgw, .wld) atau Sidecar JSON
            (.json) di folder yang sama dengan citra. Centang kotak di atas jika ingin memasukkan
            koordinat manual.
          </div>
        ) : (
          <OrthoBoundingBoxInputs
            hideHeader
            westStr={west}
            setWestStr={onWestChange}
            southStr={south}
            setSouthStr={onSouthChange}
            eastStr={east}
            setEastStr={onEastChange}
            northStr={north}
            setNorthStr={onNorthChange}
            westError={westError}
            southError={southError}
            eastError={eastError}
            northError={northError}
            disabled={disabled}
          >
            {aspectRatioWarning && (
              <div style={{ marginTop: "2px" }}>
                <Callout
                  intent={Intent.WARNING}
                  icon="warning-sign"
                  style={{ fontSize: "11px", padding: "8px 12px" }}
                >
                  {aspectRatioWarning}
                </Callout>
              </div>
            )}
          </OrthoBoundingBoxInputs>
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
  );
};
