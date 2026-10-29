import React, { useMemo, useState, useEffect } from "react";
import * as L from "leaflet";
import "../leaflet-setup";
import { Button, NonIdealState, Tooltip, Tag } from "@blueprintjs/core";
import type { LayerState, AnnotationFeature, SpatialGeometry } from "../types/layer";
import type { LayerPayload } from "../types/map";
import { getInitialMapLayers, getSavedSpatialResults } from "../services/api";
import { useAnnotations } from "../hooks/useAnnotations";
import { LayerCategoryFilter } from "../molecules/LayerCategoryFilter";
import { LayerItem } from "../molecules/LayerItem";
import { AnnotationSidebarForm } from "../molecules/AnnotationSidebarForm";
import { calculateLineLengthMeters, calculateGeodesicAreaMeters } from "../lib/geoJson";
import { isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { FLIGHT_PHOTOS_LAYER_ID } from "../config/mapConfig";

/** Reads the persisted visibility map (shared with ortho toggles). */
const readSavedModes = (): Record<string, boolean> => {
  try {
    const raw = localStorage.getItem("aap_ortho_display_modes");
    return raw ? (JSON.parse(raw) as Record<string, boolean>) : {};
  } catch {
    return {};
  }
};

/** Reads the persisted opacity map. */
const readSavedOpacities = (): Record<string, number> => {
  try {
    const raw = localStorage.getItem("aap_layer_opacities");
    return raw ? (JSON.parse(raw) as Record<string, number>) : {};
  } catch {
    return {};
  }
};

/**
 * The synthetic Flight Photos layer entry.
 *
 * Built at least once per mount (initial state) and re-checked after every
 * list change: an entry that only existed inside the one-shot load effect
 * stayed missing after hot reload, which left the overlays on screen with no
 * visibility/opacity control in the panel.
 */
const buildFlightPhotosLayer = (): LayerState => {
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

/**
 * Returns the list with the Flight Photos entry guaranteed present.
 * Pure — safe inside setState updaters (a lint-clean substitute for
 * re-injecting from an effect after state was preserved across hot reload).
 */
const ensureFlightPhotos = (list: LayerState[]): LayerState[] =>
  list.some((l) => l.layer_id === FLIGHT_PHOTOS_LAYER_ID)
    ? list
    : [buildFlightPhotosLayer(), ...list];

type LayerManagerProps = {
  map?: L.Map | null;
  onAnnotationsChange?: (annotations: AnnotationFeature[]) => void;
  onLayersChange?: (layers: LayerState[]) => void;
};

export const LayerManager: React.FC<LayerManagerProps> = ({
  map,
  onAnnotationsChange,
  onLayersChange,
}) => {
  const [layers, setLayers] = useState<LayerState[]>(() => [buildFlightPhotosLayer()]);
  const [selectedCategory, setSelectedCategory] = useState("All");
  const [isAddingAnnotation, setIsAddingAnnotation] = useState(false);

  // Hook manajemen anotasi mandiri
  const { annotations, addAnnotation, removeAnnotation } = useAnnotations();

  // The Flight Photos entry is part of the initial state, but state
  // preserved across hot reload can predate it — derive the displayed/sent
  // list so the entry and its controls always exist.
  const effectiveLayers = useMemo(() => ensureFlightPhotos(layers), [layers]);

  // =========================
  // CATEGORY LIST (Kategori Base Maps tidak disertakan)
  // =========================
  const categories = useMemo(() => {
    const uniqueCategories = Array.from(new Set(effectiveLayers.map((layer) => layer.category)));
    return ["All", ...uniqueCategories];
  }, [effectiveLayers]);

  // =========================
  // FILTERED LAYERS
  // =========================
  const visibleLayers = useMemo(() => {
    if (selectedCategory === "All") {
      return effectiveLayers;
    }
    return effectiveLayers.filter((layer) => layer.category === selectedCategory);
  }, [effectiveLayers, selectedCategory]);

  // =========================
  // SEND ANNOTATIONS & LAYERS TO PARENT
  // =========================
  useEffect(() => {
    onAnnotationsChange?.(annotations);
  }, [annotations, onAnnotationsChange]);

  useEffect(() => {
    onLayersChange?.(effectiveLayers);
  }, [effectiveLayers, onLayersChange]);

  useEffect(() => {
    let cancelled = false;

    Promise.all([getInitialMapLayers().catch(() => []), getSavedSpatialResults().catch(() => [])])
      .then(([initialData, spatialData]) => {
        if (!cancelled) {
          const savedModes = (() => {
            try {
              const raw = localStorage.getItem("aap_ortho_display_modes");
              return raw ? (JSON.parse(raw) as Record<string, boolean>) : {};
            } catch {
              return {};
            }
          })();

          const savedOpacities = (() => {
            try {
              const raw = localStorage.getItem("aap_layer_opacities");
              return raw ? (JSON.parse(raw) as Record<string, number>) : {};
            } catch {
              return {};
            }
          })();

          // Hapus item Base Maps agar tidak tampil di daftar layer sidebar
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

          // Flight Photos stays first so its toggle/opacity controls are
          // visible without scrolling; build fresh to honor any preference
          // the user already changed.
          setLayers([buildFlightPhotosLayer(), ...filtered, ...spatialLayers]);
        }
      })
      .catch((err) => {
        console.error("Gagal memuat initial layers dari backend:", err);
      });

    // Event listener jika ada spatial result baru yang disimpan atau diperbarui
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
            const savedModes = (() => {
              try {
                const raw = localStorage.getItem("aap_ortho_display_modes");
                return raw ? (JSON.parse(raw) as Record<string, boolean>) : {};
              } catch {
                return {};
              }
            })();
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

  // Sinkronisasi dua arah: dengarkan event toggle dari popup di peta
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

          // Perbarui juga preferensi localStorage
          try {
            const raw = localStorage.getItem("aap_ortho_display_modes");
            const map = raw ? (JSON.parse(raw) as Record<string, boolean>) : {};
            map[item.layer_id] = nextVal;
            if (item.source_id) {
              map[item.source_id] = nextVal;
            }
            if (item.layer_name) {
              map[item.layer_name] = nextVal;
            }
            localStorage.setItem("aap_ortho_display_modes", JSON.stringify(map));
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

  // =========================
  // LAYER MANAGEMENT
  // =========================
  const handleToggleVisibility = (layer: LayerState) => {
    setLayers((currentLayers) =>
      ensureFlightPhotos(currentLayers).map((item) => {
        if (item.layer_id === layer.layer_id) {
          const nextVal = !item.is_visible;

          // Simpan sinkronisasi ke localStorage agar persist
          try {
            const raw = localStorage.getItem("aap_ortho_display_modes");
            const map = raw ? (JSON.parse(raw) as Record<string, boolean>) : {};
            map[item.layer_id] = nextVal;
            if (item.source_id) {
              map[item.source_id] = nextVal;
            }
            if (item.layer_name) {
              map[item.layer_name] = nextVal;
            }
            localStorage.setItem("aap_ortho_display_modes", JSON.stringify(map));
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
            const map = raw ? (JSON.parse(raw) as Record<string, number>) : {};
            map[item.layer_id] = value;
            if (item.source_id) {
              map[item.source_id] = value;
            }
            if (item.layer_name) {
              map[item.layer_name] = value;
            }
            localStorage.setItem("aap_layer_opacities", JSON.stringify(map));
          } catch {
            // ignore
          }
          return { ...item, opacity: value };
        }
        return item;
      }),
    );
  };

  const handleCategoryChange = (category: string) => {
    setSelectedCategory(category);
  };

  // =========================
  // ANNOTATION ACTIONS
  // =========================
  const handleSaveAnnotation = async (args: { text: string; geometry: SpatialGeometry }) => {
    await addAnnotation(args);
  };

  const handleDeleteAnnotation = async (id: string) => {
    if (!id) return;
    await removeAnnotation(id);
  };

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

  const annotationFormRef = React.useRef<HTMLDivElement | null>(null);

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%" }}>
      {/* Category Filter Pills */}
      <LayerCategoryFilter
        categories={categories}
        selectedCategory={selectedCategory}
        onSelectCategory={handleCategoryChange}
      />

      {/* Layer List */}
      {visibleLayers.length === 0 ? (
        <NonIdealState
          icon="layers"
          title="Tidak ada layer"
          description="Tidak ada layer dalam kategori ini"
        />
      ) : (
        visibleLayers.map((layer) => (
          <LayerItem
            key={layer.layer_id}
            layer={layer}
            onToggleVisibility={(l) => {
              handleToggleVisibility(l);
            }}
            onOpacityChange={(l, value) => {
              handleSetOpacity(l, value);
            }}
          />
        ))
      )}

      {/* Anotasi Spasial Section */}
      <div
        style={{
          marginTop: "20px",
          paddingTop: "16px",
          borderTop: "1px solid var(--color-hairline-silver)",
        }}
      >
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
            marginBottom: "12px",
          }}
        >
          <div>
            <div
              style={{
                fontSize: "13px",
                fontWeight: 700,
                color: "var(--color-ink)",
              }}
            >
              Anotasi Spasial
            </div>
            <div style={{ fontSize: "11px", color: "var(--color-slate)" }}>
              {annotations.length} catatan inspeksi di peta
            </div>
          </div>
          <Button
            small
            intent={isAddingAnnotation ? "none" : "primary"}
            icon={isAddingAnnotation ? "cross" : "plus"}
            text={isAddingAnnotation ? "Tutup Form" : "Tambah Anotasi"}
            onClick={() => {
              const next = !isAddingAnnotation;
              setIsAddingAnnotation(next);
              if (next) {
                setTimeout(() => {
                  annotationFormRef.current?.scrollIntoView({
                    behavior: "smooth",
                    block: "nearest",
                  });
                }, 80);
              }
            }}
            style={{ borderRadius: "8px", fontWeight: 600 }}
          />
        </div>

        {/* Formulir Pembuatan Anotasi di Sidebar (Bukan Modal/Popup, Peta Tidak Terhalang) */}
        {isAddingAnnotation && (
          <div
            ref={annotationFormRef}
            style={{
              marginBottom: "16px",
              padding: "12px",
              background: "var(--color-gallery-white, #ffffff)",
              borderRadius: "14px",
              boxShadow: "var(--shadow-subtle)",
              border: "1px solid #e2e8f0",
            }}
          >
            <AnnotationSidebarForm
              map={map || null}
              onSaveAnnotation={async (data) => {
                await handleSaveAnnotation(data);
                setIsAddingAnnotation(false);
              }}
              onCancel={() => setIsAddingAnnotation(false)}
            />
          </div>
        )}

        {/* Daftar Anotasi */}
        {annotations.length > 0 && (
          <div style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
            {annotations.map((annotation) => {
              const { geometry, annotation_id, id, text } = annotation;
              const targetId = id || annotation_id || "";
              let metaText = "";

              if (geometry.type === "Point") {
                const [lng, lat] = geometry.coordinates;
                metaText = `📍 ${lat.toFixed(6)}, ${lng.toFixed(6)}`;
              } else if (geometry.type === "LineString") {
                const len = calculateLineLengthMeters(geometry.coordinates);
                const lenStr =
                  len >= 1000 ? `${(len / 1000).toFixed(2)} km` : `${len.toFixed(0)} m`;
                metaText = `📏 ${geometry.coordinates.length} titik • ${lenStr}`;
              } else if (geometry.type === "Polygon") {
                const ring = (geometry.coordinates[0] || []) as [number, number][];
                const area = calculateGeodesicAreaMeters(ring);
                const areaStr =
                  area >= 10000 ? `${(area / 10000).toFixed(2)} ha` : `${area.toFixed(0)} m²`;
                metaText = `⬡ ${ring.length} sudut • ${areaStr}`;
              }

              return (
                <div
                  key={targetId}
                  style={{
                    padding: "12px",
                    background: "var(--color-gallery-white)",
                    borderRadius: "12px",
                    boxShadow: "var(--shadow-subtle)",
                    border: "1px solid #e2e8f0",
                  }}
                >
                  <div
                    style={{
                      display: "flex",
                      alignItems: "flex-start",
                      justifyContent: "space-between",
                      gap: "8px",
                    }}
                  >
                    <div>
                      <div
                        style={{
                          display: "flex",
                          alignItems: "center",
                          gap: "6px",
                          marginBottom: "4px",
                        }}
                      >
                        <Tag
                          minimal
                          round
                          style={{
                            fontSize: "10px",
                            background: "rgba(245, 158, 11, 0.12)",
                            color: "#b45309",
                            fontWeight: 600,
                          }}
                        >
                          {geometry.type === "Point"
                            ? "Titik"
                            : geometry.type === "LineString"
                              ? "Garis"
                              : "Poligon"}
                        </Tag>
                        <div
                          style={{
                            fontWeight: 600,
                            fontSize: "13px",
                            color: "var(--color-ink)",
                          }}
                        >
                          {text}
                        </div>
                      </div>

                      <div
                        style={{
                          fontSize: "11px",
                          color: "var(--color-slate)",
                          fontFamily: "monospace",
                        }}
                      >
                        {metaText}
                      </div>
                    </div>

                    <div style={{ display: "flex", gap: "4px" }}>
                      {map && (
                        <Tooltip content="Fokuskan peta ke anotasi ini" placement="top">
                          <Button
                            minimal
                            small
                            icon="locate"
                            onClick={() => handleFocusAnnotation(annotation)}
                          />
                        </Tooltip>
                      )}
                      <Tooltip content="Hapus anotasi" placement="top">
                        <Button
                          minimal
                          small
                          intent="danger"
                          icon="trash"
                          onClick={() => void handleDeleteAnnotation(targetId)}
                        />
                      </Tooltip>
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
};
