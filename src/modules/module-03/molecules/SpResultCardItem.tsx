import React from "react";
import { Button, Tooltip } from "@blueprintjs/core";
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

  return (
    <div
      className={`sp-result-card-item ${isSelected ? "selected" : ""}`}
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        padding: "14px 16px",
        marginBottom: "12px",
        borderRadius: "16px",
        backgroundColor: "#ffffff",
        border: "none",
        boxShadow: isSelected
          ? "0 8px 24px -4px rgba(0, 0, 0, 0.1), 0 2px 6px rgba(0, 0, 0, 0.04)"
          : "0 2px 12px -2px rgba(0, 0, 0, 0.05), 0 1px 3px rgba(0, 0, 0, 0.02)",
        transition: "all 0.2s cubic-bezier(0.4, 0, 0.2, 1)",
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
              fontSize: "14px",
              fontWeight: 600,
              whiteSpace: "nowrap",
              overflow: "hidden",
              textOverflow: "ellipsis",
              color: isCorrupted ? "#ef4444" : "#0f172a",
              maxWidth: "180px",
              marginBottom: "3px",
            }}
            title={item.layer_name}
          >
            {item.layer_name || item.id}
          </div>

          <div style={{ display: "flex", gap: "6px", alignItems: "center" }}>
            {isCorrupted ? (
              <span style={{ fontSize: "11px", color: "#ef4444", fontWeight: 500 }}>
                Metadata rusak
              </span>
            ) : (
              <>
                <span
                  style={{
                    fontSize: "11px",
                    fontWeight: 500,
                    color: "#64748b",
                  }}
                >
                  {item.geom_type === "Point"
                    ? "Titik"
                    : item.geom_type === "Polygon"
                      ? "Poligon"
                      : item.geom_type === "Line" || item.geom_type === "LineString"
                        ? "Garis"
                        : item.geom_type}
                </span>
                <span style={{ fontSize: "11px", color: "#cbd5e1" }}>•</span>
                <span style={{ fontSize: "11px", color: "#94a3b8" }}>
                  {item.feature_count} fitur
                </span>
                {item.has_manual_edits && (
                  <span
                    style={{
                      fontSize: "10px",
                      fontWeight: 600,
                      color: "#f59e0b",
                      background: "rgba(245, 158, 11, 0.1)",
                      padding: "1px 5px",
                      borderRadius: "4px",
                    }}
                    title="Memiliki atribut kustom atau nilai hasil edit manual"
                  >
                    Diedit
                  </span>
                )}
              </>
            )}
          </div>
        </div>
      </div>

      {/* Kanan: Tombol Aksi */}
      <div style={{ display: "flex", gap: "4px", alignItems: "center", flexShrink: 0 }}>
        <Tooltip content="Fokus ke Geometri di Peta">
          <Button
            icon="locate"
            minimal
            small
            disabled={isCorrupted || isProcessing}
            style={{ borderRadius: "6px" }}
            onClick={() => onFocus(item)}
          />
        </Tooltip>

        <Tooltip content="Edit Nama & Tampilan">
          <Button
            icon="edit"
            minimal
            small
            disabled={isCorrupted || isProcessing}
            style={{ borderRadius: "6px" }}
            onClick={() => onEdit(item)}
          />
        </Tooltip>

        <Tooltip content="Hapus Layer Hasil Analisis">
          <Button
            icon="trash"
            minimal
            small
            disabled={isProcessing}
            style={{
              borderRadius: "6px",
              color: "var(--color-slate, #5c7080)",
            }}
            onClick={() => onDelete(item)}
          />
        </Tooltip>
      </div>
    </div>
  );
};
