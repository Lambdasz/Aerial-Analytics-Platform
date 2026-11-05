import React, { useState, useEffect, useCallback, useMemo } from "react";
import * as L from "leaflet";
import { InputGroup, Button, ButtonGroup, Spinner, NonIdealState } from "@blueprintjs/core";
import type {
  DeleteSpatialResultSummary,
  SpatialResultSummary,
  UpdateSpatialResultPayload,
} from "../types/spatialResult";
import {
  countSpatialResultDependents,
  deleteSavedSpatialResult,
  getSavedSpatialResult,
  getSavedSpatialResultsSummary,
  updateSavedSpatialResult,
} from "../services/api";
import { SpResultCardItem } from "../molecules/SpResultCardItem";
import { EditSpResultModal } from "../molecules/EditSpResultModal";
import { DeleteSpResultAlert } from "../molecules/DeleteSpResultAlert";
import { showToast } from "../lib/toaster";

export interface SpResultListPanelProps {
  map?: L.Map | null;
}

export const SpResultListPanel: React.FC<SpResultListPanelProps> = ({ map }) => {
  const [items, setItems] = useState<SpatialResultSummary[]>([]);
  const [isLoading, setIsLoading] = useState<boolean>(true);
  const [searchQuery, setSearchQuery] = useState<string>("");
  const [typeFilter, setTypeFilter] = useState<string>("all");
  const [selectedId, setSelectedId] = useState<string | null>(null);

  // Modal states
  const [editingItem, setEditingItem] = useState<SpatialResultSummary | null>(null);
  const [isSavingEdit, setIsSavingEdit] = useState<boolean>(false);

  const [deletingItem, setDeletingItem] = useState<SpatialResultSummary | null>(null);
  const [deleteDependents, setDeleteDependents] = useState<DeleteSpatialResultSummary | null>(null);
  const [isLoadingDeleteCount, setIsLoadingDeleteCount] = useState<boolean>(false);
  const [isDeleting, setIsDeleting] = useState<boolean>(false);

  const [isProcessing, setIsProcessing] = useState<boolean>(false);

  const fetchItems = useCallback(() => {
    getSavedSpatialResultsSummary()
      .then((data) => {
        setItems(data || []);
        setIsLoading(false);
      })
      .catch((err: unknown) => {
        console.error("Gagal memuat daftar hasil analisis:", err);
        void showToast("Gagal memuat hasil analisis spasial", "danger");
        setIsLoading(false);
      });
  }, []);

  useEffect(() => {
    fetchItems();

    const handleSaved = () => fetchItems();
    const handleDeleted = () => fetchItems();
    const handleSelectResult = (e: Event) => {
      const customEvent = e as CustomEvent<{ id?: string }>;
      if (customEvent.detail?.id) {
        const targetId = customEvent.detail.id;
        setSelectedId(targetId);
        setTimeout(() => {
          const el = document.getElementById(`sp-result-card-${targetId}`);
          if (el) {
            el.scrollIntoView({ behavior: "smooth", block: "center" });
          }
        }, 150);

        // Opsi A: Otomatis terbangkan peta ke poligon/fitur spasial terkait (tanpa memuat gambar raster di peta)
        if (map) {
          getSavedSpatialResult(targetId)
            .then((detail) => {
              if (!detail.spatial_results || detail.spatial_results.length === 0) return;
              const points: [number, number][] = [];
              for (const feat of detail.spatial_results) {
                const geom = feat.geometry;
                if (geom.type === "Point") {
                  points.push([geom.coordinates[1], geom.coordinates[0]]);
                } else if (geom.type === "Polygon") {
                  for (const ring of geom.coordinates) {
                    for (const coord of ring) {
                      points.push([coord[1], coord[0]]);
                    }
                  }
                } else if (geom.type === "LineString") {
                  for (const coord of geom.coordinates) {
                    points.push([coord[1], coord[0]]);
                  }
                }
              }

              if (points.length === 1) {
                map.flyTo(points[0], 18, { duration: 1.2 });
              } else if (points.length > 1) {
                const bounds = L.latLngBounds(points);
                map.fitBounds(bounds, {
                  padding: [60, 60],
                  maxZoom: 18,
                  animate: true,
                  duration: 1.2,
                });
              }
            })
            .catch((err) => {
              console.warn("Gagal mengarahkan peta ke hasil spasial:", err);
            });
        }
      }
    };

    window.addEventListener("spatial_result:saved", handleSaved);
    window.addEventListener("spatial_result:deleted", handleDeleted);
    window.addEventListener("open-results-tab", handleSelectResult);

    return () => {
      window.removeEventListener("spatial_result:saved", handleSaved);
      window.removeEventListener("spatial_result:deleted", handleDeleted);
      window.removeEventListener("open-results-tab", handleSelectResult);
    };
  }, [fetchItems, map]);

  const filteredItems = useMemo(() => {
    return items.filter((item) => {
      const matchesSearch =
        searchQuery.trim() === "" ||
        item.layer_name.toLowerCase().includes(searchQuery.toLowerCase()) ||
        item.id.toLowerCase().includes(searchQuery.toLowerCase());

      const matchesType =
        typeFilter === "all" || item.geom_type.toLowerCase() === typeFilter.toLowerCase();

      return matchesSearch && matchesType;
    });
  }, [items, searchQuery, typeFilter]);

  const handleFocus = async (item: SpatialResultSummary) => {
    setSelectedId(item.id);
    if (!map) return;

    try {
      setIsProcessing(true);
      const detail = await getSavedSpatialResult(item.id);
      // 1. Dukungan ImageOverlay dengan koordinat bounds WGS84
      if (
        detail.display_preference &&
        detail.display_preference.type === "image_overlay" &&
        detail.display_preference.bounds
      ) {
        const b = detail.display_preference.bounds;
        const bounds = L.latLngBounds(b[0], b[1]);
        if (bounds.isValid()) {
          map.fitBounds(bounds, {
            padding: [50, 50],
            maxZoom: 18,
            animate: true,
            duration: 1.2,
          });
          return;
        }
      }

      if (!detail.spatial_results || detail.spatial_results.length === 0) return;

      // 2. Kumpulkan semua titik koordinat untuk menghitung Bounding Box WGS84
      const points: [number, number][] = [];
      for (const feat of detail.spatial_results) {
        const geom = feat.geometry;
        if (geom.type === "Point") {
          points.push([geom.coordinates[1], geom.coordinates[0]]);
        } else if (geom.type === "Polygon") {
          for (const ring of geom.coordinates) {
            for (const coord of ring) {
              points.push([coord[1], coord[0]]);
            }
          }
        } else if (geom.type === "LineString") {
          for (const coord of geom.coordinates) {
            points.push([coord[1], coord[0]]);
          }
        }
      }

      if (points.length > 0) {
        const bounds = L.latLngBounds(points);
        if (bounds.isValid()) {
          map.fitBounds(bounds, {
            padding: [50, 50],
            maxZoom: 18,
            animate: true,
            duration: 1.2,
          });
        }
      }
    } catch (err: unknown) {
      console.warn("Gagal mengambil geometri hasil untuk fokus:", err);
    } finally {
      setIsProcessing(false);
    }
  };

  const handleSaveEdit = async (payload: UpdateSpatialResultPayload) => {
    setIsSavingEdit(true);
    try {
      const res = await updateSavedSpatialResult(payload);
      void showToast(`Layer '${res.layer_name}' berhasil diperbarui.`, "success");
      window.dispatchEvent(
        new CustomEvent("spatial_result:saved", {
          detail: res,
        }),
      );
      fetchItems();
      setEditingItem(null);
    } finally {
      setIsSavingEdit(false);
    }
  };

  const handleDeleteClick = (target: SpatialResultSummary) => {
    setDeletingItem(target);
    if (!target.is_corrupted) {
      setIsLoadingDeleteCount(true);
      countSpatialResultDependents(target.id)
        .then((res) => {
          setDeleteDependents(res);
        })
        .catch(() => {
          setDeleteDependents({
            id: target.id,
            deleted_from_disk: true,
            layers_removed: 1,
            feature_count: target.feature_count,
            photos_count: 0,
          });
        })
        .finally(() => {
          setIsLoadingDeleteCount(false);
        });
    } else {
      setDeleteDependents(null);
    }
  };

  const handleConfirmDelete = async () => {
    if (!deletingItem) return;
    setIsDeleting(true);
    const targetId = deletingItem.id;
    const targetName = deletingItem.layer_name;

    try {
      await deleteSavedSpatialResult(targetId);
      window.dispatchEvent(
        new CustomEvent("spatial_result:deleted", {
          detail: { id: targetId },
        }),
      );
      void showToast(`Layer '${targetName}' berhasil dihapus.`, "success");
      setItems((prev) => prev.filter((i) => i.id !== targetId));
      if (selectedId === targetId) {
        setSelectedId(null);
      }
      setDeletingItem(null);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      void showToast(`Gagal menghapus layer: ${msg}`, "danger");
    } finally {
      setIsDeleting(false);
    }
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%", gap: "12px" }}>
      {/* Header Info */}
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          paddingBottom: "8px",
          borderBottom: "1px solid rgba(16, 22, 26, 0.1)",
        }}
      >
        <span style={{ fontSize: "14px", fontWeight: 700, color: "var(--color-ink, #182026)" }}>
          Hasil Analisis
        </span>
        <span
          style={{
            fontSize: "11px",
            background: "rgba(46, 204, 64, 0.15)",
            color: "#2ecc40",
            padding: "2px 8px",
            borderRadius: "10px",
            fontWeight: 600,
          }}
        >
          {items.length} Tersimpan
        </span>
      </div>

      {/* Search Input */}
      <InputGroup
        leftIcon="search"
        placeholder="Cari layer analisis..."
        value={searchQuery}
        onChange={(e) => setSearchQuery(e.target.value)}
        rightElement={
          searchQuery ? (
            <Button icon="cross" minimal onClick={() => setSearchQuery("")} />
          ) : undefined
        }
        small
      />

      {/* Filter Kategori Tipe Geometri */}
      <ButtonGroup fill style={{ marginBottom: "4px" }}>
        <Button
          text="Semua"
          active={typeFilter === "all"}
          onClick={() => setTypeFilter("all")}
          small
        />
        <Button
          text="Polygon"
          active={typeFilter === "polygon"}
          onClick={() => setTypeFilter("polygon")}
          small
        />
        <Button
          text="Point"
          active={typeFilter === "point"}
          onClick={() => setTypeFilter("point")}
          small
        />
        <Button
          text="Line"
          active={typeFilter === "linestring"}
          onClick={() => setTypeFilter("linestring")}
          small
        />
      </ButtonGroup>

      {/* List Konten */}
      <div style={{ flex: 1, overflowY: "auto" }}>
        {isLoading ? (
          <div style={{ textAlign: "center", padding: "40px 0" }}>
            <Spinner size={24} />
            <div style={{ marginTop: "10px", fontSize: "12px", color: "#8a9ba8" }}>
              Memuat data hasil analisis...
            </div>
          </div>
        ) : filteredItems.length === 0 ? (
          <NonIdealState
            icon="polygon-filter"
            title="Tidak Ada Hasil Analisis"
            description={
              searchQuery
                ? "Tidak ada hasil analisis yang sesuai dengan pencarian."
                : "Belum ada layer hasil analisis yang tersimpan di sistem."
            }
          />
        ) : (
          filteredItems.map((item) => (
            <SpResultCardItem
              key={item.id}
              item={item}
              isSelected={selectedId === item.id}
              isProcessing={isProcessing}
              onFocus={(target) => {
                void handleFocus(target);
              }}
              onEdit={(target) => setEditingItem(target)}
              onDelete={handleDeleteClick}
            />
          ))
        )}
      </div>

      {/* Modals & Alerts */}
      <EditSpResultModal
        isOpen={Boolean(editingItem)}
        item={editingItem}
        onClose={() => setEditingItem(null)}
        onSave={handleSaveEdit}
        onLoadDetail={getSavedSpatialResult}
        isSubmitting={isSavingEdit}
      />

      <DeleteSpResultAlert
        isOpen={Boolean(deletingItem)}
        item={deletingItem}
        dependents={deleteDependents}
        isLoadingCount={isLoadingDeleteCount}
        isDeleting={isDeleting}
        onClose={() => setDeletingItem(null)}
        onConfirm={() => {
          void handleConfirmDelete();
        }}
      />
    </div>
  );
};
