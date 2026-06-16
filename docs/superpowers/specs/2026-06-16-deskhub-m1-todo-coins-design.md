# DeskHub M1 — Todo + 金币系统 设计 (Spec)

> 日期：2026-06-16
> 范围：[开发计划.md](../../../开发计划.md) 的 **M1 — Todo + 金币系统（MVP 核心）**
> 前置：M0 已完成（rusqlite + `PRAGMA user_version` 迁移框架、`AppError`/`AppResult` 约定、命令模式、前端 `lib/api` + `lib/stores`、`kv` 表）。
> 本里程碑复用 M0 基建，新增 todo 与金币经济。

---

## 一、目标与验收

**目标：** 完成 [项目.md](../../../项目.md) 4.1 与第八章中的 Todo 与金币奖励闭环。

**验收标准：**
1. 新建任务 → 勾选完成 → 金币 +10。
2. 重启应用后任务与金币余额保留。
3. 重复勾选不重复发奖（取消完成不退币，再次完成不再发币）。
4. 今日列表 = 全部未完成 + 今天完成的任务。
5. `cargo test` 全过；`cargo clippy -D warnings` 干净；`npm run check` 0 errors。

**关键产品决策（已确认）：**
- **今日任务范围：** 全部未完成 + 今天完成的（`due_date` 退化为展示/排序字段，不参与过滤）。
- **取消完成语义：** 不退币、再次完成不重发（金币赚到即保留，靠 `coin_ledger` 的 `ref_id` 去重）。

---

## 二、数据模型（迁移 0002）

时间戳统一用 `datetime('now','localtime')`（本地单用户应用，"今天"按本地时区判断）。

```sql
CREATE TABLE todos (
  id          INTEGER PRIMARY KEY,
  title       TEXT NOT NULL,
  note        TEXT,
  done        INTEGER NOT NULL DEFAULT 0,
  due_date    TEXT,                       -- ISO8601 日期，仅展示/排序
  reward_coin INTEGER NOT NULL DEFAULT 10,
  created_at  TEXT NOT NULL DEFAULT (datetime('now','localtime')),
  done_at     TEXT
);

CREATE TABLE game_profile (
  id        INTEGER PRIMARY KEY CHECK (id = 1),
  coins     INTEGER NOT NULL DEFAULT 0,
  exp       INTEGER NOT NULL DEFAULT 0,   -- M6 使用
  level     INTEGER NOT NULL DEFAULT 1,   -- M6 使用
  last_tick TEXT NOT NULL DEFAULT (datetime('now','localtime'))  -- M6 使用
);

CREATE TABLE coin_ledger (
  id         INTEGER PRIMARY KEY,
  amount     INTEGER NOT NULL,
  reason     TEXT NOT NULL,               -- M1 仅 'todo_done'
  ref_id     INTEGER,                     -- 关联 todos.id
  created_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
);
```

---

## 三、后端模块（扩展 M0 的 `db/`、`commands/`）

所有 SQL 仍只在 `db/`。

- `db/todos.rs`：`create / update / delete / toggle_done / list_today`。
- `db/game.rs`：金币经济聚合一处（余额与流水一起变更）——
  - `ensure_profile(conn)`：`INSERT OR IGNORE INTO game_profile(id) VALUES (1)`，保证单行存在。
  - `get_profile(conn) -> GameProfile`。
  - `award_for_todo(conn_tx, todo_id, amount) -> i64`：在调用方事务内，若 `coin_ledger` 无 `(reason='todo_done', ref_id=todo_id)` 则插一笔正流水并 `coins += amount`，返回实发币（无则 0）。
- `models/mod.rs`：`Todo`、`GameProfile`（`serde::Serialize`）；`ToggleResult { todo, awarded, coins }`。
- `db/migrations.rs`：新增迁移 `(2, "...")`。

单元职责：
- `db/todos.rs` 拥有任务 CRUD 与状态切换的 SQL。
- `db/game.rs` 拥有金币经济不变式（发奖去重 + 余额更新 + 审计流水）。
- `commands/*` 仅转发与错误传播，不含 SQL。

---

## 四、命令（均返回 `AppResult<T>`）

- `todo_create(title: String, note: Option<String>, due_date: Option<String>) -> Todo`
- `todo_update(id: i64, title: String, note: Option<String>, due_date: Option<String>) -> Todo`
- `todo_delete(id: i64) -> ()`
- `todo_list_today() -> Vec<Todo>`
- `todo_toggle_done(id: i64) -> ToggleResult`
- `game_get_profile() -> GameProfile`

`ToggleResult` 字段：`todo: Todo`（切换后的任务）、`awarded: i64`（本次实发金币，0 表示未发）、`coins: i64`（切换后总余额）。

---

## 五、金币核心不变式（单事务）

`todo_toggle_done(id)` 在**一个事务**内：

1. 读取 `todos` 行（不存在 → `AppError::NotFound`）。
2. 若当前 `done = 0`（未完成 → 完成）：
   - `UPDATE todos SET done=1, done_at=datetime('now','localtime') WHERE id=?`。
   - `awarded = game::award_for_todo(tx, id, reward_coin)`（内部按 `ref_id` 去重）。
3. 若当前 `done = 1`（完成 → 未完成）：
   - `UPDATE todos SET done=0, done_at=NULL WHERE id=?`。
   - `awarded = 0`，不写流水、不改余额。
4. 提交事务；返回 `ToggleResult { todo(刷新后), awarded, coins(刷新后) }`。

`coin_ledger` 既支撑去重，也作为审计日志。

---

## 六、今日列表查询

```sql
SELECT id, title, note, done, due_date, reward_coin, created_at, done_at
FROM todos
WHERE done = 0
   OR (done = 1 AND date(done_at) = date('now','localtime'))
ORDER BY done ASC, created_at DESC;
```

未完成在前、按创建时间倒序；今天完成的排在后面。

---

## 七、前端

- `lib/api/index.ts`：新增 `todoCreate / todoUpdate / todoDelete / todoListToday / todoToggleDone / gameGetProfile` 类型化封装；定义 TS 类型 `Todo`、`GameProfile`、`ToggleResult`。
- `lib/stores/todos.ts`：`writable<Todo[]>`；`load()` 调 `todoListToday`；增删改/切换后重载列表。
- `lib/stores/game.ts`：`writable<number>`（coins）；`refresh()` 调 `gameGetProfile`；`toggle` 返回值直接更新余额。
- 页面 `routes/+page.svelte`：
  - 顶部：金币余额 + 主题切换按钮（移到顶角）。
  - 新建任务输入框（标题，回车/按钮提交）。
  - 任务列表：复选框完成 / 标题 / 编辑 / 删除。
  - 完成发币时显示极简 `+N🪙` 浮字反馈。

---

## 八、测试与门禁

**Rust 单元测试（内存库，TDD）：**
- `db/todos`：create 后 list 可见；update 改字段；delete 移除；list_today 含未完成 + 今天完成、排除往日完成。
- `db/game`：`ensure_profile` 幂等、初始 coins=0；`award_for_todo` 首次发币使余额 += amount，重复调用（同 ref_id）不再加、返回 0。
- `toggle_done`：未完成→完成发一次币；完成→未完成不退币；再→完成不重发（ledger 去重）。

**门禁：** `cargo test`、`cargo clippy -- -D warnings`、`npm run check`。
**手动验收：** 新建 → 完成（金币 +10、浮字）→ 重启（任务/金币保留）→ 取消完成（币不减）→ 再完成（币不增）。

---

## 九、不在 M1 范围（YAGNI）

- exp / 等级 / 自动产出 / 离线收益 → M6（`game_profile` 相关字段存在但不读写）。
- `reward_coin` 固定 10，无自定义 UI（列已预留）。
- 奖励反馈仅 `+N🪙` 浮字，无复杂动画。
- 邮件/背景等其它模块 → 各自里程碑。
