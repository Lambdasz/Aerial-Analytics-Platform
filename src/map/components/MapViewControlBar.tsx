import React, { useState, useEffect } from "react";
import { useMap } from "react-leaflet";
import { Button, Tooltip } from "@blueprintjs/core";
import * as L from "leaflet";
import { useImageMarkers } from "./useImageMarkers";
import "./MapViewControlBar.css";

export const MapViewControlBar: React.FC = () => {
  const map = useMap();
  const [zoom, setZoom] = useState(map.getZoom());
  const { markers } = useImageMarkers(500);

  // Sync state zoom dengan peta Leaflet asli
  useEffect(() => {
    const handleZoom = () => setZoom(map.getZoom());
    map.on("zoomend", handleZoom);
    return () => {
      map.off("zoomend", handleZoom);
    };
  }, [map]);

  const handleZoomIn = () => map.zoomIn();
  const handleZoomOut = () => map.zoomOut();

  // FitBounds: Membingkai semua foto proyek ke layar
  const handleFitBounds = () => {
    if (markers.length === 0) return;
    const bounds = L.latLngBounds(markers.map((m) => [m.latitude, m.longitude]));
    map.fitBounds(bounds, { padding: [40, 40], animate: true, duration: 1.5 });
  };

  // Reset North / Kembali ke tengah
  const handleResetNorth = () => {
    map.flyTo([-1.247, 116.893], 16, { animate: true, duration: 1.5 });
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
