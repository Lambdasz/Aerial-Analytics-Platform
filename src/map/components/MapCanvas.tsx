import React, { useState } from "react";
import {
  MapContainer,
  TileLayer,
  ScaleControl,
  Marker,
  Popup,
  Polygon,
} from "react-leaflet";
import { Button, ButtonGroup } from "@blueprintjs/core";

import { useImageMarkers } from "./useImageMarkers";
import { DroneImageMarker } from "./DroneImageMarker";
import { MapSyncHandler } from "./MapSyncHandler";
import { MapViewControlBar } from "./MapViewControlBar";
import { AoiDemoToolbar } from "./AoiDemoToolbar";
import { useSpatialResultLayers } from "./useSpatialResultLayers";
import { MapLegend } from "./MapLegend";
import { LayerManager } from "./LayerManager";

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
        const [longitude, latitude] =
          annotation.geometry.coordinates;

        return (
          <Marker
            key={annotation.annotation_id}
            position={[latitude, longitude]}
          >
            <Popup>
              <div
                style={{
                  minWidth: "180px",
                }}
              >
                <strong>
                  Spatial Annotation
                </strong>

                <div
                  style={{
                    marginTop: "8px",
                    marginBottom: "8px",
                  }}
                >
                  {annotation.text}
                </div>

                <small>
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
  const sortedLayers = [...layers].sort(
    (a, b) => a.z_index - b.z_index,
  );

  return (
    <>
      {sortedLayers.map((layer) => {
        if (!layer.is_visible) {
          return null;
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
  },

  satellite: {
    name: "Esri Satellite",
    url: "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}",
    attribution:
      "Tiles &copy; Esri &mdash; Source: Esri, i-cubed, USDA, USGS, AEX, GeoEye, Getmapping, Aerogrid, IGN, IGP, UPR-EGP, and the GIS User Community",
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
  },
};

type BasemapKey = keyof typeof BASEMAPS;

// =========================
// MAP CANVAS
// =========================

export const MapCanvas: React.FC = () => {
  const [activeBasemap, setActiveBasemap] =
    useState<BasemapKey>("osm");

  const [activeLayers, setActiveLayers] =
    useState<LayerPayload[]>([]);

  const [layers, setLayers] =
    useState<LayerState[]>([]);

  const [annotations, setAnnotations] =
    useState<AnnotationFeature[]>([]);

  const { markers } = useImageMarkers(500);

  return (
    <div className="map-wrapper">
      {/* =========================
          BASEMAP SWITCHER
          ========================= */}

      <div className="basemap-switcher">
        <ButtonGroup>
          {(Object.keys(BASEMAPS) as BasemapKey[]).map(
            (key) => (
              <Button
                key={key}
                intent={
                  activeBasemap === key
                    ? "primary"
                    : "none"
                }
                onClick={() =>
                  setActiveBasemap(key)
                }
                text={BASEMAPS[key].name}
              />
            ),
          )}
        </ButtonGroup>
      </div>

      {/* =========================
          SPATIAL RESULT LEGEND
          ========================= */}

      <MapLegend layers={activeLayers} />

      {/* =========================
          LAYER MANAGEMENT
          ========================= */}

      <LayerManager
        onAnnotationsChange={setAnnotations}
        onLayersChange={setLayers}
      />

      {/* =========================
          MAP ENGINE
          ========================= */}

      <MapContainer
        center={[-1.247, 116.893]}
        zoom={16}
        minZoom={3}
        maxZoom={21}
        className="leaflet-map-container"
        zoomControl={false}
      >
        <MapSyncHandler />

        <SpatialResultLayer
          onLayersChange={setActiveLayers}
        />

        <MapViewControlBar />

        <AoiDemoToolbar />

        <ScaleControl
          position="bottomright"
          imperial={false}
        />

        {/* =========================
            BASEMAP
            ========================= */}

        <TileLayer
          key={activeBasemap}
          url={BASEMAPS[activeBasemap].url}
          attribution={
            BASEMAPS[activeBasemap].attribution
          }
          maxZoom={21}
        />

        {/* =========================
            SPATIAL ANNOTATIONS
            ========================= */}

        <AnnotationMarkers
          annotations={annotations}
        />

        {/* =========================
            IMAGE LOCATION MARKERS
            ========================= */}

        <ManagedMapLayers
          layers={layers}
          markers={markers}
        />
      </MapContainer>
    </div>
  );
};