import { invoke } from "@tauri-apps/api/core";

export interface AppErrorShape {
  kind: string;
  message: string;
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (err) {
    console.error(`command ${cmd} failed:`, err);
    throw err;
  }
}

export function kvGet(key: string): Promise<string | null> {
  return call<string | null>("kv_get", { key });
}

export function kvSet(key: string, value: string): Promise<void> {
  return call<void>("kv_set", { key, value });
}
