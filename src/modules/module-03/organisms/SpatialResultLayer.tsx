import React from "react";
import { useSpatialResultLayers } from "../hooks/useSpatialResultLayers";
import { MIN_POLYGON_FEATURE_ZOOM, MIN_POINT_FEATURE_ZOOM } from "../config/mapConfig";
import type { LayerPayload } from "../types/map";
import type { LayerState } from "../types/layer";

export interface SpatialResultLayerProps {
  onLayersChange?: (layers: LayerPayload[]) => void;
  minPolygonZoom?: number;
  minPointZoom?: number;
  layers?: LayerState[];
}

export const SpatialResultLayer: React.FC<SpatialResultLayerProps> = ({
  onLayersChange,
  minPolygonZoom = MIN_POLYGON_FEATURE_ZOOM,
  minPointZoom = MIN_POINT_FEATURE_ZOOM,
  layers = [],
}) => {
  const { activeLayers } = useSpatialResultLayers(minPolygonZoom, minPointZoom, layers);

  React.useEffect(() => {
    onLayersChange?.(activeLayers);
  }, [activeLayers, onLayersChange]);

  return null;
};
