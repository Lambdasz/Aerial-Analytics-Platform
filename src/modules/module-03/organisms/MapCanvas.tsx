import React from "react";
import { MapContainer, TileLayer, ScaleControl } from "react-leaflet";
import type * as L from "leaflet";
import { DEFAULT_MAP_VIEW, BASEMAPS, type BasemapKey } from "../config/mapConfig";

import { useImageMarkers } from "../hooks/useImageMarkers";
import { MapSyncHandler } from "./MapSyncHandler";
import { MapViewControlBar } from "./MapViewControlBar";
import { SpatialResultLayer } from "./SpatialResultLayer";
import { ManagedMapLayers } from "./ManagedMapLayers";
import { AnnotationMarkers } from "../molecules/AnnotationMarkers";
import type { LayerPayload } from "../types/map";
import type { LayerState, AnnotationFeature } from "../types/layer";

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
  const { markers } = useImageMarkers(500);

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
      <MapSyncHandler />

      <SpatialResultLayer onLayersChange={onActiveLayersChange} />

      {/* =========================
          MAP CONTROLS
          ========================= */}
      <MapViewControlBar />

      <ScaleControl position="bottomright" imperial={false} />

      {/* =========================
          BASEMAP
          ========================= */}
      <TileLayer
        key={activeBasemap}
        url={BASEMAPS[activeBasemap].url}
        attribution={BASEMAPS[activeBasemap].attribution}
        maxZoom={22}
        maxNativeZoom={BASEMAPS[activeBasemap].maxNativeZoom}
      />

      {/* =========================
          SPATIAL ANNOTATIONS
          ========================= */}
      <AnnotationMarkers annotations={annotations} />

      {/* =========================
          IMAGE LOCATION MARKERS
          ========================= */}
      <ManagedMapLayers layers={layers} markers={markers} />
    </MapContainer>
  );
};
