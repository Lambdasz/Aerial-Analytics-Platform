import React, { useRef, useEffect } from "react";
import { Marker, Popup } from "react-leaflet";
import * as L from "leaflet";
import type { DroneImageMetadata } from "../types/marker";
import { ImagePopupCard } from "./ImagePopupCard";
import { useImageSync } from "../store/ImageSyncContext";

interface DroneImageMarkerProps {
  image: DroneImageMetadata;
}

export const DroneImageMarker: React.FC<DroneImageMarkerProps> = ({ image }) => {
  const { activeImageId, setActiveImage, interactionSource } = useImageSync();
  const markerRef = useRef<L.Marker>(null);

  const isActive = activeImageId === image.image_id;

  useEffect(() => {
    // Buka popup secara programatis jika diaktifkan dari Galeri
    if (isActive && interactionSource === "gallery") {
      markerRef.current?.openPopup();
    }
  }, [isActive, interactionSource]);

  // SVG Icon pointing to the drone's heading (dengan efek aktif)
  const svgIcon = `
    <svg width="${isActive ? 36 : 24}" height="${isActive ? 36 : 24}" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg" style="transform: rotate(${image.heading_deg}deg); transition: all 0.3s;">
      ${isActive ? '<circle cx="12" cy="12" r="12" fill="rgba(255, 165, 0, 0.4)"/>' : ""}
      <circle cx="12" cy="12" r="10" fill="${isActive ? "#FFA500" : "#2B95D6"}" stroke="white" stroke-width="2"/>
      <path d="M12 4L16 14H8L12 4Z" fill="white"/>
    </svg>
  `;

  const customIcon = L.divIcon({
    className: "custom-drone-marker",
    html: svgIcon,
    iconSize: isActive ? [36, 36] : [24, 24],
    iconAnchor: isActive ? [18, 18] : [12, 12],
    popupAnchor: [0, -15], // popup opens above the marker
  });

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
