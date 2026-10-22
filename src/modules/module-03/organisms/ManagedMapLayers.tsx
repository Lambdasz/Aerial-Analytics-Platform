import React from "react";
import { Polygon } from "react-leaflet";
import { DroneImageMarker } from "./DroneImageMarker";
import type { LayerState } from "../types/layer";
import type { DroneImageMetadata } from "../types/marker";

export interface ManagedMapLayersProps {
  layers: LayerState[];
  markers: ReadonlyArray<DroneImageMetadata>;
}

export const ManagedMapLayers: React.FC<ManagedMapLayersProps> = ({ layers, markers }) => {
  const sortedLayers = [...layers].sort((a, b) => a.z_index - b.z_index);

  return (
    <>
      {sortedLayers.map((layer) => {
        if (!layer.is_visible) {
          return null;
        }

        // =========================
        // DRONE IMAGERY
        // =========================
        if (layer.layer_id === "flight_session_01") {
          return (
            <React.Fragment key={layer.layer_id}>
              {markers.map((marker) => (
                <DroneImageMarker
                  key={`${layer.layer_id}-${marker.image_id}`}
                  image={marker}
                  opacity={layer.opacity}
                />
              ))}
            </React.Fragment>
          );
        }

        // =========================
        // VEGETATION MASK (sisa data template, dipertahankan sesuai temuan)
        // =========================
        if (layer.layer_id === "plugin_veg_mask_01") {
          return (
            <Polygon
              key={layer.layer_id}
              positions={[
                [-1.221, 116.8575],
                [-1.2208, 116.8582],
                [-1.2212, 116.859],
                [-1.222, 116.8592],
                [-1.2228, 116.8586],
                [-1.2225, 116.8578],
                [-1.2218, 116.857],
                [-1.221, 116.8575],
              ]}
              pathOptions={{
                opacity: layer.opacity,
                fillOpacity: layer.opacity,
              }}
            />
          );
        }

        // =========================
        // TREE DETECTION (sisa data template, dipertahankan sesuai temuan)
        // =========================
        if (layer.layer_id === "plugin_tree_detect_01") {
          return (
            <Polygon
              key={layer.layer_id}
              positions={[
                [-1.2212, 116.8595],
                [-1.221, 116.8605],
                [-1.2216, 116.861],
                [-1.2225, 116.8612],
                [-1.2232, 116.8608],
                [-1.223, 116.86],
                [-1.2222, 116.8592],
                [-1.2212, 116.8595],
              ]}
              pathOptions={{
                opacity: layer.opacity,
                fillOpacity: layer.opacity,
              }}
            />
          );
        }

        return null;
      })}
    </>
  );
};
