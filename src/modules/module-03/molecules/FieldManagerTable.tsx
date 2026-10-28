import React, { useState } from "react";
import {
  Button,
  ButtonGroup,
  Dialog,
  FormGroup,
  HTMLSelect,
  HTMLTable,
  InputGroup,
  Tag,
  Tooltip,
} from "@blueprintjs/core";
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
          <span style={{ fontWeight: 600, fontSize: "13px" }}>Kelola Field & Atribut</span>
          <div style={{ fontSize: "11px", color: "#64748b" }}>
            Atur urutan, visibilitas, judul tampil, dan satuan pengukuran pada popup
          </div>
        </div>
        <Button
          icon="plus"
          intent="primary"
          minimal
          small
          text="Tambah Field"
          onClick={handleOpenAdd}
        />
      </div>

      <div
        style={{
          maxHeight: "260px",
          overflowY: "auto",
          border: "1px solid #e2e8f0",
          borderRadius: "6px",
        }}
      >
        <HTMLTable compact style={{ width: "100%", fontSize: "12px" }}>
          <thead>
            <tr style={{ background: "#f8fafc" }}>
              <th style={{ width: "52px", textAlign: "center" }}>Urutan</th>
              <th style={{ width: "40px", textAlign: "center" }}>Tampil</th>
              <th>Label Tampil</th>
              <th style={{ width: "70px" }}>Satuan</th>
              <th style={{ width: "70px", textAlign: "center" }}>Tipe</th>
              <th style={{ width: "40px", textAlign: "center" }}>Aksi</th>
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
                  }}
                >
                  {/* Urutan */}
                  <td style={{ textAlign: "center", padding: "4px" }}>
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
                  <td style={{ textAlign: "center", padding: "4px" }}>
                    <Button
                      icon={f.visible ? "eye-open" : "eye-off"}
                      minimal
                      small
                      intent={f.visible ? "primary" : "none"}
                      onClick={() => toggleVisible(idx)}
                      title={f.visible ? "Sembunyikan dari popup" : "Tampilkan di popup"}
                    />
                  </td>

                  {/* Label Inline */}
                  <td style={{ padding: "4px" }}>
                    <InputGroup
                      value={f.label}
                      onChange={(e) => updateLabel(idx, e.target.value)}
                      small
                      intent={labelErr ? "danger" : "none"}
                      placeholder="Label field"
                    />
                    <div style={{ fontSize: "10px", color: "#94a3b8", marginTop: "1px" }}>
                      Key: <code>{f.key}</code>
                    </div>
                  </td>

                  {/* Unit Inline */}
                  <td style={{ padding: "4px" }}>
                    <InputGroup
                      value={f.unit || ""}
                      onChange={(e) => updateUnit(idx, e.target.value)}
                      small
                      placeholder="e.g. m, m²"
                    />
                  </td>

                  {/* Kind */}
                  <td style={{ textAlign: "center", padding: "4px" }}>
                    <Tag
                      minimal
                      intent={
                        f.kind === "number" ? "warning" : f.kind === "boolean" ? "success" : "none"
                      }
                      style={{ fontSize: "10px" }}
                    >
                      {f.kind === "number" ? "Angka" : f.kind === "boolean" ? "Bool" : "Teks"}
                    </Tag>
                  </td>

                  {/* Aksi Hapus */}
                  <td style={{ textAlign: "center", padding: "4px" }}>
                    {isCustom ? (
                      <Button
                        icon="trash"
                        minimal
                        small
                        intent="danger"
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
        </HTMLTable>
      </div>

      {/* Dialog Tambah Field Baru */}
      <Dialog
        isOpen={isAddOpen}
        onClose={() => setIsAddOpen(false)}
        title="Tambah Field Baru"
        style={{ width: "420px" }}
      >
        <div style={{ padding: "16px 20px" }}>
          {addError && (
            <div
              style={{
                background: "#fee2e2",
                color: "#991b1b",
                padding: "8px 12px",
                borderRadius: "6px",
                fontSize: "12px",
                marginBottom: "12px",
              }}
            >
              {addError}
            </div>
          )}

          <FormGroup label="Label Field (Judul Tampil)" labelFor="new-field-label">
            <InputGroup
              id="new-field-label"
              value={newLabel}
              onChange={(e) => handleLabelChange(e.target.value)}
              placeholder="Contoh: Kondisi Daun"
            />
          </FormGroup>

          <FormGroup
            label="Key Teknis (Huruf kecil, angka, underscore)"
            labelFor="new-field-key"
            helperText="Digunakan secara unik di dalam data GeoJSON properties"
          >
            <InputGroup
              id="new-field-key"
              value={newKey}
              onChange={(e) => setNewKey(e.target.value.toLowerCase())}
              placeholder="kondisi_daun"
            />
          </FormGroup>

          <div style={{ display: "flex", gap: "12px" }}>
            <FormGroup label="Tipe Data" labelFor="new-field-kind" style={{ flex: 1 }}>
              <HTMLSelect
                id="new-field-kind"
                value={newKind}
                onChange={(e) => setNewKind(e.target.value as FieldKind)}
                fill
              >
                <option value="text">Teks</option>
                <option value="number">Angka (Number)</option>
                <option value="boolean">Boolean (Ya / Tidak)</option>
              </HTMLSelect>
            </FormGroup>

            <FormGroup label="Satuan (Opsional)" labelFor="new-field-unit" style={{ flex: 1 }}>
              <InputGroup
                id="new-field-unit"
                value={newUnit}
                onChange={(e) => setNewUnit(e.target.value)}
                placeholder="m, m², %, dll."
              />
            </FormGroup>
          </div>

          <FormGroup
            label="Nilai Awal Seluruh Fitur (Opsional)"
            labelFor="new-field-init"
            helperText="Akan diset ke setiap fitur saat disimpan"
          >
            {newKind === "boolean" ? (
              <HTMLSelect
                id="new-field-init"
                value={newInitialVal}
                onChange={(e) => setNewInitialVal(e.target.value)}
                fill
              >
                <option value="">(Kosong / Belum Diisi)</option>
                <option value="true">Ya (True)</option>
                <option value="false">Tidak (False)</option>
              </HTMLSelect>
            ) : (
              <InputGroup
                id="new-field-init"
                type={newKind === "number" ? "number" : "text"}
                value={newInitialVal}
                onChange={(e) => setNewInitialVal(e.target.value)}
                placeholder={newKind === "number" ? "0" : "Isi nilai default..."}
              />
            )}
          </FormGroup>

          <div
            style={{
              display: "flex",
              justifyContent: "flex-end",
              gap: "8px",
              marginTop: "20px",
            }}
          >
            <Button text="Batal" onClick={() => setIsAddOpen(false)} />
            <Button text="Tambah Field" intent="primary" onClick={handleSaveNewField} />
          </div>
        </div>
      </Dialog>

      {/* Dialog Konfirmasi Hapus Field */}
      <Dialog
        isOpen={Boolean(confirmDeleteField)}
        onClose={() => setConfirmDeleteField(null)}
        title="Konfirmasi Hapus Field Custom"
        style={{ width: "420px" }}
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
            <Button text="Batal" onClick={() => setConfirmDeleteField(null)} />
            <Button
              text="Hapus Field"
              intent="danger"
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
