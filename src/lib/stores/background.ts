import { writable } from "svelte/store";
import { bgGetCurrent, bgRestoreDefault, type CurrentBackground } from "$lib/api";

export const currentBg = writable<CurrentBackground | null>(null);

export async function loadBackground(): Promise<void> {
  currentBg.set(await bgGetCurrent());
}

export async function clearBackground(): Promise<void> {
  await bgRestoreDefault();
  await loadBackground();
}
