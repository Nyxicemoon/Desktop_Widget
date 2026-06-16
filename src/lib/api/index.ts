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

export interface Todo {
  id: number;
  title: string;
  note: string | null;
  done: boolean;
  due_date: string | null;
  reward_coin: number;
  created_at: string;
  done_at: string | null;
}

export interface GameProfile {
  coins: number;
  exp: number;
  level: number;
  last_tick: string;
}

export interface ToggleResult {
  todo: Todo;
  awarded: number;
  coins: number;
}

export function todoCreate(
  title: string,
  note: string | null = null,
  dueDate: string | null = null,
): Promise<Todo> {
  return call<Todo>("todo_create", { title, note, dueDate });
}

export function todoUpdate(
  id: number,
  title: string,
  note: string | null = null,
  dueDate: string | null = null,
): Promise<Todo> {
  return call<Todo>("todo_update", { id, title, note, dueDate });
}

export function todoDelete(id: number): Promise<void> {
  return call<void>("todo_delete", { id });
}

export function todoListToday(): Promise<Todo[]> {
  return call<Todo[]>("todo_list_today");
}

export function todoToggleDone(id: number): Promise<ToggleResult> {
  return call<ToggleResult>("todo_toggle_done", { id });
}

export function gameGetProfile(): Promise<GameProfile> {
  return call<GameProfile>("game_get_profile");
}
