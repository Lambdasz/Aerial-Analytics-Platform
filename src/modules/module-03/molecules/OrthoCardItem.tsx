import React, { useState } from "react";
import { Button, Icon, Tag } from "@blueprintjs/core";
import type { StoredImageMetadata } from "../types/storage";
import { MAX_DISPLAY_FILE_SIZE_BYTES } from "../config/mapConfig";

export interface OrthoCardItemProps {
  img: StoredImageMetadata;
  thumbnailUrl?: string;
  isSelected: boolean;
  isProcessing: boolean;
  onFocus: (img: StoredImageMetadata) => void;
  onEdit: (img: StoredImageMetadata) => void;
  onDelete: (img: StoredImageMetadata) => void;
}

export const OrthoCardItem: React.FC<OrthoCardItemProps> = ({
  img,
  thumbnailUrl,
  isSelected,
  isProcessing,
  onFocus,
  onEdit,
  onDelete,
}) => {
  const itemKey = img.id || img.filename;
  const isCorrupted = Boolean(img.is_corrupted);
  const [hasImgError, setHasImgError] = useState(false);

  const formatBytes = (bytes: number): string => {
    if (bytes >= 1024 * 1024) {
      return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    }
    return `${(bytes / 1024).toFixed(0)} KB`;
  };

  const isOversized = Boolean(
    img.display_size_bytes && img.display_size_bytes > MAX_DISPLAY_FILE_SIZE_BYTES,
  );

  return (
    <div
      id={`gallery-item-${itemKey}`}
      className={`gallery-card ${isSelected ? "active" : ""} ${isCorrupted ? "corrupted" : ""}`}
      style={{
        display: "flex",
        flexDirection: "row",
        alignItems: "center",
        justifyContent: "space-between",
        gap: "10px",
        borderColor: isCorrupted ? "rgba(217, 56, 56, 0.4)" : undefined,
      }}
    >
      {/* Kiri: Thumbnail & Info Utama */}
      <button
        type="button"
        style={{
          display: "flex",
          alignItems: "center",
          gap: "10px",
          flex: 1,
          minWidth: 0,
          background: "none",
          border: "none",
          padding: 0,
          textAlign: "left",
          cursor: isCorrupted ? "default" : "pointer",
        }}
        disabled={isCorrupted}
        onClick={() => {
          if (!isCorrupted) {
            onFocus(img);
          }
        }}
      >
        <div className="gallery-thumb-container" style={{ flexShrink: 0 }}>
          {isCorrupted ? (
            <div
              style={{
                width: "100%",
                height: "100%",
                display: "flex",
                flexDirection: "column",
                alignItems: "center",
                justifyContent: "center",
                background: "rgba(217, 56, 56, 0.08)",
                color: "#d93838",
              }}
            >
              <Icon icon="warning-sign" size={24} />
            </div>
          ) : !thumbnailUrl || hasImgError ? (
            <div
              style={{
                width: "100%",
                height: "100%",
                display: "flex",
                flexDirection: "column",
                alignItems: "center",
                justifyContent: "center",
                background: "rgba(143, 153, 168, 0.12)",
                color: "#8a9ba8",
              }}
              title="Tidak ada pratinjau citra"
            >
              <Icon icon="media" size={22} />
              <span style={{ fontSize: "9px", marginTop: "2px", fontWeight: 500 }}>No Image</span>
            </div>
          ) : (
            <>
              <img
                src={thumbnailUrl}
                alt={img.filename}
                loading="lazy"
                onError={() => setHasImgError(true)}
              />
              <span className="gallery-thumb-tag">{img.format || "ORTHO"}</span>
            </>
          )}
        </div>

        <div className="gallery-card-info" style={{ flex: 1, minWidth: 0 }}>
          <div
            className="gallery-card-title"
            title={img.filename}
            style={{
              color: isCorrupted ? "var(--color-danger, #d93838)" : undefined,
              fontWeight: 600,
            }}
          >
            {img.filename}
          </div>

          <div className="gallery-card-sub" style={{ flexWrap: "wrap", gap: "6px" }}>
            {isCorrupted ? (
              <Tag intent="danger" minimal round style={{ fontSize: "10px", fontWeight: 700 }}>
                Metadata rusak
              </Tag>
            ) : (
              <>
                <span>{formatBytes(img.size_bytes)}</span>
                <span>•</span>
                {img.dimensions && (
                  <>
                    <span>
                      {img.dimensions[0]}×{img.dimensions[1]}
                    </span>
                    <span>•</span>
                  </>
                )}
                <span>Ortho WGS84</span>
              </>
            )}
          </div>

          {isOversized && (
            <div
              style={{
                color: "var(--color-danger, #d93838)",
                fontSize: "10px",
                fontWeight: 600,
              }}
            >
              ⚠️ Display &gt; 5 MB (Pratinjau nonaktif)
            </div>
          )}
        </div>
      </button>

      {/* Kanan: Toolbar Aksi */}
      <div
        className="gallery-card-actions"
        style={{
          display: "flex",
          alignItems: "center",
          gap: "4px",
          flexShrink: 0,
        }}
      >
        <Button
          icon="locate"
          minimal
          small
          disabled={isCorrupted || isProcessing}
          onClick={(e) => {
            e.stopPropagation();
            onFocus(img);
          }}
          title="Fokus ke area peta"
        />
        <Button
          icon="edit"
          minimal
          small
          disabled={isCorrupted || isProcessing}
          onClick={(e) => {
            e.stopPropagation();
            onEdit(img);
          }}
          title="Edit nama & koordinat bounding box"
        />
        <Button
          icon="trash"
          minimal
          small
          intent="danger"
          disabled={isProcessing}
          onClick={(e) => {
            e.stopPropagation();
            onDelete(img);
          }}
          title="Hapus citra orthomosaic"
        />
      </div>
    </div>
  );
};
