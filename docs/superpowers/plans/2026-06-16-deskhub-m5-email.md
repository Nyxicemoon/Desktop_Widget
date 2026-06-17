# DeskHub M5 邮件管理 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans. Steps use checkbox (`- [ ]`).

**Goal:** 接入 Gmail（OAuth2 PKCE + loopback，Token 入 Windows 凭据管理器），实现收件箱列表/详情/搜索、标记读写、未读数 widget。

**Architecture:** 纯函数（PKCE、授权 URL、loopback 解析、Gmail JSON/MIME 解析）单测覆盖；OAuth 往返 + token 刷新 + API 调用走 reqwest blocking（在 `spawn_blocking` 中）；refresh_token 存 keyring，access_token 存内存 managed state。无 migration。

**Tech Stack:** Rust + Tauri v2、reqwest(blocking,json)、keyring、sha2、getrandom、std TcpListener、SvelteKit + TS。

**参考 spec：** `docs/superpowers/specs/2026-06-16-deskhub-m5-email-design.md`

**质量门禁（每任务末）：** `src-tauri/` 下 `cargo test`、`cargo clippy -- -D warnings`；前端 `npm run check`。
> npm 经 nvm：若 `npm` 不在 PATH，PowerShell 先 `$env:Path = "C:\nvm4w\nodejs;" + $env:Path`。

> ⚠️ **实现建议（spike first）：** OAuth 往返是规格难点。建议先把 Task 2（PKCE/URL/loopback 纯函数，可单测）做完，再做 Task 3（token 交换/刷新）并用一个真实 Google 客户端手动打通一次，再继续 UI。Win32/网络这类外部依赖代码，实现者应对照 crate 实际签名编译迭代（keyring 3.x、reqwest blocking）。

---

## Task 1: 依赖 + 配置（client id/secret）+ 模型 + widget 可见性

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/config.rs`
- Modify: `src-tauri/src/models/mod.rs`
- Modify: `src-tauri/src/window/mod.rs`
- Modify: `src-tauri/src/lib.rs`（setup fallback 的 WidgetVisibility）

- [ ] **Step 1: 加依赖**

在 `src-tauri/Cargo.toml` `[dependencies]` 追加：

```toml
keyring = "3"
sha2 = "0.10"
getrandom = "0.2"
```

- [ ] **Step 2: config 读写 Google client**

查看 `src-tauri/src/config.rs` 现有 Pexels key 读写模式，照其结构增加两个字段的读写。config.json 形如 `{"pexels_key": "...", "google_client_id": "...", "google_client_secret": "..."}`。新增函数（命名对齐既有风格）：
- `get_google_client() -> AppResult<Option<(String, String)>>`：读 id+secret，任一缺失返回 None。
- `set_google_client(id: &str, secret: &str) -> AppResult<()>`：写入并保留既有字段（用既有的「读整个 json→改字段→写回」逻辑，勿覆盖 pexels_key）。

> 实现者：先 Read `config.rs`，复用其 serde 结构（给 struct 加 `google_client_id: Option<String>`、`google_client_secret: Option<String>` 字段，`#[serde(default)]`）。

- [ ] **Step 3: 模型**

在 `src-tauri/src/models/mod.rs` 追加：

```rust
#[derive(Debug, Serialize)]
pub struct MailSummary {
    pub id: String,
    pub from: String,
    pub subject: String,
    pub date: String,
    pub snippet: String,
    pub unread: bool,
}

#[derive(Debug, Serialize)]
pub struct MailDetail {
    pub id: String,
    pub from: String,
    pub to: String,
    pub subject: String,
    pub date: String,
    pub body: String,
    pub is_html: bool,
    pub unread: bool,
}

#[derive(Debug, Serialize)]
pub struct GmailStatus {
    pub connected: bool,
    pub email: Option<String>,
}
```

并把 `WidgetVisibility` 增加 `mail` 字段：

```rust
#[derive(Debug, Serialize)]
pub struct WidgetVisibility {
    pub todo: bool,
    pub coins: bool,
    pub apps: bool,
    pub mail: bool,
}
```

- [ ] **Step 4: window 支持 mail widget**

在 `src-tauri/src/window/mod.rs` 的 `widget_config` match 增加：

```rust
        "mail" => Ok(("widget-mail", "/widgets/mail", 200.0, 90.0, 580.0, 40.0)),
```

并把 `read_visibility` 增加 mail：

```rust
        mail: kv::get(conn, "widget.mail.visible")?.as_deref() == Some("1"),
```

在 `visibility_defaults_false` 测试加 `assert!(!v.mail);`。

- [ ] **Step 5: lib.rs fallback + setup 恢复**

在 `src-tauri/src/lib.rs` setup 中：
- 把 `WidgetVisibility { todo: false, coins: false, apps: false }` fallback 改为加 `mail: false`。
- 在恢复 widget 处加 `if vis.mail { let _ = window::open_widget(app.handle(), "mail"); }`。

- [ ] **Step 6: 门禁**

Run: `cd src-tauri; cargo build`（此刻仅结构改动，应编译通过；mail widget 路由前端稍后建，后端不依赖）。
> 注意：此步还没注册新命令、没建 gmail 模块，cargo test/clippy 应通过（无死代码：新模型被后续任务用；若 clippy 报 MailSummary 等未使用，先继续到 Task 4 注册命令后再统一过 clippy —— 或本步暂只 `cargo build`，门禁留到 Task 4）。

- [ ] **Step 7: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/config.rs src-tauri/src/models src-tauri/src/window src-tauri/src/lib.rs
git commit -m "feat(m5): deps, config google client, mail models, mail widget visibility"
```

---

## Task 2: OAuth 纯函数（PKCE / 授权 URL / loopback 解析）+ 单测

**Files:**
- Create: `src-tauri/src/gmail/mod.rs`
- Create: `src-tauri/src/gmail/auth.rs`
- Modify: `src-tauri/src/lib.rs`（`mod gmail;`）

- [ ] **Step 1: 创建 `src-tauri/src/gmail/mod.rs`**

```rust
pub mod api;
pub mod auth;

use std::time::Instant;

/// In-memory access token + expiry. Managed by Tauri.
#[derive(Default)]
pub struct GmailState(pub std::sync::Mutex<Option<AccessToken>>);

#[derive(Clone)]
pub struct AccessToken {
    pub value: String,
    pub expires_at: Instant,
}
```

- [ ] **Step 2: 创建 `src-tauri/src/gmail/auth.rs` 的纯函数部分 + 测试**

先写可单测的纯函数（先不含网络/loopback IO）：

```rust
use crate::error::{AppError, AppResult};
use base64::Engine;

const AUTH_ENDPOINT: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const SCOPE: &str = "https://www.googleapis.com/auth/gmail.modify";

/// URL-encode a query component (RFC 3986 unreserved kept).
pub fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// PKCE code_challenge = base64url-nopad(SHA256(verifier)).
pub fn code_challenge(verifier: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}

/// Random base64url string of `n` bytes (for verifier / state).
pub fn random_token(n: usize) -> String {
    let mut buf = vec![0u8; n];
    getrandom::getrandom(&mut buf).expect("getrandom");
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&buf)
}

/// Build the Google authorization URL.
pub fn build_auth_url(
    client_id: &str,
    redirect_uri: &str,
    challenge: &str,
    state: &str,
) -> String {
    format!(
        "{AUTH_ENDPOINT}?client_id={}&redirect_uri={}&response_type=code&scope={}\
         &code_challenge={}&code_challenge_method=S256&state={}&access_type=offline&prompt=consent",
        url_encode(client_id),
        url_encode(redirect_uri),
        url_encode(SCOPE),
        url_encode(challenge),
        url_encode(state),
    )
}

/// Parse `code` and `state` from the loopback request's first line, e.g.
/// "GET /?code=abc&state=xyz HTTP/1.1".
pub fn parse_redirect(request_line: &str) -> AppResult<(String, String)> {
    let path = request_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| AppError::Other("bad request line".into()))?;
    let query = path.split('?').nth(1).unwrap_or("");
    let mut code = None;
    let mut state = None;
    for kv in query.split('&') {
        let mut it = kv.splitn(2, '=');
        match (it.next(), it.next()) {
            (Some("code"), Some(v)) => code = Some(v.to_string()),
            (Some("state"), Some(v)) => state = Some(v.to_string()),
            _ => {}
        }
    }
    match (code, state) {
        (Some(c), Some(s)) => Ok((c, s)),
        _ => Err(AppError::Other("missing code/state in redirect".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_challenge_matches_rfc7636_vector() {
        // RFC 7636 Appendix B test vector.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(code_challenge(verifier), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    #[test]
    fn auth_url_has_required_params() {
        let u = build_auth_url("cid.apps", "http://127.0.0.1:5000", "chal", "st8");
        assert!(u.contains("client_id=cid.apps"));
        assert!(u.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A5000"));
        assert!(u.contains("code_challenge=chal"));
        assert!(u.contains("code_challenge_method=S256"));
        assert!(u.contains("state=st8"));
        assert!(u.contains("gmail.modify"));
        assert!(u.contains("access_type=offline"));
    }

    #[test]
    fn parse_redirect_extracts_code_and_state() {
        let (c, s) = parse_redirect("GET /?code=abc123&state=xyz HTTP/1.1").unwrap();
        assert_eq!(c, "abc123");
        assert_eq!(s, "xyz");
    }

    #[test]
    fn parse_redirect_missing_code_errors() {
        assert!(parse_redirect("GET /?state=xyz HTTP/1.1").is_err());
    }

    #[test]
    fn random_token_is_urlsafe_and_nonempty() {
        let t = random_token(32);
        assert!(!t.is_empty());
        assert!(t.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }
}
```

- [ ] **Step 3: 声明模块**

在 `src-tauri/src/lib.rs` 顶部模块区按字母序加 `mod gmail;`（在 `mod error;` 之后、`mod models;` 之前）。
> `gmail/api.rs` 此刻尚未创建，但 `mod.rs` 已 `pub mod api;` → 需要 Task 3 创建 `api.rs` 才能编译。**为保证本任务可编译**：本步先在 `gmail/mod.rs` 暂时注释掉 `pub mod api;`（留 `pub mod auth;`），Task 3 再恢复。

- [ ] **Step 4: 门禁**

Run: `cd src-tauri; cargo test gmail; cargo clippy -- -D warnings`
Expected: 5 个纯函数测试通过；clippy 干净（auth.rs 的 pub 函数被测试引用，无死代码）。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/gmail src-tauri/src/lib.rs
git commit -m "feat(m5): OAuth pure fns (PKCE, auth URL, redirect parse) + tests"
```

---

## Task 3: OAuth 往返（loopback + token 交换/刷新）+ keyring

> 网络/keyring 代码，实现者对照 crate 实际签名编译迭代；建议用真实 Google 客户端手动打通。

**Files:**
- Modify: `src-tauri/src/gmail/auth.rs`
- Modify: `src-tauri/src/gmail/mod.rs`（恢复 `pub mod api;` 留到 Task 4；本任务仍只用 auth）

- [ ] **Step 1: keyring 封装（auth.rs 内）**

追加：

```rust
const KR_SERVICE: &str = "com.deskhub.app";
const KR_ACCOUNT: &str = "gmail-refresh";

fn kr_entry() -> AppResult<keyring::Entry> {
    keyring::Entry::new(KR_SERVICE, KR_ACCOUNT).map_err(|e| AppError::Other(e.to_string()))
}

pub fn save_refresh(token: &str) -> AppResult<()> {
    kr_entry()?.set_password(token).map_err(|e| AppError::Other(e.to_string()))
}

pub fn load_refresh() -> AppResult<Option<String>> {
    match kr_entry()?.get_password() {
        Ok(t) => Ok(Some(t)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(AppError::Other(e.to_string())),
    }
}

pub fn delete_refresh() -> AppResult<()> {
    match kr_entry()?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(AppError::Other(e.to_string())),
    }
}
```

- [ ] **Step 2: token 交换/刷新 + loopback（auth.rs 内）**

追加（用 reqwest blocking；token 端点 `https://oauth2.googleapis.com/token`）：

```rust
use serde::Deserialize;

const TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";

#[derive(Deserialize)]
struct TokenResp {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    expires_in: u64,
}

/// Run a one-shot loopback server, return (code, state, redirect_uri).
/// Opens the browser via caller (command layer) BEFORE awaiting here.
pub fn run_loopback(expected_state: &str) -> AppResult<(String, String)> {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| AppError::Io(e.to_string()))?;
    let port = listener
        .local_addr()
        .map_err(|e| AppError::Io(e.to_string()))?
        .port();
    let redirect_uri = format!("http://127.0.0.1:{port}");
    // NOTE: caller must build auth URL with this redirect_uri and open browser.
    // To do that, this fn returns the redirect first via out-param pattern:
    // (handled by connect(): it binds, then opens browser, then accepts.)
    // Here we both bind and accept — see connect() which restructures this.
    let (mut stream, _) = listener.accept().map_err(|e| AppError::Io(e.to_string()))?;
    let mut buf = [0u8; 2048];
    let n = stream.read(&mut buf).map_err(|e| AppError::Io(e.to_string()))?;
    let req = String::from_utf8_lossy(&buf[..n]);
    let first = req.lines().next().unwrap_or("");
    let (code, state) = parse_redirect(first)?;
    let body = "<html><body>DeskHub 已连接，可关闭本页。 / Connected, you may close this tab.</body></html>";
    let resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(resp.as_bytes());
    if state != expected_state {
        return Err(AppError::Other("state mismatch".into()));
    }
    let _ = redirect_uri; // redirect_uri is recomputed in connect()
    Ok((code, redirect_uri))
}

/// Exchange authorization code for tokens. Returns (access, refresh, expires_in).
pub fn exchange_code(
    client_id: &str,
    client_secret: &str,
    code: &str,
    verifier: &str,
    redirect_uri: &str,
) -> AppResult<(String, String, u64)> {
    let client = reqwest::blocking::Client::new();
    let resp: TokenResp = client
        .post(TOKEN_ENDPOINT)
        .form(&[
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("code", code),
            ("code_verifier", verifier),
            ("grant_type", "authorization_code"),
            ("redirect_uri", redirect_uri),
        ])
        .send()
        .map_err(|e| AppError::Network(e.to_string()))?
        .error_for_status()
        .map_err(|e| AppError::Network(e.to_string()))?
        .json()
        .map_err(|e| AppError::Network(e.to_string()))?;
    let refresh = resp
        .refresh_token
        .ok_or_else(|| AppError::Other("no refresh_token returned".into()))?;
    Ok((resp.access_token, refresh, resp.expires_in))
}

/// Refresh the access token using a stored refresh_token. Returns (access, expires_in).
pub fn refresh_access(
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
) -> AppResult<(String, u64)> {
    let client = reqwest::blocking::Client::new();
    let resp: TokenResp = client
        .post(TOKEN_ENDPOINT)
        .form(&[
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .map_err(|e| AppError::Network(e.to_string()))?
        .error_for_status()
        .map_err(|e| AppError::Network(e.to_string()))?
        .json()
        .map_err(|e| AppError::Network(e.to_string()))?;
    Ok((resp.access_token, resp.expires_in))
}
```

> **实现注意：** 上面 `run_loopback` 把「bind→开浏览器→accept」揉在一起不便（开浏览器需要在 accept 前、且要拿到 port 拼 URL）。Task 4 的 `gmail_connect` 命令应**重构**为：先 `TcpListener::bind` 拿 port、拼 redirect_uri、`build_auth_url`、用 opener 开浏览器、再 `listener.accept()` 收 code。可把 `run_loopback` 拆成 `bind_loopback()->(TcpListener,port)` 与 `accept_code(listener, expected_state)->code`。实现者按此调整（保留 `parse_redirect`/`exchange_code`/`refresh_access` 不变）。

- [ ] **Step 3: 门禁**

Run: `cd src-tauri; cargo build`（网络函数无单测；确保编译通过、签名正确）。`cargo clippy -- -D warnings`。
> 若出现未使用警告（这些 pub fn 要到 Task 4 才被命令调用）：本步门禁可只 `cargo build`，clippy 留到 Task 4 接好命令后跑。

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/gmail/auth.rs
git commit -m "feat(m5): keyring token storage + loopback + token exchange/refresh"
```

---

## Task 4: Gmail API + 命令层 + 注册

**Files:**
- Create: `src-tauri/src/gmail/api.rs`
- Modify: `src-tauri/src/gmail/mod.rs`（恢复 `pub mod api;`）
- Create: `src-tauri/src/commands/mail.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`（manage GmailState + 注册命令）

- [ ] **Step 1: `gmail/api.rs`**

实现（要点，实现者补全 JSON 结构体）：
- `valid_access(app) -> AppResult<String>`：读 `GmailState`；若有且未过期返回；否则 `config::get_google_client()` + `auth::load_refresh()` → `auth::refresh_access()` → 更新 state（`expires_at = Instant::now() + Duration::from_secs(expires_in - 60)`）→ 返回新 token；无 refresh 则 `Err(NotFound)`。
- `list(app, query, max) -> Vec<MailSummary>`：`GET /messages?maxResults&labelIds=INBOX&q=`（搜索时传 q）拿 ids → 对每个 `GET /messages/{id}?format=metadata&metadataHeaders=From&metadataHeaders=Subject&metadataHeaders=Date` 解析 header + `labelIds` 含 `UNREAD` + `snippet`。
- `get(app, id) -> MailDetail`：`GET /messages/{id}?format=full` → headers + 递归 MIME 取正文（`text/plain` 优先，否则 `text/html`，`body.data` base64url 解码）。
- `mark_read(app, id, read)`：`POST /messages/{id}/modify`，read→removeLabelIds=["UNREAD"]，否则 addLabelIds。
- `unread_count(app) -> i64`：`GET /labels/INBOX` → `messagesUnread`。
- `profile_email(app) -> String`：`GET /profile` → `emailAddress`。
- 所有 GET/POST 用 `Authorization: Bearer`；收到 401 时刷新一次重试。
- **纯函数抽出单测**：`fn pick_body(payload: &PayloadJson) -> (String, bool)`（选 plain/html + 是否 html）、`fn header(headers, name) -> String`、`fn decode_b64url(s) -> String`。给样例 JSON 单测。

实现者：定义 serde 结构体（`MessageListResp{messages: Vec<{id}>}`、`MessageResp{snippet, labelIds, payload}`、`Payload{mimeType, headers: Vec<{name,value}>, body: {data}, parts: Vec<Payload>}`）。把 body 选取与 header 提取写成纯函数并单测。

- [ ] **Step 2: `commands/mail.rs`**

```rust
use crate::config;
use crate::error::{AppError, AppResult};
use crate::gmail::{api, auth, GmailState};
use crate::models::{GmailStatus, MailDetail, MailSummary};
use crate::db::{kv, Db};
use tauri::{AppHandle, Manager, State};

#[tauri::command]
pub async fn gmail_status(app: AppHandle, db: State<'_, Db>) -> AppResult<GmailStatus> {
    let email = {
        let conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
        kv::get(&conn, "gmail.email")?
    };
    let connected = email.is_some() && auth::load_refresh()?.is_some();
    Ok(GmailStatus { connected, email })
}

#[tauri::command]
pub async fn gmail_connect(app: AppHandle) -> AppResult<GmailStatus> {
    // see spike note: bind loopback -> open browser -> accept -> exchange -> save refresh -> set state -> profile -> kv email
    let status = tauri::async_runtime::spawn_blocking(move || api::connect(&app))
        .await
        .map_err(|e| AppError::Other(e.to_string()))?;
    status
}

#[tauri::command]
pub async fn gmail_disconnect(app: AppHandle, db: State<'_, Db>) -> AppResult<()> {
    auth::delete_refresh()?;
    *app.state::<GmailState>().0.lock().map_err(|e| AppError::Other(e.to_string()))? = None;
    let conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
    kv::set(&conn, "gmail.email", "")?; // or delete; empty = disconnected
    Ok(())
}

#[tauri::command]
pub async fn mail_list(app: AppHandle) -> AppResult<Vec<MailSummary>> {
    tauri::async_runtime::spawn_blocking(move || api::list(&app, "", 25))
        .await
        .map_err(|e| AppError::Other(e.to_string()))?
}

#[tauri::command]
pub async fn mail_search(app: AppHandle, query: String) -> AppResult<Vec<MailSummary>> {
    tauri::async_runtime::spawn_blocking(move || api::list(&app, &query, 25))
        .await
        .map_err(|e| AppError::Other(e.to_string()))?
}

#[tauri::command]
pub async fn mail_get(app: AppHandle, id: String) -> AppResult<MailDetail> {
    tauri::async_runtime::spawn_blocking(move || api::get(&app, &id))
        .await
        .map_err(|e| AppError::Other(e.to_string()))?
}

#[tauri::command]
pub async fn mail_mark_read(app: AppHandle, id: String, read: bool) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || api::mark_read(&app, &id, read))
        .await
        .map_err(|e| AppError::Other(e.to_string()))?
}

#[tauri::command]
pub async fn mail_unread_count(app: AppHandle) -> AppResult<i64> {
    tauri::async_runtime::spawn_blocking(move || api::unread_count(&app))
        .await
        .map_err(|e| AppError::Other(e.to_string()))?
}

#[tauri::command]
pub async fn config_has_google(app: AppHandle) -> AppResult<bool> {
    Ok(config::get_google_client_from(&app)?.is_some())
}

#[tauri::command]
pub async fn config_set_google(app: AppHandle, id: String, secret: String) -> AppResult<()> {
    config::set_google_client_for(&app, &id, &secret)
}
```

> 实现者：`api::connect(&app)` 内部完成「bind loopback→build_auth_url（用 config 的 client_id + redirect_uri）→opener 开浏览器→accept_code→exchange_code→save_refresh→设置 GmailState→profile_email→kv 写 gmail.email」，返回 `GmailStatus`。config 的读写按 Task 1 的实际函数名对齐（这里 `get_google_client_from`/`set_google_client_for` 仅示意，用 Task 1 真实命名）。

- [ ] **Step 3: 注册模块与命令**

- `src-tauri/src/commands/mod.rs` 加 `pub mod mail;`（字母序，`kv` 之后）。
- `src-tauri/src/gmail/mod.rs` 恢复/确认 `pub mod api;`。
- `src-tauri/src/lib.rs`：setup 中 `app.manage(gmail::GmailState::default());`；`generate_handler!` 注册 `gmail_status/gmail_connect/gmail_disconnect/mail_list/mail_search/mail_get/mail_mark_read/mail_unread_count/config_has_google/config_set_google`。

- [ ] **Step 4: 门禁**

Run: `cd src-tauri; cargo test; cargo clippy -- -D warnings`
Expected: 全过（含 api.rs 的纯函数单测）；clippy 干净。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/gmail src-tauri/src/commands src-tauri/src/lib.rs
git commit -m "feat(m5): gmail api + mail commands + registration"
```

---

## Task 5: 前端 API + 邮件页 + widget + 导航/勾选框

**Files:**
- Modify: `src/lib/api/index.ts`
- Create: `src/routes/(app)/mail/+page.svelte`
- Create: `src/routes/(widget)/widgets/mail/+page.svelte`
- Modify: `src/routes/(app)/+layout.svelte`、`src/routes/(app)/+page.svelte`

- [ ] **Step 1: api 封装**

在 `src/lib/api/index.ts` 追加类型与函数：

```ts
export interface MailSummary {
  id: string; from: string; subject: string; date: string; snippet: string; unread: boolean;
}
export interface MailDetail {
  id: string; from: string; to: string; subject: string; date: string;
  body: string; is_html: boolean; unread: boolean;
}
export interface GmailStatus { connected: boolean; email: string | null; }

export function gmailStatus(): Promise<GmailStatus> { return call<GmailStatus>("gmail_status"); }
export function gmailConnect(): Promise<GmailStatus> { return call<GmailStatus>("gmail_connect"); }
export function gmailDisconnect(): Promise<void> { return call<void>("gmail_disconnect"); }
export function mailList(): Promise<MailSummary[]> { return call<MailSummary[]>("mail_list"); }
export function mailSearch(query: string): Promise<MailSummary[]> { return call<MailSummary[]>("mail_search", { query }); }
export function mailGet(id: string): Promise<MailDetail> { return call<MailDetail>("mail_get", { id }); }
export function mailMarkRead(id: string, read: boolean): Promise<void> { return call<void>("mail_mark_read", { id, read }); }
export function mailUnreadCount(): Promise<number> { return call<number>("mail_unread_count"); }
export function configHasGoogle(): Promise<boolean> { return call<boolean>("config_has_google"); }
export function configSetGoogle(id: string, secret: string): Promise<void> { return call<void>("config_set_google", { id, secret }); }
```

并把 `WidgetVisibility` 接口加 `mail: boolean`，`widgetSetVisible` 的 kind 联合加 `"mail"`。

- [ ] **Step 2: 邮件页**

新建 `src/routes/(app)/mail/+page.svelte`：状态机——
1. `configHasGoogle()` 否 → 显示 Client ID/Secret 输入 + 保存（`configSetGoogle`）+ 申请步骤说明链接。
2. `gmailStatus()` 未连接 → 「连接 Gmail」按钮（`gmailConnect()`，await 后刷新）。
3. 已连接 → 顶栏邮箱 + 断开；搜索框（回车 `mailSearch`，空则 `mailList`）；左列表（未读加粗 + ●，点选 `mailGet` 并可自动 `mailMarkRead(id,true)`）；右详情（From/Subject/Date + 正文：`is_html` 用 `<iframe sandbox="" srcdoc={body}>`，否则 `<pre>`）；详情顶部「标已读/未读」。
（实现者按现有页面样式与 Svelte5 runes 写；HTML 正文务必用 `sandbox=""`（空 = 全部禁用，含脚本）。）

- [ ] **Step 3: 未读数 widget**

新建 `src/routes/(widget)/widgets/mail/+page.svelte`，仿 coins widget：`onMount` 调 `mailUnreadCount()`，每 5 分钟轮询；显示 `📧 {count}`，透明卡 + `data-tauri-drag-region`。

- [ ] **Step 4: 导航 + 勾选框**

- `src/routes/(app)/+layout.svelte` `<nav>` 加 `<a href="/mail">邮件 / Mail</a>`。
- `src/routes/(app)/+page.svelte`：仿 apps，加 `widgetMail` 状态 + onMount 读 `v.mail` + 「桌面邮件组件 / Mail widget」勾选框 + `toggleWidget("mail", ...)` 分支。

- [ ] **Step 5: 托盘开关**

`src-tauri/src/tray.rs` 仿 `toggle_apps` 加 `toggle_mail` 菜单项 + 事件分支 `spawn_toggle(app, "mail")`。

- [ ] **Step 6: 门禁**

Run: `npm run check`（前端）；`cd src-tauri; cargo clippy -- -D warnings`（托盘改动）。
Expected: 0 errors / 干净。

- [ ] **Step 7: Commit**

```bash
git add src/lib/api/index.ts "src/routes/(app)/mail/+page.svelte" "src/routes/(widget)/widgets/mail/+page.svelte" "src/routes/(app)/+layout.svelte" "src/routes/(app)/+page.svelte" src-tauri/src/tray.rs
git commit -m "feat(m5): mail page, unread widget, nav/tray/checkbox"
```

---

## Task 6: 全量验证

- [ ] **Step 1:** `cd src-tauri; cargo test` → 全过（PKCE/URL/redirect + api 纯函数）。
- [ ] **Step 2:** `cd src-tauri; cargo clippy -- -D warnings` → 干净。
- [ ] **Step 3:** `npm run check` → 0 errors。
- [ ] **Step 4: 报告 + 手动验证清单：**
  - 填 Client ID/Secret → 「连接 Gmail」→ 浏览器授权（Testing 模式 test user）→ 回到应用显示已连接邮箱
  - 收件箱列表加载、未读高亮、搜索
  - 点开邮件看正文、标记已读/未读
  - 邮件 widget 显示未读数；托盘/勾选框开关
  - 重启应用仍保持连接（refresh_token 自动刷新）

---

## 自检 / Self-Review 结论

- **Spec 覆盖：** OAuth PKCE+loopback(Task2/3)、token 刷新(Task3/4)、keyring(Task3)、列表/详情/搜索(Task4/5)、读写状态(Task4/5)、未读 widget(Task5)、主窗页(Task5)、client 凭据 config(Task1/4) —— 全覆盖；奖励金币按决策不在本轮。
- **占位符：** 纯函数与命令给完整代码；网络/API 部分给签名 + 要点 + 单测点（标注实现者补全 serde 结构，符合「外部依赖代码需编译迭代」原则）。
- **类型一致：** `MailSummary/MailDetail/GmailStatus` 前后端字段一致；`WidgetVisibility.mail` 在 models/window/read_visibility/lib-fallback/前端 五处补齐；命令名前后端一致。
- **依赖顺序：** Task2 临时注释 `pub mod api;` 保证可编译，Task4 恢复；纯函数先行可单测，网络后行；`AppError::Network` 变体已存在（M2 引入）。
- **spike 提示：** OAuth 往返建议实现时先用真实 Google 客户端打通 Task2/3 再继续。
