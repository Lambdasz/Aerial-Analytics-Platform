import React from "react";
import type { FieldDef } from "../../types/spatialResult";
import type { LayerPayload } from "../../types/map";
import { FeatureValuesEditor } from "../FeatureValuesEditor";

export interface SpResultValuesTabProps {
  layerDetail: LayerPayload | null;
  fields: FieldDef[];
  selectedIndex: number;
  onSelectIndex: (index: number) => void;
  draftSets: Map<number, Record<string, unknown>>;
  onUpdateValue: (featureIndex: number, key: string, value: unknown) => void;
}

export const SpResultValuesTab: React.FC<SpResultValuesTabProps> = ({
  layerDetail,
  fields,
  selectedIndex,
  onSelectIndex,
  draftSets,
  onUpdateValue,
}) => {
  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        gap: "20px",
      }}
    >
      {layerDetail && layerDetail.spatial_results.length > 0 ? (
        <FeatureValuesEditor
          features={layerDetail.spatial_results}
          fields={fields}
          selectedIndex={selectedIndex}
          onSelectIndex={onSelectIndex}
          draftSets={draftSets}
          onUpdateValue={onUpdateValue}
        />
      ) : (
        <div
          style={{
            textAlign: "center",
            padding: "40px 20px",
            color: "#64748b",
            fontSize: "13px",
            background: "#f8fafc",
            borderRadius: "12px",
          }}
        >
          Tidak ada fitur spasial untuk diedit.
        </div>
      )}
    </div>
  );
};
