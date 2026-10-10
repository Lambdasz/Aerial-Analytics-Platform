import React, { useMemo } from "react";
import { Icon } from "@blueprintjs/core";
import { LayerPayload } from "../types/map";
import "./MapLegend.css";

interface MapLegendProps {
  layers: LayerPayload[];
}

export const MapLegend: React.FC<MapLegendProps> = ({ layers }) => {
  const [isOpen, setIsOpen] = React.useState(false);

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
    <div className={`map-legend-popup-card ${isOpen ? "open" : ""}`}>
      <div
        style={{
          fontSize: "16px",
          fontWeight: 700,
          color: "var(--color-ink)",
          marginBottom: "20px",
          letterSpacing: "-0.3px",
        }}
      >
        Legenda Peta
      </div>

      <div className="map-legend-scroll-area">
        {Object.entries(groupedLayers).map(([sector, items], index, array) => (
          <div key={sector} className="map-legend-sector-group">
            <div
              style={{
                fontSize: "11px",
                fontWeight: 600,
                color: "var(--color-slate)",
                textTransform: "uppercase",
                letterSpacing: "0.5px",
                marginBottom: "10px",
                paddingBottom: "4px",
                borderBottom: "1px solid var(--color-hairline-silver)",
              }}
            >
              Sektor {sector}
            </div>
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
            {index < array.length - 1 && (
              <div style={{ height: "16px" }} /> /* Spacer between sectors instead of lines */
            )}
          </div>
        ))}
      </div>
    </div>
  );

  return (
    <div className="map-legend-container">
      {isOpen && legendContent}

      <div
        role="button"
        tabIndex={0}
        className="map-legend-button apple-button-hover"
        onClick={() => setIsOpen(!isOpen)}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            setIsOpen(!isOpen);
          }
        }}
      >
        <Icon icon="properties" size={16} />
        <span>Legenda</span>
      </div>
    </div>
  );
};
