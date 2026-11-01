import React, { useEffect } from "react";
import { useMap } from "react-leaflet";
import * as L from "leaflet";
import { AnnotationMarkers } from "../molecules/AnnotationMarkers";
import { useMapZoom } from "../hooks/useMapZoom";
import { MIN_POINT_FEATURE_ZOOM, MIN_POLYGON_FEATURE_ZOOM } from "../config/mapConfig";
import { buildAnnotationPopupHtml } from "../lib/popupHtml";
import { isPointInPolygon } from "../lib/geoJson";
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
 *
 * Mengimplementasikan delegasi klik peta untuk poligon anotasi agar area fill di luar
 * batas ortomosaik tetap dapat diklik untuk membuka popup, sedangkan area di atas
 * ortomosaik dapat ditembus langsung oleh klik mouse.
 */
export const AnnotationLayer: React.FC<AnnotationLayerProps> = ({
  annotations,
  minPointZoom = MIN_POINT_FEATURE_ZOOM,
  minPolygonZoom = MIN_POLYGON_FEATURE_ZOOM,
}) => {
  const map = useMap();
  const currentZoom = useMapZoom();

  const visibleAnnotations = annotations.filter((annotation) => {
    const isPoint = annotation.geometry.type === "Point";
    const threshold = isPoint ? minPointZoom : minPolygonZoom;
    return currentZoom >= threshold;
  });

  // Delegasi handler klik peta untuk area fill poligon anotasi di luar batas ortomosaik
  useEffect(() => {
    if (visibleAnnotations.length === 0) return;

    const handleMapClick = (e: L.LeafletMouseEvent) => {
      if (e.originalEvent.defaultPrevented) return;
      if ((e.originalEvent as unknown as { _stopped?: boolean })._stopped) return;

      // PENTING: Periksa apakah klik mengenai elemen citra ortomosaik atau popup aktif
      if (typeof document !== "undefined") {
        const elements = document.elementsFromPoint(
          e.originalEvent.clientX,
          e.originalEvent.clientY,
        );
        const hitOrthoOrPopup = elements.some(
          (el) =>
            el.classList.contains("ortho-image-overlay") ||
            el.classList.contains("ortho-outline-rect") ||
            el.closest(".leaflet-popup") !== null,
        );
        if (hitOrthoOrPopup) return;
      }

      const clickPoint: [number, number] = [e.latlng.lng, e.latlng.lat];

      const polygonAnnotations = visibleAnnotations.filter((a) => a.geometry.type === "Polygon");

      for (const annotation of polygonAnnotations) {
        if (annotation.geometry.type === "Polygon") {
          if (isPointInPolygon(clickPoint, annotation.geometry.coordinates)) {
            // Tandai agar tidak diproses ganda oleh layer AOI di bawahnya
            (
              e.originalEvent as unknown as { _handledByAnnotation?: boolean }
            )._handledByAnnotation = true;

            L.popup({ maxWidth: 320, className: "aoi-custom-popup" })
              .setLatLng(e.latlng)
              .setContent(buildAnnotationPopupHtml(annotation))
              .openOn(map);
            break;
          }
        }
      }
    };

    map.on("click", handleMapClick);
    return () => {
      map.off("click", handleMapClick);
    };
  }, [map, visibleAnnotations]);

  return (
    <AnnotationMarkers annotations={visibleAnnotations} isVisible={visibleAnnotations.length > 0} />
  );
};
