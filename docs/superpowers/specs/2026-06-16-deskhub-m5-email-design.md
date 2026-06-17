# DeskHub M5 邮件管理 — 设计文档

> 日期：2026-06-16
> 关联 issue：[#2](https://github.com/Nyxicemoon/Desktop_Widget/issues/2)
> 范围：完成 [项目.md](../../../项目.md) 4.3「邮件管理」（规格第九章难点：OAuth 登录与 Token 管理）。

---

## 一、目标与范围

接入 **Gmail**，在 DeskHub 内查看/搜索/读写邮件状态，并提供桌面未读数 widget。

**本轮做：**
1. **Gmail OAuth 2.0**（Authorization Code + PKCE，loopback 回调），Token 存 **Windows 凭据管理器**。
2. **拉取收件箱列表 / 详情 / 搜索**。
3. **已读/未读**：显示状态 + 标记读/未读（scope=`gmail.modify`）。
4. **Token 刷新与过期处理**。
5. **主窗口「邮件」页** + **透明未读数 widget**。

**非目标（明确排除）：**
- 行为奖励（阅邮给金币）—— 按用户决策**留到 M6**。
- Outlook / Microsoft Graph —— 后续扩展。
- 写邮件 / 回复 / 发送 —— 本轮只读 + 改状态，不发信。
- 邮件正文/附件持久化到本地 —— 隐私考虑，不落 SQLite（仅内存/即时展示）。
- 多账号 —— 本轮单账号。

---

## 二、关键决策（已与用户确认）

| 决策点 | 选择 |
| -- | -- |
| 授权范围 | **`https://www.googleapis.com/auth/gmail.modify`**（读 + 标记读/未读 + 归档） |
| 阅邮奖励金币 | **留到 M6**（M5 只做邮件核心） |
| 展示位置 | **主窗口「邮件」页 + 透明未读数 widget** |
| OAuth 回调 | **loopback**（`http://127.0.0.1:{随机端口}`，Google 推荐的 native app 方式） |
| 客户端凭据 | **用户自带** Google OAuth Desktop 客户端（Client ID + Secret 存本地 `config.json`） |
| Token 存储 | **Windows 凭据管理器**（`keyring` crate），不入 SQLite 明文 |

---

## 三、OAuth 流程（Authorization Code + PKCE + loopback）

### 凭据来源
- 用户在 Google Cloud Console 建项目 → 启用 Gmail API → 创建 OAuth 客户端（类型 **Desktop app**）→ 拿到 **Client ID** 和 **Client Secret**。
- 在应用「邮件」页或设置里填入，写到 `app_data_dir/config.json`（与 Pexels key 同机制：本地明文、不入库、不进 git）。
- OAuth 同意屏设为 **Testing** 模式并把自己的邮箱加为 **test user** —— 个人使用即可，无需 Google 审核（`gmail.modify` 在生产发布才需审核）。

### 流程步骤
1. 生成 PKCE：`code_verifier`（43–128 字符随机，`getrandom`）+ `code_challenge` = base64url(SHA256(verifier))；以及随机 `state`。
2. 启动 loopback：`TcpListener::bind("127.0.0.1:0")`，取实际端口 `port`。
3. 构造授权 URL（`accounts.google.com/o/oauth2/v2/auth`），参数：
   `client_id`、`redirect_uri=http://127.0.0.1:{port}`、`response_type=code`、
   `scope=https://www.googleapis.com/auth/gmail.modify`、`code_challenge`、`code_challenge_method=S256`、
   `state`、`access_type=offline`、`prompt=consent`（确保拿到 refresh_token）。
4. 用 `tauri-plugin-opener` 在系统浏览器打开授权 URL。
5. loopback 接收一个 HTTP 请求，从 GET 行解析 `code` 与 `state`（校验 state 一致）；回写一段「授权完成，可关闭本页」的 HTML；关闭 listener。
6. 用 `code` + `code_verifier` 向 `oauth2.googleapis.com/token` 换取
   `access_token`、`refresh_token`、`expires_in`（`grant_type=authorization_code`，带 `client_secret`、`redirect_uri`）。
7. 存储：`refresh_token` → 凭据管理器；`access_token` + 到期时刻 → 内存（managed state）；连接的邮箱地址 → kv（用 `gmail/profile` 或解析 id_token / 调 `users/me/profile` 取 `emailAddress`）。

### Token 刷新
- API 调用前检查内存 `access_token` 是否过期（留 60s 余量）；过期则用凭据库的 `refresh_token` 向 token 端点 `grant_type=refresh_token` 刷新，更新内存 token。
- 若刷新失败（refresh_token 失效）→ 标记未连接，前端提示重新连接。

### 断开
- `gmail_disconnect`：删凭据库 refresh_token + 清内存 token + 清 kv 连接信息。

---

## 四、Gmail API（reqwest blocking 直连 REST）

基址 `https://gmail.googleapis.com/gmail/v1/users/me`，请求头 `Authorization: Bearer {access_token}`。

| 操作 | 端点 |
| -- | -- |
| 列表 / 搜索 | `GET /messages?q={query}&maxResults=25&labelIds=INBOX` → 返回 `{id, threadId}[]` |
| 详情 | `GET /messages/{id}?format=full` → headers(From/Subject/Date)、snippet、body |
| 标记已读 | `POST /messages/{id}/modify` body `{"removeLabelIds":["UNREAD"]}` |
| 标记未读 | `POST /messages/{id}/modify` body `{"addLabelIds":["UNREAD"]}` |
| 未读数 | `GET /labels/INBOX` → `messagesUnread` |
| 邮箱地址 | `GET /profile` → `emailAddress` |

- 列表只拿 id；详情按需取（点开邮件才 `GET /messages/{id}`）。或列表后批量取 metadata（From/Subject/Date/snippet/UNREAD）以渲染列表。**本轮**：列表取每封的 `format=metadata`（headers: From,Subject,Date + labelIds 判断 UNREAD + snippet），详情点开取 `format=full` 解析正文。
- 正文解析：MIME parts，优先 `text/plain`，无则 `text/html`（base64url 解码 `body.data`）；嵌套 `multipart` 递归找。
- 401 → 自动刷新 token 重试一次。

---

## 五、数据模型

**不新建邮件表、不持久化邮件内容。** 仅：
- 凭据库（keyring，service=`com.deskhub.app`，account=`gmail-refresh`）：`refresh_token`。
- `config.json`：`google_client_id`、`google_client_secret`。
- kv：`gmail.email`（已连接邮箱地址，判断连接态 + 显示）、`widget.mail.visible`、`mail.unread`（最近未读数缓存，给 widget 快速初值）。

> 故**无 migration**。`WidgetVisibility` 增加 `mail` 字段（同 apps 模式）。

---

## 六、后端结构

```
src-tauri/src/
  gmail/
    mod.rs        # pub mod auth; pub mod api; 共享 token state 定义
    auth.rs       # PKCE、loopback、授权 URL、token 交换/刷新、connect/disconnect
    api.rs        # list/get/search/modify/unread_count/profile（reqwest，401 自动刷新）
  commands/
    mail.rs       # gmail_connect/disconnect/status + mail_list/get/search/mark_read/unread_count
  models/mod.rs   # MailSummary, MailDetail, GmailStatus；WidgetVisibility + mail
```

- **Token state**：`GmailState(Mutex<Option<AccessToken>>)`（`AccessToken{ value, expires_at }`），`app.manage`。
- **config**：`config.rs` 增加读写 `google_client_id`/`google_client_secret`（沿用现有 config.json 读写）。
- **keyring**：封装 `save_refresh / load_refresh / delete_refresh`。
- 命令均 `async`（OAuth 与网络 IO 用 `tauri::async_runtime::spawn_blocking` 包阻塞调用，避免占用主线程；window-creating 教训不适用，但网络阻塞仍应 off-thread）。

### models
```rust
#[derive(Serialize)]
pub struct MailSummary { pub id: String, pub from: String, pub subject: String,
                         pub date: String, pub snippet: String, pub unread: bool }
#[derive(Serialize)]
pub struct MailDetail  { pub id: String, pub from: String, pub to: String, pub subject: String,
                         pub date: String, pub body: String, pub is_html: bool, pub unread: bool }
#[derive(Serialize)]
pub struct GmailStatus { pub connected: bool, pub email: Option<String> }
```

### 命令
```
gmail_connect()        -> GmailStatus     // 跑完整 OAuth，成功后返回已连接
gmail_disconnect()     -> ()
gmail_status()         -> GmailStatus
mail_list()            -> Vec<MailSummary>     // 收件箱前 25
mail_search(q)         -> Vec<MailSummary>
mail_get(id)           -> MailDetail           // 取详情（不自动标已读，交前端决定）
mail_mark_read(id, read: bool) -> ()
mail_unread_count()    -> i64
```

---

## 七、前端

- **`routes/(app)/mail/+page.svelte`**：
  - 未配置 client → 提示填 `Client ID`/`Secret`（输入框 + 保存，写 config）。
  - 已配置未连接 → 「连接 Gmail / Connect」按钮 → `gmail_connect`（浏览器授权）。
  - 已连接 → 顶部显示邮箱地址 + 断开按钮；搜索框；收件箱列表（未读加粗/圆点）；点条目 → 详情面板（From/Subject/Date/正文）；详情里「标已读/未读」按钮；打开邮件时可选自动标已读。
  - 正文为 HTML 时用受限渲染（`<iframe sandbox>` 或纯文本兜底，避免 XSS/远程加载）。本轮：优先纯文本；HTML 放入 `sandbox` iframe（禁脚本）。
- **`routes/(widget)/widgets/mail/+page.svelte`**：透明卡片显示 `📧 {unread}`，`onMount` + 每 5 分钟轮询 `mail_unread_count`，点击可（可选）唤起主窗邮件页。
- 导航加「邮件 / Mail」；主窗口加「邮件 widget」勾选框；托盘加开关。
- `lib/api/index.ts` 封装；`lib/stores/mail.ts`（可选）。

---

## 八、依赖

- `keyring = "3"`（Windows 凭据管理器后端）。
- `sha2 = "0.10"`（PKCE code_challenge）。
- `getrandom = "0.2"`（code_verifier / state 随机源）。
- loopback 用 std `TcpListener`（无新增 dep）。
- `reqwest`(blocking, json) 已有，复用（HTTPS 已用于 Pexels）。
- URL 拼接用手写 `urlencoding`（小工具函数，或加 `urlencoding = "2"` 轻量 crate；倾向手写避免新依赖）。

---

## 九、测试策略

OAuth/网络依赖外部，难纯单测；策略：
- **可单测（纯函数）**：
  - PKCE：`code_challenge = base64url_nopad(sha256(verifier))` 对已知向量断言（RFC 7636 测试向量）。
  - 授权 URL 构造：给定参数断言含必需 query（client_id/redirect_uri/scope/code_challenge/S256/state）。
  - loopback 请求解析：给定 `GET /?code=abc&state=xyz HTTP/1.1` 行，断言解析出 code/state。
  - Gmail 响应解析：用样例 JSON 断言 `MailSummary`/`MailDetail`/正文 MIME 选取（text/plain 优先、base64url 解码）。
- **不单测**：真实 OAuth 往返、token 刷新、keyring 读写（手动验证）。
- **门禁**：`cargo test`、`cargo clippy -- -D warnings`、`npm run check` 全绿。

---

## 十、风险与缓解

- **OAuth 回调/loopback**（规格难点）：loopback 是 Google 对 native app 的推荐方式，无需自定义 scheme；单次请求解析简单。**建议实现时先 spike**（loopback + 授权 URL + token 交换打通）再接 UI。
- **Google「未验证应用」提示**：自有客户端 + Testing 模式 + 自己为 test user → 同意屏会有「未验证」提醒但可继续；个人使用无需审核。文档（README/页面提示）写清申请步骤。
- **client_secret 本地存放**：Desktop 客户端的 secret 非真正机密（Google 文档说明），仅本地 config.json，不进 git；可接受。
- **HTML 邮件安全**：sandbox iframe 禁脚本、禁自动加载远程资源（避免追踪像素/XSS）；优先纯文本。
- **令牌泄漏面**：refresh_token 进凭据管理器；access_token 仅内存。

---

## 十一、文件清单（预计改动，供 plan 细化）

**后端**
- `Cargo.toml`：+ `keyring`、`sha2`、`getrandom`
- `src/gmail/{mod,auth,api}.rs`（新）+ `src/lib.rs` 挂模块 + manage GmailState + 注册命令
- `src/commands/mail.rs`（新）+ `src/commands/mod.rs`
- `src/config.rs`：google client id/secret 读写
- `src/models/mod.rs`：MailSummary/MailDetail/GmailStatus + `WidgetVisibility.mail`
- `src/window/mod.rs`：`widget_config` + mail，`read_visibility` + mail
- `src/tray.rs`：mail widget 开关
- `src/keyring` 封装（并入 `gmail/auth.rs`）

**前端**
- `routes/(app)/mail/+page.svelte`（新）+ 导航
- `routes/(widget)/widgets/mail/+page.svelte`（新）
- `routes/(app)/+page.svelte`：邮件 widget 勾选框
- `lib/api/index.ts`：封装

> 实施计划见 [plan](../plans/2026-06-16-deskhub-m5-email.md)。
