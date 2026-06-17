<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { appsScan, appIcon, appLaunch, appAddDropped, type AppEntry } from "$lib/api";

  let favs = $state<AppEntry[]>([]);
  let icons = $state<Record<string, string | null>>({});
  let unlisten: (() => void) | null = null;

  async function refresh() {
    const all = await appsScan();
    favs = all.filter((a) => a.favorite);
    for (const a of favs) {
      if (!(a.launch_path in icons)) {
        icons[a.launch_path] = null;
        appIcon(a.launch_path).then((d) => (icons[a.launch_path] = d)).catch(() => {});
      }
    }
  }

  onMount(async () => {
    await refresh();
    const wv = getCurrentWebview();
    unlisten = await wv.onDragDropEvent(async (event) => {
      if (event.payload.type === "drop") {
        for (const p of event.payload.paths) {
          try {
            await appAddDropped(p);
          } catch {
            /* ignore */
          }
        }
        await refresh();
      }
    });
  });

  onDestroy(() => unlisten?.());

  function initial(name: string): string {
    return name.trim().charAt(0).toUpperCase() || "?";
  }
</script>

<div class="widget" data-tauri-drag-region>
  <div class="grid">
    {#each favs as a (a.target)}
      <button class="app" onclick={() => appLaunch(a.launch_path)} title={a.name}>
        {#if icons[a.launch_path]}
          <img src={icons[a.launch_path]} alt={a.name} />
        {:else}
          <span class="placeholder">{initial(a.name)}</span>
        {/if}
      </button>
    {/each}
    {#if favs.length === 0}
      <p class="empty">把图标拖进来 / Drag icons here</p>
    {/if}
  </div>
</div>

<style>
  :global(html), :global(body) { background: transparent !important; margin: 0; }
  .widget {
    background: rgba(20, 20, 20, 0.55);
    border-radius: 14px;
    padding: 0.6rem;
    height: 100vh;
    box-sizing: border-box;
    color: #fff;
    -webkit-backdrop-filter: blur(8px);
    backdrop-filter: blur(8px);
  }
  .grid { display: grid; grid-template-columns: repeat(4, 1fr); gap: 0.5rem; }
  .app { background: transparent; border: none; cursor: pointer; padding: 0.2rem; }
  .app img { width: 40px; height: 40px; object-fit: contain; }
  .placeholder {
    display: flex; width: 40px; height: 40px; border-radius: 8px;
    align-items: center; justify-content: center;
    background: rgba(255, 255, 255, 0.2); color: #fff; font-weight: 700;
  }
  .empty { font-size: 0.8em; opacity: 0.8; grid-column: 1 / -1; text-align: center; }
</style>
