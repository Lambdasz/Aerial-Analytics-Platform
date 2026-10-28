import React, { useState, useEffect } from "react";
import { useMap } from "react-leaflet";
import { Button, ButtonGroup, Tooltip } from "@blueprintjs/core";
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
      <ButtonGroup vertical>
        <Tooltip content="Zoom In" placement="right">
          <Button icon="plus" onClick={handleZoomIn} />
        </Tooltip>

        <Button className="zoom-indicator" text={`${zoom}z`} disabled />

        <Tooltip content="Zoom Out" placement="right">
          <Button icon="minus" onClick={handleZoomOut} />
        </Tooltip>
      </ButtonGroup>

      {/* Navigation Controllers */}
      <ButtonGroup vertical>
        <Tooltip content="Bidik Keseluruhan Proyek" placement="right">
          <Button icon="zoom-to-fit" intent="primary" onClick={handleFitBounds} />
        </Tooltip>
        <Tooltip content="Reset Koordinat Awal" placement="right">
          <Button icon="locate" onClick={handleResetNorth} />
        </Tooltip>
        <Tooltip content="Layar Penuh (Fullscreen)" placement="right">
          <Button icon="fullscreen" onClick={handleFullscreen} />
        </Tooltip>
      </ButtonGroup>
    </div>
  );
};
