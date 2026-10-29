import { useEffect } from "react";
import { useMap } from "react-leaflet";
import * as L from "leaflet";
import { invoke, isTauri } from "@tauri-apps/api/core";

/** Minimal row shape of `get_images_by_session` needed for centering. */
interface GeotaggedImage {
  locationLat: number | null;
  locationLon: number | null;
}

interface SessionRow {
  id: string;
  createdAt: string;
}

const PROJECT_KEY = "m1_last_project_id";

/**
 * Reads the module-01 selected project id (stored as a full Project JSON
 * under `m1_last_project_id`; plain-uuid strings tolerated).
 */
function readSavedProjectId(): string | null {
  try {
    const raw = window.localStorage.getItem(PROJECT_KEY);
    if (!raw) return null;
    try {
      const parsed = JSON.parse(raw) as unknown;
      if (typeof parsed === "string") return parsed;
      if (parsed && typeof parsed === "object" && "id" in parsed) {
        const id = (parsed as { id?: unknown }).id;
        if (typeof id === "string" && id) return id;
      }
      return null;
    } catch {
      // Not JSON — accept a bare uuid-looking value.
      return raw.includes("-") ? raw : null;
    }
  } catch {
    return null;
  }
}

/**
 * Fits the map to the newest session's geotagged images, once per mount.
 * Removes the "find my imagery somewhere on the globe" friction: the default
 * view is a hardcoded sample location, unrelated to the user's project.
 * Silent no-op when there is no project, no session, or no GPS data.
 */
export const AutoCenterOnImages: React.FC = () => {
  const map = useMap();

  useEffect(() => {
    let cancelled = false;

    const center = async () => {
      if (!isTauri()) return;
      try {
        let projectId = readSavedProjectId();
        if (!projectId) {
          const projects = await invoke<Array<{ id: string }>>("list_projects");
          if (!projects.length) return;
          // list_projects is oldest-first; the newest project is the default.
          projectId = projects[projects.length - 1].id;
        }
        const sessions = await invoke<SessionRow[]>("get_sessions_by_project", { projectId });
        if (!sessions.length) return;
        const newest = [...sessions].sort((a, b) => b.createdAt.localeCompare(a.createdAt))[0];
        const images = await invoke<GeotaggedImage[]>("get_images_by_session", {
          sessionId: newest.id,
        });
        const points = images
          .filter(
            (img): img is GeotaggedImage & { locationLat: number; locationLon: number } =>
              typeof img.locationLat === "number" && typeof img.locationLon === "number",
          )
          .map((img) => [img.locationLat, img.locationLon] as [number, number]);
        if (cancelled || points.length === 0) return;
        map.fitBounds(L.latLngBounds(points), {
          padding: [60, 60],
          maxZoom: 17,
          animate: true,
          duration: 1.2,
        });
      } catch (err) {
        console.warn("Auto-center skipped:", err);
      }
    };

    void center();
    return () => {
      cancelled = true;
    };
  }, [map]);

  return null;
};
