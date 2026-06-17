//! Shared data structures.

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Todo {
    pub id: i64,
    pub title: String,
    pub note: Option<String>,
    pub done: bool,
    pub due_date: Option<String>,
    pub reward_coin: i64,
    pub created_at: String,
    pub done_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GameProfile {
    pub coins: i64,
    pub exp: i64,
    pub level: i64,
    pub last_tick: String,
}

#[derive(Debug, Serialize)]
pub struct ToggleResult {
    pub todo: Todo,
    pub awarded: i64,
    pub coins: i64,
}

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

#[derive(Debug, Serialize)]
pub struct WidgetVisibility {
    pub todo: bool,
    pub coins: bool,
    pub apps: bool,
    pub mail: bool,
}

#[derive(Debug, Serialize)]
pub struct AppEntry {
    pub id: i64,
    pub name: String,
    pub target: String,
    pub args: Option<String>,
}

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
