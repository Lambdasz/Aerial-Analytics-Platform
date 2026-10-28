import { invoke } from "@tauri-apps/api/core";
import type {
  HealthStatusDto,
  JobHandleDto,
  JobStatusDto,
  PluginDetailsDto,
  PluginSummary,
  StandardJobResult,
  StartJobRequestDto,
  TargetDescriptorDto,
} from "../types/plugin";

export interface CommandError {
  code: string;
  message: string;
}

export function isCommandError(error: unknown): error is CommandError {
  return (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    "message" in error &&
    typeof (error as Record<string, unknown>).code === "string" &&
    typeof (error as Record<string, unknown>).message === "string"
  );
}

export function formatErrorMessage(error: unknown): string {
  if (isCommandError(error)) {
    return `[${error.code}] ${error.message}`;
  }
  if (error instanceof Error) {
    return error.message;
  }
  if (typeof error === "string") {
    return error;
  }
  return "An unexpected error occurred.";
}

// ---------------------------------------------------------------------------
// 1. Discovery & Registry Commands
// ---------------------------------------------------------------------------

export async function getInstalledPlugins(): Promise<PluginSummary[]> {
  return invoke<PluginSummary[]>("get_installed_plugins");
}

export async function getPluginDetails(pluginId: string): Promise<PluginDetailsDto> {
  return invoke<PluginDetailsDto>("get_plugin_details", { pluginId });
}

export async function queryCompatiblePlugins(
  target: TargetDescriptorDto,
): Promise<PluginSummary[]> {
  return invoke<PluginSummary[]>("query_compatible_plugins", { target });
}

export async function checkPluginHealth(pluginId: string): Promise<HealthStatusDto> {
  return invoke<HealthStatusDto>("check_plugin_health", { pluginId });
}

// ---------------------------------------------------------------------------
// 2. Lifecycle & State Commands
// ---------------------------------------------------------------------------

export async function installPlugin(archivePath: string): Promise<PluginSummary> {
  return invoke<PluginSummary>("install_plugin", { archivePath });
}

export async function removePlugin(pluginId: string): Promise<void> {
  return invoke<void>("remove_plugin", { pluginId });
}

export async function setPluginStatus(pluginId: string, enabled: boolean): Promise<void> {
  return invoke<void>("set_plugin_status", { pluginId, enabled });
}

/**
 * Updates plugin configuration overrides on the backend.
 *
 * @note Backend handler `plugin_manager::lifecycle::update_plugin_config` is registered in `lib.rs`.
 */
export async function updatePluginConfig(
  pluginId: string,
  customParams: Record<string, unknown>,
): Promise<void> {
  return invoke<void>("update_plugin_config", { pluginId, customParams });
}

// ---------------------------------------------------------------------------
// 3. Execution & Supervisor Commands
// ---------------------------------------------------------------------------

export async function startPluginJob(request: StartJobRequestDto): Promise<JobHandleDto> {
  return invoke<JobHandleDto>("start_plugin_job", { request });
}

export async function abortPluginJob(jobId: string): Promise<void> {
  return invoke<void>("abort_plugin_job", { jobId });
}

export async function getJobStatus(jobId: string): Promise<JobStatusDto> {
  return invoke<JobStatusDto>("get_job_status", { jobId });
}

export async function getJobResult(jobId: string): Promise<StandardJobResult> {
  return invoke<StandardJobResult>("get_job_result", { jobId });
}
