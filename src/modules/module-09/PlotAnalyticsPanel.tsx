import { useEffect, useState, type CSSProperties, type KeyboardEvent } from "react";
import { Button, Checkbox, Dialog, Icon, NonIdealState, Spinner } from "@blueprintjs/core";
import { open } from "@tauri-apps/plugin-dialog";

import { getSavedAois } from "../module-03/services/api";
import type { AoiFeature } from "../module-03/types/aoi";
import { AnalysePlotsDialog } from "./AnalysePlotsDialog";
import { PlotRankingCard } from "./PlotRankingCard";
import { PlotSummaryDialog } from "./PlotSummaryDialog";
import { RenamePlotDialog } from "./RenamePlotDialog";
import { errorMessage, humanizeIndicator, toggleMember, withMember } from "./plotAnalytics";
import {
  createPlotsFromAois,
  currentProjectId,
  deletePlot,
  formatArea,
  formatIndicator,
  formatPerimeter,
  importPlots,
  listPlotIndicators,
  listPlots,
  publishPlotsLayer,
  type PlotIndicator,
  type PlotRecord,
} from "./plotCommands";

/** Everything the tab shows, loaded together. */
interface PanelData {
  plots: PlotRecord[];
  indicators: PlotIndicator[];
  aois: AoiFeature[];
}

const fetchPanelData = (projectId: string | null): Promise<PanelData> =>
  Promise.all([
    listPlots(projectId),
    listPlotIndicators(null, projectId),
    getSavedAois().catch(() => [] as AoiFeature[]),
  ]).then(([plots, indicators, aois]) => ({ plots, indicators, aois }));

/** First two indicators of a plot, plus how many more the summary holds. */
const latestFor = (indicators: readonly PlotIndicator[], plotId: string): string => {
  const rows = indicators.filter((row) => row.plotId === plotId);
  const extra = rows.length > 2 ? [`+${rows.length - 2} more`] : [];
  return [
    ...rows
      .slice(0, 2)
      .map((row) => `${humanizeIndicator(row.indicator)}: ${formatIndicator(row)}`),
    ...extra,
  ].join(" · ");
};

const ACTIVATION_KEYS: ReadonlySet<string> = new Set(["Enter", " "]);

const GEOJSON_FILTERS = [{ name: "GeoJSON plots", extensions: ["geojson", "json"] }];

const primaryButton = (enabled: boolean, color: string): CSSProperties => ({
  flex: 1,
  borderRadius: 10,
  background: enabled ? color : "#f1f5f9",
  color: enabled ? "#ffffff" : "#94a3b8",
  fontWeight: 600,
  fontSize: 12,
  padding: "6px 12px",
  boxShadow: "none",
  outline: "none",
});

const secondaryButton = (enabled: boolean): CSSProperties => ({
  flex: 1,
  borderRadius: 10,
  background: "#ffffff",
  color: enabled ? "#0f172a" : "#94a3b8",
  fontWeight: 600,
  fontSize: 12,
  padding: "6px 12px",
  boxShadow: "0 1px 3px rgba(0,0,0,0.05)",
  outline: "none",
});

/**
 * Plots tab for the Map Explorer sidebar — the official Module 9 UI (KD-1).
 *
 * Turn saved AOIs into plots or import a GeoJSON file, rename plots, delete
 * them after confirmation, select all or some and run a plugin per plot from
 * an orthomosaic, compare/rank indicator values with summary statistics, open
 * a per-plot summary, publish the boundaries as a map layer, and refresh the
 * list. It replaces the former standalone Module 9 page (ISU-9.1). Pure
 * helpers live in `plotAnalytics.ts`.
 */
export function PlotAnalyticsPanel() {
  const [plots, setPlots] = useState<PlotRecord[]>([]);
  const [indicators, setIndicators] = useState<PlotIndicator[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [skipped, setSkipped] = useState<string[]>([]);

  const [isAoiOpen, setIsAoiOpen] = useState(false);
  const [savedAois, setSavedAois] = useState<AoiFeature[]>([]);
  const [checkedAois, setCheckedAois] = useState<Set<string>>(new Set());
  const [isAnalyseOpen, setIsAnalyseOpen] = useState(false);
  const [isImportingAois, setIsImportingAois] = useState(false);
  const [isImportingFile, setIsImportingFile] = useState(false);

  const [renameTarget, setRenameTarget] = useState<PlotRecord | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<PlotRecord | null>(null);
  const [summaryPlot, setSummaryPlot] = useState<PlotRecord | null>(null);

  // Success notices disappear on their own.
  useEffect(() => {
    const timer = notice ? setTimeout(() => setNotice(null), 4000) : undefined;
    return () => clearTimeout(timer);
  }, [notice]);

  const applyData = (data: PanelData) => {
    setPlots(data.plots);
    setIndicators(data.indicators);
    setSavedAois(data.aois);
  };

  const load = (): Promise<void> =>
    fetchPanelData(currentProjectId())
      .then((data) => {
        applyData(data);
        setError(null);
      })
      .catch((err) => setError(errorMessage(err)))
      .finally(() => setIsLoading(false));

  // Manual reload: plots, indicators and saved AOIs may change in other tabs.
  const reload = () => {
    setIsLoading(true);
    return load();
  };

  // Initial load from the app database; state updates happen in callbacks.
  useEffect(() => {
    const controller = new AbortController();
    fetchPanelData(currentProjectId())
      .then((data) => !controller.signal.aborted && applyData(data))
      .catch((err) => !controller.signal.aborted && setError(errorMessage(err)))
      .finally(() => !controller.signal.aborted && setIsLoading(false));
    return () => controller.abort();
  }, []);

  const openAoiPicker = () => {
    setIsAoiOpen(true);
    setCheckedAois(new Set());
    getSavedAois()
      .then((aois) => {
        setSavedAois(aois);
        setCheckedAois(new Set(aois.map((aoi) => aoi.properties.aoi_id)));
      })
      .catch((err) => setError(errorMessage(err)));
  };

  const createFromAois = (
    aois: AoiFeature[] = savedAois.filter((aoi) => checkedAois.has(aoi.properties.aoi_id)),
  ) => {
    setIsImportingAois(true);
    return createPlotsFromAois(aois).then((failures) => {
      setIsImportingAois(false);
      setIsAoiOpen(false);
      setError((previous) => (failures.length > 0 ? failures.join("; ") : previous));
      return load();
    });
  };

  // FR-9.1.2: invalid features are reported, the valid ones are still stored.
  const importFile = (path: string) => {
    setIsImportingFile(true);
    return importPlots(path, currentProjectId())
      .then((outcome) => {
        setSkipped(outcome.skipped);
        setNotice(`${outcome.plots.length} plot${outcome.plots.length === 1 ? "" : "s"} imported`);
        return load();
      })
      .catch((err) => setError(errorMessage(err)))
      .finally(() => setIsImportingFile(false));
  };

  const importGeoJson = () => {
    setError(null);
    setSkipped([]);
    open({ multiple: false, directory: false, filters: GEOJSON_FILTERS })
      .then((chosen) => (typeof chosen === "string" ? importFile(chosen) : undefined))
      .catch((err) => setError(errorMessage(err)));
  };

  // Called from the confirmation dialog only: indicators of the plot go with it.
  const removePlot = (plot: PlotRecord) =>
    deletePlot(plot.id)
      .then(() => {
        setSelected((previous) => withMember(previous, plot.id, false));
        return load();
      })
      .catch((err) => setError(errorMessage(err)))
      .finally(() => setDeleteTarget(null));

  const showOnMap = () =>
    publishPlotsLayer(currentProjectId(), "Plots")
      .then((layer) => setNotice(`Layer "${layer.layer_name}" published`))
      .catch((err) => setError(errorMessage(err)));

  const toggleAoi = (aoiId: string) => setCheckedAois((previous) => toggleMember(previous, aoiId));

  const activateAoi = (event: KeyboardEvent, aoiId: string) => {
    event.preventDefault();
    toggleAoi(aoiId);
  };

  const selectedPlots = plots.filter((plot) => selected.has(plot.id));
  const linkedAoiIds = new Set(plots.map((plot) => plot.sourceAoiId).filter(Boolean));
  const pendingAois = savedAois.filter((aoi) => !linkedAoiIds.has(aoi.properties.aoi_id));
  const hasSelection = selectedPlots.length > 0;
  const allSelected = plots.length > 0 && selectedPlots.length === plots.length;

  const selectAll = (checked: boolean) =>
    setSelected(checked ? new Set(plots.map((plot) => plot.id)) : new Set<string>());

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
      {/* Unified Page Header */}
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          paddingBottom: "8px",
          borderBottom: "1px solid rgba(16, 22, 26, 0.08)",
          marginBottom: "2px",
        }}
      >
        <span style={{ fontSize: "14px", fontWeight: 700, color: "#0f172a" }}>Analisis Plot</span>
        <div style={{ display: "flex", alignItems: "center", gap: 4 }}>
          <span style={{ fontSize: "11px", color: "#64748b", fontWeight: 500 }}>
            {plots.length} Plot
          </span>
          <Button
            minimal
            small
            icon="refresh"
            title="Refresh plots"
            loading={isLoading}
            onClick={() => void reload()}
            style={{ color: "#64748b", borderRadius: 6, outline: "none" }}
          />
        </div>
      </div>

      <div style={{ display: "flex", gap: 6 }}>
        <Button
          minimal
          small
          icon="selection"
          text="From AOI"
          onClick={openAoiPicker}
          style={primaryButton(true, "#0f172a")}
        />
        <Button
          minimal
          small
          icon="import"
          text="Import GeoJSON"
          loading={isImportingFile}
          onClick={importGeoJson}
          style={secondaryButton(true)}
        />
      </div>
      <div style={{ display: "flex", gap: 6 }}>
        <Button
          minimal
          small
          icon="chart"
          text={`Analyse (${selectedPlots.length})`}
          disabled={!hasSelection}
          onClick={() => setIsAnalyseOpen(true)}
          style={primaryButton(hasSelection, "#2563eb")}
        />
        <Button
          minimal
          small
          icon="map"
          text="On map"
          disabled={plots.length === 0}
          onClick={() => void showOnMap()}
          style={secondaryButton(plots.length > 0)}
        />
      </div>

      {pendingAois.length > 0 && (
        <div
          style={{
            background: "#eff6ff",
            border: "1px solid #dbeafe",
            borderRadius: 12,
            padding: "10px 14px",
            color: "#1e40af",
            fontSize: 12,
            fontWeight: 500,
          }}
        >
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: 6,
              fontWeight: 600,
              marginBottom: 4,
            }}
          >
            <Icon icon="info-sign" size={14} color="#2563eb" />
            <span>
              {`${pendingAois.length} saved AOI${pendingAois.length === 1 ? "" : "s"} not added as plot${
                pendingAois.length === 1 ? "" : "s"
              }`}
            </span>
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: 8, marginTop: 4 }}>
            <span style={{ flex: 1, fontSize: 11, color: "#3b82f6" }}>
              AOIs come from the AOI tab; add them here to analyse and rank them.
            </span>
            <Button
              minimal
              small
              text="Add all"
              loading={isImportingAois}
              onClick={() => void createFromAois(pendingAois)}
              style={{
                background: "#2563eb",
                color: "#ffffff",
                borderRadius: 8,
                fontSize: 11,
                fontWeight: 600,
                padding: "4px 10px",
                outline: "none",
              }}
            />
          </div>
        </div>
      )}

      {error && (
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            background: "#fef2f2",
            border: "1px solid #fecaca",
            color: "#991b1b",
            borderRadius: 12,
            padding: "8px 12px",
            fontSize: 12,
            fontWeight: 500,
            gap: 8,
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: 8, flex: 1 }}>
            <Icon icon="error" size={14} color="#dc2626" />
            <span>{error}</span>
          </div>
          <Button
            minimal
            small
            icon="cross"
            onClick={() => setError(null)}
            style={{ color: "#991b1b", padding: 2, minWidth: 20, minHeight: 20, outline: "none" }}
          />
        </div>
      )}

      {skipped.length > 0 && (
        <div
          style={{
            background: "#fffbeb",
            border: "1px solid #fde68a",
            color: "#92400e",
            borderRadius: 12,
            padding: "8px 12px",
            fontSize: 12,
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: 8, fontWeight: 600 }}>
            <Icon icon="warning-sign" size={14} color="#d97706" />
            <span style={{ flex: 1 }}>
              {skipped.length} feature{skipped.length === 1 ? "" : "s"} skipped
            </span>
            <Button
              minimal
              small
              icon="cross"
              onClick={() => setSkipped([])}
              style={{ color: "#92400e", padding: 2, minWidth: 20, minHeight: 20, outline: "none" }}
            />
          </div>
          <ul style={{ margin: "4px 0 0", paddingLeft: 18, fontSize: 11 }}>
            {skipped.map((reason) => (
              <li key={reason}>{reason}</li>
            ))}
          </ul>
        </div>
      )}

      {notice && (
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            background: "#f0fdf4",
            border: "1px solid #bbf7d0",
            color: "#166534",
            borderRadius: 12,
            padding: "8px 12px",
            fontSize: 12,
            fontWeight: 500,
            gap: 8,
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: 8, flex: 1 }}>
            <Icon icon="tick-circle" size={14} color="#16a34a" />
            <span>{notice}</span>
          </div>
          <Button
            minimal
            small
            icon="cross"
            onClick={() => setNotice(null)}
            style={{ color: "#166534", padding: 2, minWidth: 20, minHeight: 20, outline: "none" }}
          />
        </div>
      )}

      {isLoading && (
        <div style={{ padding: 24, textAlign: "center" }}>
          <Spinner size={24} />
        </div>
      )}

      {!isLoading && plots.length === 0 && (
        <NonIdealState
          icon="polygon-filter"
          title="No plots"
          description="Draw and save an AOI on the map, then add it here with From AOI — or import a GeoJSON file."
          action={<Button small icon="selection" text="From AOI" onClick={openAoiPicker} />}
        />
      )}

      {!isLoading && plots.length > 0 && (
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          <Checkbox
            checked={allSelected}
            indeterminate={hasSelection && !allSelected}
            label="Select all"
            onChange={(event) => selectAll(event.currentTarget.checked)}
            style={{ marginBottom: 0, paddingLeft: 42, fontSize: 12, color: "#64748b" }}
          />
          {plots.map((plot) => (
            <div
              key={plot.id}
              style={{
                background: "var(--color-gallery-white, #ffffff)",
                border: "none",
                borderRadius: 16,
                padding: "14px 16px",
                boxShadow: "0 2px 12px -2px rgba(0, 0, 0, 0.05), 0 1px 3px rgba(0, 0, 0, 0.02)",
              }}
            >
              <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                <Checkbox
                  checked={selected.has(plot.id)}
                  onChange={(event) => {
                    const checked = event.currentTarget.checked;
                    setSelected((previous) => withMember(previous, plot.id, checked));
                  }}
                  style={{ marginBottom: 0 }}
                />
                <div style={{ flex: 1, minWidth: 0 }}>
                  <button
                    type="button"
                    title="Show plot summary"
                    onClick={() => setSummaryPlot(plot)}
                    style={{
                      display: "block",
                      maxWidth: "100%",
                      padding: 0,
                      border: "none",
                      background: "none",
                      cursor: "pointer",
                      textAlign: "left",
                      fontSize: 13,
                      fontWeight: 600,
                      color: "var(--color-ink, #182026)",
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                      whiteSpace: "nowrap",
                    }}
                  >
                    {plot.name}
                  </button>
                  <div style={{ fontSize: 11, color: "var(--color-slate, #64748b)", marginTop: 2 }}>
                    {formatArea(plot.areaM2, plot.areaHa)} · {formatPerimeter(plot.perimeterM)}
                  </div>
                </div>
                <Button
                  minimal
                  small
                  icon="document"
                  style={{ color: "var(--color-slate, #64748b)", borderRadius: 6 }}
                  title="Plot summary"
                  onClick={() => setSummaryPlot(plot)}
                />
                <Button
                  minimal
                  small
                  icon="edit"
                  style={{ color: "var(--color-slate, #64748b)", borderRadius: 6 }}
                  title="Rename plot"
                  onClick={() => setRenameTarget(plot)}
                />
                <Button
                  minimal
                  small
                  icon="trash"
                  style={{
                    color: "var(--color-slate, #64748b)",
                    borderRadius: 6,
                  }}
                  title="Delete plot"
                  onClick={() => setDeleteTarget(plot)}
                />
              </div>
              <div
                style={{
                  fontSize: 11,
                  color: "var(--color-slate)",
                  marginTop: 4,
                  paddingLeft: 24,
                }}
              >
                {latestFor(indicators, plot.id) || "no indicators yet"}
              </div>
            </div>
          ))}
        </div>
      )}

      <PlotRankingCard plots={plots} indicators={indicators} />

      <Dialog
        isOpen={isAoiOpen}
        onClose={() => setIsAoiOpen(false)}
        title="Add plots from saved AOIs"
        style={{ width: 480, borderRadius: 16, background: "#ffffff" }}
      >
        <div
          className="bp6-dialog-body"
          style={{ maxHeight: 380, overflowY: "auto", padding: "16px 20px" }}
        >
          {savedAois.length === 0 && (
            <div
              style={{
                background: "#f8fafc",
                borderRadius: 12,
                padding: "14px 16px",
                fontSize: 12,
                color: "#64748b",
              }}
            >
              No AOIs saved yet. Draw an area in the AOI tab, save it, then return here.
            </div>
          )}
          {savedAois.map((aoi) => {
            const aoiId = aoi.properties.aoi_id;
            const isChecked = checkedAois.has(aoiId);
            return (
              <div
                key={aoiId}
                role="button"
                tabIndex={0}
                onClick={() => toggleAoi(aoiId)}
                onKeyDown={(event) => ACTIVATION_KEYS.has(event.key) && activateAoi(event, aoiId)}
                style={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "space-between",
                  background: isChecked ? "#f1f5f9" : "#f8fafc",
                  borderRadius: 10,
                  padding: "10px 14px",
                  marginBottom: 6,
                  cursor: "pointer",
                  transition: "background 0.15s ease",
                  outline: "none",
                }}
              >
                <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                  <Checkbox
                    checked={isChecked}
                    onClick={(e) => e.stopPropagation()}
                    onChange={(event) => {
                      const checked = event.currentTarget.checked;
                      setCheckedAois((previous) => withMember(previous, aoiId, checked));
                    }}
                    style={{ marginBottom: 0 }}
                  />
                  <span style={{ fontWeight: 600, color: "#0f172a", fontSize: 13 }}>
                    {aoi.properties.name}
                  </span>
                </div>
                <span style={{ fontSize: 12, color: "#64748b", fontWeight: 500 }}>
                  {aoi.properties.calculated_area_sq_m.toFixed(0)} m²
                </span>
              </div>
            );
          })}
        </div>
        <div className="bp6-dialog-footer">
          <div className="bp6-dialog-footer-actions">
            <Button
              minimal
              text="Cancel"
              onClick={() => setIsAoiOpen(false)}
              style={{
                borderRadius: 10,
                background: "#f1f5f9",
                color: "#475569",
                fontWeight: 600,
                padding: "7px 14px",
              }}
            />
            <Button
              minimal
              text={`Add ${checkedAois.size}`}
              loading={isImportingAois}
              disabled={checkedAois.size === 0}
              onClick={() => void createFromAois()}
              style={{
                borderRadius: 10,
                background: checkedAois.size === 0 ? "#f1f5f9" : "#0f172a",
                color: checkedAois.size === 0 ? "#94a3b8" : "#ffffff",
                fontWeight: 600,
                padding: "7px 16px",
              }}
            />
          </div>
        </div>
      </Dialog>

      <RenamePlotDialog
        key={renameTarget?.id ?? "none"}
        plot={renameTarget}
        onClose={() => setRenameTarget(null)}
        onRenamed={() => {
          setRenameTarget(null);
          void load();
        }}
      />

      <Dialog
        isOpen={deleteTarget !== null}
        onClose={() => setDeleteTarget(null)}
        title="Delete plot"
        style={{ width: 420, borderRadius: 16, background: "#ffffff" }}
      >
        <div className="bp6-dialog-body" style={{ padding: "16px 20px", fontSize: 13 }}>
          Delete <strong>{deleteTarget?.name}</strong>? Indicators computed for it are discarded.
        </div>
        <div className="bp6-dialog-footer">
          <div className="bp6-dialog-footer-actions">
            <Button
              minimal
              text="Cancel"
              onClick={() => setDeleteTarget(null)}
              style={{ borderRadius: 10, background: "#f1f5f9", color: "#475569", fontWeight: 600 }}
            />
            <Button
              minimal
              text="Delete"
              onClick={() => void (deleteTarget && removePlot(deleteTarget))}
              style={{ borderRadius: 10, background: "#dc2626", color: "#ffffff", fontWeight: 600 }}
            />
          </div>
        </div>
      </Dialog>

      <PlotSummaryDialog
        plot={summaryPlot}
        indicators={indicators}
        onClose={() => setSummaryPlot(null)}
      />

      <AnalysePlotsDialog
        isOpen={isAnalyseOpen}
        plots={selectedPlots}
        onClose={() => setIsAnalyseOpen(false)}
        onFinished={() => void load()}
      />
    </div>
  );
}
