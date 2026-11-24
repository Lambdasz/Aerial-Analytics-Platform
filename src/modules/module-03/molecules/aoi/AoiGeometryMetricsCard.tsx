import React from "react";
import { Button, Icon } from "@blueprintjs/core";

export interface AoiGeometryMetricsCardProps {
  isEditing: boolean;
  hasGeometry: boolean;
  formattedArea: string;
  formattedPerim: string;
  onStartDrawOnMap: () => void;
  onStartModifyPolygon: () => void;
  disabled?: boolean;
}

export const AoiGeometryMetricsCard: React.FC<AoiGeometryMetricsCardProps> = ({
  isEditing,
  hasGeometry,
  formattedArea,
  formattedPerim,
  onStartDrawOnMap,
  onStartModifyPolygon,
  disabled = false,
}) => {
  return (
    <div style={{ marginBottom: "14px" }}>
      <div
        style={{
          fontSize: "12px",
          fontWeight: 600,
          color: "#64748b",
          marginBottom: "6px",
        }}
      >
        Batas Geometri Poligon
      </div>

      {!isEditing ? (
        /* Mode Buat */
        hasGeometry ? (
          <div
            style={{
              background: "#f0fdf4",
              border: "1px solid #bbf7d0",
              borderRadius: "12px",
              padding: "12px 14px",
              display: "flex",
              alignItems: "center",
              justifyContent: "space-between",
              gap: "10px",
            }}
          >
            <div>
              <div
                style={{
                  fontSize: "12px",
                  fontWeight: 600,
                  color: "#166534",
                  display: "flex",
                  alignItems: "center",
                  gap: "6px",
                }}
              >
                <Icon icon="tick-circle" size={14} color="#166534" />
                <span>Poligon Siap Disimpan</span>
              </div>
              <div style={{ fontSize: "11px", color: "#15803d", marginTop: "2px" }}>
                Luas: <strong>{formattedArea}</strong> • Keliling: <strong>{formattedPerim}</strong>
              </div>
            </div>
            <Button
              minimal
              small
              icon="refresh"
              text="Gambar Ulang"
              onClick={onStartDrawOnMap}
              disabled={disabled}
              style={{
                background: "#ffffff",
                color: "#166534",
                borderRadius: "8px",
                fontWeight: 600,
                fontSize: "11px",
                padding: "4px 10px",
                outline: "none",
                boxShadow: "0 1px 3px rgba(0,0,0,0.06)",
              }}
            />
          </div>
        ) : (
          <div
            style={{
              background: "#f8fafc",
              border: "1px dashed #cbd5e1",
              borderRadius: "12px",
              padding: "16px 14px",
              textAlign: "center",
            }}
          >
            <div style={{ fontSize: "12px", color: "#64748b", marginBottom: "10px" }}>
              Batas poligon area belum ditentukan di atas peta.
            </div>
            <Button
              minimal
              icon="draw"
              text="Gambar Poligon di Peta"
              onClick={onStartDrawOnMap}
              disabled={disabled}
              style={{
                background: "#0f172a",
                color: "#ffffff",
                borderRadius: "8px",
                fontWeight: 600,
                fontSize: "12px",
                padding: "6px 16px",
                outline: "none",
              }}
            />
          </div>
        )
      ) : (
        /* Mode Edit */
        <div
          style={{
            background: "#f8fafc",
            border: "1px solid #f1f5f9",
            borderRadius: "12px",
            padding: "12px 14px",
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            gap: "10px",
          }}
        >
          <div>
            <div style={{ fontSize: "12px", fontWeight: 600, color: "#334155" }}>
              Batas Geometri Terpasang
            </div>
            <div style={{ fontSize: "11px", color: "#64748b", marginTop: "2px" }}>
              Luas: <strong>{formattedArea}</strong> • Keliling: <strong>{formattedPerim}</strong>
            </div>
          </div>
          <Button
            minimal
            small
            icon="polygon-filter"
            text="Ubah Bentuk di Peta"
            onClick={onStartModifyPolygon}
            disabled={disabled}
            style={{
              background: "#0f172a",
              color: "#ffffff",
              borderRadius: "8px",
              fontWeight: 600,
              fontSize: "11px",
              padding: "6px 12px",
              outline: "none",
            }}
          />
        </div>
      )}
    </div>
  );
};
