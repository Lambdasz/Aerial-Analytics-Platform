import React, { useState } from "react";
import { Button } from "@blueprintjs/core";
import type { FieldDef } from "../types/spatialResult";
import { FieldSchemaRow } from "./spatial/FieldSchemaRow";
import { AddCustomFieldDialog } from "./spatial/AddCustomFieldDialog";
import { DeleteCustomFieldDialog } from "./spatial/DeleteCustomFieldDialog";

export interface FieldManagerTableProps {
  fields: FieldDef[];
  onChange: (fields: FieldDef[]) => void;
  onDeleteCustomField: (field: FieldDef) => void;
  onAddField: (field: FieldDef, initialValue?: unknown) => void;
}

const TABLE_HEADERS = [
  { label: "Urutan", width: "52px", align: "center" as const },
  { label: "Tampil", width: "40px", align: "center" as const },
  { label: "Label Tampil", width: undefined, align: "left" as const },
  { label: "Satuan", width: "80px", align: "left" as const },
  { label: "Tipe", width: "70px", align: "center" as const },
  { label: "Aksi", width: "40px", align: "center" as const },
];

export const FieldManagerTable: React.FC<FieldManagerTableProps> = ({
  fields,
  onChange,
  onDeleteCustomField,
  onAddField,
}) => {
  const [isAddOpen, setIsAddOpen] = useState(false);
  const [confirmDeleteField, setConfirmDeleteField] = useState<FieldDef | null>(null);

  const moveField = (index: number, direction: -1 | 1) => {
    const target = index + direction;
    if (target < 0 || target >= fields.length) return;
    const copy = [...fields];
    const temp = copy[index];
    copy[index] = copy[target];
    copy[target] = temp;
    onChange(copy);
  };

  const toggleVisible = (index: number) => {
    const copy = [...fields];
    copy[index] = { ...copy[index], visible: !copy[index].visible };
    onChange(copy);
  };

  const updateLabel = (index: number, label: string) => {
    const copy = [...fields];
    copy[index] = { ...copy[index], label };
    onChange(copy);
  };

  const updateUnit = (index: number, unit: string) => {
    const copy = [...fields];
    copy[index] = { ...copy[index], unit: unit ? unit : null };
    onChange(copy);
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          marginBottom: "2px",
        }}
      >
        <div>
          <span style={{ fontWeight: 600, fontSize: "13px", color: "#0f172a" }}>
            Kelola Field & Atribut
          </span>
          <div style={{ fontSize: "11px", color: "#64748b" }}>
            Atur urutan, visibilitas, judul tampil, dan satuan pengukuran pada popup
          </div>
        </div>
        <Button
          icon="plus"
          minimal
          small
          text="Tambah Field"
          onClick={() => setIsAddOpen(true)}
          style={{
            background: "#0f172a",
            color: "#ffffff",
            borderRadius: "8px",
            padding: "5px 12px",
            fontSize: "11px",
            fontWeight: 600,
          }}
        />
      </div>

      <div style={{ maxHeight: "260px", overflowY: "auto", borderRadius: "10px" }}>
        <table style={{ width: "100%", fontSize: "12px", borderCollapse: "collapse" }}>
          <thead>
            <tr style={{ background: "#f8fafc" }}>
              {TABLE_HEADERS.map((th) => (
                <th
                  key={th.label}
                  style={{
                    width: th.width,
                    textAlign: th.align,
                    padding: "8px 6px",
                    color: "#64748b",
                    fontSize: "11px",
                    fontWeight: 600,
                  }}
                >
                  {th.label}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {fields.map((f, idx) => (
              <FieldSchemaRow
                key={f.key}
                field={f}
                index={idx}
                isFirst={idx === 0}
                isLast={idx === fields.length - 1}
                onMove={moveField}
                onToggleVisible={toggleVisible}
                onUpdateLabel={updateLabel}
                onUpdateUnit={updateUnit}
                onRequestDelete={(fieldToDelete) => setConfirmDeleteField(fieldToDelete)}
              />
            ))}
          </tbody>
        </table>
      </div>

      <AddCustomFieldDialog
        isOpen={isAddOpen}
        onClose={() => setIsAddOpen(false)}
        existingFields={fields}
        onAddField={onAddField}
      />
      <DeleteCustomFieldDialog
        field={confirmDeleteField}
        onClose={() => setConfirmDeleteField(null)}
        onConfirm={onDeleteCustomField}
      />
    </div>
  );
};
