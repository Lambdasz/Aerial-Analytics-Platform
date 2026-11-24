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

function message(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  if (error && typeof error === "object" && "message" in error) {
    return String(error.message);
  }
  return "Unexpected error.";
}

function mimeFor(image: StoredImage): string {
  const format = (image.format ?? "").toLowerCase();
  const extension = image.filePath.split(".").pop()?.toLowerCase() ?? "";

  if (format === "png" || extension === "png") return "image/png";
  if (format === "tiff" || extension === "tif" || extension === "tiff") {
    return "image/tiff";
  }
  return "image/jpeg";
}

const POLL_INTERVAL_MS = 700;

/**
 * Runs one plugin over every selected plot, stores the resulting indicators,
 * and reports progress. The source can be a flight-session image or an
 * orthomosaic stored in the application.
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

  const [sourceType, setSourceType] = useState<"session" | "ortho">("session");
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

  const abortRef = useRef(false);
  const activeJobRef = useRef<string | null>(null);

  // Load plugins, projects, and orthomosaics when the dialog opens.
  useEffect(() => {
    if (!isOpen) return;

    let cancelled = false;

    Promise.all([getInstalledPlugins(), listProjects(), listOrthoImages()])
      .then(([installed, projectList, orthoList]) => {
        if (cancelled) return;

        const enabled = installed.filter((plugin) => plugin.enabled);
        setPlugins(enabled);
        setPluginId((previous) => previous || enabled[0]?.id || "");

        setProjects(projectList);
        setProjectId((previous) => previous || projectList[0]?.id || "");

        const validOrthos = (orthoList ?? []).filter(
          (ortho) => ortho.bounds && !ortho.is_corrupted && ortho.id,
        );
        setOrthos(validOrthos);
        setOrthoId((previous) => previous || validOrthos[0]?.id || "");
        setIsLoading(false);
      })
      .catch(() => {
        if (!cancelled) setIsLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [isOpen]);

  // Load sessions whenever the selected project changes.
  useEffect(() => {
    if (!projectId) {
      setSessions([]);
      setSessionId("");
      setImages([]);
      setImageId("");
      return;
    }

    let cancelled = false;

    getSessionsByProject(projectId)
      .then((loaded) => {
        if (cancelled) return;

        setSessions(loaded);
        setSessionId(loaded[0]?.id ?? "");
        setImages([]);
        setImageId("");
      })
      .catch(() => {
        if (!cancelled) {
          setSessions([]);
          setSessionId("");
          setImages([]);
          setImageId("");
        }
      });

    return () => {
      cancelled = true;
    };
  }, [projectId]);

  // Load analysis-ready images whenever the selected session changes.
  useEffect(() => {
    if (!sessionId) {
      setImages([]);
      setImageId("");
      return;
    }

    let cancelled = false;

    getImagesBySession(sessionId)
      .then((loaded) => {
        if (cancelled) return;

        const usable = loaded.filter((image) => image.analysisReady !== false);
        setImages(usable);
        setImageId(usable[0]?.id ?? "");
      })
      .catch(() => {
        if (!cancelled) {
          setImages([]);
          setImageId("");
        }
      });

    return () => {
      cancelled = true;
    };
  }, [sessionId]);

  const selectedImage = images.find((image) => image.id === imageId) ?? null;
  const selectedOrtho = orthos.find((ortho) => ortho.id === orthoId) ?? null;

  const canRun = Boolean(
    pluginId &&
      plots.length > 0 &&
      !isRunning &&
      (sourceType === "session" ? selectedImage : selectedOrtho),
  );

  const waitForJob = async (
    jobId: string,
  ): Promise<"completed" | "failed"> => {
    for (;;) {
      if (abortRef.current) return "failed";

      try {
        const status = (await getJobStatus(jobId)) as JobStatusPayload;
        const state = (status?.state ?? "").toLowerCase();

        if (state === "completed") return "completed";
        if (state === "failed" || state === "aborted") return "failed";
      } catch {
        return "failed";
      }

      await new Promise((resolve) => setTimeout(resolve, POLL_INTERVAL_MS));
    }
  };

  const run = async () => {
    if (!canRun) return;

    abortRef.current = false;
    setIsRunning(true);
    setFinished(false);
    setDone(0);
    setErrors([]);
    setCurrentPlot(null);

    // Orthomosaic processing is handled by the native raster pipeline.
    if (sourceType === "ortho") {
      if (!selectedOrtho) {
        setIsRunning(false);
        return;
      }

      setProgressMessage("Memulai pemotongan georeferenced raster...");
      setProgressPercent(0);

      let unlisten: (() => void) | undefined;

      try {
        unlisten = await listen<PlotAnalysisProgressEvent>(
          "plot-analysis-progress",
          (event) => {
            setProgressMessage(event.payload.message);
            setProgressPercent(event.payload.percent);
            setDone(event.payload.plotIndex);
            setCurrentPlot(event.payload.plotName);
          },
        );

        const outcome = await analysePlotsFromOrtho({
          orthoId: selectedOrtho.id,
          plotIds: plots.map((plot) => plot.id),
          pluginId,
        });

        if (outcome.failedPlots.length > 0) {
          setErrors(outcome.failedPlots);
        }

        setDone(outcome.successfulPlots);
        setFinished(true);
        onFinished();
      } catch (error) {
        setErrors([message(error)]);
      } finally {
        unlisten?.();
        setIsRunning(false);
        setProgressMessage(null);
        setProgressPercent(null);
        setCurrentPlot(null);
      }

      return;
    }

    // Flight-session processing runs one plugin job per selected plot.
    if (!selectedImage) {
      setIsRunning(false);
      return;
    }

    const failures: string[] = [];

    for (const [index, plot] of plots.entries()) {
      if (abortRef.current) break;

      setCurrentPlot(plot.name);

      try {
        const handle = await startPluginJob({
          plugin_id: pluginId,
          session_id: selectedImage.sessionId,
          target_image_path: selectedImage.filePath,
          mime_type: mimeFor(selectedImage),
          aoi: {
            type: "geojson_polygon",
            geometry: plot.geometry,
          },
          parameters: {},
        });

        activeJobRef.current = handle.job_id;

        const state = await waitForJob(handle.job_id);

        activeJobRef.current = null;

        if (state !== "completed") {
          failures.push(`${plot.name}: plugin run failed`);
          continue;
        }

        const result = await getJobResult(handle.job_id);

        if (String(result.status).toLowerCase() === "failure") {
          failures.push(
            `${plot.name}: ${result.error ?? "plugin reported failure"}`,
          );
          continue;
        }

        await savePlotIndicators(
          plot.id,
          pluginId,
          handle.job_id,
          result.metrics,
        );
      } catch (error) {
        activeJobRef.current = null;
        failures.push(`${plot.name}: ${message(error)}`);
      } finally {
        setDone(index + 1);
      }
    }

    setCurrentPlot(null);
    setIsRunning(false);
    setFinished(true);
    setErrors(failures);
    onFinished();
  };

  const cancel = async () => {
    abortRef.current = true;

    const jobId = activeJobRef.current;
    if (!jobId) return;

    try {
      await abortPluginJob(jobId);
    } catch {
      // The polling loop will still stop when abortRef is set.
    }
  };

  const percent =
    progressPercent ??
    (plots.length === 0 ? 0 : Math.round((done / plots.length) * 100));

  return (
    <Dialog
      isOpen={isOpen}
      onClose={() => {
        if (isRunning) void cancel();
        onClose();
      }}
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
        {isLoading ? (
          <div style={{ textAlign: "center", padding: 24 }}>
            <Spinner size={28} />
          </div>
        ) : (
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
                  ...(plugins.length === 0
                    ? [{ label: "No enabled plugins", value: "" }]
                    : []),
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
                intent={
                  sourceType === "session" ? Intent.PRIMARY : Intent.NONE
                }
                onClick={() => setSourceType("session")}
                disabled={isRunning}
              />
              <Button
                text="Map Orthomosaic"
                intent={sourceType === "ortho" ? Intent.PRIMARY : Intent.NONE}
                onClick={() => setSourceType("ortho")}
                disabled={isRunning}
              />
            </ButtonGroup>

            {sourceType === "ortho" ? (
              <>
                <div className="bp6-label">
                  Orthomosaic ({orthos.length} available)
                </div>
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
                      ortho.dimensions
                        ? `(${ortho.dimensions[0]}×${ortho.dimensions[1]} px)`
                        : ""
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
                  Sub-citra plot akan dipotong (<i>clipped</i>) dan dibagi
                  menjadi ubin (<i>sliced</i>, maks. 1024×1024 px) secara
                  otomatis dari orthomosaic untuk mencegah lonjakan memori RAM
                  (OOM).
                </div>
              </>
            ) : (
              <>
                <div className="bp6-label">Project</div>
                <HTMLSelect
                  fill
                  value={projectId}
                  disabled={projects.length === 0 || isRunning}
                  onChange={(event) => setProjectId(event.currentTarget.value)}
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
                  onChange={(event) => setSessionId(event.currentTarget.value)}
                  options={sessions.map((session) => ({
                    label: `${session.name} (${
                      session.dateStart?.slice(0, 10) || "no date"
                    })`,
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
                    label:
                      image.filePath.split(/[/\\]/).pop() ?? image.filePath,
                    value: image.id,
                  }))}
                />
              </>
            )}

            {isRunning && (
              <div style={{ marginTop: 16 }}>
                <ProgressBar
                  value={percent / 100}
                  intent={Intent.PRIMARY}
                  animate={false}
                />
                <div
                  style={{
                    marginTop: 6,
                    fontSize: 12,
                    color: "var(--color-slate)",
                  }}
                >
                  {progressMessage ??
                    `${done}/${plots.length} plots · ${
                      currentPlot ?? "starting"
                    }`}
                </div>
              </div>
            )}

            {finished && errors.length === 0 && (
              <Callout
                intent={Intent.SUCCESS}
                style={{ marginTop: 16, borderRadius: 10 }}
              >
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
              The plugin runs once per selected plot with the plot boundary as
              its area of interest. Existing indicator values for the same
              plugin are replaced.
            </p>
          </>
        )}
      </div>

      <div className="bp6-dialog-footer">
        <div className="bp6-dialog-footer-actions">
          {isRunning ? (
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
          ) : (
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
                onClick={() => void run()}
                style={{
                  borderRadius: 999,
                  background: !canRun ? "#f1f5f9" : "#0f172a",
                  color: !canRun ? "#94a3b8" : "#ffffff",
                  fontWeight: 600,
                  padding: "6px 18px",
                  boxShadow: !canRun
                    ? "none"
                    : "0 2px 8px rgba(15,23,42,0.15)",
                }}
              />
            </>
          )}
        </div>
      </div>
    </Dialog>
  );
}
