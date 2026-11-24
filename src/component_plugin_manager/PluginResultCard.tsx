import React, { useState, useMemo } from "react";
import { Button, Callout, Card, Collapse, Elevation, Intent, Tag, Icon } from "@blueprintjs/core";
import type { StandardJobResult, VerifiedArtifact } from "../types/plugin";

export interface PluginResultCardProps {
  result: StandardJobResult;
  onClear?: () => void;
  className?: string;
}

const ACRONYMS: Record<string, string> = {
  vari: "VARI",
  gli: "GLI",
  exg: "ExG",
  ndvi: "NDVI",
  rgb: "RGB",
  std: "Std Dev",
  aoi: "AOI",
  gps: "GPS",
  id: "ID",
  px: "px",
  pct: "%",
  r: "Red",
  g: "Green",
  b: "Blue",
};

/** Converts snake_case / camelCase metric keys into friendly labels. */
function formatMetricLabel(key: string): string {
  // Check exact custom mappings first
  const customMap: Record<string, string> = {
    total_pixels: "Total Pixels",
    vegetation_pixels: "Canopy Pixels",
    vegetation_coverage_pct: "Canopy Coverage",
    yellowing_pct: "Yellowing",
    vigorous_pct: "Vigorous Canopy",
    moderate_stress_pct: "Moderate Stress",
    mean_vari: "Mean VARI",
    mean_gli: "Mean GLI",
    mean_exg: "Mean ExG",
    mean_hue_deg: "Mean Hue",
    mean_saturation: "Mean Saturation",
    mean_r: "Mean Red",
    mean_g: "Mean Green",
    mean_b: "Mean Blue",
    std_r: "Red Std Dev",
    std_g: "Green Std Dev",
    std_b: "Blue Std Dev",
  };

  if (customMap[key]) {
    return customMap[key];
  }

  // Tokenize snake_case
  const parts = key.split("_");
  return parts
    .map((part) => {
      const lower = part.toLowerCase();
      if (ACRONYMS[lower]) {
        return ACRONYMS[lower];
      }
      return part.charAt(0).toUpperCase() + part.slice(1);
    })
    .join(" ");
}

/** Formats metric values into display string and unit. */
function formatMetricValue(key: string, value: unknown): { display: string; unit?: string } {
  if (value === null || value === undefined) {
    return { display: "—" };
  }

  if (typeof value === "boolean") {
    return { display: value ? "Yes" : "No" };
  }

  if (typeof value === "number") {
    const isPct = key.endsWith("_pct") || key.endsWith("_percentage") || key.endsWith("_ratio");
    const isPixel = key.includes("pixel");
    const isDegree = key.endsWith("_deg") || key.endsWith("_degrees");

    if (isPct) {
      if (value === 0) return { display: "0", unit: "%" };
      if (Math.abs(value) < 0.0001 && value !== 0) {
        return { display: value.toExponential(2), unit: "%" };
      }
      if (Math.abs(value) < 0.01) {
        return {
          display: value.toLocaleString(undefined, {
            maximumFractionDigits: 6,
          }),
          unit: "%",
        };
      }
      return {
        display: value.toLocaleString(undefined, {
          minimumFractionDigits: 2,
          maximumFractionDigits: 2,
        }),
        unit: "%",
      };
    }

    if (isPixel) {
      return {
        display: Math.round(value).toLocaleString(),
        unit: "px",
      };
    }

    if (isDegree) {
      return {
        display: value.toFixed(1),
        unit: "°",
      };
    }

    if (Number.isInteger(value)) {
      return { display: value.toLocaleString() };
    }

    // Decimal float
    if (Math.abs(value) < 0.0001 && value !== 0) {
      return { display: value.toExponential(3) };
    }
    return {
      display: value.toLocaleString(undefined, {
        minimumFractionDigits: 2,
        maximumFractionDigits: 4,
      }),
    };
  }

  if (typeof value === "string") {
    return { display: value };
  }

  return { display: JSON.stringify(value) };
}

/** Formats execution time in milliseconds or seconds. */
function formatExecutionTime(ms: number): string {
  if (ms < 1000) {
    return `${ms} ms`;
  }
  return `${(ms / 1000).toFixed(2)} s`;
}

/** Extracts filename from path. */
function getFilename(filePath: string): string {
  return filePath.split(/[/\\]/).pop() || filePath;
}

export const PluginResultCard: React.FC<PluginResultCardProps> = ({
  result,
  onClear,
  className = "",
}) => {
  const [isRawJsonOpen, setIsRawJsonOpen] = useState(false);
  const [copiedJson, setCopiedJson] = useState(false);
  const [copiedArtifact, setCopiedArtifact] = useState<string | null>(null);

  const statusIntent = useMemo(() => {
    switch (result.status) {
      case "failure":
        return Intent.DANGER;
      case "warning":
        return Intent.WARNING;
      case "success":
      default:
        return Intent.SUCCESS;
    }
  }, [result.status]);

  const statusIcon = useMemo(() => {
    switch (result.status) {
      case "failure":
        return "error";
      case "warning":
        return "warning-sign";
      case "success":
      default:
        return "tick-circle";
    }
  }, [result.status]);

  const metricsEntries = useMemo(() => {
    return Object.entries(result.metrics ?? {});
  }, [result.metrics]);

  const artifactsEntries = useMemo(() => {
    return Object.entries(result.artifacts ?? {});
  }, [result.artifacts]);

  const handleCopyJson = async () => {
    try {
      await navigator.clipboard.writeText(JSON.stringify(result, null, 2));
      setCopiedJson(true);
      setTimeout(() => setCopiedJson(false), 2000);
    } catch {
      // ignore
    }
  };

  const handleCopyArtifactPath = async (key: string, path: string) => {
    try {
      await navigator.clipboard.writeText(path);
      setCopiedArtifact(key);
      setTimeout(() => setCopiedArtifact(null), 2000);
    } catch {
      // ignore
    }
  };

  return (
    <Card
      elevation={Elevation.ZERO}
      className={`plugin-result-card ${className}`}
      style={{
        marginBottom: 16,
        padding: "14px 16px",
        borderRadius: 12,
        border: "1px solid var(--color-hairline-silver, #d6d6d6)",
        background: "var(--color-gallery-white, #ffffff)",
      }}
    >
      {/* 1. Header */}
      <div
        className="result-header"
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          marginBottom: 12,
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <Tag
            intent={statusIntent}
            icon={statusIcon}
            round
            minimal
            style={{ fontWeight: 600, fontSize: 11, textTransform: "uppercase" }}
          >
            {result.status}
          </Tag>
          <span
            style={{
              fontFamily: '"DM Mono", monospace',
              fontSize: 11,
              color: "var(--color-slate, #707070)",
            }}
          >
            {formatExecutionTime(result.execution_time_ms)}
          </span>
        </div>

        {onClear && (
          <Button
            icon="cross"
            minimal
            small
            title="Clear Result"
            onClick={onClear}
            style={{ color: "var(--color-slate, #707070)" }}
          />
        )}
      </div>

      {/* 2. Error Callout if any */}
      {result.error && (
        <Callout
          intent={Intent.DANGER}
          icon="error"
          title="Execution Issue"
          style={{ marginBottom: 12, fontSize: 12 }}
        >
          {result.error}
        </Callout>
      )}

      {/* 3. Metrics Section */}
      {metricsEntries.length > 0 && (
        <div style={{ marginBottom: 14 }}>
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: 6,
              marginBottom: 8,
              fontSize: 10,
              fontFamily: '"DM Mono", monospace',
              color: "var(--color-slate, #707070)",
              letterSpacing: "0.08em",
              textTransform: "uppercase",
              fontWeight: 600,
            }}
          >
            <Icon icon="chart" size={12} />
            <span>Metrics Summary</span>
          </div>

          <div
            className="result-metrics-grid"
            style={{
              display: "grid",
              gridTemplateColumns: "repeat(2, minmax(0, 1fr))",
              gap: 8,
            }}
          >
            {metricsEntries.map(([k, v]) => {
              const label = formatMetricLabel(k);
              const { display, unit } = formatMetricValue(k, v);
              return (
                <div
                  key={k}
                  className="result-metric-tile"
                  style={{
                    padding: "8px 10px",
                    borderRadius: 8,
                    background: "var(--color-studio-mist, #f5f5f7)",
                    border: "1px solid var(--color-hairline-silver, #e5e5ea)",
                    display: "flex",
                    flexDirection: "column",
                    justifyContent: "center",
                    minWidth: 0,
                  }}
                  title={`${label}: ${display}${unit ? ` ${unit}` : ""}`}
                >
                  <span
                    style={{
                      fontSize: 10,
                      color: "var(--color-slate, #707070)",
                      fontWeight: 500,
                      marginBottom: 2,
                      whiteSpace: "nowrap",
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                    }}
                  >
                    {label}
                  </span>
                  <div
                    style={{
                      fontFamily: '"DM Mono", monospace',
                      fontSize: 12,
                      fontWeight: 600,
                      color: "var(--color-ink, #1d1d1f)",
                      whiteSpace: "nowrap",
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                      display: "flex",
                      alignItems: "baseline",
                      gap: 3,
                    }}
                  >
                    <span>{display}</span>
                    {unit && (
                      <span
                        style={{
                          fontSize: 10,
                          color: "var(--color-slate, #707070)",
                          fontWeight: 400,
                        }}
                      >
                        {unit}
                      </span>
                    )}
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* 4. Artifacts Section */}
      {artifactsEntries.length > 0 && (
        <div style={{ marginBottom: 12 }}>
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: 6,
              marginBottom: 8,
              fontSize: 10,
              fontFamily: '"DM Mono", monospace',
              color: "var(--color-slate, #707070)",
              letterSpacing: "0.08em",
              textTransform: "uppercase",
              fontWeight: 600,
            }}
          >
            <Icon icon="layers" size={12} />
            <span>Generated Layers & Artifacts</span>
          </div>

          <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            {artifactsEntries.map(([key, artifact]: [string, VerifiedArtifact]) => {
              const filename = getFilename(artifact.file_path);
              const label = formatMetricLabel(key);
              const formatUpper = (
                artifact.format.split("/").pop() || artifact.format
              ).toUpperCase();
              const isCopied = copiedArtifact === key;

              return (
                <div
                  key={key}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "space-between",
                    gap: 8,
                    padding: "6px 10px",
                    borderRadius: 8,
                    background: "var(--color-studio-mist, #f5f5f7)",
                    border: "1px solid var(--color-hairline-silver, #e5e5ea)",
                    fontSize: 11,
                  }}
                >
                  <div
                    style={{
                      display: "flex",
                      alignItems: "center",
                      gap: 6,
                      minWidth: 0,
                    }}
                  >
                    <Icon
                      icon={formatUpper.includes("GEOJSON") ? "polygon-filter" : "media"}
                      size={13}
                      style={{ color: "var(--color-apple-blue, #0066cc)", flexShrink: 0 }}
                    />
                    <div style={{ minWidth: 0 }}>
                      <div
                        style={{
                          fontWeight: 600,
                          color: "var(--color-ink, #1d1d1f)",
                          whiteSpace: "nowrap",
                          overflow: "hidden",
                          textOverflow: "ellipsis",
                        }}
                      >
                        {label}
                      </div>
                      <div
                        style={{
                          fontSize: 10,
                          color: "var(--color-slate, #707070)",
                          fontFamily: '"DM Mono", monospace',
                          whiteSpace: "nowrap",
                          overflow: "hidden",
                          textOverflow: "ellipsis",
                        }}
                        title={artifact.file_path}
                      >
                        {filename}
                      </div>
                    </div>
                  </div>

                  <div style={{ display: "flex", alignItems: "center", gap: 4, flexShrink: 0 }}>
                    <Tag minimal round style={{ fontSize: 9, padding: "1px 6px" }}>
                      {formatUpper}
                    </Tag>
                    <Button
                      icon={isCopied ? "tick" : "clipboard"}
                      minimal
                      small
                      intent={isCopied ? Intent.SUCCESS : Intent.NONE}
                      title={isCopied ? "Path copied!" : "Copy file path"}
                      onClick={() => void handleCopyArtifactPath(key, artifact.file_path)}
                    />
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* 5. Collapsible Raw JSON for Power Users / Developers */}
      <div
        style={{
          marginTop: 10,
          borderTop: "1px solid var(--color-hairline-silver, #e5e5ea)",
          paddingTop: 8,
        }}
      >
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
          }}
        >
          <Button
            minimal
            small
            icon={isRawJsonOpen ? "chevron-down" : "chevron-right"}
            text={isRawJsonOpen ? "Hide Raw Payload" : "Inspect Raw Payload"}
            style={{ fontSize: 11, color: "var(--color-slate, #707070)", padding: 0 }}
            onClick={() => setIsRawJsonOpen(!isRawJsonOpen)}
          />

          {isRawJsonOpen && (
            <Button
              minimal
              small
              icon={copiedJson ? "tick" : "clipboard"}
              text={copiedJson ? "Copied" : "Copy JSON"}
              intent={copiedJson ? Intent.SUCCESS : Intent.NONE}
              style={{ fontSize: 11 }}
              onClick={() => void handleCopyJson()}
            />
          )}
        </div>

        <Collapse isOpen={isRawJsonOpen}>
          <pre
            style={{
              margin: "8px 0 0",
              padding: "8px 10px",
              fontSize: "0.74em",
              fontFamily: '"DM Mono", monospace',
              background: "var(--color-studio-mist, #f5f5f7)",
              borderRadius: 6,
              maxHeight: 180,
              overflow: "auto",
              whiteSpace: "pre-wrap",
              wordBreak: "break-all",
              border: "1px solid var(--color-hairline-silver, #e5e5ea)",
            }}
          >
            {JSON.stringify(result, null, 2)}
          </pre>
        </Collapse>
      </div>
    </Card>
  );
};
