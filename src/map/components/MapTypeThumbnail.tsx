import React from "react";

interface Props {
  activeBasemap: string;
  onToggle: () => void;
  isSidebarCollapsed?: boolean;
}

export const MapTypeThumbnail: React.FC<Props> = ({
  activeBasemap,
  onToggle,
  isSidebarCollapsed,
}) => {
  const isOsm = activeBasemap === "osm" || activeBasemap === "positron";
  // Gunakan tile yang lebih reliable untuk preview
  const bgImage = isOsm
    ? "url(https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/4/8/12)" // Satellite
    : "url(https://server.arcgisonline.com/ArcGIS/rest/services/World_Street_Map/MapServer/tile/4/8/12)"; // Street Map

  const label = isOsm ? "Satelit" : "Peta";

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onToggle}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onToggle();
        }
      }}
      style={{
        position: "absolute",
        bottom: "40px" /* Sejajar dengan legend button yang baru */,
        left: isSidebarCollapsed ? "24px" : "404px",
        transition:
          "left 0.3s cubic-bezier(0.4, 0.0, 0.2, 1), transform 0.2s cubic-bezier(0.4, 0, 0.2, 1), box-shadow 0.2s",
        zIndex: 1000,
        width: "64px",
        height: "64px",
        borderRadius: "50%",
        border: "3px solid white",
        boxShadow: "0 6px 16px rgba(0, 0, 0, 0.15), var(--shadow-subtle)",
        backgroundImage: bgImage,
        backgroundSize: "cover",
        overflow: "hidden",
        display: "flex",
        alignItems: "flex-end",
        justifyContent: "center",
        cursor: "pointer",
        backgroundColor: "var(--color-control-gray)",
      }}
      className="apple-button-hover"
    >
      <div
        style={{
          background: "rgba(255, 255, 255, 0.85)",
          backdropFilter: "blur(4px)",
          WebkitBackdropFilter: "blur(4px)",
          color: "var(--color-ink)",
          width: "100%",
          textAlign: "center",
          fontSize: "11px",
          fontWeight: 700,
          padding: "3px 0 2px 0",
          fontFamily: "var(--font-sf-pro-text)",
          textTransform: "uppercase",
          letterSpacing: "0.5px",
        }}
      >
        {label}
      </div>
    </div>
  );
};
