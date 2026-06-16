# DeskHub M2 — 背景图片管理 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 用 Pexels 搜索自然风景图、下载到本地、设为应用窗口背景，并强制记录来源（licensing）。

**Architecture:** 复用 M0/M1 的 `db/` 与命令模式；新增迁移 0003 建 `backgrounds`；网络（搜索/下载）在 `pexels.rs` 用 `reqwest` blocking，并在**异步命令 + `spawn_blocking`** 中执行以免冻结 UI；API key 存 `config.rs`(app_data_dir/config.json)；当前背景以 base64 data URL 回传前端，由 `+layout.svelte` 的固定背景层显示。

**Tech Stack:** Rust + Tauri v2 + rusqlite + reqwest(blocking) + base64 + serde；前端 SvelteKit + Svelte 5 + TS。

> **编译前置：** `cargo` 命令前确保 `build/` 存在（已生成）。
> **cargo：** `"$USERPROFILE/.cargo/bin/cargo.exe" --manifest-path src-tauri/Cargo.toml`。
> **Tauri 参数：** JS camelCase → Rust snake_case。

---

## 文件结构

后端：
- `db/migrations.rs`（改）— 迁移 `(3, ...)` + 测试表清单加 `backgrounds`。
- `error.rs`（改）— 新增 `Network` 变体。
- `Cargo.toml`（改）— `reqwest`(blocking,json)、`base64`。
- `models/mod.rs`（改）— `PhotoResult`、`Background`、`CurrentBackground`。
- `config.rs`（新）— 读写 `app_data_dir/config.json`。
- `pexels.rs`（新）— `parse_search_response`(纯) + `search`/`download`(网络)。
- `db/backgrounds.rs`（新）— insert/set_current/get_current/restore_default/list。
- `commands/backgrounds.rs`（新）— 6 个命令。
- `commands/mod.rs`、`lib.rs`（改）— 声明 config/pexels、注册命令。

前端：
- `lib/api/index.ts`（改）— 类型 + 封装。
- `lib/stores/background.ts`（新）。
- `routes/+layout.svelte`（改）— 顶栏(导航+金币+主题) + 背景层。
- `app.css`（改）— `.bg-layer`。
- `routes/+page.svelte`（改）— 去掉自带顶栏。
- `routes/backgrounds/+page.svelte`（新）— 设置 + 搜索 + 网格。

---

## Task 1: 迁移 0003（backgrounds 表）

**Files:** Modify `src-tauri/src/db/migrations.rs`

- [ ] **Step 1: 增加迁移 3**

在 `MIGRATIONS` 数组里、迁移 `(2, ...)` 之后追加一项（注意 `(2,...)` 末尾逗号）：

```rust
    (
        3,
        "CREATE TABLE backgrounds (
            id         INTEGER PRIMARY KEY,
            local_path TEXT NOT NULL,
            source_url TEXT NOT NULL,
            author     TEXT,
            license    TEXT,
            keyword    TEXT,
            is_current INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
        );",
    ),
```

- [ ] **Step 2: 测试表清单加 backgrounds**

把 `applies_migrations_on_empty_db` 测试里的表数组改为：

```rust
        for t in ["kv", "todos", "game_profile", "coin_ledger", "backgrounds"] {
```

- [ ] **Step 3: 运行测试**

Run: `"$USERPROFILE/.cargo/bin/cargo.exe" test --manifest-path src-tauri/Cargo.toml migrations::`
Expected: 2 测试 PASS（版本到 3、五张表）。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/db/migrations.rs
git commit -m "feat(m2): add migration 0003 (backgrounds)"
```

---

## Task 2: AppError::Network 变体

**Files:** Modify `src-tauri/src/error.rs`

- [ ] **Step 1: 增加变体**

在 `src-tauri/src/error.rs` 的 `enum AppError` 中，`Io` 变体之后加入：

```rust
    #[error("network error: {0}")]
    Network(String),
```

- [ ] **Step 2: 更新 kind() 映射**

在 `fn kind` 的 match 中，`AppError::Io(_) => "Io",` 之后加入：

```rust
            AppError::Network(_) => "Network",
```

- [ ] **Step 3: 编译检查**

Run: `"$USERPROFILE/.cargo/bin/cargo.exe" build --manifest-path src-tauri/Cargo.toml`
Expected: 编译通过（`Network` 暂未构造，dead_code 警告正常，Task 5 用到后消失）。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/error.rs
git commit -m "feat(m2): add AppError::Network variant"
```

---

## Task 3: 依赖 + 数据模型

**Files:** Modify `src-tauri/Cargo.toml`, `src-tauri/src/models/mod.rs`

- [ ] **Step 1: 增加依赖**

在 `src-tauri/Cargo.toml` 的 `[dependencies]` 末尾追加：

```toml
reqwest = { version = "0.12", features = ["blocking", "json"] }
base64 = "0.22"
```

- [ ] **Step 2: 增加模型**

在 `src-tauri/src/models/mod.rs` 末尾追加（保留已有 `Todo`/`GameProfile`/`ToggleResult`）：

```rust
use serde::Deserialize;

#[derive(Debug, Serialize, Deserialize)]
pub struct PhotoResult {
    pub id: i64,
    pub source_url: String,
    pub author: String,
    pub author_url: String,
    pub thumb_url: String,
    pub download_url: String,
    pub alt: String,
}

#[derive(Debug, Serialize)]
pub struct Background {
    pub id: i64,
    pub local_path: String,
    pub source_url: String,
    pub author: Option<String>,
    pub license: Option<String>,
    pub keyword: Option<String>,
    pub is_current: bool,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct CurrentBackground {
    pub data_url: String,
    pub source_url: String,
    pub author: Option<String>,
}
```

> `models/mod.rs` 顶部已 `use serde::Serialize;`（M1）。本步新增 `use serde::Deserialize;`。

- [ ] **Step 3: 编译检查（拉取 reqwest，首次较久）**

Run: `"$USERPROFILE/.cargo/bin/cargo.exe" build --manifest-path src-tauri/Cargo.toml`
Expected: 编译通过（新模型 dead_code 警告正常）。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/models/mod.rs
git commit -m "feat(m2): add reqwest/base64 deps and background models"
```

---

## Task 4: 配置读写 `config.rs`

**Files:** Create `src-tauri/src/config.rs`; Modify `src-tauri/src/lib.rs`

- [ ] **Step 1: 声明模块**

在 `src-tauri/src/lib.rs` 顶部模块声明区（`mod commands;` 等附近）加入：

```rust
mod config;
```

- [ ] **Step 2: 实现 + 测试**

创建 `src-tauri/src/config.rs`：

```rust
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AppConfig {
    pub pexels_api_key: Option<String>,
}

pub fn load(dir: &Path) -> AppResult<AppConfig> {
    let path = dir.join("config.json");
    if !path.exists() {
        return Ok(AppConfig::default());
    }
    let text = std::fs::read_to_string(&path)?;
    serde_json::from_str(&text).map_err(|e| AppError::Other(format!("parse config: {e}")))
}

pub fn save(dir: &Path, cfg: &AppConfig) -> AppResult<()> {
    std::fs::create_dir_all(dir)?;
    let text =
        serde_json::to_string_pretty(cfg).map_err(|e| AppError::Other(format!("write config: {e}")))?;
    std::fs::write(dir.join("config.json"), text)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("deskhub_cfg_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn load_missing_returns_default() {
        let dir = temp_dir();
        let cfg = load(&dir).unwrap();
        assert!(cfg.pexels_api_key.is_none());
    }

    #[test]
    fn save_then_load_roundtrips_key() {
        let dir = temp_dir();
        let cfg = AppConfig {
            pexels_api_key: Some("abc123".into()),
        };
        save(&dir, &cfg).unwrap();
        let loaded = load(&dir).unwrap();
        assert_eq!(loaded.pexels_api_key.as_deref(), Some("abc123"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
```

- [ ] **Step 3: 运行测试**

Run: `"$USERPROFILE/.cargo/bin/cargo.exe" test --manifest-path src-tauri/Cargo.toml config::`
Expected: 2 测试 PASS。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/config.rs src-tauri/src/lib.rs
git commit -m "feat(m2): add config.json read/write for pexels key"
```

---

## Task 5: Pexels 客户端 `pexels.rs`

**Files:** Create `src-tauri/src/pexels.rs`; Modify `src-tauri/src/lib.rs`

- [ ] **Step 1: 声明模块**

在 `src-tauri/src/lib.rs` 顶部加入：

```rust
mod pexels;
```

- [ ] **Step 2: 实现 + 解析测试**

创建 `src-tauri/src/pexels.rs`：

```rust
use crate::error::{AppError, AppResult};
use crate::models::PhotoResult;
use serde::Deserialize;
use std::path::Path;

#[derive(Deserialize)]
struct SearchResponse {
    photos: Vec<Photo>,
}

#[derive(Deserialize)]
struct Photo {
    id: i64,
    url: String,
    photographer: String,
    photographer_url: String,
    src: Src,
    #[serde(default)]
    alt: String,
}

#[derive(Deserialize)]
struct Src {
    medium: String,
    large2x: String,
}

pub fn parse_search_response(json: &str) -> AppResult<Vec<PhotoResult>> {
    let resp: SearchResponse =
        serde_json::from_str(json).map_err(|e| AppError::Other(format!("parse pexels json: {e}")))?;
    Ok(resp
        .photos
        .into_iter()
        .map(|p| PhotoResult {
            id: p.id,
            source_url: p.url,
            author: p.photographer,
            author_url: p.photographer_url,
            thumb_url: p.src.medium,
            download_url: p.src.large2x,
            alt: p.alt,
        })
        .collect())
}

pub fn search(query: &str, key: &str) -> AppResult<Vec<PhotoResult>> {
    let client = reqwest::blocking::Client::new();
    let resp = client
        .get("https://api.pexels.com/v1/search")
        .header("Authorization", key)
        .query(&[("query", query), ("per_page", "24")])
        .send()
        .map_err(|e| AppError::Network(format!("pexels request failed: {e}")))?;
    if !resp.status().is_success() {
        return Err(AppError::Network(format!("pexels status {}", resp.status())));
    }
    let text = resp
        .text()
        .map_err(|e| AppError::Network(e.to_string()))?;
    parse_search_response(&text)
}

pub fn download(url: &str, dest: &Path) -> AppResult<()> {
    let client = reqwest::blocking::Client::new();
    let bytes = client
        .get(url)
        .send()
        .map_err(|e| AppError::Network(format!("download failed: {e}")))?
        .bytes()
        .map_err(|e| AppError::Network(e.to_string()))?;
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(dest, &bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "photos": [
            {
                "id": 1011,
                "url": "https://www.pexels.com/photo/snow-mountain-1011/",
                "photographer": "Jane Doe",
                "photographer_url": "https://www.pexels.com/@jane",
                "src": { "medium": "https://img/medium.jpg", "large2x": "https://img/large2x.jpg" },
                "alt": "snow mountain"
            }
        ]
    }"#;

    #[test]
    fn parses_photo_fields() {
        let v = parse_search_response(SAMPLE).unwrap();
        assert_eq!(v.len(), 1);
        let p = &v[0];
        assert_eq!(p.id, 1011);
        assert_eq!(p.source_url, "https://www.pexels.com/photo/snow-mountain-1011/");
        assert_eq!(p.author, "Jane Doe");
        assert_eq!(p.thumb_url, "https://img/medium.jpg");
        assert_eq!(p.download_url, "https://img/large2x.jpg");
        assert_eq!(p.alt, "snow mountain");
    }

    #[test]
    fn parses_empty_photos() {
        let v = parse_search_response(r#"{"photos":[]}"#).unwrap();
        assert!(v.is_empty());
    }

    #[test]
    fn invalid_json_errors() {
        assert!(parse_search_response("not json").is_err());
    }
}
```

- [ ] **Step 3: 运行测试**

Run: `"$USERPROFILE/.cargo/bin/cargo.exe" test --manifest-path src-tauri/Cargo.toml pexels::`
Expected: 3 测试 PASS。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/pexels.rs src-tauri/src/lib.rs
git commit -m "feat(m2): add pexels search/download with testable parser"
```

---

## Task 6: 背景表逻辑 `db/backgrounds.rs`

**Files:** Create `src-tauri/src/db/backgrounds.rs`; Modify `src-tauri/src/db/mod.rs`

- [ ] **Step 1: 声明模块**

在 `src-tauri/src/db/mod.rs` 顶部加入（保持字母序）：

```rust
pub mod backgrounds;
```

（最终顶部应为 `pub mod backgrounds; pub mod game; pub mod kv; pub mod migrations; pub mod todos;`）

- [ ] **Step 2: 实现 + 测试**

创建 `src-tauri/src/db/backgrounds.rs`：

```rust
use crate::error::{AppError, AppResult};
use crate::models::Background;
use rusqlite::{Connection, OptionalExtension, Row};

const COLS: &str = "id, local_path, source_url, author, license, keyword, is_current, created_at";

fn row_to_bg(row: &Row) -> rusqlite::Result<Background> {
    Ok(Background {
        id: row.get("id")?,
        local_path: row.get("local_path")?,
        source_url: row.get("source_url")?,
        author: row.get("author")?,
        license: row.get("license")?,
        keyword: row.get("keyword")?,
        is_current: row.get("is_current")?,
        created_at: row.get("created_at")?,
    })
}

pub fn insert(
    conn: &Connection,
    local_path: &str,
    source_url: &str,
    author: Option<&str>,
    license: Option<&str>,
    keyword: Option<&str>,
) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO backgrounds (local_path, source_url, author, license, keyword)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        (local_path, source_url, author, license, keyword),
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn set_current(conn: &mut Connection, id: i64) -> AppResult<()> {
    let tx = conn.transaction()?;
    tx.execute("UPDATE backgrounds SET is_current = 0", [])?;
    let n = tx.execute("UPDATE backgrounds SET is_current = 1 WHERE id = ?1", [id])?;
    if n == 0 {
        return Err(AppError::NotFound(format!("background {id}")));
    }
    tx.commit()?;
    Ok(())
}

pub fn get_current(conn: &Connection) -> AppResult<Option<Background>> {
    let sql = format!("SELECT {COLS} FROM backgrounds WHERE is_current = 1");
    Ok(conn.query_row(&sql, [], row_to_bg).optional()?)
}

pub fn restore_default(conn: &Connection) -> AppResult<()> {
    conn.execute("UPDATE backgrounds SET is_current = 0", [])?;
    Ok(())
}

#[allow(dead_code)]
pub fn list(conn: &Connection) -> AppResult<Vec<Background>> {
    let sql = format!("SELECT {COLS} FROM backgrounds ORDER BY created_at DESC");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], row_to_bg)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations;

    fn setup() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        migrations::apply(&mut conn).unwrap();
        conn
    }

    #[test]
    fn insert_get_current_set_current_unique() {
        let mut conn = setup();
        // nothing current yet
        assert!(get_current(&conn).unwrap().is_none());

        let a = insert(&conn, "/p/a.jpg", "http://src/a", Some("A"), Some("Pexels License"), Some("forest")).unwrap();
        let b = insert(&conn, "/p/b.jpg", "http://src/b", Some("B"), Some("Pexels License"), Some("lake")).unwrap();

        set_current(&mut conn, a).unwrap();
        assert_eq!(get_current(&conn).unwrap().unwrap().id, a);

        // switching current makes the old one non-current (uniqueness)
        set_current(&mut conn, b).unwrap();
        let cur = get_current(&conn).unwrap().unwrap();
        assert_eq!(cur.id, b);
        let current_count: i64 = conn
            .query_row("SELECT count(*) FROM backgrounds WHERE is_current = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(current_count, 1);
    }

    #[test]
    fn restore_default_clears_current() {
        let mut conn = setup();
        let a = insert(&conn, "/p/a.jpg", "http://src/a", None, None, None).unwrap();
        set_current(&mut conn, a).unwrap();
        restore_default(&conn).unwrap();
        assert!(get_current(&conn).unwrap().is_none());
    }

    #[test]
    fn set_current_missing_errors() {
        let mut conn = setup();
        assert!(set_current(&mut conn, 999).is_err());
    }
}
```

- [ ] **Step 3: 运行测试**

Run: `"$USERPROFILE/.cargo/bin/cargo.exe" test --manifest-path src-tauri/Cargo.toml backgrounds::`
Expected: 3 测试 PASS。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/db/backgrounds.rs src-tauri/src/db/mod.rs
git commit -m "feat(m2): add backgrounds table logic with unique current"
```

---

## Task 7: 命令层 + 注册 + 后端门禁

**Files:** Create `src-tauri/src/commands/backgrounds.rs`; Modify `src-tauri/src/commands/mod.rs`, `src-tauri/src/lib.rs`

- [ ] **Step 1: 命令实现**

创建 `src-tauri/src/commands/backgrounds.rs`：

```rust
use crate::config;
use crate::db::{backgrounds, Db};
use crate::error::{AppError, AppResult};
use crate::models::{CurrentBackground, PhotoResult};
use crate::pexels;
use base64::Engine;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State};

fn data_dir(app: &AppHandle) -> AppResult<PathBuf> {
    app.path()
        .app_data_dir()
        .map_err(|e| AppError::Io(e.to_string()))
}

#[tauri::command]
pub fn config_has_key(app: AppHandle) -> AppResult<bool> {
    let cfg = config::load(&data_dir(&app)?)?;
    Ok(cfg
        .pexels_api_key
        .as_deref()
        .map(|k| !k.is_empty())
        .unwrap_or(false))
}

#[tauri::command]
pub fn config_set_pexels_key(app: AppHandle, key: String) -> AppResult<()> {
    let dir = data_dir(&app)?;
    let mut cfg = config::load(&dir)?;
    cfg.pexels_api_key = Some(key);
    config::save(&dir, &cfg)
}

#[tauri::command]
pub async fn bg_search(app: AppHandle, keyword: String) -> AppResult<Vec<PhotoResult>> {
    let cfg = config::load(&data_dir(&app)?)?;
    let key = cfg
        .pexels_api_key
        .filter(|k| !k.is_empty())
        .ok_or_else(|| AppError::Other("Pexels API key not set".into()))?;
    tauri::async_runtime::spawn_blocking(move || pexels::search(&keyword, &key))
        .await
        .map_err(|e| AppError::Other(e.to_string()))?
}

#[tauri::command]
pub async fn bg_download_and_set(
    app: AppHandle,
    db: State<'_, Db>,
    photo: PhotoResult,
    keyword: String,
) -> AppResult<()> {
    let dest = data_dir(&app)?
        .join("backgrounds")
        .join(format!("{}.jpg", photo.id));
    let url = photo.download_url.clone();
    let dest_for_dl = dest.clone();
    tauri::async_runtime::spawn_blocking(move || pexels::download(&url, &dest_for_dl))
        .await
        .map_err(|e| AppError::Other(e.to_string()))??;

    let mut conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
    let id = backgrounds::insert(
        &conn,
        &dest.to_string_lossy(),
        &photo.source_url,
        Some(&photo.author),
        Some("Pexels License"),
        Some(&keyword),
    )?;
    backgrounds::set_current(&mut conn, id)
}

#[tauri::command]
pub fn bg_get_current(app: AppHandle, db: State<Db>) -> AppResult<Option<CurrentBackground>> {
    let _ = app;
    let conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
    let Some(bg) = backgrounds::get_current(&conn)? else {
        return Ok(None);
    };
    let bytes = std::fs::read(&bg.local_path)?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(Some(CurrentBackground {
        data_url: format!("data:image/jpeg;base64,{b64}"),
        source_url: bg.source_url,
        author: bg.author,
    }))
}

#[tauri::command]
pub fn bg_restore_default(db: State<Db>) -> AppResult<()> {
    let conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
    backgrounds::restore_default(&conn)
}
```

> `bg_get_current` 不需要 `app`，但保留参数与前端调用形态一致；用 `let _ = app;` 消除未使用警告。

把 `src-tauri/src/commands/mod.rs` 整个替换为：

```rust
pub mod backgrounds;
pub mod game;
pub mod kv;
pub mod todos;
```

- [ ] **Step 2: 注册命令**

把 `src-tauri/src/lib.rs` 的 `invoke_handler(tauri::generate_handler![...])` 整段替换为：

```rust
        .invoke_handler(tauri::generate_handler![
            commands::kv::kv_get,
            commands::kv::kv_set,
            commands::todos::todo_create,
            commands::todos::todo_update,
            commands::todos::todo_delete,
            commands::todos::todo_list_today,
            commands::todos::todo_toggle_done,
            commands::game::game_get_profile,
            commands::backgrounds::config_has_key,
            commands::backgrounds::config_set_pexels_key,
            commands::backgrounds::bg_search,
            commands::backgrounds::bg_download_and_set,
            commands::backgrounds::bg_get_current,
            commands::backgrounds::bg_restore_default
        ])
```

- [ ] **Step 3: 全量测试 + lint**

Run:
```
"$USERPROFILE/.cargo/bin/cargo.exe" test --manifest-path src-tauri/Cargo.toml
"$USERPROFILE/.cargo/bin/cargo.exe" clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
```
Expected: 全部测试 PASS（M0/M1 的 12 + M2 的 8 = 20）；clippy 无警告。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/commands/ src-tauri/src/lib.rs
git commit -m "feat(m2): add background/config commands and register them"
```

---

## Task 8: 前端 API 封装

**Files:** Modify `src/lib/api/index.ts`

- [ ] **Step 1: 增加类型与封装**

在 `src/lib/api/index.ts` 末尾追加：

```ts
export interface PhotoResult {
  id: number;
  source_url: string;
  author: string;
  author_url: string;
  thumb_url: string;
  download_url: string;
  alt: string;
}

export interface CurrentBackground {
  data_url: string;
  source_url: string;
  author: string | null;
}

export function configHasKey(): Promise<boolean> {
  return call<boolean>("config_has_key");
}

export function configSetPexelsKey(key: string): Promise<void> {
  return call<void>("config_set_pexels_key", { key });
}

export function bgSearch(keyword: string): Promise<PhotoResult[]> {
  return call<PhotoResult[]>("bg_search", { keyword });
}

export function bgDownloadAndSet(photo: PhotoResult, keyword: string): Promise<void> {
  return call<void>("bg_download_and_set", { photo, keyword });
}

export function bgGetCurrent(): Promise<CurrentBackground | null> {
  return call<CurrentBackground | null>("bg_get_current");
}

export function bgRestoreDefault(): Promise<void> {
  return call<void>("bg_restore_default");
}
```

- [ ] **Step 2: 类型检查**

Run: `npm run check`
Expected: 0 errors。

- [ ] **Step 3: 提交**

```bash
git add src/lib/api/index.ts
git commit -m "feat(m2): add background/config typed api wrappers"
```

---

## Task 9: 背景 store + 布局重构（导航 + 背景层）

**Files:** Create `src/lib/stores/background.ts`; Modify `src/app.css`, `src/routes/+layout.svelte`, `src/routes/+page.svelte`

- [ ] **Step 1: 背景 store**

创建 `src/lib/stores/background.ts`：

```ts
import { writable } from "svelte/store";
import { bgGetCurrent, bgRestoreDefault, type CurrentBackground } from "$lib/api";

export const currentBg = writable<CurrentBackground | null>(null);

export async function loadBackground(): Promise<void> {
  currentBg.set(await bgGetCurrent());
}

export async function clearBackground(): Promise<void> {
  await bgRestoreDefault();
  await loadBackground();
}
```

- [ ] **Step 2: app.css 增加背景层**

在 `src/app.css` 末尾追加：

```css
.bg-layer {
  position: fixed;
  inset: 0;
  z-index: -1;
  background-size: cover;
  background-position: center;
}

.bg-layer::after {
  content: "";
  position: absolute;
  inset: 0;
  background: var(--bg);
  opacity: 0.55;
}
```

- [ ] **Step 3: 布局加顶栏 + 背景层**

把 `src/routes/+layout.svelte` 整个替换为：

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import "../app.css";
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
```

- [ ] **Step 4: 待办页去掉自带顶栏**

把 `src/routes/+page.svelte` 整个替换为（移除原 `<header class="bar">` 与 theme 相关导入；保留待办逻辑，金币用共享 store 设置）：

```svelte
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
```

- [ ] **Step 5: 类型检查**

Run: `npm run check`
Expected: 0 errors。

- [ ] **Step 6: 提交**

```bash
git add src/lib/stores/background.ts src/app.css src/routes/+layout.svelte src/routes/+page.svelte
git commit -m "feat(m2): move nav/coins/theme to layout, add background layer"
```

---

## Task 10: 背景页（设置 + 搜索 + 网格）

**Files:** Create `src/routes/backgrounds/+page.svelte`

- [ ] **Step 1: 实现页面**

创建 `src/routes/backgrounds/+page.svelte`：

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import {
    configHasKey,
    configSetPexelsKey,
    bgSearch,
    bgDownloadAndSet,
    type PhotoResult,
  } from "$lib/api";
  import { loadBackground, clearBackground } from "$lib/stores/background";

  const presets = ["森林", "雪山", "湖泊", "海边", "星空"];

  let hasKey = $state(false);
  let keyInput = $state("");
  let keyword = $state("");
  let results = $state<PhotoResult[]>([]);
  let busy = $state(false);
  let message = $state("");

  onMount(async () => {
    hasKey = await configHasKey();
  });

  async function saveKey() {
    const k = keyInput.trim();
    if (!k) return;
    await configSetPexelsKey(k);
    keyInput = "";
    hasKey = true;
    message = "已保存 Key / Key saved";
  }

  async function runSearch(q: string) {
    keyword = q;
    const term = q.trim();
    if (!term) return;
    busy = true;
    message = "";
    try {
      results = await bgSearch(term);
      if (results.length === 0) message = "没有结果 / No results";
    } catch (e) {
      message = `搜索失败 / Search failed: ${e}`;
    } finally {
      busy = false;
    }
  }

  async function pick(photo: PhotoResult) {
    busy = true;
    message = "";
    try {
      await bgDownloadAndSet(photo, keyword);
      await loadBackground();
      message = "已设为背景 / Set as background";
    } catch (e) {
      message = `设置失败 / Failed: ${e}`;
    } finally {
      busy = false;
    }
  }

  async function restore() {
    await clearBackground();
    message = "已恢复默认 / Restored default";
  }
</script>

<main class="container">
  <h1>背景图片 / Backgrounds</h1>

  {#if !hasKey}
    <section class="card">
      <p>请先填入 Pexels API Key（保存在本地，不上传）。</p>
      <p>Enter your Pexels API key (stored locally).</p>
      <div class="row">
        <input placeholder="Pexels API Key" bind:value={keyInput} />
        <button onclick={saveKey}>保存 / Save</button>
      </div>
    </section>
  {/if}

  <section class="search">
    <div class="row">
      <input
        placeholder="关键词 / Keyword..."
        bind:value={keyword}
        onkeydown={(e) => e.key === "Enter" && runSearch(keyword)}
      />
      <button onclick={() => runSearch(keyword)} disabled={busy}>搜索 / Search</button>
      <button class="ghost" onclick={restore}>恢复默认 / Restore</button>
    </div>
    <div class="presets">
      {#each presets as p}
        <button class="chip" onclick={() => runSearch(p)} disabled={busy}>{p}</button>
      {/each}
    </div>
  </section>

  {#if message}
    <p class="msg">{message}</p>
  {/if}

  <div class="grid">
    {#each results as photo (photo.id)}
      <button class="thumb" onclick={() => pick(photo)} disabled={busy} title={photo.alt}>
        <img src={photo.thumb_url} alt={photo.alt} loading="lazy" />
      </button>
    {/each}
  </div>
</main>

<style>
  .container {
    max-width: 800px;
    margin: 0 auto;
    padding: 1.5rem 1rem;
  }

  .card,
  .search {
    margin-bottom: 1rem;
  }

  .row {
    display: flex;
    gap: 0.5rem;
  }

  .row input {
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
    cursor: pointer;
  }

  .presets {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
    margin-top: 0.5rem;
  }

  .chip {
    border-radius: 999px;
    padding: 0.3em 0.9em;
  }

  .ghost {
    border-color: transparent;
    background: transparent;
  }

  .msg {
    opacity: 0.8;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 0.6rem;
  }

  .thumb {
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 8px;
    overflow: hidden;
    aspect-ratio: 4 / 3;
  }

  .thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
</style>
```

- [ ] **Step 2: 类型检查**

Run: `npm run check`
Expected: 0 errors。

- [ ] **Step 3: 提交**

```bash
git add src/routes/backgrounds/+page.svelte
git commit -m "feat(m2): add backgrounds page (key settings, search, grid)"
```

---

## Task 11: 端到端验收

**Files:** Modify `开发计划.md`

- [ ] **Step 1: 启动应用**

Run: `npm run tauri dev`
Expected: 顶栏有「待办 / 背景」导航、金币、主题；待办页正常。

- [ ] **Step 2: 功能验收（需真实 Pexels key）**

1. 进「背景」页 → 填入 Pexels key → 保存。
2. 点预置词「雪山」或输入关键词 → 出现缩略图网格。
3. 点一张 → 片刻后窗口背景变为该图、文字仍可读。
4. 切到「待办」页 → 背景一致。
5. 关闭应用 → 重新 `npm run tauri dev` → 背景保持。
6. 回「背景」页 → 「恢复默认」→ 背景回到主题底色。
7. 确认 `%APPDATA%\com.deskhub.app\backgrounds\` 有下载的图；DB `backgrounds.source_url` 有值。

- [ ] **Step 3: 勾选开发计划 M2**

在 `开发计划.md` 的 M2 小节，把已完成项 `- [ ]` 改为 `- [x]`（图片来源 API=Pexels、reqwest 搜索+下载、强制记录来源、设为/恢复背景、key 注入；扩展点保留未做）。

- [ ] **Step 4: 提交**

```bash
git add 开发计划.md
git commit -m "docs(m2): mark M2 backgrounds milestone complete"
```

---

## 自检 / Self-Review

- **Spec 覆盖：** 迁移/表(Task1) / Network 错误(Task2) / 依赖+模型(Task3) / config(Task4) / pexels 搜索下载+解析(Task5) / backgrounds 表(Task6) / 命令+注册(Task7) / 前端 api(Task8) / store+布局+背景层(Task9) / 背景页 UI(Task10) / 验收(Task11) —— 均有任务。
- **无占位符：** 所有步骤含完整代码与确切命令；网络部分明确留手动验收。
- **类型一致：** `PhotoResult`/`Background`/`CurrentBackground` 在 Rust(Task3) 与 TS(Task8) 字段一致；命令名 `config_*`/`bg_*` 在 Task7 注册、Task8 调用一致；`set_current(&mut Connection,id)`、`get_current(&Connection)`、`parse_search_response`、data URL 形态 `data:image/jpeg;base64,` 一致；JS `bgDownloadAndSet(photo, keyword)` ↔ Rust `bg_download_and_set(photo, keyword)`。
