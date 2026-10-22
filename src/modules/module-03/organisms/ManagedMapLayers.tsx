import React from "react";
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

        return null;
      })}
    </>
  );
};
