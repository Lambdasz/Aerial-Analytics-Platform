import { useState, useEffect } from "react";
import { useMap } from "react-leaflet";

/**
 * Custom hook untuk membaca dan mereaksikan perubahan level zoom peta Leaflet secara real-time.
 * Mendengarkan event Leaflet "zoom" dan "zoomend".
 */
export function useMapZoom(): number {
  const map = useMap();
  const [zoom, setZoom] = useState(() => map.getZoom());

  useEffect(() => {
    const handleZoom = () => {
      setZoom(map.getZoom());
    };

    map.on("zoom", handleZoom);
    map.on("zoomend", handleZoom);

    return () => {
      map.off("zoom", handleZoom);
      map.off("zoomend", handleZoom);
    };
  }, [map]);

  return zoom;
}
