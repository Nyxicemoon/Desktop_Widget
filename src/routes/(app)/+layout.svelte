<script lang="ts">
  import { onMount } from "svelte";
  import { theme, initTheme, toggleTheme } from "$lib/stores/theme";
  import { coins, refreshCoins } from "$lib/stores/game";
  import { currentBg, loadBackground } from "$lib/stores/background";

  let { children } = $props();

  onMount(() => {
    void initTheme();
    void refreshCoins();
    void loadBackground();
  });
</script>

{#if $currentBg}
  <div class="bg-layer" style:background-image={`url(${$currentBg.data_url})`}></div>
{/if}

<div class="app-shell">
  <header class="bar">
    <nav>
      <a href="/">待办 / Todos</a>
      <a href="/backgrounds">背景 / Backgrounds</a>
    </nav>
    <span class="grow"></span>
    <span class="coins">🪙 {$coins}</span>
    <button class="ghost" onclick={toggleTheme} title="主题 / Theme">
      {$theme === "dark" ? "🌙" : "☀️"}
    </button>
  </header>

  {@render children()}
</div>

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: 0.5rem 1rem;
    border-bottom: 1px solid var(--border);
  }

  nav {
    display: flex;
    gap: 1rem;
  }

  nav a {
    color: var(--fg);
    text-decoration: none;
    opacity: 0.8;
  }

  nav a:hover {
    opacity: 1;
  }

  .grow {
    flex: 1;
  }

  .coins {
    font-weight: 600;
  }

  .ghost {
    border: 1px solid transparent;
    background: transparent;
    color: var(--fg);
    cursor: pointer;
    padding: 0.3em 0.5em;
    border-radius: 8px;
  }
</style>
