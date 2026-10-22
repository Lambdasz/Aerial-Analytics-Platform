import { invoke } from "@tauri-apps/api/core";

export async function invokeMap<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (err) {
    console.error(`[map] ${cmd} failed:`, err);
    throw err;
  }
}

// Tugasnya tangkap kode error dari backend tauri rust agar dapat ditampilkan di frontend
