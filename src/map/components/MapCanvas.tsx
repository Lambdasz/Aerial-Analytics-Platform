import React, { useState } from "react";
import { MapContainer, TileLayer, ScaleControl } from "react-leaflet";
import { Button, ButtonGroup } from "@blueprintjs/core";
import MarkerClusterGroup from "react-leaflet-cluster";
import { useImageMarkers } from "./useImageMarkers";
import { DroneImageMarker } from "./DroneImageMarker";
import { MapSyncHandler } from "./MapSyncHandler";
import { MapViewControlBar } from "./MapViewControlBar";
import { useSpatialResultLayers } from "./useSpatialResultLayers";
import "./MapCanvas.css";

const SpatialResultLayer: React.FC = () => {
  useSpatialResultLayers();
  return null;
};

// Basemap definitions based on the specification
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
    // CartoDB now requires a free API key to remove the watermark.
    // Add VITE_CARTO_API_KEY to your .env file
    url: `https://{s}.basemaps.cartocdn.com/light_all/{z}/{x}/{y}{r}.png?key=${import.meta.env.VITE_CARTO_API_KEY || ""}`,
    attribution:
      '&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> contributors &copy; <a href="https://carto.com/attributions">CARTO</a>',
  },
};

type BasemapKey = keyof typeof BASEMAPS;

export const MapCanvas: React.FC = () => {
  const [activeBasemap, setActiveBasemap] = useState<BasemapKey>("osm");
  const { markers } = useImageMarkers(500); // Generate 500 dummy markers for testing clustering

  return (
    <div className="map-wrapper">
      {/* Basemap Switcher */}
      <div className="basemap-switcher">
        <ButtonGroup>
          {(Object.keys(BASEMAPS) as BasemapKey[]).map((key) => (
            <Button
              key={key}
              intent={activeBasemap === key ? "primary" : "none"}
              onClick={() => setActiveBasemap(key)}
              text={BASEMAPS[key].name}
            />
          ))}
        </ButtonGroup>
      </div>

      {/* Map Engine (Fitur 1) */}
      <MapContainer
        center={[-1.247, 116.893]}
        zoom={16}
        minZoom={3}
        maxZoom={21}
        className="leaflet-map-container"
        zoomControl={false} // Zoom control default digantikan oleh MapViewControlBar
      >
        <MapSyncHandler />
        <SpatialResultLayer />
        <MapViewControlBar />
        <ScaleControl position="bottomright" imperial={false} />

        <TileLayer
          key={activeBasemap}
          url={BASEMAPS[activeBasemap].url}
          attribution={BASEMAPS[activeBasemap].attribution}
          maxZoom={21}
        />

        {/* Fitur 2: Image Location Marker (Clustering) */}
        <MarkerClusterGroup chunkedLoading maxClusterRadius={50}>
          {markers.map((marker) => (
            <DroneImageMarker key={marker.image_id} image={marker} />
          ))}
        </MarkerClusterGroup>
      </MapContainer>
    </div>
  );
};
