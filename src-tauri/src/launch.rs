use crate::models::{Game, Store};
use anyhow::{bail, Context, Result};
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn launch_game(game: &Game) -> Result<()> {
    // Prefer a real on-disk executable when the launch target points at one
    // (includes user path resets for missing Xbox/manual installs).
    let target = Path::new(&game.launch_target);
    if target.is_file() {
        return spawn_exe(&game.launch_target);
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
                spawn_exe(&game.launch_target)
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
                spawn_exe(&game.launch_target)
            }
        }
    }
}

fn open_uri(uri: &str) -> Result<()> {
    Command::new("cmd")
        .args(["/C", "start", "", uri])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .context("failed to open URI")?;
    Ok(())
}

fn spawn_exe(path: &str) -> Result<()> {
    let p = Path::new(path);
    if !p.exists() {
        bail!("Executable not found: {path}");
    }
    let dir = p
        .parent()
        .map(|d| d.to_path_buf())
        .unwrap_or_else(|| Path::new(".").to_path_buf());

    // ShellExecute via `start` handles UAC/"requires elevation" (os error 740)
    // better than CreateProcess, which cannot elevate.
    match Command::new("cmd")
        .current_dir(&dir)
        .args(["/C", "start", "", path])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
    {
        Ok(_) => return Ok(()),
        Err(e) => {
            // Fall through to direct spawn / elevated retry
            let _ = e;
        }
    }

    let mut cmd = Command::new(path);
    cmd.current_dir(&dir);
    match cmd.spawn() {
        Ok(_) => Ok(()),
        Err(e) if is_elevation_required(&e) => spawn_elevated(path, &dir),
        Err(e) => Err(e).context("failed to spawn executable"),
    }
}

fn is_elevation_required(err: &std::io::Error) -> bool {
    err.raw_os_error() == Some(740)
}

/// Prompt for admin via UAC (Start-Process -Verb RunAs).
fn spawn_elevated(path: &str, dir: &Path) -> Result<()> {
    let path_ps = path.replace('\'', "''");
    let dir_ps = dir.to_string_lossy().replace('\'', "''");
    let script =
        format!("Start-Process -FilePath '{path_ps}' -WorkingDirectory '{dir_ps}' -Verb RunAs");
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

pub fn open_folder(path: &str) -> Result<()> {
    Command::new("explorer")
        .arg(path)
        .spawn()
        .context("failed to open folder")?;
    Ok(())
}
