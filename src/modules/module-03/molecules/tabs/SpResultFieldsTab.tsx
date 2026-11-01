import React from "react";
import type { FieldDef } from "../../types/spatialResult";
import { FieldManagerTable } from "../FieldManagerTable";

export interface SpResultFieldsTabProps {
  fields: FieldDef[];
  onChange: (fields: FieldDef[]) => void;
  onDeleteCustomField: (field: FieldDef) => void;
  onAddField: (field: FieldDef, initialVal?: unknown) => void;
}

export const SpResultFieldsTab: React.FC<SpResultFieldsTabProps> = ({
  fields,
  onChange,
  onDeleteCustomField,
  onAddField,
}) => {
  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        gap: "16px",
      }}
    >
      <FieldManagerTable
        fields={fields}
        onChange={onChange}
        onDeleteCustomField={onDeleteCustomField}
        onAddField={onAddField}
      />
    </div>
  );
};
