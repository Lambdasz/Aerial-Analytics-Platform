import React from "react";
import { Dialog, Classes, Button, Icon } from "@blueprintjs/core";
import type { AoiFeature, Polygon } from "../types/aoi";
import {
  calculateGeodesicAreaMeters,
  calculateLineLengthMeters,
  formatArea,
  formatPerimeter,
} from "../lib/geoJson";

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
  // Hitung metrik geometri
  let formattedArea = "-";
  let formattedPerim = "-";
  let hasGeometry = false;

  if (draftPolygon && draftPolygon[0] && draftPolygon[0].length >= 3) {
    const area = calculateGeodesicAreaMeters(draftPolygon[0]);
    const perim = calculateLineLengthMeters(draftPolygon[0]);
    formattedArea = formatArea(area);
    formattedPerim = formatPerimeter(perim);
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
      style={{
        width: "480px",
        maxWidth: "92vw",
        borderRadius: "16px",
        overflow: "hidden",
      }}
    >
      <div className={Classes.DIALOG_BODY} style={{ padding: "24px 24px 16px 24px" }}>
        {/* Judul Dialog */}
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

        {/* Input Nama Area */}
        <div style={{ marginBottom: "14px" }}>
          <div
            style={{
              fontSize: "12px",
              fontWeight: 600,
              color: "#64748b",
              marginBottom: "6px",
            }}
          >
            Nama Area <span style={{ color: "#ef4444" }}>*</span>
          </div>
          <input
            value={aoiName}
            onChange={(e) => setAoiName(e.target.value)}
            placeholder="Contoh: Lahan Blok A"
            disabled={isSaving}
            style={{
              width: "100%",
              padding: "9px 12px",
              fontSize: "13px",
              borderRadius: "10px",
              border: "none",
              background: "#f8fafc",
              color: "#0f172a",
              outline: "none",
              boxSizing: "border-box",
            }}
          />
        </div>

        {/* Input Tipe Area */}
        <div style={{ marginBottom: "16px" }}>
          <div
            style={{
              fontSize: "12px",
              fontWeight: 600,
              color: "#64748b",
              marginBottom: "6px",
            }}
          >
            Tipe Area <span style={{ fontWeight: 400, color: "#94a3b8" }}>(opsional)</span>
          </div>
          <input
            value={aoiType}
            onChange={(e) => setAoiType(e.target.value)}
            placeholder="Contoh: Kebun / Sektor / Analisis"
            disabled={isSaving}
            style={{
              width: "100%",
              padding: "9px 12px",
              fontSize: "13px",
              borderRadius: "10px",
              border: "none",
              background: "#f8fafc",
              color: "#0f172a",
              outline: "none",
              boxSizing: "border-box",
            }}
          />
        </div>

        {/* Seksi Status & Interaksi Geometri Poligon */}
        <div style={{ marginBottom: "14px" }}>
          <div
            style={{
              fontSize: "12px",
              fontWeight: 600,
              color: "#64748b",
              marginBottom: "6px",
            }}
          >
            Batas Geometri Poligon
          </div>

          {!isEditing ? (
            /* Mode Buat */
            hasGeometry ? (
              <div
                style={{
                  background: "#f0fdf4",
                  border: "1px solid #bbf7d0",
                  borderRadius: "12px",
                  padding: "12px 14px",
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "space-between",
                  gap: "10px",
                }}
              >
                <div>
                  <div
                    style={{
                      fontSize: "12px",
                      fontWeight: 600,
                      color: "#166534",
                      display: "flex",
                      alignItems: "center",
                      gap: "6px",
                    }}
                  >
                    <Icon icon="tick-circle" size={14} color="#166534" />
                    <span>Poligon Siap Disimpan</span>
                  </div>
                  <div style={{ fontSize: "11px", color: "#15803d", marginTop: "2px" }}>
                    Luas: <strong>{formattedArea}</strong> • Keliling:{" "}
                    <strong>{formattedPerim}</strong>
                  </div>
                </div>
                <Button
                  minimal
                  small
                  icon="refresh"
                  text="Gambar Ulang"
                  onClick={onStartDrawOnMap}
                  style={{
                    background: "#ffffff",
                    color: "#166534",
                    borderRadius: "8px",
                    fontWeight: 600,
                    fontSize: "11px",
                    padding: "4px 10px",
                    outline: "none",
                    boxShadow: "0 1px 3px rgba(0,0,0,0.06)",
                  }}
                />
              </div>
            ) : (
              <div
                style={{
                  background: "#f8fafc",
                  border: "1px dashed #cbd5e1",
                  borderRadius: "12px",
                  padding: "16px 14px",
                  textAlign: "center",
                }}
              >
                <div style={{ fontSize: "12px", color: "#64748b", marginBottom: "10px" }}>
                  Batas poligon area belum ditentukan di atas peta.
                </div>
                <Button
                  minimal
                  icon="draw"
                  text="Gambar Poligon di Peta"
                  onClick={onStartDrawOnMap}
                  style={{
                    background: "#0f172a",
                    color: "#ffffff",
                    borderRadius: "8px",
                    fontWeight: 600,
                    fontSize: "12px",
                    padding: "6px 16px",
                    outline: "none",
                  }}
                />
              </div>
            )
          ) : (
            /* Mode Edit */
            <div
              style={{
                background: "#f8fafc",
                border: "1px solid #f1f5f9",
                borderRadius: "12px",
                padding: "12px 14px",
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
                gap: "10px",
              }}
            >
              <div>
                <div style={{ fontSize: "12px", fontWeight: 600, color: "#334155" }}>
                  Batas Geometri Terpasang
                </div>
                <div style={{ fontSize: "11px", color: "#64748b", marginTop: "2px" }}>
                  Luas: <strong>{formattedArea}</strong> • Keliling:{" "}
                  <strong>{formattedPerim}</strong>
                </div>
              </div>
              <Button
                minimal
                small
                icon="polygon-filter"
                text="Ubah Bentuk di Peta"
                onClick={onStartModifyPolygon}
                style={{
                  background: "#0f172a",
                  color: "#ffffff",
                  borderRadius: "8px",
                  fontWeight: 600,
                  fontSize: "11px",
                  padding: "6px 12px",
                  outline: "none",
                }}
              />
            </div>
          )}
        </div>

        {/* Notifikasi Error */}
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

      {/* Footer Aksi Dialog */}
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
              outline: "none",
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
              outline: "none",
            }}
          />
        </div>
      </div>
    </Dialog>
  );
};
