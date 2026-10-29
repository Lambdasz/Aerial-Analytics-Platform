import { useEffect, useState } from "react";
import { Button, Callout, HTMLSelect, Intent, Spinner, Tag } from "@blueprintjs/core";

import { loadImageThumbnail } from "../../lib/imagePreview";
import { commandErrorMessage, type Session } from "../module-01/sessionCommands";
import {
  type ComparisonRecord,
  type SessionComparisonReport,
  type SpatialChangeReport,
  compareSessions,
  detectChangeAreas,
  exportComparisonJson,
  getChangeSummary,
} from "./module10Commands";

function fileName(path: string): string {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

const LEVEL_INTENT = {
  rendah: Intent.NONE,
  sedang: Intent.WARNING,
  tinggi: Intent.DANGER,
} as const;

/** Thumbnail for one side of the comparison, loaded lazily from the backend. */
function ImageSide({ filePath, label }: { filePath: string; label: string }) {
  const [url, setUrl] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let cancelled = false;
    loadImageThumbnail(filePath)
      .then((loaded) => {
        if (!cancelled) setUrl(loaded);
      })
      .catch(() => {
        if (!cancelled) setFailed(true);
      });
    return () => {
      cancelled = true;
    };
  }, [filePath]);

  return (
    <figure className="m10-compare__card" style={{ margin: 0 }}>
      <figcaption className="m10-compare__label">{label}</figcaption>
      <div
        style={{
          height: 150,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          background: "var(--color-studio-mist, #f5f5f7)",
          borderRadius: 10,
          overflow: "hidden",
        }}
      >
        {url ? (
          <img
            src={url}
            alt={label}
            style={{ maxWidth: "100%", maxHeight: "100%", objectFit: "contain" }}
          />
        ) : failed ? (
          <span style={{ fontSize: 11, color: "var(--color-slate)" }}>preview unavailable</span>
        ) : (
          <Spinner size={20} />
        )}
      </div>
      <div className="m10-compare__file" title={filePath}>
        {fileName(filePath)}
      </div>
    </figure>
  );
}

/**
 * Phase 2 + 3 of Module 10: pick two sessions (dates), compare vegetation
 * coverage between them, show the before/after imagery, and list earlier
 * comparisons stored for the project.
 */
export function TemporalComparePanel({
  projectId,
  sessions,
}: {
  projectId: string;
  sessions: Session[];
}) {
  const [beforeOverride, setBeforeOverride] = useState<string | null>(null);
  const [afterOverride, setAfterOverride] = useState<string | null>(null);
  const [report, setReport] = useState<SessionComparisonReport | null>(null);
  const [history, setHistory] = useState<ComparisonRecord[]>([]);
  const [isRunning, setIsRunning] = useState(false);
  const [isExporting, setIsExporting] = useState(false);
  const [exportNotice, setExportNotice] = useState<string | null>(null);
  const [spatial, setSpatial] = useState<SpatialChangeReport | null>(null);
  const [isDetecting, setIsDetecting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Default the pair to the oldest and newest session, without writing state
  // from an effect: explicit picks win, otherwise the derived pair is used.
  const orderedSessions = [...sessions].sort((a, b) => a.dateStart.localeCompare(b.dateStart));
  const beforeId = beforeOverride ?? orderedSessions[0]?.id ?? "";
  const afterId = afterOverride ?? orderedSessions[orderedSessions.length - 1]?.id ?? "";

  // Comparison history for the project (state updates in promise callbacks).
  useEffect(() => {
    if (!projectId) return;
    let cancelled = false;
    getChangeSummary(projectId)
      .then((rows) => {
        if (!cancelled) setHistory(rows);
      })
      .catch(() => {
        if (!cancelled) setHistory([]);
      });
    return () => {
      cancelled = true;
    };
  }, [projectId]);

  const canCompare = beforeId !== "" && afterId !== "" && beforeId !== afterId && !isRunning;

  const exportReport = async () => {
    if (!report) return;
    setIsExporting(true);
    setExportNotice(null);
    try {
      const path = await exportComparisonJson(report.comparisonId);
      setExportNotice(`Saved to ${path}`);
    } catch (err) {
      setError(commandErrorMessage(err));
    } finally {
      setIsExporting(false);
    }
  };

  const runSpatial = async () => {
    setIsDetecting(true);
    setSpatial(null);
    setError(null);
    try {
      const result = await detectChangeAreas(beforeId, afterId);
      setSpatial(result);
    } catch (err) {
      setError(commandErrorMessage(err));
    } finally {
      setIsDetecting(false);
    }
  };

  const run = async () => {
    setIsRunning(true);
    setError(null);
    try {
      const result = await compareSessions(beforeId, afterId);
      setReport(result);
      setHistory(await getChangeSummary(projectId));
    } catch (err) {
      setError(commandErrorMessage(err));
      setReport(null);
    } finally {
      setIsRunning(false);
    }
  };

  return (
    <section className="m10-panel">
      <div className="m10-panel__head">
        <div>
          <h2 className="m10-panel__title">Bandingkan dua sesi</h2>
          <p className="m10-panel__subtitle">
            Pilih sesi baseline dan sesi pembanding; keduanya harus satu proyek. Vegetasi diukur
            dari piksel citra terakhir setiap sesi, hasilnya disimpan sebagai riwayat.
          </p>
        </div>
        <Tag minimal round className="m10-panel__badge">
          {history.length} riwayat
        </Tag>
      </div>

      <div className="m10-header__controls" style={{ flexWrap: "wrap", gap: 12 }}>
        <div className="m10-field">
          <span className="m10-field__label">Sesi baseline</span>
          <HTMLSelect
            fill
            value={beforeId}
            onChange={(event) => setBeforeOverride(event.currentTarget.value)}
            options={sessions.map((session) => ({
              label: `${session.name} (${session.dateStart.slice(0, 10)})`,
              value: session.id,
            }))}
          />
        </div>
        <div className="m10-field">
          <span className="m10-field__label">Sesi pembanding</span>
          <HTMLSelect
            fill
            value={afterId}
            onChange={(event) => setAfterOverride(event.currentTarget.value)}
            options={sessions.map((session) => ({
              label: `${session.name} (${session.dateStart.slice(0, 10)})`,
              value: session.id,
            }))}
          />
        </div>
        <Button
          intent={Intent.PRIMARY}
          icon="comparison"
          text="Bandingkan"
          loading={isRunning}
          disabled={!canCompare}
          onClick={() => void run()}
        />
      </div>

      {error && (
        <Callout intent={Intent.DANGER} title="Perbandingan gagal">
          {error}
        </Callout>
      )}

      {report && (
        <div className="m10-grid" style={{ marginTop: 16 }}>
          <ImageSide filePath={report.before.filePath} label="Sebelum" />
          <ImageSide filePath={report.after.filePath} label="Sesudah" />
          <div className="m10-delta">
            <div className="m10-delta__label">Perubahan tutupan vegetasi</div>
            <div className="m10-delta__value">
              {report.change.change > 0 ? "+" : ""}
              {report.change.change.toFixed(2)} poin persen
            </div>
            <div className="m10-delta__interval">
              {report.beforeCoverage.coveragePercent.toFixed(2)}% →{" "}
              {report.afterCoverage.coveragePercent.toFixed(2)}% · {report.intervalDays} hari
            </div>
            <Tag
              minimal
              round
              intent={LEVEL_INTENT[report.magnitudeLevel]}
              style={{ marginTop: 6 }}
            >
              Magnitudo: {report.magnitude.toFixed(1)} ({report.magnitudeLevel})
            </Tag>
          </div>
        </div>
      )}

      {report && (
        <div style={{ marginTop: 12 }}>
          {report.summary.vegetationChange && (
            <div className="m10-footnote">{report.summary.vegetationChange}</div>
          )}
          <Button
            small
            icon="export"
            text="Ekspor JSON"
            loading={isExporting}
            onClick={() => void exportReport()}
          />
          {exportNotice && (
            <div className="m10-footnote" style={{ marginTop: 4 }}>
              {exportNotice}
            </div>
          )}
        </div>
      )}

      <div style={{ marginTop: 12, display: "flex", alignItems: "center", gap: 8 }}>
        <Button
          small
          icon="map-marker"
          text="Deteksi area perubahan"
          loading={isDetecting}
          disabled={!canCompare}
          onClick={() => void runSpatial()}
        />
        <span className="m10-footnote">
          Membandingkan masker vegetasi kedua citra; area berubah dipublikasikan sebagai layer peta.
        </span>
      </div>

      {spatial && (
        <div className="m10-grid" style={{ marginTop: 12 }}>
          <div className="m10-compare__card">
            <div className="m10-compare__label">Vegetasi bertambah</div>
            <div className="m10-coverage__value">{spatial.gainAreaM2.toFixed(0)} m²</div>
          </div>
          <div className="m10-compare__card">
            <div className="m10-compare__label">Vegetasi berkurang</div>
            <div className="m10-coverage__value">{spatial.lossAreaM2.toFixed(0)} m²</div>
          </div>
          <div className="m10-delta">
            <div className="m10-delta__label">Area berubah</div>
            <div className="m10-delta__value">
              {spatial.changedFractionPercent.toFixed(1)}% ({spatial.magnitudeLevel})
            </div>
            <div className="m10-delta__interval">{spatial.features.length} region di peta</div>
            <Tag
              minimal
              round
              intent={LEVEL_INTENT[spatial.magnitudeLevel]}
              style={{ marginTop: 6 }}
            >
              Layer: {spatial.layerName}
            </Tag>
          </div>
        </div>
      )}

      {history.length > 0 && (
        <div style={{ marginTop: 16 }}>
          <div className="m10-field__label">Riwayat perbandingan</div>
          <div className="m10-observations">
            {history.slice(0, 6).map((row) => (
              <div key={row.id} className="m10-observation">
                <div className="m10-observation__file">
                  {row.beforeDate} → {row.afterDate}
                </div>
                <div className="m10-observation__meta">
                  {row.beforeValue.toFixed(2)}% → {row.afterValue.toFixed(2)}% (
                  {row.delta > 0 ? "+" : ""}
                  {row.delta.toFixed(2)}) · {row.magnitudeLevel} · {row.intervalDays} hari
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </section>
  );
}
