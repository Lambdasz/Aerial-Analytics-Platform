import React, { useState } from "react";
import { Button, Dialog } from "@blueprintjs/core";
import type { FieldDef, FieldKind } from "../../types/spatialResult";
import {
  validateCustomFieldKey,
  validateCustomFieldsCount,
  validateFieldLabel,
  validateFieldUnit,
} from "../../lib/fieldValidation";

export interface AddCustomFieldDialogProps {
  isOpen: boolean;
  onClose: () => void;
  existingFields: FieldDef[];
  onAddField: (field: FieldDef, initialValue?: unknown) => void;
}

export const AddCustomFieldDialog: React.FC<AddCustomFieldDialogProps> = ({
  isOpen,
  onClose,
  existingFields,
  onAddField,
}) => {
  const [newLabel, setNewLabel] = useState("");
  const [newKey, setNewKey] = useState("");
  const [newUnit, setNewUnit] = useState("");
  const [newKind, setNewKind] = useState<FieldKind>("text");
  const [newInitialVal, setNewInitialVal] = useState("");
  const [addError, setAddError] = useState<string | null>(null);

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

  const handleResetAndClose = () => {
    setNewLabel("");
    setNewKey("");
    setNewUnit("");
    setNewKind("text");
    setNewInitialVal("");
    setAddError(null);
    onClose();
  };

  const handleSave = () => {
    // 1. Validasi label
    const labelErr = validateFieldLabel(newLabel);
    if (labelErr) {
      setAddError(labelErr);
      return;
    }

    // 2. Validasi key
    const existingKeys = existingFields.map((f) => f.key);
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
      ...existingFields,
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
    handleResetAndClose();
  };

  return (
    <Dialog
      isOpen={isOpen}
      onClose={handleResetAndClose}
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
            onClick={handleResetAndClose}
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
            onClick={handleSave}
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
  );
};
