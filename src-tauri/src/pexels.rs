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
    let text = resp.text().map_err(|e| AppError::Network(e.to_string()))?;
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
