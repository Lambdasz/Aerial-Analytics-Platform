import { useState, useEffect } from "react";
import { getSavedAois } from "../services/api";
import type { AoiFeature } from "../types/aoi";

export function useSavedAois() {
  const [savedAois, setSavedAois] = useState<AoiFeature[]>([]);
  const [isLoadingAois, setIsLoadingAois] = useState(true);

  useEffect(() => {
    let cancelled = false;

    getSavedAois()
      .then((data) => {
        if (!cancelled) {
          const list = Array.isArray(data)
            ? data
            : ((data as unknown as { aois?: AoiFeature[] })?.aois ?? []);
          setSavedAois(list);
        }
      })
      .catch((err) => {
        console.warn("Gagal memuat saved AOIs:", err);
      })
      .finally(() => {
        if (!cancelled) setIsLoadingAois(false);
      });

    const handleAoiSaved = (event: Event) => {
      const customEvent = event as CustomEvent<AoiFeature>;
      if (customEvent.detail) {
        setSavedAois((prev) => {
          const id = customEvent.detail.properties.aoi_id;
          const exists = prev.some((a) => a.properties.aoi_id === id);
          if (exists) {
            return prev.map((a) => (a.properties.aoi_id === id ? customEvent.detail : a));
          }
          return [customEvent.detail, ...prev];
        });
      }
    };

    const handleAoiDeleted = (event: Event) => {
      const customEvent = event as CustomEvent<{ aoiId: string }>;
      if (customEvent.detail?.aoiId) {
        setSavedAois((prev) =>
          prev.filter((a) => a.properties.aoi_id !== customEvent.detail.aoiId),
        );
      }
    };

    window.addEventListener("aoi:saved", handleAoiSaved);
    window.addEventListener("aoi:deleted", handleAoiDeleted);

    return () => {
      cancelled = true;
      window.removeEventListener("aoi:saved", handleAoiSaved);
      window.removeEventListener("aoi:deleted", handleAoiDeleted);
    };
  }, []);

  return { savedAois, setSavedAois, isLoadingAois };
}
