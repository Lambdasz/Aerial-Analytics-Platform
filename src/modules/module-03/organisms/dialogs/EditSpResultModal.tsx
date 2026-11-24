import React from "react";
import { Dialog, Button, Classes, Callout, Spinner } from "@blueprintjs/core";
import type { SpatialResultSummary, UpdateSpatialResultPayload } from "../../types/spatialResult";
import type { LayerPayload } from "../../types/map";
import { SpResultTabBar } from "../../molecules/tabs/SpResultTabBar";
import { SpResultInfoTab } from "../../molecules/tabs/SpResultInfoTab";
import { SpResultFieldsTab } from "../../molecules/tabs/SpResultFieldsTab";
import { SpResultValuesTab } from "../../molecules/tabs/SpResultValuesTab";
import { SpResultPopupPreviewTab } from "../../molecules/tabs/SpResultPopupPreviewTab";
import { UnsavedChangesDialog } from "../../molecules/spatial/UnsavedChangesDialog";
import { useSpResultFormController } from "../../hooks/useSpResultFormController";

export interface EditSpResultModalProps {
  isOpen: boolean;
  item: SpatialResultSummary | null;
  onClose: () => void;
  onSave: (payload: UpdateSpatialResultPayload) => Promise<void>;
  onLoadDetail?: (id: string) => Promise<LayerPayload>;
  isSubmitting?: boolean;
}

interface EditSpResultFormProps {
  item: SpatialResultSummary;
  onClose: () => void;
  onSave: (payload: UpdateSpatialResultPayload) => Promise<void>;
  onLoadDetail?: (id: string) => Promise<LayerPayload>;
  isSubmitting?: boolean;
}

const EditSpResultForm: React.FC<EditSpResultFormProps> = ({
  item,
  onClose,
  onSave,
  onLoadDetail,
  isSubmitting = false,
}) => {
  const ctrl = useSpResultFormController({
    item,
    onClose,
    onSave,
    onLoadDetail,
    isSubmitting,
  });

  const isSaveDisabled = !ctrl.isDirty || isSubmitting || ctrl.isLoadingDetail;

  return (
    <form onSubmit={ctrl.handleSubmit}>
      <div
        className={Classes.DIALOG_BODY}
        style={{ minHeight: "320px", maxHeight: "72vh", overflowY: "auto", padding: "16px 20px" }}
      >
        {ctrl.errorMessage && (
          <Callout intent="danger" style={{ marginBottom: "14px" }}>
            {ctrl.errorMessage}
          </Callout>
        )}

        {ctrl.isLoadingDetail ? (
          <div
            style={{
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              justifyContent: "center",
              padding: "40px",
              gap: "10px",
            }}
          >
            <Spinner size={30} />
            <span style={{ fontSize: "12px", color: "#64748b" }}>
              Memuat data dan atribut layer...
            </span>
          </div>
        ) : (
          <>
            <SpResultTabBar
              selectedTab={ctrl.selectedTab}
              onSelectTab={ctrl.setSelectedTab}
              fieldsCount={ctrl.draftFields.length}
            />

            {ctrl.selectedTab === "general" && (
              <SpResultInfoTab
                item={item}
                name={ctrl.name}
                onNameChange={ctrl.setName}
                displayColor={ctrl.displayColor}
                onDisplayColorChange={ctrl.setDisplayColor}
                icon={ctrl.icon}
                onIconChange={ctrl.setIcon}
                imageAction={ctrl.imageAction}
                previewUrl={ctrl.previewUrl}
                selectedFileName={ctrl.selectedFileName}
                onSelectFile={() => void ctrl.handleSelectFile()}
                onDeletePhoto={ctrl.handleDeletePhoto}
                onResetPhoto={ctrl.handleResetPhoto}
                fileInputRef={ctrl.fileInputRef}
                onFileInputChange={ctrl.handleFileInputChange}
                isSubmitting={isSubmitting}
              />
            )}

            {ctrl.selectedTab === "fields" && (
              <SpResultFieldsTab
                fields={ctrl.draftFields}
                onChange={ctrl.setDraftFields}
                onDeleteCustomField={ctrl.handleDeleteCustomField}
                onAddField={ctrl.handleAddField}
              />
            )}

            {ctrl.selectedTab === "values" && (
              <SpResultValuesTab
                layerDetail={ctrl.layerDetail}
                fields={ctrl.draftFields}
                selectedIndex={ctrl.selectedFeatureIndex}
                onSelectIndex={ctrl.setSelectedFeatureIndex}
                draftSets={ctrl.draftSets}
                onUpdateValue={ctrl.handleUpdatePropertyValue}
              />
            )}

            {ctrl.selectedTab === "preview" && (
              <SpResultPopupPreviewTab
                layerName={ctrl.name.trim() || item.layer_name}
                displayColor={ctrl.displayColor}
                previewUrl={ctrl.previewUrl}
                imageAction={ctrl.imageAction}
                layerDetail={ctrl.layerDetail}
                selectedFeatureIndex={ctrl.selectedFeatureIndex}
                onSelectIndex={ctrl.setSelectedFeatureIndex}
                selectedFeature={ctrl.selectedFeature}
                fields={ctrl.draftFields}
                draftSets={ctrl.draftSets}
              />
            )}
          </>
        )}
      </div>

      <div className={Classes.DIALOG_FOOTER}>
        <div className={Classes.DIALOG_FOOTER_ACTIONS}>
          <Button
            text="Batal"
            minimal
            onClick={ctrl.handleRequestClose}
            disabled={isSubmitting}
            style={{
              borderRadius: "999px",
              background: "#f1f5f9",
              color: "#475569",
              fontWeight: 600,
              padding: "6px 16px",
            }}
          />
          <Button
            minimal
            text={isSubmitting ? "Menyimpan..." : "Simpan Perubahan"}
            type="submit"
            loading={isSubmitting}
            disabled={isSaveDisabled}
            style={{
              borderRadius: "999px",
              background: isSaveDisabled ? "#f1f5f9" : "#0f172a",
              color: isSaveDisabled ? "#94a3b8" : "#ffffff",
              fontWeight: 600,
              padding: "6px 18px",
              boxShadow: isSaveDisabled ? "none" : "0 2px 8px rgba(15,23,42,0.15)",
            }}
          />
        </div>
      </div>

      <UnsavedChangesDialog
        isOpen={ctrl.showCloseConfirm}
        onCancel={() => ctrl.setShowCloseConfirm(false)}
        onConfirmDiscard={() => {
          ctrl.setShowCloseConfirm(false);
          onClose();
        }}
      />
    </form>
  );
};

export const EditSpResultModal: React.FC<EditSpResultModalProps> = ({
  isOpen,
  item,
  onClose,
  onSave,
  onLoadDetail,
  isSubmitting = false,
}) => {
  if (!isOpen || !item) return null;

  return (
    <Dialog
      isOpen={isOpen}
      onClose={onClose}
      title={`Edit Layer Hasil Analisis: ${item.layer_name || item.id}`}
      style={{ width: "720px", maxWidth: "94vw", borderRadius: "16px" }}
    >
      <EditSpResultForm
        key={item.id}
        item={item}
        onClose={onClose}
        onSave={onSave}
        onLoadDetail={onLoadDetail}
        isSubmitting={isSubmitting}
      />
    </Dialog>
  );
};
