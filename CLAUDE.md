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

## Git / PR Workflow

- Remote: `origin` → https://github.com/Nyxicemoon/Desktop_Widget (public, MIT). Default branch: **`main`**.
- **Never commit or push directly to `main`.** Every change goes through a pull request.
- Per feature/milestone:
  1. Branch from up-to-date `main` (e.g. `m4-icons`, `fix-xyz`).
  2. Commit per task (TDD: keep `cargo test` / `cargo clippy -- -D warnings` / `npm run check` green before each commit).
  3. Push the branch, open a PR with `gh pr create`.
  4. Merge after review (`gh pr merge --squash --delete-branch`), then `git checkout main && git pull`.
- Use the **`gh` CLI** for all GitHub-platform operations (PRs, issues, releases, repo settings). `gh` is installed at `C:\Program Files\GitHub CLI\gh.exe`; if not on `PATH` in a fresh shell, call it by full path. Authenticated as `Nyxicemoon`.
- Roadmap milestones are tracked as GitHub issues: M4 = [#1](https://github.com/Nyxicemoon/Desktop_Widget/issues/1), M5 = [#2](https://github.com/Nyxicemoon/Desktop_Widget/issues/2), M6 = [#3](https://github.com/Nyxicemoon/Desktop_Widget/issues/3). Reference the issue in the PR (e.g. `Closes #1`).
