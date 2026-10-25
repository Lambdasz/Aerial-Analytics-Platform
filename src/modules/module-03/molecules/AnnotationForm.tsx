import React from "react";
import { Button, Callout, Icon, Tag, Tooltip, type IconName } from "@blueprintjs/core";

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
}) => {
  return (
    <div
      style={{
        marginTop: "20px",
        paddingTop: "16px",
        borderTop: "1px solid var(--color-hairline-silver)",
      }}
    >
      {/* Header Form */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          marginBottom: "12px",
        }}
      >
        <span
          style={{
            fontWeight: 600,
            fontSize: "14px",
            color: "var(--color-ink)",
            display: "flex",
            alignItems: "center",
            gap: "6px",
          }}
        >
          <Icon icon="map-marker" size={16} color="#f59e0b" />
          Buat Anotasi Spasial
        </span>
        {hasGeometry && (
          <Tag minimal intent="success" round style={{ fontSize: "11px" }}>
            {geomType === "Point"
              ? "Titik Terpilih"
              : geomType === "LineString"
                ? "Garis Tergambar"
                : "Poligon Tergambar"}
          </Tag>
        )}
      </div>

      {/* Catatan Anotasi */}
      <div
        style={{
          marginBottom: "12px",
          background: "var(--color-gallery-white)",
          boxShadow: "var(--shadow-subtle)",
          borderRadius: "12px",
          padding: "8px 12px",
          border: "1px solid #e2e8f0",
        }}
      >
        <div
          style={{
            fontSize: "11px",
            fontWeight: 600,
            color: "var(--color-slate)",
            marginBottom: "4px",
          }}
        >
          Catatan / Keterangan Lokasi <span style={{ fontWeight: 400 }}>(wajib)</span>
        </div>
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
            color: "var(--color-ink)",
            fontSize: "13px",
            fontFamily: "var(--font-sf-pro-text)",
            resize: "vertical",
          }}
        />
      </div>

      {/* Pilihan Bentuk Geometri: Titik vs Garis vs Poligon */}
      <div style={{ marginBottom: "12px" }}>
        <div
          style={{
            fontSize: "11px",
            fontWeight: 600,
            color: "var(--color-slate)",
            marginBottom: "6px",
          }}
        >
          Bentuk Anotasi:
        </div>
        <div
          style={{
            display: "flex",
            gap: "4px",
            background: "var(--color-control-gray)",
            padding: "3px",
            borderRadius: "10px",
          }}
        >
          {(
            [
              { id: "Point", label: "Titik", icon: "map-marker" },
              { id: "LineString", label: "Garis", icon: "timeline-line-chart" },
              { id: "Polygon", label: "Poligon", icon: "polygon-filter" },
            ] as const satisfies Array<{
              id: AnnotationGeomType;
              label: string;
              icon: IconName;
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
                  gap: "5px",
                  padding: "6px 8px",
                  borderRadius: "8px",
                  border: "none",
                  background: isActive ? "var(--color-gallery-white)" : "transparent",
                  boxShadow: isActive ? "0 2px 4px rgba(0,0,0,0.08)" : "none",
                  color: isActive ? "var(--color-ink)" : "var(--color-slate)",
                  fontWeight: isActive ? 600 : 500,
                  fontSize: "12px",
                  cursor: isSubmitting || isPickingLocation ? "not-allowed" : "pointer",
                  transition: "all 0.15s ease",
                }}
              >
                <Icon
                  icon={item.icon}
                  size={13}
                  color={isActive ? "#f59e0b" : "var(--color-slate)"}
                />
                {item.label}
              </button>
            );
          })}
        </div>
      </div>

      {/* Pilihan Metode: Gambar di Peta vs Input Manual */}
      <div style={{ marginBottom: "12px" }}>
        <div
          style={{
            fontSize: "11px",
            fontWeight: 600,
            color: "var(--color-slate)",
            marginBottom: "6px",
          }}
        >
          Metode Penentuan:
        </div>

        <div
          style={{
            display: "flex",
            gap: "6px",
            background: "var(--color-control-gray)",
            padding: "3px",
            borderRadius: "10px",
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
              padding: "6px 10px",
              borderRadius: "8px",
              border: "none",
              background: coordMode === "map" ? "var(--color-gallery-white)" : "transparent",
              boxShadow: coordMode === "map" ? "0 2px 4px rgba(0,0,0,0.08)" : "none",
              color: coordMode === "map" ? "var(--color-ink)" : "var(--color-slate)",
              fontWeight: coordMode === "map" ? 600 : 500,
              fontSize: "12px",
              cursor: isSubmitting || isPickingLocation ? "not-allowed" : "pointer",
              transition: "all 0.15s ease",
            }}
          >
            <Icon icon="locate" size={13} color={coordMode === "map" ? "#f59e0b" : undefined} />
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
              padding: "6px 10px",
              borderRadius: "8px",
              border: "none",
              background: coordMode === "manual" ? "var(--color-gallery-white)" : "transparent",
              boxShadow: coordMode === "manual" ? "0 2px 4px rgba(0,0,0,0.08)" : "none",
              color: coordMode === "manual" ? "var(--color-ink)" : "var(--color-slate)",
              fontWeight: coordMode === "manual" ? 600 : 500,
              fontSize: "12px",
              cursor: isSubmitting || isPickingLocation ? "not-allowed" : "pointer",
              transition: "all 0.15s ease",
            }}
          >
            <Icon icon="edit" size={13} color={coordMode === "manual" ? "#f59e0b" : undefined} />
            Input Manual
          </button>
        </div>
      </div>

      {/* Konten Mode: GAMBAR DI PETA */}
      {coordMode === "map" && (
        <div style={{ marginBottom: "12px" }}>
          <div style={{ display: "flex", gap: "8px" }}>
            <Button
              icon={isPickingLocation ? "cross" : "locate"}
              intent={isPickingLocation ? "warning" : hasGeometry ? "primary" : "primary"}
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
                fontWeight: 600,
                padding: "8px 12px",
                fontSize: "12px",
              }}
            />
          </div>

          {isPickingLocation && (
            <Callout
              intent="warning"
              icon="info-sign"
              style={{
                marginTop: "8px",
                fontSize: "12px",
                padding: "8px 12px",
                borderRadius: "8px",
              }}
            >
              {geomType === "Point" && (
                <span>
                  Arahkan kursor dan <strong>klik pada titik di peta</strong> untuk menentukan
                  posisi anotasi.
                </span>
              )}
              {geomType === "LineString" && (
                <span>
                  Klik di peta untuk membuat titik garis. <strong>Klik dua kali</strong> pada titik
                  terakhir untuk menyelesaikan garis.
                </span>
              )}
              {geomType === "Polygon" && (
                <span>
                  Klik di peta untuk membuat titik poligon. <strong>Klik titik awal</strong> untuk
                  menutup poligon.
                </span>
              )}
            </Callout>
          )}

          {hasGeometry && !isPickingLocation && (
            <div
              style={{
                marginTop: "8px",
                padding: "8px 12px",
                background: "#fef3c7",
                border: "1px solid #fde68a",
                borderRadius: "10px",
                fontSize: "12px",
                color: "#92400e",
                display: "flex",
                justifyContent: "space-between",
                alignItems: "center",
              }}
            >
              <span>
                {geomType === "Point" && (
                  <>
                    📍 <strong>{latitude}</strong>, <strong>{longitude}</strong>
                  </>
                )}
                {geomType !== "Point" && (
                  <>
                    {geomType === "LineString" ? "📏 " : "⬡ "}
                    <strong>{geometrySummary || "Bentuk spasial siap disimpan"}</strong>
                  </>
                )}
              </span>
              <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
                <Tooltip content="Hapus bentuk sementara" placement="top">
                  <Icon
                    icon="cross"
                    size={13}
                    style={{ cursor: "pointer", color: "#b45309" }}
                    onClick={onClearGeometry}
                  />
                </Tooltip>
                <Tooltip content="Bentuk ini siap disimpan" placement="top">
                  <Icon icon="tick-circle" color="#d97706" size={14} />
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
                    background: "var(--color-gallery-white)",
                    boxShadow: "var(--shadow-subtle)",
                    borderRadius: "10px",
                    padding: "6px 10px",
                    border: "1px solid #e2e8f0",
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
                    background: "var(--color-gallery-white)",
                    boxShadow: "var(--shadow-subtle)",
                    borderRadius: "10px",
                    padding: "6px 10px",
                    border: "1px solid #e2e8f0",
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
                  background: "var(--color-gallery-white)",
                  boxShadow: "var(--shadow-subtle)",
                  borderRadius: "10px",
                  padding: "8px 10px",
                  border: "1px solid #e2e8f0",
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

      {/* Error Callout */}
      {error && (
        <Callout
          intent="danger"
          icon="error"
          style={{
            marginBottom: "12px",
            fontSize: "12px",
            padding: "8px 12px",
            borderRadius: "8px",
          }}
        >
          {error}
        </Callout>
      )}

      {/* Tombol Simpan / Buat Annotation */}
      <Button
        icon="add"
        text={isSubmitting ? "Menyimpan Anotasi..." : "Buat Annotation"}
        fill
        onClick={onSubmit}
        loading={isSubmitting}
        disabled={isSubmitting || !annotationText.trim() || !hasGeometry}
        style={{
          borderRadius: "999px",
          background:
            isSubmitting || !annotationText.trim() || !hasGeometry
              ? "var(--color-control-gray)"
              : "#f59e0b",
          color:
            isSubmitting || !annotationText.trim() || !hasGeometry ? "var(--color-slate)" : "#fff",
          boxShadow: "none",
          fontWeight: 600,
          padding: "8px 16px",
          transition: "all 0.2s ease",
        }}
      />
    </div>
  );
};
