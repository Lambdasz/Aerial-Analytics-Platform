import React from "react";
import { Alert, Spinner } from "@blueprintjs/core";
import type { DeleteSpatialResultSummary, SpatialResultSummary } from "../types/spatialResult";

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
    <Alert
      isOpen={isOpen}
      onCancel={onClose}
      onConfirm={onConfirm}
      intent="danger"
      icon="trash"
      cancelButtonText="Batal"
      confirmButtonText={isDeleting ? "Menghapus..." : "Hapus Permanen"}
      loading={isDeleting}
      style={{ maxWidth: "450px" }}
    >
      <p style={{ marginBottom: "12px" }}>
        Apakah Anda yakin ingin menghapus layer hasil analisis{" "}
        <strong>{item.layer_name || item.id}</strong>? Tindakan ini bersifat permanen dan tidak
        dapat dibatalkan.
      </p>

      {isLoadingCount ? (
        <div style={{ display: "flex", alignItems: "center", gap: "8px", margin: "12px 0" }}>
          <Spinner size={16} />
          <span style={{ fontSize: "12px", color: "#8a9ba8" }}>Memeriksa dependensi...</span>
        </div>
      ) : item.is_corrupted ? (
        <p style={{ fontSize: "12px", color: "#d93838" }}>
          Menghapus berkas/folder hasil analisis yang rusak secara tuntas dari disk.
        </p>
      ) : (
        <div
          style={{
            background: "rgba(217, 56, 56, 0.05)",
            border: "1px solid rgba(217, 56, 56, 0.2)",
            borderRadius: "6px",
            padding: "8px 12px",
            fontSize: "12px",
            marginBottom: "12px",
          }}
        >
          <div style={{ fontWeight: 600, color: "#d93838", marginBottom: "4px" }}>
            Data yang akan dibersihkan:
          </div>
          <ul style={{ margin: 0, paddingLeft: "16px", color: "#5c7080" }}>
            <li>
              Berkas spasial dan fitur ({dependents?.feature_count ?? item.feature_count} fitur)
            </li>
            <li>Registrasi layer tampilan pada peta ({dependents?.layers_removed ?? 1} layer)</li>
          </ul>
        </div>
      )}

      {/* Penegasan bahwa foto yang dirujuk TIDAK ikut terhapus */}
      {item.thumbnail_url && !item.is_corrupted && (
        <p style={{ fontSize: "11px", color: "#5c7080", margin: 0 }}>
          Catatan: Berkas citra/foto bersama yang dirujuk (<code>{item.thumbnail_url}</code>){" "}
          <strong>tidak akan dihapus</strong> dari penyimpanan.
        </p>
      )}
    </Alert>
  );
};
