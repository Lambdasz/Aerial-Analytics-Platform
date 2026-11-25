import { useState, useMemo, useEffect } from "react";
import type { LayerState } from "../types/layer";
import type { LayerPayload } from "../types/map";
import { getInitialMapLayers, getSavedSpatialResults } from "../services/api";
import { FLIGHT_PHOTOS_LAYER_ID } from "../config/mapConfig";
import { isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** Reads the persisted visibility map (shared with ortho toggles). */
export const readSavedModes = (): Record<string, boolean> => {
  try {
    const raw = localStorage.getItem("aap_ortho_display_modes");
    return raw ? (JSON.parse(raw) as Record<string, boolean>) : {};
  } catch {
    return {};
  }
};

/** Reads the persisted opacity map. */
export const readSavedOpacities = (): Record<string, number> => {
  try {
    const raw = localStorage.getItem("aap_layer_opacities");
    return raw ? (JSON.parse(raw) as Record<string, number>) : {};
  } catch {
    return {};
  }
};

/** The synthetic Flight Photos layer entry. */
export const buildFlightPhotosLayer = (): LayerState => {
  const modes = readSavedModes();
  const opacities = readSavedOpacities();
  return {
    layer_id: FLIGHT_PHOTOS_LAYER_ID,
    layer_name: "Flight Photos",
    category: "image_overlay",
    is_visible: modes[FLIGHT_PHOTOS_LAYER_ID] ?? true,
    opacity: opacities[FLIGHT_PHOTOS_LAYER_ID] ?? 0.5,
    z_index: 12,
    source_type: "session_images",
    source_id: FLIGHT_PHOTOS_LAYER_ID,
  };
};

/** Returns the list with the Flight Photos entry guaranteed present. */
export const ensureFlightPhotos = (list: LayerState[]): LayerState[] =>
  list.some((l) => l.layer_id === FLIGHT_PHOTOS_LAYER_ID)
    ? list
    : [buildFlightPhotosLayer(), ...list];

export interface UseManagedLayersOptions {
  onLayersChange?: (layers: LayerState[]) => void;
}

export function useManagedLayers({ onLayersChange }: UseManagedLayersOptions = {}) {
  const [layers, setLayers] = useState<LayerState[]>(() => [buildFlightPhotosLayer()]);
  const [selectedCategory, setSelectedCategory] = useState("All");

  const effectiveLayers = useMemo(() => ensureFlightPhotos(layers), [layers]);

  const categories = useMemo(() => {
    const uniqueCategories = Array.from(new Set(effectiveLayers.map((layer) => layer.category)));
    return ["All", ...uniqueCategories];
  }, [effectiveLayers]);

  const visibleLayers = useMemo(() => {
    if (selectedCategory === "All") {
      return effectiveLayers;
    }
    return effectiveLayers.filter((layer) => layer.category === selectedCategory);
  }, [effectiveLayers, selectedCategory]);

  useEffect(() => {
    onLayersChange?.(effectiveLayers);
  }, [effectiveLayers, onLayersChange]);

  useEffect(() => {
    let cancelled = false;

    Promise.all([getInitialMapLayers().catch(() => []), getSavedSpatialResults().catch(() => [])])
      .then(([initialData, spatialData]) => {
        if (!cancelled) {
          const savedModes = readSavedModes();
          const savedOpacities = readSavedOpacities();

          const filtered = (initialData || [])
            .filter(
              (layer) =>
                layer.category !== "Base Maps" &&
                layer.layer_id !== "base_osm" &&
                !layer.layer_name?.toLowerCase().includes("base map"),
            )
            .map((layer) => {
              const saved =
                layer.source_id && savedModes[layer.source_id] !== undefined
                  ? savedModes[layer.source_id]
                  : layer.layer_id && savedModes[layer.layer_id] !== undefined
                    ? savedModes[layer.layer_id]
                    : layer.layer_name && savedModes[layer.layer_name] !== undefined
                      ? savedModes[layer.layer_name]
                      : undefined;
              const isVisible = saved !== undefined ? saved : layer.is_visible;
              const key = layer.source_id || layer.layer_id || layer.layer_name;
              const opacity =
                savedOpacities[key] !== undefined ? savedOpacities[key] : layer.opacity;
              return { ...layer, is_visible: isVisible, opacity };
            });

          const existingIds = new Set(filtered.map((l) => l.layer_id));
          const spatialLayers: LayerState[] = (spatialData || [])
            .filter((sr) => !existingIds.has(sr.layer_id))
            .map((sr) => {
              const saved =
                sr.layer_id && savedModes[sr.layer_id] !== undefined
                  ? savedModes[sr.layer_id]
                  : sr.layer_name && savedModes[sr.layer_name] !== undefined
                    ? savedModes[sr.layer_name]
                    : undefined;
              const isVisible = saved !== undefined ? saved : true;
              const key = sr.layer_id;
              const opacity = savedOpacities[key] !== undefined ? savedOpacities[key] : 1.0;
              return {
                layer_id: sr.layer_id,
                layer_name: sr.layer_name,
                category: "spatial_result",
                is_visible: isVisible,
                opacity,
                z_index: 10,
                source_type: "spatial_result",
                source_id: sr.layer_id,
              };
            });

          setLayers([buildFlightPhotosLayer(), ...filtered, ...spatialLayers]);
        }
      })
      .catch((err) => {
        console.error("Gagal memuat initial layers dari backend:", err);
      });

    const handleSpatialResultSaved = (event: Event) => {
      const customEvent = event as CustomEvent<LayerPayload>;
      const payload = customEvent.detail;
      if (!payload?.layer_id) return;

      setLayers((prev) => {
        const existingIndex = prev.findIndex((l) => l.layer_id === payload.layer_id);
        if (existingIndex >= 0) {
          const updated = [...prev];
          updated[existingIndex] = {
            ...updated[existingIndex],
            layer_name: payload.layer_name,
          };
          return updated;
        } else {
          return [
            ...prev,
            {
              layer_id: payload.layer_id,
              layer_name: payload.layer_name,
              category: "spatial_result",
              is_visible: true,
              opacity: 1.0,
              z_index: 10,
              source_type: "spatial_result",
              source_id: payload.layer_id,
            },
          ];
        }
      });
    };

    const handleSpatialResultDeleted = (event: Event) => {
      const customEvent = event as CustomEvent<{ id: string }>;
      const id = customEvent.detail?.id;
      if (!id) return;
      setLayers((prev) => prev.filter((l) => l.layer_id !== id && l.source_id !== id));
    };

    const handleOrthoImageSaved = () => {
      getInitialMapLayers()
        .then((initialData) => {
          setLayers((prev) => {
            const savedModes = readSavedModes();
            const filtered = (initialData || [])
              .filter(
                (layer) =>
                  layer.category !== "Base Maps" &&
                  layer.layer_id !== "base_osm" &&
                  !layer.layer_name?.toLowerCase().includes("base map"),
              )
              .map((layer) => {
                const saved =
                  layer.source_id && savedModes[layer.source_id] !== undefined
                    ? savedModes[layer.source_id]
                    : layer.layer_id && savedModes[layer.layer_id] !== undefined
                      ? savedModes[layer.layer_id]
                      : layer.layer_name && savedModes[layer.layer_name] !== undefined
                        ? savedModes[layer.layer_name]
                        : undefined;
                const isVisible = saved !== undefined ? saved : layer.is_visible;
                return { ...layer, is_visible: isVisible };
              });
            const rasterIds = new Set(filtered.map((l) => l.layer_id));
            const nonRasters = prev.filter((l) => !rasterIds.has(l.layer_id));
            return [...filtered, ...nonRasters];
          });
        })
        .catch(() => {});
    };

    const handleOrthoImageDeleted = (event: Event) => {
      const customEvent = event as CustomEvent<{ id: string }>;
      const id = customEvent.detail?.id;
      if (!id) return;
      setLayers((prev) => prev.filter((l) => l.layer_id !== id && l.source_id !== id));
    };

    window.addEventListener("spatial_result:saved", handleSpatialResultSaved);
    window.addEventListener("spatial_result:deleted", handleSpatialResultDeleted);
    window.addEventListener("ortho_image:saved", handleOrthoImageSaved);
    window.addEventListener("ortho_image:deleted", handleOrthoImageDeleted);

    let unlistenSaved: (() => void) | undefined;
    let unlistenDeleted: (() => void) | undefined;

    if (isTauri()) {
      void listen<LayerPayload>("spatial_result:saved", (event) => {
        handleSpatialResultSaved(
          new CustomEvent("spatial_result:saved", { detail: event.payload }),
        );
      }).then((fn) => {
        unlistenSaved = fn;
      });

      void listen<{ id: string }>("spatial_result:deleted", (event) => {
        handleSpatialResultDeleted(
          new CustomEvent("spatial_result:deleted", { detail: event.payload }),
        );
      }).then((fn) => {
        unlistenDeleted = fn;
      });
    }

    return () => {
      cancelled = true;
      window.removeEventListener("spatial_result:saved", handleSpatialResultSaved);
      window.removeEventListener("spatial_result:deleted", handleSpatialResultDeleted);
      window.removeEventListener("ortho_image:saved", handleOrthoImageSaved);
      window.removeEventListener("ortho_image:deleted", handleOrthoImageDeleted);
      unlistenSaved?.();
      unlistenDeleted?.();
    };
  }, []);

  useEffect(() => {
    const handleToggleLayerVisibility = (e: Event) => {
      const customEvent = e as CustomEvent<{ layerId: string; nextState?: boolean }>;
      const { layerId, nextState } = customEvent.detail || {};
      if (!layerId) return;

      setLayers((currentLayers) =>
        ensureFlightPhotos(currentLayers).map((item) => {
          const matches =
            item.layer_id === layerId || item.source_id === layerId || item.layer_name === layerId;
          if (!matches) return item;

          const nextVal = nextState !== undefined ? nextState : !item.is_visible;

          try {
            const raw = localStorage.getItem("aap_ortho_display_modes");
            const mapObj = raw ? (JSON.parse(raw) as Record<string, boolean>) : {};
            mapObj[item.layer_id] = nextVal;
            if (item.source_id) {
              mapObj[item.source_id] = nextVal;
            }
            if (item.layer_name) {
              mapObj[item.layer_name] = nextVal;
            }
            localStorage.setItem("aap_ortho_display_modes", JSON.stringify(mapObj));
          } catch {
            // ignore
          }

          return {
            ...item,
            is_visible: nextVal,
          };
        }),
      );
    };

    window.addEventListener("toggle-layer-visibility", handleToggleLayerVisibility);
    return () => {
      window.removeEventListener("toggle-layer-visibility", handleToggleLayerVisibility);
    };
  }, []);

  const handleToggleVisibility = (layer: LayerState) => {
    setLayers((currentLayers) =>
      ensureFlightPhotos(currentLayers).map((item) => {
        if (item.layer_id === layer.layer_id) {
          const nextVal = !item.is_visible;
          try {
            const raw = localStorage.getItem("aap_ortho_display_modes");
            const mapObj = raw ? (JSON.parse(raw) as Record<string, boolean>) : {};
            mapObj[item.layer_id] = nextVal;
            if (item.source_id) {
              mapObj[item.source_id] = nextVal;
            }
            if (item.layer_name) {
              mapObj[item.layer_name] = nextVal;
            }
            localStorage.setItem("aap_ortho_display_modes", JSON.stringify(mapObj));
          } catch {
            // ignore
          }
          return { ...item, is_visible: nextVal };
        }
        return item;
      }),
    );
  };

  const handleSetOpacity = (layer: LayerState, value: number) => {
    setLayers((currentLayers) =>
      ensureFlightPhotos(currentLayers).map((item) => {
        if (item.layer_id === layer.layer_id) {
          try {
            const raw = localStorage.getItem("aap_layer_opacities");
            const mapObj = raw ? (JSON.parse(raw) as Record<string, number>) : {};
            mapObj[item.layer_id] = value;
            if (item.source_id) {
              mapObj[item.source_id] = value;
            }
            if (item.layer_name) {
              mapObj[item.layer_name] = value;
            }
            localStorage.setItem("aap_layer_opacities", JSON.stringify(mapObj));
          } catch {
            // ignore
          }
          return { ...item, opacity: value };
        }
        return item;
      }),
    );
  };

  return {
    layers,
    effectiveLayers,
    selectedCategory,
    setSelectedCategory,
    categories,
    visibleLayers,
    handleToggleVisibility,
    handleSetOpacity,
  };
}
