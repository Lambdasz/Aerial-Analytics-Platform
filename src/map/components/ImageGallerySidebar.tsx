import React, { useEffect, useRef } from "react";
import { Card, Elevation, H5, Spinner } from "@blueprintjs/core";
import { useImageMarkers } from "./useImageMarkers";
import { useImageSync } from "../store/ImageSyncContext";
import "./ImageGallerySidebar.css";

export const ImageGallerySidebar: React.FC = () => {
  const { markers, isLoading } = useImageMarkers(500);
  const { activeImageId, setActiveImage, interactionSource } = useImageSync();
  const listRef = useRef<HTMLDivElement>(null);

  // Alur B: Scroll otomatis jika marker di-klik dari peta
  useEffect(() => {
    if (interactionSource === "map" && activeImageId) {
      const activeElement = document.getElementById(`gallery-item-${activeImageId}`);
      if (activeElement && listRef.current) {
        activeElement.scrollIntoView({ behavior: "smooth", block: "center" });
      }
    }
  }, [activeImageId, interactionSource]);

  return (
    <div className="gallery-sidebar bp5-dark">
      <div className="gallery-header">
        <H5 style={{ margin: 0 }}>Project Gallery</H5>
        <div className="bp5-text-muted">{markers.length} Photos</div>
      </div>

      <div className="gallery-list" ref={listRef}>
        {isLoading ? (
          <Spinner />
        ) : (
          markers.map((img) => (
            <Card
              key={img.image_id}
              id={`gallery-item-${img.image_id}`}
              interactive={true}
              elevation={activeImageId === img.image_id ? Elevation.THREE : Elevation.ZERO}
              className={`gallery-card ${activeImageId === img.image_id ? "active" : ""}`}
              onClick={() => setActiveImage(img.image_id, "gallery")}
            >
              <img src={img.thumbnail_url} alt={img.file_name} loading="lazy" />
              <div className="gallery-card-info">
                <strong>{img.file_name}</strong>
                <span className="bp5-text-muted">Alt: {img.altitude_agl.toFixed(0)}m</span>
              </div>
            </Card>
          ))
        )}
      </div>
    </div>
  );
};
