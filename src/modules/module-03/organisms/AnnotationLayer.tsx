import React from "react";
import { AnnotationMarkers } from "../molecules/AnnotationMarkers";
import { useMapZoom } from "../hooks/useMapZoom";
import { MIN_POINT_FEATURE_ZOOM, MIN_POLYGON_FEATURE_ZOOM } from "../config/mapConfig";
import type { AnnotationFeature } from "../types/layer";

export interface AnnotationLayerProps {
  annotations: AnnotationFeature[];
  minPointZoom?: number;
  minPolygonZoom?: number;
}

/**
 * Organism pembungkus AnnotationMarkers yang mengatur visibilitas anotasi
 * berdasarkan level zoom peta Leaflet (LOD threshold):
 * - Poligon / square muncul pada zoom 15 ke atas (minPolygonZoom).
 * - Titik / dot / penanda kecil muncul pada zoom 17 ke atas (minPointZoom).
 */
export const AnnotationLayer: React.FC<AnnotationLayerProps> = ({
  annotations,
  minPointZoom = MIN_POINT_FEATURE_ZOOM,
  minPolygonZoom = MIN_POLYGON_FEATURE_ZOOM,
}) => {
  const currentZoom = useMapZoom();

  const visibleAnnotations = annotations.filter((annotation) => {
    const isPoint = annotation.geometry.type === "Point";
    const threshold = isPoint ? minPointZoom : minPolygonZoom;
    return currentZoom >= threshold;
  });

  return (
    <AnnotationMarkers annotations={visibleAnnotations} isVisible={visibleAnnotations.length > 0} />
  );
};
