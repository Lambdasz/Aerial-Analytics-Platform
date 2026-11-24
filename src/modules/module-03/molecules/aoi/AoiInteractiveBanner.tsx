import React from "react";
import { Button, Icon } from "@blueprintjs/core";

export interface AoiInteractiveBannerProps {
  isDrawing: boolean;
  isModifyingPolygon: boolean;
  onCancelDrawing: () => void;
  onCancelModifyPolygon: () => void;
  onFinishModifyPolygon: () => void;
}

export const AoiInteractiveBanner: React.FC<AoiInteractiveBannerProps> = ({
  isDrawing,
  isModifyingPolygon,
  onCancelDrawing,
  onCancelModifyPolygon,
  onFinishModifyPolygon,
}) => {
  if (isDrawing) {
    return (
      <div
        style={{
          background: "#0f172a",
          color: "#ffffff",
          padding: "14px 16px",
          borderRadius: "14px",
          display: "flex",
          flexDirection: "column",
          gap: "10px",
          boxShadow: "0 4px 16px rgba(15, 23, 42, 0.2)",
        }}
      >
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: "8px",
            fontSize: "13px",
            fontWeight: 600,
          }}
        >
          <Icon icon="locate" color="#38bdf8" size={16} />
          <span>Sedang Menggambar di Peta...</span>
        </div>
        <p style={{ fontSize: "11px", color: "#cbd5e1", margin: 0, lineHeight: 1.4 }}>
          Klik titik-titik sudut area di atas peta. Klik titik awal untuk menutup poligon.
        </p>
        <Button
          minimal
          small
          text="Batal Gambar"
          icon="cross"
          onClick={onCancelDrawing}
          style={{
            background: "rgba(255, 255, 255, 0.15)",
            color: "#ffffff",
            borderRadius: "8px",
            fontSize: "11px",
            fontWeight: 600,
            outline: "none",
          }}
        />
      </div>
    );
  }

  if (isModifyingPolygon) {
    return (
      <div
        style={{
          background: "#0f172a",
          color: "#ffffff",
          padding: "14px 16px",
          borderRadius: "14px",
          display: "flex",
          flexDirection: "column",
          gap: "10px",
          boxShadow: "0 4px 16px rgba(15, 23, 42, 0.2)",
        }}
      >
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: "8px",
            fontSize: "13px",
            fontWeight: 600,
          }}
        >
          <Icon icon="polygon-filter" color="#f59e0b" size={16} />
          <span>Ubah Bentuk Poligon di Peta</span>
        </div>
        <p style={{ fontSize: "11px", color: "#cbd5e1", margin: 0, lineHeight: 1.4 }}>
          Geser titik-titik sudut poligon di atas peta untuk menyesuaikan batas area.
        </p>
        <div style={{ display: "flex", gap: "8px" }}>
          <Button
            minimal
            small
            text="Batal"
            icon="cross"
            onClick={onCancelModifyPolygon}
            style={{
              background: "rgba(255, 255, 255, 0.15)",
              color: "#ffffff",
              borderRadius: "8px",
              fontSize: "11px",
              fontWeight: 600,
              outline: "none",
            }}
          />
          <Button
            minimal
            small
            text="Selesai & Buka Form"
            icon="tick"
            onClick={onFinishModifyPolygon}
            style={{
              background: "#38bdf8",
              color: "#0f172a",
              fontWeight: 700,
              borderRadius: "8px",
              fontSize: "11px",
              flex: 1,
              outline: "none",
            }}
          />
        </div>
      </div>
    );
  }

  return null;
};
