import React, { useState } from "react";
import { Icon } from "@blueprintjs/core";
import { resolveImageSrc } from "../lib/fieldResolution";

export interface ResultPhotoProps {
  resultId: string;
  imageUrl?: string | null;
  size?: "small" | "medium" | "large";
  alt?: string;
  onClick?: () => void;
}

export const ResultPhoto: React.FC<ResultPhotoProps> = ({
  resultId,
  imageUrl,
  size = "medium",
  alt = "Foto Hasil Analisis",
  onClick,
}) => {
  const resolvedSrc = resolveImageSrc(imageUrl);
  const currentKey = `${resultId}_${resolvedSrc || ""}`;
  const [failedKey, setFailedKey] = useState<string | null>(null);
  const hasFailed = failedKey === currentKey;

  const dimensions =
    size === "small"
      ? { width: 44, height: 44, iconSize: 18, fontSize: 10 }
      : size === "large"
        ? { width: 140, height: 100, iconSize: 28, fontSize: 12 }
        : { width: 68, height: 52, iconSize: 22, fontSize: 11 };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (onClick && (e.key === "Enter" || e.key === " ")) {
      e.preventDefault();
      onClick();
    }
  };

  // 1. Kondisi: Tidak ada gambar atau gambar gagal dimuat -> Placeholder "No Image"
  if (!imageUrl || hasFailed) {
    return (
      <div
        className="result-photo-placeholder no-image"
        onClick={onClick}
        onKeyDown={onClick ? handleKeyDown : undefined}
        role={onClick ? "button" : undefined}
        tabIndex={onClick ? 0 : undefined}
        style={{
          width: dimensions.width,
          height: dimensions.height,
          borderRadius: 6,
          background: "rgba(143, 153, 168, 0.12)",
          border: "1px dashed rgba(143, 153, 168, 0.3)",
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          justifyContent: "center",
          color: "#8a9ba8",
          flexShrink: 0,
          cursor: onClick ? "pointer" : "default",
          userSelect: "none",
        }}
        title={hasFailed ? "Foto tidak dapat dimuat" : "Tidak ada foto"}
      >
        <Icon icon="media" size={dimensions.iconSize} />
        {size !== "small" && (
          <span style={{ fontSize: dimensions.fontSize, marginTop: 2, fontWeight: 500 }}>
            {hasFailed ? "Gagal" : "No Image"}
          </span>
        )}
      </div>
    );
  }

  // 2. Kondisi: Foto tersedia -> Tampilkan gambar
  return (
    <div
      className="result-photo-container"
      onClick={onClick}
      onKeyDown={onClick ? handleKeyDown : undefined}
      role={onClick ? "button" : undefined}
      tabIndex={onClick ? 0 : undefined}
      style={{
        width: dimensions.width,
        height: dimensions.height,
        borderRadius: 6,
        overflow: "hidden",
        border: "1px solid rgba(16, 22, 26, 0.15)",
        background: "#000",
        flexShrink: 0,
        position: "relative",
        cursor: onClick ? "pointer" : "default",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
      }}
    >
      <img
        src={resolvedSrc || ""}
        alt={alt}
        loading="lazy"
        onError={() => setFailedKey(currentKey)}
        style={{
          width: "100%",
          height: "100%",
          objectFit: "cover",
          display: "block",
        }}
      />
    </div>
  );
};
