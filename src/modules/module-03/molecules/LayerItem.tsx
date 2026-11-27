import React from "react";
import type { LayerState } from "../types/layer";
import { TactileToggle } from "../atoms/TactileToggle";
import { LayerOpacitySlider } from "./LayerOpacitySlider";

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

        <TactileToggle
          isChecked={layer.is_visible}
          onToggle={() => {
            onToggleVisibility(layer);
          }}
        />
      </div>

      {/* Opacity */}
      <LayerOpacitySlider
        opacity={layer.opacity}
        isVisible={layer.is_visible}
        onChange={(newOpacity) => {
          onOpacityChange(layer, newOpacity);
        }}
      />
    </div>
  );
};
