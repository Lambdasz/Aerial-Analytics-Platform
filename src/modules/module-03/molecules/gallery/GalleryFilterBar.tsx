import React from "react";

export type GalleryFilterMode = "all" | "ortho" | "raster";

export interface GalleryFilterBarProps {
  activeFilter: GalleryFilterMode;
  onSelectFilter: (filter: GalleryFilterMode) => void;
  totalCount: number;
  orthoCount: number;
  rasterCount: number;
}

export const GalleryFilterBar: React.FC<GalleryFilterBarProps> = ({
  activeFilter,
  onSelectFilter,
  totalCount,
  orthoCount,
  rasterCount,
}) => {
  return (
    <div className="gallery-filter-bar">
      <button
        type="button"
        className={`gallery-filter-pill ${activeFilter === "all" ? "active" : ""}`}
        onClick={() => onSelectFilter("all")}
      >
        Semua ({totalCount})
      </button>
      <button
        type="button"
        className={`gallery-filter-pill ${activeFilter === "ortho" ? "active" : ""}`}
        onClick={() => onSelectFilter("ortho")}
      >
        Orthomosaic ({orthoCount})
      </button>
      <button
        type="button"
        className={`gallery-filter-pill ${activeFilter === "raster" ? "active" : ""}`}
        onClick={() => onSelectFilter("raster")}
      >
        Foto Raster ({rasterCount})
      </button>
    </div>
  );
};
