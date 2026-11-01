import React from "react";
import type { AnnotationGeomType } from "../../types/layer";

export interface AnnotationManualCoordInputsProps {
  geomType: AnnotationGeomType;
  latitude: string;
  longitude: string;
  manualCoordsText: string;
  hasGeometry: boolean;
  geometrySummary?: string;
  disabled?: boolean;
  onLatitudeChange: (lat: string) => void;
  onLongitudeChange: (lng: string) => void;
  onManualCoordsChange: (coords: string) => void;
}

export const AnnotationManualCoordInputs: React.FC<AnnotationManualCoordInputsProps> = ({
  geomType,
  latitude,
  longitude,
  manualCoordsText,
  hasGeometry,
  geometrySummary,
  disabled = false,
  onLatitudeChange,
  onLongitudeChange,
  onManualCoordsChange,
}) => {
  return (
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
                disabled={disabled}
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
                disabled={disabled}
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
              disabled={disabled}
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
  );
};
