import { invoke } from "@tauri-apps/api/core";

export type ImageMetadata = {
  path: string;
  has_gps: boolean;
  width: number;
  height: number;
  format: string;
};

export type PluginPayload = {
  plugin_id: string;
  image: ImageMetadata;
  aoi: { geojson: unknown } | null;
  parameters: Record<string, unknown>;
  output_dir: string;
};

export type PluginInputsRequirement = {
  require_gps?: boolean;
  allowed_formats?: string[];
};

export type ExecutionResult = {
  status: string;
  metrics: Record<string, unknown>;
  output_files: string[];
  error_message: string | null;
};

export function validateAndCreatePayload(
  payload: PluginPayload,
  inputsReq: PluginInputsRequirement,
  outputDir: string,
) {
  return invoke<string>("validate_and_create_payload", {
    payload,
    inputsReq,
    outputDir,
  });
}

export function processExecutionResult(
  outputDir: string,
  historyFilePath: string,
  pluginId: string,
) {
  return invoke<ExecutionResult>("process_execution_result", {
    outputDir,
    historyFilePath,
    pluginId,
  });
}
