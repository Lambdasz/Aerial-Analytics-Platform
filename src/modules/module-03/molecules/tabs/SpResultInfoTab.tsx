import React from "react";
import { Button, Tag, Icon } from "@blueprintjs/core";
import type { SpatialResultSummary } from "../../types/spatialResult";
import { GeomTypePill } from "../../atoms/GeomTypePill";
import { ResultPhoto } from "../ResultPhoto";
import { isValidHexColor } from "../../lib/fieldValidation";

export interface SpResultInfoTabProps {
  item: SpatialResultSummary;
  name: string;
  onNameChange: (val: string) => void;
  displayColor: string;
  onDisplayColorChange: (val: string) => void;
  icon: string;
  onIconChange: (val: string) => void;
  imageAction: "keep" | "delete" | "replace";
  previewUrl: string | null;
  selectedFileName: string | null;
  onSelectFile: () => void;
  onDeletePhoto: () => void;
  onResetPhoto: () => void;
  fileInputRef: React.RefObject<HTMLInputElement | null>;
  onFileInputChange: (e: React.ChangeEvent<HTMLInputElement>) => void;
  isSubmitting?: boolean;
}

const COLOR_PRESETS = [
  "#2ecc40", // Hijau terang
  "#0074d9", // Biru
  "#ff851b", // Oranye
  "#e74c3c", // Merah
  "#b10dc9", // Ungu
  "#39cccc", // Teal
  "#ffdc00", // Kuning
];

const KNOWN_ICONS = [
  { value: "", label: "(Standar / Titik Bulat)" },
  { value: "tree_marker", label: "Pohon (Tree Marker)" },
];

export const SpResultInfoTab: React.FC<SpResultInfoTabProps> = ({
  item,
  name,
  onNameChange,
  displayColor,
  onDisplayColorChange,
  icon,
  onIconChange,
  imageAction,
  previewUrl,
  selectedFileName,
  onSelectFile,
  onDeletePhoto,
  onResetPhoto,
  fileInputRef,
  onFileInputChange,
  isSubmitting = false,
}) => {
  const isPointGeom = item.geom_type === "Point";

  const isCustomIcon = Boolean(icon) && !KNOWN_ICONS.some((ki) => ki.value === icon);
  const iconOptions = [...KNOWN_ICONS];
  if (isCustomIcon) {
    iconOptions.push({ value: icon, label: `(Kustom) ${icon}` });
  }

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "16px" }}>
      {/* Ringkasan Metadata Immutable */}
      <div
        style={{
          background: "#f8fafc",
          borderRadius: "12px",
          padding: "12px 16px",
          display: "grid",
          gridTemplateColumns: "1fr 1fr 1fr",
          gap: "8px",
          fontSize: "12px",
        }}
      >
        <div>
          <span style={{ color: "#64748b" }}>ID Layer:</span>{" "}
          <code style={{ fontSize: "11px", color: "#0f172a" }}>{item.id}</code>
        </div>
        <div>
          <span style={{ color: "#64748b" }}>Geometri:</span>{" "}
          <GeomTypePill geomType={item.geom_type} />
        </div>
        <div>
          <span style={{ color: "#64748b" }}>Jumlah Fitur:</span>{" "}
          <strong style={{ color: "#0f172a" }}>{item.feature_count} fitur</strong>
        </div>
      </div>

      {/* Nama Layer */}
      <div>
        <div
          style={{
            fontSize: "12px",
            fontWeight: 600,
            color: "#64748b",
            marginBottom: "6px",
          }}
        >
          Nama Hasil Analisis
        </div>
        <input
          id="layer-name-input"
          type="text"
          value={name}
          onChange={(e) => onNameChange(e.target.value)}
          placeholder="Nama layer..."
          disabled={isSubmitting}
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
          Nama yang tampil pada sidebar dan judul popup
        </div>
      </div>

      {/* Dropdown Ikon (Hanya jika geometri Point) */}
      {isPointGeom && (
        <div>
          <div
            style={{
              fontSize: "12px",
              fontWeight: 600,
              color: "#64748b",
              marginBottom: "6px",
            }}
          >
            Ikon Titik Marker
          </div>
          <select
            id="layer-icon-select"
            value={icon}
            onChange={(e) => onIconChange(e.target.value)}
            disabled={isSubmitting}
            style={{
              width: "100%",
              padding: "9px 12px",
              fontSize: "13px",
              borderRadius: "10px",
              border: "none",
              background: "#f8fafc",
              color: "#0f172a",
              outline: "none",
              boxSizing: "border-box",
              cursor: "pointer",
            }}
          >
            {iconOptions.map((opt) => (
              <option key={opt.value} value={opt.value}>
                {opt.label}
              </option>
            ))}
          </select>
          <div style={{ fontSize: "11px", color: "#64748b", marginTop: "4px" }}>
            Pilih visualisasi titik penanda pada peta Leaflet
          </div>
        </div>
      )}

      {/* Warna Tampilan */}
      <div>
        <div
          style={{
            fontSize: "12px",
            fontWeight: 600,
            color: "#64748b",
            marginBottom: "6px",
          }}
        >
          Warna Tampilan (HEX)
        </div>
        <div style={{ display: "flex", gap: "8px", alignItems: "center" }}>
          <div
            style={{
              width: "34px",
              height: "34px",
              borderRadius: "8px",
              backgroundColor: isValidHexColor(displayColor) ? displayColor : "#e2e8f0",
              border: "none",
              boxShadow: "0 1px 3px rgba(0,0,0,0.1)",
              flexShrink: 0,
            }}
          />
          <input
            id="layer-color-input"
            type="text"
            value={displayColor}
            onChange={(e) => onDisplayColorChange(e.target.value)}
            placeholder="#2ecc40"
            disabled={isSubmitting}
            style={{
              width: "110px",
              padding: "8px 12px",
              fontSize: "13px",
              background: "#f8fafc",
              border: "none",
              borderRadius: "10px",
              outline: "none",
              color: "#0f172a",
              boxSizing: "border-box",
            }}
          />
          {/* Palet Preset */}
          <div
            style={{
              display: "flex",
              gap: "6px",
              flexWrap: "wrap",
              alignItems: "center",
            }}
          >
            {COLOR_PRESETS.map((c) => (
              <button
                key={c}
                type="button"
                onClick={() => onDisplayColorChange(c)}
                style={{
                  width: "24px",
                  height: "24px",
                  borderRadius: "6px",
                  backgroundColor: c,
                  border:
                    displayColor.toLowerCase() === c.toLowerCase() ? "2px solid #0f172a" : "none",
                  cursor: "pointer",
                  padding: 0,
                }}
              />
            ))}
          </div>
        </div>
        <div style={{ fontSize: "11px", color: "#64748b", marginTop: "4px" }}>
          Warna penanda titik, garis batas poligon, atau jalur inspeksi
        </div>
      </div>

      {/* Pengaturan Foto Layer */}
      <div>
        <div
          style={{
            fontSize: "12px",
            fontWeight: 600,
            color: "#64748b",
            marginBottom: "6px",
          }}
        >
          Foto Layer / Citra Utama
        </div>
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: "14px",
            padding: "14px",
            borderRadius: "12px",
            background: "#f8fafc",
          }}
        >
          <div style={{ flexShrink: 0 }}>
            {imageAction === "delete" ? (
              <div
                style={{
                  width: 72,
                  height: 56,
                  borderRadius: 8,
                  background: "#f1f5f9",
                  border: "1px dashed #cbd5e1",
                  display: "flex",
                  flexDirection: "column",
                  alignItems: "center",
                  justifyContent: "center",
                  color: "#94a3b8",
                }}
              >
                <Icon icon="media" size={20} />
                <span style={{ fontSize: 10, marginTop: 2, fontWeight: 500 }}>No Image</span>
              </div>
            ) : (
              <ResultPhoto
                resultId={item.id}
                imageUrl={previewUrl}
                size="medium"
                alt="Foto Layer"
              />
            )}
          </div>

          <div style={{ flex: 1, display: "flex", flexDirection: "column", gap: "6px" }}>
            <div style={{ fontSize: "12px" }}>
              {imageAction === "delete" && (
                <Tag intent="danger" minimal style={{ fontSize: "11px" }}>
                  Foto layer akan dihapus (No Image)
                </Tag>
              )}
              {imageAction === "replace" && (
                <Tag intent="success" minimal style={{ fontSize: "11px" }}>
                  Foto baru: {selectedFileName}
                </Tag>
              )}
              {imageAction === "keep" && (
                <span style={{ color: "#64748b" }}>
                  {previewUrl ? "Foto layer aktif terpasang" : "Belum ada foto layer"}
                </span>
              )}
            </div>

            <div style={{ display: "flex", gap: "8px", flexWrap: "wrap" }}>
              <Button
                icon="upload"
                small
                minimal
                text={previewUrl ? "Ganti Foto" : "Unggah Foto"}
                onClick={onSelectFile}
                disabled={isSubmitting}
                style={{
                  background: "#ffffff",
                  color: "#0f172a",
                  borderRadius: "999px",
                  padding: "4px 12px",
                  boxShadow: "0 1px 3px rgba(0,0,0,0.06)",
                  fontWeight: 600,
                }}
              />
              {previewUrl && imageAction !== "delete" && (
                <Button
                  icon="trash"
                  small
                  minimal
                  text="Hapus Foto"
                  onClick={onDeletePhoto}
                  disabled={isSubmitting}
                  style={{
                    background: "#f1f5f9",
                    color: "#475569",
                    borderRadius: "999px",
                    padding: "4px 12px",
                    fontWeight: 600,
                  }}
                />
              )}
              {imageAction !== "keep" && (
                <Button
                  icon="undo"
                  small
                  minimal
                  text="Batal Ubah"
                  onClick={onResetPhoto}
                  disabled={isSubmitting}
                  style={{
                    background: "#f1f5f9",
                    color: "#475569",
                    borderRadius: "999px",
                    padding: "4px 12px",
                    fontWeight: 600,
                  }}
                />
              )}
            </div>
          </div>

          <input
            ref={fileInputRef}
            type="file"
            accept="image/jpeg,image/png,image/webp"
            style={{ display: "none" }}
            onChange={onFileInputChange}
          />
        </div>
        <div style={{ fontSize: "11px", color: "#64748b", marginTop: "4px" }}>
          Foto representasi keseluruhan layer (tersimpan pada display_preference.image_url)
        </div>
      </div>
    </div>
  );
};
