import { writable } from "svelte/store";
import { gameGetProfile, gameStatus, type GameStatus } from "$lib/api";

export const coins = writable<number>(0);
export const gameState = writable<GameStatus | null>(null);

export async function refreshCoins(): Promise<void> {
  const profile = await gameGetProfile();
  coins.set(profile.coins);
}

export async function refreshStatus(): Promise<void> {
  const status = await gameStatus();
  gameState.set(status);
  coins.set(status.coins);
}
