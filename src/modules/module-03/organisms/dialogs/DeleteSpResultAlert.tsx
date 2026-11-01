import React from "react";
import { Dialog, Button, Spinner, Classes } from "@blueprintjs/core";
import type { DeleteSpatialResultSummary, SpatialResultSummary } from "../../types/spatialResult";

export interface DeleteSpResultAlertProps {
  isOpen: boolean;
  item: SpatialResultSummary | null;
  dependents: DeleteSpatialResultSummary | null;
  isLoadingCount: boolean;
  isDeleting: boolean;
  onClose: () => void;
  onConfirm: () => void;
}

export const DeleteSpResultAlert: React.FC<DeleteSpResultAlertProps> = ({
  isOpen,
  item,
  dependents,
  isLoadingCount,
  isDeleting,
  onClose,
  onConfirm,
}) => {
  if (!item) return null;

  return (
    <Dialog
      isOpen={isOpen}
      onClose={isDeleting ? undefined : onClose}
      canEscapeKeyClose={!isDeleting}
      canOutsideClickClose={!isDeleting}
      style={{ width: "460px", maxWidth: "92vw", borderRadius: "16px", overflow: "hidden" }}
    >
      <div className={Classes.DIALOG_BODY} style={{ padding: "24px 24px 16px 24px" }}>
        {/* Judul Dialog */}
        <div style={{ marginBottom: "12px" }}>
          <div style={{ fontSize: "16px", fontWeight: 700, color: "#0f172a" }}>
            Hapus Layer Analisis?
          </div>
        </div>

        <p style={{ fontSize: "13px", color: "#334155", lineHeight: "1.5", margin: "0 0 14px 0" }}>
          Apakah Anda yakin ingin menghapus layer <strong>"{item.layer_name || item.id}"</strong>?
          Tindakan ini bersifat permanen dan tidak dapat dibatalkan.
        </p>

        {isLoadingCount ? (
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
            <span style={{ fontSize: "12px", color: "#64748b" }}>Memeriksa dependensi...</span>
          </div>
        ) : item.is_corrupted ? (
          <div
            style={{
              background: "#f8fafc",
              border: "1px solid #f1f5f9",
              borderRadius: "12px",
              padding: "12px 16px",
              fontSize: "12px",
              color: "#dc2626",
              marginBottom: "14px",
            }}
          >
            Menghapus berkas dan folder hasil analisis yang rusak secara tuntas dari penyimpanan.
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
              Data yang akan dibersihkan:
            </div>
            <ul style={{ margin: 0, paddingLeft: "18px", color: "#475569", lineHeight: "1.6" }}>
              <li>
                Berkas spasial dan fitur ({dependents?.feature_count ?? item.feature_count} fitur)
              </li>
              <li>Registrasi layer tampilan pada peta ({dependents?.layers_removed ?? 1} layer)</li>
            </ul>
          </div>
        )}

        {/* Penegasan foto yang dirujuk */}
        {item.thumbnail_url && !item.is_corrupted && (
          <p
            style={{
              fontSize: "11px",
              color: "#64748b",
              margin: 0,
              lineHeight: "1.4",
              wordBreak: "break-all",
              overflowWrap: "anywhere",
            }}
          >
            Catatan: Berkas citra/foto bersama yang dirujuk (
            <code style={{ wordBreak: "break-all", overflowWrap: "anywhere" }}>
              {item.thumbnail_url}
            </code>
            ) tidak akan dihapus dari penyimpanan.
          </p>
        )}
      </div>

      <div className={Classes.DIALOG_FOOTER} style={{ padding: "0 24px 20px 24px" }}>
        <div className={Classes.DIALOG_FOOTER_ACTIONS} style={{ gap: "8px" }}>
          <Button
            minimal
            text="Batal"
            disabled={isDeleting}
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
            text={isDeleting ? "Menghapus..." : "Hapus Permanen"}
            loading={isDeleting}
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
