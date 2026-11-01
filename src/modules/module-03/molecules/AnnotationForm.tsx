import React from "react";
import { Button, Icon, Tooltip } from "@blueprintjs/core";

export type AnnotationGeomType = "Point" | "LineString" | "Polygon";

export interface AnnotationFormProps {
  annotationText: string;
  geomType: AnnotationGeomType;
  latitude: string;
  longitude: string;
  manualCoordsText: string;
  coordMode: "map" | "manual";
  isPickingLocation: boolean;
  hasGeometry: boolean;
  geometrySummary?: string;
  isEditing?: boolean;
  isSubmitting?: boolean;
  error?: string | null;
  onTextChange: (text: string) => void;
  onGeomTypeChange: (type: AnnotationGeomType) => void;
  onLatitudeChange: (lat: string) => void;
  onLongitudeChange: (lng: string) => void;
  onManualCoordsChange: (coords: string) => void;
  onCoordModeChange: (mode: "map" | "manual") => void;
  onTogglePickLocation: () => void;
  onClearGeometry: () => void;
  onSubmit: () => void;
  onCancel?: () => void;
}

export const AnnotationForm: React.FC<AnnotationFormProps> = ({
  annotationText,
  geomType,
  latitude,
  longitude,
  manualCoordsText,
  coordMode,
  isPickingLocation,
  hasGeometry,
  geometrySummary,
  isEditing = false,
  isSubmitting = false,
  error,
  onTextChange,
  onGeomTypeChange,
  onLatitudeChange,
  onLongitudeChange,
  onManualCoordsChange,
  onCoordModeChange,
  onTogglePickLocation,
  onClearGeometry,
  onSubmit,
  onCancel,
}) => {
  return (
    <div>
      {/* Header Form */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          marginBottom: "14px",
        }}
      >
        <span
          style={{
            fontWeight: 700,
            fontSize: "14px",
            color: "#0f172a",
          }}
        >
          {isEditing ? "Edit Anotasi Spasial" : "Buat Anotasi Spasial"}
        </span>
      </div>

      {/* Catatan Anotasi */}
      <div style={{ marginBottom: "14px" }}>
        <div
          style={{
            fontSize: "12px",
            fontWeight: 600,
            color: "#64748b",
            marginBottom: "6px",
          }}
        >
          Catatan / Keterangan Lokasi <span style={{ color: "#ef4444" }}>*</span>
        </div>
        <div
          style={{
            background: "#f8fafc",
            borderRadius: "10px",
            padding: "10px 12px",
            border: "none",
          }}
        >
          <textarea
            placeholder="Tulis catatan (misal: Patok batas lahan, jalur kabel, area genangan air, dll)..."
            value={annotationText}
            onChange={(event) => onTextChange(event.target.value)}
            disabled={isSubmitting}
            style={{
              width: "100%",
              minHeight: "56px",
              border: "none",
              outline: "none",
              background: "transparent",
              color: "#0f172a",
              fontSize: "13px",
              fontFamily: "var(--font-sf-pro-text)",
              resize: "vertical",
            }}
          />
        </div>
      </div>

      {/* Pilihan Bentuk Geometri: Titik vs Garis vs Poligon */}
      <div style={{ marginBottom: "14px" }}>
        <div
          style={{
            fontSize: "12px",
            fontWeight: 600,
            color: "#64748b",
            marginBottom: "6px",
          }}
        >
          Bentuk Anotasi
        </div>
        <div
          style={{
            display: "flex",
            gap: "3px",
            background: "#f1f5f9",
            padding: "3px",
            borderRadius: "8px",
          }}
        >
          {(
            [
              { id: "Point", label: "Titik" },
              { id: "LineString", label: "Garis" },
              { id: "Polygon", label: "Poligon" },
            ] as const satisfies Array<{
              id: AnnotationGeomType;
              label: string;
            }>
          ).map((item) => {
            const isActive = geomType === item.id;
            return (
              <button
                key={item.id}
                type="button"
                onClick={() => onGeomTypeChange(item.id)}
                disabled={isSubmitting || isPickingLocation}
                style={{
                  flex: 1,
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  padding: "5px 4px",
                  borderRadius: "6px",
                  border: "none",
                  outline: "none",
                  background: isActive ? "#ffffff" : "transparent",
                  boxShadow: isActive ? "0 1px 3px rgba(0, 0, 0, 0.08)" : "none",
                  color: isActive ? "#0f172a" : "#64748b",
                  fontWeight: isActive ? 600 : 500,
                  fontSize: "11px",
                  cursor: isSubmitting || isPickingLocation ? "not-allowed" : "pointer",
                  transition: "all 0.15s ease",
                  userSelect: "none",
                }}
              >
                {item.label}
              </button>
            );
          })}
        </div>
      </div>

      {/* Pilihan Metode: Gambar di Peta vs Input Manual */}
      <div style={{ marginBottom: "14px" }}>
        <div
          style={{
            fontSize: "12px",
            fontWeight: 600,
            color: "#64748b",
            marginBottom: "6px",
          }}
        >
          Metode Penentuan
        </div>

        <div
          style={{
            display: "flex",
            gap: "3px",
            background: "#f1f5f9",
            padding: "3px",
            borderRadius: "8px",
          }}
        >
          <button
            type="button"
            onClick={() => onCoordModeChange("map")}
            disabled={isSubmitting || isPickingLocation}
            style={{
              flex: 1,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              gap: "6px",
              padding: "5px 4px",
              borderRadius: "6px",
              border: "none",
              outline: "none",
              background: coordMode === "map" ? "#ffffff" : "transparent",
              boxShadow: coordMode === "map" ? "0 1px 3px rgba(0, 0, 0, 0.08)" : "none",
              color: coordMode === "map" ? "#0f172a" : "#64748b",
              fontWeight: coordMode === "map" ? 600 : 500,
              fontSize: "11px",
              cursor: isSubmitting || isPickingLocation ? "not-allowed" : "pointer",
              transition: "all 0.15s ease",
              userSelect: "none",
            }}
          >
            <Icon icon="locate" size={13} color={coordMode === "map" ? "#0f172a" : "#64748b"} />
            Gambar di Peta
          </button>

          <button
            type="button"
            onClick={() => onCoordModeChange("manual")}
            disabled={isSubmitting || isPickingLocation}
            style={{
              flex: 1,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              gap: "6px",
              padding: "5px 4px",
              borderRadius: "6px",
              border: "none",
              outline: "none",
              background: coordMode === "manual" ? "#ffffff" : "transparent",
              boxShadow: coordMode === "manual" ? "0 1px 3px rgba(0, 0, 0, 0.08)" : "none",
              color: coordMode === "manual" ? "#0f172a" : "#64748b",
              fontWeight: coordMode === "manual" ? 600 : 500,
              fontSize: "11px",
              cursor: isSubmitting || isPickingLocation ? "not-allowed" : "pointer",
              transition: "all 0.15s ease",
              userSelect: "none",
            }}
          >
            <Icon icon="edit" size={13} color={coordMode === "manual" ? "#0f172a" : "#64748b"} />
            Input Manual
          </button>
        </div>
      </div>

      {/* Konten Mode: GAMBAR DI PETA */}
      {coordMode === "map" && (
        <div style={{ marginBottom: "14px" }}>
          <div style={{ display: "flex", gap: "8px" }}>
            <Button
              minimal
              icon={isPickingLocation ? "cross" : "locate"}
              text={
                isPickingLocation
                  ? geomType === "Point"
                    ? "Batal Pilih Titik"
                    : geomType === "LineString"
                      ? "Batal Gambar Garis"
                      : "Batal Gambar Poligon"
                  : hasGeometry
                    ? geomType === "Point"
                      ? "Pilih Ulang Titik di Peta"
                      : geomType === "LineString"
                        ? "Gambar Ulang Garis di Peta"
                        : "Gambar Ulang Poligon di Peta"
                    : geomType === "Point"
                      ? "Pilih Titik di Peta"
                      : geomType === "LineString"
                        ? "Mulai Gambar Garis di Peta"
                        : "Mulai Gambar Poligon di Peta"
              }
              fill
              onClick={onTogglePickLocation}
              disabled={isSubmitting}
              style={{
                borderRadius: "10px",
                background: isPickingLocation ? "#f1f5f9" : "#0f172a",
                color: isPickingLocation ? "#0f172a" : "#ffffff",
                fontWeight: 600,
                padding: "8px 14px",
                fontSize: "12px",
                boxShadow: "none",
              }}
            />
          </div>

          {isPickingLocation && (
            <div
              style={{
                marginTop: "8px",
                background: "#f8fafc",
                borderRadius: "10px",
                padding: "10px 12px",
                display: "flex",
                alignItems: "flex-start",
                gap: "8px",
                fontSize: "12px",
                color: "#475569",
                lineHeight: 1.45,
              }}
            >
              <Icon
                icon="info-sign"
                size={14}
                color="#64748b"
                style={{ marginTop: "2px", flexShrink: 0 }}
              />
              <div>
                {geomType === "Point" && (
                  <span>
                    Arahkan kursor dan <strong>klik pada titik di peta</strong> untuk menentukan
                    posisi anotasi.
                  </span>
                )}
                {geomType === "LineString" && (
                  <span>
                    Klik di peta untuk membuat titik garis. <strong>Klik dua kali</strong> pada
                    titik terakhir untuk menyelesaikan garis.
                  </span>
                )}
                {geomType === "Polygon" && (
                  <span>
                    Klik di peta untuk membuat titik poligon. <strong>Klik titik awal</strong> untuk
                    menutup poligon.
                  </span>
                )}
              </div>
            </div>
          )}

          {hasGeometry && !isPickingLocation && (
            <div
              style={{
                marginTop: "8px",
                padding: "8px 12px",
                background: "#f0fdf4",
                border: "none",
                borderRadius: "10px",
                fontSize: "12px",
                color: "#166534",
                display: "flex",
                justifyContent: "space-between",
                alignItems: "center",
              }}
            >
              <span>
                {geomType === "Point" && (
                  <>
                    Koordinat: <strong>{latitude}</strong>, <strong>{longitude}</strong>
                  </>
                )}
                {geomType !== "Point" && (
                  <strong>{geometrySummary || "Bentuk spasial siap disimpan"}</strong>
                )}
              </span>
              <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
                <Tooltip content="Hapus bentuk sementara" placement="top">
                  <Icon
                    icon="cross"
                    size={13}
                    style={{ cursor: "pointer", color: "#166534" }}
                    onClick={onClearGeometry}
                  />
                </Tooltip>
                <Tooltip content="Bentuk ini siap disimpan" placement="top">
                  <Icon icon="tick-circle" color="#16a34a" size={14} />
                </Tooltip>
              </div>
            </div>
          )}
        </div>
      )}

      {/* Konten Mode: INPUT MANUAL */}
      {coordMode === "manual" && (
        <div style={{ marginBottom: "12px" }}>
          {geomType === "Point" ? (
            <div>
              <div style={{ display: "flex", gap: "8px" }}>
                <div
                  style={{
                    flex: 1,
                    background: "#f8fafc",
                    borderRadius: "10px",
                    padding: "6px 10px",
                  }}
                >
                  <div
                    style={{
                      fontSize: "10px",
                      fontWeight: 600,
                      color: "var(--color-slate)",
                      marginBottom: "2px",
                    }}
                  >
                    Latitude
                  </div>
                  <input
                    type="number"
                    step="any"
                    placeholder="-1.224311"
                    value={latitude}
                    onChange={(event) => onLatitudeChange(event.target.value)}
                    disabled={isSubmitting}
                    style={{
                      width: "100%",
                      border: "none",
                      outline: "none",
                      background: "transparent",
                      color: "var(--color-ink)",
                      fontSize: "13px",
                      fontFamily: "monospace",
                    }}
                  />
                </div>

                <div
                  style={{
                    flex: 1,
                    background: "#f8fafc",
                    borderRadius: "10px",
                    padding: "6px 10px",
                  }}
                >
                  <div
                    style={{
                      fontSize: "10px",
                      fontWeight: 600,
                      color: "var(--color-slate)",
                      marginBottom: "2px",
                    }}
                  >
                    Longitude
                  </div>
                  <input
                    type="number"
                    step="any"
                    placeholder="116.855973"
                    value={longitude}
                    onChange={(event) => onLongitudeChange(event.target.value)}
                    disabled={isSubmitting}
                    style={{
                      width: "100%",
                      border: "none",
                      outline: "none",
                      background: "transparent",
                      color: "var(--color-ink)",
                      fontSize: "13px",
                      fontFamily: "monospace",
                    }}
                  />
                </div>
              </div>
              <div style={{ fontSize: "11px", color: "var(--color-slate)", marginTop: "4px" }}>
                Format desimal WGS84 (Lat: -90 s/d 90, Lng: -180 s/d 180).
              </div>
            </div>
          ) : (
            <div>
              <div
                style={{
                  background: "#f8fafc",
                  borderRadius: "10px",
                  padding: "8px 10px",
                }}
              >
                <div
                  style={{
                    fontSize: "10px",
                    fontWeight: 600,
                    color: "var(--color-slate)",
                    marginBottom: "4px",
                  }}
                >
                  Daftar Koordinat (Lat, Lng per baris):
                </div>
                <textarea
                  placeholder={
                    geomType === "LineString"
                      ? "-1.224311, 116.855973\n-1.225100, 116.857200\n-1.226300, 116.858400"
                      : "-1.224000, 116.854000\n-1.224000, 116.858000\n-1.227000, 116.858000\n-1.227000, 116.854000"
                  }
                  value={manualCoordsText}
                  onChange={(e) => onManualCoordsChange(e.target.value)}
                  disabled={isSubmitting}
                  style={{
                    width: "100%",
                    minHeight: "70px",
                    border: "none",
                    outline: "none",
                    background: "transparent",
                    color: "var(--color-ink)",
                    fontSize: "12px",
                    fontFamily: "monospace",
                    resize: "vertical",
                  }}
                />
              </div>
              <div
                style={{
                  fontSize: "11px",
                  color: hasGeometry ? "#d97706" : "var(--color-slate)",
                  marginTop: "4px",
                  fontWeight: hasGeometry ? 600 : 400,
                }}
              >
                {hasGeometry
                  ? `✓ ${geometrySummary}`
                  : geomType === "LineString"
                    ? "Minimal 2 titik koordinat untuk membentuk garis."
                    : "Minimal 3 titik sudut untuk membentuk poligon."}
              </div>
            </div>
          )}
        </div>
      )}

      {/* Error Message */}
      {error && (
        <div
          style={{
            marginBottom: "12px",
            background: "#fef2f2",
            color: "#dc2626",
            borderRadius: "10px",
            padding: "10px 12px",
            fontSize: "12px",
            fontWeight: 500,
          }}
        >
          {error}
        </div>
      )}

      {/* Tombol Aksi Batal & Simpan Anotasi */}
      <div style={{ display: "flex", gap: "8px", marginTop: "14px" }}>
        {onCancel && (
          <Button
            minimal
            text="Batal"
            onClick={onCancel}
            disabled={isSubmitting}
            style={{
              borderRadius: "10px",
              background: "#f1f5f9",
              color: "#475569",
              fontWeight: 600,
              padding: "8px 14px",
              fontSize: "12px",
              boxShadow: "none",
            }}
          />
        )}
        <Button
          minimal
          icon={isEditing ? "floppy-disk" : "add"}
          text={
            isSubmitting
              ? isEditing
                ? "Menyimpan Perubahan..."
                : "Menyimpan Anotasi..."
              : isEditing
                ? "Simpan Perubahan"
                : "Simpan Anotasi"
          }
          onClick={onSubmit}
          loading={isSubmitting}
          disabled={isSubmitting || !annotationText.trim() || !hasGeometry}
          style={{
            flex: 1,
            borderRadius: "10px",
            background:
              isSubmitting || !annotationText.trim() || !hasGeometry ? "#f1f5f9" : "#0f172a",
            color: isSubmitting || !annotationText.trim() || !hasGeometry ? "#94a3b8" : "#ffffff",
            boxShadow: "none",
            fontWeight: 600,
            padding: "8px 14px",
            fontSize: "12px",
            transition: "all 0.2s ease",
          }}
        />
      </div>
    </div>
  );
};
