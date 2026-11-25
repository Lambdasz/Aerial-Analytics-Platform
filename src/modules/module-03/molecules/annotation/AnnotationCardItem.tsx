import React from "react";
import { Button, Tooltip } from "@blueprintjs/core";
import type { AnnotationFeature } from "../../types/layer";
import { GeomTypePill } from "../../atoms/GeomTypePill";
import { calculateLineLengthMeters, calculateGeodesicAreaMeters } from "../../lib/geoJson";

export interface AnnotationCardItemProps {
  annotation: AnnotationFeature;
  isEditing: boolean;
  hasMap: boolean;
  onFocus: (annotation: AnnotationFeature) => void;
  onEdit: (annotation: AnnotationFeature) => void;
  onDelete: (id: string) => void;
}

export const AnnotationCardItem: React.FC<AnnotationCardItemProps> = ({
  annotation,
  isEditing,
  hasMap,
  onFocus,
  onEdit,
  onDelete,
}) => {
  const { geometry, annotation_id, id, text } = annotation;
  const targetId = id || annotation_id || "";

  let metaText = "";
  if (geometry.type === "Point") {
    const [lng, lat] = geometry.coordinates;
    metaText = `${lat.toFixed(6)}, ${lng.toFixed(6)}`;
  } else if (geometry.type === "LineString") {
    const len = calculateLineLengthMeters(geometry.coordinates);
    const lenStr = len >= 1000 ? `${(len / 1000).toFixed(2)} km` : `${len.toFixed(0)} m`;
    metaText = `${geometry.coordinates.length} titik • ${lenStr}`;
  } else if (geometry.type === "Polygon") {
    const ring = (geometry.coordinates[0] || []) as [number, number][];
    const area = calculateGeodesicAreaMeters(ring);
    const areaStr = area >= 10000 ? `${(area / 10000).toFixed(2)} ha` : `${area.toFixed(0)} m²`;
    metaText = `${ring.length} sudut • ${areaStr}`;
  }

  return (
    <div
      style={{
        padding: "14px 16px",
        background: "#ffffff",
        borderRadius: "16px",
        boxShadow: isEditing
          ? "0 4px 20px -2px rgba(15, 23, 42, 0.12), 0 0 0 1.5px #0f172a"
          : "0 2px 12px -2px rgba(0, 0, 0, 0.05), 0 1px 3px rgba(0, 0, 0, 0.02)",
        border: "none",
        boxSizing: "border-box",
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "flex-start",
          justifyContent: "space-between",
          gap: "8px",
        }}
      >
        <div style={{ minWidth: 0, flex: 1 }}>
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: "6px",
              marginBottom: "4px",
            }}
          >
            <GeomTypePill geomType={geometry.type} translateLabel />
            <div
              style={{
                fontWeight: 600,
                fontSize: "13px",
                color: "var(--color-ink)",
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap",
              }}
              title={text}
            >
              {text}
            </div>
          </div>

          <div
            style={{
              fontSize: "11px",
              color: "var(--color-slate)",
              fontFamily: "monospace",
            }}
          >
            {metaText}
          </div>
        </div>

        <div style={{ display: "flex", gap: "4px", flexShrink: 0 }}>
          {hasMap && (
            <Tooltip content="Fokuskan peta ke anotasi ini" placement="top">
              <Button
                minimal
                small
                icon="locate"
                style={{ color: "#64748b", borderRadius: "6px" }}
                onClick={() => onFocus(annotation)}
              />
            </Tooltip>
          )}
          <Tooltip content="Edit anotasi" placement="top">
            <Button
              minimal
              small
              icon="edit"
              style={{
                color: isEditing ? "#0f172a" : "#64748b",
                background: isEditing ? "#f1f5f9" : "transparent",
                borderRadius: "6px",
              }}
              onClick={() => onEdit(annotation)}
            />
          </Tooltip>
          <Tooltip content="Hapus anotasi" placement="top">
            <Button
              minimal
              small
              icon="trash"
              style={{ color: "#64748b", borderRadius: "6px" }}
              onClick={() => onDelete(targetId)}
            />
          </Tooltip>
        </div>
      </div>
    </div>
  );
};
