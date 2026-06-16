import { bgRestoreDefault } from "$lib/api";

export async function clearBackground(): Promise<void> {
  await bgRestoreDefault();
}
