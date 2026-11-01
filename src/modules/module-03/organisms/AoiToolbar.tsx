import React, { useState, useEffect } from "react";
import * as L from "leaflet";
import { Card, Icon, Spinner, Tag } from "@blueprintjs/core";
import { useAOI } from "../hooks/useAOI";
import { getSavedAois, deleteSavedAoi, saveAoi as saveAoiToStorage } from "../services/api";
import type { AoiFeature } from "../types/aoi";
import { AoiCreateForm } from "../molecules/AoiCreateForm";
import { AoiCardItem } from "../molecules/AoiCardItem";

function generateAoiId(): string {
  return "aoi-" + Date.now();
}

export interface AoiToolbarProps {
  map: L.Map | null;
}

export const AoiToolbar: React.FC<AoiToolbarProps> = ({ map }) => {
  const {
    aoi,
    draftPolygon,
    isDrawing,
    isSaving,
    startDrawing,
    stopDrawing,
    clearAoi,
    loadAoiForEditing,
    saveAoi,
  } = useAOI();

  // State form pembuatan / edit bentuk
  const [aoiId, setAoiId] = useState(generateAoiId);
  const [aoiName, setAoiName] = useState("Area Demo");
  const [aoiType, setAoiType] = useState("Area Analisis");
  const [editingAoiId, setEditingAoiId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  // State daftar AOI tersimpan
  const [savedAois, setSavedAois] = useState<AoiFeature[]>([]);
  const [isLoadingAois, setIsLoadingAois] = useState(true);

  // State inline edit metadata (nama & tipe)
  const [inlineEditingId, setInlineEditingId] = useState<string | null>(null);
  const [inlineName, setInlineName] = useState("");
  const [inlineType, setInlineType] = useState("");
  const [isSavingInline, setIsSavingInline] = useState(false);

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

  // ── Handlers Pembuatan & Edit Poligon ────────────────────────────────
  const handleDraw = () => {
    setError(null);
    setEditingAoiId(null);
    if (map) startDrawing(map);
  };

  const handleCancel = () => {
    if (map) stopDrawing(map);
    clearAoi();
    setEditingAoiId(null);
    setAoiId(generateAoiId());
    setAoiName("Area Demo");
    setError(null);
  };

  const handleSave = async () => {
    setError(null);
    try {
      await saveAoi(aoiId.trim(), aoiName.trim(), aoiType.trim());
      setEditingAoiId(null);
      setAoiId(generateAoiId());
      setAoiName("Area Demo");
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

  // ── Handlers Manajemen AOI (Fokus, Edit, Hapus) ──────────────────────
  const handleFocusAoi = (targetAoi: AoiFeature) => {
    if (!map) return;
    const ring = targetAoi.geometry.coordinates[0];
    if (!ring || ring.length === 0) return;
    const latlngs = ring.map(([lng, lat]) => [lat, lng] as [number, number]);
    const bounds = L.latLngBounds(latlngs);
    map.fitBounds(bounds, { padding: [40, 40], maxZoom: 18, animate: true, duration: 1.2 });
  };

  const handleStartPolygonEdit = (targetAoi: AoiFeature) => {
    if (!map) return;
    setError(null);
    setEditingAoiId(targetAoi.properties.aoi_id);
    setAoiId(targetAoi.properties.aoi_id);
    setAoiName(targetAoi.properties.name);
    setAoiType(targetAoi.properties.aoi_type || "");
    loadAoiForEditing(targetAoi, map);
  };

  const handleStartInlineEdit = (targetAoi: AoiFeature) => {
    setInlineEditingId(targetAoi.properties.aoi_id);
    setInlineName(targetAoi.properties.name);
    setInlineType(targetAoi.properties.aoi_type || "");
    setError(null);
  };

  const handleSaveInlineEdit = async (targetAoi: AoiFeature) => {
    if (!inlineName.trim()) {
      setError("Nama area tidak boleh kosong");
      return;
    }
    setIsSavingInline(true);
    try {
      const updated: AoiFeature = {
        ...targetAoi,
        properties: {
          ...targetAoi.properties,
          name: inlineName.trim(),
          aoi_type: inlineType.trim() || undefined,
        },
      };
      await saveAoiToStorage(updated);
      window.dispatchEvent(new CustomEvent("aoi:saved", { detail: updated }));
      setInlineEditingId(null);
    } catch (err: unknown) {
      console.error("Gagal memperbarui data AOI:", err);
      setError(`Gagal memperbarui AOI: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      setIsSavingInline(false);
    }
  };

  const handleCancelInlineEdit = () => {
    setInlineEditingId(null);
    setInlineName("");
    setInlineType("");
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
      if (editingAoiId === targetAoi.properties.aoi_id) {
        clearAoi();
        setEditingAoiId(null);
        setAoiId(generateAoiId());
        setAoiName("Area Demo");
      }
    } catch (err: unknown) {
      console.error("Gagal menghapus AOI:", err);
      setError(`Gagal menghapus AOI: ${err instanceof Error ? err.message : String(err)}`);
    }
  };

  const isSaveDisabled =
    isSaving || isDrawing || !draftPolygon || aoiId.trim() === "" || aoiName.trim() === "";
  const isCancelDisabled = !isDrawing && !draftPolygon && !editingAoiId;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "16px" }}>
      <AoiCreateForm
        aoiId={aoiId}
        setAoiId={setAoiId}
        aoiName={aoiName}
        setAoiName={setAoiName}
        aoiType={aoiType}
        setAoiType={setAoiType}
        editingAoiId={editingAoiId}
        isDrawing={isDrawing}
        isSaving={isSaving}
        isSaveDisabled={isSaveDisabled}
        isCancelDisabled={isCancelDisabled}
        error={error}
        lastSavedAoi={aoi}
        onDraw={handleDraw}
        onCancel={handleCancel}
        onSave={() => void handleSave()}
      />

      <Card
        elevation={0}
        style={{
          padding: "16px",
          background: "var(--color-gallery-white)",
          boxShadow: "var(--shadow-subtle)",
          borderRadius: "14px",
          border: "1px solid #e2e8f0",
          color: "var(--color-ink)",
        }}
      >
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            marginBottom: "14px",
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
            <Icon icon="layers" size={16} color="var(--color-apple-blue)" />
            Daftar AOI di Peta
          </span>
          <Tag round minimal intent="primary">
            {savedAois.length} Area
          </Tag>
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
              padding: "20px 10px",
              color: "var(--color-slate)",
              fontSize: "13px",
              background: "#f8fafc",
              borderRadius: "10px",
            }}
          >
            <Icon icon="draw" size={24} color="#94a3b8" style={{ marginBottom: 6 }} />
            <div>Belum ada AOI di peta</div>
            <div style={{ fontSize: "11px", marginTop: 4, color: "#94a3b8" }}>
              Gunakan tombol <strong>Draw AOI</strong> di atas untuk membuat area baru.
            </div>
          </div>
        )}

        <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
          {savedAois.map((item) => (
            <AoiCardItem
              key={item.properties.aoi_id}
              item={item}
              isInline={inlineEditingId === item.properties.aoi_id}
              isEditingThisPolygon={editingAoiId === item.properties.aoi_id}
              inlineName={inlineName}
              setInlineName={setInlineName}
              inlineType={inlineType}
              setInlineType={setInlineType}
              isSavingInline={isSavingInline}
              isSaving={isSaving}
              isSaveDisabled={isSaveDisabled}
              onSaveInlineEdit={() => void handleSaveInlineEdit(item)}
              onCancelInlineEdit={handleCancelInlineEdit}
              onFocus={handleFocusAoi}
              onStartInlineEdit={handleStartInlineEdit}
              onStartPolygonEdit={handleStartPolygonEdit}
              onDelete={() => void handleDeleteAoi(item)}
              onCancelDrawing={handleCancel}
              onSaveDrawing={() => void handleSave()}
            />
          ))}
        </div>
      </Card>
    </div>
  );
};

export const AoiDemoToolbar = AoiToolbar;
