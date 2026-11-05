import { useState, useEffect } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import type { DroneImageMetadata } from "../types/marker";

/** Row shape of `get_images_by_session` used for marker construction. */
interface SessionImageRow {
  id: string;
  filePath: string;
  locationLat: number | null;
  locationLon: number | null;
  gpsAltitudeM: number | null;
  headingDeg: number | null;
  fileSize: number | null;
  width: number | null;
  height: number | null;
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
      return raw.includes("-") ? raw : null;
    }
  } catch {
    return null;
  }
}

/**
 * Geotagged images of the newest session of the saved (or newest) project,
 * shaped as map markers. Silent empty result without Tauri / data.
 */
export async function fetchSessionImageMarkers(): Promise<DroneImageMetadata[]> {
  if (!isTauri()) return [];
  let projectId = readSavedProjectId();
  if (!projectId) {
    const projects = await invoke<Array<{ id: string }>>("list_projects");
    if (!projects.length) return [];
    projectId = projects[projects.length - 1].id;
  }
  const sessions = await invoke<SessionRow[]>("get_sessions_by_project", { projectId });
  if (!sessions.length) return [];
  const newest = [...sessions].sort((a, b) => b.createdAt.localeCompare(a.createdAt))[0];
  const images = await invoke<SessionImageRow[]>("get_images_by_session", {
    sessionId: newest.id,
  });
  return images
    .filter(
      (img): img is SessionImageRow & { locationLat: number; locationLon: number } =>
        typeof img.locationLat === "number" && typeof img.locationLon === "number",
    )
    .map((img) => ({
      image_id: img.id,
      file_name: img.filePath.split(/[\\/]/).pop() ?? img.id,
      latitude: img.locationLat,
      longitude: img.locationLon,
      altitude_agl: img.gpsAltitudeM ?? 0,
      // Heading (EXIF GPSImgDirection, imported by Module 1); 0 when absent.
      heading_deg: img.headingDeg ?? 0,
      thumbnail_url: "",
      file_path: img.filePath,
      file_size: img.fileSize,
      width: img.width,
      height: img.height,
    }));
}

/**
 * Single source of truth for session photo markers (map + gallery).
 * One fetch per mount; consumers share the same array.
 */
export const useSessionImageMarkers = () => {
  const [markers, setMarkers] = useState<ReadonlyArray<DroneImageMetadata>>([]);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    fetchSessionImageMarkers()
      .then((data) => {
        if (!cancelled) setMarkers(data);
      })
      .catch((err) => {
        console.warn("Session image markers skipped:", err);
      })
      .finally(() => {
        if (!cancelled) setIsLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return { markers, isLoading };
};
