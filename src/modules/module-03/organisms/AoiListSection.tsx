import React from "react";
import { Icon, Spinner } from "@blueprintjs/core";
import type { AoiFeature } from "../types/aoi";
import { AoiCardItem } from "../molecules/AoiCardItem";

export interface AoiListSectionProps {
  savedAois: AoiFeature[];
  isLoadingAois: boolean;
  isModalOpen: boolean;
  editingAoi: AoiFeature | null;
  onFocusAoi: (aoi: AoiFeature) => void;
  onEditAoi: (aoi: AoiFeature) => void;
  onDeleteAoi: (aoi: AoiFeature) => void;
}

/**
 * Komponen sub-seksi untuk menampilkan daftar Area of Interest (AOI) tersimpan,
 * indikator loading, kartu kosong (empty state), dan daftar kartu AOI interaktif.
 */
export const AoiListSection: React.FC<AoiListSectionProps> = ({
  savedAois,
  isLoadingAois,
  isModalOpen,
  editingAoi,
  onFocusAoi,
  onEditAoi,
  onDeleteAoi,
}) => {
  const aoiList = Array.isArray(savedAois) ? savedAois : [];

  return (
    <div>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          paddingBottom: "8px",
          borderBottom: "1px solid rgba(16, 22, 26, 0.08)",
          marginBottom: "12px",
        }}
      >
        <span style={{ fontSize: "13px", fontWeight: 600, color: "#334155" }}>
          Daftar Area Tersimpan
        </span>
        <span
          style={{
            color: "#64748b",
            fontSize: "11px",
            fontWeight: 500,
          }}
        >
          {aoiList.length} Area
        </span>
      </div>

      {isLoadingAois && (
        <div style={{ display: "flex", justifyContent: "center", padding: "16px" }}>
          <Spinner size={20} />
        </div>
      )}

      {!isLoadingAois && aoiList.length === 0 && (
        <div
          style={{
            textAlign: "center",
            padding: "24px 16px",
            color: "var(--color-slate, #64748b)",
            fontSize: "13px",
            background: "#ffffff",
            borderRadius: "16px",
            boxShadow: "0 2px 12px -2px rgba(0, 0, 0, 0.05)",
          }}
        >
          <Icon icon="draw" size={24} color="#94a3b8" style={{ marginBottom: 6 }} />
          <div style={{ fontWeight: 600, color: "#0f172a" }}>Belum ada AOI di peta</div>
          <div style={{ fontSize: "11px", marginTop: 4, color: "#94a3b8" }}>
            Gunakan tombol <strong>Buat Batas Area (AOI)</strong> di atas untuk membuat area baru.
          </div>
        </div>
      )}

      <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
        {aoiList.map((item) => (
          <AoiCardItem
            key={item.properties.aoi_id}
            item={item}
            isEditing={isModalOpen && editingAoi?.properties.aoi_id === item.properties.aoi_id}
            onFocus={onFocusAoi}
            onEdit={onEditAoi}
            onDelete={() => onDeleteAoi(item)}
          />
        ))}
      </div>
    </div>
  );
};
