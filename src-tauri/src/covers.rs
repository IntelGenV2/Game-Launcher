use crate::db::covers_dir;
use crate::models::{CoverChoiceGroup, Game, Store};
use anyhow::Result;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

/// Reject oversized images so cover fetch / folder scans cannot OOM the process.
const MAX_COVER_BYTES: u64 = 2_500_000;

/// Result of a successful cover fetch (may also resolve a Steam AppID / genre).
pub struct CoverFetch {
    pub path: String,
    pub steam_app_id: Option<String>,
    pub genre: Option<String>,
}

/// Cover selection rules (always enforced):
/// 1. Prefer vertical box / library art (~2:3 portrait); square box art is OK.
/// 2. Never use headers, heroes, page backgrounds, screenshots, or wide capsules.
/// 3. After download, reject landscape images (clearly wider than tall).
pub fn ensure_cover(game: &Game, api_key: Option<&str>) -> Result<Option<CoverFetch>> {
    let mut resolved_steam: Option<String> = None;
    let need_genre = game.genre.as_ref().map(|s| s.trim().is_empty()).unwrap_or(true);

    if let Some(path) = &game.cover_path {
        if is_portrait_cover_path(path) {
            let genre = if need_genre {
                resolve_steam_id(game, &mut resolved_steam).and_then(|id| {
                    steam_app_genres(&id).ok().flatten()
                })
            } else {
                None
            };
            if genre.is_none() && resolved_steam.is_none() {
                return Ok(None);
            }
            return Ok(Some(CoverFetch {
                path: path.clone(),
                steam_app_id: resolved_steam.filter(|_| game.steam_app_id.is_none()),
                genre,
            }));
        }
    }

    fs::create_dir_all(covers_dir())?;
    purge_landscape_covers_for(game);

    // Named file already cached in the launcher covers folder
    // (id.jpg, or older id-{uuid}.png names).
    if let Some(path) = find_cover_file(game) {
        let genre = if need_genre {
            resolve_steam_id(game, &mut resolved_steam).and_then(|id| {
                steam_app_genres(&id).ok().flatten()
            })
        } else {
            None
        };
        return Ok(Some(done(
            game,
            &path,
            resolved_steam.filter(|_| game.steam_app_id.is_none()),
            genre,
        )));
    }

    // Unnamed images next to the game (manual installs, GOG, etc.) — copy in as {gameId}.ext
    if let Some(path) = import_folder_cover(game) {
        let genre = fetch_genre_if_needed(game, &mut resolved_steam, need_genre);
        return Ok(Some(done(game, &path, resolved_steam.clone(), genre)));
    }

    let dest = cover_dest_for(&game.id, "jpg");

    let search_names = search_name_variants(&game.name);

    // 0) Steam's own downloaded library art (no network)
    if let Some(app_id) = resolve_steam_id(game, &mut resolved_steam) {
        if copy_steam_library_cache(&app_id, &dest) {
            let genre = fetch_genre_if_needed(game, &mut resolved_steam, need_genre);
            return Ok(Some(done(game, &dest, resolved_steam.clone(), genre)));
        }
    }

    // 1) Stored cover_url only if it looks like box art (not a background URL)
    if let Some(url) = &game.cover_url {
        if is_box_art_url(url) && try_save_cover(url, &dest) {
            let genre = fetch_genre_if_needed(game, &mut resolved_steam, need_genre);
            return Ok(Some(done(game, &dest, resolved_steam.clone(), genre)));
        }
    }

    // 2) Steam library capsule (known ID → stored ID → name search with normalized variants)
    if let Some(app_id) = resolve_steam_id(game, &mut resolved_steam) {
        let mut urls = steam_library_capsule_urls(&app_id).unwrap_or_default();
        urls.extend(steam_cdn_urls(&app_id));
        for url in urls {
            if try_save_cover(&url, &dest) {
                let genre = fetch_genre_if_needed(game, &mut resolved_steam, need_genre);
                return Ok(Some(done(game, &dest, resolved_steam.clone(), genre)));
            }
        }
    }

    // 3) Epic tall launcher art (Epic games + name matches like Fortnite)
    if matches!(game.store, Store::Epic)
        || game.id.starts_with("epic:")
        || name_looks_like_epic(&game.name)
    {
        if let Ok(Some(url)) = epic_product_cover(&game.name, &game.launch_target) {
            if try_save_cover(&url, &dest) {
                let genre = fetch_genre_if_needed(game, &mut resolved_steam, need_genre);
                return Ok(Some(done(game, &dest, resolved_steam.clone(), genre)));
            }
        }
    }

    // 4) Microsoft Store display catalog (Xbox / Game Pass titles)
    if matches!(game.store, Store::Xbox) || game.id.starts_with("xbox:") {
        for n in &search_names {
            if let Ok(Some(url)) = microsoft_store_cover(n) {
                if try_save_cover(&url, &dest) {
                    let genre = fetch_genre_if_needed(game, &mut resolved_steam, need_genre);
                    return Ok(Some(done(game, &dest, resolved_steam.clone(), genre)));
                }
            }
        }
    }

    // 5) SteamGridDB vertical grids
    if let Some(key) = api_key {
        for n in &search_names {
            if let Ok(Some(url)) = steamgriddb_cover(n, key) {
                if try_save_cover(&url, &dest) {
                    let genre = fetch_genre_if_needed(game, &mut resolved_steam, need_genre);
                    return Ok(Some(done(game, &dest, resolved_steam.clone(), genre)));
                }
            }
        }
    }

    // 6) Wikipedia cover (must pass shape check)
    for n in &search_names {
        if let Ok(Some(url)) = wikipedia_cover(n) {
            if is_box_art_url(&url) && try_save_cover(&url, &dest) {
                let genre = fetch_genre_if_needed(game, &mut resolved_steam, need_genre);
                return Ok(Some(done(game, &dest, resolved_steam.clone(), genre)));
            }
        }
    }

    // 7) Roblox brand / game icon (square OK)
    if matches!(game.store, Store::Roblox) || compact_alnum(&game.name) == "roblox" {
        for url in roblox_cover_urls() {
            if try_save_cover(url, &dest) {
                let genre = fetch_genre_if_needed(game, &mut resolved_steam, need_genre);
                return Ok(Some(done(game, &dest, resolved_steam.clone(), genre)));
            }
        }
    }

    // Cover download failed — do not keep a landscape file as the cover.
    let genre = fetch_genre_if_needed(game, &mut resolved_steam, need_genre);
    let steam_app_id = resolved_steam.filter(|_| game.steam_app_id.is_none());
    if steam_app_id.is_some() || genre.is_some() {
        return Ok(Some(CoverFetch {
            path: String::new(),
            steam_app_id,
            genre,
        }));
    }

    Ok(None)
}

pub fn steam_app_id_for(game: &Game) -> Option<String> {
    let mut resolved = None;
    resolve_steam_id(game, &mut resolved)
}

#[allow(dead_code)]
pub fn logo_dest_for(game_id: &str) -> PathBuf {
    covers_dir().join(format!("{}.logo.png", safe_id(game_id)))
}

#[allow(dead_code)]
pub fn import_logo_file(game_id: &str, source_path: &str) -> Result<String> {
    let src = Path::new(source_path);
    if !src.exists() {
        anyhow::bail!("Logo file not found");
    }
    let dir = covers_dir();
    fs::create_dir_all(&dir)?;
    let dest = logo_dest_for(game_id);
    fs::copy(src, &dest)?;
    Ok(dest.to_string_lossy().to_string())
}

#[allow(dead_code)]
pub fn save_image_url(url: &str, dest: &Path) -> Result<bool> {
    let bytes = match download_bytes(url) {
        Ok(b) => b,
        Err(_) => return Ok(false),
    };
    if bytes.len() < 200 {
        return Ok(false);
    }
    if detect_image_ext(&bytes).is_none() {
        return Ok(false);
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::File::create(dest)?;
    file.write_all(&bytes)?;
    Ok(true)
}

fn resolve_steam_id(game: &Game, resolved: &mut Option<String>) -> Option<String> {
    if let Some(id) = resolved.clone() {
        return Some(id);
    }
    let mut steam_id = game.steam_app_id.clone();
    if steam_id.is_none() && matches!(game.store, Store::Steam) {
        if game.launch_target.chars().all(|c| c.is_ascii_digit()) {
            steam_id = Some(game.launch_target.clone());
        } else if let Some(id) = game.id.strip_prefix("steam:") {
            steam_id = Some(id.to_string());
        }
    }
    if steam_id.is_none() {
        steam_id = known_steam_app_id(&game.name).map(|s| s.to_string());
    }
    if steam_id.is_none() {
        for n in search_name_variants(&game.name) {
            if let Some(id) = steam_store_search(&n).ok().flatten() {
                steam_id = Some(id);
                break;
            }
        }
    }
    if let Some(id) = steam_id.clone() {
        *resolved = Some(id.clone());
    }
    steam_id
}

fn fetch_genre_if_needed(
    game: &Game,
    resolved: &mut Option<String>,
    need_genre: bool,
) -> Option<String> {
    if !need_genre {
        return None;
    }
    resolve_steam_id(game, resolved).and_then(|id| steam_app_genres(&id).ok().flatten())
}

fn done(game: &Game, dest: &Path, steam_app_id: Option<String>, genre: Option<String>) -> CoverFetch {
    purge_landscape_covers_for(game);
    CoverFetch {
        path: dest.to_string_lossy().to_string(),
        steam_app_id,
        genre,
    }
}

pub fn import_cover_file(game_id: &str, source_path: &str) -> Result<String> {
    let src = Path::new(source_path);
    if !src.exists() {
        anyhow::bail!("Cover file not found");
    }
    let header = {
        let mut f = fs::File::open(src)?;
        let mut buf = [0u8; 12];
        use std::io::Read;
        let n = f.read(&mut buf)?;
        buf[..n].to_vec()
    };
    let ext = match detect_image_ext(&header) {
        Some(ext) => ext.to_string(),
        None => anyhow::bail!("File is not a supported image (png, jpeg, webp, gif, bmp)"),
    };
    if !is_usable_box_art_file(src) {
        anyhow::bail!("Cover must be portrait box art (taller than wide)");
    }

    let dir = covers_dir();
    fs::create_dir_all(&dir)?;
    let dest = cover_dest_for(game_id, &ext);
    let safe = safe_id(game_id);
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(&safe) && !name.contains(".logo.") {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
    fs::copy(src, &dest)?;
    invalidate_cover_catalog();
    Ok(dest.to_string_lossy().to_string())
}

fn detect_image_ext(header: &[u8]) -> Option<&'static str> {
    if header.starts_with(&[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n']) {
        return Some("png");
    }
    if header.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some("jpg");
    }
    if header.starts_with(b"GIF87a") || header.starts_with(b"GIF89a") {
        return Some("gif");
    }
    if header.len() >= 12 && &header[0..4] == b"RIFF" && &header[8..12] == b"WEBP" {
        return Some("webp");
    }
    if header.starts_with(b"BM") {
        return Some("bmp");
    }
    None
}

fn try_save_cover(url: &str, dest: &Path) -> bool {
    if is_background_url(url) {
        return false;
    }
    let bytes = match download_bytes(url) {
        Ok(b) => b,
        Err(_) => return false,
    };
    write_cover_bytes(&bytes, dest)
}

fn write_cover_bytes(bytes: &[u8], dest: &Path) -> bool {
    if bytes.len() < 800 || bytes.len() as u64 > MAX_COVER_BYTES {
        return false;
    }
    let Some((w, h)) = image_dimensions(bytes) else {
        return false;
    };
    if w == 0 || h == 0 || (h as f32) < (w as f32) * 0.85 {
        return false;
    }
    match fs::File::create(dest).and_then(|mut f| f.write_all(bytes)) {
        Ok(()) => {
            invalidate_cover_catalog();
            true
        }
        Err(_) => false,
    }
}

fn copy_steam_library_cache(app_id: &str, dest: &Path) -> bool {
    for src in steam_library_cache_paths(app_id) {
        let Ok(meta) = fs::metadata(&src) else {
            continue;
        };
        if meta.len() < 800 || meta.len() > MAX_COVER_BYTES {
            continue;
        }
        let Ok(bytes) = fs::read(&src) else {
            continue;
        };
        if write_cover_bytes(&bytes, dest) {
            return true;
        }
    }
    false
}

fn steam_library_cache_paths(app_id: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Some(steam) = find_steam_install() else {
        return out;
    };
    let cache = steam.join("appcache").join("librarycache");
    for name in [
        format!("{app_id}_library_600x900.jpg"),
        format!("{app_id}_library_capsule.jpg"),
        format!("{app_id}_portrait.png"),
    ] {
        out.push(cache.join(name));
    }
    let nested = cache.join(app_id);
    if nested.is_dir() {
        if let Ok(entries) = fs::read_dir(&nested) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
                if name.contains("_2x") || name.contains("hero") || name.contains("header") {
                    continue;
                }
                if name.contains("library_600x900")
                    || name.contains("library_capsule")
                    || name.contains("portrait")
                {
                    out.push(entry.path());
                }
            }
        }
    }
    out
}

fn find_steam_install() -> Option<PathBuf> {
    let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
    if let Ok(key) = hkcu.open_subkey(r"Software\Valve\Steam") {
        if let Ok(path) = key.get_value::<String, _>("SteamPath") {
            let p = PathBuf::from(path.replace('/', "\\"));
            if p.exists() {
                return Some(p);
            }
        }
    }
    for c in [
        r"C:\Program Files (x86)\Steam",
        r"C:\Program Files\Steam",
        r"D:\Steam",
        r"E:\Steam",
    ] {
        let p = PathBuf::from(c);
        if p.join("steam.exe").exists() {
            return Some(p);
        }
    }
    None
}

fn steam_cdn_urls(app_id: &str) -> Vec<String> {
    let files = ["library_600x900.jpg", "library_capsule.jpg"];
    let hosts = [
        "https://shared.akamai.steamstatic.com/store_item_assets/steam/apps",
        "https://cdn.cloudflare.steamstatic.com/steam/apps",
    ];
    let mut out = Vec::new();
    for file in files {
        for host in hosts {
            out.push(format!("{host}/{app_id}/{file}"));
        }
    }
    out
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
    // WebP (VP8X / VP8 / VP8L)
    if bytes.starts_with(b"RIFF") && bytes.len() > 30 && &bytes[8..12] == b"WEBP" {
        if &bytes[12..16] == b"VP8X" && bytes.len() >= 30 {
            let w = 1 + u32::from_le_bytes([bytes[24], bytes[25], bytes[26], 0]);
            let h = 1 + u32::from_le_bytes([bytes[27], bytes[28], bytes[29], 0]);
            return Some((w, h));
        }
        if &bytes[12..16] == b"VP8 " && bytes.len() >= 30 {
            // Lossy VP8 keyframe: 3-byte frame tag, then 0x9D 0x01 0x2A, then 14-bit size.
            let mut i = 20usize;
            if bytes.len() > 23 && bytes[20] == 0x9D && bytes[21] == 0x01 && bytes[22] == 0x2A {
                i = 23;
            }
            if i + 4 <= bytes.len() {
                let w = u16::from_le_bytes([bytes[i], bytes[i + 1]]) as u32 & 0x3FFF;
                let h = u16::from_le_bytes([bytes[i + 2], bytes[i + 3]]) as u32 & 0x3FFF;
                if w > 0 && h > 0 {
                    return Some((w, h));
                }
            }
        }
        if &bytes[12..16] == b"VP8L" && bytes.len() >= 25 {
            let b0 = bytes[21] as u32;
            let b1 = bytes[22] as u32;
            let b2 = bytes[23] as u32;
            let b3 = bytes[24] as u32;
            let w = (b0 | ((b1 & 0x3F) << 8)) + 1;
            let h = (((b1 & 0xC0) >> 6) | (b2 << 2) | ((b3 & 0x0F) << 10)) + 1;
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

fn normalize_cover_key(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

fn is_uuid_str(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 36 {
        return false;
    }
    let dashes = [8usize, 13, 18, 23];
    for (i, ch) in b.iter().enumerate() {
        if dashes.contains(&i) {
            if *ch != b'-' {
                return false;
            }
        } else if !ch.is_ascii_hexdigit() {
            return false;
        }
    }
    true
}

/// `steam_553850-bf16be00-cbbd-47f5-9cb8-c339d6eb56c5` → `steam_553850`
fn strip_trailing_uuid(stem: &str) -> Option<&str> {
    if stem.len() < 38 {
        return None;
    }
    let split = stem.len() - 37;
    if stem.as_bytes().get(split) != Some(&b'-') {
        return None;
    }
    if is_uuid_str(&stem[split + 1..]) {
        Some(&stem[..split])
    } else {
        None
    }
}

fn filename_cover_keys(stem: &str) -> Vec<String> {
    let mut keys = vec![normalize_cover_key(stem)];
    if let Some(base) = strip_trailing_uuid(stem) {
        let k = normalize_cover_key(base);
        if !keys.contains(&k) {
            keys.push(k);
        }
    }
    keys
}

#[cfg(test)]
mod cover_name_tests {
    use super::*;

    #[test]
    fn strips_legacy_uuid_suffix() {
        assert_eq!(
            strip_trailing_uuid("steam_553850-bf16be00-cbbd-47f5-9cb8-c339d6eb56c5"),
            Some("steam_553850")
        );
        assert_eq!(
            strip_trailing_uuid(
                "manual_3bc7a30d-6b0c-4dec-aebe-7211b5870061-08b81dbf-9f43-49a2-a70f-89f0cc9d7189"
            ),
            Some("manual_3bc7a30d-6b0c-4dec-aebe-7211b5870061")
        );
        assert_eq!(
            strip_trailing_uuid(
                "epic_4fe75bbc5a674f4f9b356b5c90567da5-c9be1629-13c7-4f79-b74a-daa64fec8997"
            ),
            Some("epic_4fe75bbc5a674f4f9b356b5c90567da5")
        );
        assert_eq!(strip_trailing_uuid("steam_553850"), None);
    }

    #[test]
    fn normalizes_double_underscores_and_spaces() {
        assert_eq!(
            normalize_cover_key("xbox_minecraft__java_edition"),
            "xbox_minecraft_java_edition"
        );
        assert_eq!(
            normalize_cover_key("xbox_Minecraft (Java Edition)"),
            "xbox_minecraft_java_edition"
        );
        assert!(filename_cover_keys(
            "xbox_minecraft__java_edition-c18b3575-df61-423a-82b2-7aef633dddfd"
        )
        .contains(&"xbox_minecraft_java_edition".to_string()));
    }
}

fn game_cover_keys(game: &Game) -> Vec<String> {
    let mut keys = Vec::new();
    let push = |v: &mut Vec<String>, raw: &str| {
        let k = normalize_cover_key(raw);
        if !k.is_empty() && !v.contains(&k) {
            v.push(k);
        }
    };
    push(&mut keys, &safe_id(&game.id));
    if let Some((store, rest)) = game.id.split_once(':') {
        push(&mut keys, &format!("{store}_{rest}"));
        push(&mut keys, rest);
        push(&mut keys, &format!("{}_{}", store, rest.replace(' ', "_")));
    }
    if let Some(app) = &game.steam_app_id {
        push(&mut keys, &format!("steam_{app}"));
        push(&mut keys, app);
    }
    keys
}

fn looks_like_non_cover_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    [
        "screenshot",
        "gameplay",
        "header",
        "hero",
        "banner",
        "wide",
        "library_hero",
        "page_bg",
        "capsule_616",
    ]
    .iter()
    .any(|k| n.contains(k))
}

pub fn cover_file_present(path: &str) -> bool {
    match fs::metadata(path) {
        Ok(m) if m.is_file() && m.len() > 32 => true,
        _ => false,
    }
}

pub fn is_portrait_cover_path(path: &str) -> bool {
    let want = PathBuf::from(path);
    for file in cover_catalog() {
        if file.path == want {
            return file.score != i32::MIN;
        }
    }
    is_usable_box_art_file(&want)
}

fn is_known_landscape_file(path: &Path) -> bool {
    let Some(bytes) = peek_image_header(path) else {
        return false;
    };
    match image_dimensions(&bytes) {
        Some((w, h)) if w > 16 && h > 16 => (h as f32) < (w as f32) * 0.85,
        _ => false,
    }
}

/// Delete landscape banners saved for this game so they cannot fight portrait art.
pub fn purge_landscape_covers_for(game: &Game) {
    let wanted = game_cover_keys(game);
    let mut removed = false;
    for file in cover_catalog() {
        if !file.keys.iter().any(|k| wanted.iter().any(|w| w == k)) {
            continue;
        }
        if is_known_landscape_file(&file.path) {
            let _ = fs::remove_file(&file.path);
            removed = true;
        }
    }
    if removed {
        invalidate_cover_catalog();
    }
}

pub fn is_usable_box_art_file(path: impl AsRef<Path>) -> bool {
    let path = path.as_ref();
    if !path.is_file() {
        return false;
    }
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    if looks_like_non_cover_name(name) {
        return false;
    }
    let Some(bytes) = peek_image_header(path) else {
        return false;
    };
    is_usable_box_art_bytes(&bytes)
}

fn peek_image_header(path: &Path) -> Option<Vec<u8>> {
    let mut f = fs::File::open(path).ok()?;
    let mut buf = vec![0u8; 65536];
    let n = f.read(&mut buf).ok()?;
    buf.truncate(n);
    if buf.len() < 24 {
        return None;
    }
    Some(buf)
}

fn is_usable_box_art_bytes(bytes: &[u8]) -> bool {
    if bytes.len() < 24 {
        return false;
    }
    match image_dimensions(bytes) {
        Some((w, h)) if w > 0 && h > 0 => (h as f32) >= (w as f32) * 0.85,
        _ => false,
    }
}

fn cover_file_score(stem: &str, path: &Path) -> i32 {
    if looks_like_non_cover_name(stem) {
        return i32::MIN;
    }
    let Some(bytes) = peek_image_header(path) else {
        return i32::MIN;
    };
    if !is_usable_box_art_bytes(&bytes) {
        return i32::MIN;
    }
    let mut score = if strip_trailing_uuid(stem).is_none() { 2 } else { 1 };
    if path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("png"))
        == Some(true)
    {
        score += 1;
    }
    if let Some((w, h)) = image_dimensions(&bytes) {
        if h >= w {
            score += 4;
        }
    }
    score
}

#[derive(Clone)]
struct IndexedCover {
    path: PathBuf,
    keys: Vec<String>,
    score: i32,
}

fn catalog_lock() -> &'static Mutex<Option<(u64, Vec<IndexedCover>)>> {
    static CATALOG: OnceLock<Mutex<Option<(u64, Vec<IndexedCover>)>>> = OnceLock::new();
    CATALOG.get_or_init(|| Mutex::new(None))
}

pub fn invalidate_cover_catalog() {
    if let Ok(mut g) = catalog_lock().lock() {
        *g = None;
    }
}

pub fn warm_cover_catalog() {
    let _ = cover_catalog();
}

fn covers_dir_stamp() -> u64 {
    let Ok(entries) = fs::read_dir(covers_dir()) else {
        return 0;
    };
    let mut n = 0u64;
    let mut mix = 0u64;
    for entry in entries.flatten() {
        n += 1;
        if let Ok(meta) = entry.metadata() {
            mix = mix.wrapping_add(meta.len());
            if let Ok(modified) = meta.modified() {
                if let Ok(dur) = modified.duration_since(SystemTime::UNIX_EPOCH) {
                    mix = mix.wrapping_add(dur.as_secs());
                }
            }
        }
    }
    n.wrapping_mul(0x9E3779B97F4A7C15).wrapping_add(mix)
}

fn cover_catalog() -> Vec<IndexedCover> {
    let stamp = covers_dir_stamp();
    if let Ok(mut slot) = catalog_lock().lock() {
        if let Some((s, files)) = slot.as_ref() {
            if *s == stamp {
                return files.clone();
            }
        }
        let files = scan_cover_catalog();
        *slot = Some((stamp, files.clone()));
        return files;
    }
    scan_cover_catalog()
}

fn scan_cover_catalog() -> Vec<IndexedCover> {
    let mut files = Vec::new();
    let Ok(entries) = fs::read_dir(covers_dir()) else {
        return files;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() || !is_image_file(&path) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.contains(".logo.") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let keys = filename_cover_keys(stem);
        if keys.is_empty() {
            continue;
        }
        let score = cover_file_score(stem, &path);
        files.push(IndexedCover { path, keys, score });
    }
    files
}

fn lookup_cover(files: &[IndexedCover], game: &Game) -> Option<PathBuf> {
    let wanted = game_cover_keys(game);
    let mut best: Option<(i32, PathBuf)> = None;
    for file in files {
        if file.score == i32::MIN {
            continue;
        }
        if !file.keys.iter().any(|k| wanted.iter().any(|w| w == k)) {
            continue;
        }
        if best.as_ref().map(|(s, _)| file.score > *s).unwrap_or(true) {
            best = Some((file.score, file.path.clone()));
        }
    }
    best.map(|(_, p)| p)
}

/// Find a cover already in the launcher covers folder, including legacy `{id}-{uuid}.png` names.
pub fn find_cover_file(game: &Game) -> Option<PathBuf> {
    lookup_cover(&cover_catalog(), game)
}

pub fn cover_alternates_for(game: &Game) -> Vec<PathBuf> {
    cover_alternates_from(&cover_catalog(), game)
}

fn cover_alternates_from(files: &[IndexedCover], game: &Game) -> Vec<PathBuf> {
    let wanted = game_cover_keys(game);
    let mut out: Vec<PathBuf> = Vec::new();
    for file in files {
        if file.score == i32::MIN {
            continue;
        }
        if !file.keys.iter().any(|k| wanted.iter().any(|w| w == k)) {
            continue;
        }
        if !out.iter().any(|p| p == &file.path) {
            out.push(file.path.clone());
        }
    }
    out
}

pub fn cover_choice_groups(games: &[Game]) -> Vec<CoverChoiceGroup> {
    let files = cover_catalog();
    games
        .iter()
        .filter(|g| !g.hidden)
        .filter_map(|g| {
            let paths = cover_alternates_from(&files, g);
            if paths.len() < 2 {
                return None;
            }
            Some(CoverChoiceGroup {
                game_id: g.id.clone(),
                name: g.name.clone(),
                current_path: g.cover_path.clone(),
                paths: paths
                    .into_iter()
                    .map(|p| p.to_string_lossy().to_string())
                    .collect(),
            })
        })
        .collect()
}

pub fn path_is_cover_choice(game: &Game, path: &str) -> bool {
    let want = PathBuf::from(path);
    cover_alternates_for(game).iter().any(|p| p == &want)
}

/// Point games with a missing/stale cover_path at files already sitting in the covers folder.
/// Returns `(game id, cover path)` for each game that was reattached.
pub fn reattach_local_covers(
    games: &mut [Game],
    set_path: impl Fn(&str, Option<&str>),
) -> Vec<(String, String)> {
    purge_all_landscape_covers();
    let files = cover_catalog();
    let mut attached = Vec::new();
    for game in games.iter_mut() {
        let current_ok = match &game.cover_path {
            Some(p) if cover_file_present(p) && catalog_path_is_portrait(&files, p) => true,
            _ => false,
        };
        if current_ok {
            continue;
        }
        if let Some(path) = lookup_cover(&files, game) {
            let s = path.to_string_lossy().to_string();
            set_path(&game.id, Some(&s));
            game.cover_path = Some(s.clone());
            attached.push((game.id.clone(), s));
        } else if game.cover_path.is_some() {
            set_path(&game.id, None);
            game.cover_path = None;
        }
    }
    attached
}

fn catalog_path_is_portrait(files: &[IndexedCover], path: &str) -> bool {
    let want = PathBuf::from(path);
    files
        .iter()
        .find(|f| f.path == want)
        .map(|f| f.score != i32::MIN)
        .unwrap_or_else(|| is_usable_box_art_file(&want))
}

fn purge_all_landscape_covers() {
    let files = cover_catalog();
    let mut removed = false;
    for file in &files {
        if is_known_landscape_file(&file.path) {
            let _ = fs::remove_file(&file.path);
            removed = true;
        }
    }
    if removed {
        invalidate_cover_catalog();
    }
}

fn game_folder_roots(game: &Game) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let push = |v: &mut Vec<PathBuf>, p: PathBuf| {
        if p.exists() && !v.iter().any(|x| x == &p) {
            v.push(p);
        }
    };
    if let Some(raw) = &game.install_path {
        let p = PathBuf::from(raw);
        if p.is_file() {
            if let Some(parent) = p.parent() {
                push(&mut roots, parent.to_path_buf());
            }
        } else {
            push(&mut roots, p);
        }
    }
    let launch = PathBuf::from(&game.launch_target);
    if launch.is_file() {
        if let Some(parent) = launch.parent() {
            push(&mut roots, parent.to_path_buf());
        }
    } else if launch.is_dir() {
        push(&mut roots, launch);
    }
    roots
}

fn is_image_file(path: &Path) -> bool {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Some("jpg" | "jpeg" | "png" | "webp" | "bmp" | "gif") => true,
        _ => false,
    }
}

fn skip_dir_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    if n.contains("cover") || n.contains("art") || n.contains("media") || n.contains("image") {
        return false;
    }
    matches!(
        n.as_str(),
        "_commonredist"
            | "engine"
            | "binaries"
            | "redist"
            | "easyanticheat"
            | "battleye"
            | "dotnet"
            | "mono"
            | "logs"
            | "log"
            | "crashreports"
            | "node_modules"
            | ".git"
            | "movies"
            | "videos"
            | "bink"
            | "reshade-shaders"
            | "saved"
            | "__pycache__"
            | "assets"
            | "content"
            | "data"
            | "paks"
            | "sounds"
            | "music"
            | "maps"
            | "streamingassets"
    )
}

fn score_local_image(path: &Path, bytes: &[u8]) -> i32 {
    if bytes.len() < 12_000 || bytes.len() > 12_000_000 {
        return i32::MIN;
    }
    let mut score = 0i32;
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    for hint in [
        "cover", "boxart", "box_art", "poster", "portrait", "library", "vertical", "grid",
        "keyart", "key_art", "artwork",
    ] {
        if name.contains(hint) {
            score += 40;
            break;
        }
    }
    if looks_like_non_cover_name(&name) {
        return i32::MIN;
    }
    match image_dimensions(bytes) {
        Some((w, h)) if w > 0 && h > 0 => {
            if (h as f32) < (w as f32) * 0.85 {
                return i32::MIN;
            }
            if h >= w {
                score += 55;
            } else {
                score += 40;
            }
            if (400..=1400).contains(&w) && (500..=1800).contains(&h) {
                score += 15;
            }
        }
        _ => return i32::MIN,
    }
    score
}

fn best_image_in_dir(dir: &Path, recursive_one: bool) -> Option<(i32, PathBuf)> {
    let mut best: Option<(i32, PathBuf)> = None;
    let Ok(entries) = fs::read_dir(dir) else {
        return None;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if recursive_one {
                let name = entry.file_name().to_string_lossy().to_string();
                if skip_dir_name(&name) {
                    continue;
                }
                if let Some(hit) = best_image_in_dir(&path, false) {
                    if best.as_ref().map(|(s, _)| hit.0 > *s).unwrap_or(true) {
                        best = Some(hit);
                    }
                }
            }
            continue;
        }
        if !is_image_file(&path) {
            continue;
        }
        let Ok(meta) = fs::metadata(&path) else {
            continue;
        };
        if meta.len() < 8_000 || meta.len() > MAX_COVER_BYTES {
            continue;
        }
        let Some(bytes) = peek_image_header(&path) else {
            continue;
        };
        let score = score_local_image(&path, &bytes);
        if score == i32::MIN {
            continue;
        }
        if best.as_ref().map(|(s, _)| score > *s).unwrap_or(true) {
            best = Some((score, path));
        }
    }
    best
}

/// Copy an unnamed cover from the game install folder into the launcher covers dir as `{gameId}.ext`.
fn import_folder_cover(game: &Game) -> Option<PathBuf> {
    let mut best: Option<(i32, PathBuf)> = None;
    for root in game_folder_roots(game) {
        // Prefer images sitting next to the exe before walking one folder down.
        for recursive in [false, true] {
            if let Some(hit) = best_image_in_dir(&root, recursive) {
                if best.as_ref().map(|(s, _)| hit.0 > *s).unwrap_or(true) {
                    best = Some(hit);
                }
            }
            if best.as_ref().map(|(s, _)| *s >= 40).unwrap_or(false) {
                break;
            }
        }
    }
    let (_, src) = best?;
    if looks_like_non_cover_name(&src.to_string_lossy()) {
        return None;
    }
    let len = fs::metadata(&src).ok()?.len();
    if len > MAX_COVER_BYTES {
        return None;
    }
    let bytes = fs::read(&src).ok()?;
    if bytes.len() < 12_000 || !is_usable_box_art_bytes(&bytes) {
        return None;
    }
    let ext = detect_image_ext(&bytes).unwrap_or("jpg");
    let dest = cover_dest_for(&game.id, ext);
    fs::create_dir_all(covers_dir()).ok()?;
    if !write_cover_bytes(&bytes, &dest) {
        return None;
    }
    purge_landscape_covers_for(game);
    Some(dest)
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
    if let Some(len) = resp.content_length() {
        if len > MAX_COVER_BYTES {
            anyhow::bail!("image too large");
        }
    }
    let mut buf = Vec::new();
    let mut limited = resp.take(MAX_COVER_BYTES + 1);
    limited.read_to_end(&mut buf)?;
    if buf.len() as u64 > MAX_COVER_BYTES {
        anyhow::bail!("image too large");
    }
    Ok(buf)
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
    for key in ["library_capsule"] {
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
        let matched = names_close_enough(name, item_name)
            || (!name_c.is_empty()
                && name_c.len() >= 5
                && (item_c.starts_with(&name_c) || name_c.starts_with(&item_c)))
            || (name_l.len() > 4 && item_l.starts_with(&name_l));
        if matched {
            if let Some(id) = item.get("id").and_then(|v| v.as_u64()) {
                return Ok(Some(id.to_string()));
            }
        }
    }
    if let Some(item) = items.first() {
        let item_name = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let item_c = compact_alnum(item_name);
        if !name_c.is_empty()
            && name_c.len() >= 6
            && (item_c.contains(&name_c) || name_c.contains(&item_c))
        {
            if let Some(id) = item.get("id").and_then(|v| v.as_u64()) {
                return Ok(Some(id.to_string()));
            }
        }
    }
    Ok(None)
}

fn names_close_enough(query: &str, candidate: &str) -> bool {
    let q = compact_alnum(query);
    let c = compact_alnum(candidate);
    if q.is_empty() || c.is_empty() {
        return false;
    }
    if q == c {
        return true;
    }
    let ql = query.to_lowercase();
    let cl = candidate.to_lowercase();
    if ql == cl {
        return true;
    }
    // Edition suffixes: "Game Deluxe Edition" vs "Game"
    if q.len() >= 8 && c.len() >= 8 && (q.starts_with(&c) || c.starts_with(&q)) {
        return true;
    }
    false
}

/// Best-effort Steam store genres via public appdetails API.
fn steam_app_genres(app_id: &str) -> Result<Option<String>> {
    let url = format!(
        "https://store.steampowered.com/api/appdetails?appids={app_id}&filters=genres"
    );
    let resp = client()?.get(&url).send()?;
    if !resp.status().is_success() {
        return Ok(None);
    }
    let json: serde_json::Value = resp.json()?;
    let genres = json
        .get(app_id)
        .and_then(|v| v.get("data"))
        .and_then(|v| v.get("genres"))
        .and_then(|v| v.as_array());
    let Some(genres) = genres else {
        return Ok(None);
    };
    let names: Vec<String> = genres
        .iter()
        .filter_map(|g| g.get("description").and_then(|d| d.as_str()))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if names.is_empty() {
        Ok(None)
    } else {
        Ok(Some(names.join(", ")))
    }
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
                || title.to_lowercase().contains("(video game)");
            let title_close = names_close_enough(
                name,
                &title.to_lowercase().replace("(video game)", ""),
            );
            if !is_game || !title_close {
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

#[allow(dead_code)]
pub fn is_portrait_cover_bytes(bytes: &[u8]) -> bool {
    is_usable_box_art_bytes(bytes)
}

#[allow(dead_code)]
pub fn cover_as_data_url(path: &str) -> Result<String> {
    let requested = Path::new(path);
    let path_canon = requested
        .canonicalize()
        .map_err(|_| anyhow::anyhow!("Cover file not found"))?;
    if !is_inside_covers_dir(&path_canon) {
        anyhow::bail!("Cover path is outside the covers folder");
    }

    let bytes = fs::read(&path_canon)?;
    if detect_image_ext(&bytes).is_none() {
        anyhow::bail!("Cover file is not a supported image");
    }
    let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes);
    let mime = if path_canon.extension().and_then(|e| e.to_str()) == Some("png")
        || bytes.starts_with(&[0x89, b'P', b'N', b'G'])
    {
        "image/png"
    } else if path_canon.extension().and_then(|e| e.to_str()) == Some("webp")
        || (bytes.len() >= 12 && &bytes[8..12] == b"WEBP")
    {
        "image/webp"
    } else if path_canon.extension().and_then(|e| e.to_str()) == Some("gif")
        || bytes.starts_with(b"GIF")
    {
        "image/gif"
    } else {
        "image/jpeg"
    };
    Ok(format!("data:{mime};base64,{b64}"))
}

fn win_norm_path(p: &Path) -> String {
    p.to_string_lossy()
        .replacen(r"\\?\", "", 1)
        .replace('/', "\\")
        .to_ascii_lowercase()
}

fn is_inside_covers_dir(path: &Path) -> bool {
    let covers = covers_dir();
    let covers_canon = covers.canonicalize().unwrap_or_else(|_| covers.clone());
    let file_canon = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let parent = win_norm_path(&covers_canon);
    let child = win_norm_path(&file_canon);
    child == parent || child.starts_with(&(parent.clone() + "\\"))
}

/// Read a cover/logo file from the covers folder by filename (no path traversal).
pub fn read_named_cover(name: &str) -> Option<(Vec<u8>, &'static str)> {
    let file_name = Path::new(name).file_name()?.to_str()?;
    if file_name.is_empty() || file_name == "." || file_name == ".." || file_name.contains("..") {
        return None;
    }
    let path = covers_dir().join(file_name);
    if !path.is_file() || !is_inside_covers_dir(&path) {
        return None;
    }
    let len = fs::metadata(&path).ok()?.len();
    if len < 24 || len > MAX_COVER_BYTES {
        return None;
    }
    let bytes = fs::read(&path).ok()?;
    let mime = match detect_image_ext(&bytes) {
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("bmp") => "image/bmp",
        Some("jpg") => "image/jpeg",
        _ => "image/jpeg",
    };
    Some((bytes, mime))
}
