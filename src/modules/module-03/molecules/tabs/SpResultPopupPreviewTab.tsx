import React from "react";
import { Icon } from "@blueprintjs/core";
import type { FieldDef } from "../../types/spatialResult";
import type { LayerPayload, SpatialResult } from "../../types/map";
import { PopupPreviewCard } from "../PopupPreviewCard";

export interface SpResultPopupPreviewTabProps {
  layerName: string;
  displayColor: string;
  previewUrl: string | null;
  imageAction: "keep" | "delete" | "replace";
  layerDetail: LayerPayload | null;
  selectedFeatureIndex: number;
  onSelectIndex: (index: number) => void;
  selectedFeature: SpatialResult | null;
  fields: FieldDef[];
  draftSets: Map<number, Record<string, unknown>>;
}

export const SpResultPopupPreviewTab: React.FC<SpResultPopupPreviewTabProps> = ({
  layerName,
  displayColor,
  previewUrl,
  imageAction,
  layerDetail,
  selectedFeatureIndex,
  onSelectIndex,
  selectedFeature,
  fields,
  draftSets,
}) => {
  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        gap: "16px",
        padding: "8px 0 16px 0",
      }}
    >
      {layerDetail && layerDetail.spatial_results.length > 1 && (
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: "10px",
            background: "#f8fafc",
            padding: "6px 14px",
            borderRadius: "999px",
            border: "1px solid #f1f5f9",
          }}
        >
          <button
            type="button"
            disabled={selectedFeatureIndex === 0}
            onClick={() => onSelectIndex(Math.max(0, selectedFeatureIndex - 1))}
            style={{
              background: "none",
              border: "none",
              cursor: selectedFeatureIndex === 0 ? "not-allowed" : "pointer",
              opacity: selectedFeatureIndex === 0 ? 0.3 : 1,
              padding: "2px 4px",
              display: "flex",
              alignItems: "center",
              outline: "none",
            }}
          >
            <Icon icon="chevron-left" size={14} color="#0f172a" />
          </button>
          <span style={{ fontSize: "11px", fontWeight: 600, color: "#334155" }}>
            Fitur #{selectedFeatureIndex + 1} dari {layerDetail.spatial_results.length}
          </span>
          <button
            type="button"
            disabled={selectedFeatureIndex >= layerDetail.spatial_results.length - 1}
            onClick={() =>
              onSelectIndex(
                Math.min(layerDetail.spatial_results.length - 1, selectedFeatureIndex + 1),
              )
            }
            style={{
              background: "none",
              border: "none",
              cursor:
                selectedFeatureIndex >= layerDetail.spatial_results.length - 1
                  ? "not-allowed"
                  : "pointer",
              opacity: selectedFeatureIndex >= layerDetail.spatial_results.length - 1 ? 0.3 : 1,
              padding: "2px 4px",
              display: "flex",
              alignItems: "center",
              outline: "none",
            }}
          >
            <Icon icon="chevron-right" size={14} color="#0f172a" />
          </button>
        </div>
      )}

      <div style={{ width: "100%", maxWidth: "340px" }}>
        <PopupPreviewCard
          layerName={layerName}
          displayColor={displayColor}
          layerImageUrl={imageAction === "delete" ? null : previewUrl}
          feature={selectedFeature}
          fields={fields}
          draftValues={draftSets.get(selectedFeatureIndex)}
        />
      </div>
    </div>
  );
};
