use crate::models::{DiscoveredGame, Store};
use anyhow::Result;
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};

/// Playnite AmazonGamesLibrary.GetInstalledGames:
/// `%LOCALAPPDATA%\Amazon Games\Data\Games\Sql\GameInstallInfo.sqlite`
/// `SELECT * FROM DbSet WHERE Installed = 1`
pub fn scan() -> Result<Vec<DiscoveredGame>> {
    let mut games = Vec::new();
    let Some(local) = dirs::data_local_dir() else {
        return Ok(games);
    };
    let db = local
        .join("Amazon Games")
        .join("Data")
        .join("Games")
        .join("Sql")
        .join("GameInstallInfo.sqlite");
    if !db.is_file() {
        return Ok(games);
    }

    let conn = match open_ro(&db) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Amazon Games sqlite: {e:#}");
            return Ok(games);
        }
    };

    let mut stmt = match conn.prepare(
        "SELECT Id, ProductTitle, InstallDirectory FROM DbSet WHERE Installed = 1",
    ) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Amazon Games query: {e:#}");
            return Ok(games);
        }
    };

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    });
    let Ok(rows) = rows else {
        return Ok(games);
    };

    let mut seen = std::collections::HashSet::new();
    for row in rows.flatten() {
        let (id, title, install) = row;
        if id.is_empty() || !Path::new(&install).is_dir() {
            continue;
        }
        if !seen.insert(id.clone()) {
            continue;
        }
        games.push(DiscoveredGame {
            id: format!("amazon:{id}"),
            name: title,
            store: Store::Amazon,
            // Playnite default play action: amazon-games://play/{GameId}
            launch_target: format!("amazon-games://play/{id}"),
            install_path: Some(install),
            steam_app_id: None,
            playtime_minutes: None,
        });
    }

    Ok(games)
}

fn open_ro(path: &PathBuf) -> rusqlite::Result<Connection> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
}
