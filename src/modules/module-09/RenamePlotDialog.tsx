import { useState } from "react";
import { Button, Dialog, InputGroup } from "@blueprintjs/core";

import { errorMessage } from "./plotAnalytics";
import { renamePlot, type PlotRecord } from "./plotCommands";

/**
 * Rename dialog for the Plots tab (FR-9.1.3). Empty names are rejected both
 * here (Save disabled) and by the backend, whose message is shown on failure.
 * Mount it with `key={plot.id}` so the input starts from the current name.
 */
export function RenamePlotDialog({
  plot,
  onClose,
  onRenamed,
}: {
  plot: PlotRecord | null;
  onClose: () => void;
  onRenamed: (plot: PlotRecord) => void;
}) {
  const [value, setValue] = useState(plot?.name ?? "");
  const [error, setError] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const isEmpty = value.trim().length === 0;

  const save = (target: PlotRecord) => {
    setIsSaving(true);
    renamePlot(target.id, value)
      .then((renamed) => {
        setError(null);
        onRenamed(renamed);
      })
      .catch((err) => setError(errorMessage(err)))
      .finally(() => setIsSaving(false));
  };

  return (
    <Dialog
      isOpen={plot !== null}
      onClose={onClose}
      title="Rename plot"
      style={{ width: 420, borderRadius: 16, background: "#ffffff" }}
    >
      <div className="bp6-dialog-body" style={{ padding: "16px 20px" }}>
        <InputGroup
          value={value}
          onChange={(event) => setValue(event.target.value)}
          onKeyDown={(event) =>
            event.key === "Enter" && plot !== null && !isEmpty && !isSaving && save(plot)
          }
          placeholder="Plot name"
        />
        {isEmpty && (
          <div style={{ marginTop: 6, fontSize: 11, color: "#b42318" }}>
            Plot name must not be empty.
          </div>
        )}
        {error && <div style={{ marginTop: 6, fontSize: 11, color: "#b42318" }}>{error}</div>}
      </div>
      <div className="bp6-dialog-footer">
        <div className="bp6-dialog-footer-actions">
          <Button
            minimal
            text="Cancel"
            onClick={onClose}
            style={{ borderRadius: 10, background: "#f1f5f9", color: "#475569", fontWeight: 600 }}
          />
          <Button
            minimal
            text="Save"
            loading={isSaving}
            disabled={isEmpty || plot === null}
            onClick={() => plot !== null && save(plot)}
            style={{
              borderRadius: 10,
              background: isEmpty ? "#f1f5f9" : "#0f172a",
              color: isEmpty ? "#94a3b8" : "#ffffff",
              fontWeight: 600,
            }}
          />
        </div>
      </div>
    </Dialog>
  );
}
