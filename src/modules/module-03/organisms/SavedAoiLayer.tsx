import React, { useEffect, useRef } from "react";
import { useMap } from "react-leaflet";
import * as L from "leaflet";
import { getSavedAois } from "../services/api";
import { buildAoiPopupHtml } from "../lib/popupHtml";
import { estimatePolygonArea, isPointInPolygon } from "../lib/geoJson";
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
 * Mengimplementasikan `pointer-events: stroke` dengan delegasi klik peta agar citra
 * ortomosaik di bawahnya tetap dapat diklik secara tembus (*click-through*), sementara
 * garis tepi AOI dan area fill di luar ortomosaik tetap interaktif.
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
  const polygonLayersRef = useRef<Map<string, L.Polygon>>(new Map());

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
    //
    // CATATAN ARSITEKTUR & ROADMAP MASA DEPAN (Z-INDEX & CLICK DISAMBIGUATION):
    // Saat ini seluruh poligon AOI (fill + stroke) berada di `aoiPane` (z-index: 350), yang berada
    // di bawah `overlayPane` (z-index: 400) tempat fitur vektor Spatial Result berada.
    //
    // KASUS KHUSUS / TRADE-OFF:
    // Jika terdapat poligon Spatial Result (misal: tutupan lahan) yang berukuran LEBIH BESAR
    // daripada poligon AOI dan menutupi AOI kecil di dalamnya, maka klik pada area tumpang tindih
    // tersebut di kanvas peta akan ditangkap oleh poligon Spatial Result (bukan AOI).
    // Pengguna saat ini tetap dapat mengakses AOI tersebut via list kartu di Sidebar AOI atau
    // mematikan sementara visibilitas layer Spatial Result.
    //
    // Opsi peningkatan untuk pengembang mendatang:
    // 1. Opsi A (Pemisahan Pane Stroke): Buat pane khusus `aoiStrokePane` (z-index: 450) terpisah
    //    dari `aoiFillPane` (z-index: 350), sehingga outline garis tepi AOI selalu melayang di atas
    //    poligon spasial apa pun dan dijamin 100% selalu dapat diklik di peta.
    // 2. Opsi B (Multi-Object Click Disambiguation): Tangkap event klik kanvas dan jika mengenai
    //    lebih dari satu entitas (>1 poligon bertumpuk), buka popup gabungan dengan UI tab/accordion.
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
    const polygonLayers = polygonLayersRef.current;

    let editingAoiId: string | null = null;

    // Render ulang seluruh AOI dalam urutan area terbesar ke terkecil
    // sehingga area dalam (inner area) otomatis berada di layer paling atas
    const renderSortedAois = () => {
      group.clearLayers();
      polygonLayers.clear();
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
          weight: 3.5,
          dashArray: "6, 6",
          fillColor: "#007aff",
          fillOpacity: 0.16,
          className: "aoi-polygon-path",
        });

        polygon.on("mouseover", () => {
          polygon.setStyle({
            weight: 4.5,
            fillOpacity: 0.28,
            dashArray: "",
          });
        });

        polygon.on("mouseout", () => {
          polygon.setStyle({
            weight: 3.5,
            fillOpacity: 0.16,
            dashArray: "6, 6",
          });
        });

        polygon.bindPopup(() => buildAoiPopupHtml(aoi), {
          maxWidth: 320,
          className: "aoi-custom-popup",
        });

        polygon.addTo(group);
        polygonLayers.set(aoi.properties.aoi_id, polygon);
      });
    };

    let cancelled = false;

    // Muat seluruh AOI yang tersimpan dari Rust storage
    getSavedAois()
      .then((savedList) => {
        if (cancelled) return;
        const list = Array.isArray(savedList)
          ? savedList
          : ((savedList as unknown as { aois?: AoiFeature[] })?.aois ?? []);
        list.forEach((aoi) => {
          aoisMap.set(aoi.properties.aoi_id, aoi);
        });
        renderSortedAois();
        onAoisLoaded?.(list);
      })
      .catch((err) => {
        console.warn("[SavedAoiLayer] Belum dapat memuat saved AOIs:", err);
      });

    // Delegasi handler klik peta untuk area fill AOI di luar batas ortomosaik
    const handleMapClick = (e: L.LeafletMouseEvent) => {
      // 1. Abaikan jika event klik sudah dihentikan atau dicegah oleh layer lain
      if (e.originalEvent.defaultPrevented) return;
      if ((e.originalEvent as unknown as { _stopped?: boolean })._stopped) return;
      if ((e.originalEvent as unknown as { _handledByAnnotation?: boolean })._handledByAnnotation) {
        return;
      }

      // 2. PENTING: Periksa apakah klik mengenai elemen citra ortomosaik atau popup aktif
      if (typeof document !== "undefined") {
        const elements = document.elementsFromPoint(
          e.originalEvent.clientX,
          e.originalEvent.clientY,
        );
        const hitOrthoOrPopup = elements.some(
          (el) =>
            el.classList.contains("ortho-image-overlay") ||
            el.classList.contains("ortho-outline-rect") ||
            el.closest(".leaflet-popup") !== null,
        );
        if (hitOrthoOrPopup) {
          return;
        }
      }

      // 3. Konversi koordinat klik [lat, lng] ke format GeoJSON [lng, lat]
      const clickPoint: [number, number] = [e.latlng.lng, e.latlng.lat];

      // 4. Cari AOI dari area terkecil ke terbesar (prioritaskan area dalam jika nested)
      const aois = Array.from(aoisMap.values()).sort((a, b) => getAoiArea(a) - getAoiArea(b));

      for (const aoi of aois) {
        if (editingAoiId && aoi.properties.aoi_id === editingAoiId) continue;

        if (isPointInPolygon(clickPoint, aoi.geometry.coordinates)) {
          const polyLayer = polygonLayers.get(aoi.properties.aoi_id);
          if (polyLayer) {
            polyLayer.openPopup(e.latlng);
          } else {
            L.popup({ maxWidth: 320, className: "aoi-custom-popup" })
              .setLatLng(e.latlng)
              .setContent(buildAoiPopupHtml(aoi))
              .openOn(map);
          }
          break;
        }
      }
    };

    map.on("click", handleMapClick);

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
        polygonLayers.delete(customEvent.detail.aoiId);
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
      map.off("click", handleMapClick);
      window.removeEventListener("aoi:saved", handleAoiSaved);
      window.removeEventListener("aoi:deleted", handleAoiDeleted);
      window.removeEventListener("aoi:editing-started", handleEditingStarted);
      window.removeEventListener("aoi:editing-stopped", handleEditingStopped);
      if (map.hasLayer(group)) {
        map.removeLayer(group);
      }
      layerGroupRef.current = null;
      aoisMap.clear();
      polygonLayers.clear();
    };
  }, [map, minZoom, onAoisLoaded]);

  return null;
};
