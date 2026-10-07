import React, { useMemo, useState } from "react";
import { Button, Card, Tag, Slider, HTMLSelect, InputGroup, TextArea } from "@blueprintjs/core";

import type { LayerState, AnnotationFeature } from "../types/layer";

import {
  toggleLayerVisibility,
  setLayerOpacity,
  reorderLayerStack,
  filterLayersByCategory,
  createAnnotation,
} from "../api";

const INITIAL_LAYERS: LayerState[] = [
  {
    layer_id: "base_osm",
    layer_name: "Base Maps",
    category: "Base Maps",
    is_visible: true,
    opacity: 1,
    z_index: 0,
  },
  {
    layer_id: "flight_session_01",
    layer_name: "Drone Imagery",
    category: "Drone Imagery",
    is_visible: true,
    opacity: 1,
    z_index: 10,
  },
  {
    layer_id: "plugin_veg_mask_01",
    layer_name: "Vegetation Mask",
    category: "Analysis Results",
    is_visible: true,
    opacity: 0.5,
    z_index: 20,
  },
  {
    layer_id: "plugin_tree_detect_01",
    layer_name: "Tree Detection",
    category: "Analysis Results",
    is_visible: false,
    opacity: 1,
    z_index: 25,
  },
];

type LayerManagerProps = {
  onAnnotationsChange?: (annotations: AnnotationFeature[]) => void;
  onLayersChange?: (layers: LayerState[]) => void;
};

export const LayerManager: React.FC<LayerManagerProps> = ({
  onAnnotationsChange,
  onLayersChange,
}) => {
  const [layers, setLayers] = useState<LayerState[]>(INITIAL_LAYERS);

  const [selectedCategory, setSelectedCategory] = useState("All");

  const [annotations, setAnnotations] = useState<AnnotationFeature[]>([]);

  const [isExpanded, setIsExpanded] = useState(true);

  const [annotationText, setAnnotationText] = useState("");

  const [latitude, setLatitude] = useState("");

  const [longitude, setLongitude] = useState("");

  // =========================
  // CATEGORY LIST
  // =========================

  const categories = useMemo(() => {
    const uniqueCategories = Array.from(new Set(layers.map((layer) => layer.category)));

    return ["All", ...uniqueCategories];
  }, [layers]);

  // =========================
  // FILTERED LAYERS
  // =========================

  const visibleLayers = useMemo(() => {
    if (selectedCategory === "All") {
      return layers;
    }

    return layers.filter((layer) => layer.category === selectedCategory);
  }, [layers, selectedCategory]);

  // =========================
  // SEND ANNOTATIONS TO PARENT
  // =========================

  React.useEffect(() => {
    onAnnotationsChange?.(annotations);
  }, [annotations, onAnnotationsChange]);

  React.useEffect(() => {
    onLayersChange?.(layers);
    }, [layers, onLayersChange]);

  // =========================
  // LAYER MANAGEMENT
  // =========================

  const handleToggleVisibility = async (layer: LayerState) => {
    try {
      const updatedLayer = await toggleLayerVisibility(layer, !layer.is_visible);

      setLayers((currentLayers) =>
        currentLayers.map((item) =>
          item.layer_id === updatedLayer.layer_id ? updatedLayer : item,
        ),
      );
    } catch (error) {
      console.error("Gagal mengubah visibility layer:", error);
    }
  };

  const handleSetOpacity = async (layer: LayerState, value: number) => {
    try {
      const updatedLayer = await setLayerOpacity(layer, value);

      setLayers((currentLayers) =>
        currentLayers.map((item) =>
          item.layer_id === updatedLayer.layer_id ? updatedLayer : item,
        ),
      );
    } catch (error) {
      console.error("Gagal mengubah opacity layer:", error);
    }
  };

  const handleReorder = async (layer: LayerState, direction: "up" | "down") => {
    const currentIndex = layers.findIndex((item) => item.layer_id === layer.layer_id);

    if (currentIndex === -1) {
      return;
    }

    const targetIndex = direction === "up" ? currentIndex - 1 : currentIndex + 1;

    if (targetIndex < 0 || targetIndex >= layers.length) {
      return;
    }

    try {
      const updatedLayers = await reorderLayerStack(layers, currentIndex, targetIndex);

      setLayers(updatedLayers);
    } catch (error) {
      console.error("Gagal mengubah urutan layer:", error);
    }
  };

  const handleCategoryChange = async (category: string) => {
    setSelectedCategory(category);

    if (category === "All") {
      return;
    }

    try {
      await filterLayersByCategory(layers, category);
    } catch (error) {
      console.error("Gagal melakukan filter layer:", error);
    }
  };

  // =========================
  // SPATIAL ANNOTATION
  // =========================

  const handleCreateAnnotation = async () => {
    if (!annotationText.trim() || !latitude || !longitude) {
      console.error("Teks dan koordinat wajib diisi.");
      return;
    }

    const lat = Number(latitude);
    const lng = Number(longitude);

    if (Number.isNaN(lat) || Number.isNaN(lng)) {
      console.error("Latitude dan longitude harus berupa angka.");
      return;
    }

    if (lat < -90 || lat > 90 || lng < -180 || lng > 180) {
      console.error("Koordinat berada di luar batas yang valid.");
      return;
    }

    try {
      const annotation = await createAnnotation({
        annotationId: `anno_${Date.now()}`,
        text: annotationText.trim(),
        geometry: {
          type: "Point",
          coordinates: [lng, lat],
        },
        createdAt: new Date().toISOString(),
      });

      setAnnotations((current) => [...current, annotation]);

      setAnnotationText("");
      setLatitude("");
      setLongitude("");
    } catch (error) {
      console.error("Gagal membuat annotation:", error);
    }
  };

  // =========================
  // RENDER
  // =========================

  return (
    <Card
      className="layer-manager"
      elevation={2}
      style={{
        position: "absolute",
        top: "205px",
        bottom: isExpanded ? "15px" : undefined,
        left: "15px",
        zIndex: 1000,
        width: "320px",
        height: isExpanded ? undefined : "52px",
        overflowY: isExpanded ? "auto" : "hidden",
        transition: "height 0.2s ease",
        padding: isExpanded ? undefined : "10px 14px",
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          marginBottom: isExpanded ? "16px" : "0",
        }}
      >
        <h5 className="bp5-heading" style={{ margin: 0 }}>
          Layer Management
        </h5>

        <Button
          minimal
          small
          icon={isExpanded ? "chevron-up" : "chevron-down"}
          onClick={() => setIsExpanded((current) => !current)}
        />
      </div>

      {/* =========================
          CATEGORY FILTER
          ========================= */}

      <div
        style={{
          marginBottom: "16px",
        }}
      >
        <div
          style={{
            fontSize: "12px",
            fontWeight: 600,
            marginBottom: "6px",
          }}
        >
          Filter Kategori
        </div>

        <HTMLSelect
          fill
          value={selectedCategory}
          onChange={(event) => {
            void handleCategoryChange(event.target.value);
          }}
        >
          {categories.map((category) => (
            <option key={category} value={category}>
              {category}
            </option>
          ))}
        </HTMLSelect>
      </div>

      {/* =========================
          LAYER LIST
          ========================= */}

      {visibleLayers.map((layer) => {
        const actualIndex = layers.findIndex((item) => item.layer_id === layer.layer_id);

        return (
          <div
            key={layer.layer_id}
            style={{
              marginBottom: "16px",
              paddingBottom: "12px",
              borderBottom: "1px solid #e5e5e5",
            }}
          >
            <div
              style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
                gap: "10px",
              }}
            >
              <div>
                <div
                  style={{
                    fontWeight: 600,
                    marginBottom: "4px",
                  }}
                >
                  {layer.layer_name}
                </div>

                <Tag minimal>{layer.category}</Tag>
              </div>

              <Button
                icon={layer.is_visible ? "eye-open" : "eye-off"}
                text={layer.is_visible ? "ON" : "OFF"}
                onClick={() => {
                  void handleToggleVisibility(layer);
                }}
              />
            </div>

            {/* Opacity */}

            <div
              style={{
                marginTop: "10px",
              }}
            >
              <div
                style={{
                  display: "flex",
                  justifyContent: "space-between",
                  marginBottom: "4px",
                  fontSize: "12px",
                }}
              >
                <span>Opacity</span>

                <span>{Math.round(layer.opacity * 100)}%</span>
              </div>

              <Slider
                min={0}
                max={1}
                stepSize={0.1}
                labelStepSize={0.5}
                value={layer.opacity}
                onChange={(value) => {
                  void handleSetOpacity(layer, value);
                }}
              />
            </div>

            {/* Reorder */}

            <div
              style={{
                display: "flex",
                gap: "6px",
                marginTop: "8px",
              }}
            >
              <Button
                icon="arrow-up"
                text="Naik"
                small
                disabled={actualIndex === 0}
                onClick={() => {
                  void handleReorder(layer, "up");
                }}
              />

              <Button
                icon="arrow-down"
                text="Turun"
                small
                disabled={actualIndex === layers.length - 1}
                onClick={() => {
                  void handleReorder(layer, "down");
                }}
              />

              <Tag minimal>Z: {layer.z_index}</Tag>
            </div>
          </div>
        );
      })}

      {/* =========================
          SPATIAL ANNOTATION
          ========================= */}

      <div
        style={{
          marginTop: "20px",
          paddingTop: "16px",
          borderTop: "2px solid #e5e5e5",
        }}
      >
        <div
          style={{
            marginBottom: "10px",
          }}
        >
          <TextArea
            fill
            placeholder="Tulis catatan annotation..."
            value={annotationText}
            onChange={(event) => setAnnotationText(event.target.value)}
          />
        </div>

        <div
          style={{
            display: "flex",
            gap: "8px",
            marginBottom: "10px",
          }}
        >
          <InputGroup
            type="number"
            placeholder="Latitude"
            value={latitude}
            onChange={(event) => setLatitude(event.target.value)}
          />

          <InputGroup
            type="number"
            placeholder="Longitude"
            value={longitude}
            onChange={(event) => setLongitude(event.target.value)}
          />
        </div>

        <Button
          icon="add"
          text="Buat Annotation"
          intent="primary"
          fill
          onClick={() => {
            void handleCreateAnnotation();
          }}
        />
      </div>

      {/* =========================
          ANNOTATION LIST
          ========================= */}

      {annotations.length > 0 && (
        <div
          style={{
            marginTop: "16px",
          }}
        >
          <h6 className="bp5-heading">Annotation Dibuat</h6>

          {annotations.map((annotation) => {
            const isPoint = annotation.geometry.type === "Point";

            return (
              <div
                key={annotation.annotation_id}
                style={{
                  padding: "8px",
                  marginBottom: "8px",
                  background: "#f5f8fa",
                  borderRadius: "4px",
                }}
              >
                <div
                  style={{
                    fontWeight: 600,
                  }}
                >
                  {annotation.text}
                </div>

                {isPoint && (
                  <small>
                    Koordinat: {annotation.geometry.coordinates[1]},{" "}
                    {annotation.geometry.coordinates[0]}
                  </small>
                )}
              </div>
            );
          })}
        </div>
      )}
    </Card>
  );
};
