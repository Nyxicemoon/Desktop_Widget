import { invoke } from "@tauri-apps/api/core";
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";

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

export interface PhotoResult {
  id: number;
  source_url: string;
  author: string;
  author_url: string;
  thumb_url: string;
  download_url: string;
  alt: string;
}

export interface CurrentBackground {
  data_url: string;
  source_url: string;
  author: string | null;
}

export function configHasKey(): Promise<boolean> {
  return call<boolean>("config_has_key");
}

export function configSetPexelsKey(key: string): Promise<void> {
  return call<void>("config_set_pexels_key", { key });
}

export function bgSearch(keyword: string): Promise<PhotoResult[]> {
  return call<PhotoResult[]>("bg_search", { keyword });
}

export function bgDownloadAndSet(photo: PhotoResult, keyword: string): Promise<void> {
  return call<void>("bg_download_and_set", { photo, keyword });
}

export function bgGetCurrent(): Promise<CurrentBackground | null> {
  return call<CurrentBackground | null>("bg_get_current");
}

export function bgRestoreDefault(): Promise<void> {
  return call<void>("bg_restore_default");
}

export interface WidgetVisibility {
  todo: boolean;
  coins: boolean;
  apps: boolean;
}

export function widgetSetVisible(kind: "todo" | "coins" | "apps", visible: boolean): Promise<void> {
  return call<void>("widget_set_visible", { kind, visible });
}

export function widgetGetVisibility(): Promise<WidgetVisibility> {
  return call<WidgetVisibility>("widget_get_visibility");
}

export async function sendTestNotification(): Promise<void> {
  let granted = await isPermissionGranted();
  if (!granted) {
    granted = (await requestPermission()) === "granted";
  }
  if (granted) {
    sendNotification({ title: "DeskHub", body: "测试通知 / Test notification" });
  }
}

export function autostartGet(): Promise<boolean> {
  return call<boolean>("autostart_get");
}

export function autostartSet(enabled: boolean): Promise<void> {
  return call<void>("autostart_set", { enabled });
}

export function dbExport(dest: string): Promise<void> {
  return call<void>("db_export", { dest });
}

export function dbImport(src: string): Promise<void> {
  return call<void>("db_import", { src });
}

export interface AppEntry {
  id: number;
  name: string;
  target: string;
  args: string | null;
}

export function appList(): Promise<AppEntry[]> {
  return call<AppEntry[]>("app_list");
}

export function appIcon(path: string): Promise<string | null> {
  return call<string | null>("app_icon", { path });
}

export function appLaunch(path: string): Promise<void> {
  return call<void>("app_launch", { path });
}

export function appAddDropped(path: string): Promise<void> {
  return call<void>("app_add_dropped", { path });
}

export function appRemove(id: number): Promise<void> {
  return call<void>("app_remove", { id });
}

export function appRename(id: number, name: string): Promise<void> {
  return call<void>("app_rename", { id, name });
}

export function appReorder(ids: number[]): Promise<void> {
  return call<void>("app_reorder", { ids });
}
