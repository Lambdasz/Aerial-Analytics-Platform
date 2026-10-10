import React, { useState } from "react";
import {
  MapContainer,
  TileLayer,
  ScaleControl,
  Marker,
  Popup,
  Polygon,
  ImageOverlay,
} from "react-leaflet";
import type { LatLngBoundsExpression } from "leaflet";
import orthoBounds from "../dummy_image/bounds.json";

import { useImageMarkers } from "./useImageMarkers";
import { DroneImageMarker } from "./DroneImageMarker";
import { MapSyncHandler } from "./MapSyncHandler";
import { MapViewControlBar } from "./MapViewControlBar";
import { MapLeftPanel } from "./MapLeftPanel";
import { useSpatialResultLayers } from "./useSpatialResultLayers";
import { MapLegend } from "./MapLegend";
import { MapTypeThumbnail } from "./MapTypeThumbnail";
import type { LayerPayload } from "../types/map";
import type { LayerState, AnnotationFeature } from "../types/layer";

import "./MapCanvas.css";

// =========================
// SPATIAL RESULT LAYER
// =========================

const SpatialResultLayer: React.FC<{
  onLayersChange: (layers: LayerPayload[]) => void;
}> = ({ onLayersChange }) => {
  const { activeLayers } = useSpatialResultLayers();

  React.useEffect(() => {
    onLayersChange(activeLayers);
  }, [activeLayers, onLayersChange]);

  return null;
};

// =========================
// ANNOTATION MARKERS
// =========================

const AnnotationMarkers: React.FC<{
  annotations: AnnotationFeature[];
}> = ({ annotations }) => {
  return (
    <>
      {annotations.map((annotation) => {
        // Untuk sekarang annotation yang dibuat
        // oleh LayerManager adalah Point.
        if (annotation.geometry.type !== "Point") {
          return null;
        }

        // Data GeoJSON:
        // [longitude, latitude]
        const [longitude, latitude] = annotation.geometry.coordinates;

        return (
          <Marker key={annotation.annotation_id} position={[latitude, longitude]}>
            <Popup>
              <div
                style={{
                  minWidth: "200px",
                  padding: "4px",
                  backgroundColor: "#ffffff",
                  color: "#222222",
                  borderRadius: "6px",
                }}
              >
                <strong
                  style={{
                    display: "block",
                    fontSize: "14px",
                    marginBottom: "8px",
                  }}
                >
                  Spatial Annotation
                </strong>

                <div
                  style={{
                    marginBottom: "10px",
                    lineHeight: "1.4",
                  }}
                >
                  {annotation.text}
                </div>

                <small
                  style={{
                    color: "#666666",
                    lineHeight: "1.5",
                  }}
                >
                  Latitude: {latitude}
                  <br />
                  Longitude: {longitude}
                </small>
              </div>
            </Popup>
          </Marker>
        );
      })}
    </>
  );
};

// =========================
// MANAGED MAP LAYERS
// =========================

const ManagedMapLayers: React.FC<{
  layers: LayerState[];
  markers: ReturnType<typeof useImageMarkers>["markers"];
}> = ({ layers, markers }) => {
  const sortedLayers = [...layers].sort((a, b) => a.z_index - b.z_index);

  return (
    <>
      {sortedLayers.map((layer) => {
        if (!layer.is_visible) {
          return null;
        }
        // =========================
        // ORTHOMOSAIC DRONE
        // =========================

        if (layer.layer_id === "orthomosaic_01") {
          return (
            <ImageOverlay
              key={layer.layer_id}
              url="/drone_ortho.jpg"
              bounds={orthoBounds as LatLngBoundsExpression}
              opacity={layer.opacity}
              zIndex={layer.z_index}
            />
          );
        }

        // =========================
        // DRONE IMAGERY
        // =========================

        if (layer.layer_id === "flight_session_01") {
          return (
            <React.Fragment key={layer.layer_id}>
              {markers.map((marker) => (
                <DroneImageMarker
                  key={`${layer.layer_id}-${marker.image_id}`}
                  image={marker}
                  opacity={layer.opacity}
                />
              ))}
            </React.Fragment>
          );
        }

        // =========================
        // VEGETATION MASK
        // =========================

        if (layer.layer_id === "plugin_veg_mask_01") {
          return (
            <Polygon
              key={layer.layer_id}
              positions={[
                [-1.2465, 116.8925],
                [-1.2465, 116.894],
                [-1.248, 116.894],
                [-1.248, 116.8925],
              ]}
              pathOptions={{
                opacity: layer.opacity,
                fillOpacity: layer.opacity,
              }}
            />
          );
        }

        // =========================
        // TREE DETECTION
        // =========================

        if (layer.layer_id === "plugin_tree_detect_01") {
          return (
            <Polygon
              key={layer.layer_id}
              positions={[
                [-1.247, 116.8928],
                [-1.247, 116.8932],
                [-1.2474, 116.8932],
                [-1.2474, 116.8928],
              ]}
              pathOptions={{
                opacity: layer.opacity,
                fillOpacity: layer.opacity,
              }}
            />
          );
        }

        return null;
      })}
    </>
  );
};
// =========================
// BASEMAP DEFINITIONS
// =========================

const BASEMAPS = {
  osm: {
    name: "OpenStreetMap",
    url: "https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png",
    attribution:
      '&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> contributors',
    maxNativeZoom: 19,
  },

  satellite: {
    name: "Esri Satellite",
    url: "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}",
    attribution:
      "Tiles &copy; Esri &mdash; Source: Esri, i-cubed, USDA, USGS, AEX, GeoEye, Getmapping, Aerogrid, IGN, IGP, UPR-EGP, and the GIS User Community",
    maxNativeZoom: 18,
  },

  positron: {
    name: "CartoDB Positron",

    // CartoDB now requires a free API key
    // to remove the watermark.
    // Add VITE_CARTO_API_KEY to your .env file.
    url: `https://{s}.basemaps.cartocdn.com/light_all/{z}/{x}/{y}{r}.png?key=${
      import.meta.env.VITE_CARTO_API_KEY || ""
    }`,

    attribution:
      '&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> contributors &copy; <a href="https://carto.com/attributions">CARTO</a>',
    maxNativeZoom: 19,
  },
};

type BasemapKey = keyof typeof BASEMAPS;

// =========================
// MAP CANVAS
// =========================

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
          center={[-1.247, 116.893]}
          zoom={16}
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
