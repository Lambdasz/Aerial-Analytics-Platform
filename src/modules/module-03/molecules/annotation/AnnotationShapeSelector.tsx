import React from "react";
import { GeomTypePill } from "../../atoms/GeomTypePill";
import type { AnnotationGeomType } from "../../types/layer";

export interface AnnotationShapeSelectorProps {
  geomType: AnnotationGeomType;
  onChange: (type: AnnotationGeomType) => void;
  disabled?: boolean;
}

const SHAPE_OPTIONS: ReadonlyArray<{
  id: AnnotationGeomType;
  label: string;
}> = [
  { id: "Point", label: "Titik" },
  { id: "LineString", label: "Garis" },
  { id: "Polygon", label: "Poligon" },
];

export const AnnotationShapeSelector: React.FC<AnnotationShapeSelectorProps> = ({
  geomType,
  onChange,
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
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
        }}
      >
        <span>Bentuk Anotasi</span>
        <GeomTypePill geomType={geomType} translateLabel />
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
        {SHAPE_OPTIONS.map((item) => {
          const isActive = geomType === item.id;
          return (
            <button
              key={item.id}
              type="button"
              onClick={() => onChange(item.id)}
              disabled={disabled}
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
                cursor: disabled ? "not-allowed" : "pointer",
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
  );
};
