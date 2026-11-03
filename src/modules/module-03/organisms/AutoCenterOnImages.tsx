import { useEffect, useRef } from "react";
import { useMap } from "react-leaflet";
import * as L from "leaflet";
import type { DroneImageMetadata } from "../types/marker";

/**
 * Fits the map to the given session photo markers, once per non-empty array.
 * Removes the "find my imagery somewhere on the globe" friction: the default
 * view is a hardcoded sample location, unrelated to the user's project.
 * Silent no-op while markers are empty (no project / session / GPS data).
 */
export const AutoCenterOnImages: React.FC<{
  markers: ReadonlyArray<DroneImageMetadata>;
}> = ({ markers }) => {
  const map = useMap();
  const didCenter = useRef(false);

  useEffect(() => {
    if (didCenter.current || markers.length === 0) return;
    didCenter.current = true;
    const points = markers.map((m) => [m.latitude, m.longitude] as [number, number]);
    map.fitBounds(L.latLngBounds(points), {
      padding: [60, 60],
      maxZoom: 17,
      animate: true,
      duration: 1.2,
    });
  }, [markers, map]);

  return null;
};
