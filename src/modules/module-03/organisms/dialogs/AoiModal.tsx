import React from "react";
import { Dialog, Classes, Button, Icon } from "@blueprintjs/core";
import type { AoiFeature, Polygon } from "../../types/aoi";
import {
  calculateGeodesicAreaMeters,
  calculateLineLengthMeters,
  formatArea,
  formatPerimeter,
} from "../../lib/geoJson";
import { AoiTypeSelector } from "../../molecules/aoi/AoiTypeSelector";
import { AoiGeometryMetricsCard } from "../../molecules/aoi/AoiGeometryMetricsCard";

export interface AoiModalProps {
  isOpen: boolean;
  isEditing: boolean;
  aoiId: string;
  aoiName: string;
  setAoiName: (name: string) => void;
  aoiType: string;
  setAoiType: (type: string) => void;
  draftPolygon: Polygon | null;
  editingAoi: AoiFeature | null;
  isSaving: boolean;
  error: string | null;
  onClose: () => void;
  onStartDrawOnMap: () => void;
  onStartModifyPolygon: () => void;
  onSave: () => void;
}

export const AoiModal: React.FC<AoiModalProps> = ({
  isOpen,
  isEditing,
  aoiId,
  aoiName,
  setAoiName,
  aoiType,
  setAoiType,
  draftPolygon,
  editingAoi,
  isSaving,
  error,
  onClose,
  onStartDrawOnMap,
  onStartModifyPolygon,
  onSave,
}) => {
  let formattedArea = "-";
  let formattedPerim = "-";
  let hasGeometry = false;

  if (draftPolygon && draftPolygon[0] && draftPolygon[0].length >= 3) {
    formattedArea = formatArea(calculateGeodesicAreaMeters(draftPolygon[0]));
    formattedPerim = formatPerimeter(calculateLineLengthMeters(draftPolygon[0]));
    hasGeometry = true;
  } else if (editingAoi) {
    formattedArea = formatArea(editingAoi.properties.calculated_area_sq_m);
    formattedPerim = formatPerimeter(editingAoi.properties.perimeter_m);
    hasGeometry = true;
  }

  const isSaveDisabled = isSaving || !aoiName.trim() || (!isEditing && !hasGeometry);

  return (
    <Dialog
      isOpen={isOpen}
      onClose={isSaving ? undefined : onClose}
      canEscapeKeyClose={!isSaving}
      canOutsideClickClose={!isSaving}
      style={{ width: "480px", maxWidth: "92vw", borderRadius: "16px", overflow: "hidden" }}
    >
      <div className={Classes.DIALOG_BODY} style={{ padding: "24px 24px 16px 24px" }}>
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            marginBottom: "16px",
          }}
        >
          <div style={{ fontSize: "16px", fontWeight: 700, color: "#0f172a" }}>
            {isEditing ? "Edit Batas Area (AOI)" : "Buat Batas Area (AOI)"}
          </div>
          <span style={{ fontSize: "11px", color: "#94a3b8", fontFamily: "monospace" }}>
            ID: {aoiId}
          </span>
        </div>

        <AoiTypeSelector
          aoiName={aoiName}
          setAoiName={setAoiName}
          aoiType={aoiType}
          setAoiType={setAoiType}
          disabled={isSaving}
        />
        <AoiGeometryMetricsCard
          isEditing={isEditing}
          hasGeometry={hasGeometry}
          formattedArea={formattedArea}
          formattedPerim={formattedPerim}
          onStartDrawOnMap={onStartDrawOnMap}
          onStartModifyPolygon={onStartModifyPolygon}
          disabled={isSaving}
        />

        {error && (
          <div
            style={{
              background: "#fef2f2",
              border: "1px solid #fecaca",
              borderRadius: "10px",
              padding: "9px 12px",
              marginBottom: "14px",
              fontSize: "12px",
              color: "#991b1b",
              display: "flex",
              alignItems: "center",
              gap: "8px",
            }}
          >
            <Icon icon="error" size={14} color="#991b1b" />
            <span>{error}</span>
          </div>
        )}
      </div>

      <div className={Classes.DIALOG_FOOTER} style={{ padding: "0 24px 20px 24px" }}>
        <div className={Classes.DIALOG_FOOTER_ACTIONS} style={{ gap: "8px" }}>
          <Button
            minimal
            text="Batal"
            disabled={isSaving}
            onClick={onClose}
            style={{
              background: "#f1f5f9",
              color: "#475569",
              borderRadius: "999px",
              fontWeight: 600,
              padding: "8px 18px",
            }}
          />
          <Button
            minimal
            text={isSaving ? "Menyimpan..." : isEditing ? "Simpan Perubahan" : "Simpan AOI"}
            loading={isSaving}
            disabled={isSaveDisabled}
            onClick={onSave}
            style={{
              background: isSaveDisabled ? "#f1f5f9" : "#0f172a",
              color: isSaveDisabled ? "#94a3b8" : "#ffffff",
              borderRadius: "999px",
              fontWeight: 600,
              padding: "8px 20px",
              boxShadow: isSaveDisabled ? "none" : "0 2px 8px rgba(15, 23, 42, 0.15)",
            }}
          />
        </div>
      </div>
    </Dialog>
  );
};
