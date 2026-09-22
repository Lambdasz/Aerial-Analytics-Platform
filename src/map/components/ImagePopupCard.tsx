import React from "react";
import { Card, Button, H6, Divider } from "@blueprintjs/core";
import type { DroneImageMetadata } from "../types/marker";
import "./ImagePopupCard.css";

interface ImagePopupCardProps {
  image: DroneImageMetadata;
}

export const ImagePopupCard: React.FC<ImagePopupCardProps> = ({ image }) => {
  return (
    <div className="glass-popup-card">
      <Card>
        <img
          src={image.thumbnail_url}
          alt={image.file_name}
          className="popup-thumbnail"
          loading="lazy"
        />
        <H6 style={{ margin: 0, color: "#182026" }}>{image.file_name}</H6>

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

        <Divider style={{ margin: "8px 0" }} />

        <div className="popup-actions">
          <Button icon="zoom-to-fit" small intent="primary" text="Fokus" />
          <Button icon="media" small text="Galeri" />
        </div>
      </Card>
    </div>
  );
};
