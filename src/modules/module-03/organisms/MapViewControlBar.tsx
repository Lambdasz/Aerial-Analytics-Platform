import React from "react";
import { useMap } from "react-leaflet";
import { Button, Tooltip } from "@blueprintjs/core";
import * as L from "leaflet";
import { DEFAULT_MAP_VIEW } from "../config/mapConfig";
import { useMapZoom } from "../hooks/useMapZoom";
import "../styles/MapViewControlBar.css";

export const MapViewControlBar: React.FC = () => {
  const map = useMap();
  const zoom = useMapZoom();

  const handleZoomIn = () => map.zoomIn();
  const handleZoomOut = () => map.zoomOut();

  // FitBounds: Membingkai keseluruhan layer proyek (AOI, Spatial Results, Orthomosaic) atau fallback ke default bounds
  const handleFitBounds = () => {
    const bounds = L.latLngBounds([]);

    map.eachLayer((layer) => {
      // Abaikan base tile layers
      if (layer instanceof L.TileLayer) return;

      if (
        "getBounds" in layer &&
        typeof (layer as { getBounds: () => L.LatLngBounds }).getBounds === "function"
      ) {
        try {
          const b = (layer as { getBounds: () => L.LatLngBounds }).getBounds();
          if (b && typeof b.isValid === "function" && b.isValid()) {
            bounds.extend(b);
          }
        } catch {
          // Abaikan layer tanpa bounds valid
        }
      } else if (
        "getLatLng" in layer &&
        typeof (layer as { getLatLng: () => L.LatLng }).getLatLng === "function"
      ) {
        try {
          const latlng = (layer as { getLatLng: () => L.LatLng }).getLatLng();
          if (latlng) bounds.extend(latlng);
        } catch {
          // Abaikan layer tanpa latlng valid
        }
      }
    });

    if (bounds.isValid()) {
      map.fitBounds(bounds, { padding: [40, 40], animate: true, duration: 1.5 });
    } else {
      map.fitBounds(DEFAULT_MAP_VIEW.bounds, { padding: [40, 40], animate: true, duration: 1.5 });
    }
  };

  // Reset North / Kembali ke tengah orthomosaic Kariangau
  const handleResetNorth = () => {
    map.flyTo(DEFAULT_MAP_VIEW.center, DEFAULT_MAP_VIEW.zoom, { animate: true, duration: 1.5 });
  };

  // HTML5 Native Fullscreen
  const handleFullscreen = () => {
    const elem = document.documentElement;
    if (!document.fullscreenElement) {
      elem.requestFullscreen().catch((err: unknown) => {
        const errorMsg = err instanceof Error ? err.message : String(err);
        console.error(`Gagal masuk ke mode Fullscreen: ${errorMsg}`);
      });
    } else {
      void document.exitFullscreen();
    }
  };

  return (
    <div className="map-view-control-bar">
      {/* Zoom Controllers */}
      <div className="control-capsule">
        <Tooltip content="Zoom In" placement="left">
          <Button icon="plus" minimal onClick={handleZoomIn} />
        </Tooltip>

        <div className="control-divider" />

        <div className="zoom-indicator">{zoom}z</div>

        <div className="control-divider" />

        <Tooltip content="Zoom Out" placement="left">
          <Button icon="minus" minimal onClick={handleZoomOut} />
        </Tooltip>
      </div>

      {/* Navigation Controllers */}
      <div className="control-capsule">
        <Tooltip content="Bidik Keseluruhan Proyek" placement="left">
          <Button icon="zoom-to-fit" minimal onClick={handleFitBounds} />
        </Tooltip>

        <div className="control-divider" />

        <Tooltip content="Reset Koordinat Awal" placement="left">
          <Button icon="locate" minimal onClick={handleResetNorth} />
        </Tooltip>

        <div className="control-divider" />

        <Tooltip content="Layar Penuh (Fullscreen)" placement="left">
          <Button icon="fullscreen" minimal onClick={handleFullscreen} />
        </Tooltip>
      </div>
    </div>
  );
};
