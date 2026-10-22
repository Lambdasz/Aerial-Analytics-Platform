import React, { useRef, useEffect } from "react";
import { Marker, Popup } from "react-leaflet";
import type * as L from "leaflet";
import type { DroneImageMetadata } from "../types/marker";
import { ImagePopupCard } from "./ImagePopupCard";
import { useImageSync } from "../store/ImageSyncContext";
import { createDroneDivIcon } from "../lib/leaflet/markerIconBuilder";

interface DroneImageMarkerProps {
  image: DroneImageMetadata;
  opacity: number;
}

export const DroneImageMarker: React.FC<DroneImageMarkerProps> = ({ image, opacity = 1 }) => {
  const { activeImageId, setActiveImage, interactionSource } = useImageSync();
  const markerRef = useRef<L.Marker>(null);

  const isActive = activeImageId === image.image_id;

  useEffect(() => {
    // Buka popup secara programatis jika diaktifkan dari Galeri
    if (isActive && interactionSource === "gallery") {
      markerRef.current?.openPopup();
    }
  }, [isActive, interactionSource]);

  const customIcon = createDroneDivIcon(image.heading_deg, isActive, opacity);

  return (
    <Marker
      ref={markerRef}
      position={[image.latitude, image.longitude]}
      icon={customIcon}
      eventHandlers={{
        click: () => setActiveImage(image.image_id, "map"), // Alur B
      }}
    >
      <Popup>
        <ImagePopupCard image={image} />
      </Popup>
    </Marker>
  );
};
