import { get, writable } from "svelte/store";
import { kvGet, kvSet } from "$lib/api";

export type Theme = "light" | "dark";

function systemTheme(): Theme {
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

function apply(value: Theme): void {
  document.documentElement.dataset.theme = value;
}

export const theme = writable<Theme>("light");

/** Load persisted theme (or system default) and apply it. Call once on mount. */
export async function initTheme(): Promise<void> {
  const saved = await kvGet("theme");
  const value: Theme = saved === "dark" || saved === "light" ? saved : systemTheme();
  apply(value);
  theme.set(value);
}

export function setTheme(value: Theme): void {
  apply(value);
  theme.set(value);
  void kvSet("theme", value);
}

export function toggleTheme(): void {
  setTheme(get(theme) === "dark" ? "light" : "dark");
}
