<script lang="ts">
  import { onMount } from "svelte";
  import {
    configHasKey,
    configSetPexelsKey,
    bgSearch,
    bgDownloadAndSet,
    type PhotoResult,
  } from "$lib/api";
  import { clearBackground } from "$lib/stores/background";

  const presets = ["森林", "雪山", "湖泊", "海边", "星空"];

  let hasKey = $state(false);
  let keyInput = $state("");
  let keyword = $state("");
  let results = $state<PhotoResult[]>([]);
  let busy = $state(false);
  let message = $state("");

  onMount(async () => {
    hasKey = await configHasKey();
  });

  async function saveKey() {
    const k = keyInput.trim();
    if (!k) return;
    await configSetPexelsKey(k);
    keyInput = "";
    hasKey = true;
    message = "已保存 Key / Key saved";
  }

  async function runSearch(q: string) {
    keyword = q;
    const term = q.trim();
    if (!term) return;
    busy = true;
    message = "";
    try {
      results = await bgSearch(term);
      if (results.length === 0) message = "没有结果 / No results";
    } catch (e) {
      message = `搜索失败 / Search failed: ${e}`;
    } finally {
      busy = false;
    }
  }

  async function pick(photo: PhotoResult) {
    busy = true;
    message = "";
    try {
      await bgDownloadAndSet(photo, keyword);
      message = "已设为壁纸 / Set as wallpaper";
    } catch (e) {
      message = `设置失败 / Failed: ${e}`;
    } finally {
      busy = false;
    }
  }

  async function restore() {
    await clearBackground();
    message = "已恢复默认 / Restored default";
  }
</script>

<main class="container">
  <h1>背景图片 / Backgrounds</h1>

  {#if !hasKey}
    <section class="card">
      <p>请先填入 Pexels API Key（保存在本地，不上传）。</p>
      <p>Enter your Pexels API key (stored locally).</p>
      <div class="row">
        <input placeholder="Pexels API Key" bind:value={keyInput} />
        <button onclick={saveKey}>保存 / Save</button>
      </div>
    </section>
  {/if}

  <section class="search">
    <div class="row">
      <input
        placeholder="关键词 / Keyword..."
        bind:value={keyword}
        onkeydown={(e) => e.key === "Enter" && runSearch(keyword)}
      />
      <button onclick={() => runSearch(keyword)} disabled={busy}>搜索 / Search</button>
      <button class="ghost" onclick={restore}>恢复默认 / Restore</button>
    </div>
    <div class="presets">
      {#each presets as p}
        <button class="chip" onclick={() => runSearch(p)} disabled={busy}>{p}</button>
      {/each}
    </div>
  </section>

  {#if message}
    <p class="msg">{message}</p>
  {/if}

  <div class="grid">
    {#each results as photo (photo.id)}
      <button class="thumb" onclick={() => pick(photo)} disabled={busy} title={photo.alt}>
        <img src={photo.thumb_url} alt={photo.alt} loading="lazy" />
      </button>
    {/each}
  </div>
</main>

<style>
  .container {
    max-width: 800px;
    margin: 0 auto;
    padding: 1.5rem 1rem;
  }

  .card,
  .search {
    margin-bottom: 1rem;
  }

  .row {
    display: flex;
    gap: 0.5rem;
  }

  .row input {
    flex: 1;
  }

  input,
  button {
    border-radius: 8px;
    border: 1px solid var(--border);
    padding: 0.5em 0.8em;
    font-size: 1em;
    color: var(--fg);
    background: var(--surface);
    cursor: pointer;
  }

  .presets {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
    margin-top: 0.5rem;
  }

  .chip {
    border-radius: 999px;
    padding: 0.3em 0.9em;
  }

  .ghost {
    border-color: transparent;
    background: transparent;
  }

  .msg {
    opacity: 0.8;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 0.6rem;
  }

  .thumb {
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 8px;
    overflow: hidden;
    aspect-ratio: 4 / 3;
  }

  .thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
</style>
