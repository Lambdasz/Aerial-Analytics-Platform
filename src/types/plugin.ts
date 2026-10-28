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

export interface ParametersSchema {
  schema_version?: number;
  parameters: ParameterDefinition[];
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
  source: string;
  health_status: "healthy" | "unhealthy" | "unknown";
}

export interface PluginMetadata {
  id: string;
  name: string;
  version: string;
  author?: string;
  license?: string;
  description: string;
  category?: string;
  tags?: string[];
  min_core_version?: string;
}

export interface PluginRuntime {
  type: string;
  entrypoint: string;
  min_python_version?: string;
  dependencies_file?: string;
  healthcheck_flag?: string;
}

export interface PluginManifest {
  $schema?: string;
  manifest_version: number;
  metadata: PluginMetadata;
  runtime: PluginRuntime;
  parameters: string;
  inputs: string;
  outputs: string;
  execution?: {
    timeout_seconds?: number;
    provides_output_dir_arg?: boolean;
    concurrency?: string;
  };
}

export interface PluginDetailsDto {
  manifest: PluginManifest;
  parameters: ParametersSchema | { parameters?: ParameterDefinition[] } | Record<string, unknown>;
  inputs: Record<string, unknown>;
  outputs: Record<string, unknown>;
  install_path: string;
  enabled: boolean;
}

export interface HealthStatusDto {
  status: "healthy" | "unhealthy";
  plugin_id: string;
  version: string;
  details?: Record<string, unknown> | null;
  error?: string | null;
}

export interface TargetDescriptorDto {
  granularity: string;
  mime_type: string;
  has_gps: boolean;
  has_altitude: boolean;
  has_aoi: boolean;
}

export interface StartJobRequestDto {
  plugin_id: string;
  session_id?: string | null;
  target_image_path: string;
  mime_type: string;
  aoi?: unknown;
  parameters: Record<string, unknown>;
}

export interface JobHandleDto {
  job_id: string;
  plugin_id: string;
  status: string;
}

export interface JobProgress {
  job_id: string;
  percent: number;
  stage: string;
}

export type JobStatusDto =
  | { state: "Queued" }
  | { state: "Running"; data: { progress_pct: number; stage: string } }
  | { state: "Completed"; data: { execution_time_ms: number } }
  | { state: "Failed"; data: { error: string } }
  | { state: "Aborted" };

export interface VerifiedArtifact {
  file_path: string;
  format: string;
  bounds?: [number, number, number, number] | number[];
}

export interface StandardJobResult {
  job_id: string;
  execution_time_ms: number;
  status: "success" | "warning" | "failure";
  metrics: Record<string, unknown>;
  artifacts: Record<string, VerifiedArtifact>;
  error?: string | null;
}

export type ValidationResult<T> =
  { isValid: true; data: T } | { isValid: false; errors: Record<string, string> };
