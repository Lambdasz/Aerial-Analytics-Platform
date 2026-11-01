import React, { useState } from "react";
import * as L from "leaflet";
import "../leaflet-setup";
import type { LayerState, AnnotationFeature, SpatialGeometry } from "../types/layer";
import type { DroneImageMetadata } from "../types/marker";
import { useAnnotations } from "../hooks/useAnnotations";
import { useManagedLayers } from "../hooks/useManagedLayers";
import { LayerCategoryFilter } from "../molecules/LayerCategoryFilter";
import { AnnotationModal } from "./dialogs/AnnotationModal";
import type { AnnotationGeomType } from "../molecules/AnnotationForm";
import { RasterLayerListSection } from "./RasterLayerListSection";
import { AnnotationListSection } from "./AnnotationListSection";

export interface LayerManagerProps {
  map?: L.Map | null;
  /** Marker sesi aktif — sumber daftar per-foto di bawah Flight Photos. */
  markers?: ReadonlyArray<DroneImageMetadata>;
  /** image_id yang sedang disembunyikan dari peta. */
  hiddenPhotoIds?: ReadonlySet<string>;
  onTogglePhotoHidden?: (imageId: string) => void;
  onAnnotationsChange?: (annotations: AnnotationFeature[]) => void;
  onLayersChange?: (layers: LayerState[]) => void;
}

export const LayerManager: React.FC<LayerManagerProps> = ({
  map,
  markers,
  hiddenPhotoIds,
  onTogglePhotoHidden,
  onAnnotationsChange,
  onLayersChange,
}) => {
  const [isAnnotationModalOpen, setIsAnnotationModalOpen] = useState(false);
  const [editingAnnotation, setEditingAnnotation] = useState<AnnotationFeature | null>(null);
  const [isPickingAnnotation, setIsPickingAnnotation] = useState(false);
  const [pickingGeomType, setPickingGeomType] = useState<AnnotationGeomType>("Point");

  // Hook manajemen layer peta
  const {
    categories,
    selectedCategory,
    setSelectedCategory,
    visibleLayers,
    handleToggleVisibility,
    handleSetOpacity,
  } = useManagedLayers({ onLayersChange });

  // Hook manajemen anotasi mandiri
  const { annotations, addAnnotation, editAnnotation, removeAnnotation } = useAnnotations();

  React.useEffect(() => {
    onAnnotationsChange?.(annotations);
  }, [annotations, onAnnotationsChange]);

  const handleFocusAnnotation = (annotation: AnnotationFeature) => {
    if (!map) return;
    const { geometry } = annotation;

    if (geometry.type === "Point") {
      const [lng, lat] = geometry.coordinates;
      map.flyTo([lat, lng], 18, { duration: 1.2 });
    } else if (geometry.type === "LineString") {
      const positions: [number, number][] = geometry.coordinates.map(([lng, lat]) => [lat, lng]);
      const bounds = L.latLngBounds(positions);
      map.fitBounds(bounds, { padding: [40, 40], maxZoom: 18, animate: true, duration: 1.2 });
    } else if (geometry.type === "Polygon") {
      const ring = (geometry.coordinates[0] || []) as [number, number][];
      const positions: [number, number][] = ring.map(([lng, lat]) => [lat, lng]);
      const bounds = L.latLngBounds(positions);
      map.fitBounds(bounds, { padding: [40, 40], maxZoom: 18, animate: true, duration: 1.2 });
    }
  };

  const handleStartEditAnnotation = (annotation: AnnotationFeature) => {
    setEditingAnnotation(annotation);
    setIsAnnotationModalOpen(true);
    if (map) {
      handleFocusAnnotation(annotation);
    }
  };

  const handleSaveAnnotation = async (args: { text: string; geometry: SpatialGeometry }) => {
    await addAnnotation(args);
  };

  const handleUpdateAnnotation = async (args: { text: string; geometry: SpatialGeometry }) => {
    if (!editingAnnotation) return;
    const targetId = editingAnnotation.id || editingAnnotation.annotation_id || "";
    await editAnnotation({
      ...editingAnnotation,
      id: targetId,
      annotation_id: targetId,
      text: args.text,
      geometry: args.geometry,
    });
    setEditingAnnotation(null);
  };

  const handleDeleteAnnotation = async (id: string) => {
    if (!id) return;
    if (
      editingAnnotation &&
      (editingAnnotation.id === id || editingAnnotation.annotation_id === id)
    ) {
      setIsAnnotationModalOpen(false);
      setEditingAnnotation(null);
    }
    await removeAnnotation(id);
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%" }}>
      {/* Header Info */}
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          paddingBottom: "8px",
          borderBottom: "1px solid rgba(16, 22, 26, 0.08)",
          marginBottom: "12px",
        }}
      >
        <span style={{ fontSize: "14px", fontWeight: 700, color: "#0f172a" }}>Daftar Layer</span>
        <span style={{ fontSize: "11px", color: "#64748b", fontWeight: 500 }}>
          {visibleLayers.length} Layer
        </span>
      </div>

      {/* Category Filter Pills */}
      <LayerCategoryFilter
        categories={categories}
        selectedCategory={selectedCategory}
        onSelectCategory={setSelectedCategory}
      />

      {/* Raster Layer List & Flight Photos Section */}
      <RasterLayerListSection
        visibleLayers={visibleLayers}
        onToggleVisibility={handleToggleVisibility}
        onOpacityChange={handleSetOpacity}
        markers={markers}
        hiddenPhotoIds={hiddenPhotoIds}
        onTogglePhotoHidden={onTogglePhotoHidden}
      />

      {/* Spatial Annotations Section */}
      <AnnotationListSection
        annotations={annotations}
        isPickingAnnotation={isPickingAnnotation}
        pickingGeomType={pickingGeomType}
        isAnnotationModalOpen={isAnnotationModalOpen}
        editingAnnotation={editingAnnotation}
        hasMap={Boolean(map)}
        onOpenCreateModal={() => {
          setEditingAnnotation(null);
          setIsAnnotationModalOpen(true);
        }}
        onFocusAnnotation={handleFocusAnnotation}
        onStartEditAnnotation={handleStartEditAnnotation}
        onDeleteAnnotation={(id) => {
          void handleDeleteAnnotation(id);
        }}
      />

      {/* Modal Dialog Pembuatan & Edit Anotasi Spasial */}
      <AnnotationModal
        isOpen={isAnnotationModalOpen}
        onClose={() => {
          setIsAnnotationModalOpen(false);
          setEditingAnnotation(null);
        }}
        map={map || null}
        initialAnnotation={editingAnnotation}
        onSaveAnnotation={async (data) => {
          if (editingAnnotation) {
            await handleUpdateAnnotation(data);
          } else {
            await handleSaveAnnotation(data);
          }
        }}
        onDrawingStateChange={(isPicking, geomType) => {
          setIsPickingAnnotation(isPicking);
          setPickingGeomType(geomType);
        }}
      />
    </div>
  );
};
