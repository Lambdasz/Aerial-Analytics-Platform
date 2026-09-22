import { useState, useEffect } from "react";
import type { DroneImageMetadata } from "../types/marker";

// Generate dummy data around the center of Balikpapan (from MapCanvas: -1.247000, 116.893000)
const generateDummyMarkers = (count: number): DroneImageMetadata[] => {
  const baseLat = -1.247;
  const baseLng = 116.893;

  return Array.from({ length: count }).map((_, i) => ({
    image_id: `IMG_${String(i).padStart(4, "0")}`,
    file_name: `DJI_${String(i).padStart(4, "0")}.JPG`,
    latitude: baseLat + (Math.random() - 0.5) * 0.01,
    longitude: baseLng + (Math.random() - 0.5) * 0.01,
    altitude_agl: 100 + Math.random() * 50, // 100-150m AGL
    heading_deg: Math.floor(Math.random() * 360),
    thumbnail_url: `https://picsum.photos/seed/${i}/300/200`, // random image placeholder
  }));
};

export const useImageMarkers = (count: number = 200) => {
  const [markers, setMarkers] = useState<ReadonlyArray<DroneImageMetadata>>([]);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    // Simulate fetching from Rust backend
    const timer = setTimeout(() => {
      setMarkers(generateDummyMarkers(count));
      setIsLoading(false);
    }, 500);

    return () => clearTimeout(timer);
  }, [count]);

  return { markers, isLoading };
};
