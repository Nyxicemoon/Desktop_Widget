<script lang="ts">
  import { onMount } from "svelte";
  import { mailUnreadCount } from "$lib/api";

  let count = $state(0);
  let pollInterval: ReturnType<typeof setInterval> | null = null;

  async function updateCount() {
    try {
      count = await mailUnreadCount();
    } catch (e) {
      console.error("Failed to fetch unread count:", e);
    }
  }

  onMount(() => {
    updateCount();
    pollInterval = setInterval(updateCount, 5 * 60 * 1000);
    return () => {
      if (pollInterval) clearInterval(pollInterval);
    };
  });
</script>

<div class="widget" data-tauri-drag-region>📧 {count}</div>

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
