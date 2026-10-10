import React from "react";
import { Icon } from "@blueprintjs/core";
import { useMap } from "react-leaflet";
import { useImageSync } from "../store/ImageSyncContext";
import type { DroneImageMetadata } from "../types/marker";
import "./ImagePopupCard.css";

interface ImagePopupCardProps {
  image: DroneImageMetadata;
}

export const ImagePopupCard: React.FC<ImagePopupCardProps> = ({ image }) => {
  const map = useMap();
  const { setActiveImage } = useImageSync();

  const handleFocus = () => {
    map.flyTo([image.latitude, image.longitude], 20, { duration: 1.5 });
  };

  const handleOpenGallery = () => {
    setActiveImage(image.image_id, "map");
    window.dispatchEvent(new CustomEvent("open-gallery-tab"));
  };

  return (
    <div className="glass-popup-card">
      <img
        src={image.thumbnail_url}
        alt={image.file_name}
        className="popup-thumbnail"
        loading="lazy"
      />
      <div
        style={{
          fontSize: "14px",
          fontWeight: 600,
          color: "var(--color-ink)",
          marginBottom: "8px",
        }}
      >
        {image.file_name}
      </div>

      <div className="popup-telemetry">
        <span className="telemetry-label">Lat/Lon:</span>
        <span className="telemetry-value">
          {image.latitude.toFixed(4)}, {image.longitude.toFixed(4)}
        </span>

        <span className="telemetry-label">Ketinggian:</span>
        <span className="telemetry-value">{image.altitude_agl.toFixed(1)}m AGL</span>

        <span className="telemetry-label">Arah Drone:</span>
        <span className="telemetry-value">{image.heading_deg}°</span>
      </div>

      <div style={{ margin: "12px 0", borderBottom: "1px solid var(--color-hairline-silver)" }} />

      <div className="popup-actions">
        <div
          role="button"
          tabIndex={0}
          className="apple-popup-button primary"
          onClick={handleFocus}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              handleFocus();
            }
          }}
        >
          <Icon icon="zoom-to-fit" size={14} />
          <span>Fokus</span>
        </div>
        <div
          role="button"
          tabIndex={0}
          className="apple-popup-button secondary"
          onClick={handleOpenGallery}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              handleOpenGallery();
            }
          }}
        >
          <Icon icon="media" size={14} />
          <span>Galeri</span>
        </div>
      </div>
    </div>
  );
};
