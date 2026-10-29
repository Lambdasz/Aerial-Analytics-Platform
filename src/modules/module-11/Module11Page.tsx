import { useEffect, useMemo, useState } from "react";
import {
  Button,
  Callout,
  Card,
  Dialog,
  HTMLSelect,
  HTMLTable,
  Intent,
  NonIdealState,
  Spinner,
  Tag,
} from "@blueprintjs/core";

import { Bar } from "../../components/charts/bar";
import { listProjects, type Project } from "../module-01/projectCommands";
import { commandErrorMessage } from "../module-01/sessionCommands";
import { loadImageThumbnail } from "../../lib/imagePreview";
import {
  type PluginResultRow,
  type ProjectDashboard,
  currentProjectId,
  exportDashboardCsv,
  exportDashboardGeojson,
  exportDashboardJson,
  formatMetric,
  getProjectDashboard,
  listDashboardPluginResults,
} from "./module11Commands";

function fileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

function StatCard({
  label,
  value,
  hint,
}: {
  label: string;
  value: string | number;
  hint?: string;
}) {
  return (
    <Card
      style={{
        flex: "1 1 160px",
        padding: 16,
        borderRadius: 14,
        background: "var(--color-gallery-white, #fff)",
        border: "1px solid var(--color-hairline-silver, #d6d6d6)",
        boxShadow: "none",
      }}
    >
      <div style={{ fontSize: 11, letterSpacing: "0.06em", color: "var(--color-slate)" }}>
        {label.toUpperCase()}
      </div>
      <div style={{ fontSize: 26, fontWeight: 600, marginTop: 6 }}>{value}</div>
      {hint && <div style={{ fontSize: 11, color: "var(--color-slate)" }}>{hint}</div>}
    </Card>
  );
}

/** Image preview + details for one explorer row. */
function ResultRowDialog({ row, onClose }: { row: PluginResultRow | null; onClose: () => void }) {
  const [preview, setPreview] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    if (!row) return;
    let cancelled = false;
    loadImageThumbnail(row.filePath)
      .then((url) => {
        if (!cancelled) setPreview(url);
      })
      .catch(() => {
        if (!cancelled) setFailed(true);
      });
    return () => {
      cancelled = true;
    };
  }, [row]);

  const openInGallery = () => {
    window.dispatchEvent(new CustomEvent("open-module", { detail: { moduleId: "module-03" } }));
    window.dispatchEvent(new CustomEvent("open-gallery-tab"));
    onClose();
  };

  return (
    <Dialog isOpen={row !== null} onClose={onClose} title="Result detail" style={{ width: 520 }}>
      <div className="bp6-dialog-body">
        {row && (
          <>
            <div
              style={{
                height: 200,
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                background: "var(--color-studio-mist, #f5f5f7)",
                borderRadius: 12,
                overflow: "hidden",
                marginBottom: 12,
              }}
            >
              {preview ? (
                <img
                  src={preview}
                  alt={fileName(row.filePath)}
                  style={{ maxWidth: "100%", maxHeight: "100%", objectFit: "contain" }}
                />
              ) : failed ? (
                <span style={{ fontSize: 11, color: "var(--color-slate)" }}>
                  preview unavailable
                </span>
              ) : (
                <Spinner size={22} />
              )}
            </div>
            <HTMLTable compact striped style={{ width: "100%" }}>
              <tbody>
                <tr>
                  <td>Image</td>
                  <td style={{ textAlign: "right" }}>{fileName(row.filePath)}</td>
                </tr>
                <tr>
                  <td>Plugin</td>
                  <td style={{ textAlign: "right" }}>{row.pluginId}</td>
                </tr>
                <tr>
                  <td>Metric</td>
                  <td style={{ textAlign: "right" }}>
                    {row.metric.replace(/_/g, " ")} = {formatMetric(row)}
                  </td>
                </tr>
                <tr>
                  <td>Recorded</td>
                  <td style={{ textAlign: "right" }}>{new Date(row.createdAt).toLocaleString()}</td>
                </tr>
              </tbody>
            </HTMLTable>
          </>
        )}
      </div>
      <div className="bp6-dialog-footer">
        <div className="bp6-dialog-footer-actions">
          <Button text="Close" onClick={onClose} />
          <Button
            intent={Intent.PRIMARY}
            icon="map"
            text="Open in map gallery"
            onClick={openInGallery}
          />
        </div>
      </div>
    </Dialog>
  );
}

/**
 * Module 11 — analytics dashboard and reporting.
 *
 * Read-only summary of stored platform results: project totals, indicator
 * aggregates, plugin usage, the latest temporal comparison, an interactive
 * result explorer, and CSV/GeoJSON/JSON exports.
 */
export function Module11Page() {
  const [projects, setProjects] = useState<Project[]>([]);
  const [projectId, setProjectId] = useState("");
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [dashboard, setDashboard] = useState<ProjectDashboard | null>(null);
  const [results, setResults] = useState<PluginResultRow[]>([]);
  const [selectedResult, setSelectedResult] = useState<PluginResultRow | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [isExporting, setIsExporting] = useState<string | null>(null);
  const [reloadToken, setReloadToken] = useState(0);

  // Projects: use the Module 1 selection when available.
  useEffect(() => {
    let cancelled = false;
    listProjects()
      .then((list) => {
        if (cancelled) return;
        setProjects(list);
        const saved = currentProjectId();
        const initial = list.find((project) => project.id === saved) ?? list[0];
        setProjectId(initial?.id ?? "");
      })
      .catch((err) => {
        if (!cancelled) {
          setError(commandErrorMessage(err));
          setIsLoading(false);
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // Dashboard + explorer rows for the selected project.
  useEffect(() => {
    if (!projectId) return;
    let cancelled = false;
    Promise.all([getProjectDashboard(projectId), listDashboardPluginResults(projectId, 200)])
      .then(([summary, rows]) => {
        if (cancelled) return;
        setDashboard(summary);
        setResults(rows);
        setIsLoading(false);
        setError(null);
      })
      .catch((err) => {
        if (!cancelled) {
          setError(commandErrorMessage(err));
          setIsLoading(false);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [projectId, reloadToken]);

  const load = () => {
    setNotice(null);
    setError(null);
    setIsLoading(true);
    setReloadToken((token) => token + 1);
  };

  const indicatorChart = useMemo(
    () =>
      (dashboard?.indicatorAggregates ?? []).map((aggregate) => ({
        label: aggregate.indicator.replace(/_/g, " "),
        value: Number(aggregate.mean.toFixed(2)),
      })),
    [dashboard],
  );

  const pluginChart = useMemo(
    () =>
      (dashboard?.plugins ?? []).map((plugin) => ({
        label: plugin.pluginId,
        value: plugin.images,
      })),
    [dashboard],
  );

  const runExport = async (key: string, action: (projectId: string) => Promise<string>) => {
    if (!projectId) return;
    setIsExporting(key);
    setNotice(null);
    try {
      const path = await action(projectId);
      setNotice(`Saved to ${path}`);
    } catch (err) {
      setError(commandErrorMessage(err));
    } finally {
      setIsExporting(null);
    }
  };

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        gap: 16,
        height: "100%",
        overflowY: "auto",
        padding: 4,
      }}
    >
      <header style={{ display: "flex", justifyContent: "space-between", gap: 16 }}>
        <div>
          <div style={{ fontSize: 12, color: "var(--color-slate)", letterSpacing: "0.08em" }}>
            MODULE 11 · DASHBOARD &amp; REPORTING
          </div>
          <h1 style={{ margin: "4px 0 2px", fontSize: 26, color: "var(--color-ink)" }}>
            Analytics overview
          </h1>
          <div style={{ fontSize: 12, color: "var(--color-slate)" }}>
            Read-only summary of stored sessions, plots, plugin results and comparisons.
          </div>
        </div>
        <div style={{ display: "flex", gap: 8, alignItems: "flex-start", flexWrap: "wrap" }}>
          <HTMLSelect
            value={projectId}
            onChange={(event) => {
              setProjectId(event.currentTarget.value);
              setIsLoading(true);
            }}
            options={projects.map((project) => ({
              label: project.name,
              value: project.id,
            }))}
          />
          <Button icon="refresh" text="Refresh" onClick={load} loading={isLoading} />
        </div>
      </header>

      {error && (
        <Callout intent={Intent.DANGER} title="Dashboard error">
          {error}
        </Callout>
      )}
      {notice && (
        <Callout intent={Intent.SUCCESS}>
          <div style={{ display: "flex", justifyContent: "space-between", gap: 12 }}>
            <span>{notice}</span>
            <Button minimal small icon="cross" onClick={() => setNotice(null)} />
          </div>
        </Callout>
      )}

      {isLoading ? (
        <div style={{ padding: 48, textAlign: "center" }}>
          <Spinner size={32} />
        </div>
      ) : !dashboard ? (
        <NonIdealState
          icon="dashboard"
          title="No dashboard data"
          description="Select a project with imported imagery to see its analytics."
        />
      ) : (
        <>
          <div style={{ display: "flex", gap: 10, flexWrap: "wrap" }}>
            <StatCard label="Sessions" value={dashboard.sessions} />
            <StatCard
              label="Images"
              value={dashboard.images}
              hint={`${dashboard.geotaggedImages} geotagged`}
            />
            <StatCard
              label="Plots"
              value={dashboard.plots}
              hint={`${dashboard.plotAreaHa.toFixed(2)} ha total`}
            />
            <StatCard label="Plugin results" value={dashboard.pluginResults} />
            <StatCard
              label="Latest comparison"
              value={
                dashboard.latestComparison
                  ? `${dashboard.latestComparison.delta > 0 ? "+" : ""}${dashboard.latestComparison.delta.toFixed(2)} pp`
                  : "—"
              }
              hint={
                dashboard.latestComparison
                  ? `${dashboard.latestComparison.beforeDate} → ${dashboard.latestComparison.afterDate}`
                  : "needs two dates"
              }
            />
          </div>

          <div
            style={{
              display: "grid",
              gridTemplateColumns: "minmax(0, 1fr) minmax(0, 1fr)",
              gap: 12,
            }}
          >
            <Card style={{ padding: 16, borderRadius: 14 }}>
              <div style={{ fontWeight: 600, marginBottom: 8 }}>Indicator means</div>
              <div style={{ height: 220 }}>
                {indicatorChart.length > 0 ? (
                  <Bar data={indicatorChart} />
                ) : (
                  <NonIdealState
                    icon="chart"
                    title="No indicators yet"
                    description="Run an analysis from the Plots tab."
                  />
                )}
              </div>
            </Card>
            <Card style={{ padding: 16, borderRadius: 14 }}>
              <div style={{ fontWeight: 600, marginBottom: 8 }}>Images per plugin</div>
              <div style={{ height: 220 }}>
                {pluginChart.length > 0 ? (
                  <Bar data={pluginChart} color="#0066cc" />
                ) : (
                  <NonIdealState
                    icon="box"
                    title="No plugin results"
                    description="Run a plugin from Module 02."
                  />
                )}
              </div>
            </Card>
          </div>

          <Card style={{ padding: 16, borderRadius: 14 }}>
            <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 8 }}>
              <div style={{ fontWeight: 600 }}>Plugin result explorer</div>
              <Tag minimal round>
                {results.length} rows
              </Tag>
              <div style={{ flex: 1 }} />
            </div>
            {results.length === 0 ? (
              <NonIdealState
                icon="search"
                title="Nothing stored yet"
                description="Publish a plugin run and its metrics appear here."
              />
            ) : (
              <div style={{ maxHeight: 320, overflowY: "auto" }}>
                <HTMLTable striped interactive compact style={{ width: "100%" }}>
                  <thead>
                    <tr>
                      <th>Image</th>
                      <th>Plugin</th>
                      <th>Metric</th>
                      <th>Value</th>
                      <th>Recorded</th>
                    </tr>
                  </thead>
                  <tbody>
                    {results.slice(0, 100).map((row, index) => (
                      <tr
                        key={`${row.imageId}-${row.pluginId}-${row.metric}-${index}`}
                        onClick={() => setSelectedResult(row)}
                      >
                        <td>{fileName(row.filePath)}</td>
                        <td style={{ fontSize: 12 }}>{row.pluginId}</td>
                        <td>{row.metric.replace(/_/g, " ")}</td>
                        <td>{formatMetric(row)}</td>
                        <td style={{ fontSize: 11, color: "var(--color-slate)" }}>
                          {new Date(row.createdAt).toLocaleString()}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </HTMLTable>
              </div>
            )}
          </Card>

          <Card style={{ padding: 16, borderRadius: 14 }}>
            <div style={{ fontWeight: 600, marginBottom: 8 }}>Exports</div>
            <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
              <Button
                icon="export"
                text="Plot indicators (CSV)"
                loading={isExporting === "csv"}
                onClick={() => void runExport("csv", exportDashboardCsv)}
              />
              <Button
                icon="map"
                text="Plots (GeoJSON)"
                loading={isExporting === "geojson"}
                onClick={() => void runExport("geojson", exportDashboardGeojson)}
              />
              <Button
                icon="code"
                text="Dashboard snapshot (JSON)"
                loading={isExporting === "json"}
                onClick={() => void runExport("json", exportDashboardJson)}
              />
            </div>
            <div style={{ fontSize: 11, color: "var(--color-slate)", marginTop: 8 }}>
              Files are written to <code>map_storage/exports/</code>; the path is shown above.
            </div>
          </Card>
        </>
      )}

      <ResultRowDialog
        key={selectedResult ? `${selectedResult.imageId}-${selectedResult.metric}` : "empty"}
        row={selectedResult}
        onClose={() => setSelectedResult(null)}
      />
    </div>
  );
}
