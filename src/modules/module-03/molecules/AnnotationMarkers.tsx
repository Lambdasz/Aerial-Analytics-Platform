import React from "react";
import { Marker, Polyline, Polygon, Popup } from "react-leaflet";
import type { AnnotationFeature } from "../types/layer";
import { buildAnnotationPopupHtml } from "../lib/popupHtml";
import { estimatePolygonArea } from "../lib/geoJson";

export interface AnnotationMarkersProps {
  annotations: AnnotationFeature[];
  isVisible?: boolean;
}

export const AnnotationMarkers: React.FC<AnnotationMarkersProps> = ({
  annotations,
  isVisible = true,
}) => {
  if (!isVisible) {
    return null;
  }

  // Urutkan anotasi: poligon terluas di bawah, poligon terkecil di atas, garis di atas poligon
  const sortedAnnotations = [...annotations].sort((a, b) => {
    const typeScore = (geom: AnnotationFeature["geometry"]) => {
      if (geom.type === "Polygon") return 1;
      if (geom.type === "LineString") return 2;
      return 3; // Point
    };
    const diff = typeScore(a.geometry) - typeScore(b.geometry);
    if (diff !== 0) return diff;

    if (a.geometry.type === "Polygon" && b.geometry.type === "Polygon") {
      const areaA = estimatePolygonArea(a.geometry.coordinates);
      const areaB = estimatePolygonArea(b.geometry.coordinates);
      return areaB - areaA; // Area lebih besar duluan (bawah), area kecil belakangan (atas)
    }
    return 0;
  });

  return (
    <>
      {sortedAnnotations.map((annotation) => {
        const { geometry } = annotation;
        const keyId = annotation.id || annotation.annotation_id;

        if (geometry.type === "Point") {
          // Data GeoJSON: [longitude, latitude]
          const [longitude, latitude] = geometry.coordinates;

          return (
            <Marker key={keyId} position={[latitude, longitude]}>
              <Popup maxWidth={320} className="aoi-custom-popup">
                <div
                  dangerouslySetInnerHTML={{
                    __html: buildAnnotationPopupHtml(annotation),
                  }}
                />
              </Popup>
            </Marker>
          );
        }

        if (geometry.type === "LineString") {
          // GeoJSON coordinates: [lng, lat][] -> Leaflet positions: [lat, lng][]
          const positions: [number, number][] = geometry.coordinates.map(([lng, lat]) => [
            lat,
            lng,
          ]);

          return (
            <Polyline
              key={keyId}
              positions={positions}
              pathOptions={{
                color: "#f59e0b",
                weight: 4,
                opacity: 0.95,
                lineCap: "round",
                lineJoin: "round",
                dashArray: "6, 8",
              }}
            >
              <Popup maxWidth={320} className="aoi-custom-popup">
                <div
                  dangerouslySetInnerHTML={{
                    __html: buildAnnotationPopupHtml(annotation),
                  }}
                />
              </Popup>
            </Polyline>
          );
        }

        if (geometry.type === "Polygon") {
          // GeoJSON coordinates: number[][][] -> Leaflet positions: [lat, lng][]
          const outerRing = geometry.coordinates[0] || [];
          const positions: [number, number][] = outerRing.map(([lng, lat]) => [lat, lng]);

          return (
            <Polygon
              key={keyId}
              positions={positions}
              pathOptions={{
                color: "#f59e0b",
                fillColor: "#f59e0b",
                fillOpacity: 0.22,
                weight: 2.5,
                dashArray: "5, 5",
              }}
            >
              <Popup maxWidth={320} className="aoi-custom-popup">
                <div
                  dangerouslySetInnerHTML={{
                    __html: buildAnnotationPopupHtml(annotation),
                  }}
                />
              </Popup>
            </Polygon>
          );
        }

        return null;
      })}
    </>
  );
};
