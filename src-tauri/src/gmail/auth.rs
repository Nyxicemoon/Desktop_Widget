//! Gmail OAuth2 (Authorization Code + PKCE, loopback redirect).
//!
//! Pure helpers (PKCE, auth URL, redirect parsing) are unit-tested. Network
//! (token exchange/refresh) and the loopback server are integration-only.
//! refresh_token lives in the OS credential store (keyring); access_token is
//! held in memory by `GmailState`.

use crate::error::{AppError, AppResult};
use base64::Engine;
use serde::Deserialize;

const AUTH_ENDPOINT: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";
const SCOPE: &str = "https://www.googleapis.com/auth/gmail.modify";

const KR_SERVICE: &str = "com.deskhub.app";
const KR_ACCOUNT: &str = "gmail-refresh";

// ---------- pure helpers ----------

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

/// Random base64url string from `n` random bytes (for verifier / state).
pub fn random_token(n: usize) -> String {
    let mut buf = vec![0u8; n];
    getrandom::getrandom(&mut buf).expect("getrandom");
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&buf)
}

/// Build the Google authorization URL.
pub fn build_auth_url(client_id: &str, redirect_uri: &str, challenge: &str, state: &str) -> String {
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

// ---------- keyring (refresh token) ----------

fn kr_entry() -> AppResult<keyring::Entry> {
    keyring::Entry::new(KR_SERVICE, KR_ACCOUNT).map_err(|e| AppError::Other(e.to_string()))
}

pub fn save_refresh(token: &str) -> AppResult<()> {
    kr_entry()?
        .set_password(token)
        .map_err(|e| AppError::Other(e.to_string()))
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

// ---------- loopback ----------

/// Bind a loopback listener on a random port; returns (listener, redirect_uri).
pub fn bind_loopback() -> AppResult<(std::net::TcpListener, String)> {
    let listener =
        std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| AppError::Io(e.to_string()))?;
    let port = listener
        .local_addr()
        .map_err(|e| AppError::Io(e.to_string()))?
        .port();
    Ok((listener, format!("http://127.0.0.1:{port}")))
}

/// Accept a single redirect request, validate state, return the auth code.
pub fn accept_code(listener: &std::net::TcpListener, expected_state: &str) -> AppResult<String> {
    use std::io::{Read, Write};

    let (mut stream, _) = listener.accept().map_err(|e| AppError::Io(e.to_string()))?;
    let mut buf = [0u8; 4096];
    let n = stream.read(&mut buf).map_err(|e| AppError::Io(e.to_string()))?;
    let req = String::from_utf8_lossy(&buf[..n]);
    let first = req.lines().next().unwrap_or("");
    let (code, state) = parse_redirect(first)?;

    let body = "<html><body style='font-family:sans-serif'>DeskHub 已连接，可关闭本页。<br/>Connected — you may close this tab.</body></html>";
    let resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(resp.as_bytes());

    if state != expected_state {
        return Err(AppError::Other("state mismatch".into()));
    }
    Ok(code)
}

// ---------- token exchange / refresh ----------

#[derive(Deserialize)]
struct TokenResp {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    expires_in: u64,
}

/// Exchange an authorization code for tokens. Returns (access, refresh, expires_in_secs).
pub fn exchange_code(
    client_id: &str,
    client_secret: &str,
    code: &str,
    verifier: &str,
    redirect_uri: &str,
) -> AppResult<(String, String, u64)> {
    let resp: TokenResp = reqwest::blocking::Client::new()
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

/// Refresh the access token using a stored refresh_token. Returns (access, expires_in_secs).
pub fn refresh_access(
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
) -> AppResult<(String, u64)> {
    let resp: TokenResp = reqwest::blocking::Client::new()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_challenge_matches_rfc7636_vector() {
        // RFC 7636 Appendix B.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(
            code_challenge(verifier),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
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
        assert!(t
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }
}
