import React from "react";
import { Button, Icon } from "@blueprintjs/core";
import type { AnnotationGeomType } from "./AnnotationForm";

export interface AnnotationInteractiveBannerProps {
  pickingGeomType: AnnotationGeomType;
  onCancel?: () => void;
  onFinish?: () => void;
}

export const AnnotationInteractiveBanner: React.FC<AnnotationInteractiveBannerProps> = ({
  pickingGeomType,
  onCancel,
  onFinish,
}) => {
  const handleCancel = () => {
    if (onCancel) {
      onCancel();
    } else {
      window.dispatchEvent(new CustomEvent("annotation:cancel-picking"));
    }
  };

  const handleFinish = () => {
    if (onFinish) {
      onFinish();
    } else {
      window.dispatchEvent(new CustomEvent("annotation:finish-picking"));
    }
  };

  return (
    <div
      style={{
        background: "#0f172a",
        borderRadius: "12px",
        padding: "12px 14px",
        color: "#ffffff",
        boxShadow: "0 4px 12px rgba(15, 23, 42, 0.25)",
        display: "flex",
        flexDirection: "column",
        gap: "8px",
        marginBottom: "14px",
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
        <Icon icon="locate" color="#38bdf8" size={16} />
        <span style={{ fontSize: "12px", fontWeight: 600 }}>Mode Menentukan Lokasi Anotasi</span>
      </div>
      <p style={{ fontSize: "11px", color: "#94a3b8", margin: 0, lineHeight: 1.4 }}>
        {pickingGeomType === "Point"
          ? "Klik salah satu titik di atas kanvas peta untuk mencatat koordinat anotasi."
          : pickingGeomType === "LineString"
            ? "Klik titik-titik di atas kanvas peta untuk menggambar garis anotasi."
            : "Klik titik-titik sudut di atas kanvas peta untuk menggambar poligon anotasi."}
      </p>
      <div style={{ display: "flex", gap: "6px", marginTop: "2px" }}>
        <Button
          small
          text="Batal"
          onClick={handleCancel}
          style={{
            flex: 1,
            background: "#334155",
            color: "#ffffff",
            border: "none",
            borderRadius: "6px",
            fontSize: "11px",
          }}
        />
        <Button
          small
          text="Selesai & Buka Form"
          onClick={handleFinish}
          style={{
            flex: 1,
            background: "#38bdf8",
            color: "#0f172a",
            border: "none",
            fontWeight: 600,
            borderRadius: "6px",
            fontSize: "11px",
          }}
        />
      </div>
    </div>
  );
};
