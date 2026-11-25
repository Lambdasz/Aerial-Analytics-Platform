import React from "react";
import { Button, Icon, Tooltip } from "@blueprintjs/core";
import type { AnnotationGeomType } from "../../types/layer";

export interface AnnotationMapPickControlProps {
  geomType: AnnotationGeomType;
  isPickingLocation: boolean;
  hasGeometry: boolean;
  latitude?: string;
  longitude?: string;
  geometrySummary?: string;
  disabled?: boolean;
  onTogglePickLocation: () => void;
  onClearGeometry: () => void;
}

export const AnnotationMapPickControl: React.FC<AnnotationMapPickControlProps> = ({
  geomType,
  isPickingLocation,
  hasGeometry,
  latitude,
  longitude,
  geometrySummary,
  disabled = false,
  onTogglePickLocation,
  onClearGeometry,
}) => {
  return (
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
          disabled={disabled}
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
                Arahkan kursor dan <strong>klik pada titik di peta</strong> untuk menentukan posisi
                anotasi.
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
  );
};
