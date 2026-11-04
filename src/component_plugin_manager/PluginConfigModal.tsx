import React, { useEffect, useState, useCallback } from "react";
import { Dialog, Button, Callout, Intent, Spinner, NonIdealState } from "@blueprintjs/core";
import { DynamicParamForm } from "./DynamicParamForm";
import { validateParams } from "./validation";
import { getPluginDetails, updatePluginConfig, formatErrorMessage } from "./pluginCommands";
import type { ParameterDefinition } from "../types/plugin";

export interface PluginConfigModalProps {
  isOpen: boolean;
  pluginId: string | null;
  pluginName?: string;
  onClose: () => void;
  onSaved?: () => void;
}

export const PluginConfigModal: React.FC<PluginConfigModalProps> = ({
  isOpen,
  pluginId,
  pluginName,
  onClose,
  onSaved,
}) => {
  const [paramsDef, setParamsDef] = useState<ParameterDefinition[]>([]);
  const [paramValues, setParamValues] = useState<Record<string, unknown>>({});
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [isSaving, setIsSaving] = useState<boolean>(false);
  const [notice, setNotice] = useState<{ intent: Intent; message: string } | null>(null);

  useEffect(() => {
    if (!isOpen || !pluginId) return;

    let isMounted = true;
    const loadDetails = async () => {
      setIsLoading(true);
      setNotice(null);
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
          initial[d.name] = details.custom_parameters?.[d.name] ?? d.default;
        }
        setParamValues(initial);
      } catch (err) {
        if (!isMounted) return;
        setNotice({
          intent: Intent.DANGER,
          message: formatErrorMessage(err),
        });
      } finally {
        if (isMounted) setIsLoading(false);
      }
    };

    void loadDetails();

    return () => {
      isMounted = false;
    };
  }, [isOpen, pluginId]);

  const handleChange = useCallback((name: string, value: unknown) => {
    setParamValues((prev) => ({ ...prev, [name]: value }));
    setErrors((prev) => {
      if (!prev[name]) return prev;
      const next = { ...prev };
      delete next[name];
      return next;
    });
  }, []);

  const handleResetFactory = useCallback(() => {
    const factoryValues: Record<string, unknown> = {};
    for (const d of paramsDef) {
      factoryValues[d.name] = d.default;
    }
    setParamValues(factoryValues);
    setErrors({});
    setNotice({
      intent: Intent.WARNING,
      message:
        "Parameters reset to factory defaults. Click 'Save as Default' to persist these values.",
    });
  }, [paramsDef]);

  const handleSave = useCallback(async () => {
    if (!pluginId) return;

    const validation = validateParams(paramsDef, paramValues);
    if (!validation.isValid) {
      setErrors(validation.errors);
      return;
    }

    setErrors({});
    setIsSaving(true);
    setNotice(null);

    try {
      await updatePluginConfig(pluginId, validation.data);
      setNotice({
        intent: Intent.SUCCESS,
        message: "Default parameters saved successfully.",
      });
      onSaved?.();
      setTimeout(() => {
        onClose();
      }, 750);
    } catch (err) {
      setNotice({
        intent: Intent.DANGER,
        message: formatErrorMessage(err),
      });
    } finally {
      setIsSaving(false);
    }
  }, [paramsDef, paramValues, pluginId, onSaved, onClose]);

  return (
    <Dialog
      isOpen={isOpen}
      onClose={isSaving ? undefined : onClose}
      title={`Configure Default Parameters: ${pluginName ?? pluginId ?? "Plugin"}`}
      style={{ width: 560 }}
    >
      <div className="bp6-dialog-body" style={{ maxHeight: 520, overflowY: "auto" }}>
        {notice && (
          <Callout intent={notice.intent} style={{ marginBottom: 16 }}>
            {notice.message}
          </Callout>
        )}

        <Callout intent={Intent.PRIMARY} compact style={{ marginBottom: 16 }}>
          These parameter values will be used as the default configuration whenever this plugin is
          executed across sessions.
        </Callout>

        {isLoading ? (
          <div style={{ textAlign: "center", padding: "32px 0" }}>
            <Spinner size={28} />
          </div>
        ) : paramsDef.length === 0 ? (
          <NonIdealState
            icon="document"
            title="No Parameters Defined"
            description="This plugin does not declare any configurable parameters in parameters.json."
          />
        ) : (
          <DynamicParamForm
            parameters={paramsDef}
            values={paramValues}
            errors={errors}
            onChange={handleChange}
          />
        )}
      </div>

      <div className="bp6-dialog-footer">
        <div
          className="bp6-dialog-footer-actions"
          style={{ display: "flex", justifyContent: "space-between", width: "100%" }}
        >
          <div>
            {paramsDef.length > 0 && (
              <Button
                icon="reset"
                text="Reset to Factory Defaults"
                onClick={handleResetFactory}
                disabled={isLoading || isSaving}
              />
            )}
          </div>
          <div style={{ display: "flex", gap: 8 }}>
            <Button text="Cancel" onClick={onClose} disabled={isSaving} />
            <Button
              intent={Intent.PRIMARY}
              icon="floppy-disk"
              text="Save as Default"
              loading={isSaving}
              onClick={() => {
                void handleSave();
              }}
              disabled={isLoading || paramsDef.length === 0}
            />
          </div>
        </div>
      </div>
    </Dialog>
  );
};
