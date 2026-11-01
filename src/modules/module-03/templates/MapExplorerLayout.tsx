import React from "react";

export interface MapExplorerLayoutProps {
  /** Slot untuk panel kiri (sidebar tab: Layers, AOI, Gallery) */
  sidebar: React.ReactNode;
  /** Slot untuk tombol thumbnail beralih basemap (kiri bawah) */
  thumbnail: React.ReactNode;
  /** Slot untuk legenda hasil spasial (kanan bawah) */
  legend: React.ReactNode;
  /** Slot untuk kanvas mesin peta Leaflet (MapContainer & controls) */
  map: React.ReactNode;
  /** Nama kelas CSS opsional untuk kontainer pembungkus */
  className?: string;
  /** Style inline opsional */
  style?: React.CSSProperties;
}

/**
 * Template tata letak Map Explorer.
 *
 * Murni mengatur layout positioning flex dan absolute:
 * - Panel kiri mengambang di atas peta (zIndex: 1000)
 * - Area peta mengisi 100% lebar dan tinggi
 * - Widget thumbnail dan legenda diposisikan di atas peta
 */
export const MapExplorerLayout: React.FC<MapExplorerLayoutProps> = ({
  sidebar,
  thumbnail,
  legend,
  map,
  className,
  style,
}) => {
  return (
    <div
      className={className ? `map-wrapper ${className}` : "map-wrapper"}
      style={{ position: "relative", width: "100%", height: "100%", overflow: "hidden", ...style }}
    >
      {/* =========================
          LEFT SIDEBAR SLOT
          ========================= */}
      <div
        style={{
          position: "absolute",
          top: 0,
          left: 0,
          height: "100%",
          zIndex: 1000,
          pointerEvents: "none",
        }}
      >
        {sidebar}
      </div>

      {/* =========================
          MAP & FLOATING CONTROLS SLOT
          ========================= */}
      <div style={{ width: "100%", height: "100%", position: "absolute", top: 0, left: 0 }}>
        {thumbnail}
        {legend}
        {map}
      </div>
    </div>
  );
};
