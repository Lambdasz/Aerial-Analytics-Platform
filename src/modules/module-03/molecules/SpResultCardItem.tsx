import React from "react";
import { Button, Tag, Tooltip } from "@blueprintjs/core";
import type { SpatialResultSummary } from "../types/spatialResult";
import { ResultPhoto } from "./ResultPhoto";

export interface SpResultCardItemProps {
  item: SpatialResultSummary;
  isSelected: boolean;
  isProcessing: boolean;
  onFocus: (item: SpatialResultSummary) => void;
  onEdit: (item: SpatialResultSummary) => void;
  onDelete: (item: SpatialResultSummary) => void;
}

export const SpResultCardItem: React.FC<SpResultCardItemProps> = ({
  item,
  isSelected,
  isProcessing,
  onFocus,
  onEdit,
  onDelete,
}) => {
  const isCorrupted = Boolean(item.is_corrupted);

  const getGeomIntent = (geomType: string) => {
    switch (geomType.toLowerCase()) {
      case "polygon":
        return "success";
      case "point":
        return "primary";
      case "linestring":
        return "warning";
      case "imageoverlay":
        return "none";
      default:
        return "none";
    }
  };

  return (
    <div
      className={`sp-result-card-item ${isSelected ? "selected" : ""}`}
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        padding: "8px 10px",
        marginBottom: "6px",
        borderRadius: "6px",
        backgroundColor: isSelected ? "rgba(19, 124, 189, 0.08)" : "#ffffff",
        border: isSelected
          ? "1px solid #106ba3"
          : isCorrupted
            ? "1px solid rgba(219, 55, 55, 0.4)"
            : "1px solid rgba(16, 22, 26, 0.12)",
        boxShadow: isSelected ? "0 0 0 1px #106ba3" : "0 1px 2px rgba(16, 22, 26, 0.04)",
        transition: "all 0.15s ease",
        opacity: isProcessing ? 0.6 : 1,
      }}
    >
      {/* Kiri: Foto Thumbnail */}
      <div style={{ display: "flex", alignItems: "center", gap: "10px", minWidth: 0 }}>
        <ResultPhoto
          resultId={item.id}
          imageUrl={item.thumbnail_url}
          size="medium"
          alt={item.layer_name}
        />

        {/* Informasi Utama */}
        <div style={{ minWidth: 0 }}>
          <div
            style={{
              fontSize: "13px",
              fontWeight: 600,
              whiteSpace: "nowrap",
              overflow: "hidden",
              textOverflow: "ellipsis",
              color: isCorrupted ? "#db3737" : "var(--color-ink, #182026)",
              maxWidth: "180px",
            }}
            title={item.layer_name}
          >
            {item.layer_name || item.id}
          </div>

          <div style={{ display: "flex", gap: "4px", marginTop: "3px", alignItems: "center" }}>
            {isCorrupted ? (
              <Tag intent="danger" minimal style={{ fontSize: "10px" }}>
                Metadata rusak
              </Tag>
            ) : (
              <>
                <Tag intent={getGeomIntent(item.geom_type)} minimal style={{ fontSize: "10px" }}>
                  {item.geom_type === "Point"
                    ? "Titik"
                    : item.geom_type === "Polygon"
                      ? "Poligon"
                      : item.geom_type === "Line" || item.geom_type === "LineString"
                        ? "Garis"
                        : item.geom_type === "Mixed"
                          ? "Campuran"
                          : item.geom_type === "ImageOverlay"
                            ? "Overlay"
                            : item.geom_type}
                </Tag>
                <Tag minimal style={{ fontSize: "10px" }}>
                  {item.feature_count} fitur
                </Tag>
                {item.has_manual_edits && (
                  <Tag
                    intent="warning"
                    minimal
                    style={{ fontSize: "10px" }}
                    title="Memiliki atribut kustom atau nilai hasil edit manual"
                  >
                    Diedit
                  </Tag>
                )}
              </>
            )}
          </div>
        </div>
      </div>

      {/* Kanan: Tombol Aksi */}
      <div style={{ display: "flex", gap: "2px", alignItems: "center", flexShrink: 0 }}>
        <Tooltip content="Fokus ke Geometri di Peta">
          <Button
            icon="locate"
            minimal
            small
            disabled={isCorrupted || isProcessing}
            onClick={() => onFocus(item)}
          />
        </Tooltip>

        <Tooltip content="Edit Nama & Tampilan">
          <Button
            icon="edit"
            minimal
            small
            disabled={isCorrupted || isProcessing}
            onClick={() => onEdit(item)}
          />
        </Tooltip>

        <Tooltip content="Hapus Layer Hasil Analisis">
          <Button
            icon="trash"
            minimal
            small
            intent="danger"
            disabled={isProcessing}
            onClick={() => onDelete(item)}
          />
        </Tooltip>
      </div>
    </div>
  );
};
