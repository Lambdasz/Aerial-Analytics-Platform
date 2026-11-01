import { useState, useEffect, useRef } from "react";
import * as L from "leaflet";
import { useAOI } from "./useAOI";
import { deleteSavedAoi, saveAoi as saveAoiToStorage } from "../services/api";
import type { AoiFeature, Polygon } from "../types/aoi";

function generateAoiId(): string {
  return "aoi-" + Date.now();
}

export interface UseAoiModalWorkflowOptions {
  map: L.Map | null;
}

export interface UseAoiModalWorkflowReturn {
  isDrawing: boolean;
  isModifyingPolygon: boolean;
  isSaving: boolean;
  isModalOpen: boolean;
  editingAoi: AoiFeature | null;
  handleOpenCreateModal: () => void;
  handleFocusAoi: (targetAoi: AoiFeature) => void;
  handleStartEdit: (targetAoi: AoiFeature) => void;
  handleDeleteAoi: (targetAoi: AoiFeature) => Promise<void>;
  handleCancelDrawing: () => void;
  handleStartModifyPolygon: () => void;
  handleFinishModifyPolygon: () => void;
  handleCancelModifyPolygon: () => void;
  modalProps: {
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
  };
}

/**
 * Controller hook untuk manajemen state dan alur kerja pembuatan / modifikasi AOI modal dialog.
 * Mengisolasi state form, validasi, penggambaran peta Leaflet/Geoman, dan persistensi API.
 */
export function useAoiModalWorkflow({
  map,
}: UseAoiModalWorkflowOptions): UseAoiModalWorkflowReturn {
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

  // Otomatis buka kembali modal saat poligon selesai digambar
  const prevIsDrawing = useRef(isDrawing);
  useEffect(() => {
    if (prevIsDrawing.current && !isDrawing && draftPolygon) {
      setIsModalOpen(true);
    }
    prevIsDrawing.current = isDrawing;
  }, [isDrawing, draftPolygon]);

  // Handlers Pembuatan & Edit AOI
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

  const handleFocusAoi = (targetAoi: AoiFeature) => {
    if (!map) return;
    const ring = targetAoi.geometry.coordinates[0];
    if (!ring || ring.length === 0) return;
    const latlngs = ring.map(([lng, lat]) => [lat, lng] as [number, number]);
    const bounds = L.latLngBounds(latlngs);
    map.fitBounds(bounds, { padding: [40, 40], maxZoom: 18, animate: true, duration: 1.2 });
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

  return {
    isDrawing,
    isModifyingPolygon,
    isSaving,
    isModalOpen,
    editingAoi,
    handleOpenCreateModal,
    handleFocusAoi,
    handleStartEdit,
    handleDeleteAoi,
    handleCancelDrawing,
    handleStartModifyPolygon,
    handleFinishModifyPolygon,
    handleCancelModifyPolygon,
    modalProps: {
      isOpen: isModalOpen,
      isEditing: editingAoi !== null,
      aoiId,
      aoiName,
      setAoiName,
      aoiType,
      setAoiType,
      draftPolygon,
      editingAoi,
      isSaving,
      error,
      onClose: handleCloseModal,
      onStartDrawOnMap: handleStartDrawOnMap,
      onStartModifyPolygon: handleStartModifyPolygon,
      onSave: () => void handleSave(),
    },
  };
}
