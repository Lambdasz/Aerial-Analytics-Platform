import React from "react";
import { Checkbox, Callout, Intent, Tag, Spinner, Icon } from "@blueprintjs/core";
import type { GeotiffInspectionResult } from "../../types/storage";
import { ImportDropzone, type FileSelectionResult } from "./ImportDropzone";
import { ImportMetadataCard } from "./ImportMetadataCard";
import { OrthoBoundingBoxInputs } from "../ortho/OrthoBoundingBoxInputs";

export interface ImportOptionBTabProps {
  selectedTiffFile: FileSelectionResult | null;
  onSelectTiffFile: (file: FileSelectionResult) => void;
  onRemoveTiffFile: () => void;
  isInspectingTiff: boolean;
  tiffInspection: GeotiffInspectionResult | null;
  customNameTiff: string;
  onCustomNameTiffChange: (val: string) => void;
  saveOriginalTiff: boolean;
  onSaveOriginalTiffChange: (val: boolean) => void;
  tiffWest: string;
  onTiffWestChange: (val: string) => void;
  tiffSouth: string;
  onTiffSouthChange: (val: string) => void;
  tiffEast: string;
  onTiffEastChange: (val: string) => void;
  tiffNorth: string;
  onTiffNorthChange: (val: string) => void;
  tiffSouthError: string | null;
  tiffNorthError: string | null;
  tiffWestError: string | null;
  tiffEastError: string | null;
  isBboxEdited: boolean;
  onResetToDetectedBbox: () => void;
  hasMap: boolean;
  onFillTiffFromMap: () => void;
  tiffError: string | null;
  onError: (err: string) => void;
  disabled?: boolean;
}

export const ImportOptionBTab: React.FC<ImportOptionBTabProps> = ({
  selectedTiffFile,
  onSelectTiffFile,
  onRemoveTiffFile,
  isInspectingTiff,
  tiffInspection,
  customNameTiff,
  onCustomNameTiffChange,
  saveOriginalTiff,
  onSaveOriginalTiffChange,
  tiffWest,
  onTiffWestChange,
  tiffSouth,
  onTiffSouthChange,
  tiffEast,
  onTiffEastChange,
  tiffNorth,
  onTiffNorthChange,
  tiffSouthError,
  tiffNorthError,
  tiffWestError,
  tiffEastError,
  isBboxEdited,
  onResetToDetectedBbox,
  hasMap,
  onFillTiffFromMap,
  tiffError,
  onError,
  disabled = false,
}) => {
  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        gap: "14px",
      }}
    >
      {/* 1. PILIH BERKAS GEOTIFF */}
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
          <ImportMetadataCard
            fileName={selectedTiffFile.name}
            formatTag="GEOTIFF"
            mimeType={selectedTiffFile.mimeType || "image/tiff"}
            dimensions={tiffInspection ? [tiffInspection.width, tiffInspection.height] : undefined}
            fileSizeBytes={tiffInspection?.file_size_bytes ?? selectedTiffFile.size}
            bitDepth={tiffInspection?.bit_depth}
            hasAlpha={tiffInspection?.has_alpha}
            compression={tiffInspection?.compression}
            onRemove={onRemoveTiffFile}
            disabled={disabled}
          />
        ) : (
          <ImportDropzone
            label="Pilih Berkas GeoTIFF (.tif, .tiff, .geotiff)"
            sublabel="Drag & drop berkas atau klik untuk membuka dialog pemilihan berkas GeoTIFF"
            allowedExtensions={["tif", "tiff", "geotiff"]}
            filterName="Citra GeoTIFF (TIF, TIFF, GEOTIFF)"
            onFileSelect={onSelectTiffFile}
            onError={onError}
            disabled={disabled}
          />
        )}
      </div>

      {/* 2. NAMA LAYER KUSTOM TAB B */}
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
          onChange={(e) => onCustomNameTiffChange(e.target.value)}
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

      {/* 3. CHECKBOX "Simpan file TIFF asli" */}
      <div
        style={{
          background: saveOriginalTiff ? "#fef3c7" : "#f8fafc",
          padding: "12px",
          borderRadius: "12px",
        }}
      >
        <Checkbox
          checked={saveOriginalTiff}
          onChange={(e) => onSaveOriginalTiffChange(e.currentTarget.checked)}
          label="Simpan file TIFF asli di penyimpanan aplikasi"
          disabled={disabled}
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
              ⚠️ <strong>Peringatan Ruang Disk:</strong> Berkas TIFF asli berukuran penuh (dapat
              mencapai puluhan hingga ratusan MB) akan disalin ke direktori penyimpanan. Gunakan
              hanya jika Anda memerlukan re-ekspor atau analisis piksel asli.
            </span>
          ) : (
            <span>
              <strong>Rekomendasi (Default Nonaktif):</strong> Berkas TIFF asli tidak disimpan di
              storage aplikasi untuk menghemat ruang disk. Sistem hanya menyimpan pratinjau display
              teroptimasi (≤ 10 MiB / ≤ 5 MiB) yang cukup untuk navigasi visual peta.
            </span>
          )}
        </div>
      </div>

      {/* 4. GEOREFERENSI, KOREKSI BBOX HASIL DETEKSI, & FALLBACK MANUAL */}
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
                  disabled={disabled}
                  onClick={onResetToDetectedBbox}
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
              {hasMap && (
                <button
                  type="button"
                  disabled={disabled}
                  onClick={onFillTiffFromMap}
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
                Silakan masukkan batas koordinat WGS84 secara manual di bawah ini (atau gunakan
                tombol <strong>Isi dari Viewport Peta</strong>) agar citra dapat ditampilkan pada
                posisi yang tepat.
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
              Koordinat di bawah ini diekstrak otomatis dari GeoTIFF. Anda dapat mengoreksi atau
              menyesuaikan batas koordinat jika diperlukan.
            </div>
          )}

          <OrthoBoundingBoxInputs
            hideHeader
            westStr={tiffWest}
            setWestStr={onTiffWestChange}
            southStr={tiffSouth}
            setSouthStr={onTiffSouthChange}
            eastStr={tiffEast}
            setEastStr={onTiffEastChange}
            northStr={tiffNorth}
            setNorthStr={onTiffNorthChange}
            westError={tiffWestError}
            southError={tiffSouthError}
            eastError={tiffEastError}
            northError={tiffNorthError}
            disabled={disabled}
          />
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
  );
};
