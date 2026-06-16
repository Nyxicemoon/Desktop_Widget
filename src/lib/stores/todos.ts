import { writable } from "svelte/store";
import {
  todoListToday,
  todoCreate,
  todoUpdate,
  todoDelete,
  todoToggleDone,
  type Todo,
  type ToggleResult,
} from "$lib/api";

export const todos = writable<Todo[]>([]);

export async function loadTodos(): Promise<void> {
  todos.set(await todoListToday());
}

export async function addTodo(title: string): Promise<void> {
  await todoCreate(title);
  await loadTodos();
}

export async function editTodo(id: number, title: string): Promise<void> {
  await todoUpdate(id, title);
  await loadTodos();
}

export async function removeTodo(id: number): Promise<void> {
  await todoDelete(id);
  await loadTodos();
}

export async function toggleTodo(id: number): Promise<ToggleResult> {
  const res = await todoToggleDone(id);
  await loadTodos();
  return res;
}
