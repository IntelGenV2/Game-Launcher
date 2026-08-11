use crate::models::{
    AppSettings, DailyPlaytime, DiscoveredGame, Game, GameGroup, GameStats, PlaySession, Store,
};
use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use rusqlite::{params, Connection};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const APP_DATA_FOLDER: &str = "IntelLauncher";
const LEGACY_APP_DATA_FOLDER: &str = "UnifiedGameLauncher";

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path).context("open sqlite")?;
        let db = Self { conn };
        db.migrate()?;
        db.rewrite_legacy_cover_paths()?;
        Ok(db)
    }

    /// Fix absolute cover paths after migrating UnifiedGameLauncher → IntelLauncher.
    fn rewrite_legacy_cover_paths(&self) -> Result<()> {
        let _ = self.conn.execute(
            "UPDATE games SET cover_path = REPLACE(cover_path, ?1, ?2) WHERE cover_path LIKE ?3",
            params![
                LEGACY_APP_DATA_FOLDER,
                APP_DATA_FOLDER,
                format!("%{LEGACY_APP_DATA_FOLDER}%")
            ],
        );
        Ok(())
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS games (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                store TEXT NOT NULL,
                launch_target TEXT NOT NULL,
                install_path TEXT,
                cover_url TEXT,
                cover_path TEXT,
                favorite INTEGER NOT NULL DEFAULT 0,
                hidden INTEGER NOT NULL DEFAULT 0,
                missing INTEGER NOT NULL DEFAULT 0,
                playtime_minutes INTEGER NOT NULL DEFAULT 0,
                last_played_at TEXT,
                date_added TEXT NOT NULL,
                steam_app_id TEXT,
                path_override INTEGER NOT NULL DEFAULT 0,
                genre TEXT
            );

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                game_id TEXT NOT NULL,
                started_at TEXT NOT NULL,
                ended_at TEXT,
                avg_fps REAL
            );

            CREATE TABLE IF NOT EXISTS fps_samples (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                game_id TEXT NOT NULL,
                recorded_at TEXT NOT NULL,
                fps REAL NOT NULL,
                note TEXT
            );

            CREATE TABLE IF NOT EXISTS game_groups (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                sort_order INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS game_group_members (
                group_id TEXT NOT NULL,
                game_id TEXT NOT NULL,
                sort_order INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (group_id, game_id)
            );
            "#,
        )?;
        // Migrations for existing DBs
        let _ = self
            .conn
            .execute("ALTER TABLE sessions ADD COLUMN avg_fps REAL", []);
        let _ = self.conn.execute(
            "ALTER TABLE games ADD COLUMN path_override INTEGER NOT NULL DEFAULT 0",
            [],
        );
        let _ = self
            .conn
            .execute("ALTER TABLE games ADD COLUMN genre TEXT", []);
        let _ = self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS game_groups (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                sort_order INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS game_group_members (
                group_id TEXT NOT NULL,
                game_id TEXT NOT NULL,
                sort_order INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (group_id, game_id)
            );
            "#,
        );
        Ok(())
    }

    fn map_game(row: &rusqlite::Row<'_>) -> rusqlite::Result<Game> {
        Ok(Game {
            id: row.get("id")?,
            name: row.get("name")?,
            store: Store::from_str(&row.get::<_, String>("store")?).unwrap_or(Store::Manual),
            launch_target: row.get("launch_target")?,
            install_path: row.get("install_path")?,
            cover_url: row.get("cover_url")?,
            cover_path: row.get("cover_path")?,
            favorite: row.get::<_, i64>("favorite")? != 0,
            hidden: row.get::<_, i64>("hidden")? != 0,
            missing: row.get::<_, i64>("missing")? != 0,
            playtime_minutes: row.get("playtime_minutes")?,
            last_played_at: row.get("last_played_at")?,
            date_added: row.get("date_added")?,
            steam_app_id: row.get("steam_app_id")?,
            genre: row.get("genre")?,
        })
    }

    pub fn list_games(&self, include_hidden: bool) -> Result<Vec<Game>> {
        let sql = if include_hidden {
            "SELECT id, name, store, launch_target, install_path, cover_url, cover_path,
                    favorite, hidden, missing, playtime_minutes, last_played_at, date_added,
                    steam_app_id, genre
             FROM games ORDER BY name COLLATE NOCASE"
        } else {
            "SELECT id, name, store, launch_target, install_path, cover_url, cover_path,
                    favorite, hidden, missing, playtime_minutes, last_played_at, date_added,
                    steam_app_id, genre
             FROM games WHERE hidden = 0 ORDER BY name COLLATE NOCASE"
        };
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map([], Self::map_game)?;
        let mut games = Vec::new();
        for g in rows {
            games.push(g?);
        }
        Ok(games)
    }

    pub fn get_game(&self, id: &str) -> Result<Option<Game>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, store, launch_target, install_path, cover_url, cover_path,
                    favorite, hidden, missing, playtime_minutes, last_played_at, date_added,
                    steam_app_id, genre
             FROM games WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], Self::map_game)?;
        Ok(rows.next().transpose()?)
    }

    pub fn upsert_discovered(&self, discovered: &[DiscoveredGame]) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let found_ids: Vec<String> = discovered.iter().map(|g| g.id.clone()).collect();

        let tx = self.conn.unchecked_transaction()?;

        // Mark auto-discovered stores as missing first; manual stays until re-found.
        // Custom path overrides are not auto-marked missing (verified after scan).
        tx.execute(
            "UPDATE games SET missing = 1 WHERE store NOT IN ('manual') AND COALESCE(path_override, 0) = 0",
            [],
        )?;

        for g in discovered {
            let existing = tx.query_row(
                "SELECT favorite, hidden, playtime_minutes, last_played_at, date_added, cover_url, cover_path, COALESCE(path_override, 0) FROM games WHERE id = ?1",
                params![g.id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, Option<String>>(6)?,
                        row.get::<_, i64>(7)?,
                    ))
                },
            );

            match existing {
                Ok((
                    favorite,
                    hidden,
                    playtime,
                    last_played,
                    date_added,
                    _cover_url,
                    _cover_path,
                    path_override,
                )) => {
                    let mut playtime = playtime;
                    if let Some(imported) = g.playtime_minutes {
                        if imported > playtime {
                            playtime = imported;
                        }
                    }
                    if path_override != 0 {
                        // Keep user-chosen launch/install paths
                        tx.execute(
                            r#"UPDATE games SET
                                store = ?1,
                                steam_app_id = COALESCE(steam_app_id, ?2),
                                playtime_minutes = ?3,
                                missing = 0,
                                favorite = ?4,
                                hidden = ?5,
                                last_played_at = ?6,
                                date_added = ?7
                            WHERE id = ?8"#,
                            params![
                                g.store.as_str(),
                                g.steam_app_id,
                                playtime,
                                favorite,
                                hidden,
                                last_played,
                                date_added,
                                g.id
                            ],
                        )?;
                    } else {
                        tx.execute(
                            r#"UPDATE games SET
                                store = ?1,
                                launch_target = ?2,
                                install_path = ?3,
                                steam_app_id = COALESCE(steam_app_id, ?4),
                                playtime_minutes = ?5,
                                missing = 0,
                                favorite = ?6,
                                hidden = ?7,
                                last_played_at = ?8,
                                date_added = ?9
                            WHERE id = ?10"#,
                            params![
                                g.store.as_str(),
                                g.launch_target,
                                g.install_path,
                                g.steam_app_id,
                                playtime,
                                favorite,
                                hidden,
                                last_played,
                                date_added,
                                g.id
                            ],
                        )?;
                    }
                }
                Err(_) => {
                    let playtime = g.playtime_minutes.unwrap_or(0);
                    let cover_url: Option<String> = None; // local cover_path is the source of truth
                    tx.execute(
                        r#"INSERT INTO games (
                            id, name, store, launch_target, install_path, cover_url, cover_path,
                            favorite, hidden, missing, playtime_minutes, last_played_at, date_added, steam_app_id, path_override
                        ) VALUES (?1,?2,?3,?4,?5,?6,NULL,0,0,0,?7,NULL,?8,?9,0)"#,
                        params![
                            g.id,
                            g.name,
                            g.store.as_str(),
                            g.launch_target,
                            g.install_path,
                            cover_url,
                            playtime,
                            now,
                            g.steam_app_id
                        ],
                    )?;
                }
            }
        }

        // Re-check custom path overrides that were not rediscovered
        {
            let mut stmt = tx.prepare(
                "SELECT id, launch_target FROM games WHERE COALESCE(path_override, 0) = 1",
            )?;
            let rows: Vec<(String, String)> = stmt
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                .filter_map(|r| r.ok())
                .collect();
            drop(stmt);
            for (id, launch_target) in rows {
                let exists = std::path::Path::new(&launch_target).exists();
                tx.execute(
                    "UPDATE games SET missing = ?1 WHERE id = ?2",
                    params![(!exists) as i64, id],
                )?;
            }
        }

        let _ = found_ids;
        tx.commit()?;
        Ok(())
    }

    pub fn add_manual_game(
        &self,
        id: &str,
        name: &str,
        launch_target: &str,
        install_path: Option<&str>,
    ) -> Result<Game> {
        let now = Utc::now().to_rfc3339();
        // INSERT only — never REPLACE, so restarts/rescans cannot wipe manual entries.
        self.conn.execute(
            r#"INSERT INTO games (
                id, name, store, launch_target, install_path, cover_url, cover_path,
                favorite, hidden, missing, playtime_minutes, last_played_at, date_added, steam_app_id
            ) VALUES (?1,?2,'manual',?3,?4,NULL,NULL,0,0,0,0,NULL,?5,NULL)"#,
            params![id, name, launch_target, install_path, now],
        )?;
        Ok(self.get_game(id)?.expect("just inserted"))
    }

    pub fn toggle_favorite(&self, id: &str) -> Result<Game> {
        self.conn.execute(
            "UPDATE games SET favorite = CASE favorite WHEN 1 THEN 0 ELSE 1 END WHERE id = ?1",
            params![id],
        )?;
        self.get_game(id)?
            .ok_or_else(|| anyhow::anyhow!("game not found"))
    }

    pub fn set_hidden(&self, id: &str, hidden: bool) -> Result<Game> {
        self.conn.execute(
            "UPDATE games SET hidden = ?1 WHERE id = ?2",
            params![hidden as i64, id],
        )?;
        self.get_game(id)?
            .ok_or_else(|| anyhow::anyhow!("game not found"))
    }

    pub fn set_cover(
        &self,
        id: &str,
        cover_url: Option<&str>,
        cover_path: Option<&str>,
    ) -> Result<()> {
        // Never clear an existing cover when a field is omitted — only overwrite when provided.
        if cover_url.is_some() && cover_path.is_some() {
            self.conn.execute(
                "UPDATE games SET cover_url = ?1, cover_path = ?2 WHERE id = ?3",
                params![cover_url, cover_path, id],
            )?;
        } else if cover_path.is_some() {
            self.conn.execute(
                "UPDATE games SET cover_path = ?1, cover_url = COALESCE(?2, cover_url) WHERE id = ?3",
                params![cover_path, cover_url, id],
            )?;
        } else if cover_url.is_some() {
            self.conn.execute(
                "UPDATE games SET cover_url = ?1 WHERE id = ?2",
                params![cover_url, id],
            )?;
        }
        Ok(())
    }

    pub fn set_steam_app_id(&self, id: &str, steam_app_id: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE games SET steam_app_id = COALESCE(steam_app_id, ?1) WHERE id = ?2",
            params![steam_app_id, id],
        )?;
        Ok(())
    }

    pub fn set_genre(&self, id: &str, genre: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE games SET genre = ?1 WHERE id = ?2",
            params![genre, id],
        )?;
        Ok(())
    }

    /// Point a game at a new executable (or install folder). Clears the missing flag.
    pub fn set_game_path(&self, id: &str, path: &str) -> Result<Game> {
        let p = std::path::Path::new(path);
        if !p.exists() {
            anyhow::bail!("Path not found: {path}");
        }
        let (launch_target, install_path) = if p.is_file() {
            let parent = p
                .parent()
                .map(|d| d.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string());
            (path.to_string(), Some(parent))
        } else {
            (path.to_string(), Some(path.to_string()))
        };

        self.conn.execute(
            "UPDATE games SET launch_target = ?1, install_path = ?2, missing = 0, path_override = 1 WHERE id = ?3",
            params![launch_target, install_path, id],
        )?;
        self.get_game(id)?
            .ok_or_else(|| anyhow::anyhow!("game not found"))
    }

    pub fn set_game_name(&self, id: &str, name: &str) -> Result<Game> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            anyhow::bail!("Name cannot be empty");
        }
        if trimmed.chars().count() > 120 {
            anyhow::bail!("Name is too long");
        }
        self.conn.execute(
            "UPDATE games SET name = ?1 WHERE id = ?2",
            params![trimmed, id],
        )?;
        self.get_game(id)?
            .ok_or_else(|| anyhow::anyhow!("game not found"))
    }

    pub fn remove_game(&self, id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM sessions WHERE game_id = ?1", params![id])?;
        let _ = self
            .conn
            .execute("DELETE FROM fps_samples WHERE game_id = ?1", params![id]);
        self.conn.execute(
            "DELETE FROM game_group_members WHERE game_id = ?1",
            params![id],
        )?;
        self.conn
            .execute("DELETE FROM games WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn record_launch(&self, id: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE games SET last_played_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        self.conn.execute(
            "INSERT INTO sessions (game_id, started_at) VALUES (?1, ?2)",
            params![id, now],
        )?;
        Ok(())
    }

    pub fn end_session_and_add_playtime(&self, id: &str, minutes: i64) -> Result<Game> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE sessions SET ended_at = ?1 WHERE id = (
                SELECT id FROM sessions WHERE game_id = ?2 AND ended_at IS NULL ORDER BY id DESC LIMIT 1
            )",
            params![now, id],
        )?;
        self.conn.execute(
            "UPDATE games SET playtime_minutes = playtime_minutes + ?1, last_played_at = ?2 WHERE id = ?3",
            params![minutes.max(0), now, id],
        )?;
        self.get_game(id)?
            .ok_or_else(|| anyhow::anyhow!("game not found"))
    }

    pub fn game_stats(&self, game_id: &str) -> Result<GameStats> {
        let game = self
            .get_game(game_id)?
            .ok_or_else(|| anyhow::anyhow!("game not found"))?;

        let mut sess_stmt = self.conn.prepare(
            "SELECT id, game_id, started_at, ended_at FROM sessions WHERE game_id = ?1 ORDER BY id ASC",
        )?;
        let sess_rows = sess_stmt.query_map(params![game_id], |row| {
            let started: String = row.get(2)?;
            let ended: Option<String> = row.get(3)?;
            let duration = match &ended {
                Some(e) => {
                    let s = chrono::DateTime::parse_from_rfc3339(&started)
                        .ok()
                        .map(|d| d.with_timezone(&Utc));
                    let e = chrono::DateTime::parse_from_rfc3339(e)
                        .ok()
                        .map(|d| d.with_timezone(&Utc));
                    match (s, e) {
                        (Some(s), Some(e)) => (e - s).num_minutes().max(0),
                        _ => 0,
                    }
                }
                None => 0,
            };
            Ok(PlaySession {
                id: row.get(0)?,
                game_id: row.get(1)?,
                started_at: started,
                ended_at: ended,
                duration_minutes: duration,
            })
        })?;
        let sessions: Vec<PlaySession> = sess_rows.filter_map(|r| r.ok()).collect();

        let mut daily: BTreeMap<String, i64> = BTreeMap::new();
        let today = Utc::now().date_naive();
        for i in 0..14 {
            let day = (today - Duration::days(13 - i))
                .format("%Y-%m-%d")
                .to_string();
            daily.insert(day, 0);
        }
        for s in &sessions {
            if let Ok(started) = chrono::DateTime::parse_from_rfc3339(&s.started_at) {
                let day = started.with_timezone(&Utc).format("%Y-%m-%d").to_string();
                if let Some(slot) = daily.get_mut(&day) {
                    *slot += s.duration_minutes.max(1);
                }
            }
        }
        let daily_playtime: Vec<DailyPlaytime> = daily
            .into_iter()
            .map(|(day, minutes)| DailyPlaytime { day, minutes })
            .collect();

        let session_count = sessions.len() as i64;
        let avg_session = if session_count > 0 {
            sessions.iter().map(|s| s.duration_minutes).sum::<i64>() as f64 / session_count as f64
        } else {
            0.0
        };
        let first_played = sessions.first().map(|s| s.started_at.clone());

        Ok(GameStats {
            game_id: game_id.to_string(),
            total_playtime_minutes: game.playtime_minutes,
            session_count,
            avg_session_minutes: avg_session,
            last_played_at: game.last_played_at,
            first_played_at: first_played,
            daily_playtime,
            sessions,
        })
    }

    pub fn get_settings(&self) -> Result<AppSettings> {
        let mut settings = AppSettings::default();
        let mut stmt = self.conn.prepare("SELECT key, value FROM settings")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (k, v) = row?;
            match k.as_str() {
                "steam_grid_db_api_key" => settings.steam_grid_db_api_key = Some(v),
                "sort_by" => settings.sort_by = Some(v),
                "theme" => settings.theme = Some(v),
                "card_scale" => {
                    if let Ok(n) = v.parse::<f64>() {
                        settings.card_scale = Some(n);
                    }
                }
                "library_order" => settings.library_order = Some(v),
                _ => {}
            }
        }
        Ok(settings)
    }

    pub fn save_settings(&self, settings: &AppSettings) -> Result<()> {
        if let Some(key) = &settings.steam_grid_db_api_key {
            self.conn.execute(
                "INSERT INTO settings (key, value) VALUES ('steam_grid_db_api_key', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key],
            )?;
        } else {
            self.conn.execute(
                "DELETE FROM settings WHERE key = 'steam_grid_db_api_key'",
                [],
            )?;
        }
        if let Some(sort) = &settings.sort_by {
            self.conn.execute(
                "INSERT INTO settings (key, value) VALUES ('sort_by', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![sort],
            )?;
        }
        if let Some(theme) = &settings.theme {
            self.conn.execute(
                "INSERT INTO settings (key, value) VALUES ('theme', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![theme],
            )?;
        }
        if let Some(scale) = settings.card_scale {
            self.conn.execute(
                "INSERT INTO settings (key, value) VALUES ('card_scale', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![scale.to_string()],
            )?;
        }
        if let Some(order) = &settings.library_order {
            self.conn.execute(
                "INSERT INTO settings (key, value) VALUES ('library_order', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![order],
            )?;
        }
        Ok(())
    }

    pub fn list_groups(&self) -> Result<Vec<GameGroup>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, sort_order, created_at FROM game_groups ORDER BY sort_order, name COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        let mut groups = Vec::new();
        for row in rows {
            let (id, name, sort_order, created_at) = row?;
            let game_ids = self.group_member_ids(&id)?;
            groups.push(GameGroup {
                id,
                name,
                sort_order,
                created_at,
                game_ids,
            });
        }
        Ok(groups)
    }

    fn group_member_ids(&self, group_id: &str) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT game_id FROM game_group_members WHERE group_id = ?1 ORDER BY sort_order, game_id",
        )?;
        let rows = stmt.query_map(params![group_id], |row| row.get::<_, String>(0))?;
        let mut ids = Vec::new();
        for r in rows {
            ids.push(r?);
        }
        Ok(ids)
    }

    pub fn create_group(&self, name: &str, game_ids: &[String]) -> Result<GameGroup> {
        let name = name.trim();
        if name.is_empty() {
            anyhow::bail!("Group name cannot be empty");
        }
        let id = format!("group:{}", uuid::Uuid::new_v4());
        let now = Utc::now().to_rfc3339();
        let sort_order: i64 = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM game_groups",
                [],
                |row| row.get(0),
            )
            .unwrap_or(1);
        self.conn.execute(
            "INSERT INTO game_groups (id, name, sort_order, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![id, name, sort_order, now],
        )?;
        for (i, gid) in game_ids.iter().enumerate() {
            // A game can only belong to one group — move it.
            self.conn.execute(
                "DELETE FROM game_group_members WHERE game_id = ?1",
                params![gid],
            )?;
            self.conn.execute(
                "INSERT INTO game_group_members (group_id, game_id, sort_order) VALUES (?1, ?2, ?3)",
                params![id, gid, i as i64],
            )?;
        }
        Ok(GameGroup {
            id: id.clone(),
            name: name.to_string(),
            sort_order,
            created_at: now,
            game_ids: self.group_member_ids(&id)?,
        })
    }

    pub fn rename_group(&self, id: &str, name: &str) -> Result<GameGroup> {
        let name = name.trim();
        if name.is_empty() {
            anyhow::bail!("Group name cannot be empty");
        }
        let n = self.conn.execute(
            "UPDATE game_groups SET name = ?1 WHERE id = ?2",
            params![name, id],
        )?;
        if n == 0 {
            anyhow::bail!("Group not found");
        }
        self.get_group(id)?
            .ok_or_else(|| anyhow::anyhow!("Group not found"))
    }

    pub fn delete_group(&self, id: &str) -> Result<()> {
        // Members only — never delete games.
        self.conn.execute(
            "DELETE FROM game_group_members WHERE group_id = ?1",
            params![id],
        )?;
        let n = self
            .conn
            .execute("DELETE FROM game_groups WHERE id = ?1", params![id])?;
        if n == 0 {
            anyhow::bail!("Group not found");
        }
        Ok(())
    }

    pub fn add_game_to_group(&self, group_id: &str, game_id: &str) -> Result<GameGroup> {
        if self.get_group(group_id)?.is_none() {
            anyhow::bail!("Group not found");
        }
        if self.get_game(game_id)?.is_none() {
            anyhow::bail!("Game not found");
        }
        self.conn.execute(
            "DELETE FROM game_group_members WHERE game_id = ?1",
            params![game_id],
        )?;
        let next: i64 = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM game_group_members WHERE group_id = ?1",
                params![group_id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        self.conn.execute(
            "INSERT INTO game_group_members (group_id, game_id, sort_order) VALUES (?1, ?2, ?3)",
            params![group_id, game_id, next],
        )?;
        self.get_group(group_id)?
            .ok_or_else(|| anyhow::anyhow!("Group not found"))
    }

    pub fn remove_game_from_group(&self, group_id: &str, game_id: &str) -> Result<GameGroup> {
        self.conn.execute(
            "DELETE FROM game_group_members WHERE group_id = ?1 AND game_id = ?2",
            params![group_id, game_id],
        )?;
        self.get_group(group_id)?
            .ok_or_else(|| anyhow::anyhow!("Group not found"))
    }

    fn get_group(&self, id: &str) -> Result<Option<GameGroup>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, sort_order, created_at FROM game_groups WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        let Some(row) = rows.next().transpose()? else {
            return Ok(None);
        };
        let (gid, name, sort_order, created_at) = row;
        Ok(Some(GameGroup {
            id: gid.clone(),
            name,
            sort_order,
            created_at,
            game_ids: self.group_member_ids(&gid)?,
        }))
    }
}

pub fn app_data_dir() -> PathBuf {
    let root = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    let new_dir = root.join(APP_DATA_FOLDER);
    let old_dir = root.join(LEGACY_APP_DATA_FOLDER);
    migrate_legacy_app_data(&old_dir, &new_dir);
    new_dir
}

/// Move `%APPDATA%\UnifiedGameLauncher` → `%APPDATA%\IntelLauncher` (once).
fn migrate_legacy_app_data(old_dir: &Path, new_dir: &Path) {
    if !old_dir.exists() {
        return;
    }

    // Fresh rename when the new folder does not exist yet
    if !new_dir.exists() {
        if std::fs::rename(old_dir, new_dir).is_ok() {
            return;
        }
        // Cross-volume fallback: copy then remove
        if copy_dir_recursive(old_dir, new_dir).is_ok() {
            let _ = std::fs::remove_dir_all(old_dir);
        }
        return;
    }

    // New folder already exists — copy any missing pieces, then remove old if empty-ish
    let _ = copy_dir_recursive(old_dir, new_dir);
    // Only delete old if it looks fully migrated (library.db present in new)
    if new_dir.join("library.db").exists() {
        let _ = std::fs::remove_dir_all(old_dir);
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else if !to.exists() {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

pub fn default_db_path() -> PathBuf {
    app_data_dir().join("library.db")
}

pub fn covers_dir() -> PathBuf {
    app_data_dir().join("covers")
}
