import { useEffect, useState } from "react";
import {
  Button,
  Callout,
  Checkbox,
  Dialog,
  HTMLSelect,
  Intent,
  NonIdealState,
  Spinner,
  Tag,
} from "@blueprintjs/core";

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
      <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
        <Button small icon="selection" text="From AOI" onClick={() => void openAoiPicker()} fill />
        <Button
          small
          icon="chart"
          intent={selectedPlots.length > 0 ? Intent.SUCCESS : Intent.NONE}
          text={`Analyse (${selectedPlots.length})`}
          disabled={selectedPlots.length === 0}
          onClick={() => setIsAnalyseOpen(true)}
        />
        <Button
          small
          icon="map"
          text="On map"
          disabled={plots.length === 0}
          onClick={() => void showOnMap()}
        />
      </div>

      {pendingAois.length > 0 && (
        <Callout
          intent={Intent.PRIMARY}
          icon="info-sign"
          title={`${pendingAois.length} saved AOI${pendingAois.length === 1 ? "" : "s"} not added as plot${
            pendingAois.length === 1 ? "" : "s"
          }`}
        >
          <div style={{ display: "flex", alignItems: "center", gap: 8, marginTop: 4 }}>
            <span style={{ flex: 1, fontSize: 12 }}>
              AOIs come from the AOI tab; add them here to analyse and rank them.
            </span>
            <Button
              small
              intent={Intent.PRIMARY}
              text="Add all"
              loading={isImportingAois}
              onClick={() => void createFromAois(pendingAois)}
            />
          </div>
        </Callout>
      )}

      {error && <Callout intent={Intent.DANGER}>{error}</Callout>}
      {notice && <Callout intent={Intent.SUCCESS}>{notice}</Callout>}

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
                background: "var(--color-gallery-white)",
                border: "1px solid var(--color-hairline-silver)",
                borderRadius: 12,
                padding: "8px 10px",
              }}
            >
              <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
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
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                      whiteSpace: "nowrap",
                    }}
                  >
                    {plot.name}
                  </div>
                  <div style={{ fontSize: 11, color: "var(--color-slate)" }}>
                    {formatArea(plot.areaM2, plot.areaHa)} · {formatPerimeter(plot.perimeterM)}
                  </div>
                </div>
                <Button
                  minimal
                  small
                  icon="trash"
                  intent={Intent.DANGER}
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
            background: "var(--color-gallery-white)",
            border: "1px solid var(--color-hairline-silver)",
            borderRadius: 12,
            padding: 10,
          }}
        >
          <div style={{ fontSize: 11, color: "var(--color-slate)", marginBottom: 6 }}>RANKING</div>
          <HTMLSelect
            fill
            value={activeKey}
            onChange={(event) => setComparedIndicator(event.currentTarget.value)}
            options={indicatorKeys.map((key) => ({
              label: key.replace(/_/g, " "),
              value: key,
            }))}
          />
          <div style={{ display: "flex", flexDirection: "column", gap: 4, marginTop: 8 }}>
            {ranking.map((entry, index) => (
              <div
                key={entry.plot.id}
                style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 12 }}
              >
                <Tag
                  minimal
                  round
                  intent={
                    index === 0
                      ? Intent.SUCCESS
                      : index === ranking.length - 1 && ranking.length > 1
                        ? Intent.DANGER
                        : Intent.NONE
                  }
                  style={{ minWidth: 22, textAlign: "center" }}
                >
                  {index + 1}
                </Tag>
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
        style={{ width: 480 }}
      >
        <div className="bp6-dialog-body" style={{ maxHeight: 380, overflowY: "auto" }}>
          {savedAois.length === 0 ? (
            <Callout intent={Intent.NONE}>
              No AOIs saved yet. Draw an area in the AOI tab, save it, then return here.
            </Callout>
          ) : (
            savedAois.map((aoi) => (
              <Checkbox
                key={aoi.properties.aoi_id}
                checked={checkedAois.has(aoi.properties.aoi_id)}
                onChange={(event) => {
                  const checked = event.currentTarget.checked;
                  setCheckedAois((previous) => {
                    const next = new Set(previous);
                    if (checked) next.add(aoi.properties.aoi_id);
                    else next.delete(aoi.properties.aoi_id);
                    return next;
                  });
                }}
                labelElement={
                  <span>
                    <strong>{aoi.properties.name}</strong>
                    <span style={{ marginLeft: 6, fontSize: 11, color: "var(--color-slate)" }}>
                      {aoi.properties.calculated_area_sq_m.toFixed(0)} m²
                    </span>
                  </span>
                }
              />
            ))
          )}
        </div>
        <div className="bp6-dialog-footer">
          <div className="bp6-dialog-footer-actions">
            <Button text="Cancel" onClick={() => setIsAoiOpen(false)} />
            <Button
              intent={Intent.PRIMARY}
              text={`Add ${checkedAois.size}`}
              loading={isImportingAois}
              disabled={checkedAois.size === 0}
              onClick={() => void createFromAois()}
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
