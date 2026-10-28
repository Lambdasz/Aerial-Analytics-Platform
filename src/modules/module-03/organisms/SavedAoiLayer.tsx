import React, { useEffect, useRef } from "react";
import { useMap } from "react-leaflet";
import * as L from "leaflet";
import { getSavedAois } from "../services/api";
import { buildAoiPopupHtml } from "../lib/popupHtml";
import { estimatePolygonArea } from "../lib/geoJson";
import { useMapZoom } from "../hooks/useMapZoom";
import { MIN_POLYGON_FEATURE_ZOOM } from "../config/mapConfig";
import type { AoiFeature } from "../types/aoi";

export interface SavedAoiLayerProps {
  onAoisLoaded?: (aois: AoiFeature[]) => void;
  minZoom?: number;
}

const getAoiArea = (aoi: AoiFeature): number => {
  if (aoi.properties.calculated_area_sq_m && aoi.properties.calculated_area_sq_m > 0) {
    return aoi.properties.calculated_area_sq_m;
  }
  return estimatePolygonArea(aoi.geometry.coordinates);
};

/**
 * Komponen Leaflet untuk merender semua Area of Interest (AOI) yang tersimpan
 * di backend `/storage/aois/` dengan styling poligon biru dan popup informatif.
 *
 * Menggunakan custom pane `aoiPane` (z-index: 350) agar selalu berada di bawah layer
 * hasil analisis spasial (`overlayPane`, z-index: 400).
 * Jika ada AOI bertumpuk (nested), area terluas ditaruh di bawah dan area dalam yang
 * lebih kecil otomatis berada di layer paling atas.
 *
 * Menyembunyikan seluruh poligon AOI saat zoom < minZoom (default 15) untuk menjaga
 * overview peta tetap bersih.
 */
export const SavedAoiLayer: React.FC<SavedAoiLayerProps> = ({
  onAoisLoaded,
  minZoom = MIN_POLYGON_FEATURE_ZOOM,
}) => {
  const map = useMap();
  const currentZoom = useMapZoom();
  const isVisible = currentZoom >= minZoom;
  const layerGroupRef = useRef<L.LayerGroup | null>(null);
  const aoisMapRef = useRef<Map<string, AoiFeature>>(new Map());

  // Sinkronisasi visibilitas layer group AOI saat level zoom berubah
  useEffect(() => {
    const group = layerGroupRef.current;
    if (!group) return;

    if (isVisible && !map.hasLayer(group)) {
      map.addLayer(group);
    } else if (!isVisible && map.hasLayer(group)) {
      map.removeLayer(group);
    }
  }, [isVisible, map]);

  useEffect(() => {
    // 1. Buat custom pane untuk AOI agar berada di bawah overlayPane (analisis spasial)
    if (!map.getPane("aoiPane")) {
      const pane = map.createPane("aoiPane");
      pane.style.zIndex = "350"; // Di bawah default overlayPane (zIndex: 400)
    }

    const group = L.layerGroup();
    if (map.getZoom() >= minZoom) {
      group.addTo(map);
    }
    layerGroupRef.current = group;
    const aoisMap = aoisMapRef.current;

    let editingAoiId: string | null = null;

    // Render ulang seluruh AOI dalam urutan area terbesar ke terkecil
    // sehingga area dalam (inner area) otomatis berada di layer paling atas
    const renderSortedAois = () => {
      group.clearLayers();
      const aois = Array.from(aoisMap.values());

      // Urutkan area terbesar di bawah (dibuat duluan), area terkecil di atas
      const sortedAois = [...aois].sort((a, b) => getAoiArea(b) - getAoiArea(a));

      sortedAois.forEach((aoi) => {
        // Jangan render poligon biru jika AOI ini sedang dalam mode edit (sedang digambar orange oleh editor)
        if (editingAoiId && aoi.properties.aoi_id === editingAoiId) {
          return;
        }

        // GeoJSON coordinates: [longitude, latitude] -> Leaflet: [latitude, longitude]
        const rings = aoi.geometry.coordinates.map((ring) =>
          ring.map(([lng, lat]) => [lat, lng] as [number, number]),
        );

        const polygon = L.polygon(rings, {
          pane: "aoiPane",
          color: "#007aff",
          weight: 2.5,
          dashArray: "6, 6",
          fillColor: "#007aff",
          fillOpacity: 0.16,
        });

        polygon.on("mouseover", () => {
          polygon.setStyle({
            weight: 3.5,
            fillOpacity: 0.28,
            dashArray: "",
          });
        });

        polygon.on("mouseout", () => {
          polygon.setStyle({
            weight: 2.5,
            fillOpacity: 0.16,
            dashArray: "6, 6",
          });
        });

        polygon.bindPopup(buildAoiPopupHtml(aoi), {
          maxWidth: 320,
          className: "aoi-custom-popup",
        });

        polygon.addTo(group);
      });
    };

    let cancelled = false;

    // Muat seluruh AOI yang tersimpan dari Rust storage
    getSavedAois()
      .then((savedList) => {
        if (cancelled) return;
        savedList.forEach((aoi) => {
          aoisMap.set(aoi.properties.aoi_id, aoi);
        });
        renderSortedAois();
        onAoisLoaded?.(savedList);
      })
      .catch((err) => {
        console.warn("[SavedAoiLayer] Belum dapat memuat saved AOIs:", err);
      });

    // Event listener saat ada AOI baru yang berhasil disimpan
    const handleAoiSaved = (event: Event) => {
      const customEvent = event as CustomEvent<AoiFeature>;
      if (customEvent.detail) {
        aoisMap.set(customEvent.detail.properties.aoi_id, customEvent.detail);
        renderSortedAois();
      }
    };

    // Event listener saat ada AOI yang dihapus
    const handleAoiDeleted = (event: Event) => {
      const customEvent = event as CustomEvent<{ aoiId: string }>;
      if (customEvent.detail?.aoiId) {
        aoisMap.delete(customEvent.detail.aoiId);
        renderSortedAois();
      }
    };

    // Event listener saat mode edit poligon dimulai (sembunyikan layer biru untuk AOI ini)
    const handleEditingStarted = (event: Event) => {
      const customEvent = event as CustomEvent<{ aoiId: string }>;
      if (customEvent.detail?.aoiId) {
        editingAoiId = customEvent.detail.aoiId;
        renderSortedAois();
      }
    };

    // Event listener saat mode edit poligon selesai atau dibatalkan
    const handleEditingStopped = () => {
      if (editingAoiId !== null) {
        editingAoiId = null;
        renderSortedAois();
      }
    };

    window.addEventListener("aoi:saved", handleAoiSaved);
    window.addEventListener("aoi:deleted", handleAoiDeleted);
    window.addEventListener("aoi:editing-started", handleEditingStarted);
    window.addEventListener("aoi:editing-stopped", handleEditingStopped);

    return () => {
      cancelled = true;
      window.removeEventListener("aoi:saved", handleAoiSaved);
      window.removeEventListener("aoi:deleted", handleAoiDeleted);
      window.removeEventListener("aoi:editing-started", handleEditingStarted);
      window.removeEventListener("aoi:editing-stopped", handleEditingStopped);
      if (map.hasLayer(group)) {
        map.removeLayer(group);
      }
      layerGroupRef.current = null;
      aoisMap.clear();
    };
  }, [map, minZoom, onAoisLoaded]);

  return null;
};
