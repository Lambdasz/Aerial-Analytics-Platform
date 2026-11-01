import React, { useState } from "react";
import { Button, Tooltip } from "@blueprintjs/core";
import type { AoiFeature } from "../types/aoi";
import { formatArea, formatPerimeter } from "../lib/geoJson";

export interface AoiCardItemProps {
  item: AoiFeature;
  isEditing?: boolean;
  onFocus: (item: AoiFeature) => void;
  onEdit: (item: AoiFeature) => void;
  onDelete: (item: AoiFeature) => void;
}

export const AoiCardItem: React.FC<AoiCardItemProps> = ({
  item,
  isEditing = false,
  onFocus,
  onEdit,
  onDelete,
}) => {
  const [isHovered, setIsHovered] = useState(false);

  return (
    <div
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
      style={{
        background: "#ffffff",
        borderRadius: "16px",
        padding: "14px 16px",
        border: isEditing ? "1.5px solid #0f172a" : "1px solid rgba(226, 232, 240, 0.8)",
        boxShadow: isEditing
          ? "0 4px 20px -2px rgba(15, 23, 42, 0.12)"
          : isHovered
            ? "0 6px 16px -2px rgba(15, 23, 42, 0.08), 0 2px 6px -1px rgba(15, 23, 42, 0.04)"
            : "0 2px 10px -2px rgba(15, 23, 42, 0.05), 0 1px 3px rgba(15, 23, 42, 0.03)",
        transform: isHovered && !isEditing ? "translateY(-2px)" : "translateY(0)",
        transition: "all 0.2s cubic-bezier(0.16, 1, 0.3, 1)",
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "flex-start",
          justifyContent: "space-between",
          gap: 12,
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: "12px", minWidth: 0, flex: 1 }}>
          {/* Visual Icon Badge */}
          <div
            style={{
              width: "34px",
              height: "34px",
              borderRadius: "10px",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              background: "rgba(99, 102, 241, 0.12)",
              color: "#4f46e5",
              border: "1px solid rgba(99, 102, 241, 0.22)",
              flexShrink: 0,
            }}
          >
            <svg
              width="15"
              height="15"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <path d="M12 2l8 4.5v11L12 22l-8-4.5v-11L12 2z" />
            </svg>
          </div>

          <div style={{ minWidth: 0, flex: 1 }}>
            <div
              style={{
                fontWeight: 600,
                fontSize: "13.5px",
                color: "#0f172a",
                whiteSpace: "nowrap",
                overflow: "hidden",
                textOverflow: "ellipsis",
                lineHeight: 1.3,
                marginBottom: "2px",
              }}
              title={item.properties.name || "Tanpa Nama"}
            >
              {item.properties.name || "Tanpa Nama"}
            </div>
            <div
              style={{
                fontSize: "11px",
                color: "#64748b",
                fontFamily: "monospace",
              }}
            >
              {item.properties.aoi_id}
            </div>
          </div>
        </div>

        <span
          style={{
            background: "rgba(99, 102, 241, 0.1)",
            color: "#4f46e5",
            border: "1px solid rgba(99, 102, 241, 0.22)",
            fontSize: "10px",
            fontWeight: 600,
            padding: "2px 8px",
            borderRadius: "6px",
            whiteSpace: "nowrap",
          }}
        >
          {item.properties.aoi_type || "Area Analisis"}
        </span>
      </div>

      {/* Informasi Metrik di recessed container */}
      <div
        style={{
          display: "flex",
          gap: "12px",
          fontSize: "11.5px",
          color: "#64748b",
          marginTop: "10px",
          padding: "8px 10px",
          background: "#f8fafc",
          borderRadius: "8px",
          border: "1px solid rgba(226, 232, 240, 0.6)",
        }}
      >
        <span>
          Luas:{" "}
          <strong style={{ color: "#334155" }}>
            {formatArea(item.properties.calculated_area_sq_m)}
          </strong>
        </span>
        <span style={{ color: "#cbd5e1" }}>•</span>
        <span>
          Keliling:{" "}
          <strong style={{ color: "#334155" }}>
            {formatPerimeter(item.properties.perimeter_m)}
          </strong>
        </span>
      </div>

      {/* Tombol Aksi Bersih */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: "4px",
          marginTop: "10px",
          justifyContent: "flex-end",
        }}
      >
        <Tooltip content="Fokuskan ke area di peta" placement="top">
          <Button
            minimal
            small
            icon="locate"
            text="Fokus"
            style={{
              color: "#475569",
              fontSize: "11px",
              fontWeight: 500,
              borderRadius: "8px",
              outline: "none",
            }}
            onClick={() => onFocus(item)}
          />
        </Tooltip>

        <Tooltip content="Edit nama, tipe & poligon" placement="top">
          <Button
            minimal
            small
            icon="edit"
            text="Edit"
            style={{
              color: isEditing ? "#0f172a" : "#475569",
              fontSize: "11px",
              fontWeight: isEditing ? 700 : 500,
              borderRadius: "8px",
              outline: "none",
              background: isEditing ? "#f1f5f9" : "transparent",
            }}
            onClick={() => onEdit(item)}
          />
        </Tooltip>

        <Tooltip content="Hapus AOI dari peta" placement="top">
          <Button
            minimal
            small
            icon="trash"
            style={{
              color: "#475569",
              fontSize: "11px",
              fontWeight: 500,
              borderRadius: "8px",
              outline: "none",
            }}
            onClick={() => onDelete(item)}
          />
        </Tooltip>
      </div>
    </div>
  );
};
