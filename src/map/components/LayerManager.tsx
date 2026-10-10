import React, { useMemo, useState } from "react";
import { Button, Tag, Slider, HTMLSelect } from "@blueprintjs/core";

import type { LayerState, AnnotationFeature } from "../types/layer";

import {
  toggleLayerVisibility,
  setLayerOpacity,
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
    layer_id: "orthomosaic_01",
    layer_name: "Orthomosaic Drone",
    category: "Drone Imagery",
    is_visible: true,
    opacity: 1,
    z_index: 5,
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
    <div style={{ paddingBottom: "20px" }}>
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
          className="apple-select"
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
        return (
          <div
            key={layer.layer_id}
            style={{
              marginBottom: "16px",
              padding: "16px",
              background: "var(--color-gallery-white)",
              borderRadius: "var(--radius-cards)",
              boxShadow: "var(--shadow-subtle)",
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

                <Tag
                  minimal
                  round
                  style={{
                    background: "var(--color-control-gray)",
                    color: "var(--color-ink)",
                    fontSize: "11px",
                    fontWeight: 500,
                  }}
                >
                  {layer.category}
                </Tag>
              </div>

              <Button
                icon={layer.is_visible ? "eye-open" : "eye-off"}
                text={layer.is_visible ? "ON" : "OFF"}
                minimal={!layer.is_visible}
                style={{
                  borderRadius: "var(--radius-buttons)",
                  background: layer.is_visible ? "var(--color-control-gray)" : "transparent",
                  boxShadow: "none",
                  color: layer.is_visible ? "var(--color-apple-blue)" : "var(--color-slate)",
                  fontWeight: 600,
                }}
                onClick={() => {
                  void handleToggleVisibility(layer);
                }}
              />
            </div>

            {/* Opacity */}

            <div
              style={{
                marginTop: "10px",
                padding: "0 8px",
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
          borderTop: "1px solid var(--color-hairline-silver)",
        }}
      >
        <div
          style={{
            marginBottom: "10px",
            background: "var(--color-gallery-white)",
            boxShadow: "var(--shadow-subtle)",
            borderRadius: "12px",
            padding: "8px 12px",
          }}
        >
          <textarea
            placeholder="Tulis catatan annotation..."
            value={annotationText}
            onChange={(event) => setAnnotationText(event.target.value)}
            style={{
              width: "100%",
              minHeight: "60px",
              border: "none",
              outline: "none",
              background: "transparent",
              color: "var(--color-ink)",
              fontSize: "14px",
              fontFamily: "var(--font-sf-pro-text)",
              resize: "vertical",
            }}
          />
        </div>

        <div
          style={{
            display: "flex",
            gap: "8px",
            marginBottom: "12px",
          }}
        >
          <div
            style={{
              flex: 1,
              display: "flex",
              alignItems: "center",
              background: "var(--color-gallery-white)",
              boxShadow: "var(--shadow-subtle)",
              borderRadius: "12px",
              padding: "8px 12px",
            }}
          >
            <input
              type="number"
              placeholder="Latitude"
              value={latitude}
              onChange={(event) => setLatitude(event.target.value)}
              style={{
                width: "100%",
                border: "none",
                outline: "none",
                background: "transparent",
                color: "var(--color-ink)",
                fontSize: "14px",
                fontFamily: "var(--font-sf-pro-text)",
              }}
            />
          </div>

          <div
            style={{
              flex: 1,
              display: "flex",
              alignItems: "center",
              background: "var(--color-gallery-white)",
              boxShadow: "var(--shadow-subtle)",
              borderRadius: "12px",
              padding: "8px 12px",
            }}
          >
            <input
              type="number"
              placeholder="Longitude"
              value={longitude}
              onChange={(event) => setLongitude(event.target.value)}
              style={{
                width: "100%",
                border: "none",
                outline: "none",
                background: "transparent",
                color: "var(--color-ink)",
                fontSize: "14px",
                fontFamily: "var(--font-sf-pro-text)",
              }}
            />
          </div>
        </div>

        <Button
          icon="add"
          text="Buat Annotation"
          fill
          onClick={() => {
            void handleCreateAnnotation();
          }}
          style={{
            borderRadius: "999px",
            background: "var(--color-apple-blue)",
            color: "#fff",
            boxShadow: "none",
            fontWeight: 600,
            padding: "8px 16px",
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
                  padding: "16px",
                  marginBottom: "12px",
                  background: "var(--color-gallery-white)",
                  borderRadius: "var(--radius-cards)",
                  boxShadow: "var(--shadow-subtle)",
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
    </div>
  );
};
