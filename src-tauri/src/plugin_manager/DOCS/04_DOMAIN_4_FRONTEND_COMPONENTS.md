# Domain 4: Dynamic UI & Form Validation (TypeScript / Blueprint.js Frontend)

> **Module 2: Plugin Architecture & Extension Manager**  
> **Parent Guide**: [README.md](README.md)

---

## 1. Domain Purpose & Overview

Domain 4 bridges the Rust backend with the user interface. It provides:

1. The **Plugin Manager UI** for browsing, enabling, configuring, and installing plugins.
2. The dynamic **Schema-Driven Parameter Form Generator** mapping `parameters.json` to Blueprint.js 6 controls.
3. The self-contained **`<PluginExecutionModal />`**, allowing Modules 1 & 3 to launch analyses and stream progress without writing duplicate dialog or validation code.

---

## 2. Component & Helper Matrix

| Component / Function       | Scope                       | Props / Input Parameters                                                                                        | Return / Output Type                        | Purity / Category                | Description                                                                                                                                 |
| :------------------------- | :-------------------------- | :-------------------------------------------------------------------------------------------------------------- | :------------------------------------------ | :------------------------------- | :------------------------------------------------------------------------------------------------------------------------------------------ |
| **`PluginManagerView`**    | `export (React Component)`  | `None` (Fetches via Tauri IPC)                                                                                  | `React.ReactElement`                        | **Stateful Container Component** | Full Extension Manager page: search bar, category filters, "Install Plugin" button, and responsive grid of `PluginCard`s.                   |
| **`PluginCard`**           | `export (React Component)`  | `plugin: PluginSummary, onToggle, onConfigure, onHealthcheck, onUninstall`                                      | `React.ReactElement`                        | **Pure Presentation Component**  | Blueprint.js `<Card>` displaying metadata, health badge, toggle switch, and action buttons.                                                 |
| **`DynamicParamForm`**     | `export (React Component)`  | `parameters: ParameterDefinition[], values: Record<string, unknown>, errors?: Record<string, string>, onChange` | `React.ReactElement`                        | **Pure Presentation Component**  | Iterates over `parameters.json` definitions and renders corresponding Blueprint.js form controls (`number`, `select`, `boolean`, `string`). |
| **`renderDynamicInput`**   | `export (Helper Component)` | `param: ParameterDefinition, value: unknown, error?: string, onChange`                                          | `React.ReactElement`                        | **Pure Sub-Component**           | Pure pattern-matcher mapping a single `ParameterDefinition` to `<NumericInput>`, `<HTMLSelect>`, `<Switch>`, or `<InputGroup>`.             |
| **`validateParams`**       | `export (Helper Function)`  | `defs: ParameterDefinition[], values: Record<string, unknown>`                                                  | `ValidationResult<Record<string, unknown>>` | **Pure Function**                | Validates user input values against schema rules (`min`, `max`, `step`, allowed `options`). Returns sanitized values or error messages.     |
| **`PluginExecutionModal`** | `export (React Component)`  | `isOpen: boolean, pluginId: string, target: TargetDescriptor, onClose, onSuccess`                               | `React.ReactElement`                        | **Stateful Modal Controller**    | Standardized launch dialog managed by Module 2. Handles parameter tweaks, validation, job submission, progress bar, and error display.      |
| **`PluginInstallDialog`**  | `export (React Component)`  | `isOpen: boolean, onClose: () => void, onInstalled`                                                             | `React.ReactElement`                        | **Stateful Modal Component**     | File picker dialog for `.zip` upload with installation progress and validation feedback callouts.                                           |

---

## 3. TypeScript Domain Models (`src/types/plugin.ts`)

```typescript
export type ParameterType = "number" | "boolean" | "select" | "string";

export interface ParameterDefinition {
  name: string;
  label: string;
  type: ParameterType;
  default: unknown;
  description?: string;
  min?: number;
  max?: number;
  step?: number;
  options?: string[];
}

export interface PluginSummary {
  id: string;
  name: string;
  version: string;
  author: string;
  category: string;
  description: string;
  tags: string[];
  runtime_type: "python" | "binary" | "wasm";
  enabled: boolean;
}

export interface VerifiedArtifact {
  file_path: string;
  format: string;
  bounds?: [number, number, number, number];
}

export interface StandardJobResult {
  job_id: string;
  execution_time_ms: number;
  status: "success" | "warning" | "failure";
  metrics: Record<string, unknown>;
  artifacts: Record<string, VerifiedArtifact>;
  error?: string;
}

export type ValidationResult<T> =
  { isValid: true; data: T } | { isValid: false; errors: Record<string, string> };
```

---

## 4. Reusable Execution Modal (`PluginExecutionModal.tsx`)

> [!TIP]
> Module 3 (Map Explorer) and Module 1 (Image Manager) do not need to build parameter dialogs or job trackers. They simply render `<PluginExecutionModal />` and handle the `onSuccess` callback.

```tsx
import React, { useEffect, useState } from "react";
import { Dialog, Button, ProgressBar, Callout, Intent } from "@blueprintjs/core";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { DynamicParamForm } from "./DynamicParamForm";
import { validateParams } from "./validation";
import { ParameterDefinition, StandardJobResult } from "../../types/plugin";

interface PluginExecutionModalProps {
  isOpen: boolean;
  pluginId: string;
  target: {
    imagePath: string;
    mimeType: string;
    aoi?: unknown;
  };
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
  const [activeJobId, setActiveJobId] = useState<string | null>(null);
  const [progress, setProgress] = useState<{ percent: number; stage: string } | null>(null);
  const [executionError, setExecutionError] = useState<string | null>(null);

  // 1. Fetch plugin parameters on open
  useEffect(() => {
    if (!isOpen) return;
    invoke<{ parameters: { parameters: ParameterDefinition[] } }>("get_plugin_details", {
      pluginId,
    }).then((details) => {
      const defs = details.parameters.parameters ?? [];
      setParamsDef(defs);
      const initial: Record<string, unknown> = {};
      defs.forEach((d) => (initial[d.name] = d.default));
      setParamValues(initial);
    });
  }, [isOpen, pluginId]);

  // 2. Listen for live progress events from Tauri supervisor
  useEffect(() => {
    if (!activeJobId) return;
    const unlistenPromise = listen<{ job_id: string; percent: number; stage: string }>(
      "plugin://progress",
      (event) => {
        if (event.payload.job_id === activeJobId) {
          setProgress({ percent: event.payload.percent, stage: event.payload.stage });
        }
      },
    );
    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, [activeJobId]);

  // 3. Handle Job Submission
  const handleLaunch = async () => {
    const validation = validateParams(paramsDef, paramValues);
    if (!validation.isValid) {
      setErrors(validation.errors);
      return;
    }
    setErrors({});
    setExecutionError(null);

    try {
      const handle = await invoke<{ job_id: string }>("start_plugin_job", {
        request: {
          plugin_id: pluginId,
          target_image_path: target.imagePath,
          mime_type: target.mimeType,
          aoi: target.aoi,
          parameters: validation.data,
        },
      });
      setActiveJobId(handle.job_id);

      // Await final result
      const result = await invoke<StandardJobResult>("get_job_result", {
        jobId: handle.job_id,
      });
      onSuccess(result);
      onClose();
    } catch (err) {
      setExecutionError(String(err));
      setActiveJobId(null);
    }
  };

  return (
    <Dialog isOpen={isOpen} onClose={onClose} title={`Run Analysis: ${pluginId}`}>
      <div className="bp6-dialog-body">
        {executionError && (
          <Callout intent={Intent.DANGER} title="Execution Failed" style={{ marginBottom: 12 }}>
            {executionError}
          </Callout>
        )}

        {!activeJobId ? (
          <DynamicParamForm
            parameters={paramsDef}
            values={paramValues}
            errors={errors}
            onChange={(k, v) => setParamValues((prev) => ({ ...prev, [k]: v }))}
          />
        ) : (
          <div style={{ padding: "16px 0" }}>
            <p>
              <strong>Stage:</strong> {progress?.stage ?? "Initializing..."}
            </p>
            <ProgressBar
              value={progress ? progress.percent / 100 : undefined}
              intent={Intent.PRIMARY}
            />
          </div>
        )}
      </div>
      <div className="bp6-dialog-footer">
        <div className="bp6-dialog-footer-actions">
          <Button onClick={onClose} text="Cancel" />
          {!activeJobId && <Button intent={Intent.PRIMARY} text="Execute" onClick={handleLaunch} />}
        </div>
      </div>
    </Dialog>
  );
};
```

---

## 5. Pure Validation & Form Controls

### 5.1 Pure Client-Side Form Validation (`validateParams`)

```typescript
export function validateParams(
  defs: ParameterDefinition[],
  values: Record<string, unknown>,
): ValidationResult<Record<string, unknown>> {
  const errors: Record<string, string> = {};
  const sanitized: Record<string, unknown> = {};

  for (const def of defs) {
    const rawVal = values[def.name] ?? def.default;

    switch (def.type) {
      case "number": {
        const num = Number(rawVal);
        if (isNaN(num)) {
          errors[def.name] = `${def.label} must be a valid number.`;
        } else if (def.min !== undefined && num < def.min) {
          errors[def.name] = `${def.label} cannot be less than ${def.min}.`;
        } else if (def.max !== undefined && num > def.max) {
          errors[def.name] = `${def.label} cannot exceed ${def.max}.`;
        } else {
          sanitized[def.name] = num;
        }
        break;
      }
      case "select": {
        const strVal = String(rawVal);
        if (def.options && !def.options.includes(strVal)) {
          errors[def.name] = `Invalid option selected for ${def.label}.`;
        } else {
          sanitized[def.name] = strVal;
        }
        break;
      }
      case "boolean": {
        sanitized[def.name] = Boolean(rawVal);
        break;
      }
      case "string": {
        sanitized[def.name] = String(rawVal ?? "");
        break;
      }
    }
  }

  if (Object.keys(errors).length > 0) {
    return { isValid: false, errors };
  }
  return { isValid: true, data: sanitized };
}
```

### 5.2 Dynamic Form Control Renderer (`renderDynamicInput`)

```tsx
import React from "react";
import { FormGroup, NumericInput, HTMLSelect, Switch, InputGroup, Intent } from "@blueprintjs/core";
import { ParameterDefinition } from "../../types/plugin";

export const RenderDynamicInput: React.FC<{
  param: ParameterDefinition;
  value: unknown;
  error?: string;
  onChange: (value: unknown) => void;
}> = ({ param, value, error, onChange }) => {
  const currentValue = value ?? param.default;
  const intent = error ? Intent.DANGER : Intent.NONE;

  return (
    <FormGroup
      label={param.label}
      helperText={error ?? param.description}
      intent={intent}
      inline={param.type === "boolean"}
    >
      {param.type === "number" && (
        <NumericInput
          value={currentValue as number}
          min={param.min}
          max={param.max}
          stepSize={param.step ?? 0.05}
          intent={intent}
          onValueChange={(val) => onChange(isNaN(val) ? param.default : val)}
          fill
        />
      )}

      {param.type === "select" && (
        <HTMLSelect
          value={currentValue as string}
          options={param.options ?? []}
          onChange={(e) => onChange(e.currentTarget.value)}
          fill
        />
      )}

      {param.type === "boolean" && (
        <Switch
          checked={Boolean(currentValue)}
          onChange={(e) => onChange(e.currentTarget.checked)}
        />
      )}

      {param.type === "string" && (
        <InputGroup
          value={currentValue as string}
          intent={intent}
          placeholder={String(param.default)}
          onChange={(e) => onChange(e.currentTarget.value)}
          fill
        />
      )}
    </FormGroup>
  );
};
```
