import React from "react";
import { Marker, Popup } from "react-leaflet";
import type { AnnotationFeature } from "../types/layer";

export interface AnnotationMarkersProps {
  annotations: AnnotationFeature[];
}

export const AnnotationMarkers: React.FC<AnnotationMarkersProps> = ({ annotations }) => {
  return (
    <>
      {annotations.map((annotation) => {
        // Untuk sekarang annotation yang dibuat oleh LayerManager adalah Point.
        if (annotation.geometry.type !== "Point") {
          return null;
        }

        // Data GeoJSON: [longitude, latitude]
        const [longitude, latitude] = annotation.geometry.coordinates;

        return (
          <Marker key={annotation.annotation_id} position={[latitude, longitude]}>
            <Popup>
              <div
                style={{
                  minWidth: "200px",
                  padding: "4px",
                  backgroundColor: "#ffffff",
                  color: "#222222",
                  borderRadius: "6px",
                }}
              >
                <strong
                  style={{
                    display: "block",
                    fontSize: "14px",
                    marginBottom: "8px",
                  }}
                >
                  Spatial Annotation
                </strong>

                <div
                  style={{
                    marginBottom: "10px",
                    lineHeight: "1.4",
                  }}
                >
                  {annotation.text}
                </div>

                <small
                  style={{
                    color: "#666666",
                    lineHeight: "1.5",
                  }}
                >
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
