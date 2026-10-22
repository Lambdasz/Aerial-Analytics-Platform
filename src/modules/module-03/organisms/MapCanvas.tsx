import React, { useState } from "react";
import { MapContainer, TileLayer, ScaleControl } from "react-leaflet";
import type * as L from "leaflet";
import { DEFAULT_MAP_VIEW, BASEMAPS, type BasemapKey } from "../config/mapConfig";

import { useImageMarkers } from "../hooks/useImageMarkers";
import { MapSyncHandler } from "./MapSyncHandler";
import { MapViewControlBar } from "./MapViewControlBar";
import { MapLeftPanel } from "./MapLeftPanel";
import { SpatialResultLayer } from "./SpatialResultLayer";
import { ManagedMapLayers } from "./ManagedMapLayers";
import { MapLegend } from "../molecules/MapLegend";
import { MapTypeThumbnail } from "../atoms/MapTypeThumbnail";
import { AnnotationMarkers } from "../molecules/AnnotationMarkers";
import type { LayerPayload } from "../types/map";
import type { LayerState, AnnotationFeature } from "../types/layer";

import "../styles/MapCanvas.css";

export const MapCanvas: React.FC = () => {
  const [activeBasemap, setActiveBasemap] = useState<BasemapKey>("osm");
  const [activeLayers, setActiveLayers] = useState<LayerPayload[]>([]);
  const [layers, setLayers] = useState<LayerState[]>([]);
  const [annotations, setAnnotations] = useState<AnnotationFeature[]>([]);
  const [mapInstance, setMapInstance] = useState<L.Map | null>(null);
  const [isSidebarCollapsed, setIsSidebarCollapsed] = useState<boolean>(false);

  const { markers } = useImageMarkers(500);

  const toggleBasemap = () => {
    setActiveBasemap((current) => (current === "osm" ? "satellite" : "osm"));
  };

  return (
    <div
      className="map-wrapper"
      style={{ position: "relative", width: "100%", height: "100vh", overflow: "hidden" }}
    >
      {/* =========================
          LEFT SIDEBAR (TABS)
          ========================= */}
      <div
        style={{
          position: "absolute",
          top: 0,
          left: 0,
          height: "100%",
          zIndex: 1000,
          pointerEvents: "none",
        }}
      >
        <MapLeftPanel
          map={mapInstance}
          onAnnotationsChange={setAnnotations}
          onLayersChange={setLayers}
          isCollapsed={isSidebarCollapsed}
          setIsCollapsed={setIsSidebarCollapsed}
        />
      </div>

      <div style={{ width: "100%", height: "100%", position: "absolute", top: 0, left: 0 }}>
        {/* =========================
            GMAPS THUMBNAIL SWITCHER
            ========================= */}
        <MapTypeThumbnail
          activeBasemap={activeBasemap}
          onToggle={toggleBasemap}
          isSidebarCollapsed={isSidebarCollapsed}
        />

        {/* =========================
            SPATIAL RESULT LEGEND
            ========================= */}
        <MapLegend layers={activeLayers} />

        {/* =========================
            MAP ENGINE
            ========================= */}
        <MapContainer
          center={DEFAULT_MAP_VIEW.center}
          zoom={DEFAULT_MAP_VIEW.zoom}
          minZoom={3}
          maxZoom={21}
          className="leaflet-map-container"
          style={{ flex: 1, width: "100%", height: "100%" }}
          zoomControl={false}
          ref={setMapInstance}
        >
          <MapSyncHandler />

          <SpatialResultLayer onLayersChange={setActiveLayers} />

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
      </div>
    </div>
  );
};
