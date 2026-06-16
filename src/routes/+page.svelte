<script lang="ts">
  import { onMount } from "svelte";
  import { theme, initTheme, toggleTheme } from "$lib/stores/theme";

  onMount(() => {
    void initTheme();
  });
</script>

<main class="container">
  <h1>DeskHub</h1>
  <p>当前主题 / Theme: {$theme}</p>
  <button onclick={toggleTheme}>切换主题 / Toggle theme</button>
</main>

<style>
  .container {
    padding: 10vh 1rem;
    text-align: center;
  }

  button {
    border-radius: 8px;
    border: 1px solid var(--border);
    padding: 0.6em 1.2em;
    font-size: 1em;
    color: var(--fg);
    background: var(--surface);
    cursor: pointer;
  }
</style>
