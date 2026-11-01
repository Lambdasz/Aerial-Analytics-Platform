import React from "react";
import { Button } from "@blueprintjs/core";
import type { LayerState } from "../types/layer";

export interface LayerItemProps {
  layer: LayerState;
  onToggleVisibility: (layer: LayerState) => void;
  onOpacityChange: (layer: LayerState, opacity: number) => void;
}

export const LayerItem: React.FC<LayerItemProps> = ({
  layer,
  onToggleVisibility,
  onOpacityChange,
}) => {
  if (
    layer?.category === "Base Maps" ||
    layer?.layer_id === "base_osm" ||
    layer?.layer_name?.toLowerCase().includes("base map")
  ) {
    return null;
  }

  // Format layer name: shorten raw hexadecimal IDs (e.g. 5d2bcc3ff416f40006cffca8 -> Raster 5d2bcc...)
  const isHexId = /^[a-f0-9]{20,}$/i.test(layer.layer_name);
  const displayName = isHexId ? `Raster (${layer.layer_name.slice(0, 8)}...)` : layer.layer_name;

  return (
    <div
      style={{
        marginBottom: "12px",
        padding: "16px",
        background: "#ffffff",
        borderRadius: "16px",
        border: "none",
        boxShadow: "0 2px 12px -2px rgba(0, 0, 0, 0.05), 0 1px 3px rgba(0, 0, 0, 0.02)",
        transition: "all 0.2s cubic-bezier(0.4, 0, 0.2, 1)",
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          gap: "10px",
        }}
      >
        <div style={{ minWidth: 0, flex: 1 }}>
          <div
            style={{
              fontWeight: 600,
              fontSize: "14px",
              color: "#0f172a",
              whiteSpace: "nowrap",
              overflow: "hidden",
              textOverflow: "ellipsis",
              marginBottom: "4px",
            }}
            title={layer.layer_name}
          >
            {displayName}
          </div>

          <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
            <span
              style={{
                fontSize: "11px",
                fontWeight: 500,
                color: "#64748b",
              }}
            >
              {layer.category}
            </span>
          </div>
        </div>

        <Button
          text={layer.is_visible ? "ON" : "OFF"}
          small
          minimal
          style={{
            borderRadius: "999px",
            fontWeight: 600,
            fontSize: "11px",
            padding: "4px 12px",
            background: layer.is_visible ? "#0f172a" : "#f1f5f9",
            color: layer.is_visible ? "#ffffff" : "#64748b",
            boxShadow: "none",
          }}
          onClick={() => {
            onToggleVisibility(layer);
          }}
        />
      </div>

      {/* Opacity */}
      <div
        style={{
          marginTop: "12px",
          paddingTop: "10px",
          borderTop: "1px solid #f1f5f9",
          opacity: layer.is_visible ? 1 : 0.55,
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
            {Math.round(layer.opacity * 100)}%
          </span>
        </div>

        <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
          <input
            type="range"
            min="0"
            max="1"
            step="0.05"
            value={layer.opacity}
            onChange={(e) => {
              onOpacityChange(layer, parseFloat(e.target.value));
            }}
            style={{
              width: "100%",
              height: "4px",
              borderRadius: "999px",
              background: `linear-gradient(to right, #0f172a ${layer.opacity * 100}%, #e2e8f0 ${layer.opacity * 100}%)`,
              outline: "none",
              cursor: "pointer",
              appearance: "none",
              WebkitAppearance: "none",
              accentColor: "#0f172a",
            }}
          />
        </div>
      </div>
    </div>
  );
};
