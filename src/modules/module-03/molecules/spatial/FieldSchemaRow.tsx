import React from "react";
import { Button, ButtonGroup, Tooltip } from "@blueprintjs/core";
import type { FieldDef } from "../../types/spatialResult";
import { validateFieldLabel } from "../../lib/fieldValidation";

export interface FieldSchemaRowProps {
  field: FieldDef;
  index: number;
  isFirst: boolean;
  isLast: boolean;
  onMove: (index: number, direction: -1 | 1) => void;
  onToggleVisible: (index: number) => void;
  onUpdateLabel: (index: number, label: string) => void;
  onUpdateUnit: (index: number, unit: string) => void;
  onRequestDelete: (field: FieldDef) => void;
}

export const FieldSchemaRow: React.FC<FieldSchemaRowProps> = ({
  field,
  index,
  isFirst,
  isLast,
  onMove,
  onToggleVisible,
  onUpdateLabel,
  onUpdateUnit,
  onRequestDelete,
}) => {
  const isCustom = Boolean(field.custom);
  const labelErr = validateFieldLabel(field.label);

  return (
    <tr
      style={{
        backgroundColor: field.visible ? "inherit" : "#fbfbfc",
        opacity: field.visible ? 1 : 0.65,
        borderBottom: "1px solid #f1f5f9",
      }}
    >
      {/* Urutan */}
      <td style={{ textAlign: "center", padding: "6px 4px" }}>
        <ButtonGroup minimal>
          <Button icon="chevron-up" small disabled={isFirst} onClick={() => onMove(index, -1)} />
          <Button icon="chevron-down" small disabled={isLast} onClick={() => onMove(index, 1)} />
        </ButtonGroup>
      </td>

      {/* Toggle Tampil */}
      <td style={{ textAlign: "center", padding: "6px 4px" }}>
        <Button
          icon={field.visible ? "eye-open" : "eye-off"}
          minimal
          small
          style={{ color: field.visible ? "#0f172a" : "#94a3b8" }}
          onClick={() => onToggleVisible(index)}
          title={field.visible ? "Sembunyikan dari popup" : "Tampilkan di popup"}
        />
      </td>

      {/* Label Inline */}
      <td style={{ padding: "6px 4px" }}>
        <input
          type="text"
          value={field.label}
          onChange={(e) => onUpdateLabel(index, e.target.value)}
          placeholder="Label field"
          style={{
            width: "100%",
            padding: "6px 10px",
            fontSize: "12px",
            background: "#f8fafc",
            border: labelErr ? "1px solid #ef4444" : "none",
            borderRadius: "8px",
            color: "#0f172a",
            outline: "none",
            boxSizing: "border-box",
          }}
        />
        <div style={{ fontSize: "10px", color: "#94a3b8", marginTop: "2px" }}>
          Key: <code>{field.key}</code>
        </div>
      </td>

      {/* Unit Inline */}
      <td style={{ padding: "6px 4px" }}>
        <input
          type="text"
          value={field.unit || ""}
          onChange={(e) => onUpdateUnit(index, e.target.value)}
          placeholder="m, m²"
          style={{
            width: "100%",
            padding: "6px 8px",
            fontSize: "12px",
            background: "#f8fafc",
            border: "none",
            borderRadius: "8px",
            color: "#0f172a",
            outline: "none",
            boxSizing: "border-box",
          }}
        />
      </td>

      {/* Kind */}
      <td style={{ textAlign: "center", padding: "6px 4px" }}>
        <span
          style={{
            display: "inline-block",
            padding: "2px 8px",
            borderRadius: "6px",
            fontSize: "10px",
            fontWeight: 600,
            background: "#f1f5f9",
            color: "#475569",
          }}
        >
          {field.kind === "number" ? "Angka" : field.kind === "boolean" ? "Bool" : "Teks"}
        </span>
      </td>

      {/* Aksi Hapus */}
      <td style={{ textAlign: "center", padding: "6px 4px" }}>
        {isCustom ? (
          <Button
            icon="trash"
            minimal
            small
            style={{ color: "#64748b" }}
            title="Hapus field custom"
            onClick={() => onRequestDelete(field)}
          />
        ) : (
          <Tooltip content="Field bawaan plugin tidak dapat dihapus, hanya dapat disembunyikan">
            <Button icon="trash" minimal small disabled style={{ opacity: 0.3 }} />
          </Tooltip>
        )}
      </td>
    </tr>
  );
};
