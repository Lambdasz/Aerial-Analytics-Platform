import { useState, useEffect, useMemo, useCallback } from "react";
import * as L from "leaflet";
import "../leaflet-setup";
import { useImageSync } from "../store/ImageSyncContext";
import {
  listOrthoImages,
  updateOrthoImage,
  deleteOrthoImage,
  getOrthoCascadeSummary,
} from "../services/api";
import type {
  DeleteOrthoSummary,
  StoredImageMetadata,
  UpdateOrthoImageParams,
} from "../types/storage";
import type { DroneImageMetadata } from "../types/marker";
import { isValidWgs84Bounds, resolveImageOverlayUrl } from "../lib/leaflet/spatialLayerBuilder";
import type { GalleryFilterMode } from "../molecules/gallery/GalleryFilterBar";
import { showToast } from "../lib/toaster";

export interface UseGalleryImagesOptions {
  map?: L.Map | null;
  markers?: ReadonlyArray<DroneImageMetadata>;
}

export function useGalleryImages({ map, markers = [] }: UseGalleryImagesOptions = {}) {
  const { activeImageId, setActiveImage, interactionSource } = useImageSync();
  const [storedImages, setStoredImages] = useState<StoredImageMetadata[]>([]);
  const [isImagesLoading, setIsImagesLoading] = useState(true);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [activeFilter, setActiveFilter] = useState<GalleryFilterMode>("all");
  const [isImportModalOpen, setIsImportModalOpen] = useState(false);
  const [editingImage, setEditingImage] = useState<StoredImageMetadata | null>(null);
  const [deletingImage, setDeletingImage] = useState<StoredImageMetadata | null>(null);
  const [cascadeSummary, setCascadeSummary] = useState<DeleteOrthoSummary | null>(null);
  const [isLoadingSummary, setIsLoadingSummary] = useState(false);
  const [isActionProcessing, setIsActionProcessing] = useState(false);

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

  const orthoImages = useMemo(
    () => storedImages.filter((img) => img.category === "ortho"),
    [storedImages],
  );

  const rasterImages = useMemo(
    () => storedImages.filter((img) => img.category === "raster"),
    [storedImages],
  );

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

  const totalCount = orthoImages.length + rasterImages.length + markers.length;

  return {
    activeImageId,
    interactionSource,
    storedImages,
    isImagesLoading,
    selectedId,
    activeFilter,
    setActiveFilter,
    isImportModalOpen,
    setIsImportModalOpen,
    editingImage,
    setEditingImage,
    deletingImage,
    setDeletingImage,
    cascadeSummary,
    isLoadingSummary,
    isActionProcessing,
    orthoImages,
    rasterImages,
    totalCount,
    resolveThumbnailUrl,
    handleFocusOrtho,
    handleFocusRaster,
    handleFocusMarker,
    handleImportSuccess,
    handleOpenDelete,
    handleConfirmDelete,
    handleSaveOrtho,
    getCurrentMapBounds,
  };
}
