import React from "react";
import { FormGroup, NumericInput, HTMLSelect, Switch, InputGroup, Intent } from "@blueprintjs/core";
import type { ParameterDefinition } from "../types/plugin";

export interface RenderDynamicInputProps {
  param: ParameterDefinition;
  value: unknown;
  error?: string;
  onChange: (value: unknown) => void;
}

export const RenderDynamicInput: React.FC<RenderDynamicInputProps> = ({
  param,
  value,
  error,
  onChange,
}) => {
  const currentValue = value !== undefined ? value : param.default;
  const stringVal =
    typeof currentValue === "string"
      ? currentValue
      : typeof currentValue === "number" || typeof currentValue === "boolean"
        ? currentValue.toString()
        : "";
  const placeholderVal =
    typeof param.default === "string"
      ? param.default
      : typeof param.default === "number" || typeof param.default === "boolean"
        ? param.default.toString()
        : "";
  const intent = error ? Intent.DANGER : Intent.NONE;

  return (
    <FormGroup
      label={param.label}
      helperText={error ?? param.description}
      intent={intent}
      inline={param.type === "boolean"}
      style={{ marginBottom: 16 }}
    >
      {param.type === "number" && (
        <NumericInput
          value={typeof currentValue === "number" ? currentValue : Number(currentValue) || 0}
          min={param.min}
          max={param.max}
          stepSize={param.step ?? 0.05}
          intent={intent}
          onValueChange={(val) => onChange(Number.isNaN(val) ? param.default : val)}
          fill
        />
      )}

      {param.type === "select" && (
        <HTMLSelect
          value={stringVal}
          options={param.options ?? []}
          onChange={(e) => onChange(e.currentTarget.value)}
          fill
        />
      )}

      {param.type === "boolean" && (
        <Switch
          checked={Boolean(currentValue)}
          onChange={(e) => onChange(e.currentTarget.checked)}
          style={{ marginBottom: 0 }}
        />
      )}

      {param.type === "string" && (
        <InputGroup
          value={stringVal}
          intent={intent}
          placeholder={placeholderVal}
          onChange={(e) => onChange(e.currentTarget.value)}
          fill
        />
      )}
    </FormGroup>
  );
};

export interface DynamicParamFormProps {
  parameters: ParameterDefinition[];
  values: Record<string, unknown>;
  errors?: Record<string, string>;
  onChange: (name: string, value: unknown) => void;
}

export const DynamicParamForm: React.FC<DynamicParamFormProps> = ({
  parameters,
  values,
  errors = {},
  onChange,
}) => {
  if (parameters.length === 0) {
    return <p className="bp6-text-muted">No configurable parameters required for this plugin.</p>;
  }

  return (
    <div className="dynamic-param-form">
      {parameters.map((param) => (
        <RenderDynamicInput
          key={param.name}
          param={param}
          value={values[param.name]}
          error={errors[param.name]}
          onChange={(val) => onChange(param.name, val)}
        />
      ))}
    </div>
  );
};
