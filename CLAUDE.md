# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

DeskHub — a Windows desktop efficiency tool (Todo, desktop-icon management, email, background-image management, idle game). The full requirements spec is in Chinese.

@项目.md

Iteration plan (milestones M0–M6): see [开发计划.md](开发计划.md).

## Stack

Tauri v2 (desktop shell) + Svelte + TypeScript (frontend) + Rust (backend/system calls) + SQLite (local storage). System integration via `windows-rs`; HTTP/image download via `reqwest`. Chosen for low memory and good background-run performance over Electron.

- SQLite access layer: **rusqlite** (sync, lightweight). All DB access is isolated behind the `src-tauri/src/db/` module so the layer stays swappable.

## Conventions

- Spec and planning docs are written in Chinese; match that language when editing them. Code identifiers and comments follow normal English conventions unless existing code says otherwise.
- This is a local-first app: persist data to SQLite, not remote services. Background images must record their source URL for licensing (see spec section 十).

## Commands

- `npm run tauri dev` — run the app in dev mode
- `npm run tauri build` — package the Windows app
- `npm run check` — Svelte/TS type-check (svelte-check)
- `cargo test` — Rust tests (run inside `src-tauri/`)
- `cargo clippy` — Rust lints (run inside `src-tauri/`)
