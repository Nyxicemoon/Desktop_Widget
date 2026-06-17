//! Gmail REST calls (reqwest blocking) + the OAuth `connect` orchestration.
//! Access token is refreshed on demand via `auth::refresh_access`; a single
//! 401 retry forces a refresh. MIME body selection is unit-tested.

use crate::config;
use crate::db::{kv, Db};
use crate::error::{AppError, AppResult};
use crate::gmail::auth;
use crate::gmail::{AccessToken, GmailState};
use crate::models::{GmailStatus, MailDetail, MailSummary};
use serde::Deserialize;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

const BASE: &str = "https://gmail.googleapis.com/gmail/v1/users/me";

// ---------- response shapes ----------

#[derive(Deserialize)]
struct ListResp {
    #[serde(default)]
    messages: Vec<MsgRef>,
}
#[derive(Deserialize)]
struct MsgRef {
    id: String,
}
#[derive(Deserialize)]
struct MsgResp {
    #[serde(default)]
    snippet: Option<String>,
    #[serde(default, rename = "labelIds")]
    label_ids: Vec<String>,
    #[serde(default)]
    payload: Payload,
}
#[derive(Deserialize, Default)]
struct Payload {
    #[serde(default, rename = "mimeType")]
    mime_type: String,
    #[serde(default)]
    headers: Vec<Header>,
    #[serde(default)]
    body: Body,
    #[serde(default)]
    parts: Vec<Payload>,
}
#[derive(Deserialize, Default)]
struct Body {
    #[serde(default)]
    data: Option<String>,
}
#[derive(Deserialize, Clone)]
struct Header {
    name: String,
    value: String,
}
#[derive(Deserialize)]
struct LabelResp {
    #[serde(default, rename = "messagesUnread")]
    messages_unread: i64,
}
#[derive(Deserialize)]
struct ProfileResp {
    #[serde(rename = "emailAddress")]
    email_address: String,
}

// ---------- pure helpers (unit-tested) ----------

fn header(headers: &[Header], name: &str) -> String {
    headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case(name))
        .map(|h| h.value.clone())
        .unwrap_or_default()
}

fn decode_b64url(s: &str) -> String {
    use base64::Engine;
    let cleaned: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(cleaned.trim_end_matches('='))
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(&cleaned))
        .unwrap_or_default();
    String::from_utf8_lossy(&bytes).to_string()
}

/// Choose a displayable body: prefer text/plain, else text/html. Returns (text, is_html).
fn pick_body(p: &Payload) -> Option<(String, bool)> {
    if p.parts.is_empty() {
        return p.body.data.as_ref().map(|d| {
            let is_html = p.mime_type.contains("html");
            (decode_b64url(d), is_html)
        });
    }
    let mut html: Option<String> = None;
    for part in &p.parts {
        if let Some((txt, is_html)) = pick_body(part) {
            if !is_html {
                return Some((txt, false));
            }
            if html.is_none() {
                html = Some(txt);
            }
        }
    }
    html.map(|h| (h, true))
}

// ---------- token plumbing ----------

fn client_creds(app: &AppHandle) -> AppResult<(String, String)> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Io(e.to_string()))?;
    let cfg = config::load(&dir)?;
    match (cfg.google_client_id, cfg.google_client_secret) {
        (Some(id), Some(secret)) if !id.is_empty() && !secret.is_empty() => Ok((id, secret)),
        _ => Err(AppError::Other(
            "缺少 Google 客户端配置 / Google client not configured".into(),
        )),
    }
}

fn set_token(app: &AppHandle, access: String, expires_in: u64) -> AppResult<()> {
    let tok = AccessToken {
        value: access,
        expires_at: Instant::now() + Duration::from_secs(expires_in.saturating_sub(60)),
    };
    *app.state::<GmailState>()
        .0
        .lock()
        .map_err(|e| AppError::Other(e.to_string()))? = Some(tok);
    Ok(())
}

fn invalidate(app: &AppHandle) {
    if let Ok(mut g) = app.state::<GmailState>().0.lock() {
        *g = None;
    }
}

/// Return a non-expired access token, refreshing via the stored refresh_token if needed.
fn valid_access(app: &AppHandle) -> AppResult<String> {
    {
        let st = app.state::<GmailState>();
        let guard = st.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
        if let Some(tok) = guard.as_ref() {
            if tok.expires_at > Instant::now() {
                return Ok(tok.value.clone());
            }
        }
    }
    let (id, secret) = client_creds(app)?;
    let refresh = auth::load_refresh()?
        .ok_or_else(|| AppError::NotFound("gmail not connected".into()))?;
    let (access, expires_in) = auth::refresh_access(&id, &secret, &refresh)?;
    set_token(app, access.clone(), expires_in)?;
    Ok(access)
}

/// Send a bearer-authed request; on 401 force one refresh + retry.
fn send_authed<F>(app: &AppHandle, build: F) -> AppResult<reqwest::blocking::Response>
where
    F: Fn(&str) -> reqwest::blocking::RequestBuilder,
{
    let token = valid_access(app)?;
    let resp = build(&token)
        .send()
        .map_err(|e| AppError::Network(e.to_string()))?;
    let resp = if resp.status().as_u16() == 401 {
        invalidate(app);
        let t2 = valid_access(app)?;
        build(&t2)
            .send()
            .map_err(|e| AppError::Network(e.to_string()))?
    } else {
        resp
    };
    resp.error_for_status()
        .map_err(|e| AppError::Network(e.to_string()))
}

// ---------- OAuth connect ----------

pub fn connect(app: &AppHandle) -> AppResult<GmailStatus> {
    use tauri_plugin_opener::OpenerExt;

    let (id, secret) = client_creds(app)?;
    let verifier = auth::random_token(48);
    let challenge = auth::code_challenge(&verifier);
    let state = auth::random_token(16);

    let (listener, redirect_uri) = auth::bind_loopback()?;
    let url = auth::build_auth_url(&id, &redirect_uri, &challenge, &state);
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| AppError::Other(e.to_string()))?;

    let code = auth::accept_code(&listener, &state)?;
    let (access, refresh, expires_in) =
        auth::exchange_code(&id, &secret, &code, &verifier, &redirect_uri)?;
    auth::save_refresh(&refresh)?;
    set_token(app, access, expires_in)?;

    let email = profile_email(app)?;
    {
        let db = app.state::<Db>();
        let conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
        kv::set(&conn, "gmail.email", &email)?;
    }
    Ok(GmailStatus {
        connected: true,
        email: Some(email),
    })
}

// ---------- REST ----------

pub fn profile_email(app: &AppHandle) -> AppResult<String> {
    let client = reqwest::blocking::Client::new();
    let url = format!("{BASE}/profile");
    let resp = send_authed(app, |t| client.get(&url).bearer_auth(t))?;
    let p: ProfileResp = resp.json().map_err(|e| AppError::Network(e.to_string()))?;
    Ok(p.email_address)
}

pub fn list(app: &AppHandle, query: &str, max: u32) -> AppResult<Vec<MailSummary>> {
    let client = reqwest::blocking::Client::new();
    let url = format!("{BASE}/messages");
    let max_s = max.to_string();
    let resp = send_authed(app, |t| {
        let mut q: Vec<(&str, &str)> = vec![("maxResults", &max_s), ("labelIds", "INBOX")];
        if !query.is_empty() {
            q.push(("q", query));
        }
        client.get(&url).query(&q).bearer_auth(t)
    })?;
    let list: ListResp = resp.json().map_err(|e| AppError::Network(e.to_string()))?;
    let mut out = Vec::new();
    for m in list.messages {
        out.push(summary(app, &client, &m.id)?);
    }
    Ok(out)
}

fn summary(app: &AppHandle, client: &reqwest::blocking::Client, id: &str) -> AppResult<MailSummary> {
    let url = format!("{BASE}/messages/{id}");
    let resp = send_authed(app, |t| {
        client
            .get(&url)
            .query(&[
                ("format", "metadata"),
                ("metadataHeaders", "From"),
                ("metadataHeaders", "Subject"),
                ("metadataHeaders", "Date"),
            ])
            .bearer_auth(t)
    })?;
    let msg: MsgResp = resp.json().map_err(|e| AppError::Network(e.to_string()))?;
    Ok(MailSummary {
        id: id.to_string(),
        from: header(&msg.payload.headers, "From"),
        subject: header(&msg.payload.headers, "Subject"),
        date: header(&msg.payload.headers, "Date"),
        snippet: msg.snippet.unwrap_or_default(),
        unread: msg.label_ids.iter().any(|l| l == "UNREAD"),
    })
}

pub fn get(app: &AppHandle, id: &str) -> AppResult<MailDetail> {
    let client = reqwest::blocking::Client::new();
    let url = format!("{BASE}/messages/{id}");
    let resp = send_authed(app, |t| {
        client.get(&url).query(&[("format", "full")]).bearer_auth(t)
    })?;
    let msg: MsgResp = resp.json().map_err(|e| AppError::Network(e.to_string()))?;
    let (body, is_html) =
        pick_body(&msg.payload).unwrap_or_else(|| (msg.snippet.clone().unwrap_or_default(), false));
    Ok(MailDetail {
        id: id.to_string(),
        from: header(&msg.payload.headers, "From"),
        to: header(&msg.payload.headers, "To"),
        subject: header(&msg.payload.headers, "Subject"),
        date: header(&msg.payload.headers, "Date"),
        body,
        is_html,
        unread: msg.label_ids.iter().any(|l| l == "UNREAD"),
    })
}

pub fn mark_read(app: &AppHandle, id: &str, read: bool) -> AppResult<()> {
    let client = reqwest::blocking::Client::new();
    let url = format!("{BASE}/messages/{id}/modify");
    let body = if read {
        serde_json::json!({ "removeLabelIds": ["UNREAD"] })
    } else {
        serde_json::json!({ "addLabelIds": ["UNREAD"] })
    };
    send_authed(app, |t| client.post(&url).bearer_auth(t).json(&body))?;
    Ok(())
}

pub fn unread_count(app: &AppHandle) -> AppResult<i64> {
    let client = reqwest::blocking::Client::new();
    let url = format!("{BASE}/labels/INBOX");
    let resp = send_authed(app, |t| client.get(&url).bearer_auth(t))?;
    let l: LabelResp = resp.json().map_err(|e| AppError::Network(e.to_string()))?;
    Ok(l.messages_unread)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(name: &str, value: &str) -> Header {
        Header {
            name: name.into(),
            value: value.into(),
        }
    }

    #[test]
    fn header_is_case_insensitive() {
        let hs = vec![h("From", "a@b.com"), h("Subject", "Hi")];
        assert_eq!(header(&hs, "from"), "a@b.com");
        assert_eq!(header(&hs, "SUBJECT"), "Hi");
        assert_eq!(header(&hs, "Missing"), "");
    }

    #[test]
    fn decode_b64url_decodes_unpadded() {
        // "Hello" -> base64url "SGVsbG8"
        assert_eq!(decode_b64url("SGVsbG8"), "Hello");
    }

    #[test]
    fn pick_body_prefers_plain_over_html() {
        let payload = Payload {
            mime_type: "multipart/alternative".into(),
            parts: vec![
                Payload {
                    mime_type: "text/html".into(),
                    body: Body {
                        data: Some("PGI-aGk8L2I-".into()), // "<b>hi</b>"
                    },
                    ..Default::default()
                },
                Payload {
                    mime_type: "text/plain".into(),
                    body: Body {
                        data: Some("aGVsbG8".into()), // "hello"
                    },
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let (txt, is_html) = pick_body(&payload).unwrap();
        assert_eq!(txt, "hello");
        assert!(!is_html);
    }

    #[test]
    fn pick_body_falls_back_to_html() {
        let payload = Payload {
            mime_type: "text/html".into(),
            body: Body {
                data: Some("PGI-aGk8L2I-".into()),
            },
            ..Default::default()
        };
        let (txt, is_html) = pick_body(&payload).unwrap();
        assert_eq!(txt, "<b>hi</b>");
        assert!(is_html);
    }
}
