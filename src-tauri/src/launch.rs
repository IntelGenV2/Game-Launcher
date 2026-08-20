use crate::models::{Game, Store};
use anyhow::{bail, Context, Result};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn launch_game(game: &Game) -> Result<()> {
    let extra_args = split_args(game.launch_args.as_deref());
    let cwd = game
        .working_dir
        .as_deref()
        .map(PathBuf::from)
        .filter(|p| p.exists());
    let elevate = game.run_as_admin;

    let target = Path::new(&game.launch_target);
    if target.is_file() {
        return spawn_exe(&game.launch_target, extra_args, cwd.as_deref(), elevate);
    }

    match game.store {
        Store::Steam => {
            let uri = format!("steam://rungameid/{}", game.launch_target);
            open_uri(&uri)
        }
        Store::Epic | Store::Gog | Store::Battlenet | Store::Ubisoft | Store::Amazon => {
            if game.launch_target.contains("://") {
                open_uri(&game.launch_target)
            } else {
                spawn_exe(&game.launch_target, extra_args, cwd.as_deref(), elevate)
            }
        }
        Store::Riot => crate::scanners::riot::launch(game),
        Store::Rockstar => crate::scanners::rockstar::launch(game),
        Store::Xbox
        | Store::Ea
        | Store::Roblox
        | Store::Wargaming
        | Store::Itch
        | Store::Humble
        | Store::Manual => {
            if game.launch_target.contains("://") {
                open_uri(&game.launch_target)
            } else {
                spawn_exe(&game.launch_target, extra_args, cwd.as_deref(), elevate)
            }
        }
    }
}

#[allow(dead_code)]
pub fn launch_path(path: &str, as_admin: bool) -> Result<()> {
    let p = Path::new(path);
    if p.is_dir() {
        return open_folder(path);
    }
    spawn_exe(path, Vec::new(), None, as_admin)
}

fn open_uri(uri: &str) -> Result<()> {
    Command::new("cmd")
        .args(["/C", "start", "", uri])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .context("failed to open URI")?;
    Ok(())
}

fn spawn_exe(path: &str, extra_args: Vec<String>, cwd: Option<&Path>, elevate: bool) -> Result<()> {
    let p = Path::new(path);
    if !p.exists() {
        bail!("Executable not found: {path}");
    }
    let dir = cwd
        .map(|d| d.to_path_buf())
        .or_else(|| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| Path::new(".").to_path_buf());

    if elevate {
        return spawn_elevated(path, &dir, &extra_args);
    }

    if extra_args.is_empty() {
        match Command::new("cmd")
            .current_dir(&dir)
            .args(["/C", "start", "", path])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
        {
            Ok(_) => return Ok(()),
            Err(_) => {}
        }
    }

    let mut cmd = Command::new(path);
    cmd.current_dir(&dir).args(&extra_args);
    match cmd.spawn() {
        Ok(_) => Ok(()),
        Err(e) if is_elevation_required(&e) => spawn_elevated(path, &dir, &extra_args),
        Err(e) => Err(e).context("failed to spawn executable"),
    }
}

fn is_elevation_required(err: &std::io::Error) -> bool {
    err.raw_os_error() == Some(740)
}

fn spawn_elevated(path: &str, dir: &Path, extra_args: &[String]) -> Result<()> {
    let path_ps = path.replace('\'', "''");
    let dir_ps = dir.to_string_lossy().replace('\'', "''");
    let arg_list = if extra_args.is_empty() {
        String::new()
    } else {
        let joined = extra_args
            .iter()
            .map(|a| format!("'{}'", a.replace('\'', "''")))
            .collect::<Vec<_>>()
            .join(",");
        format!(" -ArgumentList {joined}")
    };
    let script = format!(
        "Start-Process -FilePath '{path_ps}' -WorkingDirectory '{dir_ps}'{arg_list} -Verb RunAs"
    );
    Command::new("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .context("failed to launch elevated executable")?;
    Ok(())
}

fn split_args(raw: Option<&str>) -> Vec<String> {
    let Some(raw) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote = None::<char>;
    for c in raw.chars() {
        if let Some(q) = quote {
            if c == q {
                quote = None;
            } else {
                cur.push(c);
            }
        } else if c == '"' || c == '\'' {
            quote = Some(c);
        } else if c.is_whitespace() {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(c);
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

pub fn open_folder(path: &str) -> Result<()> {
    Command::new("explorer")
        .arg(path)
        .spawn()
        .context("failed to open folder")?;
    Ok(())
}

pub fn set_start_with_windows(enabled: bool, background: bool) -> Result<()> {
    use winreg::enums::*;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Run")?;
    const VALUE: &str = "IntelGenGameLauncher";
    if enabled {
        let exe = std::env::current_exe()?;
        let cmd = if background {
            format!("\"{}\" --background", exe.display())
        } else {
            format!("\"{}\"", exe.display())
        };
        key.set_value(VALUE, &cmd)?;
    } else {
        let _ = key.delete_value(VALUE);
    }
    Ok(())
}
