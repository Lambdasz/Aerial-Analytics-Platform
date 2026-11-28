import { Button, Callout, Dialog, HTMLTable } from "@blueprintjs/core";

import { humanizeIndicator, indicatorsByPlugin } from "./plotAnalytics";
import {
  formatArea,
  formatIndicator,
  formatPerimeter,
  type PlotIndicator,
  type PlotRecord,
} from "./plotCommands";

/**
 * Structured summary of one plot (FR-9.7): name, area, perimeter, creation
 * date, and its latest indicators grouped per plugin. A plot without results
 * shows a message instead of an empty table (FR-9.7.2).
 */
export function PlotSummaryDialog({
  plot,
  indicators,
  onClose,
}: {
  plot: PlotRecord | null;
  indicators: readonly PlotIndicator[];
  onClose: () => void;
}) {
  const groups = plot === null ? [] : indicatorsByPlugin(indicators, plot.id);

  return (
    <Dialog
      isOpen={plot !== null}
      onClose={onClose}
      title={`Plot summary — ${plot?.name ?? ""}`}
      style={{ width: 480, borderRadius: 16, background: "#ffffff" }}
    >
      <div
        className="bp6-dialog-body"
        style={{ maxHeight: 460, overflowY: "auto", padding: "16px 20px" }}
      >
        {plot && (
          <>
            <HTMLTable compact style={{ width: "100%", marginBottom: 14, fontSize: 12 }}>
              <tbody>
                <tr>
                  <td style={{ color: "#64748b" }}>Name</td>
                  <td style={{ textAlign: "right", fontWeight: 600 }}>{plot.name}</td>
                </tr>
                <tr>
                  <td style={{ color: "#64748b" }}>Area</td>
                  <td style={{ textAlign: "right", fontWeight: 600 }}>
                    {formatArea(plot.areaM2, plot.areaHa)}
                  </td>
                </tr>
                <tr>
                  <td style={{ color: "#64748b" }}>Perimeter</td>
                  <td style={{ textAlign: "right", fontWeight: 600 }}>
                    {formatPerimeter(plot.perimeterM)}
                  </td>
                </tr>
                <tr>
                  <td style={{ color: "#64748b" }}>Created</td>
                  <td style={{ textAlign: "right" }}>
                    {new Date(plot.createdAt).toLocaleString()}
                  </td>
                </tr>
              </tbody>
            </HTMLTable>

            {groups.length === 0 ? (
              <Callout style={{ borderRadius: 10, fontSize: 12 }}>
                No analysis results yet. Select this plot and run Analyse.
              </Callout>
            ) : (
              groups.map((group) => (
                <div key={group.pluginId} style={{ marginBottom: 14 }}>
                  <div style={{ fontSize: 11, fontWeight: 600, color: "#64748b", marginBottom: 4 }}>
                    {group.pluginId}
                  </div>
                  <HTMLTable compact striped style={{ width: "100%", fontSize: 12 }}>
                    <tbody>
                      {group.rows.map((row) => (
                        <tr key={`${group.pluginId}-${row.indicator}`}>
                          <td style={{ textTransform: "capitalize" }}>
                            {humanizeIndicator(row.indicator)}
                          </td>
                          <td style={{ textAlign: "right", fontVariantNumeric: "tabular-nums" }}>
                            {formatIndicator(row)}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </HTMLTable>
                </div>
              ))
            )}
          </>
        )}
      </div>
      <div className="bp6-dialog-footer">
        <div className="bp6-dialog-footer-actions">
          <Button
            minimal
            text="Close"
            onClick={onClose}
            style={{ borderRadius: 10, background: "#f1f5f9", color: "#475569", fontWeight: 600 }}
          />
        </div>
      </div>
    </Dialog>
  );
}
