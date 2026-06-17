# DeskHub M6 挂机游戏深化 — 设计文档

> 日期：2026-06-16
> 关联 issue：[#3](https://github.com/Nyxicemoon/Desktop_Widget/issues/3)
> 范围：完成 [项目.md](../../../项目.md) 4.5「挂机小游戏」完整玩法。

---

## 一、目标与范围

把已有的金币系统深化为一个轻量挂机游戏：自动产出、经验/等级、离线收益，并统一 Todo/邮件的行为奖励。

**本轮做：**
1. **自动产出**：按等级被动产出金币（运行时定时结算 + 启动时离线结算）。
2. **经验/等级**：行为驱动获得经验，等级提升产出速率。
3. **离线收益**：启动按 `now - last_tick` 结算，**上限 8 小时**，启动弹提示。
4. **行为奖励统一**：完成 Todo +经验；首次阅读邮件 +金币+经验（补 M5 留项），均走 `coin_ledger`。
5. **展示**：主页游戏面板（等级/经验条/速率/金币）+ 金币 widget 显示等级。

**非目标：**
- 复杂玩法（建筑/升级树/多资源）—— 仅单一金币产出 + 等级。
- 手动点击刷金币 —— 纯被动（挂机）。
- 新建表 / migration —— 复用 `game_profile` + `coin_ledger` + kv。

---

## 二、关键决策（已与用户确认）

| 决策点 | 选择 |
| -- | -- |
| 经验来源 | **行为驱动**：完成 todo + 阅读邮件 |
| 离线收益上限 | **8 小时** |
| 离线收益告知 | **启动弹提示**（前端 banner/modal） |

---

## 三、机制

### 自动产出
- 产出速率：`rate_per_min(level) = level`（等级 1 = 1 金币/分钟，可调）。
- **结算函数** `settle_idle(now)`：
  - `elapsed_secs = now - last_tick`（用 `julianday` 在 SQL 计算，或读 last_tick 字符串解析）。
  - `capped = min(elapsed_secs, 8h=28800)`。
  - `earned = (capped / 60) * rate_per_min(level)`（整数分钟）。
  - 若 `earned > 0`：写 `coin_ledger(amount=earned, reason='idle')`，`game_profile.coins += earned`，`last_tick = now`。
  - 返回 `earned`。
  - 若 `earned == 0`（不足 1 分钟）：不更新 last_tick（避免零头丢失）。
- **运行时**：后台线程每 60 秒调 `settle_idle`（持续累积，反映到 UI）。
- **启动时**：setup 中调一次 `settle_idle` 结算离线收益，结果写 kv `idle.offline_earned`（累加，供前端读取后清零）。

### 经验 / 等级
- `exp` 为累计总经验。
- `level_for_exp(exp)`：满足 `100·(L-1)·L/2 ≤ exp` 的最大 `L`（L≥1）。
  - 升到 2 级需 100，3 级累计 300，4 级累计 600 …（每级增量 100·(L-1)）。
- 每次 exp 变化后用 `level_for_exp` 重算并写回 `game_profile.level`。
- 等级提升 → 产出速率提升（`rate_per_min = level`）。

### 行为奖励（统一走 coin_ledger）
- **完成 Todo**：现有 `award_for_todo` 加发经验 `+5`（同一事务），重算等级。reason 仍 `todo_done`，按 `ref_id`(todo_id) 去重。
- **首次阅读邮件**：`award_for_mail(msg_id)` —— 金币 `+2`、经验 `+3`；按 message id 去重（kv `reward:mail:<id>`，因 `coin_ledger.ref_id` 是 INTEGER 存不下字符串 id）；reason `mail_read`，ref_id=NULL。在 `mail_mark_read(read=true)` 成功后调用。

### 离线收益告知
- 启动 `settle_idle` 后，若 `earned>0`，kv `idle.offline_earned += earned`。
- 前端主页 `onMount` 调 `game_take_offline_earned()`（返回并清零 kv）→ 若 >0 显示「离线获得 N 金币 / Earned N coins while away」可关闭提示。

---

## 四、数据模型

**无新建表、无 migration。** 复用：
- `game_profile`：coins / exp / level / last_tick（均已存在）。
- `coin_ledger`：reason ∈ {`todo_done`, `idle`, `mail_read`}；`idle`/`mail_read` 的 ref_id 可为 NULL。
- kv：`idle.offline_earned`（待展示的离线收益）、`reward:mail:<id>`（邮件奖励去重）。

---

## 五、后端结构

```
src-tauri/src/
  db/game.rs        # settle_idle, rate_per_min, level_for_exp, compute_earned, add_exp,
                    #   award_for_mail；扩展 award_for_todo 加经验
  idle.rs           # 后台 60s 结算线程（仿 reminder.rs）
  commands/game.rs  # game_status, game_take_offline_earned（game_get_profile 已存在）
  models/mod.rs     # GameStatus（带派生字段）
  lib.rs            # setup 启动离线结算 + spawn idle 线程；注册命令
  commands/mail.rs  # mail_mark_read 成功后 award_for_mail
```

### 纯函数（单测）
```rust
pub fn rate_per_min(level: i64) -> i64 { level.max(1) }
pub fn level_for_exp(exp: i64) -> i64 { /* 最大 L: 100*(L-1)*L/2 <= exp */ }
pub fn compute_earned(elapsed_secs: i64, level: i64, cap_secs: i64) -> i64 {
    let s = elapsed_secs.clamp(0, cap_secs);
    (s / 60) * rate_per_min(level)
}
```

### GameStatus（前端展示用）
```rust
#[derive(Serialize)]
pub struct GameStatus {
    pub coins: i64,
    pub exp: i64,
    pub level: i64,
    pub exp_into_level: i64,   // 当前等级内已积累
    pub exp_for_next: i64,     // 升下一级所需（本级跨度）
    pub rate_per_min: i64,
}
```
- `cumulative(L) = 100*(L-1)*L/2`；`exp_into_level = exp - cumulative(level)`；`exp_for_next = cumulative(level+1) - cumulative(level) = 100*level`。

### 命令
- `game_get_profile`（已存在）。
- `game_status() -> GameStatus`：先 `settle_idle`（让面板打开即最新），再读 profile + 派生。
- `game_take_offline_earned() -> i64`：读 kv `idle.offline_earned`，清零，返回。

### 结算线程 `idle.rs`
- 仿 `reminder.rs`：`std::thread::spawn` + 每 60s `settle_idle`；失败吞掉续跑。

---

## 六、前端

- **主页游戏面板**（`routes/(app)/+page.svelte` 顶部加一块，或抽组件）：
  - 显示 `Lv {level}`、经验条（`exp_into_level / exp_for_next`）、`产出 {rate}/min`、金币（已有）。
  - `onMount`：`game_status()` 取数；`game_take_offline_earned()` >0 → 弹可关闭提示「离线获得 N 金币」。
  - 轻量轮询：每 60s `game_status()` 刷新（反映自动产出），同步 coins store。
- **金币 widget**（`routes/(widget)/widgets/coins/+page.svelte`）：在 `🪙 N` 旁显示 `Lv L`；onMount + 每 60s 刷新（复用 game store）。
- `lib/api/index.ts`：`gameStatus()`、`gameTakeOfflineEarned()`；`lib/stores/game.ts` 扩展（level/exp/rate + refreshStatus）。

---

## 七、测试策略

- **纯函数单测**：`rate_per_min`、`level_for_exp`（边界：0→1、99→1、100→2、299→2、300→3）、`compute_earned`（不足分钟=0、封顶生效）。
- **db 单测**：`settle_idle` 用 in-memory + 手动设 `last_tick` 为过去 → 断言 earned 与 coins/last_tick 更新、封顶；`award_for_todo` 加经验后等级重算；`award_for_mail` 去重（同 id 二次=0）。
- **门禁**：`cargo test`、`cargo clippy -- -D warnings`、`npm run check` 全绿。

---

## 八、风险与平衡

- **数值膨胀**：8h 封顶 + 速率 `level/min` 较温和；纯被动无点击刷取。参数集中在 `rate_per_min` 与奖励常量，便于调。
- **last_tick 时区**：`game_profile.last_tick` 存 `datetime('now','localtime')`；结算用 `julianday(..,'localtime')` 保持一致，避免时区漂移。
- **零头丢失**：earned<1 分钟时不推进 last_tick，累积到够 1 分钟再结算。
- **性能**：60s 一次轻量 SQL，单线程多在 sleep，开销可忽略。

---

## 九、文件清单（预计改动）

**后端**
- `src/db/game.rs`：纯函数 + `settle_idle` + `add_exp` + `award_for_mail` + 扩展 `award_for_todo` + 测试
- `src/idle.rs`（新）+ `lib.rs`：mod、setup 启动离线结算 + spawn 线程、注册命令
- `src/commands/game.rs`：`game_status` + `game_take_offline_earned`
- `src/commands/mail.rs`：`mail_mark_read` 成功后 `award_for_mail`
- `src/models/mod.rs`：`GameStatus`

**前端**
- `routes/(app)/+page.svelte`：游戏面板 + 离线提示 + 轮询
- `routes/(widget)/widgets/coins/+page.svelte`：显示等级
- `lib/api/index.ts`、`lib/stores/game.ts`

> 实施计划见 [plan](../plans/2026-06-16-deskhub-m6-game.md)。
