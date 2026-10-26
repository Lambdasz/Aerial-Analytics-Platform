import { invoke } from "@tauri-apps/api/core";
import type { LayerPayload } from "../types/map";
import type { SpatialResultSummary } from "../types/spatialResult";

const spatialResultModules = import.meta.glob("../../../../map_storage/spatial_results/*.json", {
  eager: true,
  import: "default",
});

const aoiModules = import.meta.glob("../../../../map_storage/aois/*.json", {
  eager: true,
  import: "default",
});

const imageModules = import.meta.glob("../../../../map_storage/images/**/meta.json", {
  eager: true,
  import: "default",
});

function handleBrowserFallback<T>(cmd: string, args?: Record<string, unknown>): T {
  if (cmd === "get_saved_spatial_results") {
    const results = Object.values(spatialResultModules);
    return results as unknown as T;
  }

  if (cmd === "get_saved_spatial_results_summary") {
    const rawResults = Object.values(spatialResultModules) as LayerPayload[];
    const summaries: SpatialResultSummary[] = rawResults.map((layer) => {
      let geomType = "Mixed";
      let displayColor = "#2ECC71";
      let icon: string | null = null;
      let thumbnail: string | null = null;

      if ("color" in layer.display_preference) {
        displayColor = layer.display_preference.color;
      }
      if ("icon" in layer.display_preference) {
        icon = layer.display_preference.icon;
      }
      if (layer.display_preference.type === "polygon") {
        geomType = "Polygon";
      } else if (layer.display_preference.type === "point") {
        geomType = "Point";
      } else if (layer.display_preference.type === "image_overlay") {
        geomType = "ImageOverlay";
        thumbnail = layer.display_preference.image_url;
      }

      if (!thumbnail && layer.spatial_results.length > 0) {
        for (const feat of layer.spatial_results) {
          if (feat.image_url) {
            thumbnail = feat.image_url;
            break;
          }
        }
      }

      return {
        id: layer.layer_id,
        layer_name: layer.layer_name,
        feature_count: layer.spatial_results.length,
        geom_type: geomType,
        display_color: displayColor,
        icon,
        thumbnail_url: thumbnail,
        format_type: "flat",
        is_corrupted: false,
        updated_at: layer.updated_at,
        has_manual_edits: false,
        concurrency_token: layer.updated_at || "mock_token",
      };
    });
    return summaries as unknown as T;
  }

  if (cmd === "get_saved_spatial_result") {
    const rawResults = Object.values(spatialResultModules) as LayerPayload[];
    const targetId = args?.layerId as string;
    const found = rawResults.find((r) => r.layer_id === targetId);
    if (!found) {
      throw new Error(`Spatial result '${targetId}' tidak ditemukan`);
    }
    return found as unknown as T;
  }

  if (cmd === "count_spatial_result_dependents") {
    return {
      id: args?.id,
      deleted_from_disk: true,
      layers_removed: 1,
      feature_count: 1,
    } as unknown as T;
  }

  if (cmd === "get_saved_aois") {
    const aois = Object.values(aoiModules);
    return aois as unknown as T;
  }

  if (cmd === "get_saved_images") {
    const images = Object.values(imageModules);
    return images as unknown as T;
  }

  if (cmd === "list_ortho_images") {
    const images = Object.values(imageModules);
    return images as unknown as T;
  }

  if (cmd === "get_ortho_cascade_summary") {
    return {
      id: args?.id,
      display_layer_deleted: true,
      annotations_deleted_count: 0,
      folder_deleted: true,
    } as unknown as T;
  }

  if (cmd === "get_saved_annotations") {
    return [] as unknown as T;
  }

  if (cmd === "get_initial_map_layers") {
    return [] as unknown as T;
  }

  if (cmd === "get_project_image_markers") {
    return [] as unknown as T;
  }

  throw new Error(`[browser mock] Command '${cmd}' not supported in browser-only mode`);
}

export async function invokeMap<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (typeof window !== "undefined" && !("__TAURI_INTERNALS__" in window)) {
    return Promise.resolve(handleBrowserFallback<T>(cmd, args));
  }

  try {
    return await invoke<T>(cmd, args);
  } catch (err) {
    console.error(`[map] ${cmd} failed:`, err);
    throw err;
  }
}
