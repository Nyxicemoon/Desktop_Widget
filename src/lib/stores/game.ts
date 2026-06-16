import { writable } from "svelte/store";
import { gameGetProfile } from "$lib/api";

export const coins = writable<number>(0);

export async function refreshCoins(): Promise<void> {
  const profile = await gameGetProfile();
  coins.set(profile.coins);
}
