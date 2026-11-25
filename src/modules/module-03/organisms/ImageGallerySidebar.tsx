import React, { useEffect, useRef } from "react";
import type * as L from "leaflet";
import { Button, NonIdealState, Spinner } from "@blueprintjs/core";
import type { DroneImageMetadata } from "../types/marker";
import { isValidWgs84Bounds } from "../lib/leaflet/spatialLayerBuilder";
import { ImportImageModal } from "./dialogs/ImportImageModal";
import { OrthoCardItem } from "../molecules/OrthoCardItem";
import { EditOrthoModal } from "./dialogs/EditOrthoModal";
import { DeleteOrthoAlert } from "./dialogs/DeleteOrthoAlert";
import { GalleryFilterBar } from "../molecules/gallery/GalleryFilterBar";
import { GalleryPhotoCard } from "../molecules/gallery/GalleryPhotoCard";
import { useGalleryImages } from "../hooks/useGalleryImages";
import "../styles/ImageGallerySidebar.css";

export interface ImageGallerySidebarProps {
  map?: L.Map | null;
  /** Session photo markers (shared with the map). */
  markers?: ReadonlyArray<DroneImageMetadata>;
}

export const ImageGallerySidebar: React.FC<ImageGallerySidebarProps> = ({ map, markers = [] }) => {
  const gallery = useGalleryImages({ map, markers });
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (gallery.interactionSource === "map" && gallery.activeImageId) {
      const activeElement = document.getElementById(`gallery-item-${gallery.activeImageId}`);
      if (activeElement && listRef.current) {
        activeElement.scrollIntoView({ behavior: "smooth", block: "center" });
      }
    }
  }, [gallery.activeImageId, gallery.interactionSource]);

  return (
    <div className="gallery-sidebar">
      {/* UNIFIED PAGE HEADER (OPSI A) */}
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          paddingBottom: "8px",
          borderBottom: "1px solid rgba(16, 22, 26, 0.08)",
          marginBottom: "12px",
        }}
      >
        <div>
          <div style={{ fontSize: "14px", fontWeight: 700, color: "#0f172a" }}>Project Gallery</div>
          <div style={{ fontSize: "11px", color: "#64748b", fontWeight: 500 }}>
            {gallery.totalCount} Citra Tersimpan
          </div>
        </div>
        <Button
          icon="import"
          text="Impor"
          small
          minimal
          onClick={() => gallery.setIsImportModalOpen(true)}
          style={{
            borderRadius: "8px",
            background: "#0f172a",
            color: "#ffffff",
            fontWeight: 600,
            padding: "5px 12px",
            fontSize: "11px",
            boxShadow: "none",
            outline: "none",
          }}
        />
      </div>

      {/* FILTER PILLS */}
      <GalleryFilterBar
        activeFilter={gallery.activeFilter}
        onSelectFilter={gallery.setActiveFilter}
        totalCount={gallery.totalCount}
        orthoCount={gallery.orthoImages.length}
        rasterCount={gallery.rasterImages.length + markers.length}
      />

      <div className="gallery-list" ref={listRef}>
        {gallery.isImagesLoading ? (
          <div style={{ display: "flex", justifyContent: "center", padding: "40px" }}>
            <Spinner />
          </div>
        ) : gallery.totalCount === 0 ? (
          <div style={{ padding: "40px 16px" }}>
            <NonIdealState
              icon="media"
              title="Belum Ada Citra Tersimpan"
              description="Penyimpanan masih kosong. Impor citra orthomosaic (JPG/PNG) dengan koordinat area WGS84 untuk memulai eksplorasi peta."
              action={
                <Button
                  intent="primary"
                  icon="import"
                  text="Impor Citra Ortho (JPG/PNG)"
                  onClick={() => gallery.setIsImportModalOpen(true)}
                />
              }
            />
          </div>
        ) : (
          <>
            {/* 1: FOTO ORTHOMOSAIC DRONE */}
            {(gallery.activeFilter === "all" || gallery.activeFilter === "ortho") &&
              gallery.orthoImages.length > 0 && (
                <div className="gallery-section">
                  <div className="gallery-section-header">
                    <div className="gallery-section-title">Foto Orthomosaic Drone</div>
                    <span className="gallery-section-badge">{gallery.orthoImages.length}</span>
                  </div>

                  {gallery.orthoImages.map((img) => {
                    const itemKey = img.id || img.filename;
                    const isSelected =
                      gallery.selectedId === itemKey || gallery.activeImageId === itemKey;
                    return (
                      <OrthoCardItem
                        key={itemKey}
                        img={img}
                        thumbnailUrl={gallery.resolveThumbnailUrl(img)}
                        isSelected={isSelected}
                        isProcessing={gallery.isActionProcessing}
                        onFocus={gallery.handleFocusOrtho}
                        onEdit={(target) => gallery.setEditingImage(target)}
                        onDelete={gallery.handleOpenDelete}
                      />
                    );
                  })}
                </div>
              )}

            {/* 2: FOTO RASTER BIASA & DRONE CAPTURES */}
            {(gallery.activeFilter === "all" || gallery.activeFilter === "raster") && (
              <div className="gallery-section">
                <div className="gallery-section-header">
                  <div className="gallery-section-title">Foto Raster (Hasil Analisis)</div>
                  <span className="gallery-section-badge">
                    {gallery.rasterImages.length + markers.length}
                  </span>
                </div>

                {gallery.rasterImages.map((img) => {
                  const itemKey = img.id || img.filename;
                  const isSelected =
                    gallery.selectedId === itemKey || gallery.activeImageId === itemKey;
                  const thumbSrc = gallery.resolveThumbnailUrl(img);
                  return (
                    <GalleryPhotoCard
                      key={itemKey}
                      type="raster"
                      image={img}
                      thumbnailUrl={thumbSrc}
                      isSelected={isSelected}
                      hasValidBounds={Boolean(img.bounds && isValidWgs84Bounds(img.bounds))}
                      onFocus={gallery.handleFocusRaster}
                    />
                  );
                })}

                {/* Drone Flight Capture Markers */}
                {markers.map((img) => {
                  const isSelected =
                    gallery.selectedId === img.image_id || gallery.activeImageId === img.image_id;
                  return (
                    <GalleryPhotoCard
                      key={img.image_id}
                      type="marker"
                      marker={img}
                      isSelected={isSelected}
                      onFocus={gallery.handleFocusMarker}
                    />
                  );
                })}
              </div>
            )}
          </>
        )}
      </div>

      <ImportImageModal
        isOpen={gallery.isImportModalOpen}
        onClose={() => gallery.setIsImportModalOpen(false)}
        onImportSuccess={gallery.handleImportSuccess}
        map={map}
      />

      <EditOrthoModal
        isOpen={Boolean(gallery.editingImage)}
        onClose={() => gallery.setEditingImage(null)}
        image={gallery.editingImage}
        getCurrentMapBounds={gallery.getCurrentMapBounds}
        onSave={gallery.handleSaveOrtho}
        isSubmitting={gallery.isActionProcessing}
      />

      <DeleteOrthoAlert
        isOpen={Boolean(gallery.deletingImage)}
        onClose={() => gallery.setDeletingImage(null)}
        image={gallery.deletingImage}
        summary={gallery.cascadeSummary}
        isLoadingSummary={gallery.isLoadingSummary}
        isProcessing={gallery.isActionProcessing}
        onConfirm={() => {
          void gallery.handleConfirmDelete();
        }}
      />
    </div>
  );
};
