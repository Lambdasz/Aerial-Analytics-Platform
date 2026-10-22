import React, { useMemo, useState } from "react";
import type { LayerState, AnnotationFeature } from "../types/layer";
import {
  toggleLayerVisibility,
  setLayerOpacity,
  filterLayersByCategory,
  createAnnotation,
  getInitialMapLayers,
  getSampleAnnotations,
} from "../services/api";
import { LayerCategoryFilter } from "../molecules/LayerCategoryFilter";
import { LayerItem } from "../molecules/LayerItem";
import { AnnotationForm } from "../molecules/AnnotationForm";

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
  // SEND ANNOTATIONS & LAYERS TO PARENT
  // =========================
  React.useEffect(() => {
    onAnnotationsChange?.(annotations);
  }, [annotations, onAnnotationsChange]);

  React.useEffect(() => {
    onLayersChange?.(layers);
  }, [layers, onLayersChange]);

  React.useEffect(() => {
    let cancelled = false;

    getInitialMapLayers()
      .then((data) => {
        if (!cancelled && data.length > 0) {
          setLayers(data);
        }
      })
      .catch((err) => {
        console.error("Gagal memuat initial layers dari backend:", err);
      });

    getSampleAnnotations()
      .then((data) => {
        if (!cancelled && data.length > 0) {
          setAnnotations(data);
        }
      })
      .catch((err) => {
        console.error("Gagal memuat sample annotations dari backend:", err);
      });

    return () => {
      cancelled = true;
    };
  }, []);

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

  return (
    <div style={{ paddingBottom: "20px" }}>
      <LayerCategoryFilter
        categories={categories}
        selectedCategory={selectedCategory}
        onSelectCategory={(category) => {
          void handleCategoryChange(category);
        }}
      />

      {visibleLayers.map((layer) => (
        <LayerItem
          key={layer.layer_id}
          layer={layer}
          onToggleVisibility={(l) => {
            void handleToggleVisibility(l);
          }}
          onOpacityChange={(l, value) => {
            void handleSetOpacity(l, value);
          }}
        />
      ))}

      <AnnotationForm
        annotationText={annotationText}
        latitude={latitude}
        longitude={longitude}
        onTextChange={setAnnotationText}
        onLatitudeChange={setLatitude}
        onLongitudeChange={setLongitude}
        onSubmit={() => {
          void handleCreateAnnotation();
        }}
      />

      {annotations.length > 0 && (
        <div style={{ marginTop: "16px" }}>
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
                <div style={{ fontWeight: 600 }}>{annotation.text}</div>

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
