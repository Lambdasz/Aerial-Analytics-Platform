import React from "react";

export interface AoiTypeSelectorProps {
  aoiName: string;
  setAoiName: (name: string) => void;
  aoiType: string;
  setAoiType: (type: string) => void;
  disabled?: boolean;
}

export const AoiTypeSelector: React.FC<AoiTypeSelectorProps> = ({
  aoiName,
  setAoiName,
  aoiType,
  setAoiType,
  disabled = false,
}) => {
  return (
    <>
      {/* Input Nama Area */}
      <div style={{ marginBottom: "14px" }}>
        <div
          style={{
            fontSize: "12px",
            fontWeight: 600,
            color: "#64748b",
            marginBottom: "6px",
          }}
        >
          Nama Area <span style={{ color: "#ef4444" }}>*</span>
        </div>
        <input
          value={aoiName}
          onChange={(e) => setAoiName(e.target.value)}
          placeholder="Contoh: Lahan Blok A"
          disabled={disabled}
          style={{
            width: "100%",
            padding: "9px 12px",
            fontSize: "13px",
            borderRadius: "10px",
            border: "none",
            background: "#f8fafc",
            color: "#0f172a",
            outline: "none",
            boxSizing: "border-box",
          }}
        />
      </div>

      {/* Input Tipe Area */}
      <div style={{ marginBottom: "16px" }}>
        <div
          style={{
            fontSize: "12px",
            fontWeight: 600,
            color: "#64748b",
            marginBottom: "6px",
          }}
        >
          Tipe Area <span style={{ fontWeight: 400, color: "#94a3b8" }}>(opsional)</span>
        </div>
        <input
          value={aoiType}
          onChange={(e) => setAoiType(e.target.value)}
          placeholder="Contoh: Kebun / Sektor / Analisis"
          disabled={disabled}
          style={{
            width: "100%",
            padding: "9px 12px",
            fontSize: "13px",
            borderRadius: "10px",
            border: "none",
            background: "#f8fafc",
            color: "#0f172a",
            outline: "none",
            boxSizing: "border-box",
          }}
        />
      </div>
    </>
  );
};
