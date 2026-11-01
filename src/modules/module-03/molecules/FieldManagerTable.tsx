import React, { useState } from "react";
import { Button, ButtonGroup, Dialog, Tooltip } from "@blueprintjs/core";
import type { FieldDef, FieldKind } from "../types/spatialResult";
import {
  validateCustomFieldKey,
  validateCustomFieldsCount,
  validateFieldLabel,
  validateFieldUnit,
} from "../lib/fieldValidation";

export interface FieldManagerTableProps {
  fields: FieldDef[];
  onChange: (fields: FieldDef[]) => void;
  onDeleteCustomField: (field: FieldDef) => void;
  onAddField: (field: FieldDef, initialValue?: unknown) => void;
}

export const FieldManagerTable: React.FC<FieldManagerTableProps> = ({
  fields,
  onChange,
  onDeleteCustomField,
  onAddField,
}) => {
  const [isAddOpen, setIsAddOpen] = useState(false);
  const [newLabel, setNewLabel] = useState("");
  const [newKey, setNewKey] = useState("");
  const [newUnit, setNewUnit] = useState("");
  const [newKind, setNewKind] = useState<FieldKind>("text");
  const [newInitialVal, setNewInitialVal] = useState("");
  const [addError, setAddError] = useState<string | null>(null);

  const [confirmDeleteField, setConfirmDeleteField] = useState<FieldDef | null>(null);

  // Auto-slugify label to snake_case key
  const handleLabelChange = (val: string) => {
    setNewLabel(val);
    const slug = val
      .toLowerCase()
      .trim()
      .replace(/[^a-z0-9_]+/g, "_")
      .replace(/^_+|_+$/g, "")
      .substring(0, 40);
    setNewKey(slug);
    setAddError(null);
  };

  const handleOpenAdd = () => {
    setNewLabel("");
    setNewKey("");
    setNewUnit("");
    setNewKind("text");
    setNewInitialVal("");
    setAddError(null);
    setIsAddOpen(true);
  };

  const handleSaveNewField = () => {
    // 1. Validasi label
    const labelErr = validateFieldLabel(newLabel);
    if (labelErr) {
      setAddError(labelErr);
      return;
    }

    // 2. Validasi key
    const existingKeys = fields.map((f) => f.key);
    const keyErr = validateCustomFieldKey(newKey, existingKeys);
    if (keyErr) {
      setAddError(keyErr);
      return;
    }

    // 3. Validasi unit
    const unitErr = validateFieldUnit(newUnit);
    if (unitErr) {
      setAddError(unitErr);
      return;
    }

    // 4. Validasi kuota custom
    const countErr = validateCustomFieldsCount([
      ...fields,
      {
        key: newKey,
        label: newLabel,
        custom: true,
        visible: true,
      },
    ]);
    if (countErr) {
      setAddError(countErr);
      return;
    }

    let parsedInitial: unknown = undefined;
    if (newInitialVal.trim().length > 0) {
      if (newKind === "number") {
        const num = Number(newInitialVal.trim());
        if (!Number.isFinite(num)) {
          setAddError("Nilai awal harus berupa angka yang valid");
          return;
        }
        parsedInitial = num;
      } else if (newKind === "boolean") {
        parsedInitial = newInitialVal.trim().toLowerCase() === "true";
      } else {
        parsedInitial = newInitialVal.trim();
      }
    }

    const created: FieldDef = {
      key: newKey.trim(),
      label: newLabel.trim(),
      unit: newUnit.trim() ? newUnit.trim() : null,
      visible: true,
      custom: true,
      kind: newKind,
    };

    onAddField(created, parsedInitial);
    setIsAddOpen(false);
  };

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
          onClick={handleOpenAdd}
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

      <div
        style={{
          maxHeight: "260px",
          overflowY: "auto",
          borderRadius: "10px",
        }}
      >
        <table style={{ width: "100%", fontSize: "12px", borderCollapse: "collapse" }}>
          <thead>
            <tr style={{ background: "#f8fafc" }}>
              <th
                style={{
                  width: "52px",
                  textAlign: "center",
                  padding: "8px 6px",
                  color: "#64748b",
                  fontSize: "11px",
                  fontWeight: 600,
                }}
              >
                Urutan
              </th>
              <th
                style={{
                  width: "40px",
                  textAlign: "center",
                  padding: "8px 6px",
                  color: "#64748b",
                  fontSize: "11px",
                  fontWeight: 600,
                }}
              >
                Tampil
              </th>
              <th
                style={{
                  padding: "8px 6px",
                  textAlign: "left",
                  color: "#64748b",
                  fontSize: "11px",
                  fontWeight: 600,
                }}
              >
                Label Tampil
              </th>
              <th
                style={{
                  width: "80px",
                  padding: "8px 6px",
                  textAlign: "left",
                  color: "#64748b",
                  fontSize: "11px",
                  fontWeight: 600,
                }}
              >
                Satuan
              </th>
              <th
                style={{
                  width: "70px",
                  textAlign: "center",
                  padding: "8px 6px",
                  color: "#64748b",
                  fontSize: "11px",
                  fontWeight: 600,
                }}
              >
                Tipe
              </th>
              <th
                style={{
                  width: "40px",
                  textAlign: "center",
                  padding: "8px 6px",
                  color: "#64748b",
                  fontSize: "11px",
                  fontWeight: 600,
                }}
              >
                Aksi
              </th>
            </tr>
          </thead>
          <tbody>
            {fields.map((f, idx) => {
              const isFirst = idx === 0;
              const isLast = idx === fields.length - 1;
              const isCustom = Boolean(f.custom);
              const labelErr = validateFieldLabel(f.label);

              return (
                <tr
                  key={f.key}
                  style={{
                    backgroundColor: f.visible ? "inherit" : "#fbfbfc",
                    opacity: f.visible ? 1 : 0.65,
                    borderBottom: "1px solid #f1f5f9",
                  }}
                >
                  {/* Urutan */}
                  <td style={{ textAlign: "center", padding: "6px 4px" }}>
                    <ButtonGroup minimal>
                      <Button
                        icon="chevron-up"
                        small
                        disabled={isFirst}
                        onClick={() => moveField(idx, -1)}
                      />
                      <Button
                        icon="chevron-down"
                        small
                        disabled={isLast}
                        onClick={() => moveField(idx, 1)}
                      />
                    </ButtonGroup>
                  </td>

                  {/* Toggle Tampil */}
                  <td style={{ textAlign: "center", padding: "6px 4px" }}>
                    <Button
                      icon={f.visible ? "eye-open" : "eye-off"}
                      minimal
                      small
                      style={{ color: f.visible ? "#0f172a" : "#94a3b8" }}
                      onClick={() => toggleVisible(idx)}
                      title={f.visible ? "Sembunyikan dari popup" : "Tampilkan di popup"}
                    />
                  </td>

                  {/* Label Inline */}
                  <td style={{ padding: "6px 4px" }}>
                    <input
                      type="text"
                      value={f.label}
                      onChange={(e) => updateLabel(idx, e.target.value)}
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
                      Key: <code>{f.key}</code>
                    </div>
                  </td>

                  {/* Unit Inline */}
                  <td style={{ padding: "6px 4px" }}>
                    <input
                      type="text"
                      value={f.unit || ""}
                      onChange={(e) => updateUnit(idx, e.target.value)}
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
                      {f.kind === "number" ? "Angka" : f.kind === "boolean" ? "Bool" : "Teks"}
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
                        onClick={() => setConfirmDeleteField(f)}
                      />
                    ) : (
                      <Tooltip content="Field bawaan plugin tidak dapat dihapus, hanya dapat disembunyikan">
                        <Button icon="trash" minimal small disabled style={{ opacity: 0.3 }} />
                      </Tooltip>
                    )}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>

      {/* Dialog Tambah Field Baru */}
      <Dialog
        isOpen={isAddOpen}
        onClose={() => setIsAddOpen(false)}
        title="Tambah Field Baru"
        style={{ width: "420px", borderRadius: "16px" }}
      >
        <div style={{ padding: "16px 20px" }}>
          {addError && (
            <div
              style={{
                background: "#fee2e2",
                color: "#991b1b",
                padding: "8px 12px",
                borderRadius: "8px",
                fontSize: "12px",
                marginBottom: "12px",
              }}
            >
              {addError}
            </div>
          )}

          <div style={{ marginBottom: "12px" }}>
            <div
              style={{
                fontSize: "12px",
                fontWeight: 600,
                color: "#64748b",
                marginBottom: "4px",
              }}
            >
              Label Field (Judul Tampil)
            </div>
            <input
              id="new-field-label"
              type="text"
              value={newLabel}
              onChange={(e) => handleLabelChange(e.target.value)}
              placeholder="Contoh: Kondisi Daun"
              style={{
                width: "100%",
                padding: "9px 12px",
                fontSize: "13px",
                background: "#f8fafc",
                border: "none",
                borderRadius: "10px",
                outline: "none",
                color: "#0f172a",
                boxSizing: "border-box",
              }}
            />
          </div>

          <div style={{ marginBottom: "12px" }}>
            <div
              style={{
                fontSize: "12px",
                fontWeight: 600,
                color: "#64748b",
                marginBottom: "4px",
              }}
            >
              Key Teknis (Huruf kecil, angka, underscore)
            </div>
            <input
              id="new-field-key"
              type="text"
              value={newKey}
              onChange={(e) => setNewKey(e.target.value.toLowerCase())}
              placeholder="kondisi_daun"
              style={{
                width: "100%",
                padding: "9px 12px",
                fontSize: "13px",
                background: "#f8fafc",
                border: "none",
                borderRadius: "10px",
                outline: "none",
                color: "#0f172a",
                boxSizing: "border-box",
              }}
            />
            <div style={{ fontSize: "11px", color: "#64748b", marginTop: "3px" }}>
              Digunakan secara unik di dalam data GeoJSON properties
            </div>
          </div>

          <div style={{ display: "flex", gap: "12px", marginBottom: "12px" }}>
            <div style={{ flex: 1 }}>
              <div
                style={{
                  fontSize: "12px",
                  fontWeight: 600,
                  color: "#64748b",
                  marginBottom: "4px",
                }}
              >
                Tipe Data
              </div>
              <select
                id="new-field-kind"
                value={newKind}
                onChange={(e) => setNewKind(e.target.value as FieldKind)}
                style={{
                  width: "100%",
                  padding: "9px 12px",
                  fontSize: "13px",
                  borderRadius: "10px",
                  border: "none",
                  background: "#f8fafc",
                  color: "#0f172a",
                  outline: "none",
                  boxSizing: "border-box",
                  cursor: "pointer",
                }}
              >
                <option value="text">Teks</option>
                <option value="number">Angka (Number)</option>
                <option value="boolean">Boolean (Ya / Tidak)</option>
              </select>
            </div>

            <div style={{ flex: 1 }}>
              <div
                style={{
                  fontSize: "12px",
                  fontWeight: 600,
                  color: "#64748b",
                  marginBottom: "4px",
                }}
              >
                Satuan (Opsional)
              </div>
              <input
                id="new-field-unit"
                type="text"
                value={newUnit}
                onChange={(e) => setNewUnit(e.target.value)}
                placeholder="m, m², %, dll."
                style={{
                  width: "100%",
                  padding: "9px 12px",
                  fontSize: "13px",
                  background: "#f8fafc",
                  border: "none",
                  borderRadius: "10px",
                  outline: "none",
                  color: "#0f172a",
                  boxSizing: "border-box",
                }}
              />
            </div>
          </div>

          <div style={{ marginBottom: "16px" }}>
            <div
              style={{
                fontSize: "12px",
                fontWeight: 600,
                color: "#64748b",
                marginBottom: "4px",
              }}
            >
              Nilai Awal Seluruh Fitur (Opsional)
            </div>
            {newKind === "boolean" ? (
              <select
                id="new-field-init"
                value={newInitialVal}
                onChange={(e) => setNewInitialVal(e.target.value)}
                style={{
                  width: "100%",
                  padding: "9px 12px",
                  fontSize: "13px",
                  borderRadius: "10px",
                  border: "none",
                  background: "#f8fafc",
                  color: "#0f172a",
                  outline: "none",
                  boxSizing: "border-box",
                  cursor: "pointer",
                }}
              >
                <option value="">(Kosong / Belum Diisi)</option>
                <option value="true">Ya (True)</option>
                <option value="false">Tidak (False)</option>
              </select>
            ) : (
              <input
                id="new-field-init"
                type={newKind === "number" ? "number" : "text"}
                value={newInitialVal}
                onChange={(e) => setNewInitialVal(e.target.value)}
                placeholder={newKind === "number" ? "0" : "Isi nilai default..."}
                style={{
                  width: "100%",
                  padding: "9px 12px",
                  fontSize: "13px",
                  background: "#f8fafc",
                  border: "none",
                  borderRadius: "10px",
                  outline: "none",
                  color: "#0f172a",
                  boxSizing: "border-box",
                }}
              />
            )}
            <div style={{ fontSize: "11px", color: "#64748b", marginTop: "3px" }}>
              Akan diset ke setiap fitur saat disimpan
            </div>
          </div>

          <div
            style={{
              display: "flex",
              justifyContent: "flex-end",
              gap: "8px",
              marginTop: "20px",
            }}
          >
            <Button
              minimal
              text="Batal"
              onClick={() => setIsAddOpen(false)}
              style={{
                borderRadius: "8px",
                background: "#f1f5f9",
                color: "#475569",
                fontWeight: 600,
                padding: "6px 14px",
              }}
            />
            <Button
              minimal
              text="Tambah Field"
              onClick={handleSaveNewField}
              style={{
                borderRadius: "8px",
                background: "#0f172a",
                color: "#ffffff",
                fontWeight: 600,
                padding: "6px 16px",
              }}
            />
          </div>
        </div>
      </Dialog>

      {/* Dialog Konfirmasi Hapus Field */}
      <Dialog
        isOpen={Boolean(confirmDeleteField)}
        onClose={() => setConfirmDeleteField(null)}
        title="Konfirmasi Hapus Field Custom"
        style={{ width: "420px", borderRadius: "16px" }}
      >
        <div style={{ padding: "16px 20px" }}>
          <p style={{ fontSize: "13px", lineHeight: "1.5" }}>
            Hapus field custom <strong>{confirmDeleteField?.label}</strong> (key:{" "}
            <code>{confirmDeleteField?.key}</code>)?
          </p>
          <p style={{ fontSize: "12px", color: "#64748b" }}>
            Key ini akan dihapus dari seluruh fitur pada layer ini saat disimpan. Aksi ini tidak
            dapat dibatalkan.
          </p>
          <div
            style={{
              display: "flex",
              justifyContent: "flex-end",
              gap: "8px",
              marginTop: "16px",
            }}
          >
            <Button
              minimal
              text="Batal"
              onClick={() => setConfirmDeleteField(null)}
              style={{
                borderRadius: "8px",
                background: "#f1f5f9",
                color: "#475569",
                fontWeight: 600,
                padding: "6px 14px",
              }}
            />
            <Button
              minimal
              text="Hapus Field"
              style={{
                borderRadius: "8px",
                background: "#ef4444",
                color: "#ffffff",
                fontWeight: 600,
                padding: "6px 16px",
              }}
              onClick={() => {
                if (confirmDeleteField) {
                  onDeleteCustomField(confirmDeleteField);
                  setConfirmDeleteField(null);
                }
              }}
            />
          </div>
        </div>
      </Dialog>
    </div>
  );
};
