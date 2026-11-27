import React from "react";
import type * as L from "leaflet";
import { Dialog, DialogBody, DialogFooter, Button } from "@blueprintjs/core";
import type { StoredImageMetadata } from "../../types/storage";
import { useImportWorkflow } from "../../hooks/useImportWorkflow";
import { ImportTabBar } from "../../molecules/import/ImportTabBar";
import { ImportOptionATab } from "../../molecules/import/ImportOptionATab";
import { ImportOptionBTab } from "../../molecules/import/ImportOptionBTab";
import { ImportProgressModal } from "./ImportProgressModal";

export interface ImportImageModalProps {
  isOpen: boolean;
  onClose: () => void;
  onImportSuccess: (image: StoredImageMetadata) => void;
  map?: L.Map | null;
}

export const ImportImageModal: React.FC<ImportImageModalProps> = ({
  isOpen,
  onClose,
  onImportSuccess,
  map,
}) => {
  const workflow = useImportWorkflow({ map, onImportSuccess, onClose });

  const isSubmitDisabled =
    workflow.isStartingJob ||
    (workflow.activeTab === "A"
      ? !workflow.isFormValid
      : !workflow.isTiffFormValid || workflow.isInspectingTiff);

  const submitText = workflow.isStartingJob
    ? "Memulai..."
    : workflow.activeTab === "A"
      ? "Mulai Impor"
      : "Mulai Impor GeoTIFF";

  const handleSubmit = () => {
    void (workflow.activeTab === "A"
      ? workflow.handleStartImport()
      : workflow.handleStartImportTiff());
  };

  return (
    <>
      <Dialog
        isOpen={isOpen && !workflow.activeJobId}
        onClose={workflow.handleClose}
        title="Impor Citra Orthomosaic"
        icon="import"
        style={{ width: "580px", borderRadius: "16px" }}
        canOutsideClickClose={!workflow.isStartingJob}
      >
        <DialogBody>
          <ImportTabBar activeTab={workflow.activeTab} onTabChange={workflow.setActiveTab} />

          {workflow.activeTab === "A" && (
            <ImportOptionATab
              selectedFile={workflow.selectedFile}
              onSelectFile={workflow.handleSelectFile}
              onRemoveFile={workflow.handleRemoveFile}
              imgDimensions={workflow.imgDimensions}
              customName={workflow.customName}
              onCustomNameChange={workflow.setCustomName}
              saveOriginal={workflow.saveOriginal}
              onSaveOriginalChange={workflow.setSaveOriginal}
              useManualBounds={workflow.useManualBounds}
              onUseManualBoundsChange={workflow.setUseManualBounds}
              south={workflow.south}
              onSouthChange={workflow.setSouth}
              north={workflow.north}
              onNorthChange={workflow.setNorth}
              west={workflow.west}
              onWestChange={workflow.setWest}
              east={workflow.east}
              onEastChange={workflow.setEast}
              southError={workflow.southError}
              northError={workflow.northError}
              westError={workflow.westError}
              eastError={workflow.eastError}
              aspectRatioWarning={workflow.aspectRatioWarning}
              hasMap={Boolean(map)}
              onFillFromMap={workflow.handleFillFromMap}
              errorMessage={workflow.errorMessage}
              onError={(err) => workflow.setErrorMessage(err)}
              disabled={workflow.isStartingJob}
            />
          )}

          {workflow.activeTab === "B" && (
            <ImportOptionBTab
              selectedTiffFile={workflow.selectedTiffFile}
              onSelectTiffFile={(file) => void workflow.handleSelectTiffFile(file)}
              onRemoveTiffFile={workflow.handleRemoveTiffFile}
              isInspectingTiff={workflow.isInspectingTiff}
              tiffInspection={workflow.tiffInspection}
              customNameTiff={workflow.customNameTiff}
              onCustomNameTiffChange={workflow.setCustomNameTiff}
              saveOriginalTiff={workflow.saveOriginalTiff}
              onSaveOriginalTiffChange={workflow.setSaveOriginalTiff}
              tiffWest={workflow.tiffWest}
              onTiffWestChange={workflow.setTiffWest}
              tiffSouth={workflow.tiffSouth}
              onTiffSouthChange={workflow.setTiffSouth}
              tiffEast={workflow.tiffEast}
              onTiffEastChange={workflow.setTiffEast}
              tiffNorth={workflow.tiffNorth}
              onTiffNorthChange={workflow.setTiffNorth}
              tiffSouthError={workflow.tiffSouthError}
              tiffNorthError={workflow.tiffNorthError}
              tiffWestError={workflow.tiffWestError}
              tiffEastError={workflow.tiffEastError}
              isBboxEdited={workflow.isBboxEdited}
              onResetToDetectedBbox={workflow.handleResetToDetectedBbox}
              hasMap={Boolean(map)}
              onFillTiffFromMap={workflow.handleFillTiffFromMap}
              tiffError={workflow.tiffError}
              onError={(err) => workflow.setTiffError(err)}
              disabled={workflow.isStartingJob}
            />
          )}
        </DialogBody>

        <DialogFooter
          actions={
            <div style={{ display: "flex", gap: "8px", justifyContent: "flex-end" }}>
              <Button
                text="Batal"
                minimal
                disabled={workflow.isStartingJob}
                onClick={workflow.handleClose}
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
                icon="cloud-upload"
                text={submitText}
                disabled={isSubmitDisabled}
                loading={workflow.isStartingJob}
                onClick={handleSubmit}
                style={{
                  borderRadius: "999px",
                  background: isSubmitDisabled ? "#f1f5f9" : "#0f172a",
                  color: isSubmitDisabled ? "#94a3b8" : "#ffffff",
                  fontWeight: 600,
                  padding: "6px 18px",
                  boxShadow: isSubmitDisabled ? "none" : "0 2px 8px rgba(15,23,42,0.15)",
                }}
              />
            </div>
          }
        />
      </Dialog>

      <ImportProgressModal
        isOpen={Boolean(workflow.activeJobId)}
        jobId={workflow.activeJobId}
        onClose={() => workflow.setActiveJobId(null)}
        onSuccess={(id) => void workflow.handleJobSuccess(id)}
      />
    </>
  );
};
