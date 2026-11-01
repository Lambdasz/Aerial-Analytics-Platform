import React from "react";
import { Button, Tag, Slider } from "@blueprintjs/core";
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

  return (
    <div
      style={{
        marginBottom: "16px",
        padding: "16px",
        background: "var(--color-gallery-white)",
        borderRadius: "var(--radius-cards)",
        boxShadow: "var(--shadow-subtle)",
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
        <div>
          <div
            style={{
              fontWeight: 600,
              marginBottom: "4px",
            }}
          >
            {layer.layer_name}
          </div>

          <Tag
            minimal
            round
            style={{
              background: "var(--color-control-gray)",
              color: "var(--color-ink)",
              fontSize: "11px",
              fontWeight: 500,
            }}
          >
            {layer.category}
          </Tag>
        </div>

        <Button
          icon={layer.is_visible ? "eye-open" : "eye-off"}
          text={layer.is_visible ? "ON" : "OFF"}
          minimal={!layer.is_visible}
          style={{
            borderRadius: "var(--radius-buttons)",
            background: layer.is_visible ? "var(--color-control-gray)" : "transparent",
            boxShadow: "none",
            color: layer.is_visible ? "var(--color-apple-blue)" : "var(--color-slate)",
            fontWeight: 600,
          }}
          onClick={() => {
            onToggleVisibility(layer);
          }}
        />
      </div>

      {/* Opacity */}
      <div
        style={{
          marginTop: "10px",
          padding: "0 8px",
        }}
      >
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            marginBottom: "4px",
            fontSize: "12px",
          }}
        >
          <span>Opacity</span>
          <span>{Math.round(layer.opacity * 100)}%</span>
        </div>

        <Slider
          min={0}
          max={1}
          stepSize={0.1}
          labelStepSize={0.5}
          value={layer.opacity}
          onChange={(value) => {
            onOpacityChange(layer, value);
          }}
        />
      </div>
    </div>
  );
};
