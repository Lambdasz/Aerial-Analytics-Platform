import React, { useState, useCallback, useEffect } from "react";
import type * as L from "leaflet";
import Leaflet from "leaflet";

// Leaflet & Geoman styles
import "leaflet/dist/leaflet.css";
import "@geoman-io/leaflet-geoman-free/dist/leaflet-geoman.css";

// Module styles
import "./styles/MapCanvas.css";
import "./styles/MapLegend.css";
import "./styles/MapViewControlBar.css";
import "./styles/ImageGallerySidebar.css";
import "./styles/ImagePopupCard.css";

// State and layout
import { ImageSyncProvider } from "./store/ImageSyncContext";
import { MapExplorerLayout } from "./templates/MapExplorerLayout";

// Organisms, molecules, atoms
import { MapLeftPanel } from "./organisms/MapLeftPanel";
import { MapCanvas } from "./organisms/MapCanvas";
import { MapLegend } from "./molecules/MapLegend";
import { MapTypeThumbnail } from "./atoms/MapTypeThumbnail";

// Types and config
import type { BasemapKey } from "./config/mapConfig";
import type { LayerPayload } from "./types/map";
import type { LayerState, AnnotationFeature } from "./types/layer";
import { getStorageRootPath, getInstanceStatus } from "./services/api";
import { setCachedStorageRoot } from "./lib/fieldResolution";
import { isTauri } from "@tauri-apps/api/core";

// Leaflet default icon fix for Vite bundler
import markerIcon2x from "leaflet/dist/images/marker-icon-2x.png";
import markerIcon from "leaflet/dist/images/marker-icon.png";
import markerShadow from "leaflet/dist/images/marker-shadow.png";

delete (Leaflet.Icon.Default.prototype as { _getIconUrl?: unknown })._getIconUrl;
Leaflet.Icon.Default.mergeOptions({
  iconUrl: markerIcon,
  iconRetinaUrl: markerIcon2x,
  shadowUrl: markerShadow,
});

export interface MapExplorerProps {
  /** Nama kelas CSS opsional untuk kontainer terluar */
  className?: string;
  /**
   * Override gaya CSS inline untuk kontainer terluar.
   * Nilai ini DIGABUNG (shallow merge) dengan default:
   * `{ width: "100%", height: "100%", position: "relative", ...style }`
   */
  style?: React.CSSProperties;
  /** Callback opsional saat daftar layer mengalami perubahan */
  onLayersChange?: (layers: LayerState[]) => void;
  /** Callback opsional saat daftar anotasi mengalami perubahan */
  onAnnotationsChange?: (annotations: AnnotationFeature[]) => void;
}

/**
 * Composition Root mandiri untuk Modul 03 (Aerial Image Map Explorer).
 *
 * Mengenkapsulasi:
 * - ImageSyncProvider untuk sinkronisasi dua arah foto dan peta
 * - Perbaikan Leaflet default icon URL pada Vite
 * - Impor mandiri style Leaflet, Geoman, dan stylesheet modul
 * - Orkestrasi state antara MapLeftPanel, MapCanvas, MapTypeThumbnail, dan MapLegend
 */
export const MapExplorer: React.FC<MapExplorerProps> = ({
  className,
  style,
  onLayersChange,
  onAnnotationsChange,
}) => {
  const [activeBasemap, setActiveBasemap] = useState<BasemapKey>("osm");
  const [activeLayers, setActiveLayers] = useState<LayerPayload[]>([]);
  const [layers, setLayers] = useState<LayerState[]>([]);
  const [annotations, setAnnotations] = useState<AnnotationFeature[]>([]);
  const [mapInstance, setMapInstance] = useState<L.Map | null>(null);
  const [isSidebarCollapsed, setIsSidebarCollapsed] = useState<boolean>(false);
  const [isReadOnly, setIsReadOnly] = useState<boolean>(false);

  useEffect(() => {
    if (isTauri()) {
      void getStorageRootPath()
        .then((path) => {
          setCachedStorageRoot(path);
        })
        .catch((err) => {
          console.warn("Gagal mengambil storage root path:", err);
        });

      void getInstanceStatus()
        .then((status) => {
          setIsReadOnly(status.is_read_only);
        })
        .catch((err) => {
          console.warn("Gagal mengambil status instance:", err);
        });
    }
  }, []);

  const toggleBasemap = () => {
    setActiveBasemap((current) => (current === "osm" ? "satellite" : "osm"));
  };

  const handleLayersChange = useCallback(
    (newLayers: LayerState[]) => {
      setLayers(newLayers);
      onLayersChange?.(newLayers);
    },
    [onLayersChange],
  );

  const handleAnnotationsChange = useCallback(
    (newAnnotations: AnnotationFeature[]) => {
      setAnnotations(newAnnotations);
      onAnnotationsChange?.(newAnnotations);
    },
    [onAnnotationsChange],
  );

  const mergedStyle: React.CSSProperties = {
    width: "100%",
    height: "100%",
    position: "relative",
    ...style,
  };

  return (
    <ImageSyncProvider>
      {isReadOnly && (
        <div
          role="alert"
          style={{
            position: "absolute",
            top: 12,
            left: "50%",
            transform: "translateX(-50%)",
            zIndex: 3000,
            backgroundColor: "#fff3cd",
            color: "#856404",
            border: "1px solid #ffeeba",
            borderRadius: 6,
            padding: "8px 16px",
            boxShadow: "0 4px 12px rgba(0,0,0,0.15)",
            fontSize: 13,
            fontWeight: 500,
            display: "flex",
            alignItems: "center",
            gap: 8,
            pointerEvents: "auto",
          }}
        >
          <span>
            ⚠️ <strong>Mode Hanya-Baca:</strong> Aplikasi dibuka sebagai instansi sekunder.
            Modifikasi data storage dinonaktifkan demi perlindungan file.
          </span>
        </div>
      )}
      <MapExplorerLayout
        className={className}
        style={mergedStyle}
        sidebar={
          <MapLeftPanel
            map={mapInstance}
            onAnnotationsChange={handleAnnotationsChange}
            onLayersChange={handleLayersChange}
            isCollapsed={isSidebarCollapsed}
            setIsCollapsed={setIsSidebarCollapsed}
          />
        }
        thumbnail={
          <MapTypeThumbnail
            activeBasemap={activeBasemap}
            onToggle={toggleBasemap}
            isSidebarCollapsed={isSidebarCollapsed}
          />
        }
        legend={<MapLegend layers={activeLayers} />}
        map={
          <MapCanvas
            activeBasemap={activeBasemap}
            layers={layers}
            annotations={annotations}
            onMapReady={setMapInstance}
            onActiveLayersChange={setActiveLayers}
          />
        }
      />
    </ImageSyncProvider>
  );
};
