import React from "react";
import { Button } from "@blueprintjs/core";
import { AnnotationShapeSelector } from "./annotation/AnnotationShapeSelector";
import {
  AnnotationCoordModeSelector,
  type CoordMode,
} from "./annotation/AnnotationCoordModeSelector";
import { AnnotationManualCoordInputs } from "./annotation/AnnotationManualCoordInputs";
import { AnnotationMapPickControl } from "./annotation/AnnotationMapPickControl";
import type { AnnotationGeomType } from "../types/layer";

export type { AnnotationGeomType };

export interface AnnotationFormProps {
  annotationText: string;
  geomType: AnnotationGeomType;
  latitude: string;
  longitude: string;
  manualCoordsText: string;
  coordMode: CoordMode;
  isPickingLocation: boolean;
  hasGeometry: boolean;
  geometrySummary?: string;
  isEditing?: boolean;
  isSubmitting?: boolean;
  error?: string | null;
  onTextChange: (text: string) => void;
  onGeomTypeChange: (type: AnnotationGeomType) => void;
  onLatitudeChange: (lat: string) => void;
  onLongitudeChange: (lng: string) => void;
  onManualCoordsChange: (coords: string) => void;
  onCoordModeChange: (mode: CoordMode) => void;
  onTogglePickLocation: () => void;
  onClearGeometry: () => void;
  onSubmit: () => void;
  onCancel?: () => void;
}

/**
 * Orkestrator formulir pembuatan dan penyuntingan anotasi spasial.
 * Mengintegrasikan selektor bentuk, mode koordinat, kontrol picker peta, dan input manual.
 */
export const AnnotationForm: React.FC<AnnotationFormProps> = ({
  annotationText,
  geomType,
  latitude,
  longitude,
  manualCoordsText,
  coordMode,
  isPickingLocation,
  hasGeometry,
  geometrySummary,
  isEditing = false,
  isSubmitting = false,
  error,
  onTextChange,
  onGeomTypeChange,
  onLatitudeChange,
  onLongitudeChange,
  onManualCoordsChange,
  onCoordModeChange,
  onTogglePickLocation,
  onClearGeometry,
  onSubmit,
  onCancel,
}) => {
  const isSubmitDisabled = isSubmitting || !annotationText.trim() || !hasGeometry;

  return (
    <div>
      <div style={{ display: "flex", alignItems: "center", marginBottom: "14px" }}>
        <span style={{ fontWeight: 700, fontSize: "14px", color: "#0f172a" }}>
          {isEditing ? "Edit Anotasi Spasial" : "Buat Anotasi Spasial"}
        </span>
      </div>

      <div style={{ marginBottom: "14px" }}>
        <div style={{ fontSize: "12px", fontWeight: 600, color: "#64748b", marginBottom: "6px" }}>
          Catatan / Keterangan Lokasi <span style={{ color: "#ef4444" }}>*</span>
        </div>
        <div style={{ background: "#f8fafc", borderRadius: "10px", padding: "10px 12px" }}>
          <textarea
            placeholder="Tulis catatan (misal: Patok batas lahan, jalur kabel, area genangan air, dll)..."
            value={annotationText}
            onChange={(e) => onTextChange(e.target.value)}
            disabled={isSubmitting}
            style={{
              width: "100%",
              minHeight: "56px",
              border: "none",
              outline: "none",
              background: "transparent",
              color: "#0f172a",
              fontSize: "13px",
              fontFamily: "var(--font-sf-pro-text)",
              resize: "vertical",
            }}
          />
        </div>
      </div>

      <AnnotationShapeSelector
        geomType={geomType}
        onChange={onGeomTypeChange}
        disabled={isSubmitting || isPickingLocation}
      />
      <AnnotationCoordModeSelector
        coordMode={coordMode}
        onChange={onCoordModeChange}
        disabled={isSubmitting || isPickingLocation}
      />

      {coordMode === "map" ? (
        <AnnotationMapPickControl
          geomType={geomType}
          isPickingLocation={isPickingLocation}
          hasGeometry={hasGeometry}
          latitude={latitude}
          longitude={longitude}
          geometrySummary={geometrySummary}
          disabled={isSubmitting}
          onTogglePickLocation={onTogglePickLocation}
          onClearGeometry={onClearGeometry}
        />
      ) : (
        <AnnotationManualCoordInputs
          geomType={geomType}
          latitude={latitude}
          longitude={longitude}
          manualCoordsText={manualCoordsText}
          hasGeometry={hasGeometry}
          geometrySummary={geometrySummary}
          disabled={isSubmitting}
          onLatitudeChange={onLatitudeChange}
          onLongitudeChange={onLongitudeChange}
          onManualCoordsChange={onManualCoordsChange}
        />
      )}

      {error && (
        <div
          style={{
            marginBottom: "12px",
            background: "#fef2f2",
            color: "#dc2626",
            borderRadius: "10px",
            padding: "10px 12px",
            fontSize: "12px",
            fontWeight: 500,
          }}
        >
          {error}
        </div>
      )}

      <div style={{ display: "flex", gap: "8px", marginTop: "14px" }}>
        {onCancel && (
          <Button
            minimal
            text="Batal"
            onClick={onCancel}
            disabled={isSubmitting}
            style={{
              borderRadius: "10px",
              background: "#f1f5f9",
              color: "#475569",
              fontWeight: 600,
              padding: "8px 14px",
              fontSize: "12px",
            }}
          />
        )}
        <Button
          minimal
          icon={isEditing ? "floppy-disk" : "add"}
          text={
            isSubmitting
              ? isEditing
                ? "Menyimpan Perubahan..."
                : "Menyimpan Anotasi..."
              : isEditing
                ? "Simpan Perubahan"
                : "Simpan Anotasi"
          }
          onClick={onSubmit}
          loading={isSubmitting}
          disabled={isSubmitDisabled}
          style={{
            flex: 1,
            borderRadius: "10px",
            background: isSubmitDisabled ? "#f1f5f9" : "#0f172a",
            color: isSubmitDisabled ? "#94a3b8" : "#ffffff",
            fontWeight: 600,
            padding: "8px 14px",
            fontSize: "12px",
          }}
        />
      </div>
    </div>
  );
};
