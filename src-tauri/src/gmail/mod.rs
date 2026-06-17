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
