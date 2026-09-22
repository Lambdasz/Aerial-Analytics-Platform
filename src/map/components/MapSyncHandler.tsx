import React, { useEffect } from "react";
import { useMap } from "react-leaflet";
import { useImageSync } from "../store/ImageSyncContext";
import { useImageMarkers } from "./useImageMarkers";

export const MapSyncHandler: React.FC = () => {
  const map = useMap();
  const { activeImageId, interactionSource } = useImageSync();
  const { markers } = useImageMarkers(500); // Reuse dummy markers hook

  useEffect(() => {
    // Alur A: Peta bergeser saat foto di-klik di sidebar
    if (interactionSource === "gallery" && activeImageId) {
      const targetImage = markers.find((m) => m.image_id === activeImageId);
      if (targetImage) {
        map.flyTo([targetImage.latitude, targetImage.longitude], 19, {
          duration: 1.5,
          animate: true,
        });
      }
    }
  }, [activeImageId, interactionSource, map, markers]);

  return null; // Komponen invisible
};
