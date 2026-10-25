import React, { useEffect, useRef, useState, useMemo, useCallback } from "react";
import * as L from "leaflet";
import { Button, Icon, NonIdealState, Spinner } from "@blueprintjs/core";
import { useImageSync } from "../store/ImageSyncContext";
import {
  listOrthoImages,
  deleteOrthoImage,
  getOrthoCascadeSummary,
  updateOrthoImage,
} from "../services/api";
import type {
  DeleteOrthoSummary,
  StoredImageMetadata,
  UpdateOrthoImageParams,
} from "../types/storage";
import type { DroneImageMetadata } from "../types/marker";
import { isValidWgs84Bounds, resolveImageOverlayUrl } from "../lib/leaflet/spatialLayerBuilder";
import { ImportImageModal } from "./ImportImageModal";
import { OrthoCardItem } from "../molecules/OrthoCardItem";
import { EditOrthoModal } from "../molecules/EditOrthoModal";
import { DeleteOrthoAlert } from "../molecules/DeleteOrthoAlert";
import { showToast } from "../lib/toaster";
import "../styles/ImageGallerySidebar.css";

const EMPTY_MARKERS: ReadonlyArray<DroneImageMetadata> = [];

interface RasterThumbProps {
  src?: string;
  filename: string;
  format?: string;
  extension?: string;
}

const RasterThumb: React.FC<RasterThumbProps> = ({ src, filename, format, extension }) => {
  const [hasError, setHasError] = useState(false);

  if (!src || hasError) {
    return (
      <div
        style={{
          width: "100%",
          height: "100%",
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          justifyContent: "center",
          background: "rgba(143, 153, 168, 0.12)",
          color: "#8a9ba8",
        }}
        title="Tidak ada pratinjau citra"
      >
        <Icon icon="media" size={22} />
        <span style={{ fontSize: "9px", marginTop: "2px", fontWeight: 500 }}>No Image</span>
      </div>
    );
  }

  return (
    <>
      <img src={src} alt={filename} loading="lazy" onError={() => setHasError(true)} />
      <span className="gallery-thumb-tag">{(extension || format || "IMG").toUpperCase()}</span>
    </>
  );
};

export interface ImageGallerySidebarProps {
  map?: L.Map | null;
}

export const ImageGallerySidebar: React.FC<ImageGallerySidebarProps> = ({ map }) => {
  const markers = EMPTY_MARKERS;
  const { activeImageId, setActiveImage, interactionSource } = useImageSync();
  const [storedImages, setStoredImages] = useState<StoredImageMetadata[]>([]);
  const [isImagesLoading, setIsImagesLoading] = useState(true);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [activeFilter, setActiveFilter] = useState<"all" | "ortho" | "raster">("all");
  const [isImportModalOpen, setIsImportModalOpen] = useState(false);
  const [editingImage, setEditingImage] = useState<StoredImageMetadata | null>(null);
  const [deletingImage, setDeletingImage] = useState<StoredImageMetadata | null>(null);
  const [cascadeSummary, setCascadeSummary] = useState<DeleteOrthoSummary | null>(null);
  const [isLoadingSummary, setIsLoadingSummary] = useState(false);
  const [isActionProcessing, setIsActionProcessing] = useState(false);
  const listRef = useRef<HTMLDivElement>(null);

  const fetchImages = useCallback(() => {
    listOrthoImages()
      .then((data) => {
        setStoredImages(data || []);
        setIsImagesLoading(false);
      })
      .catch((err) => {
        console.error("Gagal memuat citra tersimpan:", err);
        setIsImagesLoading(false);
      });
  }, []);

  useEffect(() => {
    // Safety timeout: jika dalam 3 detik backend tidak merespon, hentikan spinner
    const timer = setTimeout(() => {
      setIsImagesLoading(false);
    }, 3000);

    fetchImages();

    const handleExternalChange = () => {
      fetchImages();
    };

    window.addEventListener("ortho_image:saved", handleExternalChange);
    window.addEventListener("ortho_image:deleted", handleExternalChange);

    return () => {
      clearTimeout(timer);
      window.removeEventListener("ortho_image:saved", handleExternalChange);
      window.removeEventListener("ortho_image:deleted", handleExternalChange);
    };
  }, [fetchImages]);

  // Alur B: Scroll otomatis jika marker di-klik dari peta
  useEffect(() => {
    if (interactionSource === "map" && activeImageId) {
      const activeElement = document.getElementById(`gallery-item-${activeImageId}`);
      if (activeElement && listRef.current) {
        activeElement.scrollIntoView({ behavior: "smooth", block: "center" });
      }
    }
  }, [activeImageId, interactionSource]);

  const orthoImages = useMemo(
    () => storedImages.filter((img) => img.category === "ortho"),
    [storedImages],
  );

  const rasterImages = useMemo(
    () => storedImages.filter((img) => img.category === "raster"),
    [storedImages],
  );

  const formatBytes = (bytes: number): string => {
    if (bytes >= 1024 * 1024) {
      return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    }
    return `${(bytes / 1024).toFixed(0)} KB`;
  };

  const resolveThumbnailUrl = (img: StoredImageMetadata): string => {
    if (img.thumbnail_url) {
      return resolveImageOverlayUrl(img.thumbnail_url);
    }
    if (img.file_path) {
      return resolveImageOverlayUrl(img.file_path);
    }
    return "";
  };

  const handleFocusOrtho = (img: StoredImageMetadata) => {
    const key = img.id || img.filename;
    setSelectedId(key);
    setActiveImage(key, "gallery");
    if (map && img.bounds && isValidWgs84Bounds(img.bounds)) {
      const targetBounds = L.latLngBounds(img.bounds);
      const calculatedZoom = map.getBoundsZoom(targetBounds);
      const targetZoom = Math.max(calculatedZoom, 16);
      map.flyTo(targetBounds.getCenter(), Math.min(targetZoom, 18), {
        duration: 1.2,
      });
    }
  };

  const handleFocusRaster = (img: StoredImageMetadata) => {
    const key = img.id || img.filename;
    setSelectedId(key);
    setActiveImage(key, "gallery");

    if (img.id && img.file_path?.includes("result_image")) {
      window.dispatchEvent(
        new CustomEvent("open-results-tab", {
          detail: { id: img.id },
        }),
      );
      void showToast(`Membuka hasil analisis spasial '${img.filename}'`, "success");
      return;
    }

    if (map && img.bounds && isValidWgs84Bounds(img.bounds)) {
      map.flyToBounds(img.bounds, {
        padding: [50, 50],
        maxZoom: 17,
        duration: 1.2,
      });
    } else {
      void showToast(
        `Citra '${img.filename}' adalah foto raster tanpa koordinat geografis (non-spasial di peta).`,
        "warning",
      );
    }
  };

  const handleFocusMarker = (markerId: string, lat: number, lng: number) => {
    setSelectedId(markerId);
    setActiveImage(markerId, "gallery");
    if (map) {
      map.flyTo([lat, lng], 18, {
        duration: 1.2,
      });
    }
  };

  const handleImportSuccess = (imported: StoredImageMetadata) => {
    const itemKey = imported.id || imported.filename;
    setStoredImages((prev) => [imported, ...prev.filter((i) => (i.id || i.filename) !== itemKey)]);
    setSelectedId(itemKey);
    setActiveImage(itemKey, "gallery");
    if (map && imported.bounds && isValidWgs84Bounds(imported.bounds)) {
      const targetBounds = L.latLngBounds(imported.bounds);
      const calculatedZoom = map.getBoundsZoom(targetBounds);
      const targetZoom = Math.max(calculatedZoom, 16);
      map.flyTo(targetBounds.getCenter(), Math.min(targetZoom, 18), {
        duration: 1.2,
      });
    }
  };

  const handleOpenDelete = (img: StoredImageMetadata) => {
    setDeletingImage(img);
    setCascadeSummary(null);
    if (img.id) {
      setIsLoadingSummary(true);
      getOrthoCascadeSummary(img.id)
        .then((res) => {
          setCascadeSummary(res);
        })
        .catch((err: unknown) => {
          console.warn("Gagal mengambil ringkasan cascade delete:", err);
          setCascadeSummary({
            id: img.id || "",
            image_deleted: false,
            layer_deleted: true,
            annotations_deleted_count: 0,
          });
        })
        .finally(() => {
          setIsLoadingSummary(false);
        });
    }
  };

  const handleConfirmDelete = async () => {
    if (!deletingImage || !deletingImage.id) return;
    setIsActionProcessing(true);
    const targetId = deletingImage.id;
    const targetName = deletingImage.filename;
    try {
      await deleteOrthoImage(targetId);
      setStoredImages((prev) => prev.filter((img) => (img.id || img.filename) !== targetId));
      window.dispatchEvent(
        new CustomEvent("ortho_image:deleted", {
          detail: { id: targetId },
        }),
      );
      await showToast(`Citra '${targetName}' beserta layer terkait berhasil dihapus.`, "success");
      setDeletingImage(null);
      setCascadeSummary(null);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      await showToast(`Gagal menghapus citra: ${msg}`, "danger");
    } finally {
      setIsActionProcessing(false);
    }
  };

  const handleSaveOrtho = async (params: UpdateOrthoImageParams) => {
    setIsActionProcessing(true);
    try {
      const updated = await updateOrthoImage(params);
      setStoredImages((prev) => prev.map((img) => (img.id === updated.id ? updated : img)));
      window.dispatchEvent(
        new CustomEvent("ortho_image:saved", {
          detail: updated,
        }),
      );
      await showToast(`Metadata citra '${updated.filename}' berhasil diperbarui.`, "success");
      setEditingImage(null);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      await showToast(`Gagal memperbarui citra: ${msg}`, "danger");
      throw err;
    } finally {
      setIsActionProcessing(false);
    }
  };

  const getCurrentMapBounds = useCallback(() => {
    if (!map) return null;
    const b = map.getBounds();
    return {
      west: Number(b.getWest().toFixed(6)),
      south: Number(b.getSouth().toFixed(6)),
      east: Number(b.getEast().toFixed(6)),
      north: Number(b.getNorth().toFixed(6)),
    };
  }, [map]);

  const isLoading = isImagesLoading;
  const totalCount = orthoImages.length + rasterImages.length + markers.length;

  return (
    <div className="gallery-sidebar">
      <div
        className="gallery-header"
        style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start" }}
      >
        <div>
          <div
            style={{
              fontSize: "14px",
              fontWeight: 700,
              color: "var(--color-ink)",
              marginBottom: "3px",
              letterSpacing: "-0.2px",
            }}
          >
            Project Gallery
          </div>
          <div style={{ fontSize: "12px", color: "var(--color-slate)" }}>
            {totalCount} Citra Tersimpan
          </div>
        </div>
        <Button
          icon="import"
          text="Impor"
          small
          minimal
          intent="primary"
          onClick={() => setIsImportModalOpen(true)}
          title="Impor citra JPG/PNG dengan koordinat WGS84"
        />
      </div>

      {/* FILTER PILLS */}
      <div className="gallery-filter-bar">
        <button
          type="button"
          className={`gallery-filter-pill ${activeFilter === "all" ? "active" : ""}`}
          onClick={() => setActiveFilter("all")}
        >
          Semua ({totalCount})
        </button>
        <button
          type="button"
          className={`gallery-filter-pill ${activeFilter === "ortho" ? "active" : ""}`}
          onClick={() => setActiveFilter("ortho")}
        >
          Orthomosaic ({orthoImages.length})
        </button>
        <button
          type="button"
          className={`gallery-filter-pill ${activeFilter === "raster" ? "active" : ""}`}
          onClick={() => setActiveFilter("raster")}
        >
          Foto Raster ({rasterImages.length + markers.length})
        </button>
      </div>

      <div className="gallery-list" ref={listRef}>
        {isLoading ? (
          <div style={{ display: "flex", justifyContent: "center", padding: "40px" }}>
            <Spinner />
          </div>
        ) : totalCount === 0 ? (
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
                  onClick={() => setIsImportModalOpen(true)}
                />
              }
            />
          </div>
        ) : (
          <>
            {/* =========================================
                BAGIAN 1: FOTO ORTHOMOSAIC DRONE (JPG/PNG)
                ========================================= */}
            {(activeFilter === "all" || activeFilter === "ortho") && orthoImages.length > 0 && (
              <div className="gallery-section">
                <div className="gallery-section-header">
                  <div className="gallery-section-title">
                    <span
                      style={{
                        width: "7px",
                        height: "7px",
                        borderRadius: "50%",
                        background: "#34c759",
                      }}
                    />
                    Foto Orthomosaic Drone
                  </div>
                  <span className="gallery-section-badge badge-ortho">
                    {orthoImages.length} Citra Ortho (JPG/PNG)
                  </span>
                </div>

                {orthoImages.map((img) => {
                  const itemKey = img.id || img.filename;
                  const isSelected = selectedId === itemKey || activeImageId === itemKey;
                  return (
                    <OrthoCardItem
                      key={itemKey}
                      img={img}
                      thumbnailUrl={resolveThumbnailUrl(img)}
                      isSelected={isSelected}
                      isProcessing={isActionProcessing}
                      onFocus={handleFocusOrtho}
                      onEdit={(target) => setEditingImage(target)}
                      onDelete={handleOpenDelete}
                    />
                  );
                })}
              </div>
            )}

            {/* =========================================
                BAGIAN 2: FOTO RASTER BIASA (JPG/PNG & CAPTURES)
                ========================================= */}
            {(activeFilter === "all" || activeFilter === "raster") && (
              <div className="gallery-section">
                <div className="gallery-section-header">
                  <div className="gallery-section-title">
                    <span
                      style={{
                        width: "7px",
                        height: "7px",
                        borderRadius: "50%",
                        background: "#007aff",
                      }}
                    />
                    Foto Raster (Hasil Analisis)
                  </div>
                  <span className="gallery-section-badge badge-raster">
                    {rasterImages.length + markers.length} File
                  </span>
                </div>

                {rasterImages.map((img) => {
                  const itemKey = img.id || img.filename;
                  const isSelected = selectedId === itemKey || activeImageId === itemKey;
                  const thumbSrc = resolveThumbnailUrl(img);
                  return (
                    <div
                      key={itemKey}
                      id={`gallery-item-${itemKey}`}
                      className={`gallery-card ${isSelected ? "active" : ""}`}
                      role="button"
                      tabIndex={0}
                      onClick={() => handleFocusRaster(img)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter" || e.key === " ") {
                          e.preventDefault();
                          handleFocusRaster(img);
                        }
                      }}
                    >
                      <div className="gallery-thumb-container">
                        <RasterThumb
                          src={thumbSrc}
                          filename={img.filename}
                          format={img.format}
                          extension={img.extension}
                        />
                      </div>
                      <div className="gallery-card-info">
                        <div className="gallery-card-title" title={img.filename}>
                          {img.filename}
                        </div>
                        <div className="gallery-card-sub">
                          <span>{formatBytes(img.size_bytes)}</span>
                          <span>•</span>
                          <span>
                            {img.file_path?.includes("result_image")
                              ? "Hasil Analisis"
                              : "Raster Image"}
                          </span>
                        </div>
                        {img.file_path?.includes("result_image") ? (
                          <div className="gallery-focus-hint" style={{ color: "#007aff" }}>
                            Buka Hasil Analisis ↗
                          </div>
                        ) : img.bounds && isValidWgs84Bounds(img.bounds) ? (
                          <div className="gallery-focus-hint">Fokus ke area peta ↗</div>
                        ) : (
                          <div className="gallery-focus-hint" style={{ opacity: 0.65 }}>
                            Non-spasial (Tanpa koordinat)
                          </div>
                        )}
                      </div>
                    </div>
                  );
                })}

                {/* Drone Flight Capture Markers (jika ada) */}
                {markers.map((img) => {
                  const isSelected = selectedId === img.image_id || activeImageId === img.image_id;
                  return (
                    <div
                      key={img.image_id}
                      id={`gallery-item-${img.image_id}`}
                      className={`gallery-card ${isSelected ? "active" : ""}`}
                      role="button"
                      tabIndex={0}
                      onClick={() => handleFocusMarker(img.image_id, img.latitude, img.longitude)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter" || e.key === " ") {
                          e.preventDefault();
                          handleFocusMarker(img.image_id, img.latitude, img.longitude);
                        }
                      }}
                    >
                      <div className="gallery-thumb-container">
                        <RasterThumb
                          src={img.thumbnail_url}
                          filename={img.file_name}
                          format="DRONE"
                        />
                      </div>
                      <div className="gallery-card-info">
                        <div className="gallery-card-title" title={img.file_name}>
                          {img.file_name}
                        </div>
                        <div className="gallery-card-sub">
                          <span>Alt: {img.altitude_agl.toFixed(0)}m</span>
                          <span>•</span>
                          <span>Heading: {img.heading_deg}°</span>
                        </div>
                        <div className="gallery-focus-hint">Fokus ke titik foto ↗</div>
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </>
        )}
      </div>

      <ImportImageModal
        isOpen={isImportModalOpen}
        onClose={() => setIsImportModalOpen(false)}
        onImportSuccess={handleImportSuccess}
        map={map}
      />

      <EditOrthoModal
        isOpen={Boolean(editingImage)}
        onClose={() => setEditingImage(null)}
        image={editingImage}
        getCurrentMapBounds={getCurrentMapBounds}
        onSave={handleSaveOrtho}
        isSubmitting={isActionProcessing}
      />

      <DeleteOrthoAlert
        isOpen={Boolean(deletingImage)}
        onClose={() => setDeletingImage(null)}
        image={deletingImage}
        summary={cascadeSummary}
        isLoadingSummary={isLoadingSummary}
        isProcessing={isActionProcessing}
        onConfirm={() => {
          void handleConfirmDelete();
        }}
      />
    </div>
  );
};
