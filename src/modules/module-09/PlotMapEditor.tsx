import { useEffect, useRef, useState } from "react";
import * as L from "leaflet";
import { MapContainer, TileLayer, useMap } from "react-leaflet";
import type { Layer as LeafletLayer, Polygon as LeafletPolygon } from "leaflet";
import type { Feature, Polygon as GeoPolygon } from "geojson";
import "../module-03/leaflet-setup";
import "@geoman-io/leaflet-geoman-free";

import { BASEMAPS, DEFAULT_MAP_VIEW } from "../module-03/config/mapConfig";
import { useSessionImageMarkers } from "../module-03/hooks/useSessionImageMarkers";
import type { PlotGeometry, PlotRecord } from "./plotCommands";

/** Leaflet `pm` plugin surface used here (not part of the core typings). */
type GeomanMap = L.Map & {
  pm?: {
    enableDraw?: (shape: string) => void;
    disableDraw?: () => void;
  };
};

type GeomanCreateEvent = L.LeafletEvent & { layer: LeafletLayer };

const PLOT_STYLE: L.PathOptions = {
  color: "#1a7f37",
  weight: 2,
  fillColor: "#1a7f37",
  fillOpacity: 0.18,
};

/** Renders stored plots and reports each drawn polygon back to the page. */
function PlotsLayer({
  plots,
  focusPoints,
  onDrawn,
}: {
  plots: PlotRecord[];
  focusPoints: [number, number][];
  onDrawn: (geometry: PlotGeometry) => void;
}) {
  const map = useMap() as GeomanMap;
  const groupRef = useRef<L.FeatureGroup | null>(null);
  const onDrawnRef = useRef(onDrawn);

  useEffect(() => {
    onDrawnRef.current = onDrawn;
  }, [onDrawn]);

  // Keep one feature group for the stored plots; redraw when the list changes.
  useEffect(() => {
    const group = L.featureGroup().addTo(map);
    groupRef.current = group;
    return () => {
      group.remove();
      groupRef.current = null;
    };
  }, [map]);

  useEffect(() => {
    const group = groupRef.current;
    if (!group) return;
    group.clearLayers();
    for (const plot of plots) {
      const feature: Feature<GeoPolygon> = {
        type: "Feature",
        properties: { name: plot.name },
        geometry: plot.geometry,
      };
      const layer = L.geoJSON(feature, { style: PLOT_STYLE });
      layer.bindTooltip(plot.name, { sticky: true });
      group.addLayer(layer);
    }
    if (plots.length > 0) {
      const bounds = group.getBounds();
      if (bounds.isValid()) map.fitBounds(bounds, { padding: [24, 24], maxZoom: 19 });
    } else if (focusPoints.length > 0) {
      const bounds = L.latLngBounds(focusPoints);
      if (bounds.isValid()) map.fitBounds(bounds, { padding: [24, 24], maxZoom: 18 });
    }
  }, [plots, focusPoints, map]);

  // Geoman drawing: each finished polygon becomes a plot candidate.
  // (The parent is notified through `onDrawn`; it resets its drawing state.)
  useEffect(() => {
    const handleCreate = (event: GeomanCreateEvent) => {
      const layer = event.layer as LeafletPolygon;
      const latlngs = layer.getLatLngs();
      const ring = Array.isArray(latlngs[0]) ? latlngs[0] : latlngs;
      const coordinates = (ring as L.LatLng[]).map((point) => [point.lng, point.lat]);
      if (coordinates.length < 3) return;
      const first = coordinates[0];
      const last = coordinates[coordinates.length - 1];
      if (first[0] !== last[0] || first[1] !== last[1]) {
        coordinates.push(first);
      }
      map.pm?.disableDraw?.();
      map.removeLayer(layer);
      onDrawnRef.current({ type: "Polygon", coordinates: [coordinates] });
    };

    map.on("pm:create", handleCreate);
    return () => {
      map.off("pm:create", handleCreate);
    };
  }, [map]);

  return null;
}

/** Small map for defining plots: draw a polygon, it is handed back for saving. */
export function PlotMapEditor({
  plots,
  startDrawing,
  onDrawingFinished,
  onDrawn,
}: {
  plots: PlotRecord[];
  startDrawing: boolean;
  onDrawingFinished: () => void;
  onDrawn: (geometry: PlotGeometry) => void;
}) {
  const [map, setMap] = useState<L.Map | null>(null);
  const { markers } = useSessionImageMarkers();
  const focusPoints = markers.map(
    (marker) => [marker.latitude, marker.longitude] as [number, number],
  );

  useEffect(() => {
    if (!map) return;
    const geoman = map as GeomanMap;
    if (startDrawing) {
      geoman.pm?.enableDraw?.("Polygon");
      return;
    }
    geoman.pm?.disableDraw?.();
  }, [map, startDrawing]);

  return (
    <MapContainer
      center={DEFAULT_MAP_VIEW.center}
      zoom={DEFAULT_MAP_VIEW.zoom}
      minZoom={3}
      maxZoom={21}
      style={{ flex: 1, width: "100%", height: "100%", minHeight: 320, borderRadius: 14 }}
      zoomControl
      ref={setMap}
    >
      <TileLayer
        url={BASEMAPS.osm.url}
        attribution={BASEMAPS.osm.attribution}
        maxZoom={22}
        maxNativeZoom={BASEMAPS.osm.maxNativeZoom}
      />
      <PlotsLayer
        plots={plots}
        focusPoints={focusPoints}
        onDrawn={(geometry) => {
          onDrawn(geometry);
          onDrawingFinished();
        }}
      />
    </MapContainer>
  );
}
