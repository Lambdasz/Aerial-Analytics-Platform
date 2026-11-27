import React from "react";
import { Icon } from "@blueprintjs/core";

export interface OrthoBoundingBoxInputsProps {
  westStr: string;
  setWestStr: (val: string) => void;
  southStr: string;
  setSouthStr: (val: string) => void;
  eastStr: string;
  setEastStr: (val: string) => void;
  northStr: string;
  setNorthStr: (val: string) => void;
  westError?: string | null;
  southError?: string | null;
  eastError?: string | null;
  northError?: string | null;
  disabled?: boolean;
  onUseCurrentView?: () => void;
  useCurrentViewLabel?: string;
  title?: string;
  hideHeader?: boolean;
  headerRightActions?: React.ReactNode;
  containerStyle?: React.CSSProperties;
  children?: React.ReactNode;
}

export const OrthoBoundingBoxInputs: React.FC<OrthoBoundingBoxInputsProps> = ({
  westStr,
  setWestStr,
  southStr,
  setSouthStr,
  eastStr,
  setEastStr,
  northStr,
  setNorthStr,
  westError,
  southError,
  eastError,
  northError,
  disabled = false,
  onUseCurrentView,
  useCurrentViewLabel = "Gunakan Area Peta Saat Ini",
  title = "Koordinat Bounding Box (WGS84)",
  hideHeader = false,
  headerRightActions,
  containerStyle,
  children,
}) => {
  return (
    <div style={{ marginTop: hideHeader ? undefined : "16px", ...containerStyle }}>
      {!hideHeader && (
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
            marginBottom: "8px",
          }}
        >
          <div style={{ fontSize: "12px", fontWeight: 600, color: "#64748b" }}>{title}</div>
          <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
            {headerRightActions}
            {onUseCurrentView && (
              <button
                type="button"
                disabled={disabled}
                onClick={onUseCurrentView}
                style={{
                  padding: "4px 10px",
                  fontSize: "11px",
                  fontWeight: 600,
                  borderRadius: "999px",
                  border: "none",
                  background: "#f1f5f9",
                  color: "#0f172a",
                  cursor: disabled ? "not-allowed" : "pointer",
                  display: "flex",
                  alignItems: "center",
                  gap: "4px",
                }}
              >
                <Icon icon="locate" size={12} color="#0f172a" />
                <span>{useCurrentViewLabel}</span>
              </button>
            )}
          </div>
        </div>
      )}

      <div
        style={{
          display: "grid",
          gridTemplateColumns: "1fr 1fr",
          gap: "10px",
        }}
      >
        <div>
          <div
            style={{
              fontSize: "11px",
              fontWeight: 600,
              color: southError ? "#ef4444" : "#64748b",
              marginBottom: "4px",
            }}
          >
            South (Lat Min °)
          </div>
          <input
            id="bbox-south"
            type="text"
            inputMode="decimal"
            placeholder="-1.225000"
            value={southStr}
            disabled={disabled}
            onChange={(e) => setSouthStr(e.target.value)}
            style={{
              width: "100%",
              padding: "8px 12px",
              fontSize: "12px",
              background: "#f8fafc",
              border: southError ? "1px solid #fca5a5" : "none",
              borderRadius: "10px",
              outline: "none",
              color: "#0f172a",
              boxSizing: "border-box",
            }}
          />
          {southError && (
            <div style={{ fontSize: "10px", color: "#ef4444", marginTop: "3px" }}>{southError}</div>
          )}
        </div>

        <div>
          <div
            style={{
              fontSize: "11px",
              fontWeight: 600,
              color: northError ? "#ef4444" : "#64748b",
              marginBottom: "4px",
            }}
          >
            North (Lat Max °)
          </div>
          <input
            id="bbox-north"
            type="text"
            inputMode="decimal"
            placeholder="-1.218000"
            value={northStr}
            disabled={disabled}
            onChange={(e) => setNorthStr(e.target.value)}
            style={{
              width: "100%",
              padding: "8px 12px",
              fontSize: "12px",
              background: "#f8fafc",
              border: northError ? "1px solid #fca5a5" : "none",
              borderRadius: "10px",
              outline: "none",
              color: "#0f172a",
              boxSizing: "border-box",
            }}
          />
          {northError && (
            <div style={{ fontSize: "10px", color: "#ef4444", marginTop: "3px" }}>{northError}</div>
          )}
        </div>

        <div>
          <div
            style={{
              fontSize: "11px",
              fontWeight: 600,
              color: westError ? "#ef4444" : "#64748b",
              marginBottom: "4px",
            }}
          >
            West (Lon Min °)
          </div>
          <input
            id="bbox-west"
            type="text"
            inputMode="decimal"
            placeholder="116.856000"
            value={westStr}
            disabled={disabled}
            onChange={(e) => setWestStr(e.target.value)}
            style={{
              width: "100%",
              padding: "8px 12px",
              fontSize: "12px",
              background: "#f8fafc",
              border: westError ? "1px solid #fca5a5" : "none",
              borderRadius: "10px",
              outline: "none",
              color: "#0f172a",
              boxSizing: "border-box",
            }}
          />
          {westError && (
            <div style={{ fontSize: "10px", color: "#ef4444", marginTop: "3px" }}>{westError}</div>
          )}
        </div>

        <div>
          <div
            style={{
              fontSize: "11px",
              fontWeight: 600,
              color: eastError ? "#ef4444" : "#64748b",
              marginBottom: "4px",
            }}
          >
            East (Lon Max °)
          </div>
          <input
            id="bbox-east"
            type="text"
            inputMode="decimal"
            placeholder="116.865000"
            value={eastStr}
            disabled={disabled}
            onChange={(e) => setEastStr(e.target.value)}
            style={{
              width: "100%",
              padding: "8px 12px",
              fontSize: "12px",
              background: "#f8fafc",
              border: eastError ? "1px solid #fca5a5" : "none",
              borderRadius: "10px",
              outline: "none",
              color: "#0f172a",
              boxSizing: "border-box",
            }}
          />
          {eastError && (
            <div style={{ fontSize: "10px", color: "#ef4444", marginTop: "3px" }}>{eastError}</div>
          )}
        </div>

        {children && <div style={{ gridColumn: "1 / -1" }}>{children}</div>}
      </div>
    </div>
  );
};
