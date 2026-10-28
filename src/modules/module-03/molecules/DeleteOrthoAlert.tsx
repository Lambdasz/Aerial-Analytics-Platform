import React from "react";
import { Alert, Spinner } from "@blueprintjs/core";
import type { DeleteOrthoSummary, StoredImageMetadata } from "../types/storage";

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
    <Alert
      isOpen={isOpen}
      onClose={onClose}
      onConfirm={onConfirm}
      onCancel={onClose}
      canEscapeKeyCancel={!isProcessing}
      canOutsideClickCancel={!isProcessing}
      confirmButtonText={isProcessing ? "Menghapus..." : "Hapus Permanen"}
      cancelButtonText="Batal"
      intent="danger"
      icon="trash"
      loading={isProcessing}
    >
      <div style={{ fontSize: "14px", lineHeight: "1.5" }}>
        <p style={{ marginTop: 0 }}>
          Apakah Anda yakin ingin menghapus citra <strong>{image.filename}</strong>? Tindakan ini
          bersifat permanen dan tidak dapat dibatalkan.
        </p>

        {isLoadingSummary ? (
          <div style={{ display: "flex", alignItems: "center", gap: "8px", margin: "16px 0" }}>
            <Spinner size={16} />
            <span style={{ fontSize: "12px", color: "var(--color-slate, #64748b)" }}>
              Menghitung data turunan yang terikat...
            </span>
          </div>
        ) : (
          <div
            style={{
              background: "rgba(217, 56, 56, 0.05)",
              border: "1px solid rgba(217, 56, 56, 0.2)",
              borderRadius: "8px",
              padding: "10px 14px",
              marginTop: "12px",
              fontSize: "12px",
            }}
          >
            <div style={{ fontWeight: 600, color: "#d93838", marginBottom: "6px" }}>
              Rincian data yang akan dibersihkan:
            </div>
            <ul style={{ margin: 0, paddingLeft: "18px", color: "var(--color-ink, #1e293b)" }}>
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
    </Alert>
  );
};
