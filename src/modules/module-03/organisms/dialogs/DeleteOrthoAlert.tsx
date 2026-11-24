import React from "react";
import { Dialog, Button, Spinner, Classes } from "@blueprintjs/core";
import type { DeleteOrthoSummary, StoredImageMetadata } from "../../types/storage";

export interface DeleteOrthoAlertProps {
  isOpen: boolean;
  onClose: () => void;
  image: StoredImageMetadata | null;
  summary: DeleteOrthoSummary | null;
  isLoadingSummary: boolean;
  isProcessing: boolean;
  onConfirm: () => void;
}

export const DeleteOrthoAlert: React.FC<DeleteOrthoAlertProps> = ({
  isOpen,
  onClose,
  image,
  summary,
  isLoadingSummary,
  isProcessing,
  onConfirm,
}) => {
  if (!image) return null;

  const isCorrupted = Boolean(image.is_corrupted);
  const annoCount = summary ? summary.annotations_deleted_count : 0;

  return (
    <Dialog
      isOpen={isOpen}
      onClose={isProcessing ? undefined : onClose}
      canEscapeKeyClose={!isProcessing}
      canOutsideClickClose={!isProcessing}
      style={{ width: "460px", maxWidth: "92vw", borderRadius: "16px", overflow: "hidden" }}
    >
      <div className={Classes.DIALOG_BODY} style={{ padding: "24px 24px 16px 24px" }}>
        {/* Judul Dialog */}
        <div style={{ marginBottom: "12px" }}>
          <div style={{ fontSize: "16px", fontWeight: 700, color: "#0f172a" }}>
            Hapus Citra Ortofoto?
          </div>
        </div>

        <p style={{ fontSize: "13px", color: "#334155", lineHeight: "1.5", margin: "0 0 14px 0" }}>
          Apakah Anda yakin ingin menghapus citra <strong>"{image.filename}"</strong>? Tindakan ini
          bersifat permanen dan tidak dapat dibatalkan.
        </p>

        {isLoadingSummary ? (
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: "8px",
              padding: "16px",
              background: "#f8fafc",
              borderRadius: "12px",
              border: "1px solid #f1f5f9",
              marginBottom: "14px",
            }}
          >
            <Spinner size={16} />
            <span style={{ fontSize: "12px", color: "#64748b" }}>
              Menghitung data turunan yang terikat...
            </span>
          </div>
        ) : (
          <div
            style={{
              background: "#f8fafc",
              border: "1px solid #f1f5f9",
              borderRadius: "12px",
              padding: "12px 16px",
              fontSize: "12px",
              marginBottom: "14px",
            }}
          >
            <div style={{ fontWeight: 600, color: "#0f172a", marginBottom: "6px" }}>
              Rincian data yang akan dibersihkan:
            </div>
            <ul style={{ margin: 0, paddingLeft: "18px", color: "#475569", lineHeight: "1.6" }}>
              {isCorrupted ? (
                <li>Menghapus folder berkas citra yang metadata-nya rusak secara tuntas.</li>
              ) : (
                <li>Menghapus folder citra asli dan pratinjau display.</li>
              )}
              <li>Menghapus 1 entitas layer di panel layer.</li>
              <li>Menghapus {annoCount} anotasi yang terikat pada citra ini.</li>
            </ul>
          </div>
        )}
      </div>

      <div className={Classes.DIALOG_FOOTER} style={{ padding: "0 24px 20px 24px" }}>
        <div className={Classes.DIALOG_FOOTER_ACTIONS} style={{ gap: "8px" }}>
          <Button
            minimal
            text="Batal"
            disabled={isProcessing}
            onClick={onClose}
            style={{
              background: "#f1f5f9",
              color: "#475569",
              borderRadius: "999px",
              fontWeight: 600,
              padding: "8px 18px",
              outline: "none",
            }}
          />
          <Button
            minimal
            text={isProcessing ? "Menghapus..." : "Hapus Permanen"}
            loading={isProcessing}
            onClick={onConfirm}
            style={{
              background: "#ef4444",
              color: "#ffffff",
              borderRadius: "999px",
              fontWeight: 600,
              padding: "8px 18px",
              boxShadow: "0 2px 8px rgba(239, 68, 68, 0.25)",
              outline: "none",
            }}
          />
        </div>
      </div>
    </Dialog>
  );
};
