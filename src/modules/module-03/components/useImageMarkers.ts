import { useState, useEffect } from "react";
import type { DroneImageMetadata } from "../types/marker";
import { getProjectImageMarkers } from "../api";

/**
 * Hook penyedia marker foto drone proyek dari backend Rust secara deterministik.
 * Menggantikan generator acak lokal dengan data georeferensi orthomosaic Kariangau.
 */
export const useImageMarkers = (count: number = 200) => {
  const [markers, setMarkers] = useState<ReadonlyArray<DroneImageMetadata>>([]);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;

    getProjectImageMarkers()
      .then((data) => {
        if (cancelled) return;
        setMarkers(data.slice(0, count));
        setIsLoading(false);
      })
      .catch((err) => {
        if (cancelled) return;
        console.error("Gagal memuat project image markers dari backend:", err);
        setIsLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [count]);

  return { markers, isLoading };
};
