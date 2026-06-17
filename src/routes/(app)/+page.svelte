<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { coins, gameState, refreshStatus } from "$lib/stores/game";
  import { widgetSetVisible, widgetGetVisibility, sendTestNotification, gameTakeOfflineEarned } from "$lib/api";
  import {
    todos,
    loadTodos,
    addTodo,
    editTodo,
    removeTodo,
    toggleTodo,
  } from "$lib/stores/todos";

  let newTitle = $state("");
  let editingId = $state<number | null>(null);
  let editingTitle = $state("");
  let reward = $state(0);
  let widgetTodo = $state(false);
  let widgetCoins = $state(false);
  let widgetApps = $state(false);
  let widgetMail = $state(false);
  let offlineEarned = $state(0);
  let showOfflineBanner = $state(false);
  let statusRefreshInterval: number | null = $state(null);

  onMount(async () => {
    void loadTodos();
    const v = await widgetGetVisibility();
    widgetTodo = v.todo;
    widgetCoins = v.coins;
    widgetApps = v.apps;
    widgetMail = v.mail;

    // Load game status
    await refreshStatus();

    // Check for offline earnings
    const earned = await gameTakeOfflineEarned();
    if (earned > 0) {
      offlineEarned = earned;
      showOfflineBanner = true;
    }

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

  async function toggleWidget(kind: "todo" | "coins" | "apps" | "mail", on: boolean) {
    await widgetSetVisible(kind, on);
    if (kind === "todo") widgetTodo = on;
    else if (kind === "coins") widgetCoins = on;
    else if (kind === "apps") widgetApps = on;
    else if (kind === "mail") widgetMail = on;
  }

  async function submitNew(e: Event) {
    e.preventDefault();
    const t = newTitle.trim();
    if (!t) return;
    newTitle = "";
    await addTodo(t);
  }

  async function onToggle(id: number) {
    const res = await toggleTodo(id);
    coins.set(res.coins);
    if (res.awarded > 0) {
      reward = res.awarded;
      setTimeout(() => (reward = 0), 1200);
    }
  }

  function startEdit(id: number, title: string) {
    editingId = id;
    editingTitle = title;
  }

  async function saveEdit(id: number) {
    const t = editingTitle.trim();
    editingId = null;
    if (t) {
      await editTodo(id, t);
    } else {
      await loadTodos();
    }
  }
</script>

<main class="container">
  <h1>DeskHub</h1>

  {#if showOfflineBanner}
    <div class="offline-banner">
      <span>🎁 离线获得 {offlineEarned} 金币 / Earned {offlineEarned} coins while away</span>
      <button class="ghost" onclick={() => (showOfflineBanner = false)}>✕</button>
    </div>
  {/if}

  {#if $gameState}
    <div class="game-panel">
      <div class="game-header">
        <span class="level">Lv {$gameState.level}</span>
        <span class="rate">⚙ {$gameState.rate_per_min}/min</span>
        <span class="coins">🪙 {$gameState.coins}</span>
      </div>
      <div class="exp-bar">
        <div
          class="exp-fill"
          style="width: {($gameState.exp_into_level / $gameState.exp_for_next) * 100}%"
        ></div>
      </div>
    </div>
  {/if}

  <section class="widgets">
    <label>
      <input
        type="checkbox"
        checked={widgetTodo}
        onchange={(e) => toggleWidget("todo", e.currentTarget.checked)}
      />
      桌面 Todo 组件 / Todo widget
    </label>
    <label>
      <input
        type="checkbox"
        checked={widgetCoins}
        onchange={(e) => toggleWidget("coins", e.currentTarget.checked)}
      />
      桌面金币组件 / Coins widget
    </label>
    <label>
      <input
        type="checkbox"
        checked={widgetApps}
        onchange={(e) => toggleWidget("apps", e.currentTarget.checked)}
      />
      桌面应用组件 / Apps widget
    </label>
    <label>
      <input
        type="checkbox"
        checked={widgetMail}
        onchange={(e) => toggleWidget("mail", e.currentTarget.checked)}
      />
      桌面邮件组件 / Mail widget
    </label>
    <button class="notify-btn" onclick={() => sendTestNotification()}>
      🔔 发送测试通知 / Send test notification
    </button>
  </section>

  {#if reward > 0}
    <div class="reward">+{reward}🪙</div>
  {/if}

  <form class="add" onsubmit={submitNew}>
    <input placeholder="新建任务 / New task..." bind:value={newTitle} />
    <button type="submit">添加 / Add</button>
  </form>

  <ul class="list">
    {#each $todos as todo (todo.id)}
      <li class:done={todo.done}>
        <input
          type="checkbox"
          checked={todo.done}
          onchange={() => onToggle(todo.id)}
        />
        {#if editingId === todo.id}
          <!-- svelte-ignore a11y_autofocus -->
          <input
            class="edit"
            bind:value={editingTitle}
            onblur={() => saveEdit(todo.id)}
            onkeydown={(e) => e.key === "Enter" && saveEdit(todo.id)}
            autofocus
          />
        {:else}
          <span class="title">{todo.title}</span>
        {/if}
        <span class="tag">+{todo.reward_coin}🪙</span>
        <button class="ghost" onclick={() => startEdit(todo.id, todo.title)}>✎</button>
        <button class="ghost" onclick={() => removeTodo(todo.id)}>🗑</button>
      </li>
    {/each}
    {#if $todos.length === 0}
      <li class="empty">今天还没有任务 / No tasks yet</li>
    {/if}
  </ul>
</main>

<style>
  .container {
    max-width: 640px;
    margin: 0 auto;
    padding: 1.5rem 1rem;
  }

  h1 {
    text-align: center;
  }

  .offline-banner {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 0.5rem;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.7rem 1rem;
    margin: 0.5rem 0 1rem;
    font-size: 0.9rem;
  }

  .game-panel {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 1rem;
    margin: 0.5rem 0 1rem;
  }

  .game-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 1rem;
    margin-bottom: 0.5rem;
    font-size: 0.95rem;
  }

  .level,
  .rate,
  .coins {
    font-weight: 500;
  }

  .exp-bar {
    width: 100%;
    height: 8px;
    background: var(--border);
    border-radius: 4px;
    overflow: hidden;
  }

  .exp-fill {
    height: 100%;
    background: linear-gradient(90deg, #4a9eff, #7c4dff);
    transition: width 0.3s ease;
  }

  .reward {
    text-align: center;
    color: #e0a300;
    font-weight: 700;
    animation: floatup 1.2s ease-out;
  }

  @keyframes floatup {
    from {
      opacity: 1;
      transform: translateY(0);
    }
    to {
      opacity: 0;
      transform: translateY(-1.5rem);
    }
  }

  .add {
    display: flex;
    gap: 0.5rem;
    margin: 1rem 0;
  }

  .add input {
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
  }

  button {
    cursor: pointer;
  }

  .ghost {
    border-color: transparent;
    background: transparent;
    padding: 0.3em 0.5em;
  }

  .list {
    list-style: none;
    padding: 0;
    margin: 0;
  }

  .list li {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.4rem 0;
    border-bottom: 1px solid var(--border);
  }

  .list li.done .title {
    text-decoration: line-through;
    opacity: 0.6;
  }

  .title {
    flex: 1;
  }

  .edit {
    flex: 1;
  }

  .tag {
    font-size: 0.85em;
    opacity: 0.7;
  }

  .empty {
    justify-content: center;
    opacity: 0.6;
  }

  .widgets {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    margin: 0.5rem 0 1rem;
    font-size: 0.9rem;
    opacity: 0.9;
  }

  .widgets label {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }

  .notify-btn {
    align-self: flex-start;
    margin-top: 0.3rem;
  }
</style>
