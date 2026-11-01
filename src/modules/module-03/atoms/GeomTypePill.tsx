import React from "react";

export interface GeomTypePillProps {
  geomType: string;
  translateLabel?: boolean;
}

export const GeomTypePill: React.FC<GeomTypePillProps> = ({ geomType, translateLabel = false }) => {
  let label = geomType;
  if (translateLabel) {
    if (geomType === "Point") {
      label = "Titik";
    } else if (geomType === "Line" || geomType === "LineString") {
      label = "Garis";
    } else if (geomType === "Polygon") {
      label = "Poligon";
    }
  }

  return (
    <span
      style={{
        background: "#e2e8f0",
        color: "#334155",
        padding: "2px 8px",
        borderRadius: "6px",
        fontSize: "10px",
        fontWeight: 600,
      }}
    >
      {label}
    </span>
  );
};
