import React from "react";
import * as L from "leaflet";
import { Button } from "@blueprintjs/core";
import { useSavedAois } from "../hooks/useSavedAois";
import { useAoiModalWorkflow } from "../hooks/useAoiModalWorkflow";
import { AoiInteractiveBanner } from "../molecules/aoi/AoiInteractiveBanner";
import { AoiListSection } from "./AoiListSection";
import { AoiModal } from "./dialogs/AoiModal";

export interface AoiToolbarProps {
  map: L.Map | null;
}

/**
 * Toolbar header dan orkestrator pembuatan / modifikasi Area of Interest (AOI).
 * Mendelegasikan logika form modal ke useAoiModalWorkflow, banner interaktif ke
 * AoiInteractiveBanner, dan rendering daftar kartu ke AoiListSection.
 */
export const AoiToolbar: React.FC<AoiToolbarProps> = ({ map }) => {
  const { savedAois, isLoadingAois } = useSavedAois();
  const {
    isDrawing,
    isModifyingPolygon,
    isModalOpen,
    editingAoi,
    handleOpenCreateModal,
    handleFocusAoi,
    handleStartEdit,
    handleDeleteAoi,
    handleCancelDrawing,
    handleCancelModifyPolygon,
    handleFinishModifyPolygon,
    modalProps,
  } = useAoiModalWorkflow({ map });

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "16px" }}>
      {/* Unified Page Header */}
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          paddingBottom: "8px",
          borderBottom: "1px solid rgba(16, 22, 26, 0.08)",
        }}
      >
        <span style={{ fontSize: "14px", fontWeight: 700, color: "#0f172a" }}>
          Area of Interest (AOI)
        </span>
        <span style={{ fontSize: "11px", color: "#64748b", fontWeight: 500 }}>
          {savedAois.length} Tersimpan
        </span>
      </div>

      {/* Tombol Utama Buat Batas Area (Membuka Modal) */}
      <Button
        minimal
        icon="add"
        text="Buat Batas Area (AOI)"
        onClick={handleOpenCreateModal}
        style={{
          width: "100%",
          padding: "10px 16px",
          fontSize: "13px",
          fontWeight: 600,
          background: "#0f172a",
          color: "#ffffff",
          borderRadius: "12px",
          boxShadow: "0 2px 8px rgba(15, 23, 42, 0.12)",
          outline: "none",
        }}
      />

      {/* Banner Pemandu Interaktif saat Menggambar Baru atau Menggeser Verteks Poligon */}
      <AoiInteractiveBanner
        isDrawing={isDrawing}
        isModifyingPolygon={isModifyingPolygon}
        onCancelDrawing={handleCancelDrawing}
        onCancelModifyPolygon={handleCancelModifyPolygon}
        onFinishModifyPolygon={handleFinishModifyPolygon}
      />

      {/* Sub-Seksi Daftar AOI */}
      <AoiListSection
        savedAois={savedAois}
        isLoadingAois={isLoadingAois}
        isModalOpen={isModalOpen}
        editingAoi={editingAoi}
        onFocusAoi={handleFocusAoi}
        onEditAoi={handleStartEdit}
        onDeleteAoi={(item) => void handleDeleteAoi(item)}
      />

      {/* Modal Dialog Buat & Edit AOI */}
      <AoiModal {...modalProps} />
    </div>
  );
};

export const AoiDemoToolbar = AoiToolbar;
