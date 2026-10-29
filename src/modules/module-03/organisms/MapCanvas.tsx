import React from "react";
import { MapContainer, TileLayer, ScaleControl } from "react-leaflet";
import type * as L from "leaflet";
import { DEFAULT_MAP_VIEW, BASEMAPS, type BasemapKey } from "../config/mapConfig";

import { MapViewControlBar } from "./MapViewControlBar";
import { SpatialResultLayer } from "./SpatialResultLayer";
import { SavedAoiLayer } from "./SavedAoiLayer";
import { SavedOrthoImageLayer } from "./SavedOrthoImageLayer";
import { ManagedMapLayers } from "./ManagedMapLayers";
import { AnnotationLayer } from "./AnnotationLayer";
import { AutoCenterOnImages } from "./AutoCenterOnImages";
import type { LayerPayload } from "../types/map";
import type { LayerState, AnnotationFeature } from "../types/layer";
import type { DroneImageMetadata } from "../types/marker";

const EMPTY_MARKERS: ReadonlyArray<DroneImageMetadata> = [];

export interface MapCanvasProps {
  /** Basemap aktif yang sedang digunakan */
  activeBasemap?: BasemapKey;
  /** Daftar layer aktif dari Layer Manager */
  layers?: LayerState[];
  /** Daftar fitur anotasi spasial titik */
  annotations?: AnnotationFeature[];
  /** Callback ref saat instance Leaflet map siap */
  onMapReady?: (map: L.Map | null) => void;
  /** Callback saat daftar layer hasil spasial diperbarui */
  onActiveLayersChange?: (layers: LayerPayload[]) => void;
}

/**
 * Organism mesin peta Leaflet (MapContainer, TileLayer, ScaleControl, dan sub-layer).
 */
export const MapCanvas: React.FC<MapCanvasProps> = ({
  activeBasemap = "osm",
  layers = [],
  annotations = [],
  onMapReady,
  onActiveLayersChange,
}) => {
  return (
    <MapContainer
      center={DEFAULT_MAP_VIEW.center}
      zoom={DEFAULT_MAP_VIEW.zoom}
      minZoom={3}
      maxZoom={21}
      className="leaflet-map-container"
      style={{ flex: 1, width: "100%", height: "100%" }}
      zoomControl={false}
      ref={onMapReady}
    >
      {/* =========================
          BASEMAP (TILE LAYER - TILE PANE Z-INDEX 200)
          ========================= */}
      <TileLayer
        key={activeBasemap}
        url={BASEMAPS[activeBasemap].url}
        attribution={BASEMAPS[activeBasemap].attribution}
        maxZoom={22}
        maxNativeZoom={BASEMAPS[activeBasemap].maxNativeZoom}
      />

      {/* =========================
          ORTHOMOSAIC IMAGE OVERLAYS (ORTHO PANE Z-INDEX 250)
          ========================= */}
      <SavedOrthoImageLayer layers={layers} />

      {/* =========================
          AREA OF INTEREST (AOI PANE Z-INDEX 350)
          ========================= */}
      <SavedAoiLayer />

      {/* =========================
          SPATIAL RESULTS (OVERLAY PANE Z-INDEX 400 & MARKER PANE 600)
          ========================= */}
      <SpatialResultLayer layers={layers} onLayersChange={onActiveLayersChange} />

      {/* =========================
          SPATIAL ANNOTATIONS (OVERLAY PANE Z-INDEX 400 & MARKER PANE 600)
          ========================= */}
      <AnnotationLayer annotations={annotations} />

      {/* =========================
          IMAGE LOCATION MARKERS (MARKER PANE Z-INDEX 600)
          ========================= */}
      <ManagedMapLayers layers={layers} markers={EMPTY_MARKERS} />

      {/* =========================
          AUTO-CENTER (ONCE, TO SESSION IMAGERY)
          ========================= */}
      <AutoCenterOnImages />

      {/* =========================
          MAP CONTROLS
          ========================= */}
      <MapViewControlBar />

      <ScaleControl position="bottomright" imperial={false} />
    </MapContainer>
  );
};
