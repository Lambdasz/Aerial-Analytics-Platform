import React from "react";
import { Button, Tag, Tooltip } from "@blueprintjs/core";
import type { AoiFeature } from "../types/aoi";
import { formatArea, formatPerimeter } from "../lib/geoJson";

export interface AoiCardItemProps {
  item: AoiFeature;
  isInline: boolean;
  isEditingThisPolygon: boolean;
  inlineName: string;
  setInlineName: (name: string) => void;
  inlineType: string;
  setInlineType: (type: string) => void;
  isSavingInline: boolean;
  isSaving: boolean;
  isSaveDisabled: boolean;
  onSaveInlineEdit: (item: AoiFeature) => void;
  onCancelInlineEdit: () => void;
  onFocus: (item: AoiFeature) => void;
  onStartInlineEdit: (item: AoiFeature) => void;
  onStartPolygonEdit: (item: AoiFeature) => void;
  onDelete: (item: AoiFeature) => void;
  onCancelDrawing: () => void;
  onSaveDrawing: () => void;
}

export const AoiCardItem: React.FC<AoiCardItemProps> = ({
  item,
  isInline,
  isEditingThisPolygon,
  inlineName,
  setInlineName,
  inlineType,
  setInlineType,
  isSavingInline,
  isSaving,
  isSaveDisabled,
  onSaveInlineEdit,
  onCancelInlineEdit,
  onFocus,
  onStartInlineEdit,
  onStartPolygonEdit,
  onDelete,
  onCancelDrawing,
  onSaveDrawing,
}) => {
  return (
    <div
      style={{
        background: isEditingThisPolygon ? "#fffbeb" : "#f8fafc",
        borderRadius: "12px",
        padding: "12px",
        border: isEditingThisPolygon ? "1.5px solid #f59e0b" : "1px solid #e2e8f0",
        transition: "all 0.2s ease",
      }}
    >
      {/* Mode Inline Edit Nama & Tipe */}
      {isInline ? (
        <div>
          <div
            style={{
              fontSize: "12px",
              fontWeight: 600,
              color: "var(--color-slate)",
              marginBottom: 4,
            }}
          >
            Edit Nama:
          </div>
          <input
            value={inlineName}
            onChange={(e) => setInlineName(e.target.value)}
            style={{
              width: "100%",
              padding: "6px 8px",
              fontSize: "13px",
              borderRadius: "8px",
              border: "1px solid #cbd5e1",
              marginBottom: 8,
              outline: "none",
            }}
          />
          <div
            style={{
              fontSize: "12px",
              fontWeight: 600,
              color: "var(--color-slate)",
              marginBottom: 4,
            }}
          >
            Edit Tipe:
          </div>
          <input
            value={inlineType}
            onChange={(e) => setInlineType(e.target.value)}
            style={{
              width: "100%",
              padding: "6px 8px",
              fontSize: "13px",
              borderRadius: "8px",
              border: "1px solid #cbd5e1",
              marginBottom: 10,
              outline: "none",
            }}
          />
          <div style={{ display: "flex", gap: "6px" }}>
            <Button
              small
              intent="primary"
              icon="tick"
              text="Simpan"
              loading={isSavingInline}
              onClick={() => onSaveInlineEdit(item)}
            />
            <Button small icon="cross" text="Batal" onClick={onCancelInlineEdit} />
          </div>
        </div>
      ) : (
        /* Tampilan Normal Item AOI */
        <div>
          <div
            style={{
              display: "flex",
              alignItems: "flex-start",
              justifyContent: "space-between",
              gap: 8,
            }}
          >
            <div>
              <div style={{ fontWeight: 600, fontSize: "13px", color: "var(--color-ink)" }}>
                {item.properties.name || "Tanpa Nama"}
              </div>
              <div
                style={{
                  fontSize: "11px",
                  color: "var(--color-slate)",
                  fontFamily: "monospace",
                }}
              >
                {item.properties.aoi_id}
              </div>
            </div>
            <Tag minimal round intent="primary" style={{ fontSize: "10px" }}>
              {item.properties.aoi_type || "Area Analisis"}
            </Tag>
          </div>

          {/* Informasi Metrik */}
          <div
            style={{
              display: "flex",
              gap: "12px",
              fontSize: "12px",
              color: "var(--color-slate)",
              marginTop: "8px",
              paddingTop: "6px",
              borderTop: "1px solid #e2e8f0",
            }}
          >
            <span>
              Luas: <strong>{formatArea(item.properties.calculated_area_sq_m)}</strong>
            </span>
            <span>
              Keliling: <strong>{formatPerimeter(item.properties.perimeter_m)}</strong>
            </span>
          </div>

          {/* Tombol Aksi */}
          {isEditingThisPolygon ? (
            <div
              style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
                gap: "8px",
                marginTop: "12px",
                paddingTop: "8px",
                borderTop: "1px dashed #f59e0b",
              }}
            >
              <span style={{ fontSize: "11px", color: "#b45309", fontWeight: 500 }}>
                Sedang diedit di peta...
              </span>
              <div style={{ display: "flex", gap: "6px" }}>
                <Button
                  small
                  icon="cross"
                  text="Batal"
                  onClick={onCancelDrawing}
                  disabled={isSaving}
                />
                <Button
                  small
                  intent="success"
                  icon="floppy-disk"
                  text="Simpan Perubahan"
                  loading={isSaving}
                  disabled={isSaveDisabled}
                  onClick={onSaveDrawing}
                />
              </div>
            </div>
          ) : (
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
                <Button minimal small icon="locate" text="Fokus" onClick={() => onFocus(item)} />
              </Tooltip>

              <Tooltip content="Ubah nama & tipe" placement="top">
                <Button
                  minimal
                  small
                  icon="edit"
                  text="Edit Info"
                  onClick={() => onStartInlineEdit(item)}
                />
              </Tooltip>

              <Tooltip content="Ubah titik sudut poligon di peta" placement="top">
                <Button
                  minimal
                  small
                  intent="warning"
                  icon="polygon-filter"
                  text="Ubah Bentuk"
                  onClick={() => onStartPolygonEdit(item)}
                />
              </Tooltip>

              <Tooltip content="Hapus AOI dari peta" placement="top">
                <Button minimal small intent="danger" icon="trash" onClick={() => onDelete(item)} />
              </Tooltip>
            </div>
          )}
        </div>
      )}
    </div>
  );
};
