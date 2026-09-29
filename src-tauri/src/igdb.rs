//! IGDB (Twitch) game identity — covers and metadata.
//!
//! Discord “Playing …” detection is a separate thing: it matches running `.exe`
//! names against Discord’s own detectable-apps list. IGDB is the actual game
//! catalog (same one Twitch uses): one record per title, with Steam/GOG/Epic
//! IDs, box art, genres, and companies.

use crate::models::AppSettings;
use anyhow::Result;
use chrono::{Datelike, TimeZone, Utc};
use serde_json::Value;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const TOKEN_SKEW: Duration = Duration::from_secs(60);
const MIN_GAP: Duration = Duration::from_millis(280);

struct TokenCache {
    client_id: String,
    access_token: String,
    expires_at: Instant,
}

struct Gate {
    token: Option<TokenCache>,
    last_call: Option<Instant>,
}

static GATE: Mutex<Gate> = Mutex::new(Gate {
    token: None,
    last_call: None,
});

#[derive(Clone)]
pub struct IgdbAuth {
    pub client_id: String,
    pub client_secret: String,
}

impl IgdbAuth {
    pub fn from_settings(settings: &AppSettings) -> Option<Self> {
        let client_id = settings.twitch_client_id.as_deref()?.trim();
        let client_secret = settings.twitch_client_secret.as_deref()?.trim();
        if client_id.is_empty() || client_secret.is_empty() {
            return None;
        }
        Some(Self {
            client_id: client_id.to_string(),
            client_secret: client_secret.to_string(),
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct IgdbGame {
    pub name: String,
    pub cover_image_id: Option<String>,
    pub summary: Option<String>,
    pub first_release_date: Option<i64>,
    pub genres: Vec<String>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub steam_app_id: Option<String>,
}

impl IgdbGame {
    pub fn cover_url(&self) -> Option<String> {
        self.cover_image_id.as_deref().map(cover_cdn_url)
    }
}

pub fn cover_cdn_url(image_id: &str) -> String {
    format!("https://images.igdb.com/igdb/image/upload/t_cover_big_2x/{image_id}.jpg")
}

pub fn lookup(auth: &IgdbAuth, name: &str, steam_id: Option<&str>) -> Result<Option<IgdbGame>> {
    if let Some(id) = steam_id.map(str::trim).filter(|s| !s.is_empty()) {
        if let Some(hit) = by_steam_id(auth, id)? {
            return Ok(Some(hit));
        }
    }
    by_name(auth, name)
}

fn client() -> Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent("IntelGenGameLauncher/0.3 (igdb)")
        .build()?)
}

fn throttle() {
    let mut gate = GATE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(last) = gate.last_call {
        let wait = MIN_GAP.saturating_sub(last.elapsed());
        if !wait.is_zero() {
            drop(gate);
            std::thread::sleep(wait);
            gate = GATE.lock().unwrap_or_else(|e| e.into_inner());
        }
    }
    gate.last_call = Some(Instant::now());
}

fn token(auth: &IgdbAuth) -> Result<String> {
    {
        let gate = GATE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(cached) = &gate.token {
            if cached.client_id == auth.client_id && cached.expires_at > Instant::now() {
                return Ok(cached.access_token.clone());
            }
        }
    }

    throttle();
    let url = format!(
        "https://id.twitch.tv/oauth2/token?client_id={}&client_secret={}&grant_type=client_credentials",
        urlencoding::encode(&auth.client_id),
        urlencoding::encode(&auth.client_secret)
    );
    let resp: Value = client()?.post(url).send()?.error_for_status()?.json()?;
    let access = resp
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("IGDB token missing"))?
        .to_string();
    let expires_in = resp
        .get("expires_in")
        .and_then(|v| v.as_u64())
        .unwrap_or(3600);
    let ttl = Duration::from_secs(expires_in).saturating_sub(TOKEN_SKEW);

    let mut gate = GATE.lock().unwrap_or_else(|e| e.into_inner());
    gate.token = Some(TokenCache {
        client_id: auth.client_id.clone(),
        access_token: access.clone(),
        expires_at: Instant::now() + ttl,
    });
    Ok(access)
}

fn igdb_post(auth: &IgdbAuth, endpoint: &str, body: &str) -> Result<Value> {
    let access = token(auth)?;
    throttle();
    let url = format!("https://api.igdb.com/v4/{endpoint}");
    let resp = client()?
        .post(url)
        .header("Client-ID", &auth.client_id)
        .header("Authorization", format!("Bearer {access}"))
        .header("Accept", "application/json")
        .body(body.to_string())
        .send()?;
    if !resp.status().is_success() {
        anyhow::bail!("IGDB {endpoint} {}", resp.status());
    }
    Ok(resp.json()?)
}

fn by_steam_id(auth: &IgdbAuth, steam_id: &str) -> Result<Option<IgdbGame>> {
    let uid = steam_id.replace('"', "");
    if uid.is_empty() || !uid.chars().all(|c| c.is_ascii_digit()) {
        return Ok(None);
    }
    let body = format!(
        r#"fields game.name,game.cover.image_id,game.summary,game.first_release_date,game.genres.name,game.involved_companies.company.name,game.involved_companies.developer,game.involved_companies.publisher,game.external_games.uid,game.external_games.category;
where uid = "{uid}" & category = 1;
limit 1;"#
    );
    let json = igdb_post(auth, "external_games", &body)?;
    let Some(game) = json
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|row| row.get("game"))
    else {
        return Ok(None);
    };
    Ok(parse_game(game))
}

fn by_name(auth: &IgdbAuth, name: &str) -> Result<Option<IgdbGame>> {
    let q = name.trim();
    if q.len() < 2 {
        return Ok(None);
    }
    let escaped = q.replace('"', "");
    let body = format!(
        r#"search "{escaped}";
fields name,cover.image_id,summary,first_release_date,genres.name,involved_companies.company.name,involved_companies.developer,involved_companies.publisher,external_games.uid,external_games.category;
where version_parent = null & category = (0,8,9,10);
limit 8;"#
    );
    let json = igdb_post(auth, "games", &body)?;
    let Some(arr) = json.as_array() else {
        return Ok(None);
    };
    let needle = compact(q);
    let mut best: Option<(u32, IgdbGame)> = None;
    for row in arr {
        let Some(hit) = parse_game(row) else {
            continue;
        };
        let score = name_score(&needle, &compact(&hit.name));
        if score == 0 {
            continue;
        }
        if best.as_ref().map(|(s, _)| score > *s).unwrap_or(true) {
            best = Some((score, hit));
        }
    }
    Ok(best.map(|(_, g)| g))
}

fn parse_game(v: &Value) -> Option<IgdbGame> {
    let name = v.get("name")?.as_str()?.trim().to_string();
    if name.is_empty() {
        return None;
    }
    let cover_image_id = v
        .pointer("/cover/image_id")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string());
    let summary = v
        .get("summary")
        .and_then(|x| x.as_str())
        .map(clip_summary)
        .filter(|s| !s.is_empty());
    let first_release_date = v.get("first_release_date").and_then(|x| x.as_i64());
    let genres = v
        .get("genres")
        .and_then(|x| x.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|g| g.get("name").and_then(|n| n.as_str()))
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let mut developer = None;
    let mut publisher = None;
    if let Some(cos) = v.get("involved_companies").and_then(|x| x.as_array()) {
        for c in cos {
            let cname = c
                .pointer("/company/name")
                .and_then(|x| x.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            let Some(cname) = cname else { continue };
            if c.get("developer").and_then(|x| x.as_bool()) == Some(true) && developer.is_none() {
                developer = Some(cname.clone());
            }
            if c.get("publisher").and_then(|x| x.as_bool()) == Some(true) && publisher.is_none() {
                publisher = Some(cname);
            }
        }
    }

    let steam_app_id = v
        .get("external_games")
        .and_then(|x| x.as_array())
        .and_then(|arr| {
            arr.iter().find_map(|eg| {
                let cat = eg.get("category").and_then(|c| c.as_i64())?;
                if cat != 1 {
                    return None;
                }
                eg.get("uid")
                    .and_then(|u| u.as_str())
                    .filter(|s| s.chars().all(|c| c.is_ascii_digit()))
                    .map(|s| s.to_string())
            })
        });

    Some(IgdbGame {
        name,
        cover_image_id,
        summary,
        first_release_date,
        genres,
        developer,
        publisher,
        steam_app_id,
    })
}

fn clip_summary(s: &str) -> String {
    let t = s.trim();
    if t.chars().count() <= 800 {
        return t.to_string();
    }
    t.chars().take(797).collect::<String>() + "…"
}

pub fn release_year(unix: i64) -> Option<i64> {
    if unix <= 0 {
        return None;
    }
    Utc.timestamp_opt(unix, 0)
        .single()
        .map(|d| i64::from(d.year()))
}

fn compact(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

fn name_score(needle: &str, hay: &str) -> u32 {
    if needle.is_empty() || hay.is_empty() {
        return 0;
    }
    if needle == hay {
        return 100;
    }
    if hay.starts_with(needle) || needle.starts_with(hay) {
        return 80;
    }
    if hay.contains(needle) || needle.contains(hay) {
        return 50;
    }
    0
}
