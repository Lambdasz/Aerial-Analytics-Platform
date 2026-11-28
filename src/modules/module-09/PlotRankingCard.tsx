import { useState } from "react";
import { Button, ButtonGroup } from "@blueprintjs/core";

import {
  comparisonEntries,
  defaultDirection,
  indicatorOptions,
  indicatorStats,
  rankEntries,
  type SortDirection,
} from "./plotAnalytics";
import type { PlotIndicator, PlotRecord } from "./plotCommands";

const DIRECTION_LABELS: Record<SortDirection, string> = {
  desc: "Highest first",
  asc: "Lowest first",
};

const formatNumber = (value: number, unit: string | null): string =>
  `${value.toFixed(2)}${unit ? ` ${unit}` : ""}`;

/**
 * Comparison, statistics and ranking of one indicator across plots
 * (FR-9.4.2, FR-9.5, FR-9.6). Indicators are chosen per plugin so equal names
 * from different plugins never mix; the sort direction defaults per indicator
 * and can be flipped.
 */
export function PlotRankingCard({
  plots,
  indicators,
}: {
  plots: readonly PlotRecord[];
  indicators: readonly PlotIndicator[];
}) {
  const options = indicatorOptions(indicators);
  const [chosenKey, setChosenKey] = useState("");
  const [directionOverride, setDirectionOverride] = useState<SortDirection | null>(null);

  const active = options.find((option) => option.key === chosenKey) ?? options[0] ?? null;
  const direction = directionOverride ?? defaultDirection(active?.indicator ?? "");
  const entries = comparisonEntries(plots, indicators, active);
  const ranking = rankEntries(direction)(entries);
  const stats = indicatorStats(entries.map((entry) => entry.value));
  const unit = entries[0]?.unit ?? null;

  const statCells: ReadonlyArray<[string, string]> = stats
    ? [
        ["Plots", String(stats.count)],
        ["Mean", formatNumber(stats.mean, unit)],
        ["Min", formatNumber(stats.min, unit)],
        ["Max", formatNumber(stats.max, unit)],
        ["Std dev", formatNumber(stats.stdDev, unit)],
      ]
    : [];

  return plots.length === 0 ? null : (
    <div
      style={{
        background: "#ffffff",
        borderRadius: 16,
        padding: "14px 16px",
        boxShadow: "0 2px 12px -2px rgba(0, 0, 0, 0.05), 0 1px 3px rgba(0, 0, 0, 0.02)",
      }}
    >
      <div style={{ fontSize: 11, fontWeight: 600, color: "#64748b", marginBottom: 6 }}>
        COMPARISON &amp; RANKING
      </div>
      {options.length === 0 && (
        <div style={{ fontSize: 12, color: "#64748b", lineHeight: 1.5 }}>
          Belum ada indikator untuk dibandingkan. Langkahnya:
          <ol style={{ margin: "6px 0 0", paddingLeft: 18 }}>
            <li>Centang plot yang ingin dibandingkan (minimal 2).</li>
            <li>
              Klik <strong>Analyse</strong>, pilih plugin dan orthomosaic, lalu{" "}
              <strong>Run analysis</strong>.
            </li>
            <li>
              Setelah selesai, perbandingan, statistik (rata-rata, min, maks, simpangan baku) dan
              ranking muncul di sini.
            </li>
          </ol>
        </div>
      )}
      {options.length > 0 && (
        <>
          <select
            value={active?.key ?? ""}
            onChange={(event) => {
              setChosenKey(event.currentTarget.value);
              setDirectionOverride(null);
            }}
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
            {options.map((option) => (
              <option key={option.key} value={option.key}>
                {option.label}
              </option>
            ))}
          </select>

          <ButtonGroup fill style={{ marginTop: 8 }}>
            {(Object.keys(DIRECTION_LABELS) as SortDirection[]).map((value) => (
              <Button
                key={value}
                small
                text={DIRECTION_LABELS[value]}
                active={direction === value}
                onClick={() => setDirectionOverride(value)}
                style={{ fontSize: 11 }}
              />
            ))}
          </ButtonGroup>

          {stats && (
            <div
              style={{
                display: "grid",
                gridTemplateColumns: "repeat(auto-fit, minmax(70px, 1fr))",
                gap: 6,
                marginTop: 10,
              }}
            >
              {statCells.map(([label, value]) => (
                <div
                  key={label}
                  style={{
                    background: "#f8fafc",
                    borderRadius: 8,
                    padding: "6px 8px",
                    minWidth: 0,
                  }}
                >
                  <div style={{ fontSize: 10, color: "#64748b" }}>{label}</div>
                  <div
                    style={{
                      fontSize: 12,
                      fontWeight: 600,
                      color: "#0f172a",
                      fontVariantNumeric: "tabular-nums",
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                      whiteSpace: "nowrap",
                    }}
                  >
                    {value}
                  </div>
                </div>
              ))}
            </div>
          )}

          <div style={{ display: "flex", flexDirection: "column", gap: 4, marginTop: 10 }}>
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
                  {formatNumber(entry.value, entry.unit)}
                </span>
              </div>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
