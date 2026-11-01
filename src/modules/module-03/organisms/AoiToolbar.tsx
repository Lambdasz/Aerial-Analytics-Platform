import React, { useState, useEffect, useRef } from "react";
import * as L from "leaflet";
import { Button, Icon, Spinner } from "@blueprintjs/core";
import { useAOI } from "../hooks/useAOI";
import { getSavedAois, deleteSavedAoi, saveAoi as saveAoiToStorage } from "../services/api";
import type { AoiFeature } from "../types/aoi";
import { AoiCardItem } from "../molecules/AoiCardItem";
import { AoiModal } from "../molecules/AoiModal";

function generateAoiId(): string {
  return "aoi-" + Date.now();
}

export interface AoiToolbarProps {
  map: L.Map | null;
}

export const AoiToolbar: React.FC<AoiToolbarProps> = ({ map }) => {
  const {
    draftPolygon,
    isDrawing,
    isSaving,
    startDrawing,
    stopDrawing,
    clearAoi,
    loadAoiForEditing,
    saveAoi,
  } = useAOI();

  // State Modal Dialog
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [editingAoi, setEditingAoi] = useState<AoiFeature | null>(null);

  // State Form
  const [aoiId, setAoiId] = useState(generateAoiId);
  const [aoiName, setAoiName] = useState("Area Demo");
  const [aoiType, setAoiType] = useState("Area Analisis");
  const [isModifyingPolygon, setIsModifyingPolygon] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // State daftar AOI tersimpan
  const [savedAois, setSavedAois] = useState<AoiFeature[]>([]);
  const [isLoadingAois, setIsLoadingAois] = useState(true);

  // ── Muat daftar AOI & dengarkan perubahan ───────────────────────────
  useEffect(() => {
    let cancelled = false;

    getSavedAois()
      .then((list) => {
        if (!cancelled) setSavedAois(list);
      })
      .catch((err) => {
        console.warn("Gagal memuat saved AOIs:", err);
      })
      .finally(() => {
        if (!cancelled) setIsLoadingAois(false);
      });

    const handleAoiSaved = (event: Event) => {
      const customEvent = event as CustomEvent<AoiFeature>;
      if (customEvent.detail) {
        setSavedAois((prev) => {
          const id = customEvent.detail.properties.aoi_id;
          const exists = prev.some((a) => a.properties.aoi_id === id);
          if (exists) {
            return prev.map((a) => (a.properties.aoi_id === id ? customEvent.detail : a));
          }
          return [customEvent.detail, ...prev];
        });
      }
    };

    const handleAoiDeleted = (event: Event) => {
      const customEvent = event as CustomEvent<{ aoiId: string }>;
      if (customEvent.detail?.aoiId) {
        setSavedAois((prev) =>
          prev.filter((a) => a.properties.aoi_id !== customEvent.detail.aoiId),
        );
      }
    };

    window.addEventListener("aoi:saved", handleAoiSaved);
    window.addEventListener("aoi:deleted", handleAoiDeleted);

    return () => {
      cancelled = true;
      window.removeEventListener("aoi:saved", handleAoiSaved);
      window.removeEventListener("aoi:deleted", handleAoiDeleted);
    };
  }, []);

  // ── Otomatis buka kembali modal saat poligon selesai digambar ────────
  const prevIsDrawing = useRef(isDrawing);
  useEffect(() => {
    if (prevIsDrawing.current && !isDrawing && draftPolygon) {
      setIsModalOpen(true);
    }
    prevIsDrawing.current = isDrawing;
  }, [isDrawing, draftPolygon]);

  // ── Handlers Pembuatan & Edit AOI ────────────────────────────────────
  const handleOpenCreateModal = () => {
    setError(null);
    if (map && isDrawing) stopDrawing(map);
    if (isModifyingPolygon) clearAoi();

    setEditingAoi(null);
    setIsModifyingPolygon(false);
    setAoiId(generateAoiId());
    setAoiName("Area Demo");
    setAoiType("Area Analisis");
    setIsModalOpen(true);
  };

  const handleStartEdit = (targetAoi: AoiFeature) => {
    setError(null);
    if (map && isDrawing) stopDrawing(map);
    if (isModifyingPolygon) clearAoi();

    setEditingAoi(targetAoi);
    setIsModifyingPolygon(false);
    setAoiId(targetAoi.properties.aoi_id);
    setAoiName(targetAoi.properties.name);
    setAoiType(targetAoi.properties.aoi_type || "");
    setIsModalOpen(true);
    handleFocusAoi(targetAoi);
  };

  const handleCloseModal = () => {
    setIsModalOpen(false);
    if (!editingAoi) {
      clearAoi();
    }
    setError(null);
  };

  const handleStartDrawOnMap = () => {
    setIsModalOpen(false);
    setError(null);
    if (map) startDrawing(map);
  };

  const handleCancelDrawing = () => {
    if (map) stopDrawing(map);
    clearAoi();
  };

  const handleStartModifyPolygon = () => {
    if (!map || !editingAoi) return;
    setIsModalOpen(false);
    setError(null);
    setIsModifyingPolygon(true);
    loadAoiForEditing(editingAoi, map);
  };

  const handleFinishModifyPolygon = () => {
    setIsModalOpen(true);
  };

  const handleCancelModifyPolygon = () => {
    clearAoi();
    setIsModifyingPolygon(false);
    window.dispatchEvent(new CustomEvent("aoi:editing-stopped"));
  };

  const handleSave = async () => {
    if (!aoiName.trim()) {
      setError("Nama area tidak boleh kosong");
      return;
    }
    setError(null);

    try {
      if (editingAoi) {
        // Mode Edit
        if (isModifyingPolygon && draftPolygon) {
          await saveAoi(editingAoi.properties.aoi_id, aoiName.trim(), aoiType.trim());
        } else {
          const updated: AoiFeature = {
            ...editingAoi,
            properties: {
              ...editingAoi.properties,
              name: aoiName.trim(),
              aoi_type: aoiType.trim() || undefined,
            },
          };
          await saveAoiToStorage(updated);
          window.dispatchEvent(new CustomEvent("aoi:saved", { detail: updated }));
        }
        setIsModalOpen(false);
        setEditingAoi(null);
        setIsModifyingPolygon(false);
        setAoiId(generateAoiId());
        setAoiName("Area Demo");
        setAoiType("Area Analisis");
      } else {
        // Mode Buat Baru
        await saveAoi(aoiId.trim(), aoiName.trim(), aoiType.trim());
        setIsModalOpen(false);
        setAoiId(generateAoiId());
        setAoiName("Area Demo");
        setAoiType("Area Analisis");
      }
    } catch (err: unknown) {
      if (err instanceof Error) {
        setError(err.message);
      } else if (typeof err === "object" && err !== null && "message" in err) {
        setError(String(err.message));
      } else {
        setError("Terjadi kesalahan yang tidak diketahui");
      }
    }
  };

  // ── Handlers Manajemen AOI (Fokus & Hapus) ───────────────────────────
  const handleFocusAoi = (targetAoi: AoiFeature) => {
    if (!map) return;
    const ring = targetAoi.geometry.coordinates[0];
    if (!ring || ring.length === 0) return;
    const latlngs = ring.map(([lng, lat]) => [lat, lng] as [number, number]);
    const bounds = L.latLngBounds(latlngs);
    map.fitBounds(bounds, { padding: [40, 40], maxZoom: 18, animate: true, duration: 1.2 });
  };

  const handleDeleteAoi = async (targetAoi: AoiFeature) => {
    const name = targetAoi.properties.name || targetAoi.properties.aoi_id;
    if (!window.confirm(`Hapus AOI "${name}" dari peta dan penyimpanan?`)) {
      return;
    }
    try {
      await deleteSavedAoi(targetAoi.properties.aoi_id);
      window.dispatchEvent(
        new CustomEvent("aoi:deleted", {
          detail: { aoiId: targetAoi.properties.aoi_id },
        }),
      );
      if (editingAoi?.properties.aoi_id === targetAoi.properties.aoi_id) {
        handleCloseModal();
      }
    } catch (err: unknown) {
      console.error("Gagal menghapus AOI:", err);
      setError(`Gagal menghapus AOI: ${err instanceof Error ? err.message : String(err)}`);
    }
  };

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

      {/* Banner Pemandu Interaktif saat Menggambar Baru di Peta */}
      {isDrawing && (
        <div
          style={{
            background: "#0f172a",
            color: "#ffffff",
            padding: "14px 16px",
            borderRadius: "14px",
            display: "flex",
            flexDirection: "column",
            gap: "10px",
            boxShadow: "0 4px 16px rgba(15, 23, 42, 0.2)",
          }}
        >
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: "8px",
              fontSize: "13px",
              fontWeight: 600,
            }}
          >
            <Icon icon="locate" color="#38bdf8" size={16} />
            <span>Sedang Menggambar di Peta...</span>
          </div>
          <p style={{ fontSize: "11px", color: "#cbd5e1", margin: 0, lineHeight: 1.4 }}>
            Klik titik-titik sudut area di atas peta. Klik titik awal untuk menutup poligon.
          </p>
          <Button
            minimal
            small
            text="Batal Gambar"
            icon="cross"
            onClick={handleCancelDrawing}
            style={{
              background: "rgba(255, 255, 255, 0.15)",
              color: "#ffffff",
              borderRadius: "8px",
              fontSize: "11px",
              fontWeight: 600,
              outline: "none",
            }}
          />
        </div>
      )}

      {/* Banner Pemandu Interaktif saat Menggeser Verteks Poligon di Peta */}
      {isModifyingPolygon && (
        <div
          style={{
            background: "#0f172a",
            color: "#ffffff",
            padding: "14px 16px",
            borderRadius: "14px",
            display: "flex",
            flexDirection: "column",
            gap: "10px",
            boxShadow: "0 4px 16px rgba(15, 23, 42, 0.2)",
          }}
        >
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: "8px",
              fontSize: "13px",
              fontWeight: 600,
            }}
          >
            <Icon icon="polygon-filter" color="#f59e0b" size={16} />
            <span>Ubah Bentuk Poligon di Peta</span>
          </div>
          <p style={{ fontSize: "11px", color: "#cbd5e1", margin: 0, lineHeight: 1.4 }}>
            Geser titik-titik sudut poligon di atas peta untuk menyesuaikan batas area.
          </p>
          <div style={{ display: "flex", gap: "8px" }}>
            <Button
              minimal
              small
              text="Batal"
              icon="cross"
              onClick={handleCancelModifyPolygon}
              style={{
                background: "rgba(255, 255, 255, 0.15)",
                color: "#ffffff",
                borderRadius: "8px",
                fontSize: "11px",
                fontWeight: 600,
                outline: "none",
              }}
            />
            <Button
              minimal
              small
              text="Selesai & Buka Form"
              icon="tick"
              onClick={handleFinishModifyPolygon}
              style={{
                background: "#38bdf8",
                color: "#0f172a",
                fontWeight: 700,
                borderRadius: "8px",
                fontSize: "11px",
                flex: 1,
                outline: "none",
              }}
            />
          </div>
        </div>
      )}

      {/* Sub-Seksi Daftar AOI */}
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
            {savedAois.length} Area
          </span>
        </div>

        {isLoadingAois && (
          <div style={{ display: "flex", justifyContent: "center", padding: "16px" }}>
            <Spinner size={20} />
          </div>
        )}

        {!isLoadingAois && savedAois.length === 0 && (
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
          {savedAois.map((item) => (
            <AoiCardItem
              key={item.properties.aoi_id}
              item={item}
              isEditing={isModalOpen && editingAoi?.properties.aoi_id === item.properties.aoi_id}
              onFocus={handleFocusAoi}
              onEdit={handleStartEdit}
              onDelete={() => void handleDeleteAoi(item)}
            />
          ))}
        </div>
      </div>

      {/* Modal Dialog Buat & Edit AOI */}
      <AoiModal
        isOpen={isModalOpen}
        isEditing={editingAoi !== null}
        aoiId={aoiId}
        aoiName={aoiName}
        setAoiName={setAoiName}
        aoiType={aoiType}
        setAoiType={setAoiType}
        draftPolygon={draftPolygon}
        editingAoi={editingAoi}
        isSaving={isSaving}
        error={error}
        onClose={handleCloseModal}
        onStartDrawOnMap={handleStartDrawOnMap}
        onStartModifyPolygon={handleStartModifyPolygon}
        onSave={() => void handleSave()}
      />
    </div>
  );
};

export const AoiDemoToolbar = AoiToolbar;
