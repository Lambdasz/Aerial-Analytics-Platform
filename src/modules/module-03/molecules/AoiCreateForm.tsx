import React from "react";
import { Button, Callout, Card, Icon, Spinner, Tag } from "@blueprintjs/core";
import type { AoiFeature } from "../types/aoi";
import { formatArea } from "../lib/geoJson";

export interface AoiCreateFormProps {
  aoiId: string;
  setAoiId: (id: string) => void;
  aoiName: string;
  setAoiName: (name: string) => void;
  aoiType: string;
  setAoiType: (type: string) => void;
  editingAoiId: string | null;
  isDrawing: boolean;
  isSaving: boolean;
  isSaveDisabled: boolean;
  isCancelDisabled: boolean;
  error: string | null;
  lastSavedAoi: AoiFeature | null;
  onDraw: () => void;
  onCancel: () => void;
  onSave: () => void;
}

export const AoiCreateForm: React.FC<AoiCreateFormProps> = ({
  aoiId,
  setAoiId,
  aoiName,
  setAoiName,
  aoiType,
  setAoiType,
  editingAoiId,
  isDrawing,
  isSaving,
  isSaveDisabled,
  isCancelDisabled,
  error,
  lastSavedAoi,
  onDraw,
  onCancel,
  onSave,
}) => {
  const isInputDisabled = isSaving || isDrawing;

  return (
    <Card
      elevation={0}
      style={{
        padding: "16px",
        background: "var(--color-gallery-white)",
        boxShadow: "var(--shadow-subtle)",
        borderRadius: "14px",
        border: editingAoiId ? "2px solid #ff9500" : "1px solid #e2e8f0",
        color: "var(--color-ink)",
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          marginBottom: "12px",
        }}
      >
        <span
          style={{
            fontWeight: 600,
            fontSize: "14px",
            display: "flex",
            alignItems: "center",
            gap: 6,
          }}
        >
          <Icon
            icon={editingAoiId ? "edit" : "polygon-filter"}
            size={16}
            color={editingAoiId ? "#ff9500" : "var(--color-apple-blue)"}
          />
          {editingAoiId ? "Edit Bentuk AOI di Peta" : "Buat Batas Area (AOI)"}
        </span>
        {editingAoiId && (
          <Tag minimal intent="warning" round>
            Mode Edit
          </Tag>
        )}
      </div>

      {editingAoiId && (
        <Callout intent="warning" icon="info-sign" style={{ marginBottom: 12, fontSize: "12px" }}>
          Anda sedang mengedit poligon di peta. Geser titik-titik sudut pada peta, lalu klik{" "}
          <strong>Simpan Perubahan</strong>.
        </Callout>
      )}

      <div style={{ marginBottom: 12 }}>
        <div
          style={{
            fontSize: "12px",
            fontWeight: 600,
            color: "var(--color-slate)",
            marginBottom: "6px",
          }}
        >
          AOI ID <span style={{ fontWeight: 400 }}>(wajib)</span>
        </div>
        <div
          style={{
            display: "flex",
            alignItems: "center",
            background: editingAoiId ? "#f1f5f9" : "var(--color-control-gray)",
            borderRadius: "10px",
            padding: "8px 12px",
            opacity: isInputDisabled ? 0.6 : 1,
          }}
        >
          <input
            value={aoiId}
            onChange={(e) => setAoiId(e.target.value)}
            placeholder="aoi-001"
            disabled={isInputDisabled || editingAoiId !== null}
            style={{
              border: "none",
              outline: "none",
              background: "transparent",
              color: "var(--color-ink)",
              fontSize: "13px",
              width: "100%",
              fontFamily: "monospace",
            }}
          />
        </div>
      </div>

      <div style={{ marginBottom: 12 }}>
        <div
          style={{
            fontSize: "12px",
            fontWeight: 600,
            color: "var(--color-slate)",
            marginBottom: "6px",
          }}
        >
          Nama Area <span style={{ fontWeight: 400 }}>(wajib)</span>
        </div>
        <div
          style={{
            display: "flex",
            alignItems: "center",
            background: "var(--color-control-gray)",
            borderRadius: "10px",
            padding: "8px 12px",
            opacity: isInputDisabled ? 0.6 : 1,
          }}
        >
          <input
            value={aoiName}
            onChange={(e) => setAoiName(e.target.value)}
            placeholder="Lahan Sawit Blok A"
            disabled={isInputDisabled}
            style={{
              border: "none",
              outline: "none",
              background: "transparent",
              color: "var(--color-ink)",
              fontSize: "13px",
              width: "100%",
            }}
          />
        </div>
      </div>

      <div style={{ marginBottom: 16 }}>
        <div
          style={{
            fontSize: "12px",
            fontWeight: 600,
            color: "var(--color-slate)",
            marginBottom: "6px",
          }}
        >
          Tipe Area <span style={{ fontWeight: 400 }}>(opsional)</span>
        </div>
        <div
          style={{
            display: "flex",
            alignItems: "center",
            background: "var(--color-control-gray)",
            borderRadius: "10px",
            padding: "8px 12px",
            opacity: isInputDisabled ? 0.6 : 1,
          }}
        >
          <input
            value={aoiType}
            onChange={(e) => setAoiType(e.target.value)}
            placeholder="Area Analisis / Kebun / Sektor"
            disabled={isInputDisabled}
            style={{
              border: "none",
              outline: "none",
              background: "transparent",
              color: "var(--color-ink)",
              fontSize: "13px",
              width: "100%",
            }}
          />
        </div>
      </div>

      <div style={{ display: "flex", gap: "8px", paddingTop: "4px" }}>
        {!editingAoiId && (
          <Button
            icon="edit"
            text={isDrawing ? "Menggambar..." : "Draw AOI"}
            onClick={onDraw}
            disabled={isDrawing || isSaving}
            style={{
              borderRadius: "999px",
              background: "var(--color-apple-blue)",
              color: "#fff",
              boxShadow: "none",
              fontWeight: 600,
              padding: "6px 16px",
              flex: 1,
            }}
          />
        )}
        <Button
          icon="cross"
          text="Batal"
          onClick={onCancel}
          disabled={isCancelDisabled}
          style={{
            borderRadius: "999px",
            background: "var(--color-control-gray)",
            color: "var(--color-ink)",
            boxShadow: "none",
            fontWeight: 500,
            padding: "6px 16px",
            flex: editingAoiId ? 1 : undefined,
          }}
        />
      </div>

      <div style={{ marginTop: "10px" }}>
        <Button
          icon="floppy-disk"
          text={editingAoiId ? "Simpan Perubahan " : "Simpan "}
          fill
          onClick={onSave}
          disabled={isSaveDisabled}
          style={{
            borderRadius: "999px",
            background: isSaveDisabled ? "var(--color-control-gray)" : "#34c759",
            color: isSaveDisabled ? "var(--color-slate)" : "#fff",
            boxShadow: "none",
            fontWeight: 600,
            padding: "8px 16px",
          }}
        />
      </div>

      {isSaving && (
        <div style={{ display: "flex", alignItems: "center", gap: 8, marginTop: 12 }}>
          <Spinner size={16} />
          <span style={{ fontSize: "12px", color: "var(--color-slate)" }}>
            Menyimpan data AOI ke /map_storage...
          </span>
        </div>
      )}

      {error && (
        <Callout intent="danger" style={{ marginTop: 12, fontSize: "12px" }}>
          {error}
        </Callout>
      )}

      {lastSavedAoi && !editingAoiId && (
        <Card
          style={{
            marginTop: 12,
            borderRadius: "10px",
            border: "1px solid #bbf7d0",
            background: "#f0fdf4",
          }}
        >
          <div
            style={{
              color: "#166534",
              fontWeight: 600,
              fontSize: "13px",
              display: "flex",
              alignItems: "center",
              gap: 6,
            }}
          >
            <Icon icon="tick-circle" color="#166534" size={14} />
            AOI Berhasil Disimpan
          </div>
          <div style={{ fontSize: "12px", marginTop: 4, color: "#1e293b" }}>
            <strong>{lastSavedAoi.properties.name}</strong> (
            {formatArea(lastSavedAoi.properties.calculated_area_sq_m)})
          </div>
        </Card>
      )}
    </Card>
  );
};
