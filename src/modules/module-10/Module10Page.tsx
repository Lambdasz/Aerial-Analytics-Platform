import { useEffect, useMemo, useState } from "react";
import {
  Button,
  Callout,
  Card,
  HTMLSelect,
  Intent,
  NonIdealState,
  ProgressBar,
  Spinner,
  Tag,
} from "@blueprintjs/core";

import {
  type Session,
  commandErrorMessage,
  getSessionsByProject,
  isCommandError,
} from "../module-01/sessionCommands";
import { type Project, listProjects } from "../module-01/projectCommands";

import {
  type ImageObservation,
  type ObservationSkipReason,
  type SessionObservationPeriods,
  type VegetationChangeReport,
  getObservationPeriods,
  getVegetationChange,
} from "./module10Commands";

import { TemporalComparePanel } from "./TemporalComparePanel";

import "./Module10Page.css";

/** Same key Module 1 uses, so the project picked there carries over. */
const PROJECT_STORAGE_KEY = "m1_last_project_id";

function fileName(path: string): string {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

function readSavedProjectId(): string {
  try {
    const saved = localStorage.getItem(PROJECT_STORAGE_KEY);
    if (!saved) return "";
    const parsed = JSON.parse(saved) as { id?: string };
    return typeof parsed?.id === "string" ? parsed.id : "";
  } catch {
    return "";
  }
}

function saveProjectId(projectId: string) {
  try {
    const project = JSON.parse(localStorage.getItem(PROJECT_STORAGE_KEY) ?? "null") as {
      id?: string;
    } | null;
    if (projectId) {
      localStorage.setItem(
        PROJECT_STORAGE_KEY,
        JSON.stringify({ ...(project ?? {}), id: projectId }),
      );
    } else {
      localStorage.removeItem(PROJECT_STORAGE_KEY);
    }
  } catch {
    // localStorage is optional; the selection still works for this session.
  }
}

function formatPeriodDate(date: string): string {
  const parsed = new Date(`${date.slice(0, 10)}T00:00:00`);
  if (Number.isNaN(parsed.getTime())) return date;
  return parsed.toLocaleDateString("id-ID", {
    weekday: "long",
    day: "numeric",
    month: "long",
    year: "numeric",
  });
}

function formatTime(capturedAt: string): string {
  return capturedAt.slice(11, 16) || "--:--";
}

function formatPercent(value: number): string {
  return `${value.toLocaleString("id-ID", {
    minimumFractionDigits: 1,
    maximumFractionDigits: 1,
  })}%`;
}

function formatDelta(value: number): string {
  const formatted = Math.abs(value).toLocaleString("id-ID", {
    minimumFractionDigits: 1,
    maximumFractionDigits: 1,
  });
  return `${value > 0 ? "+" : value < 0 ? "-" : ""}${formatted} pp`;
}

function formatGps(observation: ImageObservation): string {
  if (observation.latitude === null || observation.longitude === null) return "GPS -";
  return `${observation.latitude.toFixed(5)}, ${observation.longitude.toFixed(5)}`;
}

function formatAltitude(altitudeM: number | null): string {
  return altitudeM === null ? "alt -" : `${altitudeM.toFixed(1)} m`;
}

function formatDimensions(observation: ImageObservation): string {
  if (observation.width === null || observation.height === null) return "-";
  return `${observation.width}x${observation.height}`;
}

function skipReasonLabel(reason: ObservationSkipReason): string {
  return reason === "missing_capture_time"
    ? "tidak punya waktu pengambilan (EXIF DateTimeOriginal)"
    : "ditandai belum siap dianalisis oleh Modul 1";
}

function directionLabel(change: number): string {
  if (change > 0) return "Tutupan vegetasi bertambah";
  if (change < 0) return "Tutupan vegetasi berkurang";
  return "Tidak ada perubahan tutupan";
}

function describeError(err: unknown): string {
  if (isCommandError(err)) {
    return `${err.message} (${err.code})`;
  }

  const message = commandErrorMessage(err);
  if (/not found|unknown command|not registered/i.test(message)) {
    return `${message} - command Modul 10 belum terdaftar di backend build ini. Pastikan src-tauri/src/modules/module_10 terdaftar di lib.rs, lalu build ulang aplikasi.`;
  }
  return message;
}

/** Result of 10.1 + 10.2 + 10.3, tagged with the session it belongs to. */
type AnalysisState = {
  sessionId: string;
  periods: SessionObservationPeriods | null;
  report: VegetationChangeReport | null;
  error: string | null;
};

/** Sessions are tagged with their project so a stale list is never rendered. */
type SessionsState = {
  projectId: string;
  list: Session[];
};

export function Module10Page() {
  const [projects, setProjects] = useState<Project[]>([]);
  const [projectId, setProjectId] = useState("");
  const [sessionsState, setSessionsState] = useState<SessionsState>({ projectId: "", list: [] });
  const [sessionId, setSessionId] = useState("");
  const [analysis, setAnalysis] = useState<AnalysisState | null>(null);
  const [reloadToken, setReloadToken] = useState(0);
  const [pageError, setPageError] = useState<string | null>(null);
  const [loadingProjects, setLoadingProjects] = useState(true);

  // Module 1.4 is the data source: list its projects first.
  useEffect(() => {
    let alive = true;

    listProjects()
      .then((list) => {
        if (!alive) return;
        setProjects(list);
        const saved = readSavedProjectId();
        const initial = list.find((project) => project.id === saved) ?? list[0];
        setProjectId(initial?.id ?? "");
      })
      .catch((err: unknown) => {
        if (alive) setPageError(describeError(err));
      })
      .finally(() => {
        if (alive) setLoadingProjects(false);
      });

    return () => {
      alive = false;
    };
  }, []);

  // Sessions belong to a project; default to the first active one.
  useEffect(() => {
    if (!projectId) return;

    let alive = true;

    getSessionsByProject(projectId)
      .then((list) => {
        if (!alive) return;
        setSessionsState({ projectId, list });
        const first = list.find((session) => session.status === "active") ?? list[0];
        setSessionId(first?.id ?? "");
      })
      .catch((err: unknown) => {
        if (alive) setPageError(describeError(err));
      });

    return () => {
      alive = false;
    };
  }, [projectId]);

  // Derived from the tagged states: no reset writes inside effects.
  const sessions = useMemo(
    () => (sessionsState.projectId === projectId ? sessionsState.list : []),
    [sessionsState, projectId],
  );
  const loadingSessions = projectId !== "" && sessionsState.projectId !== projectId;
  const selectedSessionId = sessions.some((session) => session.id === sessionId) ? sessionId : "";
  const loadingAnalysis = selectedSessionId !== "" && analysis?.sessionId !== selectedSessionId;
  const periods = analysis?.sessionId === selectedSessionId ? analysis.periods : null;
  const report = analysis?.sessionId === selectedSessionId ? analysis.report : null;
  const analysisError = analysis?.sessionId === selectedSessionId ? analysis.error : null;

  // 10.1 then 10.2 + 10.3 for the selected session.
  useEffect(() => {
    if (!selectedSessionId) return;

    let alive = true;

    getObservationPeriods(selectedSessionId)
      .then(async (result) => {
        if (!alive) return;
        if (result.periods.length < 2) {
          setAnalysis({
            sessionId: selectedSessionId,
            periods: result,
            report: null,
            error: null,
          });
          return;
        }

        const change = await getVegetationChange(selectedSessionId);
        if (!alive) return;
        setAnalysis({
          sessionId: selectedSessionId,
          periods: result,
          report: change,
          error: null,
        });
      })
      .catch((err: unknown) => {
        if (!alive) return;
        setAnalysis({
          sessionId: selectedSessionId,
          periods: null,
          report: null,
          error: describeError(err),
        });
      });

    return () => {
      alive = false;
    };
  }, [selectedSessionId, reloadToken]);

  const selectedSession = useMemo(
    () => sessions.find((session) => session.id === selectedSessionId) ?? null,
    [sessions, selectedSessionId],
  );

  const observationCount = useMemo(
    () => periods?.periods.reduce((total, period) => total + period.observations.length, 0) ?? 0,
    [periods],
  );

  const handleProjectChange = (nextProjectId: string) => {
    setProjectId(nextProjectId);
    setPageError(null);
    saveProjectId(nextProjectId);
  };

  const hasEnoughPeriods = (periods?.periods.length ?? 0) >= 2;

  return (
    <main className="m10-page">
      <header className="m10-panel m10-header">
        <div className="m10-header__info">
          <div className="m10-header__tag-row">
            <Tag minimal>Modul 10</Tag>
          </div>
          <h1 className="m10-header__title">Analisis Perubahan Temporal</h1>
          <p className="m10-header__subtitle">
            Membandingkan citra udara antar tanggal pengambilan. Data citra dan metadata diambil
            langsung dari Modul 1.4 (sesi penerbangan), bukan data contoh.
          </p>
        </div>

        <div className="m10-header__controls">
          <label className="m10-field">
            <span className="m10-field__label">Proyek</span>
            <HTMLSelect
              fill
              value={projectId}
              onChange={(event) => handleProjectChange(event.currentTarget.value)}
              disabled={loadingProjects || projects.length === 0}
            >
              {projects.length === 0 && <option value="">Tidak ada proyek</option>}
              {projects.map((project) => (
                <option key={project.id} value={project.id}>
                  {project.name}
                </option>
              ))}
            </HTMLSelect>
          </label>

          <label className="m10-field">
            <span className="m10-field__label">Sesi penerbangan</span>
            <HTMLSelect
              fill
              value={selectedSessionId}
              onChange={(event) => setSessionId(event.currentTarget.value)}
              disabled={loadingSessions || sessions.length === 0}
            >
              {sessions.length === 0 && <option value="">Tidak ada sesi</option>}
              {sessions.map((session) => (
                <option key={session.id} value={session.id}>
                  {session.name}
                  {session.status === "archived" ? " (arsip)" : ""}
                </option>
              ))}
            </HTMLSelect>
          </label>

          <Button
            icon="refresh"
            text="Muat ulang"
            onClick={() => setReloadToken((token) => token + 1)}
            disabled={!selectedSessionId || loadingAnalysis}
          />
        </div>
      </header>

      {projectId !== "" && sessions.length > 0 && (
        <TemporalComparePanel projectId={projectId} sessions={sessions} />
      )}

      {pageError && (
        <Callout intent={Intent.DANGER} icon="error" title="Gagal memuat data Modul 1.4">
          {pageError}
        </Callout>
      )}

      {!loadingProjects && projects.length === 0 && (
        <Card className="m10-panel m10-state">
          <NonIdealState
            icon="folder-open"
            title="Belum ada proyek"
            description="Modul 10 membaca data dari Modul 1.4. Buat proyek dan impor citra di Modul 1 terlebih dahulu."
          />
        </Card>
      )}

      {projects.length > 0 && !selectedSession && (
        <Card className="m10-panel m10-state">
          <NonIdealState
            icon="timeline-events"
            title="Pilih sesi penerbangan"
            description={
              loadingSessions
                ? "Memuat sesi..."
                : "Sesi yang dipilih menentukan citra mana yang dibandingkan."
            }
          />
        </Card>
      )}

      {selectedSession && (
        <div className="m10-grid">
          <section className="m10-panel m10-panel--periods" aria-labelledby="m10-periods-heading">
            <div className="m10-panel__head">
              <div>
                <h2 id="m10-periods-heading" className="m10-panel__title">
                  10.1 Periode Pengamatan
                </h2>
                <p className="m10-panel__subtitle">
                  Citra sesi <strong>{selectedSession.name}</strong> dikelompokkan menurut tanggal
                  di metadata EXIF, lalu diurutkan secara kronologis.
                </p>
              </div>
              {periods && (
                <Tag minimal className="m10-panel__badge">
                  {periods.periods.length} periode / {observationCount} citra
                </Tag>
              )}
            </div>

            {loadingAnalysis && !periods ? (
              <div className="m10-loading">
                <Spinner size={20} />
                <span>Membaca citra dan metadata dari Modul 1.4...</span>
              </div>
            ) : !periods || periods.periods.length === 0 ? (
              <NonIdealState
                icon="calendar"
                title="Belum ada citra bertanggal"
                description="Sesi ini belum punya citra dengan waktu pengambilan (EXIF DateTimeOriginal), sehingga belum bisa dibagi ke periode."
              />
            ) : (
              <div className="m10-periods">
                {periods.periods.map((period) => (
                  <Card key={period.date} className="m10-period">
                    <div className="m10-period__head">
                      <span className="m10-period__date">{formatPeriodDate(period.date)}</span>
                      <Tag minimal>{period.observations.length} citra</Tag>
                    </div>

                    <ul className="m10-observations">
                      {period.observations.map((observation) => (
                        <li key={observation.imageId} className="m10-observation">
                          <span className="m10-observation__time">
                            {formatTime(observation.capturedAt)}
                          </span>
                          <span className="m10-observation__file" title={observation.filePath}>
                            {fileName(observation.filePath)}
                          </span>
                          <span className="m10-observation__meta">{formatGps(observation)}</span>
                          <span className="m10-observation__meta">
                            {formatAltitude(observation.altitudeM)}
                          </span>
                          <span className="m10-observation__meta">
                            {formatDimensions(observation)}
                          </span>
                        </li>
                      ))}
                    </ul>
                  </Card>
                ))}
              </div>
            )}

            {periods && periods.skipped.length > 0 && (
              <Callout
                className="m10-skipped-callout"
                intent={Intent.WARNING}
                icon="warning-sign"
                title={`${periods.skipped.length} citra dilewati`}
              >
                <ul className="m10-skipped">
                  {periods.skipped.map((skipped) => (
                    <li key={skipped.imageId}>
                      <span className="m10-skipped__file" title={skipped.filePath}>
                        {fileName(skipped.filePath)}
                      </span>
                      <span className="m10-skipped__reason">{skipReasonLabel(skipped.reason)}</span>
                    </li>
                  ))}
                </ul>
              </Callout>
            )}
          </section>

          <section className="m10-panel m10-panel--change" aria-labelledby="m10-change-heading">
            <div className="m10-panel__head">
              <div>
                <h2 id="m10-change-heading" className="m10-panel__title">
                  10.2 - 10.3 Perbandingan &amp; Tutupan Vegetasi
                </h2>
                <p className="m10-panel__subtitle">
                  Dua periode terakhir dibandingkan. Tutupan vegetasi diukur dari piksel citra
                  (indeks ExG sementara, sampai hasil plugin Modul 4/5 tersedia).
                </p>
              </div>
            </div>

            {loadingAnalysis && (
              <div className="m10-loading">
                <Spinner size={20} />
                <span>Menghitung perubahan...</span>
              </div>
            )}

            {analysisError && !loadingAnalysis && (
              <Callout intent={Intent.DANGER} icon="error" title="Analisis tidak selesai">
                <div className="m10-error-body">
                  <span>{analysisError}</span>
                  <Button
                    small
                    text="Coba lagi"
                    onClick={() => setReloadToken((token) => token + 1)}
                    disabled={loadingAnalysis}
                  />
                </div>
              </Callout>
            )}

            {!loadingAnalysis && !analysisError && periods && !hasEnoughPeriods && (
              <Callout intent={Intent.PRIMARY} icon="info-sign" title="Butuh dua periode">
                Sesi ini baru punya {periods.periods.length} tanggal pengambilan. Tambahkan citra
                dari tanggal lain agar perubahan bisa dihitung.
              </Callout>
            )}

            {!loadingAnalysis && !analysisError && report && (
              <>
                <div className="m10-compare">
                  {(
                    [
                      {
                        label: "Before",
                        observation: report.before,
                        coverage: report.beforeCoverage,
                      },
                      { label: "After", observation: report.after, coverage: report.afterCoverage },
                    ] as const
                  ).map(({ label, observation, coverage }) => (
                    <Card key={label} className="m10-compare__card">
                      <span className="m10-compare__label">{label}</span>
                      <strong className="m10-compare__date">
                        {formatPeriodDate(observation.capturedAt)}
                      </strong>
                      <span className="m10-compare__file" title={observation.filePath}>
                        {fileName(observation.filePath)}
                      </span>

                      <div className="m10-coverage">
                        <span className="m10-coverage__value">
                          {formatPercent(coverage.coveragePercent)}
                        </span>
                        <ProgressBar
                          value={Math.min(1, Math.max(0, coverage.coveragePercent / 100))}
                          intent={coverage.coveragePercent >= 50 ? Intent.SUCCESS : Intent.WARNING}
                        />
                        <span className="m10-coverage__detail">
                          {coverage.vegetationPixels.toLocaleString("id-ID")} dari{" "}
                          {coverage.sampledPixels.toLocaleString("id-ID")} piksel
                        </span>
                      </div>
                    </Card>
                  ))}
                </div>

                <div className="m10-delta">
                  <span className="m10-delta__value">{formatDelta(report.change.change)}</span>
                  <span className="m10-delta__label">{directionLabel(report.change.change)}</span>
                  <span className="m10-delta__interval">
                    Jarak pengamatan: {report.intervalDays} hari
                  </span>
                </div>

                <div className="m10-contract">
                  <h3 className="m10-contract__title">Payload untuk Modul 11.5</h3>
                  <pre className="m10-contract__json">
                    {JSON.stringify(
                      {
                        before: report.change.before,
                        after: report.change.after,
                        delta: report.change.change,
                      },
                      null,
                      2,
                    )}
                  </pre>
                </div>
              </>
            )}

            <p className="m10-footnote">
              10.4 - 10.7 sudah tersedia sebagai fungsi murni Rust dan command. Input kelas, jumlah
              pohon, dan nilai per lokasi berasal dari Modul 3 - 5. 10.8 memakai kartu before/after
              di atas; integrasi peta Modul 3 dan dashboard Modul 11 menyusul.
            </p>
          </section>
        </div>
      )}
    </main>
  );
}
