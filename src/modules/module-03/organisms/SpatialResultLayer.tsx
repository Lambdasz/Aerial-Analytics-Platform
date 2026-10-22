import React from "react";
import { useSpatialResultLayers } from "../hooks/useSpatialResultLayers";
import type { LayerPayload } from "../types/map";

export interface SpatialResultLayerProps {
  onLayersChange: (layers: LayerPayload[]) => void;
}

export const SpatialResultLayer: React.FC<SpatialResultLayerProps> = ({ onLayersChange }) => {
  const { activeLayers } = useSpatialResultLayers();

  React.useEffect(() => {
    onLayersChange(activeLayers);
  }, [activeLayers, onLayersChange]);

  return null;
};
