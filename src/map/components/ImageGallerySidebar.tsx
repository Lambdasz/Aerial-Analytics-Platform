import React, { useEffect, useRef } from "react";
import { Spinner } from "@blueprintjs/core";
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
    <div className="gallery-sidebar">
      <div className="gallery-header">
        <div
          style={{
            fontSize: "14px",
            fontWeight: 600,
            color: "var(--color-ink)",
            marginBottom: "4px",
          }}
        >
          Project Gallery
        </div>
        <div style={{ fontSize: "12px", color: "var(--color-slate)" }}>{markers.length} Photos</div>
      </div>

      <div className="gallery-list" ref={listRef}>
        {isLoading ? (
          <Spinner />
        ) : (
          markers.map((img) => (
            <div
              key={img.image_id}
              id={`gallery-item-${img.image_id}`}
              className={`gallery-card ${activeImageId === img.image_id ? "active" : ""}`}
              role="button"
              tabIndex={0}
              onClick={() => setActiveImage(img.image_id, "gallery")}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  setActiveImage(img.image_id, "gallery");
                }
              }}
            >
              <img src={img.thumbnail_url} alt={img.file_name} loading="lazy" />
              <div className="gallery-card-info">
                <strong>{img.file_name}</strong>
                <span>Alt: {img.altitude_agl.toFixed(0)}m</span>
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  );
};
