import React from "react";
import * as L from "leaflet";
import { Dialog, Classes } from "@blueprintjs/core";
import type { AnnotationFeature, SpatialGeometry } from "../../types/layer";
import { AnnotationForm } from "../../molecules/AnnotationForm";
import { useAnnotationModalForm } from "../../hooks/useAnnotationModalForm";
import type { AnnotationGeomType } from "../../hooks/useAnnotationMapPicker";

export interface AnnotationModalProps {
  isOpen: boolean;
  onClose: () => void;
  map: L.Map | null;
  initialAnnotation?: AnnotationFeature | null;
  onSaveAnnotation: (args: { text: string; geometry: SpatialGeometry }) => Promise<void>;
  onDrawingStateChange?: (isPicking: boolean, geomType: AnnotationGeomType) => void;
}

/**
 * Modal dialog mandiri untuk pembuatan dan pengeditan Anotasi Spasial.
 * Menggunakan hook useAnnotationMapPicker untuk menjembatani event peta secara mulus.
 */
export const AnnotationModal: React.FC<AnnotationModalProps> = (props) => {
  const { isTemporarilyClosedForMap, isSubmitting, handleModalClose, formProps } =
    useAnnotationModalForm(props);

  return (
    <Dialog
      isOpen={props.isOpen && !isTemporarilyClosedForMap}
      onClose={handleModalClose}
      canEscapeKeyClose={!isSubmitting}
      canOutsideClickClose={!isSubmitting}
      style={{
        width: "480px",
        maxWidth: "92vw",
        borderRadius: "16px",
        overflow: "hidden",
      }}
    >
      <div className={Classes.DIALOG_BODY} style={{ padding: "24px 24px 16px 24px" }}>
        <AnnotationForm {...formProps} onCancel={handleModalClose} />
      </div>
    </Dialog>
  );
};
