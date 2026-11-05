import { useState, useEffect } from "react";
import type { DroneImageMetadata } from "../types/marker";
import { getProjectImageMarkers } from "../services/api";

/**
 * Hook penyedia marker foto drone proyek dari backend Rust.
 * Catatan Arsitektur: Hook ini tidak menjalankan interval atau polling liar di background.
 * Eksekusi bersifat single-shot berdasarkan parameter `count` saat dipanggil secara on-demand.
 */
export const useImageMarkers = (count: number = 200) => {
  const [markers, setMarkers] = useState<ReadonlyArray<DroneImageMetadata>>([]);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;

    // Safety timeout: jika dalam 3 detik backend tidak merespon, hentikan spinner
    const timer = setTimeout(() => {
      if (!cancelled) {
        setIsLoading(false);
      }
    }, 3000);

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
      })
      .finally(() => {
        clearTimeout(timer);
      });

    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [count]);

  return { markers, isLoading };
};
