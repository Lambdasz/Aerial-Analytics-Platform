import React, { useMemo } from "react";
import { Button, Card, Elevation, Popover, Position, Divider, Tag } from "@blueprintjs/core";
import { LayerPayload } from "../types/map";
import "./MapLegend.css";

interface MapLegendProps {
  layers: LayerPayload[];
}

export const MapLegend: React.FC<MapLegendProps> = ({ layers }) => {
  // Only show legend items for non-image overlay layers
  const legendItems = layers.filter((layer) => layer.display_preference.type !== "image_overlay");

  // Group by sector
  const groupedLayers = useMemo(() => {
    const groups: Record<string, typeof legendItems> = {};
    legendItems.forEach((layer) => {
      const parts = layer.layer_name.split(" - ");
      const sector = parts.length > 1 ? parts[1] : "Umum";
      const name = parts[0];
      if (!groups[sector]) groups[sector] = [];
      groups[sector].push({ ...layer, layer_name: name });
    });
    return groups;
  }, [legendItems]);

  if (legendItems.length === 0) {
    return null;
  }

  const legendContent = (
    <Card className="map-legend-popup-card" elevation={Elevation.ZERO}>
      <h5 className="bp5-heading" style={{ marginBottom: "15px" }}>
        Legenda per Sektor
      </h5>

      {Object.entries(groupedLayers).map(([sector, items], index, array) => (
        <div key={sector} className="map-legend-sector-group">
          <Tag minimal intent="primary" round style={{ marginBottom: "8px" }}>
            Sektor: {sector}
          </Tag>
          <ul className="map-legend-list">
            {items.map((layer) => (
              <li key={layer.layer_id} className="map-legend-item">
                <span
                  className="map-legend-color-box"
                  style={{
                    backgroundColor:
                      layer.display_preference.type === "point"
                        ? layer.display_preference.color
                        : "#cccccc",
                  }}
                />
                <span className="map-legend-label">{layer.layer_name}</span>
              </li>
            ))}
          </ul>
          {index < array.length - 1 && <Divider style={{ margin: "10px 0" }} />}
        </div>
      ))}
    </Card>
  );

  return (
    <div className="map-legend-container">
      <Popover
        content={legendContent}
        position={Position.TOP_LEFT}
        minimal
        modifiers={{ offset: { options: { offset: [0, 10] } } }}
      >
        <Button
          icon="properties"
          text="Legenda"
          intent="primary"
          large
          className="map-legend-button"
        />
      </Popover>
    </div>
  );
};
