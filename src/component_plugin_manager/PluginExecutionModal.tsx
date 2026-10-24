import React, { useEffect, useState, useCallback, useRef } from "react";
import { Dialog, Button, ProgressBar, Callout, Intent, Spinner } from "@blueprintjs/core";
import { listen } from "@tauri-apps/api/event";
import { DynamicParamForm } from "./DynamicParamForm";
import { validateParams } from "./validation";
import {
  getPluginDetails,
  startPluginJob,
  abortPluginJob,
  getJobResult,
  formatErrorMessage,
} from "./pluginCommands";
import type { ParameterDefinition, StandardJobResult, JobProgress } from "../types/plugin";

export interface PluginExecutionTarget {
  imagePath: string;
  mimeType: string;
  sessionId?: string;
  aoi?: unknown;
}

export interface PluginExecutionModalProps {
  isOpen: boolean;
  pluginId: string;
  target: PluginExecutionTarget;
  onClose: () => void;
  onSuccess: (result: StandardJobResult) => void;
}

export const PluginExecutionModal: React.FC<PluginExecutionModalProps> = ({
  isOpen,
  pluginId,
  target,
  onClose,
  onSuccess,
}) => {
  const [paramsDef, setParamsDef] = useState<ParameterDefinition[]>([]);
  const [paramValues, setParamValues] = useState<Record<string, unknown>>({});
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [isLoadingSchema, setIsLoadingSchema] = useState<boolean>(false);
  const [activeJobId, setActiveJobId] = useState<string | null>(null);
  const [isAborting, setIsAborting] = useState<boolean>(false);
  const [progress, setProgress] = useState<{ percent: number; stage: string } | null>(null);
  const [executionError, setExecutionError] = useState<string | null>(null);

  const activeJobIdRef = useRef<string | null>(null);

  useEffect(() => {
    activeJobIdRef.current = activeJobId;
  }, [activeJobId]);

  // 1. Fetch plugin parameter definitions when opened
  useEffect(() => {
    if (!isOpen || !pluginId) return;

    let isMounted = true;
    const fetchSchema = async () => {
      setIsLoadingSchema(true);
      setExecutionError(null);
      setActiveJobId(null);
      setProgress(null);
      setErrors({});

      try {
        const details = await getPluginDetails(pluginId);
        if (!isMounted) return;
        const rawParams = details.parameters;
        let defs: ParameterDefinition[] = [];

        if (rawParams && typeof rawParams === "object") {
          if (Array.isArray((rawParams as { parameters?: ParameterDefinition[] }).parameters)) {
            defs = (rawParams as { parameters: ParameterDefinition[] }).parameters;
          } else if (Array.isArray(rawParams)) {
            defs = rawParams as ParameterDefinition[];
          }
        }

        setParamsDef(defs);
        const initial: Record<string, unknown> = {};
        for (const d of defs) {
          initial[d.name] = d.default;
        }
        setParamValues(initial);
      } catch (err) {
        if (!isMounted) return;
        setExecutionError(formatErrorMessage(err));
      } finally {
        if (isMounted) setIsLoadingSchema(false);
      }
    };

    void fetchSchema();

    return () => {
      isMounted = false;
    };
  }, [isOpen, pluginId]);

  // 2. Listen for live progress events from Tauri supervisor
  useEffect(() => {
    if (!activeJobId) return;

    let unlistenFn: (() => void) | undefined;

    const setupListener = async () => {
      const unlisten = await listen<JobProgress>("plugin://progress", (event) => {
        if (event.payload.job_id === activeJobIdRef.current) {
          setProgress({ percent: event.payload.percent, stage: event.payload.stage });
        }
      });
      unlistenFn = unlisten;
    };

    void setupListener();

    return () => {
      if (unlistenFn) {
        unlistenFn();
      }
    };
  }, [activeJobId]);

  // 3. Handle Job Submission
  const handleLaunch = useCallback(async () => {
    const validation = validateParams(paramsDef, paramValues);
    if (!validation.isValid) {
      setErrors(validation.errors);
      return;
    }
    setErrors({});
    setExecutionError(null);

    try {
      const handle = await startPluginJob({
        plugin_id: pluginId,
        session_id: target.sessionId,
        target_image_path: target.imagePath,
        mime_type: target.mimeType,
        aoi: target.aoi,
        parameters: validation.data,
      });

      setActiveJobId(handle.job_id);

      // Await final result
      const result = await getJobResult(handle.job_id);
      onSuccess(result);
      onClose();
    } catch (err) {
      setExecutionError(formatErrorMessage(err));
      setActiveJobId(null);
    }
  }, [paramsDef, paramValues, pluginId, target, onSuccess, onClose]);

  // 4. Handle Abort Job
  const handleAbort = useCallback(async () => {
    if (!activeJobId) return;
    setIsAborting(true);
    try {
      await abortPluginJob(activeJobId);
      setActiveJobId(null);
      setProgress(null);
      setExecutionError("Analysis was cancelled by user.");
    } catch (err) {
      setExecutionError(formatErrorMessage(err));
    } finally {
      setIsAborting(false);
    }
  }, [activeJobId]);

  const handleClose = () => {
    if (activeJobId) {
      void handleAbort();
    }
    onClose();
  };

  return (
    <Dialog
      isOpen={isOpen}
      onClose={handleClose}
      title={`Run Analysis: ${pluginId}`}
      canEscapeKeyClose={!activeJobId}
      canOutsideClickClose={!activeJobId}
      style={{ width: 560 }}
    >
      <div className="bp6-dialog-body" style={{ minHeight: 180 }}>
        {executionError && (
          <Callout intent={Intent.DANGER} title="Execution Notice" style={{ marginBottom: 16 }}>
            {executionError}
          </Callout>
        )}

        {isLoadingSchema ? (
          <div style={{ textAlign: "center", padding: "32px 0" }}>
            <Spinner size={32} />
            <p className="bp6-text-muted" style={{ marginTop: 8 }}>
              Loading plugin parameters...
            </p>
          </div>
        ) : !activeJobId ? (
          <DynamicParamForm
            parameters={paramsDef}
            values={paramValues}
            errors={errors}
            onChange={(k, v) => setParamValues((prev) => ({ ...prev, [k]: v }))}
          />
        ) : (
          <div style={{ padding: "16px 0" }}>
            <p>
              <strong>Stage:</strong> {progress?.stage ?? "Initializing job..."}
            </p>
            <ProgressBar
              value={progress ? progress.percent / 100 : undefined}
              intent={Intent.PRIMARY}
            />
            {progress && (
              <p
                className="bp6-text-muted"
                style={{ textAlign: "right", marginTop: 4, fontSize: "0.85em" }}
              >
                {progress.percent}% complete
              </p>
            )}
          </div>
        )}
      </div>

      <div className="bp6-dialog-footer">
        <div className="bp6-dialog-footer-actions">
          {activeJobId ? (
            <Button
              intent={Intent.DANGER}
              text="Abort Job"
              onClick={() => {
                void handleAbort();
              }}
              loading={isAborting}
            />
          ) : (
            <>
              <Button onClick={onClose} text="Cancel" />
              <Button
                intent={Intent.PRIMARY}
                text="Execute Analysis"
                onClick={() => {
                  void handleLaunch();
                }}
                disabled={isLoadingSchema}
              />
            </>
          )}
        </div>
      </div>
    </Dialog>
  );
};
