import React from "react";
import { DroneImageMarker } from "./DroneImageMarker";
import type { LayerState } from "../types/layer";
import type { DroneImageMetadata } from "../types/marker";

export interface ManagedMapLayersProps {
  layers: LayerState[];
  markers: ReadonlyArray<DroneImageMetadata>;
  minZoom?: number;
}

/**
 * Renders session photo markers on the marker pane.
 *
 * The old layer-gated branch (`layer_id === "flight_session_01"`) was dead
 * code — no layer entry ever carried that id — so photo markers never showed.
 * They now render straight from the markers array at any zoom: the session
 * extent fits well below the 17+ point-feature threshold, and hiding them
 * right after auto-center would look broken.
 *
 * `layers` / `minZoom` stay in the props for caller compatibility.
 */
export const ManagedMapLayers: React.FC<ManagedMapLayersProps> = ({ markers }) => (
  <>
    {markers.map((marker) => (
      <DroneImageMarker key={marker.image_id} image={marker} opacity={1} />
    ))}
  </>
);
