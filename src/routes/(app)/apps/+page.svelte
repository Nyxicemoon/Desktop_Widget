<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import {
    appsScan,
    appIcon,
    appLaunch,
    appAddDropped,
    appRemoveCustom,
    appSetFavorite,
    appSetCategory,
    type AppEntry,
  } from "$lib/api";

  let apps = $state<AppEntry[]>([]);
  let icons = $state<Record<string, string | null>>({});
  let filter = $state<"all" | "favorite">("all");
  let categoryFilter = $state<string>("");
  let message = $state("");
  let unlisten: (() => void) | null = null;

  const categories = $derived(
    Array.from(new Set(apps.map((a) => a.category).filter((c): c is string => !!c))),
  );

  const shown = $derived(
    apps.filter((a) => {
      if (filter === "favorite" && !a.favorite) return false;
      if (categoryFilter && a.category !== categoryFilter) return false;
      return true;
    }),
  );

  async function refresh() {
    apps = await appsScan();
    for (const a of apps) {
      if (!(a.launch_path in icons)) {
        icons[a.launch_path] = null;
        appIcon(a.launch_path)
          .then((d) => (icons[a.launch_path] = d))
          .catch(() => (icons[a.launch_path] = null));
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
          } catch (e) {
            message = `添加失败 / Add failed: ${e}`;
          }
        }
        await refresh();
      }
    });
  });

  onDestroy(() => unlisten?.());

  async function launch(a: AppEntry) {
    try {
      await appLaunch(a.launch_path);
    } catch (e) {
      message = `启动失败 / Launch failed: ${e}`;
    }
  }

  async function toggleFav(a: AppEntry) {
    await appSetFavorite(a.target, !a.favorite);
    await refresh();
  }

  async function assignCategory(a: AppEntry) {
    const c = prompt("分类名 / Category (留空清除):", a.category ?? "");
    if (c === null) return;
    await appSetCategory(a.target, c.trim() === "" ? null : c.trim());
    await refresh();
  }

  async function remove(a: AppEntry) {
    await appRemoveCustom(a.target);
    await refresh();
  }

  function initial(name: string): string {
    return name.trim().charAt(0).toUpperCase() || "?";
  }
</script>

<main class="container">
  <h1>应用 / Apps</h1>
  <p class="hint">把桌面图标拖到这里即可添加 / Drag desktop icons here to add.</p>

  <div class="filters">
    <button class:active={filter === "all"} onclick={() => (filter = "all")}>全部 / All</button>
    <button class:active={filter === "favorite"} onclick={() => (filter = "favorite")}>收藏 / Favorites</button>
    {#if categories.length}
      <select bind:value={categoryFilter}>
        <option value="">所有分类 / All categories</option>
        {#each categories as c}
          <option value={c}>{c}</option>
        {/each}
      </select>
    {/if}
  </div>

  {#if message}<p class="msg">{message}</p>{/if}

  <div class="grid">
    {#each shown as a (a.target)}
      <div class="card">
        <button class="icon-btn" onclick={() => launch(a)} title={a.target}>
          {#if icons[a.launch_path]}
            <img src={icons[a.launch_path]} alt={a.name} />
          {:else}
            <span class="placeholder">{initial(a.name)}</span>
          {/if}
          <span class="name">{a.name}</span>
        </button>
        <div class="row">
          <button class="ghost" onclick={() => toggleFav(a)} title="收藏 / Favorite">
            {a.favorite ? "★" : "☆"}
          </button>
          <button class="ghost" onclick={() => assignCategory(a)} title="分类 / Category">🏷️</button>
          {#if a.is_custom}
            <button class="ghost" onclick={() => remove(a)} title="移除 / Remove">🗑️</button>
          {/if}
        </div>
        {#if a.category}<span class="cat">{a.category}</span>{/if}
      </div>
    {/each}
  </div>
</main>

<style>
  .container { max-width: 900px; margin: 0 auto; padding: 1.5rem 1rem; }
  .hint { opacity: 0.7; font-size: 0.9em; }
  .filters { display: flex; gap: 0.5rem; align-items: center; margin: 0.75rem 0; flex-wrap: wrap; }
  .filters button, .filters select {
    border-radius: 8px; border: 1px solid var(--border);
    padding: 0.4em 0.8em; color: var(--fg); background: var(--surface); cursor: pointer;
  }
  .filters button.active { border-color: var(--fg); font-weight: 600; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(120px, 1fr)); gap: 0.75rem; }
  .card {
    border: 1px solid var(--border); border-radius: 10px; padding: 0.6rem;
    display: flex; flex-direction: column; align-items: center; gap: 0.4rem;
  }
  .icon-btn {
    display: flex; flex-direction: column; align-items: center; gap: 0.4rem;
    background: transparent; border: none; color: var(--fg); cursor: pointer; width: 100%;
  }
  .icon-btn img { width: 48px; height: 48px; object-fit: contain; }
  .placeholder {
    width: 48px; height: 48px; border-radius: 10px; background: var(--border);
    display: flex; align-items: center; justify-content: center; font-size: 1.4rem; font-weight: 700;
  }
  .name { font-size: 0.85em; text-align: center; word-break: break-word; }
  .row { display: flex; gap: 0.3rem; }
  .ghost { background: transparent; border: none; cursor: pointer; font-size: 1em; }
  .cat { font-size: 0.75em; opacity: 0.7; }
  .msg { opacity: 0.85; }
</style>
