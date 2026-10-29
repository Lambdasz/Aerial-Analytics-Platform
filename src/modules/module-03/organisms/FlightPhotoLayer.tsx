import React, { useEffect, useMemo, useState } from "react";
import { ImageOverlay, Popup } from "react-leaflet";
import type { LayerState } from "../types/layer";
import type { DroneImageMetadata } from "../types/marker";
import { estimatePhotoBounds } from "../lib/photoFootprint";
import { loadImageThumbnail } from "../../../lib/imagePreview";
import { FLIGHT_PHOTOS_LAYER_ID } from "../config/mapConfig";
import { useImageSync } from "../store/ImageSyncContext";
import { ImagePopupCard } from "./ImagePopupCard";

/** Concurrent thumbnail fetches — 817 frames must not stampede the backend. */
const THUMB_WORKERS = 6;

/** 1×1 transparent GIF — footprint clickable before its thumbnail exists. */
const PLACEHOLDER_URL =
  "data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7";

export interface FlightPhotoLayerProps {
  markers: ReadonlyArray<DroneImageMetadata>;
  layers: LayerState[];
  /** image_ids individually hidden from the Layers panel per-photo list. */
  hiddenPhotoIds?: ReadonlySet<string>;
}

/**
 * Renders session flight photos as estimated-footprint image overlays
 * (one rectangle per frame) instead of point markers.
 *
 * Frames overlap heavily — that is expected for mapping surveys, and the
 * footprint is the camera's true ground coverage, so analysis layers placed
 * with the same georeference line up correctly. Thumbnails arrive
 * asynchronously through the session-cached `loadImageThumbnail`; every frame
 * renders immediately (transparent placeholder) and swaps in its image when
 * ready. Individually hidden frames are skipped.
 *
 * Visibility/opacity follow the synthetic `flight_photos` entry the Layer
 * Manager publishes; before that state arrives the layer defaults to on.
 */
export const FlightPhotoLayer: React.FC<FlightPhotoLayerProps> = ({
  markers,
  layers,
  hiddenPhotoIds,
}) => {
  const { setActiveImage } = useImageSync();
  const [thumbs, setThumbs] = useState<Record<string, string>>({});

  const layerState = layers.find((l) => l.layer_id === FLIGHT_PHOTOS_LAYER_ID);
  const visible = layerState ? layerState.is_visible : true;
  const opacity = layerState?.opacity ?? 0.5;

  const withPath = useMemo(
    () => markers.filter((m) => m.file_path && !hiddenPhotoIds?.has(m.image_id)),
    [markers, hiddenPhotoIds],
  );

  useEffect(() => {
    if (!visible || withPath.length === 0) return;
    let cancelled = false;
    let next = 0;

    const worker = async () => {
      while (!cancelled && next < withPath.length) {
        const image = withPath[next];
        next += 1;
        try {
          const url = await loadImageThumbnail(image.file_path as string, image.file_size);
          if (cancelled) return;
          setThumbs((prev) => (prev[image.image_id] ? prev : { ...prev, [image.image_id]: url }));
        } catch {
          // Frame stays uncovered; the rest of the layer is unaffected.
        }
      }
    };

    void Promise.all(Array.from({ length: THUMB_WORKERS }, worker));
    return () => {
      cancelled = true;
    };
  }, [visible, withPath]);

  if (!visible) return null;

  return (
    <>
      {markers.map((image) => {
        if (hiddenPhotoIds?.has(image.image_id)) return null;
        const bounds = estimatePhotoBounds(image);
        const url = thumbs[image.image_id] ?? PLACEHOLDER_URL;
        if (!bounds) return null;
        return (
          <ImageOverlay
            key={image.image_id}
            url={url}
            bounds={bounds}
            opacity={opacity}
            interactive
            eventHandlers={{
              click: () => setActiveImage(image.image_id, "map"),
            }}
          >
            <Popup>
              <ImagePopupCard image={image} />
            </Popup>
          </ImageOverlay>
        );
      })}
    </>
  );
};
