import { useEffect, useState } from "react";
import { Button, Checkbox, Dialog, Icon, NonIdealState, Spinner } from "@blueprintjs/core";

import { getSavedAois } from "../module-03/services/api";
import type { AoiFeature } from "../module-03/types/aoi";
import { AnalysePlotsDialog } from "./AnalysePlotsDialog";
import {
  createPlot,
  currentProjectId,
  deletePlot,
  formatArea,
  formatIndicator,
  formatPerimeter,
  listPlotIndicators,
  listPlots,
  publishPlotsLayer,
  type PlotIndicator,
  type PlotRecord,
} from "./plotCommands";

function message(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  if (error && typeof error === "object" && "message" in error) {
    return String(error.message);
  }
  return "Unexpected error.";
}

/**
 * Plots tab for the Map Explorer sidebar (Module 9 inside Module 3).
 *
 * Same workflow as the Module 09 page but compact and next to the map: turn
 * saved AOIs into plots, run a plugin per plot, compare indicator values, and
 * publish the boundaries as a map layer.
 */
export function PlotAnalyticsPanel() {
  const [plots, setPlots] = useState<PlotRecord[]>([]);
  const [indicators, setIndicators] = useState<PlotIndicator[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    if (!notice) return;
    const timer = setTimeout(() => {
      setNotice(null);
    }, 4000);
    return () => clearTimeout(timer);
  }, [notice]);

  const [isAoiOpen, setIsAoiOpen] = useState(false);
  const [savedAois, setSavedAois] = useState<AoiFeature[]>([]);
  const [checkedAois, setCheckedAois] = useState<Set<string>>(new Set());
  const [isAnalyseOpen, setIsAnalyseOpen] = useState(false);
  const [isImportingAois, setIsImportingAois] = useState(false);

  const [comparedIndicator, setComparedIndicator] = useState("");

  const load = async () => {
    try {
      const projectId = currentProjectId();
      const [rows, indicatorRows, aois] = await Promise.all([
        listPlots(projectId),
        listPlotIndicators(null, projectId),
        getSavedAois().catch(() => [] as AoiFeature[]),
      ]);
      setPlots(rows);
      setIndicators(indicatorRows);
      setSavedAois(aois);
      setError(null);
    } catch (err) {
      setError(message(err));
    } finally {
      setIsLoading(false);
    }
  };

  // Initial load from the app database; state updates happen in callbacks.
  useEffect(() => {
    let cancelled = false;
    const projectId = currentProjectId();
    Promise.all([
      listPlots(projectId),
      listPlotIndicators(null, projectId),
      getSavedAois().catch(() => [] as AoiFeature[]),
    ])
      .then(([rows, indicatorRows, aois]) => {
        if (cancelled) return;
        setPlots(rows);
        setIndicators(indicatorRows);
        setSavedAois(aois);
        setIsLoading(false);
      })
      .catch((err) => {
        if (cancelled) return;
        setError(message(err));
        setIsLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const openAoiPicker = async () => {
    setIsAoiOpen(true);
    setCheckedAois(new Set());
    try {
      const aois = await getSavedAois();
      setSavedAois(aois);
      setCheckedAois(new Set(aois.map((aoi) => aoi.properties.aoi_id)));
    } catch (err) {
      setError(message(err));
    }
  };

  const createFromAois = async (
    aois: AoiFeature[] = savedAois.filter((aoi) => checkedAois.has(aoi.properties.aoi_id)),
  ) => {
    setIsImportingAois(true);
    const chosen = aois;
    const failures: string[] = [];
    for (const aoi of chosen) {
      try {
        await createPlot(
          aoi.properties.name,
          aoi.geometry,
          currentProjectId(),
          null,
          aoi.properties.aoi_id,
        );
      } catch (err) {
        failures.push(`${aoi.properties.name}: ${message(err)}`);
      }
    }
    setIsImportingAois(false);
    setIsAoiOpen(false);
    if (failures.length > 0) setError(failures.join("; "));
    await load();
  };

  const removePlot = async (plot: PlotRecord) => {
    try {
      await deletePlot(plot.id);
      await load();
    } catch (err) {
      setError(message(err));
    }
  };

  const showOnMap = async () => {
    try {
      const layer = await publishPlotsLayer(currentProjectId(), "Plots");
      setNotice(`Layer "${layer.layer_name}" published`);
    } catch (err) {
      setError(message(err));
    }
  };

  const selectedPlots = plots.filter((plot) => selected.has(plot.id));
  const linkedAoiIds = new Set(plots.map((plot) => plot.sourceAoiId).filter(Boolean));
  const pendingAois = savedAois.filter((aoi) => !linkedAoiIds.has(aoi.properties.aoi_id));

  const indicatorKeys = Array.from(
    new Set(indicators.filter((row) => row.value !== null).map((row) => row.indicator)),
  ).sort();
  const activeKey = comparedIndicator || indicatorKeys[0] || "";
  const ranking = plots
    .map((plot) => {
      const row = indicators.find(
        (candidate) => candidate.plotId === plot.id && candidate.indicator === activeKey,
      );
      return { plot, value: row?.value ?? null, unit: row?.unit ?? null };
    })
    .filter(
      (entry): entry is { plot: PlotRecord; value: number; unit: string | null } =>
        entry.value !== null,
    )
    .sort((a, b) => b.value - a.value);

  const latestFor = (plotId: string): string =>
    indicators
      .filter((row) => row.plotId === plotId)
      .slice(0, 2)
      .map((row) => `${row.indicator.replace(/_/g, " ")}: ${formatIndicator(row)}`)
      .join(" · ");

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
        <span style={{ fontSize: "11px", color: "#64748b", fontWeight: 500 }}>
          {plots.length} Plot
        </span>
      </div>

      <div style={{ display: "flex", gap: 6 }}>
        <Button
          minimal
          small
          icon="selection"
          text="From AOI"
          onClick={() => void openAoiPicker()}
          style={{
            flex: 1,
            borderRadius: 10,
            background: "#0f172a",
            color: "#ffffff",
            fontWeight: 600,
            fontSize: 12,
            padding: "6px 12px",
            boxShadow: "none",
            outline: "none",
          }}
        />
        <Button
          minimal
          small
          icon="chart"
          text={`Analyse (${selectedPlots.length})`}
          disabled={selectedPlots.length === 0}
          onClick={() => setIsAnalyseOpen(true)}
          style={{
            borderRadius: 10,
            background: selectedPlots.length > 0 ? "#2563eb" : "#f1f5f9",
            color: selectedPlots.length > 0 ? "#ffffff" : "#94a3b8",
            fontWeight: 600,
            fontSize: 12,
            padding: "6px 12px",
            boxShadow: "none",
            outline: "none",
          }}
        />
        <Button
          minimal
          small
          icon="map"
          text="On map"
          disabled={plots.length === 0}
          onClick={() => void showOnMap()}
          style={{
            borderRadius: 10,
            background: "#ffffff",
            color: plots.length === 0 ? "#94a3b8" : "#0f172a",
            fontWeight: 600,
            fontSize: 12,
            padding: "6px 12px",
            boxShadow: "0 1px 3px rgba(0,0,0,0.05)",
            outline: "none",
          }}
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

      {isLoading ? (
        <div style={{ padding: 24, textAlign: "center" }}>
          <Spinner size={24} />
        </div>
      ) : plots.length === 0 ? (
        <NonIdealState
          icon="polygon-filter"
          title="No plots"
          description="Draw and save an AOI on the map, then add it here with From AOI."
          action={
            <Button small icon="selection" text="From AOI" onClick={() => void openAoiPicker()} />
          }
        />
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
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
                    setSelected((previous) => {
                      const next = new Set(previous);
                      if (checked) next.add(plot.id);
                      else next.delete(plot.id);
                      return next;
                    });
                  }}
                  style={{ marginBottom: 0 }}
                />
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div
                    style={{
                      fontSize: 13,
                      fontWeight: 600,
                      color: "var(--color-ink, #182026)",
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                      whiteSpace: "nowrap",
                    }}
                  >
                    {plot.name}
                  </div>
                  <div style={{ fontSize: 11, color: "var(--color-slate, #64748b)", marginTop: 2 }}>
                    {formatArea(plot.areaM2, plot.areaHa)} · {formatPerimeter(plot.perimeterM)}
                  </div>
                </div>
                <Button
                  minimal
                  small
                  icon="trash"
                  style={{
                    color: "var(--color-slate, #64748b)",
                    borderRadius: 6,
                  }}
                  title="Delete plot"
                  onClick={() => void removePlot(plot)}
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
                {latestFor(plot.id) || "no indicators yet"}
              </div>
            </div>
          ))}
        </div>
      )}

      {indicatorKeys.length > 0 && (
        <div
          style={{
            background: "#ffffff",
            border: "none",
            borderRadius: 16,
            padding: "14px 16px",
            boxShadow: "0 2px 12px -2px rgba(0, 0, 0, 0.05), 0 1px 3px rgba(0, 0, 0, 0.02)",
          }}
        >
          <div style={{ fontSize: 11, fontWeight: 600, color: "#64748b", marginBottom: 6 }}>
            RANKING
          </div>
          <select
            value={activeKey}
            onChange={(event) => setComparedIndicator(event.currentTarget.value)}
            style={{
              width: "100%",
              padding: "8px 12px",
              fontSize: 12,
              borderRadius: 10,
              border: "none",
              background: "#f8fafc",
              color: "#0f172a",
              outline: "none",
              cursor: "pointer",
            }}
          >
            {indicatorKeys.map((key) => (
              <option key={key} value={key}>
                {key.replace(/_/g, " ")}
              </option>
            ))}
          </select>
          <div style={{ display: "flex", flexDirection: "column", gap: 4, marginTop: 8 }}>
            {ranking.map((entry, index) => (
              <div
                key={entry.plot.id}
                style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 12 }}
              >
                <span
                  style={{
                    minWidth: 22,
                    height: 20,
                    display: "inline-flex",
                    alignItems: "center",
                    justifyContent: "center",
                    borderRadius: 6,
                    fontSize: 11,
                    fontWeight: 600,
                    background: index === 0 ? "#0f172a" : "#f1f5f9",
                    color: index === 0 ? "#ffffff" : "#64748b",
                  }}
                >
                  {index + 1}
                </span>
                <span
                  style={{
                    flex: 1,
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                    whiteSpace: "nowrap",
                  }}
                >
                  {entry.plot.name}
                </span>
                <span style={{ fontVariantNumeric: "tabular-nums" }}>
                  {entry.value.toFixed(2)}
                  {entry.unit ? ` ${entry.unit}` : ""}
                </span>
              </div>
            ))}
          </div>
        </div>
      )}

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
          {savedAois.length === 0 ? (
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
          ) : (
            savedAois.map((aoi) => {
              const isChecked = checkedAois.has(aoi.properties.aoi_id);
              return (
                <div
                  key={aoi.properties.aoi_id}
                  role="button"
                  tabIndex={0}
                  onClick={() => {
                    setCheckedAois((previous) => {
                      const next = new Set(previous);
                      if (isChecked) next.delete(aoi.properties.aoi_id);
                      else next.add(aoi.properties.aoi_id);
                      return next;
                    });
                  }}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                      e.preventDefault();
                      setCheckedAois((previous) => {
                        const next = new Set(previous);
                        if (isChecked) next.delete(aoi.properties.aoi_id);
                        else next.add(aoi.properties.aoi_id);
                        return next;
                      });
                    }
                  }}
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
                        setCheckedAois((previous) => {
                          const next = new Set(previous);
                          if (checked) next.add(aoi.properties.aoi_id);
                          else next.delete(aoi.properties.aoi_id);
                          return next;
                        });
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
            })
          )}
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

      <AnalysePlotsDialog
        isOpen={isAnalyseOpen}
        plots={selectedPlots}
        onClose={() => setIsAnalyseOpen(false)}
        onFinished={() => void load()}
      />
    </div>
  );
}
