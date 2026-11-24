import React from "react";
import { Button } from "@blueprintjs/core";
import type { AnnotationFeature } from "../types/layer";
import type { AnnotationGeomType } from "../molecules/AnnotationForm";
import { AnnotationInteractiveBanner } from "../molecules/AnnotationInteractiveBanner";
import { AnnotationCardItem } from "../molecules/annotation/AnnotationCardItem";

export interface AnnotationListSectionProps {
  annotations: AnnotationFeature[];
  isPickingAnnotation: boolean;
  pickingGeomType: AnnotationGeomType;
  isAnnotationModalOpen: boolean;
  editingAnnotation: AnnotationFeature | null;
  hasMap: boolean;
  onOpenCreateModal: () => void;
  onFocusAnnotation: (annotation: AnnotationFeature) => void;
  onStartEditAnnotation: (annotation: AnnotationFeature) => void;
  onDeleteAnnotation: (id: string) => void;
}

export const AnnotationListSection: React.FC<AnnotationListSectionProps> = ({
  annotations,
  isPickingAnnotation,
  pickingGeomType,
  isAnnotationModalOpen,
  editingAnnotation,
  hasMap,
  onOpenCreateModal,
  onFocusAnnotation,
  onStartEditAnnotation,
  onDeleteAnnotation,
}) => {
  return (
    <div
      style={{
        marginTop: "20px",
        paddingTop: "16px",
        borderTop: "1px solid var(--color-hairline-silver)",
      }}
    >
      {/* Header Seksi Anotasi */}
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          paddingBottom: "8px",
          borderBottom: "1px solid rgba(16, 22, 26, 0.08)",
          marginBottom: "12px",
        }}
      >
        <div>
          <div
            style={{
              fontSize: "13px",
              fontWeight: 700,
              color: "var(--color-ink)",
            }}
          >
            Anotasi Spasial
          </div>
          <div style={{ fontSize: "11px", color: "var(--color-slate)" }}>
            {annotations.length} catatan inspeksi di peta
          </div>
        </div>
      </div>

      {/* Tombol Utama Catat Anotasi Spasial */}
      <Button
        fill
        icon="plus"
        text="Catat Anotasi Spasial"
        onClick={onOpenCreateModal}
        style={{
          borderRadius: "12px",
          background: "#0f172a",
          color: "#ffffff",
          fontWeight: 600,
          fontSize: "13px",
          padding: "10px 16px",
          border: "none",
          boxShadow: "0 2px 8px -1px rgba(15, 23, 42, 0.2)",
          marginBottom: "14px",
        }}
      />

      {/* Interactive Helper Banner saat Mode Menentukan Lokasi Anotasi di Peta */}
      {isPickingAnnotation && <AnnotationInteractiveBanner pickingGeomType={pickingGeomType} />}

      {/* Daftar Anotasi */}
      {annotations.length > 0 && (
        <div style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
          {annotations.map((annotation) => {
            const targetId = annotation.id || annotation.annotation_id || "";
            const isEditingThis =
              isAnnotationModalOpen &&
              editingAnnotation !== null &&
              (editingAnnotation.id || editingAnnotation.annotation_id) === targetId;

            return (
              <AnnotationCardItem
                key={targetId}
                annotation={annotation}
                isEditing={isEditingThis}
                hasMap={hasMap}
                onFocus={onFocusAnnotation}
                onEdit={onStartEditAnnotation}
                onDelete={onDeleteAnnotation}
              />
            );
          })}
        </div>
      )}
    </div>
  );
};
