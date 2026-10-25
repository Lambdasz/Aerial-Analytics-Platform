import React, { useCallback, useEffect, useRef, useState } from "react";
import { useMap } from "react-leaflet";
import * as L from "leaflet";
import { listOrthoImages } from "../services/api";
import type { StoredImageMetadata } from "../types/storage";
import type { LayerState } from "../types/layer";
import { isValidWgs84Bounds, resolveOrthoDisplayUrl } from "../lib/leaflet/spatialLayerBuilder";
import { MAX_DISPLAY_FILE_SIZE_BYTES, MIN_ORTHO_ZOOM } from "../config/mapConfig";
import { buildOrthoPopupHtml } from "../lib/popupHtml";
import { showToast } from "../lib/toaster";

export interface SavedOrthoImageLayerProps {
  layers?: LayerState[];
}

/**
 * Organism untuk merender citra orthomosaic (JPG/PNG) tersimpan pada Leaflet map
 * secara selektif dan efisien:
 * 1. Syarat ukuran: display preview <= 5 MB (MAX_DISPLAY_FILE_SIZE_BYTES).
 * 2. Syarat spasial: hanya memuat citra yang berpotongan (intersect) dengan viewport peta aktif.
 * 3. Syarat ketinggian: hanya tampil pada level zoom >= 16 (MIN_ORTHO_ZOOM). Di bawah 16, overlay dilepas.
 * 4. Mode Toggle ON/OFF:
 *    - ON (Normal): Merender bitmap citra orthomosaic.
 *    - OFF (Hemat Memori): Melepas bitmap citra, hanya mengaktifkan border outline biru saat dihover.
 */
export const SavedOrthoImageLayer: React.FC<SavedOrthoImageLayerProps> = ({ layers = [] }) => {
  const map = useMap();
  const [images, setImages] = useState<StoredImageMetadata[]>([]);
  const layerGroupRef = useRef<L.LayerGroup | null>(null);
  const activeOverlaysRef = useRef<Map<string, L.ImageOverlay>>(new Map());
  const activeOutlinesRef = useRef<Map<string, L.Rectangle>>(new Map());

  // State mode tampilan per citra: true = ON (tampil gambar), false = OFF (mode outline hemat memori)
  const [displayModes, setDisplayModes] = useState<Record<string, boolean>>(() => {
    try {
      const saved = localStorage.getItem("aap_ortho_display_modes");
      return saved ? (JSON.parse(saved) as Record<string, boolean>) : {};
    } catch {
      return {};
    }
  });

  // Muat daftar citra dari storage
  useEffect(() => {
    let cancelled = false;

    const loadImages = () => {
      listOrthoImages()
        .then((data) => {
          if (!cancelled) {
            setImages(data || []);
          }
        })
        .catch((err) => {
          console.error("Gagal memuat citra untuk layer peta:", err);
        });
    };

    loadImages();

    // Event listener jika ada citra baru yang diimpor atau diperbarui
    const handleImageChanged = () => {
      loadImages();
    };

    // Event listener jika ada citra yang dihapus (langsung detach overlay dari peta)
    const handleImageDeleted = (e: Event) => {
      const customEvent = e as CustomEvent<{ id: string }>;
      const deletedId = customEvent.detail?.id;
      if (deletedId && layerGroupRef.current) {
        const overlay = activeOverlaysRef.current.get(deletedId);
        if (overlay) {
          layerGroupRef.current.removeLayer(overlay);
          activeOverlaysRef.current.delete(deletedId);
        }
        const outline = activeOutlinesRef.current.get(deletedId);
        if (outline) {
          layerGroupRef.current.removeLayer(outline);
          activeOutlinesRef.current.delete(deletedId);
        }
      }
      loadImages();
    };

    window.addEventListener("ortho_image:saved", handleImageChanged);
    window.addEventListener("ortho_image:deleted", handleImageDeleted);

    return () => {
      cancelled = true;
      window.removeEventListener("ortho_image:saved", handleImageChanged);
      window.removeEventListener("ortho_image:deleted", handleImageDeleted);
    };
  }, []);

  // Inisialisasi custom pane `orthoPane` (z-index 250) agar citra orthomosaic selalu di paling bawah
  // di atas basemap (200), tetapi di bawah AOI (350) dan hasil analisis spasial (400)
  useEffect(() => {
    if (map && !map.getPane("orthoPane")) {
      const pane = map.createPane("orthoPane");
      pane.style.zIndex = "250";
    }
  }, [map]);

  // Logika rekonsiliasi selektif & lazy untuk overlay citra dan outline
  const updateOverlays = useCallback(() => {
    let group = layerGroupRef.current;
    if (!group) {
      group = L.layerGroup();
      layerGroupRef.current = group;
    }

    if (!map.hasLayer(group)) {
      map.addLayer(group);
    }

    if (!map.getPane("orthoPane")) {
      const pane = map.createPane("orthoPane");
      pane.style.zIndex = "250";
    }

    const currentZoom = map.getZoom();
    const mapBounds = map.getBounds();
    const activeOverlays = activeOverlaysRef.current;
    const activeOutlines = activeOutlinesRef.current;

    // SYARAT 4: Zoom threshold >= 16. Jika zoom < 16, pastikan overlay dan outline dilepas dari DOM
    if (currentZoom < MIN_ORTHO_ZOOM) {
      if (activeOverlays.size > 0 || activeOutlines.size > 0) {
        group.clearLayers();
        activeOverlays.clear();
        activeOutlines.clear();
      }
      return;
    }

    // Set kunci citra yang valid & berada di viewport
    const visibleKeys = new Set<string>();

    images.forEach((img) => {
      if (img.is_corrupted) {
        return;
      }

      // SYARAT 0: Hanya citra orthomosaic drone (bukan foto hasil analisis result_image / raster non-ortho)
      if (
        img.category === "raster" ||
        img.file_path?.includes("result_image") ||
        !img.bounds ||
        !isValidWgs84Bounds(img.bounds)
      ) {
        return;
      }

      const imgKey = img.id || img.filename;

      // SYARAT 2: Ukuran display preview tidak boleh lebih dari 5 MB
      if (img.display_size_bytes && img.display_size_bytes > MAX_DISPLAY_FILE_SIZE_BYTES) {
        return;
      }

      // SYARAT 3: Spatial culling - jika viewport pengguna jauh dari area foto, jangan di-load
      const imgBounds = L.latLngBounds(img.bounds);
      if (!mapBounds.intersects(imgBounds)) {
        return;
      }

      // Periksa visibilitas layer di LayerManager
      const matchingLayer = layers.find(
        (l) =>
          (img.id && (l.source_id === img.id || l.layer_id === img.id)) ||
          l.layer_name === img.filename,
      );

      // Sinkronisasi penuh dengan layer: status ON/OFF ditentukan langsung oleh visibilitas layer!
      const isEnabled = matchingLayer ? matchingLayer.is_visible : (displayModes[imgKey] ?? true);

      const opacity = matchingLayer ? matchingLayer.opacity : 1.0;
      visibleKeys.add(imgKey);

      if (isEnabled) {
        // === MODE ON: Tampilkan Gambar Ortho ===
        // 1. Lepas outline jika sebelumnya ada
        const existingOutline = activeOutlines.get(imgKey);
        if (existingOutline) {
          group.removeLayer(existingOutline);
          activeOutlines.delete(imgKey);
        }

        // 2. Ambil URL citra display yang web-ready (aman dari .tif/.tiff)
        const resolvedUrl = resolveOrthoDisplayUrl(img);
        if (!resolvedUrl) return;

        // 3. Jika overlay sudah ada, perbarui opacity & popup konten
        const existingOverlay = activeOverlays.get(imgKey);
        if (existingOverlay) {
          existingOverlay.setOpacity(opacity);
          existingOverlay.setPopupContent(buildOrthoPopupHtml(img, true));
          return;
        }

        try {
          const overlay = L.imageOverlay(resolvedUrl, img.bounds, {
            opacity,
            interactive: true,
            className: "ortho-image-overlay",
            pane: "orthoPane",
          });

          overlay.bindPopup(buildOrthoPopupHtml(img, true), {
            maxWidth: 350,
            className: "apple-glass-popup",
          });

          overlay.addTo(group);
          activeOverlays.set(imgKey, overlay);
        } catch (err) {
          console.error("Gagal menambahkan image overlay:", err);
        }
      } else {
        // === MODE OFF: Sembunyikan Gambar (Hemat Memori), Aktifkan Hover Outline Biru ===
        // 1. Lepas image overlay dari DOM canvas agar memori VRAM bebas
        const existingOverlay = activeOverlays.get(imgKey);
        if (existingOverlay) {
          group.removeLayer(existingOverlay);
          activeOverlays.delete(imgKey);
        }

        // 2. Jika outline sudah ada di peta, cukup perbarui popup kontennya
        const existingOutline = activeOutlines.get(imgKey);
        if (existingOutline) {
          existingOutline.setPopupContent(buildOrthoPopupHtml(img, false));
          return;
        }

        try {
          // Buat rectangle transparan di sekeliling bounds
          const outline = L.rectangle(img.bounds, {
            color: "#007aff",
            weight: 0,
            opacity: 0,
            fillColor: "#007aff",
            fillOpacity: 0,
            interactive: true,
            className: "ortho-outline-rect",
            pane: "orthoPane",
          });

          // Ketika mouse menghover area citra: muncul border outline garis biru
          outline.on("mouseover", () => {
            outline.setStyle({
              color: "#007aff",
              weight: 2.5,
              opacity: 1,
              fillColor: "#007aff",
              fillOpacity: 0.08,
            });
          });

          // Ketika mouse tidak menghover area citra: border outline garis biru kembali hilang
          outline.on("mouseout", () => {
            if (outline.isPopupOpen()) {
              return; // Tetap pertahankan outline biru saat popup sedang aktif terbuka
            }
            outline.setStyle({
              weight: 0,
              opacity: 0,
              fillOpacity: 0,
            });
          });

          outline.on("popupclose", () => {
            outline.setStyle({
              weight: 0,
              opacity: 0,
              fillOpacity: 0,
            });
          });

          outline.bindPopup(buildOrthoPopupHtml(img, false), {
            maxWidth: 350,
            className: "apple-glass-popup",
          });

          outline.addTo(group);
          activeOutlines.set(imgKey, outline);
        } catch (err) {
          console.error("Gagal menambahkan outline rectangle:", err);
        }
      }
    });

    // Lepas overlay dan outline yang keluar dari viewport atau tidak lagi memenuhi syarat
    for (const [key, overlay] of activeOverlays.entries()) {
      if (!visibleKeys.has(key)) {
        group.removeLayer(overlay);
        activeOverlays.delete(key);
      }
    }
    for (const [key, outline] of activeOutlines.entries()) {
      if (!visibleKeys.has(key)) {
        group.removeLayer(outline);
        activeOutlines.delete(key);
      }
    }
  }, [displayModes, images, layers, map]);

  // Tangani event klik tombol toggle ON/OFF di dalam Popup Leaflet
  useEffect(() => {
    const handleGlobalClick = (ev: MouseEvent) => {
      const target = ev.target as HTMLElement | null;
      const btn = target?.closest<HTMLButtonElement>("[data-action='toggle-ortho']");
      if (!btn) return;

      ev.preventDefault();
      ev.stopPropagation();

      const targetId = btn.getAttribute("data-id");
      if (!targetId) return;

      // Cari matching layer untuk citra ini
      const matchingLayer = layers.find(
        (l) =>
          (l.source_id && l.source_id === targetId) ||
          l.layer_id === targetId ||
          l.layer_name === targetId,
      );

      const currentVal = matchingLayer
        ? matchingLayer.is_visible
        : (displayModes[targetId] ?? true);
      const nextVal = !currentVal;

      // 1. Dispatch event ke LayerManager agar tombol ON/OFF di sidebar ikut tersinkronisasi
      const layerIdToToggle = matchingLayer ? matchingLayer.layer_id : targetId;
      window.dispatchEvent(
        new CustomEvent("toggle-layer-visibility", {
          detail: { layerId: layerIdToToggle, nextState: nextVal },
        }),
      );

      // 2. Simpan juga ke displayModes & localStorage
      setDisplayModes((prev) => {
        const nextMap = { ...prev, [targetId]: nextVal };
        try {
          localStorage.setItem("aap_ortho_display_modes", JSON.stringify(nextMap));
        } catch (err) {
          console.warn("Gagal menyimpan preferensi display mode:", err);
        }
        return nextMap;
      });

      if (!nextVal) {
        void showToast(
          "Mode hemat memori aktif: Gambar disembunyikan. Hover area untuk melihat outline biru.",
          "warning",
        );
      } else {
        void showToast("Citra orthomosaic ditampilkan normal.", "success");
      }

      map.closePopup();
    };

    // Mencegah intervensi klik pada popup Leaflet
    const handlePopupOpen = (e: L.PopupEvent) => {
      const container = e.popup.getElement();
      if (container) {
        L.DomEvent.disableClickPropagation(container);
      }
    };

    map.on("popupopen", handlePopupOpen);
    document.addEventListener("click", handleGlobalClick, true);

    return () => {
      map.off("popupopen", handlePopupOpen);
      document.removeEventListener("click", handleGlobalClick, true);
    };
  }, [displayModes, layers, map]);

  // Pasang listener event interaksi peta (moveend & zoomend)
  useEffect(() => {
    updateOverlays();

    const handleMapMovement = () => {
      updateOverlays();
    };

    map.on("zoomend", handleMapMovement);
    map.on("moveend", handleMapMovement);

    return () => {
      map.off("zoomend", handleMapMovement);
      map.off("moveend", handleMapMovement);
    };
  }, [map, updateOverlays]);

  // Bersihkan group dan map saat unmount komponen
  useEffect(() => {
    const activeOverlays = activeOverlaysRef.current;
    const activeOutlines = activeOutlinesRef.current;
    return () => {
      if (layerGroupRef.current) {
        layerGroupRef.current.clearLayers();
        map.removeLayer(layerGroupRef.current);
        layerGroupRef.current = null;
      }
      activeOverlays.clear();
      activeOutlines.clear();
    };
  }, [map]);

  return null;
};
