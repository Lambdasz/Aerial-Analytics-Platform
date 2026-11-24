import React from "react";

export interface LayerOpacitySliderProps {
  opacity: number;
  isVisible: boolean;
  onChange: (opacity: number) => void;
}

export const LayerOpacitySlider: React.FC<LayerOpacitySliderProps> = ({
  opacity,
  isVisible,
  onChange,
}) => {
  return (
    <div
      style={{
        marginTop: "12px",
        paddingTop: "10px",
        borderTop: "1px solid #f1f5f9",
        opacity: isVisible ? 1 : 0.55,
        transition: "opacity 0.2s ease",
      }}
    >
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          marginBottom: "6px",
          fontSize: "11px",
          color: "#64748b",
        }}
      >
        <span style={{ fontWeight: 500 }}>Opacity</span>
        <span
          style={{
            fontSize: "11px",
            fontWeight: 600,
            color: "#0f172a",
            background: "#f1f5f9",
            padding: "2px 6px",
            borderRadius: "6px",
          }}
        >
          {Math.round(opacity * 100)}%
        </span>
      </div>

      <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
        <input
          type="range"
          min="0"
          max="1"
          step="0.05"
          value={opacity}
          onChange={(e) => {
            onChange(parseFloat(e.target.value));
          }}
          style={{
            width: "100%",
            height: "4px",
            borderRadius: "999px",
            background: `linear-gradient(to right, #0f172a ${opacity * 100}%, #e2e8f0 ${opacity * 100}%)`,
            outline: "none",
            cursor: "pointer",
            appearance: "none",
            WebkitAppearance: "none",
            accentColor: "#0f172a",
          }}
        />
      </div>
    </div>
  );
};
