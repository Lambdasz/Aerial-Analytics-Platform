import { useEffect, useRef, useState } from "react";
import {
  Button,
  ButtonGroup,
  Callout,
  Dialog,
  HTMLSelect,
  Intent,
  ProgressBar,
  Spinner,
} from "@blueprintjs/core";
import { listen } from "@tauri-apps/api/event";

import {
  abortPluginJob,
  getInstalledPlugins,
  getJobResult,
  getJobStatus,
  startPluginJob,
} from "../../component_plugin_manager/pluginCommands";
import type { PluginSummary } from "../../types/plugin";
import { listProjects, type Project } from "../module-01/projectCommands";
import {
  getImagesBySession,
  getSessionsByProject,
  type Session,
  type StoredImage,
} from "../module-01/sessionCommands";
import { listOrthoImages } from "../module-03/services/api";
import type { StoredImageMetadata } from "../module-03/types/storage";
import { errorMessage } from "./plotAnalytics";
import {
  analysePlotsFromOrtho,
  savePlotIndicators,
  type PlotAnalysisProgressEvent,
  type PlotRecord,
} from "./plotCommands";

interface JobStatusPayload {
  state?: string;
  data?: { error?: string };
}

type SourceType = "session" | "ortho";
type JobVerdict = "completed" | "failed";

/**
 * FR-9.3.4 / ISU-9.4: a session photo is not clipped to the plot yet, so
 * every plot would get nearly the same value. Re-enable only after a
 * plot/footprint intersection check exists (or plugins clip to the AOI).
 */
const SESSION_SOURCE_ENABLED = false;

const POLL_INTERVAL_MS = 700;

const MIME_RULES: ReadonlyArray<[string, (format: string, extension: string) => boolean]> = [
  ["image/png", (format, extension) => format === "png" || extension === "png"],
  [
    "image/tiff",
    (format, extension) => format === "tiff" || extension === "tif" || extension === "tiff",
  ],
];

const mimeFor = (image: StoredImage): string => {
  const format = (image.format ?? "").toLowerCase();
  const extension = image.filePath.split(".").pop()?.toLowerCase() ?? "";
  return MIME_RULES.find(([, matches]) => matches(format, extension))?.[0] ?? "image/jpeg";
};

const JOB_VERDICTS: Record<string, JobVerdict> = {
  completed: "completed",
  failed: "failed",
  aborted: "failed",
};

const sleep = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

/**
 * Runs one plugin over every selected plot, stores the resulting indicators,
 * and reports progress. The source is an orthomosaic stored in the
 * application, clipped per plot by the backend; the flight-session source is
 * disabled for now (see `SESSION_SOURCE_ENABLED`).
 */
export function AnalysePlotsDialog({
  isOpen,
  plots,
  onClose,
  onFinished,
}: {
  isOpen: boolean;
  plots: PlotRecord[];
  onClose: () => void;
  onFinished: () => void;
}) {
  const [plugins, setPlugins] = useState<PluginSummary[]>([]);
  const [projects, setProjects] = useState<Project[]>([]);
  const [sessions, setSessions] = useState<Session[]>([]);
  const [images, setImages] = useState<StoredImage[]>([]);

  const [sourceType, setSourceType] = useState<SourceType>("ortho");
  const [orthos, setOrthos] = useState<StoredImageMetadata[]>([]);
  const [orthoId, setOrthoId] = useState("");

  const [pluginId, setPluginId] = useState("");
  const [projectId, setProjectId] = useState("");
  const [sessionId, setSessionId] = useState("");
  const [imageId, setImageId] = useState("");

  const [isLoading, setIsLoading] = useState(true);
  const [isRunning, setIsRunning] = useState(false);
  const [done, setDone] = useState(0);
  const [currentPlot, setCurrentPlot] = useState<string | null>(null);
  const [progressMessage, setProgressMessage] = useState<string | null>(null);
  const [progressPercent, setProgressPercent] = useState<number | null>(null);
  const [errors, setErrors] = useState<string[]>([]);
  const [finished, setFinished] = useState(false);

  // React refs are the one place we assign: they are React's API for values
  // that outlive renders without re-rendering (NFR-9.1.7 exception).
  const abortRef = useRef(false);
  const activeJobRef = useRef<string | null>(null);

  // Load plugins, projects, and orthomosaics when the dialog opens.
  useEffect(() => {
    const controller = new AbortController();
    const live = () => !controller.signal.aborted;
    const request = isOpen
      ? Promise.all([getInstalledPlugins(), listProjects(), listOrthoImages()])
      : null;
    const apply = ([installed, projectList, orthoList]: [
      PluginSummary[],
      Project[],
      StoredImageMetadata[] | null | undefined,
    ]) => {
      const enabled = installed.filter((plugin) => plugin.enabled);
      const validOrthos = (orthoList ?? []).filter(
        (ortho) => ortho.bounds && !ortho.is_corrupted && ortho.id,
      );
      setPlugins(enabled);
      setPluginId((previous) => previous || enabled[0]?.id || "");
      setProjects(projectList);
      setProjectId((previous) => previous || projectList[0]?.id || "");
      setOrthos(validOrthos);
      setOrthoId((previous) => previous || validOrthos[0]?.id || "");
      setIsLoading(false);
    };
    request?.then((loaded) => live() && apply(loaded)).catch(() => live() && setIsLoading(false));
    return () => controller.abort();
  }, [isOpen]);

  // Load sessions whenever the selected project changes.
  useEffect(() => {
    const controller = new AbortController();
    const live = () => !controller.signal.aborted;
    const apply = (loaded: Session[]) => {
      setSessions(loaded);
      setSessionId(loaded[0]?.id ?? "");
      setImages([]);
      setImageId("");
    };
    const request = projectId ? getSessionsByProject(projectId) : null;
    request?.then((loaded) => live() && apply(loaded)).catch(() => live() && apply([]));
    return () => controller.abort();
  }, [projectId]);

  // Load analysis-ready images whenever the selected session changes.
  useEffect(() => {
    const controller = new AbortController();
    const live = () => !controller.signal.aborted;
    const apply = (usable: StoredImage[]) => {
      setImages(usable);
      setImageId(usable[0]?.id ?? "");
    };
    const request = sessionId ? getImagesBySession(sessionId) : null;
    request
      ?.then((loaded) => live() && apply(loaded.filter((image) => image.analysisReady !== false)))
      .catch(() => live() && apply([]));
    return () => controller.abort();
  }, [sessionId]);

  const selectedImage = images.find((image) => image.id === imageId) ?? null;
  const selectedOrtho = orthos.find((ortho) => ortho.id === orthoId) ?? null;

  const canRun = Boolean(
    pluginId &&
    plots.length > 0 &&
    !isRunning &&
    (sourceType === "session" ? SESSION_SOURCE_ENABLED && selectedImage : selectedOrtho?.id),
  );

  /** Polls a job until it finishes; recursion replaces the polling loop. */
  const waitForJob = (jobId: string): Promise<JobVerdict> =>
    abortRef.current
      ? Promise.resolve("failed")
      : getJobStatus(jobId)
          .then(
            (status) =>
              JOB_VERDICTS[((status as JobStatusPayload)?.state ?? "").toLowerCase()] ?? null,
          )
          .catch((): JobVerdict => "failed")
          .then((verdict) => verdict ?? sleep(POLL_INTERVAL_MS).then(() => waitForJob(jobId)));

  // Orthomosaic processing is handled by the native raster pipeline.
  const runOrtho = (ortho: StoredImageMetadata & { id: string }): Promise<void> => {
    setProgressMessage("Memulai pemotongan georeferenced raster...");
    setProgressPercent(0);
    return listen<PlotAnalysisProgressEvent>("plot-analysis-progress", (event) => {
      setProgressMessage(event.payload.message);
      setProgressPercent(event.payload.percent);
      setDone(event.payload.plotIndex);
      setCurrentPlot(event.payload.plotName);
    })
      .then((unlisten) =>
        analysePlotsFromOrtho({
          orthoId: ortho.id,
          plotIds: plots.map((plot) => plot.id),
          pluginId,
        })
          .then((outcome) => {
            setErrors(outcome.failedPlots);
            setDone(outcome.successfulPlots);
            setFinished(true);
            onFinished();
          })
          .finally(unlisten),
      )
      .catch((error) => setErrors([errorMessage(error)]))
      .finally(() => {
        setIsRunning(false);
        setProgressMessage(null);
        setProgressPercent(null);
        setCurrentPlot(null);
      });
  };

  /** Reads a finished job and stores its metrics; resolves to a failure message or null. */
  const storeJobResult = (plot: PlotRecord, jobId: string): Promise<string | null> =>
    getJobResult(jobId).then((result) =>
      String(result.status).toLowerCase() === "failure"
        ? `${plot.name}: ${result.error ?? "plugin reported failure"}`
        : savePlotIndicators(plot.id, pluginId, jobId, result.metrics).then(() => null),
    );

  /** One plugin job for one plot; resolves to a failure message or null. */
  const runPlotJob = (image: StoredImage, plot: PlotRecord): Promise<string | null> => {
    setCurrentPlot(plot.name);
    return startPluginJob({
      plugin_id: pluginId,
      session_id: image.sessionId,
      target_image_path: image.filePath,
      mime_type: mimeFor(image),
      aoi: {
        type: "geojson_polygon",
        geometry: plot.geometry,
      },
      parameters: {},
    })
      .then((handle) => {
        activeJobRef.current = handle.job_id;
        return waitForJob(handle.job_id).then((state) => {
          activeJobRef.current = null;
          return state === "completed"
            ? storeJobResult(plot, handle.job_id)
            : `${plot.name}: plugin run failed`;
        });
      })
      .catch((error) => {
        activeJobRef.current = null;
        return `${plot.name}: ${errorMessage(error)}`;
      });
  };

  // Flight-session processing runs one plugin job per selected plot, in order.
  const runSession = (image: StoredImage): Promise<void> =>
    plots
      .reduce<Promise<string[]>>(
        (previous, plot, index) =>
          previous.then((failures) =>
            abortRef.current
              ? failures
              : runPlotJob(image, plot)
                  .then((failure) => (failure === null ? failures : [...failures, failure]))
                  .finally(() => setDone(index + 1)),
          ),
        Promise.resolve([]),
      )
      .then((failures) => {
        setCurrentPlot(null);
        setIsRunning(false);
        setFinished(true);
        setErrors(failures);
        onFinished();
      });

  const runners: Record<SourceType, () => Promise<void>> = {
    ortho: () =>
      selectedOrtho?.id
        ? runOrtho({ ...selectedOrtho, id: selectedOrtho.id })
        : Promise.resolve(setIsRunning(false)),
    session: () =>
      selectedImage ? runSession(selectedImage) : Promise.resolve(setIsRunning(false)),
  };

  const run = (): Promise<void> => {
    abortRef.current = false;
    setIsRunning(true);
    setFinished(false);
    setDone(0);
    setErrors([]);
    setCurrentPlot(null);
    return runners[sourceType]();
  };

  const cancel = (): Promise<void> => {
    abortRef.current = true;
    const jobId = activeJobRef.current;
    // The polling stops on abortRef even when the abort command fails.
    return jobId
      ? abortPluginJob(jobId).then(
          () => undefined,
          () => undefined,
        )
      : Promise.resolve();
  };

  const closeDialog = () => {
    void (isRunning ? cancel() : undefined);
    onClose();
  };

  const percent =
    progressPercent ?? (plots.length === 0 ? 0 : Math.round((done / plots.length) * 100));

  return (
    <Dialog
      isOpen={isOpen}
      onClose={closeDialog}
      title={`Analyse ${plots.length} plot${plots.length === 1 ? "" : "s"}`}
      style={{ width: 560, borderRadius: "16px" }}
      canEscapeKeyClose={!isRunning}
    >
      <div
        className="bp6-dialog-body"
        style={{
          padding: "16px 20px",
          maxHeight: 520,
          overflowY: "auto",
        }}
      >
        {isLoading && (
          <div style={{ textAlign: "center", padding: 24 }}>
            <Spinner size={28} />
          </div>
        )}
        {!isLoading && (
          <>
            <div>
              <div
                style={{
                  fontSize: 12,
                  fontWeight: 600,
                  color: "#64748b",
                  marginBottom: 6,
                }}
              >
                Plugin
              </div>
              <HTMLSelect
                fill
                value={pluginId}
                disabled={plugins.length === 0 || isRunning}
                onChange={(event) => setPluginId(event.currentTarget.value)}
                options={[
                  ...(plugins.length === 0 ? [{ label: "No enabled plugins", value: "" }] : []),
                  ...plugins.map((plugin) => ({
                    label: `${plugin.name} (${plugin.runtime_type})`,
                    value: plugin.id,
                  })),
                ]}
              />
            </div>

            <div className="bp6-label" style={{ marginTop: 12 }}>
              Source Image Mode
            </div>

            <ButtonGroup fill style={{ marginBottom: 12 }}>
              <Button
                text="Flight Session"
                title={
                  SESSION_SOURCE_ENABLED
                    ? undefined
                    : "Temporarily disabled: a session photo is not clipped per plot yet"
                }
                intent={sourceType === "session" ? Intent.PRIMARY : Intent.NONE}
                onClick={() => setSourceType("session")}
                disabled={isRunning || !SESSION_SOURCE_ENABLED}
              />
              <Button
                text="Map Orthomosaic"
                intent={sourceType === "ortho" ? Intent.PRIMARY : Intent.NONE}
                onClick={() => setSourceType("ortho")}
                disabled={isRunning}
              />
            </ButtonGroup>

            {!SESSION_SOURCE_ENABLED && (
              <Callout
                intent={Intent.WARNING}
                icon="warning-sign"
                style={{ marginBottom: 12, borderRadius: 10, fontSize: 12 }}
              >
                Analisis per plot sementara hanya dari orthomosaic. Sumber Flight Session
                dinonaktifkan karena foto belum dipotong mengikuti batas plot, sehingga semua plot
                bisa mendapat nilai yang hampir sama.
              </Callout>
            )}

            {sourceType === "ortho" && (
              <>
                <div className="bp6-label">Orthomosaic ({orthos.length} available)</div>
                <HTMLSelect
                  fill
                  value={orthoId}
                  disabled={orthos.length === 0 || isRunning}
                  placeholder={
                    orthos.length === 0
                      ? "No georeferenced orthomosaic in storage"
                      : "Select an orthomosaic"
                  }
                  onChange={(event) => setOrthoId(event.currentTarget.value)}
                  options={orthos.map((ortho) => ({
                    label: `${ortho.filename || ortho.id || "unnamed"} ${
                      ortho.dimensions ? `(${ortho.dimensions[0]}×${ortho.dimensions[1]} px)` : ""
                    }`,
                    value: ortho.id ?? "",
                  }))}
                />

                <div
                  style={{
                    marginTop: 6,
                    fontSize: 11,
                    color: "var(--color-slate)",
                    lineHeight: 1.4,
                  }}
                >
                  Sub-citra plot akan dipotong (<i>clipped</i>) dan dibagi menjadi ubin (
                  <i>sliced</i>, maks. 1024×1024 px) secara otomatis dari orthomosaic untuk mencegah
                  lonjakan memori RAM (OOM).
                </div>
              </>
            )}

            {sourceType === "session" && (
              <>
                <div className="bp6-label">Project</div>
                <HTMLSelect
                  fill
                  value={projectId}
                  disabled={projects.length === 0 || isRunning}
                  onChange={(event) => {
                    setProjectId(event.currentTarget.value);
                    setSessions([]);
                    setSessionId("");
                    setImages([]);
                    setImageId("");
                  }}
                  options={projects.map((project) => ({
                    label: project.name,
                    value: project.id,
                  }))}
                />

                <div className="bp6-label" style={{ marginTop: 12 }}>
                  Session
                </div>
                <HTMLSelect
                  fill
                  value={sessionId}
                  disabled={sessions.length === 0 || isRunning}
                  placeholder="No sessions in this project"
                  onChange={(event) => {
                    setSessionId(event.currentTarget.value);
                    setImages([]);
                    setImageId("");
                  }}
                  options={sessions.map((session) => ({
                    label: `${session.name} (${session.dateStart?.slice(0, 10) || "no date"})`,
                    value: session.id,
                  }))}
                />

                <div className="bp6-label" style={{ marginTop: 12 }}>
                  Image ({images.length} available)
                </div>
                <HTMLSelect
                  fill
                  value={imageId}
                  disabled={images.length === 0 || isRunning}
                  placeholder="No analysis-ready image"
                  onChange={(event) => setImageId(event.currentTarget.value)}
                  options={images.map((image) => ({
                    label: image.filePath.split(/[/\\]/).pop() ?? image.filePath,
                    value: image.id,
                  }))}
                />
              </>
            )}

            {isRunning && (
              <div style={{ marginTop: 16 }}>
                <ProgressBar value={percent / 100} intent={Intent.PRIMARY} animate={false} />
                <div
                  style={{
                    marginTop: 6,
                    fontSize: 12,
                    color: "var(--color-slate)",
                  }}
                >
                  {progressMessage ??
                    `${done}/${plots.length} plots · ${currentPlot ?? "starting"}`}
                </div>
              </div>
            )}

            {finished && errors.length === 0 && (
              <Callout intent={Intent.SUCCESS} style={{ marginTop: 16, borderRadius: 10 }}>
                Indicators stored for {done} plot{done === 1 ? "" : "s"}.
              </Callout>
            )}

            {errors.length > 0 && (
              <Callout
                intent={Intent.WARNING}
                title="Some plots failed"
                style={{ marginTop: 16, borderRadius: 10 }}
              >
                <ul style={{ margin: 0, paddingLeft: 18 }}>
                  {errors.map((entry) => (
                    <li key={entry}>{entry}</li>
                  ))}
                </ul>
              </Callout>
            )}

            <p
              style={{
                marginTop: 16,
                marginBottom: 0,
                fontSize: 11,
                color: "#64748b",
              }}
            >
              The orthomosaic is clipped to each selected plot and the plugin runs per slice.
              Existing indicator values for the same plugin are replaced.
            </p>
          </>
        )}
      </div>

      <div className="bp6-dialog-footer">
        <div className="bp6-dialog-footer-actions">
          {isRunning && (
            <Button
              intent={Intent.DANGER}
              text="Stop"
              minimal
              onClick={() => void cancel()}
              style={{
                borderRadius: 999,
                background: "#ef4444",
                color: "#ffffff",
                fontWeight: 600,
                padding: "6px 16px",
                boxShadow: "none",
              }}
            />
          )}
          {!isRunning && (
            <>
              <Button
                text="Close"
                minimal
                onClick={onClose}
                style={{
                  borderRadius: 999,
                  background: "#f1f5f9",
                  color: "#475569",
                  fontWeight: 600,
                  padding: "6px 16px",
                  boxShadow: "none",
                }}
              />
              <Button
                minimal
                icon="play"
                text={finished ? "Run again" : "Run analysis"}
                disabled={!canRun}
                onClick={() => void (canRun ? run() : undefined)}
                style={{
                  borderRadius: 999,
                  background: !canRun ? "#f1f5f9" : "#0f172a",
                  color: !canRun ? "#94a3b8" : "#ffffff",
                  fontWeight: 600,
                  padding: "6px 18px",
                  boxShadow: !canRun ? "none" : "0 2px 8px rgba(15,23,42,0.15)",
                }}
              />
            </>
          )}
        </div>
      </div>
    </Dialog>
  );
}
