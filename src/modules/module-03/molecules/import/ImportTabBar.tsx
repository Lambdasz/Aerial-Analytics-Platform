import React from "react";

export interface ImportTabBarProps {
  activeTab: "A" | "B";
  onTabChange: (tab: "A" | "B") => void;
  disabled?: boolean;
}

export const ImportTabBar: React.FC<ImportTabBarProps> = ({
  activeTab,
  onTabChange,
  disabled = false,
}) => {
  return (
    <div
      style={{
        display: "flex",
        background: "#f1f5f9",
        padding: "3px",
        borderRadius: "10px",
        gap: "3px",
        marginBottom: "16px",
      }}
    >
      <button
        type="button"
        disabled={disabled}
        onClick={() => onTabChange("A")}
        style={{
          flex: 1,
          padding: "8px 12px",
          fontSize: "12px",
          fontWeight: activeTab === "A" ? 600 : 500,
          color: activeTab === "A" ? "#0f172a" : "#64748b",
          background: activeTab === "A" ? "#ffffff" : "transparent",
          border: "none",
          borderRadius: "8px",
          boxShadow: activeTab === "A" ? "0 1px 3px rgba(0,0,0,0.08)" : "none",
          cursor: disabled ? "not-allowed" : "pointer",
          transition: "all 0.15s ease",
          outline: "none",
        }}
      >
        Opsi A: Citra Ringan (JPG / PNG)
      </button>
      <button
        type="button"
        disabled={disabled}
        onClick={() => onTabChange("B")}
        style={{
          flex: 1,
          padding: "8px 12px",
          fontSize: "12px",
          fontWeight: activeTab === "B" ? 600 : 500,
          color: activeTab === "B" ? "#0f172a" : "#64748b",
          background: activeTab === "B" ? "#ffffff" : "transparent",
          border: "none",
          borderRadius: "8px",
          boxShadow: activeTab === "B" ? "0 1px 3px rgba(0,0,0,0.08)" : "none",
          cursor: disabled ? "not-allowed" : "pointer",
          transition: "all 0.15s ease",
          outline: "none",
        }}
      >
        Opsi B: GeoTIFF (.tif, .tiff)
      </button>
    </div>
  );
};
