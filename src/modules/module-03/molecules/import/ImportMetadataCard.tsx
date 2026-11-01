import React from "react";
import { Button, Tag } from "@blueprintjs/core";

export interface ImportMetadataCardProps {
  fileName: string;
  formatTag: string;
  mimeType?: string | null;
  dimensions?: [number, number] | null;
  fileSizeBytes?: number | null;
  bitDepth?: number | null;
  hasAlpha?: boolean;
  compression?: string | null;
  onRemove: () => void;
  disabled?: boolean;
}

export const ImportMetadataCard: React.FC<ImportMetadataCardProps> = ({
  fileName,
  formatTag,
  mimeType,
  dimensions,
  fileSizeBytes,
  bitDepth,
  hasAlpha,
  compression,
  onRemove,
  disabled = false,
}) => {
  const formattedSize =
    fileSizeBytes !== undefined && fileSizeBytes !== null
      ? `${(fileSizeBytes / (1024 * 1024)).toFixed(1)} MB`
      : null;

  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        padding: "12px 14px",
        background: "#f8fafc",
        borderRadius: "12px",
        boxSizing: "border-box",
      }}
    >
      <div style={{ minWidth: 0, flex: 1, marginRight: "12px" }}>
        <div
          style={{
            fontWeight: 600,
            fontSize: "13px",
            color: "#0f172a",
            whiteSpace: "nowrap",
            overflow: "hidden",
            textOverflow: "ellipsis",
          }}
          title={fileName}
        >
          {fileName}
        </div>
        <div
          style={{
            display: "flex",
            gap: "6px",
            marginTop: "6px",
            flexWrap: "wrap",
            alignItems: "center",
          }}
        >
          <Tag minimal intent="primary" round style={{ fontSize: "10px" }}>
            {formatTag.toUpperCase()}
          </Tag>
          {mimeType && (
            <Tag minimal intent="none" round style={{ fontSize: "10px" }}>
              {mimeType}
            </Tag>
          )}
          {dimensions && (
            <Tag minimal intent="none" round style={{ fontSize: "10px" }}>
              {dimensions[0]} × {dimensions[1]} px
            </Tag>
          )}
          {formattedSize && (
            <Tag minimal intent="none" round style={{ fontSize: "10px" }}>
              {formattedSize}
            </Tag>
          )}
          {bitDepth !== undefined && bitDepth !== null && (
            <Tag minimal intent="none" round style={{ fontSize: "10px" }}>
              {bitDepth}-bit
            </Tag>
          )}
          {hasAlpha !== undefined && (
            <Tag minimal intent={hasAlpha ? "warning" : "none"} round style={{ fontSize: "10px" }}>
              {hasAlpha ? "RGBA (Alpha)" : "RGB"}
            </Tag>
          )}
          {compression && (
            <Tag minimal intent="none" round style={{ fontSize: "10px" }}>
              {compression}
            </Tag>
          )}
        </div>
      </div>
      <Button
        small
        minimal
        icon="swap-horizontal"
        text="Ganti"
        disabled={disabled}
        onClick={onRemove}
        title="Ganti berkas"
        style={{
          borderRadius: "8px",
          background: "#e2e8f0",
          color: "#334155",
          fontWeight: 600,
          fontSize: "11px",
        }}
      />
    </div>
  );
};
