import React from "react";
import type { StoredImageMetadata } from "../../types/storage";
import type { DroneImageMetadata } from "../../types/marker";
import { SafeThumbnail } from "../../atoms/SafeThumbnail";

const formatBytes = (bytes: number): string => {
  if (bytes >= 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }
  return `${(bytes / 1024).toFixed(0)} KB`;
};

export interface RasterPhotoCardProps {
  type: "raster";
  image: StoredImageMetadata;
  thumbnailUrl: string;
  isSelected: boolean;
  hasValidBounds?: boolean;
  onFocus: (image: StoredImageMetadata) => void;
}

export interface DroneMarkerPhotoCardProps {
  type: "marker";
  marker: DroneImageMetadata;
  isSelected: boolean;
  onFocus: (markerId: string, lat: number, lng: number) => void;
}

export type GalleryPhotoCardProps = RasterPhotoCardProps | DroneMarkerPhotoCardProps;

export const GalleryPhotoCard: React.FC<GalleryPhotoCardProps> = (props) => {
  if (props.type === "marker") {
    const { marker, isSelected, onFocus } = props;
    return (
      <div
        id={`gallery-item-${marker.image_id}`}
        className={`gallery-card ${isSelected ? "active" : ""}`}
        role="button"
        tabIndex={0}
        onClick={() => onFocus(marker.image_id, marker.latitude, marker.longitude)}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            onFocus(marker.image_id, marker.latitude, marker.longitude);
          }
        }}
      >
        <div className="gallery-thumb-container">
          <SafeThumbnail src={marker.thumbnail_url} filename={marker.file_name} format="DRONE" />
        </div>
        <div className="gallery-card-info">
          <div className="gallery-card-title" title={marker.file_name}>
            {marker.file_name}
          </div>
          <div className="gallery-card-sub">
            <span>Alt: {marker.altitude_agl.toFixed(0)}m</span>
            <span>•</span>
            <span>Heading: {marker.heading_deg}°</span>
          </div>
          <div className="gallery-focus-hint">Fokus ke titik foto ↗</div>
        </div>
      </div>
    );
  }

  const { image, thumbnailUrl, isSelected, hasValidBounds = false, onFocus } = props;
  const itemKey = image.id || image.filename;
  const isResultImage = Boolean(image.file_path?.includes("result_image"));

  return (
    <div
      id={`gallery-item-${itemKey}`}
      className={`gallery-card ${isSelected ? "active" : ""}`}
      role="button"
      tabIndex={0}
      onClick={() => onFocus(image)}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onFocus(image);
        }
      }}
    >
      <div className="gallery-thumb-container">
        <SafeThumbnail
          src={thumbnailUrl}
          filename={image.filename}
          format={image.format}
          extension={image.extension}
        />
      </div>
      <div className="gallery-card-info">
        <div className="gallery-card-title" title={image.filename}>
          {image.filename}
        </div>
        <div className="gallery-card-sub">
          <span>{formatBytes(image.size_bytes)}</span>
          <span>•</span>
          <span>{isResultImage ? "Hasil Analisis" : "Raster Image"}</span>
        </div>
        {isResultImage ? (
          <div className="gallery-focus-hint" style={{ color: "#007aff" }}>
            Buka Hasil Analisis ↗
          </div>
        ) : hasValidBounds ? (
          <div className="gallery-focus-hint">Fokus ke area peta ↗</div>
        ) : (
          <div className="gallery-focus-hint" style={{ opacity: 0.65 }}>
            Non-spasial (Tanpa koordinat)
          </div>
        )}
      </div>
    </div>
  );
};
