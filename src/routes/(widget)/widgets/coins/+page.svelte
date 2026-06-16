<script lang="ts">
  import { onMount } from "svelte";
  import { coins, refreshCoins } from "$lib/stores/game";

  onMount(() => {
    void refreshCoins();
  });
</script>

<div class="widget" data-tauri-drag-region>🪙 {$coins}</div>

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
