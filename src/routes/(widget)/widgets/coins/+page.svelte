<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { coins, gameState, refreshStatus } from "$lib/stores/game";

  let statusRefreshInterval: number | null = null;

  onMount(() => {
    void refreshStatus();

    // Set up 60s refresh interval
    statusRefreshInterval = window.setInterval(async () => {
      await refreshStatus();
    }, 60000);
  });

  onDestroy(() => {
    if (statusRefreshInterval !== null) {
      clearInterval(statusRefreshInterval);
    }
  });
</script>

<div class="widget" data-tauri-drag-region>
  🪙 {$coins}　Lv {$gameState?.level ?? 1}
</div>

<style>
  .widget {
    height: 100vh;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(20, 20, 20, 0.55);
    color: #fff;
    border-radius: 12px;
    font-size: 1.4rem;
    font-weight: 700;
    backdrop-filter: blur(6px);
    cursor: move;
    user-select: none;
  }
</style>
