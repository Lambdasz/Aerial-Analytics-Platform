import React, { useState } from "react";
import { Icon } from "@blueprintjs/core";

export interface SafeThumbnailProps {
  src?: string;
  filename: string;
  format?: string;
  extension?: string;
}

export const SafeThumbnail: React.FC<SafeThumbnailProps> = ({
  src,
  filename,
  format,
  extension,
}) => {
  const [hasError, setHasError] = useState(false);

  if (!src || hasError) {
    return (
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
    );
  }

  return (
    <>
      <img src={src} alt={filename} loading="lazy" onError={() => setHasError(true)} />
      <span className="gallery-thumb-tag">{(extension || format || "IMG").toUpperCase()}</span>
    </>
  );
};
