use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const MAX_ENTRIES: usize = 400;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplorerPlace {
    pub id: String,
    pub name: String,
    pub path: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplorerEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub kind: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplorerListing {
    pub path: Option<String>,
    pub parent: Option<String>,
    pub label: String,
    pub entries: Vec<ExplorerEntry>,
}

#[link(name = "kernel32")]
extern "system" {
    fn GetLogicalDrives() -> u32;
}

#[link(name = "user32")]
extern "system" {
    fn LockWorkStation() -> i32;
}

#[tauri::command]
pub fn explorer_places() -> Vec<ExplorerPlace> {
    let mut places = vec![ExplorerPlace {
        id: "pc".into(),
        name: "This PC".into(),
        path: None,
    }];
    push_place(&mut places, "desktop", "Desktop", dirs::desktop_dir());
    push_place(&mut places, "documents", "Documents", dirs::document_dir());
    push_place(&mut places, "downloads", "Downloads", dirs::download_dir());
    push_place(&mut places, "pictures", "Pictures", dirs::picture_dir());
    push_place(&mut places, "music", "Music", dirs::audio_dir());
    push_place(&mut places, "videos", "Videos", dirs::video_dir());
    places
}

#[tauri::command]
pub fn list_explorer(path: Option<String>) -> Result<ExplorerListing, String> {
    list_explorer_inner(path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_explorer(path: Option<String>) -> Result<(), String> {
    open_explorer_inner(path.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_path(path: String) -> Result<(), String> {
    open_path_inner(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn system_power(action: String) -> Result<(), String> {
    power(&action).map_err(|e| e.to_string())
}

fn push_place(out: &mut Vec<ExplorerPlace>, id: &str, name: &str, path: Option<PathBuf>) {
    let Some(path) = path else { return };
    if !path.exists() {
        return;
    }
    out.push(ExplorerPlace {
        id: id.into(),
        name: name.into(),
        path: Some(path.to_string_lossy().into_owned()),
    });
}

fn list_explorer_inner(path: Option<String>) -> Result<ExplorerListing> {
    let trimmed = path.as_deref().map(str::trim).filter(|s| !s.is_empty());
    if trimmed.is_none() {
        return Ok(ExplorerListing {
            path: None,
            parent: None,
            label: "This PC".into(),
            entries: logical_drives(),
        });
    }
    let raw = trimmed.unwrap();
    let dir = PathBuf::from(raw);
    if !dir.exists() {
        bail!("Folder not found");
    }
    if !dir.is_dir() {
        bail!("Not a folder");
    }

    let mut entries = Vec::new();
    let reader = std::fs::read_dir(&dir).with_context(|| format!("Can't read {}", dir.display()))?;
    for item in reader.flatten() {
        if entries.len() >= MAX_ENTRIES {
            break;
        }
        let name = item.file_name().to_string_lossy().into_owned();
        if skip_name(&name) {
            continue;
        }
        let full = item.path();
        let is_dir = item.file_type().map(|t| t.is_dir()).unwrap_or(false) || full.is_dir();
        let kind = if is_dir {
            "folder".into()
        } else {
            file_kind(&name)
        };
        entries.push(ExplorerEntry {
            name,
            path: full.to_string_lossy().into_owned(),
            is_dir,
            kind,
        });
    }
    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    Ok(ExplorerListing {
        path: Some(dir.to_string_lossy().into_owned()),
        parent: parent_of(&dir),
        label: label_for(&dir),
        entries,
    })
}

fn skip_name(name: &str) -> bool {
    name.is_empty()
        || name.eq_ignore_ascii_case("desktop.ini")
        || name.eq_ignore_ascii_case("thumbs.db")
        || name.starts_with('$')
        || name.starts_with('.')
}

fn file_kind(name: &str) -> String {
    match Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "exe" | "msi" | "bat" | "cmd" => "app".into(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" => "image".into(),
        "mp4" | "mkv" | "avi" | "webm" | "mov" => "video".into(),
        "mp3" | "wav" | "flac" | "ogg" | "m4a" => "audio".into(),
        "zip" | "rar" | "7z" | "tar" | "gz" => "archive".into(),
        "txt" | "md" | "pdf" | "doc" | "docx" => "doc".into(),
        "lnk" | "url" => "shortcut".into(),
        _ => "file".into(),
    }
}

fn logical_drives() -> Vec<ExplorerEntry> {
    let mask = unsafe { GetLogicalDrives() };
    let mut out = Vec::new();
    for i in 0..26u8 {
        if mask & (1 << i) == 0 {
            continue;
        }
        let letter = (b'A' + i) as char;
        let path = format!("{letter}:\\");
        if !Path::new(&path).exists() {
            continue;
        }
        out.push(ExplorerEntry {
            name: format!("{letter}:"),
            path,
            is_dir: true,
            kind: "drive".into(),
        });
    }
    out
}

fn parent_of(dir: &Path) -> Option<String> {
    let parent = dir.parent()?;
    let s = parent.to_string_lossy();
    if s.is_empty() || parent.as_os_str().is_empty() {
        return None;
    }
    Some(s.into_owned())
}

fn label_for(dir: &Path) -> String {
    dir.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| dir.to_string_lossy().trim_end_matches('\\').to_string())
}

fn open_explorer_inner(path: Option<&str>) -> Result<()> {
    let mut cmd = Command::new("explorer");
    match path.map(str::trim).filter(|s| !s.is_empty()) {
        Some(p) => {
            cmd.arg(p);
        }
        None => {
            cmd.arg("shell:MyComputerFolder");
        }
    }
    cmd.spawn().context("Couldn't open File Explorer")?;
    Ok(())
}

fn open_path_inner(path: &str) -> Result<()> {
    let p = Path::new(path);
    if !p.exists() {
        bail!("Path not found");
    }
    if p.is_dir() {
        return open_explorer_inner(Some(path));
    }
    Command::new("cmd")
        .args(["/C", "start", "", path])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .context("Couldn't open file")?;
    Ok(())
}

fn power(action: &str) -> Result<()> {
    match action {
        "sleep" => {
            Command::new("powershell")
                .args([
                    "-NoProfile",
                    "-WindowStyle",
                    "Hidden",
                    "-Command",
                    "Add-Type -AssemblyName System.Windows.Forms; [void][System.Windows.Forms.Application]::SetSuspendState([System.Windows.Forms.PowerState]::Suspend, $false, $false)",
                ])
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()
                .context("Couldn't sleep")?;
        }
        "lock" => {
            let ok = unsafe { LockWorkStation() };
            if ok == 0 {
                bail!("Couldn't lock the PC");
            }
        }
        "restart" => {
            Command::new("shutdown")
                .args(["/r", "/t", "0"])
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()
                .context("Couldn't restart")?;
        }
        "shutdown" => {
            Command::new("shutdown")
                .args(["/s", "/t", "0"])
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()
                .context("Couldn't shut down")?;
        }
        _ => bail!("Unknown power action"),
    }
    Ok(())
}
