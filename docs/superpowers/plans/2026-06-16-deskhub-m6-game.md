# DeskHub M6 挂机游戏深化 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans. Steps use checkbox (`- [ ]`).

**Goal:** 自动产出 + 经验/等级 + 离线收益（8h 封顶，启动弹提示）+ 统一 Todo/邮件行为奖励，复用 `game_profile`/`coin_ledger`/kv，无 migration。

**Architecture:** 纯函数（速率/等级/收益）单测；`settle_idle` 用 julianday 计算时长入账；60s 后台线程 + 启动离线结算；前端游戏面板 + 离线提示 + 轮询。

**Tech Stack:** Rust + rusqlite、SvelteKit + TS。

**参考 spec：** `docs/superpowers/specs/2026-06-16-deskhub-m6-game-design.md`

**质量门禁：** `src-tauri/` 下 `cargo test`、`cargo clippy -- -D warnings`；前端 `npm run check`。
> npm 经 nvm：PowerShell 先 `$env:Path = "C:\nvm4w\nodejs;" + $env:Path`。

**分工：** 后端（Task 1–4）由主代理实现并编译/单测验证；前端（Task 5）交 sub-agent。

---

## Task 1: game.rs 纯函数 + settle_idle + 经验/奖励（含单测）

**Files:** `src-tauri/src/db/game.rs`、`src-tauri/src/models/mod.rs`

- [ ] **Step 1: models 加 GameStatus**

```rust
#[derive(Debug, Serialize)]
pub struct GameStatus {
    pub coins: i64,
    pub exp: i64,
    pub level: i64,
    pub exp_into_level: i64,
    pub exp_for_next: i64,
    pub rate_per_min: i64,
}
```

- [ ] **Step 2: game.rs 加纯函数 + 等级/收益逻辑**

```rust
pub fn rate_per_min(level: i64) -> i64 { level.max(1) }

/// cumulative exp required to *reach* level L = 100*(L-1)*L/2
fn cumulative(level: i64) -> i64 { 100 * (level - 1) * level / 2 }

pub fn level_for_exp(exp: i64) -> i64 {
    let mut l = 1;
    while cumulative(l + 1) <= exp { l += 1; }
    l
}

pub fn compute_earned(elapsed_secs: i64, level: i64, cap_secs: i64) -> i64 {
    let s = elapsed_secs.clamp(0, cap_secs);
    (s / 60) * rate_per_min(level)
}
```

- [ ] **Step 3: settle_idle + add_exp + award_for_mail + 扩展 award_for_todo**

```rust
pub const OFFLINE_CAP_SECS: i64 = 8 * 3600;

/// Settle idle production since last_tick (capped). Returns coins earned.
pub fn settle_idle(conn: &Connection, cap_secs: i64) -> AppResult<i64> {
    ensure_profile(conn)?;
    let (level, elapsed): (i64, i64) = conn.query_row(
        "SELECT level, CAST((julianday('now','localtime') - julianday(last_tick)) * 86400 AS INTEGER)
         FROM game_profile WHERE id = 1",
        [], |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let earned = compute_earned(elapsed.max(0), level, cap_secs);
    if earned > 0 {
        conn.execute("INSERT INTO coin_ledger (amount, reason) VALUES (?1, 'idle')", [earned])?;
        conn.execute(
            "UPDATE game_profile SET coins = coins + ?1, last_tick = datetime('now','localtime') WHERE id = 1",
            [earned],
        )?;
    }
    Ok(earned)
}

/// Add exp and recompute level. Call inside caller's transaction.
fn add_exp(conn: &Connection, amount: i64) -> AppResult<()> {
    conn.execute("UPDATE game_profile SET exp = exp + ?1 WHERE id = 1", [amount])?;
    let exp: i64 = conn.query_row("SELECT exp FROM game_profile WHERE id = 1", [], |r| r.get(0))?;
    conn.execute("UPDATE game_profile SET level = ?1 WHERE id = 1", [level_for_exp(exp)])?;
    Ok(())
}

/// Reward first read of an email (dedup via kv). Returns coins awarded (0 if already).
pub fn award_for_mail(conn: &Connection, msg_id: &str, coins: i64, exp: i64) -> AppResult<i64> {
    ensure_profile(conn)?;
    let key = format!("reward:mail:{msg_id}");
    if crate::db::kv::get(conn, &key)?.is_some() {
        return Ok(0);
    }
    conn.execute("INSERT INTO coin_ledger (amount, reason) VALUES (?1, 'mail_read')", [coins])?;
    conn.execute("UPDATE game_profile SET coins = coins + ?1 WHERE id = 1", [coins])?;
    add_exp(conn, exp)?;
    crate::db::kv::set(conn, &key, "1")?;
    Ok(coins)
}
```

并把现有 `award_for_todo` 在发金币后加一行经验（在插入 ledger + 更新 coins 之后、`Ok(amount)` 之前）：

```rust
    add_exp(conn, 5)?;
```

- [ ] **Step 4: 单测**（加入 game.rs `mod tests`）

```rust
    #[test]
    fn level_curve() {
        assert_eq!(level_for_exp(0), 1);
        assert_eq!(level_for_exp(99), 1);
        assert_eq!(level_for_exp(100), 2);
        assert_eq!(level_for_exp(299), 2);
        assert_eq!(level_for_exp(300), 3);
    }
    #[test]
    fn earned_floors_minutes_and_caps() {
        assert_eq!(compute_earned(59, 1, 28800), 0);
        assert_eq!(compute_earned(60, 1, 28800), 1);
        assert_eq!(compute_earned(600, 3, 28800), 30); // 10min * 3
        assert_eq!(compute_earned(100000, 1, 28800), 480); // capped 8h
    }
    #[test]
    fn settle_idle_credits_capped() {
        let conn = setup();
        // backdate last_tick by 2 hours, level 1 -> 120 coins
        conn.execute("UPDATE game_profile SET last_tick = datetime('now','localtime','-2 hours') WHERE id=1", []).unwrap();
        let earned = settle_idle(&conn, OFFLINE_CAP_SECS).unwrap();
        assert_eq!(earned, 120);
        assert_eq!(get_profile(&conn).unwrap().coins, 120);
    }
    #[test]
    fn mail_reward_dedups() {
        let conn = setup();
        assert_eq!(award_for_mail(&conn, "abc", 2, 3).unwrap(), 2);
        assert_eq!(award_for_mail(&conn, "abc", 2, 3).unwrap(), 0);
        assert_eq!(get_profile(&conn).unwrap().coins, 2);
        assert_eq!(get_profile(&conn).unwrap().exp, 3);
    }
    #[test]
    fn todo_award_grants_exp() {
        let conn = setup();
        award_for_todo(&conn, 1, 10).unwrap();
        assert_eq!(get_profile(&conn).unwrap().exp, 5);
    }
```

- [ ] **Step 5:** `cargo test game`；commit `feat(m6): idle settle, exp/level, mail reward (game.rs) + tests`.

---

## Task 2: idle 线程 + 启动离线结算 + 命令

**Files:** `src-tauri/src/idle.rs`(新)、`src-tauri/src/commands/game.rs`、`src-tauri/src/lib.rs`

- [ ] **Step 1: `idle.rs`**（仿 reminder.rs）

```rust
use crate::db::{game, Db};
use std::time::Duration;
use tauri::{AppHandle, Manager};

pub fn spawn_loop(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(60));
        let state = app.state::<Db>();
        if let Ok(conn) = state.0.lock() {
            let _ = game::settle_idle(&conn, game::OFFLINE_CAP_SECS);
        }
    });
}
```

- [ ] **Step 2: commands/game.rs 加命令**

```rust
#[tauri::command]
pub fn game_status(db: State<Db>) -> AppResult<GameStatus> {
    let conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
    let _ = game::settle_idle(&conn, game::OFFLINE_CAP_SECS);
    let p = game::get_profile(&conn)?;
    let cum = 100 * (p.level - 1) * p.level / 2;
    Ok(GameStatus {
        coins: p.coins, exp: p.exp, level: p.level,
        exp_into_level: p.exp - cum,
        exp_for_next: 100 * p.level,
        rate_per_min: game::rate_per_min(p.level),
    })
}

#[tauri::command]
pub fn game_take_offline_earned(db: State<Db>) -> AppResult<i64> {
    let conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
    let v = crate::db::kv::get(&conn, "idle.offline_earned")?
        .and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
    crate::db::kv::set(&conn, "idle.offline_earned", "0")?;
    Ok(v)
}
```
（实现者：补 imports；`game_status` 的 cumulative 逻辑与 spec 一致。）

- [ ] **Step 3: lib.rs**：`mod idle;`；setup 中 db 就绪后做启动离线结算并写 kv：

```rust
            {
                let state = app.state::<db::Db>();
                let conn = state.0.lock().map_err(|e| e.to_string())?;
                if let Ok(earned) = db::game::settle_idle(&conn, db::game::OFFLINE_CAP_SECS) {
                    if earned > 0 {
                        let prev = db::kv::get(&conn, "idle.offline_earned").ok().flatten()
                            .and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
                        let _ = db::kv::set(&conn, "idle.offline_earned", &(prev + earned).to_string());
                    }
                }
            }
```
spawn 线程：`idle::spawn_loop(app.handle().clone());`（在 reminder spawn 附近）。注册命令 `game_status`、`game_take_offline_earned`。

- [ ] **Step 4:** `cargo test; cargo clippy -- -D warnings`；commit `feat(m6): idle loop, startup offline settle, game commands`.

---

## Task 3: 邮件首读奖励接线

**Files:** `src-tauri/src/commands/mail.rs`

- [ ] **Step 1:** 在 `mail_mark_read` 中，`api::mark_read` 成功且 `read==true` 后，对该 id 发奖励：

```rust
#[tauri::command]
pub async fn mail_mark_read(app: AppHandle, db: State<'_, Db>, id: String, read: bool) -> AppResult<()> {
    let id2 = id.clone();
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || api::mark_read(&app2, &id2, read))
        .await
        .map_err(|e| AppError::Other(e.to_string()))??;
    if read {
        let conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
        let _ = crate::db::game::award_for_mail(&conn, &id, 2, 3);
    }
    Ok(())
}
```
（实现者：给命令加 `db: State<'_, Db>` 参数；确保 `mark_read` 命令在 lib.rs 注册不变。）

- [ ] **Step 2:** `cargo clippy -- -D warnings`；commit `feat(m6): award coins+exp on first email read`.

---

## Task 4: 前端 API + store（sub-agent 前置）

**Files:** `src/lib/api/index.ts`、`src/lib/stores/game.ts`

- [ ] **Step 1: api 封装**

```ts
export interface GameStatus {
  coins: number; exp: number; level: number;
  exp_into_level: number; exp_for_next: number; rate_per_min: number;
}
export function gameStatus(): Promise<GameStatus> { return call<GameStatus>("game_status"); }
export function gameTakeOfflineEarned(): Promise<number> { return call<number>("game_take_offline_earned"); }
```

- [ ] **Step 2: stores/game.ts** 扩展：保留 `coins` store；加 `gameState = writable<GameStatus|null>(null)` 与 `refreshStatus()`（调 `gameStatus()`，同时把 `coins` set 为最新）。

---

## Task 5: 前端展示（sub-agent）

**Files:** `src/routes/(app)/+page.svelte`、`src/routes/(widget)/widgets/coins/+page.svelte`

- [ ] **Step 1: 主页游戏面板** —— 在 `+page.svelte` 顶部（`<h1>DeskHub</h1>` 下）加一块：
  - `onMount`：`refreshStatus()`；`gameTakeOfflineEarned()` >0 → 显示可关闭提示「🎁 离线获得 {n} 金币 / Earned {n} coins while away」。
  - 显示 `Lv {level}`、经验条（宽度 = `exp_into_level/exp_for_next*100%`）、`⚙ {rate_per_min}/min`、`🪙 {coins}`。
  - `setInterval` 每 60s `refreshStatus()`（`onDestroy` 清除）。
  - 样式与现有页面一致（CSS 变量、圆角卡片）。
- [ ] **Step 2: 金币 widget** —— `widgets/coins/+page.svelte` 改为显示 `🪙 {coins}　Lv {level}`：`onMount` + 每 60s `refreshStatus()`，从 `gameState` 读 level、`coins` 读金币（保持透明卡 + drag-region）。
- [ ] **Step 3:** `npm run check`（0 errors）；commit `feat(m6): game panel, offline popup, level on coins widget`.

---

## Task 6: 全量验证

- [ ] `cargo test`（含 game 新测试）、`cargo clippy -- -D warnings`、`npm run check` 全绿。
- [ ] 手动验证清单：
  - 完成 todo → 经验涨、可能升级；金币照常
  - 挂着一会儿 → 金币按 level/min 自增（面板每分钟刷新）
  - 关应用等几分钟再开 → 弹「离线获得 N 金币」（≤8h 封顶）
  - 读一封未读邮件 → 金币 +2、经验 +3（再读同封不重复）
  - 金币 widget 显示 `Lv L`

---

## 自检 / Self-Review 结论

- **Spec 覆盖：** 自动产出(settle_idle/idle 线程)、经验等级(level_for_exp/add_exp)、离线收益(启动结算+kv+弹窗)、行为奖励统一(todo+exp、mail reward)、展示(面板+widget) —— 全覆盖。
- **占位符：** 后端给完整代码 + 单测；前端给结构 + 关键逻辑（sub-agent 按现有页面风格补全）。
- **类型一致：** `GameStatus` 字段后端/前端一致；`settle_idle`/`award_for_mail`/`rate_per_min` 跨 db/commands/idle/lib 引用一致；reason 常量 `idle`/`mail_read`/`todo_done` 一致。
- **无 migration：** 复用 game_profile/coin_ledger/kv，确认无表结构变更。
