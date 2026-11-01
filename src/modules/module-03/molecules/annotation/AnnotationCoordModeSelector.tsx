import React from "react";
import { Icon } from "@blueprintjs/core";

export type CoordMode = "map" | "manual";

export interface AnnotationCoordModeSelectorProps {
  coordMode: CoordMode;
  onChange: (mode: CoordMode) => void;
  disabled?: boolean;
}

export const AnnotationCoordModeSelector: React.FC<AnnotationCoordModeSelectorProps> = ({
  coordMode,
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
          onClick={() => onChange("map")}
          disabled={disabled}
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
            cursor: disabled ? "not-allowed" : "pointer",
            transition: "all 0.15s ease",
            userSelect: "none",
          }}
        >
          <Icon icon="locate" size={13} color={coordMode === "map" ? "#0f172a" : "#64748b"} />
          Gambar di Peta
        </button>

        <button
          type="button"
          onClick={() => onChange("manual")}
          disabled={disabled}
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
            cursor: disabled ? "not-allowed" : "pointer",
            transition: "all 0.15s ease",
            userSelect: "none",
          }}
        >
          <Icon icon="edit" size={13} color={coordMode === "manual" ? "#0f172a" : "#64748b"} />
          Input Manual
        </button>
      </div>
    </div>
  );
};
