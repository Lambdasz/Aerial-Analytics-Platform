import React from "react";
import { Dialog, Button } from "@blueprintjs/core";

export interface UnsavedChangesDialogProps {
  isOpen: boolean;
  onCancel: () => void;
  onConfirmDiscard: () => void;
}

export const UnsavedChangesDialog: React.FC<UnsavedChangesDialogProps> = ({
  isOpen,
  onCancel,
  onConfirmDiscard,
}) => {
  return (
    <Dialog
      isOpen={isOpen}
      onClose={onCancel}
      title="Perubahan Belum Disimpan"
      style={{ width: "380px", borderRadius: "16px" }}
    >
      <div style={{ padding: "16px 20px" }}>
        <p style={{ fontSize: "13px", lineHeight: "1.5" }}>
          Anda memiliki perubahan yang belum disimpan. Yakin ingin menutup formulir?
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
            text="Lanjut Mengedit"
            minimal
            style={{
              borderRadius: "8px",
              background: "#f1f5f9",
              color: "#475569",
              fontWeight: 600,
            }}
            onClick={onCancel}
          />
          <Button
            text="Tutup & Buang Perubahan"
            minimal
            style={{
              borderRadius: "8px",
              background: "#ef4444",
              color: "#ffffff",
              fontWeight: 600,
            }}
            onClick={onConfirmDiscard}
          />
        </div>
      </div>
    </Dialog>
  );
};
