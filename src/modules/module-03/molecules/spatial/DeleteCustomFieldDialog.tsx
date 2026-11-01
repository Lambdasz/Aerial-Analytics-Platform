import React from "react";
import { Button, Dialog } from "@blueprintjs/core";
import type { FieldDef } from "../../types/spatialResult";

export interface DeleteCustomFieldDialogProps {
  field: FieldDef | null;
  onClose: () => void;
  onConfirm: (field: FieldDef) => void;
}

export const DeleteCustomFieldDialog: React.FC<DeleteCustomFieldDialogProps> = ({
  field,
  onClose,
  onConfirm,
}) => {
  return (
    <Dialog
      isOpen={Boolean(field)}
      onClose={onClose}
      title="Konfirmasi Hapus Field Custom"
      style={{ width: "420px", borderRadius: "16px" }}
    >
      <div style={{ padding: "16px 20px" }}>
        <p style={{ fontSize: "13px", lineHeight: "1.5" }}>
          Hapus field custom <strong>{field?.label}</strong> (key: <code>{field?.key}</code>)?
        </p>
        <p style={{ fontSize: "12px", color: "#64748b" }}>
          Key ini akan dihapus dari seluruh fitur pada layer ini saat disimpan. Aksi ini tidak dapat
          dibatalkan.
        </p>
        <div style={{ display: "flex", justifyContent: "flex-end", gap: "8px", marginTop: "16px" }}>
          <Button
            minimal
            text="Batal"
            onClick={onClose}
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
              if (field) {
                onConfirm(field);
                onClose();
              }
            }}
          />
        </div>
      </div>
    </Dialog>
  );
};
