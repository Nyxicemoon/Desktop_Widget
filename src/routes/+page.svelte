<script lang="ts">
  import { onMount } from "svelte";
  import { coins } from "$lib/stores/game";
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

  onMount(() => {
    void loadTodos();
  });

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
</style>
