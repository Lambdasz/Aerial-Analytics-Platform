import { useCallback, useEffect, useRef, useState } from "react";
import * as L from "leaflet";
import "../leaflet-setup";
import "@geoman-io/leaflet-geoman-free";
import type { AnnotationGeomType, SpatialGeometry } from "../types/layer";
import { latLngsToLineString, latLngsToPolygon } from "../lib/geoJson";

export type { AnnotationGeomType };

export interface UseAnnotationMapPickerOptions {
  map: L.Map | null;
  isOpen: boolean;
  geomType: AnnotationGeomType;
  draftGeometry: SpatialGeometry | null;
  coordMode: "map" | "manual";
  onGeometryPicked: (geom: SpatialGeometry, latStr?: string, lngStr?: string) => void;
  onDrawingStateChange?: (isPicking: boolean, geomType: AnnotationGeomType) => void;
  onError?: (err: string | null) => void;
}

export interface UseAnnotationMapPickerReturn {
  isPickingLocation: boolean;
  isTemporarilyClosedForMap: boolean;
  togglePickLocation: () => void;
  clearPreviewLayer: () => void;
  stopPicking: () => void;
}

/**
 * Custom hook untuk mengelola interaksi pemungutan lokasi/geometri pada peta Leaflet dan Geoman.
 * Menangani cursor crosshair, event map click, event pm:create Geoman,
 * sinkronisasi preview layer sementara, dan event window picking.
 */
export function useAnnotationMapPicker({
  map,
  isOpen,
  geomType,
  draftGeometry,
  coordMode,
  onGeometryPicked,
  onDrawingStateChange,
  onError,
}: UseAnnotationMapPickerOptions): UseAnnotationMapPickerReturn {
  const [isPickingLocation, setIsPickingLocation] = useState(false);
  const [isTemporarilyClosedForMap, setIsTemporarilyClosedForMap] = useState(false);

  // Sinkronisasi state saat isOpen berubah (hindari setState dalam effect)
  const [prevIsOpen, setPrevIsOpen] = useState(isOpen);
  if (isOpen !== prevIsOpen) {
    setPrevIsOpen(isOpen);
    if (!isOpen) {
      setIsPickingLocation(false);
      setIsTemporarilyClosedForMap(false);
    }
  }

  // Ref layer pratinjau sementara di peta Leaflet
  const previewLayerRef = useRef<L.Layer | null>(null);

  const clearPreviewLayer = useCallback(() => {
    if (previewLayerRef.current && map) {
      map.removeLayer(previewLayerRef.current);
      previewLayerRef.current = null;
    }
  }, [map]);

  const stopPicking = useCallback(() => {
    if (map) {
      map.pm?.disableDraw?.();
      map.getContainer().style.cursor = "";
    }
    setIsPickingLocation(false);
    setIsTemporarilyClosedForMap(false);
    onDrawingStateChange?.(false, geomType);
  }, [map, geomType, onDrawingStateChange]);

  // Bersihkan interaksi peta eksternal saat modal ditutup
  useEffect(() => {
    if (!isOpen) {
      clearPreviewLayer();
      if (map) {
        map.pm?.disableDraw?.();
        map.getContainer().style.cursor = "";
      }
    }
  }, [isOpen, map, clearPreviewLayer]);

  // Mode Titik: Listener klik pada peta
  useEffect(() => {
    if (!map || !isPickingLocation || geomType !== "Point") return;

    const container = map.getContainer();
    const prevCursor = container.style.cursor;
    container.style.cursor = "crosshair";

    const handleMapClick = (e: L.LeafletMouseEvent) => {
      const lat = Number(e.latlng.lat.toFixed(6));
      const lng = Number(e.latlng.lng.toFixed(6));
      onGeometryPicked({ type: "Point", coordinates: [lng, lat] }, lat.toString(), lng.toString());
      setIsPickingLocation(false);
      setIsTemporarilyClosedForMap(false);
      onDrawingStateChange?.(false, "Point");
      onError?.(null);
    };

    map.on("click", handleMapClick);

    return () => {
      container.style.cursor = prevCursor;
      map.off("click", handleMapClick);
    };
  }, [map, isPickingLocation, geomType, onDrawingStateChange, onGeometryPicked, onError]);

  // Mode Garis & Poligon: Listener Geoman pm:create
  useEffect(() => {
    if (!map) return;

    const handlePmCreate = (e: { shape: string; layer: L.Layer }) => {
      if (!isPickingLocation) return;

      if (geomType === "LineString" && (e.shape === "Line" || e.layer instanceof L.Polyline)) {
        const latLngs = (e.layer as L.Polyline).getLatLngs();
        const coords = latLngsToLineString(latLngs);
        if (coords.length >= 2) {
          onGeometryPicked({ type: "LineString", coordinates: coords });
          if (previewLayerRef.current && previewLayerRef.current !== e.layer) {
            map.removeLayer(previewLayerRef.current);
          }
          previewLayerRef.current = e.layer;
          setIsPickingLocation(false);
          setIsTemporarilyClosedForMap(false);
          onDrawingStateChange?.(false, "LineString");
          map.pm?.disableDraw?.();
          onError?.(null);
        }
      } else if (
        geomType === "Polygon" &&
        (e.shape === "Polygon" || e.shape === "Rectangle" || e.layer instanceof L.Polygon)
      ) {
        const latLngs = (e.layer as L.Polygon).getLatLngs();
        const poly = latLngsToPolygon(latLngs);
        if (poly.length > 0 && poly[0].length >= 4) {
          onGeometryPicked({ type: "Polygon", coordinates: poly });
          if (previewLayerRef.current && previewLayerRef.current !== e.layer) {
            map.removeLayer(previewLayerRef.current);
          }
          previewLayerRef.current = e.layer;
          setIsPickingLocation(false);
          setIsTemporarilyClosedForMap(false);
          onDrawingStateChange?.(false, "Polygon");
          map.pm?.disableDraw?.();
          onError?.(null);
        }
      }
    };

    map.on("pm:create", handlePmCreate as unknown as L.LeafletEventHandlerFn);

    return () => {
      map.off("pm:create", handlePmCreate as unknown as L.LeafletEventHandlerFn);
    };
  }, [map, isPickingLocation, geomType, onDrawingStateChange, onGeometryPicked, onError]);

  // Sinkronisasi layer pratinjau sementara di peta saat koordinat berubah
  useEffect(() => {
    if (!map) return;

    if (isPickingLocation && (geomType === "LineString" || geomType === "Polygon")) {
      return;
    }

    if (!draftGeometry) {
      clearPreviewLayer();
      return;
    }

    if (
      previewLayerRef.current &&
      (geomType === "LineString" || geomType === "Polygon") &&
      coordMode === "map"
    ) {
      return;
    }

    clearPreviewLayer();

    if (draftGeometry.type === "Point") {
      const [lng, lat] = draftGeometry.coordinates;
      const previewIcon = L.divIcon({
        className: "annotation-preview-marker",
        html: `<div style="
          width: 24px;
          height: 24px;
          background: #f59e0b;
          border: 2.5px solid #ffffff;
          border-radius: 50%;
          box-shadow: 0 2px 8px rgba(0,0,0,0.4);
          display: flex;
          align-items: center;
          justify-content: center;
        ">
          <div style="width: 6px; height: 6px; background: white; border-radius: 50%;"></div>
        </div>`,
        iconSize: [24, 24],
        iconAnchor: [12, 12],
      });

      const marker = L.marker([lat, lng], {
        icon: previewIcon,
        zIndexOffset: 1000,
      }).addTo(map);

      marker.bindTooltip("Titik Anotasi Baru", {
        permanent: false,
        direction: "top",
        offset: [0, -12],
      });

      previewLayerRef.current = marker;
    } else if (draftGeometry.type === "LineString") {
      const positions: [number, number][] = draftGeometry.coordinates.map(([lng, lat]) => [
        lat,
        lng,
      ]);
      const polyline = L.polyline(positions, {
        color: "#f59e0b",
        weight: 4,
        dashArray: "6, 8",
        opacity: 0.95,
      }).addTo(map);
      previewLayerRef.current = polyline;
    } else if (draftGeometry.type === "Polygon") {
      const ring = (draftGeometry.coordinates[0] || []) as [number, number][];
      const positions: [number, number][] = ring.map(([lng, lat]) => [lat, lng]);
      const polygon = L.polygon(positions, {
        color: "#f59e0b",
        fillColor: "#f59e0b",
        fillOpacity: 0.25,
        weight: 2.5,
        dashArray: "5, 5",
      }).addTo(map);
      previewLayerRef.current = polygon;
    }
  }, [map, draftGeometry, isPickingLocation, geomType, coordMode, clearPreviewLayer]);

  // Event listener eksternal untuk pembatalan atau penyelesaian picking dari sidebar banner
  useEffect(() => {
    const handleCancelPicking = () => {
      stopPicking();
    };

    const handleFinishPicking = () => {
      stopPicking();
    };

    window.addEventListener("annotation:cancel-picking", handleCancelPicking);
    window.addEventListener("annotation:finish-picking", handleFinishPicking);

    return () => {
      window.removeEventListener("annotation:cancel-picking", handleCancelPicking);
      window.removeEventListener("annotation:finish-picking", handleFinishPicking);
    };
  }, [stopPicking]);

  // Bersihkan preview layer saat unmount
  useEffect(() => {
    return () => {
      clearPreviewLayer();
      if (map) {
        map.pm?.disableDraw?.();
        map.getContainer().style.cursor = "";
      }
    };
  }, [map, clearPreviewLayer]);

  const togglePickLocation = () => {
    if (!map) {
      onError?.("Peta belum siap untuk menggambar atau memilih lokasi.");
      return;
    }
    onError?.(null);

    if (isPickingLocation) {
      stopPicking();
      return;
    }

    clearPreviewLayer();

    setIsPickingLocation(true);
    setIsTemporarilyClosedForMap(true);
    onDrawingStateChange?.(true, geomType);

    if (geomType === "Point") {
      // Map on click listener akan aktif via effect
    } else if (geomType === "LineString") {
      map.pm?.enableDraw?.("Line", {
        pathOptions: {
          color: "#f59e0b",
          weight: 4,
          opacity: 0.95,
          lineCap: "round",
          lineJoin: "round",
          dashArray: "6, 8",
        },
      });
    } else if (geomType === "Polygon") {
      map.pm?.enableDraw?.("Polygon", {
        pathOptions: {
          color: "#f59e0b",
          fillColor: "#f59e0b",
          fillOpacity: 0.25,
          weight: 2.5,
          dashArray: "5, 5",
        },
      });
    }
  };

  return {
    isPickingLocation,
    isTemporarilyClosedForMap,
    togglePickLocation,
    clearPreviewLayer,
    stopPicking,
  };
}
