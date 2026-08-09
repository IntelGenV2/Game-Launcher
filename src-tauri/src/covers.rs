use crate::db::covers_dir;
use crate::models::{Game, Store};
use anyhow::Result;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Result of a successful cover fetch (may also resolve a Steam AppID).
pub struct CoverFetch {
    pub path: String,
    pub steam_app_id: Option<String>,
}

/// Cover selection rules (always enforced):
/// 1. Prefer vertical box / library art (~2:3 portrait); square box art is OK.
/// 2. Never use headers, heroes, page backgrounds, screenshots, or wide capsules.
/// 3. After download, reject landscape images (clearly wider than tall).
pub fn ensure_cover(game: &Game, api_key: Option<&str>) -> Result<Option<CoverFetch>> {
    if let Some(path) = &game.cover_path {
        if PathBuf::from(path).exists() && is_valid_portrait_file(path) {
            return Ok(Some(CoverFetch {
                path: path.clone(),
                steam_app_id: None,
            }));
        }
        // Stale landscape / background art — replace it
        let _ = fs::remove_file(path);
    }

    fs::create_dir_all(covers_dir())?;
    let dest = cover_dest_for(&game.id, "jpg");
    if dest.exists() {
        if is_valid_portrait_file(&dest) {
            return Ok(Some(CoverFetch {
                path: dest.to_string_lossy().to_string(),
                steam_app_id: None,
            }));
        }
        let _ = fs::remove_file(&dest);
    }

    let search_names = search_name_variants(&game.name);
    let mut resolved_steam: Option<String> = None;

    // 1) Stored cover_url only if it looks like box art (not a background URL)
    if let Some(url) = &game.cover_url {
        if is_box_art_url(url) {
            if try_save_portrait(url, &dest)? {
                return Ok(Some(done(&dest, None)));
            }
        }
    }

    // 2) Steam library capsule (known ID → stored ID → name search with normalized variants)
    let mut steam_id = game.steam_app_id.clone();
    if steam_id.is_none() {
        steam_id = known_steam_app_id(&game.name).map(|s| s.to_string());
    }
    if steam_id.is_none() {
        for n in &search_names {
            if let Some(id) = steam_store_search(n).ok().flatten() {
                steam_id = Some(id);
                break;
            }
        }
    }
    if let Some(app_id) = steam_id.clone() {
        resolved_steam = Some(app_id.clone());
        for url in steam_library_capsule_urls(&app_id)? {
            if try_save_portrait(&url, &dest)? {
                return Ok(Some(done(&dest, resolved_steam)));
            }
        }
        for suffix in [
            "library_600x900_2x.jpg",
            "library_600x900.jpg",
            "portrait.png",
        ] {
            for base in [
                format!("https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/{app_id}/{suffix}"),
                format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/{suffix}"),
                format!("https://shared.fastly.steamstatic.com/store_item_assets/steam/apps/{app_id}/{suffix}"),
            ] {
                if try_save_portrait(&base, &dest)? {
                    return Ok(Some(done(&dest, resolved_steam)));
                }
            }
        }
    }

    // 3) Epic tall launcher art (Epic games + name matches like Fortnite)
    if matches!(game.store, Store::Epic)
        || game.id.starts_with("epic:")
        || name_looks_like_epic(&game.name)
    {
        if let Some(url) = epic_product_cover(&game.name, &game.launch_target)? {
            if try_save_portrait(&url, &dest)? {
                return Ok(Some(done(&dest, resolved_steam)));
            }
        }
    }

    // 4) Microsoft Store display catalog (Xbox / Game Pass titles)
    if matches!(game.store, Store::Xbox) || game.id.starts_with("xbox:") {
        for n in &search_names {
            if let Some(url) = microsoft_store_cover(n)? {
                if try_save_portrait(&url, &dest)? {
                    return Ok(Some(done(&dest, resolved_steam)));
                }
            }
        }
    }

    // 5) SteamGridDB vertical grids
    if let Some(key) = api_key {
        for n in &search_names {
            if let Some(url) = steamgriddb_cover(n, key)? {
                if try_save_portrait(&url, &dest)? {
                    return Ok(Some(done(&dest, resolved_steam)));
                }
            }
        }
    }

    // 6) Wikipedia cover (must pass shape check)
    for n in &search_names {
        if let Some(url) = wikipedia_cover(n)? {
            if is_box_art_url(&url) && try_save_portrait(&url, &dest)? {
                return Ok(Some(done(&dest, resolved_steam)));
            }
        }
    }

    // 7) Roblox brand / game icon (square OK)
    if matches!(game.store, Store::Roblox) || compact_alnum(&game.name) == "roblox" {
        for url in roblox_cover_urls() {
            if try_save_portrait(url, &dest)? {
                return Ok(Some(done(&dest, resolved_steam)));
            }
        }
    }

    Ok(None)
}

fn done(dest: &Path, steam_app_id: Option<String>) -> CoverFetch {
    CoverFetch {
        path: dest.to_string_lossy().to_string(),
        steam_app_id,
    }
}

pub fn import_cover_file(game_id: &str, source_path: &str) -> Result<String> {
    let src = Path::new(source_path);
    if !src.exists() {
        anyhow::bail!("Cover file not found");
    }
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("jpg")
        .to_lowercase();
    let ext = match ext.as_str() {
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" => ext,
        _ => "jpg".into(),
    };

    let dir = covers_dir();
    fs::create_dir_all(&dir)?;
    let dest = cover_dest_for(game_id, &ext);
    let safe = safe_id(game_id);
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(&safe) {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
    fs::copy(src, &dest)?;
    Ok(dest.to_string_lossy().to_string())
}

fn try_save_portrait(url: &str, dest: &Path) -> Result<bool> {
    if is_background_url(url) {
        return Ok(false);
    }
    let bytes = match download_bytes(url) {
        Ok(b) => b,
        Err(_) => return Ok(false),
    };
    if bytes.len() < 800 {
        return Ok(false);
    }
    if !is_portrait_bytes(&bytes) {
        return Ok(false);
    }
    let mut file = fs::File::create(dest)?;
    file.write_all(&bytes)?;
    Ok(true)
}

fn is_box_art_url(url: &str) -> bool {
    !is_background_url(url)
}

fn is_background_url(url: &str) -> bool {
    let u = url.to_lowercase();
    let banned = [
        "header.jpg",
        "header_2x",
        "library_hero",
        "page_bg",
        "page_background",
        "background",
        "screenshot",
        "gameplay",
        "capsule_616",
        "capsule_467",
        "capsule_231",
        "capsule_sm",
        "main_capsule",
        "hero_capsule", // store vertical sale capsule — often not box art; still ok-ish but prefer library
        "banner",
        "blade-2560",
        "blade-1920",
        "2560x1440",
        "1920x1080",
        "3840x2160",
    ];
    banned.iter().any(|b| u.contains(b))
}

fn is_valid_portrait_file(path: impl AsRef<Path>) -> bool {
    let Ok(bytes) = fs::read(path.as_ref()) else {
        return false;
    };
    bytes.len() > 800 && is_portrait_bytes(&bytes)
}

fn is_portrait_bytes(bytes: &[u8]) -> bool {
    match image_dimensions(bytes) {
        Some((w, h)) if w > 0 && h > 0 => {
            // Accept portrait and near-square box art; reject landscape banners/headers.
            (h as f32) >= (w as f32) * 0.95
        }
        _ => false,
    }
}

fn image_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    // PNG
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") && bytes.len() >= 24 {
        let w = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
        let h = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
        return Some((w, h));
    }
    // JPEG — find SOF0/SOF2
    if bytes.len() > 4 && bytes[0] == 0xFF && bytes[1] == 0xD8 {
        let mut i = 2usize;
        while i + 9 < bytes.len() {
            if bytes[i] != 0xFF {
                i += 1;
                continue;
            }
            let marker = bytes[i + 1];
            if marker == 0xD9 || marker == 0xDA {
                break;
            }
            if i + 3 >= bytes.len() {
                break;
            }
            let len = u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
            // SOF0..SOF3, SOF5..SOF7, SOF9..SOF11, SOF13..SOF15
            if matches!(
                marker,
                0xC0 | 0xC1
                    | 0xC2
                    | 0xC3
                    | 0xC5
                    | 0xC6
                    | 0xC7
                    | 0xC9
                    | 0xCA
                    | 0xCB
                    | 0xCD
                    | 0xCE
                    | 0xCF
            ) && i + 8 < bytes.len()
            {
                let h = u16::from_be_bytes([bytes[i + 5], bytes[i + 6]]) as u32;
                let w = u16::from_be_bytes([bytes[i + 7], bytes[i + 8]]) as u32;
                return Some((w, h));
            }
            if len < 2 {
                break;
            }
            i += 2 + len;
        }
    }
    // WebP (VP8X / VP8 )
    if bytes.starts_with(b"RIFF") && bytes.len() > 30 && &bytes[8..12] == b"WEBP" {
        if &bytes[12..16] == b"VP8X" && bytes.len() >= 30 {
            let w = 1 + u32::from_le_bytes([bytes[24], bytes[25], bytes[26], 0]);
            let h = 1 + u32::from_le_bytes([bytes[27], bytes[28], bytes[29], 0]);
            return Some((w, h));
        }
    }
    None
}

fn cover_dest_for(game_id: &str, ext: &str) -> PathBuf {
    covers_dir().join(format!("{}.{}", safe_id(game_id), ext))
}

fn safe_id(game_id: &str) -> String {
    game_id.replace([':', '/', '\\'], "_")
}

fn client() -> Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(25))
        .user_agent("IntelGenGameLauncher/0.2 (cover-fetch)")
        .build()?)
}

fn download_bytes(url: &str) -> Result<Vec<u8>> {
    let resp = client()?.get(url).send()?;
    if !resp.status().is_success() {
        anyhow::bail!("HTTP {}", resp.status());
    }
    Ok(resp.bytes()?.to_vec())
}

/// Official Steam library capsule URLs (portrait box art only — never header/hero/bg).
fn steam_library_capsule_urls(app_id: &str) -> Result<Vec<String>> {
    let input = serde_json::json!({
        "ids": [{ "appid": app_id.parse::<u64>().unwrap_or(0) }],
        "context": { "language": "english", "country_code": "US" },
        "data_request": { "include_assets": true }
    });
    let url = format!(
        "https://api.steampowered.com/IStoreBrowseService/GetItems/v1/?input_json={}",
        urlencoding::encode(&input.to_string())
    );
    let json: serde_json::Value = client()?.get(&url).send()?.json()?;
    let assets = json
        .pointer("/response/store_items/0/assets")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let fmt = assets
        .get("asset_url_format")
        .and_then(|v| v.as_str())
        .unwrap_or("steam/apps/{appid}/${FILENAME}");

    let mut out = Vec::new();
    for key in ["library_capsule_2x", "library_capsule"] {
        if let Some(file) = assets.get(key).and_then(|v| v.as_str()) {
            // Skip if Steam ever points this key at a banned asset name
            if is_background_url(file) {
                continue;
            }
            let path = fmt.replace("${FILENAME}", file).replace("{appid}", app_id);
            for host in [
                "https://shared.akamai.steamstatic.com/store_item_assets/",
                "https://shared.fastly.steamstatic.com/store_item_assets/",
                "https://cdn.cloudflare.steamstatic.com/",
            ] {
                out.push(format!("{host}{path}"));
            }
        }
    }
    Ok(out)
}

fn steam_store_search(name: &str) -> Result<Option<String>> {
    let url = format!(
        "https://store.steampowered.com/api/storesearch/?term={}&l=english&cc=US",
        urlencoding::encode(name)
    );
    let json: serde_json::Value = client()?.get(&url).send()?.json()?;
    let items = json
        .get("items")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let name_l = name.to_lowercase();
    let name_c = compact_alnum(name);
    for item in &items {
        let item_name = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let item_l = item_name.to_lowercase();
        let item_c = compact_alnum(item_name);
        let matched = item_name.eq_ignore_ascii_case(name)
            || item_l == name_l
            || (!name_c.is_empty() && item_c == name_c)
            || (!name_c.is_empty()
                && name_c.len() >= 5
                && (item_c.starts_with(&name_c) || name_c.starts_with(&item_c)))
            || (name_l.len() > 3 && item_l.starts_with(&name_l));
        if matched {
            if let Some(id) = item.get("id").and_then(|v| v.as_u64()) {
                return Ok(Some(id.to_string()));
            }
        }
    }
    // Prefer first result when compact forms share a strong prefix (Xbox folder names)
    if let Some(item) = items.first() {
        let item_name = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let item_c = compact_alnum(item_name);
        if !name_c.is_empty()
            && name_c.len() >= 4
            && (item_c.contains(&name_c) || name_c.contains(&item_c))
        {
            if let Some(id) = item.get("id").and_then(|v| v.as_u64()) {
                return Ok(Some(id.to_string()));
            }
        }
    }
    Ok(None)
}

fn epic_product_cover(name: &str, launch_target: &str) -> Result<Option<String>> {
    let mut slugs = Vec::new();
    if let Some(rest) = launch_target.split("apps/").nth(1) {
        let slug = rest.split(['?', '/']).next().unwrap_or("").to_lowercase();
        if !slug.is_empty() {
            slugs.push(slug);
        }
    }
    for n in search_name_variants(name) {
        slugs.push(slugify(&n));
        let compact = compact_alnum(&n);
        if !compact.is_empty() {
            slugs.push(compact);
        }
    }
    // Well-known Epic product slugs
    let c = compact_alnum(name);
    if c.contains("fortnite") {
        slugs.insert(0, "fortnite".into());
    }

    let mut unique = Vec::new();
    for s in slugs {
        if !s.is_empty() && !unique.iter().any(|u: &String| u == &s) {
            unique.push(s);
        }
    }

    for slug in unique {
        let url =
            format!("https://store-content.ak.epicgames.com/api/en-US/content/products/{slug}");
        let Ok(resp) = client()?.get(&url).send() else {
            continue;
        };
        if !resp.status().is_success() {
            continue;
        }
        let Ok(text) = resp.text() else {
            continue;
        };
        if let Some(img) = pick_best_epic_portrait(&text) {
            return Ok(Some(img));
        }
    }
    Ok(None)
}

fn pick_best_epic_portrait(json_text: &str) -> Option<String> {
    let re = regex::Regex::new(r#"https://cdn2\.unrealengine\.com/[^"\\]+\.(?:jpg|jpeg|png|webp)"#)
        .ok()?;
    let mut scored: Vec<(i32, String)> = re
        .find_iter(json_text)
        .map(|m| m.as_str().to_string())
        .filter(|u| is_box_art_url(u))
        .map(|u| {
            let l = u.to_lowercase();
            let mut score = 0i32;
            if l.contains("dieselgameboxtall") || l.contains("gameboxtall") {
                score -= 120;
            }
            if l.contains("1200x1600") || l.contains("600x900") || l.contains("720x1080") {
                score -= 80;
            }
            if l.contains("launcher-blade") && l.contains("1200x1600") {
                score -= 40;
            }
            if l.contains("portrait") || l.contains("cover") || l.contains("keyart") {
                score -= 20;
            }
            // Punish wide marketing art hard
            if l.contains("2560x1440")
                || l.contains("1920x1080")
                || l.contains("3840x2160")
                || l.contains("dieselgamebox.")
                || l.contains("gameboxwide")
            {
                score += 100;
            }
            (score, u)
        })
        .collect();
    scored.sort_by_key(|(s, u)| (*s, u.clone()));
    scored.into_iter().next().map(|(_, u)| u)
}

fn wikipedia_cover(name: &str) -> Result<Option<String>> {
    let client = client()?;
    let queries = [
        format!("{name} (video game)"),
        format!("{name} video game"),
        name.to_string(),
    ];
    for q in queries {
        let search_url = format!(
            "https://en.wikipedia.org/w/api.php?action=query&list=search&srsearch={}&format=json&srlimit=5",
            urlencoding::encode(&q)
        );
        let search: serde_json::Value = client.get(&search_url).send()?.json()?;
        let titles: Vec<String> = search
            .pointer("/query/search")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|i| {
                        i.get("title")
                            .and_then(|t| t.as_str())
                            .map(|s| s.to_string())
                    })
                    .collect()
            })
            .unwrap_or_default();

        for title in titles {
            let t_l = title.to_lowercase();
            if t_l.contains("disambiguation") {
                continue;
            }
            let summary_url = format!(
                "https://en.wikipedia.org/api/rest_v1/page/summary/{}",
                urlencoding::encode(&title.replace(' ', "_"))
            );
            let Ok(sum) = client.get(&summary_url).send() else {
                continue;
            };
            if !sum.status().is_success() {
                continue;
            }
            let Ok(json) = sum.json::<serde_json::Value>() else {
                continue;
            };
            let desc = json
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_lowercase();
            let is_game = desc.contains("video game")
                || desc.contains("game")
                || title.to_lowercase().contains("video game");
            let title_close = title.to_lowercase().contains(&name.to_lowercase())
                || name.to_lowercase().contains(
                    &title
                        .to_lowercase()
                        .replace("(video game)", "")
                        .trim()
                        .to_string(),
                );
            if !is_game && !title_close {
                continue;
            }
            if let Some(url) = json
                .pointer("/originalimage/source")
                .or_else(|| json.pointer("/thumbnail/source"))
                .and_then(|v| v.as_str())
            {
                let clean = url.split('?').next().unwrap_or(url).to_string();
                if !is_background_url(&clean) {
                    return Ok(Some(clean));
                }
            }
        }
    }
    Ok(None)
}

fn steamgriddb_cover(name: &str, api_key: &str) -> Result<Option<String>> {
    let client = client()?;
    let search_url = format!(
        "https://www.steamgriddb.com/api/v2/search/autocomplete/{}",
        urlencoding::encode(name)
    );
    let search: serde_json::Value = client
        .get(&search_url)
        .header("Authorization", format!("Bearer {api_key}"))
        .send()?
        .json()?;

    let game_id = search
        .pointer("/data/0/id")
        .and_then(|v| v.as_i64())
        .or_else(|| {
            search
                .pointer("/data/0/id")
                .and_then(|v| v.as_u64().map(|u| u as i64))
        });
    let Some(id) = game_id else {
        return Ok(None);
    };

    // Vertical grids only
    let grids_url =
        format!("https://www.steamgriddb.com/api/v2/grids/game/{id}?dimensions=600x900");
    let grids: serde_json::Value = client
        .get(&grids_url)
        .header("Authorization", format!("Bearer {api_key}"))
        .send()?
        .json()?;

    Ok(grids
        .pointer("/data/0/url")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string()))
}

fn slugify(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else if c.is_whitespace() || c == '-' {
                '-'
            } else {
                '\0'
            }
        })
        .filter(|c| *c != '\0')
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn compact_alnum(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Humanize CamelCase / glued Xbox folder names: "ForzaHorizon5" → "Forza Horizon 5"
fn humanize_name(name: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = name.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if i > 0 {
            let prev = chars[i - 1];
            let next = chars.get(i + 1).copied();
            let boundary = (prev.is_lowercase() && c.is_uppercase())
                || (prev.is_ascii_digit() && c.is_alphabetic())
                || (prev.is_alphabetic() && c.is_ascii_digit())
                || (prev.is_uppercase()
                    && c.is_uppercase()
                    && next.is_some_and(|n| n.is_lowercase()));
            if boundary && !out.ends_with(' ') {
                out.push(' ');
            }
        }
        out.push(c);
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn search_name_variants(name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let push = |v: &mut Vec<String>, s: String| {
        let t = s.trim().to_string();
        if !t.is_empty() && !v.iter().any(|x| x.eq_ignore_ascii_case(&t)) {
            v.push(t);
        }
    };
    push(&mut out, name.to_string());
    let human = humanize_name(name);
    push(&mut out, human.clone());
    // Strip common edition suffixes
    for suffix in [
        " Standard Edition",
        " Deluxe Edition",
        " Ultimate Edition",
        " Game of the Year Edition",
        " Windows Edition",
    ] {
        if let Some(stripped) = human.strip_suffix(suffix) {
            push(&mut out, stripped.to_string());
        }
    }
    if let Some(id) = known_steam_app_id(name) {
        // Prefer canonical display names for known titles
        match id {
            "945360" => push(&mut out, "Among Us".into()),
            "1551360" => push(&mut out, "Forza Horizon 5".into()),
            _ => {}
        }
    }
    out
}

fn known_steam_app_id(name: &str) -> Option<&'static str> {
    let c = compact_alnum(name);
    let table = [
        ("amongus", "945360"),
        ("forzahorizon5", "1551360"),
        ("forzahorizon5standardedition", "1551360"),
        ("fh5", "1551360"),
    ];
    for (key, id) in table {
        if c == key || c.starts_with(key) {
            return Some(id);
        }
    }
    None
}

fn known_ms_product_id(name: &str) -> Option<&'static str> {
    let c = compact_alnum(name);
    let table = [
        ("forzahorizon5", "9NKX70BBCDRN"),
        ("forzahorizon5standardedition", "9NKX70BBCDRN"),
        ("fh5", "9NKX70BBCDRN"),
        ("minecraft", "9NBLGGH2QH4B"),
        ("minecraftlauncher", "9NBLGGH2QH4B"),
    ];
    for (key, id) in table {
        if c == key || c.starts_with(key) {
            return Some(id);
        }
    }
    // Among Us on MS Store varies by region; Steam cover is preferred via known ID
    None
}

fn name_looks_like_epic(name: &str) -> bool {
    let c = compact_alnum(name);
    c.contains("fortnite") || c.contains("rocketleague") || c.contains("fallguys")
}

fn microsoft_store_cover(name: &str) -> Result<Option<String>> {
    let Some(product_id) = known_ms_product_id(name) else {
        return Ok(None);
    };
    let url = format!(
        "https://displaycatalog.mp.microsoft.com/v7.0/products?bigIds={product_id}&market=US&languages=en-US&fieldsTemplate=Browse"
    );
    let json: serde_json::Value = client()?.get(&url).send()?.json()?;
    let images = json
        .pointer("/Products/0/LocalizedProperties/0/Images")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut ranked: Vec<(i32, String)> = Vec::new();
    for img in images {
        let purpose = img
            .get("ImagePurpose")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_lowercase();
        let uri = img.get("Uri").and_then(|v| v.as_str()).unwrap_or("");
        if uri.is_empty() {
            continue;
        }
        let w = img.get("Width").and_then(|v| v.as_u64()).unwrap_or(0);
        let h = img.get("Height").and_then(|v| v.as_u64()).unwrap_or(0);
        let mut score = 50i32;
        if purpose == "poster" {
            score -= 100;
        } else if purpose == "boxart" {
            score -= 40;
        } else if purpose.contains("logo") || purpose.contains("screenshot") {
            score += 80;
        }
        if h > 0 && w > 0 {
            if (h as f32) >= (w as f32) * 1.2 {
                score -= 30;
            } else if w > h {
                score += 60;
            }
        }
        let full = if uri.starts_with("//") {
            format!("https:{uri}")
        } else {
            uri.to_string()
        };
        if is_box_art_url(&full) {
            ranked.push((score, full));
        }
    }
    ranked.sort_by_key(|(s, u)| (*s, u.clone()));
    Ok(ranked.into_iter().next().map(|(_, u)| u))
}

fn roblox_cover_urls() -> Vec<&'static str> {
    vec![
        // Square brand / app tiles (accepted by square-tolerant cover check)
        "https://images.rbxcdn.com/5348266ea6c5e67b19d6a814cbbb70f6.jpg",
        "https://tr.rbxcdn.com/180DAY-03529af97a21dcc29156c5384cc1b01b/512/512/Image/Png/noFilter",
    ]
}

/// Public helper: true when image bytes are usable cover art (portrait or square, not landscape).
#[allow(dead_code)]
pub fn is_portrait_cover_bytes(bytes: &[u8]) -> bool {
    is_portrait_bytes(bytes)
}

pub fn cover_as_data_url(path: &str) -> Result<String> {
    let bytes = fs::read(path)?;
    let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes);
    let mime = if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".webp") {
        "image/webp"
    } else if path.ends_with(".gif") {
        "image/gif"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else {
        "image/jpeg"
    };
    Ok(format!("data:{mime};base64,{b64}"))
}
