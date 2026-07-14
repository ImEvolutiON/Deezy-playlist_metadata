pub mod crypto;
pub mod download;
pub mod models;

use models::{AlbumResult, ArtistResult, PlaylistResult, SearchResult, UserInfo};
use reqwest::cookie::Jar;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, ACCEPT_LANGUAGE, CACHE_CONTROL, CONNECTION, USER_AGENT};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;

const API_URL: &str = "https://www.deezer.com/ajax/gw-light.php";
const LEGACY_API_URL: &str = "https://api.deezer.com";

#[derive(Clone)]
pub struct DeezerClient {
    pub http: reqwest::Client,
    pub arl: String,
    pub token: String,
    pub license_token: Option<String>,
    pub user: Option<UserInfo>,
}

mod auth;
mod catalog;
mod gateway;
mod media;


fn get_quality_code(quality: &str) -> u32 {
    match quality {
        "FLAC" => 9,
        "MP3_128" => 1,
        "MP3_256" => 5,
        "MP3_320" => 3,
        "MP4_RA1" => 13,
        "MP4_RA2" => 14,
        "MP4_RA3" => 15,
        _ => 3,
    }
}

fn fallback_qualities(quality: &str) -> &'static [&'static str] {
    match quality {
        "FLAC" => &["MP3_320", "MP3_128"],
        "MP3_320" => &["MP3_128"],
        _ => &[],
    }
}

pub fn get_quality_ext(quality: &str) -> &str {
    match quality {
        "FLAC" => ".flac",
        "MP3_128" | "MP3_256" | "MP3_320" => ".mp3",
        "MP4_RA1" | "MP4_RA2" | "MP4_RA3" => ".mp4",
        _ => ".mp3",
    }
}

fn extract_string_or_u64(val: &Value) -> Option<String> {
    val.as_str()
        .map(|s| s.to_string())
        .or_else(|| val.as_u64().map(|n| n.to_string()))
        .or_else(|| val.as_i64().map(|n| n.to_string()))
}
