import React, { useState } from "react";
import {
  Dialog,
  Button,
  HTMLSelect,
  InputGroup,
  Switch,
  Tag,
  Spinner,
  NonIdealState,
} from "@blueprintjs/core";
import type { PluginDetailsDto, ParameterDefinition } from "../types/plugin";

export type ContractGroup = "metadata" | "parameter" | "input" | "output";

export interface PluginContractModalProps {
  isOpen: boolean;
  onClose: () => void;
  details: PluginDetailsDto | null;
  isLoading: boolean;
}

interface ContractPropertyRowProps {
  label: string;
  propKey?: string;
  badge?: string;
  control: React.ReactNode;
  description?: string;
}

const ContractPropertyRow: React.FC<ContractPropertyRowProps> = ({
  label,
  propKey,
  badge,
  control,
  description,
}) => {
  return (
    <div className="contract-prop-row">
      <div className="contract-prop-label-col">
        <div className="contract-prop-label">{label}</div>
        <div className="contract-prop-meta">
          {propKey && <code className="contract-prop-key">{propKey}</code>}
          {badge && (
            <Tag minimal round className="contract-prop-badge">
              {badge}
            </Tag>
          )}
        </div>
      </div>
      <div className="contract-prop-value-col">
        <div className="contract-prop-control">{control}</div>
        {description && <div className="contract-prop-description">{description}</div>}
      </div>
    </div>
  );
};

function formatUnknownValue(val: unknown): string {
  if (val === undefined || val === null) return "";
  if (typeof val === "string") return val;
  if (typeof val === "number" || typeof val === "boolean") return val.toString();
  if (Array.isArray(val)) return val.map(formatUnknownValue).join(", ");
  try {
    return JSON.stringify(val);
  } catch {
    return "";
  }
}

export const PluginContractModal: React.FC<PluginContractModalProps> = ({
  isOpen,
  onClose,
  details,
  isLoading,
}) => {
  const [activeGroup, setActiveGroup] = useState<ContractGroup>("metadata");
  const [showRawJson, setShowRawJson] = useState<boolean>(false);

  const groupOptions = [
    { value: "metadata", label: "Metadata (manifest.json)" },
    { value: "parameter", label: "Parameter (parameters.json)" },
    { value: "input", label: "Input (inputs.json)" },
    { value: "output", label: "Output (outputs.json)" },
  ];

  // Helper to extract parameters array safely
  const getParametersList = (): ParameterDefinition[] => {
    if (!details?.parameters) return [];
    const raw = details.parameters;
    if (Array.isArray(raw)) {
      return raw as ParameterDefinition[];
    }
    if (typeof raw === "object" && raw !== null && "parameters" in raw) {
      const p = (raw as { parameters?: ParameterDefinition[] }).parameters;
      if (Array.isArray(p)) return p;
    }
    return [];
  };

  const renderMetadataGroup = () => {
    if (!details) return null;
    const meta = details.manifest.metadata;
    const runtime = details.manifest.runtime;
    const execution = details.manifest.execution;

    return (
      <div className="contract-group-content">
        <ContractPropertyRow
          label="Plugin Name"
          propKey="name"
          badge="string"
          control={<InputGroup readOnly value={meta.name} fill />}
          description="Display name of the analytical plugin."
        />
        <ContractPropertyRow
          label="Plugin ID"
          propKey="id"
          badge="string"
          control={<InputGroup readOnly value={meta.id} fill />}
          description="Unique system identifier across the application."
        />
        <ContractPropertyRow
          label="Version"
          propKey="version"
          badge="semver"
          control={<InputGroup readOnly value={meta.version} fill />}
          description="Semantic version of the installed plugin package."
        />
        <ContractPropertyRow
          label="Category"
          propKey="category"
          badge="string"
          control={<InputGroup readOnly value={meta.category ?? "General"} fill />}
          description="Functional classification category in the plugin registry."
        />
        <ContractPropertyRow
          label="Author / Maintainer"
          propKey="author"
          badge="string"
          control={<InputGroup readOnly value={meta.author ?? "Unknown"} fill />}
          description="Developer, organization, or research team maintaining this package."
        />
        <ContractPropertyRow
          label="License"
          propKey="license"
          badge="string"
          control={<InputGroup readOnly value={meta.license ?? "Not specified"} fill />}
          description="Software license agreement governing use and distribution."
        />
        <ContractPropertyRow
          label="Description"
          propKey="description"
          badge="string"
          control={<InputGroup readOnly value={meta.description} fill />}
          description="High-level description of algorithms and analytical capabilities."
        />
        {meta.tags && meta.tags.length > 0 && (
          <ContractPropertyRow
            label="Tags"
            propKey="tags"
            badge="array"
            control={<InputGroup readOnly value={meta.tags.join(", ")} fill />}
            description="Searchable keyword indexing tags."
          />
        )}
        <ContractPropertyRow
          label="Min Core Version"
          propKey="min_core_version"
          badge="string"
          control={<InputGroup readOnly value={meta.min_core_version ?? "None"} fill />}
          description="Minimum version of the host Aerial Analytics Platform required."
        />
        <ContractPropertyRow
          label="Runtime Environment"
          propKey="runtime.type"
          badge={runtime.type}
          control={<InputGroup readOnly value={runtime.type} fill />}
          description="Execution runtime environment for running plugin tasks."
        />
        <ContractPropertyRow
          label="Entrypoint"
          propKey="runtime.entrypoint"
          badge="path"
          control={<InputGroup readOnly value={runtime.entrypoint} fill />}
          description="Relative path to executable binary or script entrypoint."
        />
        {runtime.healthcheck_flag && (
          <ContractPropertyRow
            label="Healthcheck Flag"
            propKey="runtime.healthcheck_flag"
            badge="cli"
            control={<InputGroup readOnly value={runtime.healthcheck_flag} fill />}
            description="CLI flag executed during pre-flight diagnostics to verify readiness."
          />
        )}
        {runtime.dependencies_file && (
          <ContractPropertyRow
            label="Dependencies Manifest"
            propKey="runtime.dependencies_file"
            badge="path"
            control={<InputGroup readOnly value={runtime.dependencies_file} fill />}
            description="Declared environment or package dependencies file."
          />
        )}
        <ContractPropertyRow
          label="Execution Timeout"
          propKey="execution.timeout_seconds"
          badge="seconds"
          control={
            <InputGroup readOnly value={`${execution?.timeout_seconds ?? 60} seconds`} fill />
          }
          description="Maximum duration the supervisor permits before automatically aborting."
        />
        <ContractPropertyRow
          label="Concurrency Policy"
          propKey="execution.concurrency"
          badge="policy"
          control={<InputGroup readOnly value={execution?.concurrency ?? "sequential"} fill />}
          description="Execution concurrency policy (sequential or parallel execution)."
        />
        <ContractPropertyRow
          label="Installation Directory"
          propKey="install_path"
          badge="path"
          control={<InputGroup readOnly value={details.install_path} fill />}
          description="Absolute path to the plugin folder on the local filesystem."
        />
        <ContractPropertyRow
          label="Status"
          propKey="enabled"
          badge="boolean"
          control={
            <Switch
              disabled
              checked={details.enabled}
              label={details.enabled ? "Active / Enabled" : "Inactive / Disabled"}
              style={{ marginBottom: 0 }}
            />
          }
          description="Whether this plugin is active and available for execution in sessions."
        />
      </div>
    );
  };

  const renderParameterGroup = () => {
    const params = getParametersList();

    if (params.length === 0) {
      return (
        <NonIdealState
          icon="document"
          title="No Parameters Defined"
          description="This plugin does not declare any configurable parameters in parameters.json."
        />
      );
    }

    return (
      <div className="contract-group-content">
        {params.map((param) => {
          let controlNode: React.ReactNode;
          const defaultValue = param.default;
          const defaultStr = formatUnknownValue(defaultValue);

          if (param.type === "select") {
            const options = param.options ?? (defaultStr ? [defaultStr] : []);
            controlNode = (
              <HTMLSelect
                disabled
                fill
                value={defaultStr}
                options={options.map((opt) => ({ label: opt, value: opt }))}
              />
            );
          } else if (param.type === "number") {
            const hasRange = param.min !== undefined || param.max !== undefined;
            const rangeText = hasRange
              ? `min: ${param.min ?? "—"}, max: ${param.max ?? "—"}, step: ${param.step ?? 1}`
              : undefined;
            controlNode = (
              <InputGroup
                readOnly
                fill
                value={defaultStr}
                rightElement={
                  rangeText ? (
                    <Tag minimal round style={{ marginRight: 6 }}>
                      {rangeText}
                    </Tag>
                  ) : undefined
                }
              />
            );
          } else if (param.type === "boolean") {
            controlNode = (
              <Switch
                disabled
                checked={Boolean(defaultValue)}
                label={defaultValue ? "Default: True" : "Default: False"}
                style={{ marginBottom: 0 }}
              />
            );
          } else {
            controlNode = (
              <InputGroup readOnly fill value={defaultStr} placeholder="(empty string)" />
            );
          }

          return (
            <ContractPropertyRow
              key={param.name}
              label={param.label || param.name}
              propKey={param.name}
              badge={param.type}
              control={controlNode}
              description={param.description}
            />
          );
        })}
      </div>
    );
  };

  const renderInputGroup = () => {
    if (!details?.inputs) return null;
    const inputs = details.inputs;

    const granularity =
      typeof inputs.granularity === "string" ? inputs.granularity : "single_image";
    const mimeTypes = Array.isArray(inputs.supported_mime_types)
      ? inputs.supported_mime_types.map(formatUnknownValue).join(", ")
      : typeof inputs.supported_mime_types === "string"
        ? inputs.supported_mime_types
        : "image/jpeg, image/png";
    const reqMeta =
      Array.isArray(inputs.requires_metadata) && inputs.requires_metadata.length > 0
        ? inputs.requires_metadata.map(formatUnknownValue).join(", ")
        : "None";
    const optMeta =
      Array.isArray(inputs.optional_metadata) && inputs.optional_metadata.length > 0
        ? inputs.optional_metadata.map(formatUnknownValue).join(", ")
        : "None";
    const supportsAoi = Boolean(inputs.supports_aoi);
    const aoiFormat = typeof inputs.aoi_format === "string" ? inputs.aoi_format : "None";

    return (
      <div className="contract-group-content">
        <ContractPropertyRow
          label="Execution Granularity"
          propKey="granularity"
          badge="string"
          control={<InputGroup readOnly value={granularity} fill />}
          description="Scope of analysis executed per invocation (single_image, session, batch)."
        />
        <ContractPropertyRow
          label="Supported MIME Types"
          propKey="supported_mime_types"
          badge="array"
          control={<InputGroup readOnly value={mimeTypes} fill />}
          description="Raster image formats accepted by the plugin engine."
        />
        <ContractPropertyRow
          label="Required Metadata"
          propKey="requires_metadata"
          badge="array"
          control={<InputGroup readOnly value={reqMeta} fill />}
          description="Mandatory metadata tags (e.g. resolution, gps) required for execution."
        />
        <ContractPropertyRow
          label="Optional Metadata"
          propKey="optional_metadata"
          badge="array"
          control={<InputGroup readOnly value={optMeta} fill />}
          description="Additional metadata consumed if available in the source image EXIF."
        />
        <ContractPropertyRow
          label="Area of Interest (AOI)"
          propKey="supports_aoi"
          badge="boolean"
          control={
            <Switch
              disabled
              checked={supportsAoi}
              label={supportsAoi ? "Supported (true)" : "Not Supported (false)"}
              style={{ marginBottom: 0 }}
            />
          }
          description="Whether analysis can be clipped to a specific polygon or boundary."
        />
        <ContractPropertyRow
          label="AOI Geometry Format"
          propKey="aoi_format"
          badge="format"
          control={<InputGroup readOnly value={aoiFormat} fill />}
          description="Spatial geometry format expected when AOI clipping is applied."
        />
      </div>
    );
  };

  const renderOutputGroup = () => {
    if (!details?.outputs) return null;
    const outputs = details.outputs as {
      metrics?: Array<{
        key: string;
        label: string;
        type: string;
        unit?: string;
        description?: string;
      }>;
      artifacts?: Array<{
        key: string;
        label: string;
        type: string;
        format: string;
        coordinate_system?: string;
        optional?: boolean;
        layer_hint?: {
          layer_type?: string;
          default_opacity?: number;
          color_palette?: string;
          stroke_color?: string;
        };
      }>;
    };

    const metrics = Array.isArray(outputs.metrics) ? outputs.metrics : [];
    const artifacts = Array.isArray(outputs.artifacts) ? outputs.artifacts : [];

    if (metrics.length === 0 && artifacts.length === 0) {
      return (
        <NonIdealState
          icon="document"
          title="No Outputs Defined"
          description="This plugin does not declare any metrics or artifacts in outputs.json."
        />
      );
    }

    return (
      <div className="contract-group-content">
        {metrics.length > 0 && (
          <div className="contract-subgroup-section">
            <h5 className="contract-subgroup-title">Metrics ({metrics.length})</h5>
            {metrics.map((m) => {
              const displayVal = `Data type: ${m.type}${m.unit ? ` | Unit: ${m.unit}` : ""}`;
              return (
                <ContractPropertyRow
                  key={m.key}
                  label={m.label || m.key}
                  propKey={m.key}
                  badge={m.type}
                  control={<InputGroup readOnly value={displayVal} fill />}
                  description={m.description || "Calculated analytical metric."}
                />
              );
            })}
          </div>
        )}

        {artifacts.length > 0 && (
          <div className="contract-subgroup-section" style={{ marginTop: 20 }}>
            <h5 className="contract-subgroup-title">Artifacts ({artifacts.length})</h5>
            {artifacts.map((a) => {
              const displayVal = `Type: ${a.type} | Format: ${a.format}${
                a.coordinate_system ? ` | CRS: ${a.coordinate_system}` : ""
              }${a.optional ? " | Optional" : " | Required"}`;

              const hintParts: string[] = [];
              if (a.layer_hint?.layer_type) hintParts.push(`Layer: ${a.layer_hint.layer_type}`);
              if (a.layer_hint?.color_palette)
                hintParts.push(`Palette: ${a.layer_hint.color_palette}`);
              if (a.layer_hint?.default_opacity !== undefined) {
                hintParts.push(`Default Opacity: ${a.layer_hint.default_opacity}`);
              }
              const hintDesc = hintParts.length > 0 ? hintParts.join(", ") : undefined;

              return (
                <ContractPropertyRow
                  key={a.key}
                  label={a.label || a.key}
                  propKey={a.key}
                  badge={a.type}
                  control={<InputGroup readOnly value={displayVal} fill />}
                  description={hintDesc || "Visual or tabular analytical artifact."}
                />
              );
            })}
          </div>
        )}
      </div>
    );
  };

  const getRawJsonData = () => {
    if (!details) return {};
    switch (activeGroup) {
      case "metadata":
        return details.manifest;
      case "parameter":
        return details.parameters;
      case "input":
        return details.inputs;
      case "output":
        return details.outputs;
    }
  };

  return (
    <Dialog
      isOpen={isOpen}
      onClose={onClose}
      title={`Contract Specifications: ${details?.manifest.metadata.name ?? "Plugin"}`}
      className="contract-inspector-dialog"
      style={{ width: 780, maxWidth: "95vw" }}
    >
      <div className="bp6-dialog-body" style={{ maxHeight: 520, overflowY: "auto", padding: 18 }}>
        {isLoading || !details ? (
          <div style={{ textAlign: "center", padding: "48px 0" }}>
            <Spinner size={32} />
          </div>
        ) : (
          <div>
            {/* Group Selector Dropdown Toolbar */}
            <div className="contract-toolbar">
              <div className="contract-toolbar-left">
                <span className="contract-toolbar-label">Contract Group:</span>
                <HTMLSelect
                  value={activeGroup}
                  onChange={(e) => setActiveGroup(e.currentTarget.value as ContractGroup)}
                  options={groupOptions}
                  className="contract-group-select"
                />
              </div>
              <div className="contract-toolbar-right">
                <Button
                  minimal
                  small
                  icon={showRawJson ? "form" : "code"}
                  text={showRawJson ? "Formatted View" : "View Raw JSON"}
                  onClick={() => setShowRawJson((prev) => !prev)}
                />
              </div>
            </div>

            {/* Main Content Area */}
            {showRawJson ? (
              <pre className="contract-raw-json">{JSON.stringify(getRawJsonData(), null, 2)}</pre>
            ) : (
              <div className="contract-form-view">
                {activeGroup === "metadata" && renderMetadataGroup()}
                {activeGroup === "parameter" && renderParameterGroup()}
                {activeGroup === "input" && renderInputGroup()}
                {activeGroup === "output" && renderOutputGroup()}
              </div>
            )}
          </div>
        )}
      </div>

      <div className="bp6-dialog-footer">
        <div className="bp6-dialog-footer-actions">
          <Button text="Close" intent="none" onClick={onClose} />
        </div>
      </div>
    </Dialog>
  );
};
