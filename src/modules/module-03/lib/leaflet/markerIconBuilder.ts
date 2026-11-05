import * as L from "leaflet";
import { createDroneMarkerSvg } from "../markerSvg";

/**
 * Membangun instance Leaflet DivIcon untuk marker foto drone.
 *
 * Deterministik, membuat objek mutable Leaflet (L.divIcon).
 */
export function createDroneDivIcon(
  headingDeg: number,
  isActive: boolean,
  opacity: number = 1,
): L.DivIcon {
  const svgIcon = createDroneMarkerSvg(headingDeg, isActive, opacity);

  return L.divIcon({
    className: "custom-drone-marker",
    html: svgIcon,
    iconSize: isActive ? [36, 36] : [24, 24],
    iconAnchor: isActive ? [18, 18] : [12, 12],
    popupAnchor: [0, -15],
  });
}
