<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import {
    appList,
    appIcon,
    appLaunch,
    appAddDropped,
    appRemove,
    appRename,
    appReorder,
    type AppEntry,
  } from "$lib/api";

  let apps = $state<AppEntry[]>([]);
  let icons = $state<Record<string, string | null>>({});
  let edit = $state(false);
  let renamingId = $state<number | null>(null);
  let renameText = $state("");
  let dragIndex = $state<number | null>(null);
  let unlisten: (() => void) | null = null;

  async function refresh() {
    apps = await appList();
    for (const a of apps) {
      if (!(a.target in icons)) {
        icons[a.target] = null;
        appIcon(a.target)
          .then((d) => (icons[a.target] = d))
          .catch(() => (icons[a.target] = null));
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

  async function onClickApp(a: AppEntry) {
    if (edit) return;
    try {
      await appLaunch(a.target);
    } catch {
      /* ignore */
    }
  }

  async function remove(a: AppEntry) {
    await appRemove(a.id);
    await refresh();
  }

  function beginRename(a: AppEntry) {
    renamingId = a.id;
    renameText = a.name;
  }

  async function commitRename(a: AppEntry) {
    const name = renameText.trim();
    renamingId = null;
    if (name && name !== a.name) {
      await appRename(a.id, name);
      await refresh();
    }
  }

  function onDragStart(i: number) {
    dragIndex = i;
  }

  async function onDrop(j: number) {
    if (dragIndex === null || dragIndex === j) {
      dragIndex = null;
      return;
    }
    const next = [...apps];
    const [moved] = next.splice(dragIndex, 1);
    next.splice(j, 0, moved);
    apps = next;
    dragIndex = null;
    await appReorder(next.map((a) => a.id));
  }

  function startResize(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    void getCurrentWindow().startResizeDragging("SouthEast" as never);
  }

  function initial(name: string): string {
    return name.trim().charAt(0).toUpperCase() || "?";
  }
</script>

<div class="widget">
  <div class="header" data-tauri-drag-region>
    <span class="dots" data-tauri-drag-region>⋮⋮</span>
    <button class="edit-toggle" onclick={() => (edit = !edit)} title="编辑 / Edit">
      {edit ? "✓" : "✎"}
    </button>
  </div>

  <div class="grid">
    {#each apps as a, i (a.id)}
      <div
        class="app"
        class:editing={edit}
        draggable={edit}
        ondragstart={() => onDragStart(i)}
        ondragover={(e) => e.preventDefault()}
        ondrop={() => onDrop(i)}
        role="button"
        tabindex="0"
      >
        {#if edit}
          <button class="del" onclick={() => remove(a)} title="移除 / Remove">✕</button>
        {/if}
        <button class="icon" onclick={() => onClickApp(a)} title={a.target}>
          {#if icons[a.target]}
            <img src={icons[a.target]} alt={a.name} />
          {:else}
            <span class="placeholder">{initial(a.name)}</span>
          {/if}
        </button>
        {#if renamingId === a.id}
          <!-- svelte-ignore a11y_autofocus -->
          <input
            class="rename"
            bind:value={renameText}
            autofocus
            onblur={() => commitRename(a)}
            onkeydown={(e) => e.key === "Enter" && commitRename(a)}
          />
        {:else}
          <span
            class="name"
            ondblclick={() => edit && beginRename(a)}
            role="textbox"
            tabindex="-1"
          >{a.name}</span>
        {/if}
      </div>
    {/each}
    {#if apps.length === 0}
      <p class="empty">把桌面图标拖进来<br />Drag desktop icons here</p>
    {/if}
  </div>

  <div
    class="resize-grip"
    onmousedown={startResize}
    role="presentation"
    title="缩放 / Resize"
  ></div>
</div>

<style>
  :global(html),
  :global(body) {
    background: transparent !important;
    margin: 0;
  }
  .widget {
    position: relative;
    height: 100vh;
    box-sizing: border-box;
    background: rgba(20, 20, 20, 0.55);
    color: #fff;
    border-radius: 14px;
    padding: 0.4rem;
    -webkit-backdrop-filter: blur(8px);
    backdrop-filter: blur(8px);
    user-select: none;
    overflow: hidden;
  }
  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 22px;
    cursor: move;
  }
  .dots {
    opacity: 0.5;
    font-size: 0.8rem;
  }
  .edit-toggle {
    background: transparent;
    border: none;
    color: #fff;
    cursor: pointer;
    opacity: 0.8;
    font-size: 0.9rem;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(64px, 1fr));
    gap: 0.4rem;
    overflow-y: auto;
    height: calc(100% - 22px);
    align-content: start;
  }
  .app {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.2rem;
    padding: 0.2rem;
    border-radius: 8px;
  }
  .app.editing {
    background: rgba(255, 255, 255, 0.08);
    cursor: grab;
  }
  .icon {
    background: transparent;
    border: none;
    cursor: pointer;
    padding: 0;
  }
  .icon img {
    width: 40px;
    height: 40px;
    object-fit: contain;
  }
  .placeholder {
    display: flex;
    width: 40px;
    height: 40px;
    border-radius: 8px;
    align-items: center;
    justify-content: center;
    background: rgba(255, 255, 255, 0.2);
    color: #fff;
    font-weight: 700;
  }
  .name {
    font-size: 0.68rem;
    text-align: center;
    word-break: break-word;
    max-width: 100%;
  }
  .rename {
    width: 90%;
    font-size: 0.68rem;
    border: none;
    border-radius: 4px;
    padding: 1px 2px;
  }
  .del {
    position: absolute;
    top: -2px;
    right: -2px;
    z-index: 2;
    width: 16px;
    height: 16px;
    line-height: 14px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: #e0533d;
    color: #fff;
    font-size: 0.7rem;
    cursor: pointer;
  }
  .empty {
    grid-column: 1 / -1;
    text-align: center;
    opacity: 0.8;
    font-size: 0.8rem;
    margin-top: 1rem;
  }
  .resize-grip {
    position: absolute;
    right: 0;
    bottom: 0;
    width: 14px;
    height: 14px;
    cursor: nwse-resize;
    background: linear-gradient(135deg, transparent 50%, rgba(255, 255, 255, 0.5) 50%);
    border-bottom-right-radius: 14px;
  }
</style>
