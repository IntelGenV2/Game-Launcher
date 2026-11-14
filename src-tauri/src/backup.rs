use crate::{commands::AppState, db::{self, Database}, game_watch};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{fs::{self, File}, io::{Read, Write}, path::{Path, PathBuf}, sync::atomic::Ordering};
use tauri::Manager;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive};

const FORMAT: u32 = 1;
const MAX_BYTES: u64 = 2 * 1024 * 1024 * 1024;
#[derive(Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
struct Manifest { format_version:u32, created_at:String }
#[derive(Serialize)]
#[serde(rename_all="camelCase")]
pub struct BackupPreview { pub game_count:i64, pub art_count:usize, pub created_at:Option<String>, pub size_bytes:u64 }
struct Scratch(PathBuf);
impl Drop for Scratch { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
fn scratch() -> Result<Scratch> {
    let path=std::env::temp_dir().join(format!("intelgen-backup-{}",uuid::Uuid::new_v4()));
    fs::create_dir(&path)?;
    Ok(Scratch(path))
}

pub fn export_library_zip(database:&Database, dest:&str) -> Result<String> {
    export_from(database, &db::app_data_dir(), Path::new(dest))
}
fn export_from(database:&Database, root:&Path, dest:&Path) -> Result<String> {
    anyhow::ensure!(dest.extension().is_some_and(|x|x.eq_ignore_ascii_case("zip")),"Choose a .zip backup filename");
    if let Some(parent)=dest.parent() { fs::create_dir_all(parent)?; }
    if let Ok(covers)=root.join("covers").canonicalize() {
        anyhow::ensure!(!dest.parent().context("Choose a backup folder")?.canonicalize()?.starts_with(covers), "Save the backup outside the artwork folder");
    }
    let temp=scratch()?;
    database.snapshot(&temp.0.join("library.db"))?;
    let part=dest.with_file_name(format!(".intelgen-{}.zip",uuid::Uuid::new_v4()));
    let result=(|| -> Result<()> {
        let mut zip=zip::ZipWriter::new(File::create(&part)?);
        let options=SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        zip.start_file("manifest.json", options)?;
        zip.write_all(&serde_json::to_vec(&Manifest { format_version:FORMAT, created_at:chrono::Utc::now().to_rfc3339() })?)?;
        zip.start_file("library.db", options)?;
        std::io::copy(&mut File::open(temp.0.join("library.db"))?, &mut zip)?;
        let covers=root.join("covers");
        if covers.exists() {
            for entry in fs::read_dir(covers)? {
                let entry=entry?;
                if !entry.file_type()?.is_file() { continue; }
                zip.start_file(format!("covers/{}",entry.file_name().to_string_lossy()),options)?;
                std::io::copy(&mut File::open(entry.path())?, &mut zip)?;
            }
        }
        zip.finish()?.sync_all()?;
        // Keep the previous export until the replacement has been written successfully.
        if dest.exists() {
            let previous=dest.with_file_name(format!(".intelgen-old-{}.zip",uuid::Uuid::new_v4()));
            fs::rename(dest,&previous)?;
            if let Err(error)=fs::rename(&part,dest) { let _=fs::rename(&previous,dest); return Err(error.into()); }
            let _=fs::remove_file(previous);
        } else { fs::rename(&part,dest)?; }
        Ok(())
    })();
    if result.is_err() { let _=fs::remove_file(&part); }
    result?;
    Ok(dest.to_string_lossy().into())
}

fn allowed_name(name:&str) -> bool {
    if name=="library.db" || name=="manifest.json" { return true; }
    name.strip_prefix("covers/").is_some_and(|tail| !tail.is_empty() && tail!="." && tail!=".." &&
        !tail.contains(['/', '\\', ':']) && !tail.ends_with([' ', '.']))
}
fn extract(path:&Path, destination:&Path) -> Result<BackupPreview> {
    let mut archive=ZipArchive::new(File::open(path).context("Open backup")?)?;
    anyhow::ensure!(archive.len()<=20000,"Backup contains too many files");
    let mut size=0u64;
    let mut art_count=0;
    let mut seen=std::collections::HashSet::new();
    for i in 0..archive.len() {
        let mut file=archive.by_index(i)?;
        let name=file.name().to_string();
        if file.is_dir() && name=="covers/" { continue; }
        anyhow::ensure!(allowed_name(&name) && file.enclosed_name().is_some(),"Unsupported or unsafe backup entry: {name}");
        anyhow::ensure!(seen.insert(name.to_lowercase()),"Duplicate backup entry: {name}");
        anyhow::ensure!(file.unix_mode().map(|m| m & 0o170000 != 0o120000).unwrap_or(true),"Backup contains a symbolic link");
        size=size.checked_add(file.size()).context("Backup too large")?;
        anyhow::ensure!(size<=MAX_BYTES,"Backup exceeds the 2 GB restore limit");
        let target=destination.join(&name);
        if let Some(parent)=target.parent() { fs::create_dir_all(parent)?; }
        let expected=file.size();
        let written=std::io::copy(&mut file.by_ref().take(expected+1),&mut File::create(target)?)?;
        anyhow::ensure!(written==expected,"Damaged backup entry");
        if name.starts_with("covers/") { art_count+=1; }
    }
    anyhow::ensure!(seen.contains("library.db"),"This zip does not contain a Studio 256 library");
    let manifest_path=destination.join("manifest.json");
    let created_at=if manifest_path.exists() {
        anyhow::ensure!(fs::metadata(&manifest_path)?.len()<16384,"Invalid backup manifest");
        let manifest:Manifest=serde_json::from_slice(&fs::read(manifest_path)?)?;
        anyhow::ensure!(manifest.format_version==FORMAT,"This backup requires a newer launcher");
        Some(manifest.created_at)
    } else { None };
    let db=rusqlite::Connection::open_with_flags(destination.join("library.db"),rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let check:String=db.query_row("PRAGMA quick_check",[],|r|r.get(0))?;
    anyhow::ensure!(check=="ok","The backup database is damaged");
    let game_count=db.query_row("SELECT COUNT(*) FROM games",[],|r|r.get(0))?;
    db.prepare("SELECT key,value FROM settings")?;
    Ok(BackupPreview { game_count, art_count, created_at, size_bytes:size })
}

pub fn preview(path:&str) -> Result<BackupPreview> {
    let temp=scratch()?;
    extract(Path::new(path),&temp.0)
}

pub fn stage_restore(state:&AppState,path:&str) -> Result<String> {
    let _operation=state.operation.lock().map_err(|e|anyhow::anyhow!(e.to_string()))?;
    anyhow::ensure!(!game_watch::active(state),"Finish running games before restoring a library");
    anyhow::ensure!(!state.maintenance.swap(true,Ordering::SeqCst),"A restore is already prepared");
    let result=(|| -> Result<String> {
        let root=db::app_data_dir();
        let stage=root.with_file_name(format!("IntelLauncher-restore-{}",uuid::Uuid::new_v4()));
        let staged=Scratch(stage);
        fs::create_dir(&staged.0)?;
        extract(Path::new(path),&staged.0)?;
        {
            let restored=Database::open(&staged.0.join("library.db"))?;
            restored.relocate_art(&root)?;
            restored.recover_sessions()?;
        }
        let safety=root.join("backups").join(format!("before-restore-{}.zip",chrono::Utc::now().format("%Y%m%d-%H%M%S")));
        {
            let database=state.db.lock().map_err(|e|anyhow::anyhow!(e.to_string()))?;
            export_from(&database,&root,&safety)?;
        }
        // Keep the recovery zip accessible from Settings after the directory swap.
        let recovery=staged.0.join("backups");
        fs::create_dir_all(&recovery)?;
        fs::copy(&safety,recovery.join(safety.file_name().context("Invalid recovery filename")?))?;
        fs::write(root.join(".restore-request"),staged.0.to_string_lossy().as_bytes())?;
        std::mem::forget(staged);
        Ok(safety.to_string_lossy().into())
    })();
    if result.is_err() { state.maintenance.store(false,Ordering::SeqCst); }
    result
}

/// Switch complete data directories before SQLite is opened. Roll back if activation fails.
pub fn apply_pending_restore() -> Result<()> {
    let root=db::app_data_dir();
    apply_pending_restore_at(&root)
}

fn apply_pending_restore_at(root:&Path) -> Result<()> {
    let request=root.join(".restore-request");
    if !request.exists() { return Ok(()); }
    let staged=PathBuf::from(fs::read_to_string(&request)?);
    let parent=root.parent().context("Invalid data folder")?.canonicalize()?;
    anyhow::ensure!(staged.parent().context("Invalid restore folder")?.canonicalize()?==parent &&
        staged.file_name().unwrap_or_default().to_string_lossy().starts_with("IntelLauncher-restore-"),"Invalid restore location");
    // Validate the prepared database again before touching the existing library.
    { let checked=rusqlite::Connection::open_with_flags(staged.join("library.db"),rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
      let status:String=checked.query_row("PRAGMA quick_check",[],|r|r.get(0))?;
      anyhow::ensure!(status=="ok","Prepared restore is damaged"); }
    let previous=root.with_file_name(format!("IntelLauncher-before-restore-{}",uuid::Uuid::new_v4()));
    fs::rename(&root,&previous)?;
    if let Err(error)=fs::rename(&staged,&root) {
        fs::rename(&previous,&root).context("Rollback restore")?;
        return Err(error.into());
    }
    // Retain the entire old data folder as an additional recovery copy.
    let _=fs::remove_file(previous.join(".restore-request"));
    fs::write(root.join("restore-recovery.txt"),previous.to_string_lossy().as_bytes())?;
    Ok(())
}

pub fn start_automatic_backups(app:tauri::AppHandle) {
    std::thread::spawn(move || loop {
        {
            let state=app.state::<AppState>();
            if !state.maintenance.load(Ordering::SeqCst) {
                if let Ok(database)=state.db.lock() {
                    if database.get_settings().ok().and_then(|s|s.automatic_backups).unwrap_or(true) {
                        let folder=db::app_data_dir().join("backups");
                        let path=folder.join(format!("auto-{}.zip",chrono::Local::now().format("%Y-%m-%d")));
                        if !path.exists() && export_from(&database,&db::app_data_dir(),&path).is_ok() {
                            if let Ok(entries)=fs::read_dir(&folder) {
                                let mut files:Vec<_>=entries.flatten().map(|e|e.path()).filter(|p|p.is_file() && p.file_name().unwrap_or_default().to_string_lossy().starts_with("auto-") && p.extension().is_some_and(|x|x=="zip")).collect();
                                files.sort();
                                let remove=files.len().saturating_sub(7);
                                for old in files.into_iter().take(remove) { let _=fs::remove_file(old); }
                            }
                        }
                    }
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(900));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_paths_outside_backup_contents() {
        for name in ["../library.db","covers/../../a","covers/C:bad","covers/a\\b","/library.db","library.db-wal","covers/.."] { assert!(!allowed_name(name),"{name}"); }
        assert!(allowed_name("library.db"));
        assert!(allowed_name("covers/steam_123.jpg"));
    }
    #[test]
    fn round_trip_restores_database_and_art_without_credentials() {
        let temp=scratch().unwrap();
        let root=temp.0.join("source");
        fs::create_dir_all(root.join("covers")).unwrap();
        let db=Database::open(&root.join("library.db")).unwrap();
        db.add_manual_game("test","Test","game.exe",None).unwrap();
        let mut settings=crate::models::AppSettings::default();
        settings.twitch_client_secret=Some("private-test-value".into());
        db.save_settings(&settings).unwrap();
        fs::write(root.join("covers/test.png"),b"test artwork").unwrap();
        let zip=temp.0.join("backup.zip");
        export_from(&db,&root,&zip).unwrap();
        let dest=temp.0.join("restored");
        let info=extract(&zip,&dest).unwrap();
        assert_eq!(info.game_count,1);
        assert_eq!(info.art_count,1);
        let restored=Database::open(&dest.join("library.db")).unwrap();
        assert_eq!(restored.get_game("test").unwrap().unwrap().name,"Test");
        assert!(restored.get_settings().unwrap().twitch_client_secret.is_none());
        let bytes=fs::read(dest.join("library.db")).unwrap();
        assert!(!bytes.windows(b"private-test-value".len()).any(|w|w==b"private-test-value"));
        assert_eq!(fs::read(dest.join("covers/test.png")).unwrap(),b"test artwork");
    }

    #[test]
    fn restore_switch_retains_the_original_library_and_rejects_invalid_stage() {
        let temp=scratch().unwrap();
        let root=temp.0.join("IntelLauncher");
        let stage=temp.0.join("IntelLauncher-restore-test");
        let old=Database::open(&root.join("library.db")).unwrap();
        old.add_manual_game("old","Original","old.exe",None).unwrap();
        drop(old);
        fs::create_dir(&stage).unwrap();
        fs::write(root.join(".restore-request"),stage.to_string_lossy().as_bytes()).unwrap();
        assert!(apply_pending_restore_at(&root).is_err());
        assert!(root.join("library.db").exists());
        let new=Database::open(&stage.join("library.db")).unwrap();
        new.add_manual_game("new","Restored","new.exe",None).unwrap();
        drop(new);
        apply_pending_restore_at(&root).unwrap();
        let restored=Database::open(&root.join("library.db")).unwrap();
        assert!(restored.get_game("new").unwrap().is_some());
        let previous=PathBuf::from(fs::read_to_string(root.join("restore-recovery.txt")).unwrap());
        assert!(Database::open(&previous.join("library.db")).unwrap().get_game("old").unwrap().is_some());
        assert!(!previous.join(".restore-request").exists());
    }
}
