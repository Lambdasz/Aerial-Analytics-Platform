import React, { useState } from "react";
import { Icon } from "@blueprintjs/core";
import { open } from "@tauri-apps/plugin-dialog";

export interface FileSelectionResult {
  path: string;
  name: string;
  ext: string;
  size?: number;
  mimeType?: string;
}

const getMimeTypeFromExt = (ext: string): string => {
  switch (ext.toLowerCase()) {
    case "jpg":
    case "jpeg":
      return "image/jpeg";
    case "png":
      return "image/png";
    case "tif":
    case "tiff":
    case "geotiff":
      return "image/tiff";
    default:
      return "application/octet-stream";
  }
};

export interface ImportDropzoneProps {
  label: string;
  sublabel?: string;
  allowedExtensions: string[];
  filterName: string;
  onFileSelect: (file: FileSelectionResult) => void;
  onError?: (error: string) => void;
  disabled?: boolean;
}

export const ImportDropzone: React.FC<ImportDropzoneProps> = ({
  label,
  sublabel,
  allowedExtensions,
  filterName,
  onFileSelect,
  onError,
  disabled = false,
}) => {
  const [isDragOver, setIsDragOver] = useState(false);

  const processPath = (rawPath: string, size?: number, mimeType?: string) => {
    const filename = rawPath.split(/[/\\]/).pop() || rawPath;
    const ext = filename.split(".").pop()?.toLowerCase() || "";

    if (!allowedExtensions.map((e) => e.toLowerCase()).includes(ext)) {
      const err = `Format berkas tidak valid (.${ext}). Harap pilih berkas dengan ekstensi: ${allowedExtensions.map((e) => `.${e}`).join(", ")}.`;
      if (onError) onError(err);
      return;
    }

    onFileSelect({
      path: rawPath,
      name: filename,
      ext,
      size,
      mimeType: mimeType || getMimeTypeFromExt(ext),
    });
  };

  const handleOpenDialog = async () => {
    if (disabled) return;
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        filters: [
          {
            name: filterName,
            extensions: allowedExtensions,
          },
        ],
      });

      if (!selected || typeof selected !== "string") {
        return;
      }

      processPath(selected);
    } catch (err) {
      console.error("Gagal membuka file dialog:", err);
      if (onError) onError("Gagal membuka dialog pemilihan berkas.");
    }
  };

  const handleDragOver = (e: React.DragEvent<HTMLDivElement>) => {
    e.preventDefault();
    e.stopPropagation();
    if (!disabled && !isDragOver) {
      setIsDragOver(true);
    }
  };

  const handleDragLeave = (e: React.DragEvent<HTMLDivElement>) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragOver(false);
  };

  const handleDrop = (e: React.DragEvent<HTMLDivElement>) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragOver(false);
    if (disabled) return;

    if (e.dataTransfer.files && e.dataTransfer.files.length > 0) {
      const file = e.dataTransfer.files[0];
      // On web / Tauri, File object may have path property or webkitRelativePath
      const filePath = (file as unknown as { path?: string }).path;
      if (filePath) {
        processPath(filePath, file.size, file.type || undefined);
      } else {
        // Fallback for file name
        const filename = file.name;
        const ext = filename.split(".").pop()?.toLowerCase() || "";
        if (!allowedExtensions.map((e) => e.toLowerCase()).includes(ext)) {
          if (onError) {
            onError(
              `Format berkas tidak valid. Harap pilih: ${allowedExtensions.map((e) => `.${e}`).join(", ")}`,
            );
          }
          return;
        }
        // If no absolute path from drop, instruct user to use file picker
        if (onError) {
          onError(
            "Untuk keamanan akses lokal, silakan klik tombol di bawah untuk memilih file melalui dialog sistem.",
          );
        }
      }
    }
  };

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={() => void handleOpenDialog()}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          void handleOpenDialog();
        }
      }}
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
      onDrop={handleDrop}
      style={{
        width: "100%",
        padding: "20px 16px",
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        gap: "6px",
        background: isDragOver ? "#f1f5f9" : "#f8fafc",
        border: isDragOver ? "2px dashed #0f172a" : "1px dashed #cbd5e1",
        borderRadius: "12px",
        cursor: disabled ? "not-allowed" : "pointer",
        color: "#0f172a",
        outline: "none",
        transition: "all 0.15s ease",
        boxSizing: "border-box",
        opacity: disabled ? 0.6 : 1,
      }}
    >
      <div
        style={{
          width: "36px",
          height: "36px",
          borderRadius: "50%",
          background: "#ffffff",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          boxShadow: "0 1px 3px rgba(0,0,0,0.06)",
        }}
      >
        <Icon icon="folder-open" size={18} color="#475569" />
      </div>
      <div style={{ fontSize: "13px", fontWeight: 600, color: "#0f172a", textAlign: "center" }}>
        {label}
      </div>
      {sublabel && (
        <div style={{ fontSize: "11px", color: "#64748b", textAlign: "center" }}>{sublabel}</div>
      )}
    </div>
  );
};
