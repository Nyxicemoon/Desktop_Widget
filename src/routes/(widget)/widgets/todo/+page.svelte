<script lang="ts">
  import { onMount } from "svelte";
  import { todos, loadTodos, toggleTodo } from "$lib/stores/todos";

  onMount(() => {
    void loadTodos();
  });

  async function onToggle(id: number) {
    await toggleTodo(id);
  }
</script>

<div class="widget">
  <div class="head" data-tauri-drag-region>📋 今日 / Today</div>
  <ul>
    {#each $todos as todo (todo.id)}
      <li>
        <input type="checkbox" checked={todo.done} onchange={() => onToggle(todo.id)} />
        <span class:done={todo.done}>{todo.title}</span>
      </li>
    {/each}
    {#if $todos.length === 0}
      <li class="empty">无任务 / Empty</li>
    {/if}
  </ul>
</div>

<style>
  .widget {
    height: 100vh;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    background: rgba(20, 20, 20, 0.55);
    color: #fff;
    border-radius: 12px;
    padding: 0.5rem 0.7rem;
    backdrop-filter: blur(6px);
    overflow: hidden;
  }

  .head {
    font-weight: 600;
    padding: 0.2rem 0.1rem 0.4rem;
    cursor: move;
    user-select: none;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    overflow: auto;
  }

  li {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.2rem 0;
  }

  .done {
    text-decoration: line-through;
    opacity: 0.6;
  }

  .empty {
    opacity: 0.6;
    justify-content: center;
  }
</style>
