use crate::models::{
    AppSettings, DailyPlaytime, DiscoveredGame, DuplicateGroup, Game, GameCopy, GameGroup, GameStats,
    LibraryOverview, PlaySession, Store, TopPlayedGame, YearInReview,
};
use anyhow::{Context, Result};
use chrono::{Datelike, Duration, Utc};
use rusqlite::{params, Connection};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const APP_DATA_FOLDER: &str = "IntelLauncher";
const LEGACY_APP_DATA_FOLDER: &str = "UnifiedGameLauncher";
const MAX_SESSION_MINUTES: i64 = 12 * 60;

fn clip_text(value: Option<String>, max_chars: usize) -> Option<String> {
    let text = value?.trim().to_string();
    if text.is_empty() {
        return None;
    }
    if text.chars().count() <= max_chars {
        return Some(text);
    }
    Some(text.chars().take(max_chars).collect::<String>() + "…")
}

fn session_duration_minutes(started: &str, ended: Option<&str>) -> i64 {
    let Ok(start) = chrono::DateTime::parse_from_rfc3339(started) else {
        return 0;
    };
    let Some(ended) = ended else {
        return 0;
    };
    let Ok(end) = chrono::DateTime::parse_from_rfc3339(ended) else {
        return 0;
    };
    (end.with_timezone(&Utc) - start.with_timezone(&Utc))
        .num_minutes()
        .max(0)
        .min(MAX_SESSION_MINUTES)
}

const GAME_COLS: &str = "id, name, store, launch_target, install_path, cover_url, cover_path,
    cover_source, favorite, hidden, missing, playtime_minutes, last_played_at, date_added,
    steam_app_id, genre, notes, developer, publisher, release_year, description, genres_json,
    hltb_main, hltb_extra, hltb_completionist, logo_path, launch_args, working_dir, run_as_admin,
    config_path, mod_manager_path, save_folder, pinned, preferred_copy_id";

/// Library grid payload: skip Wikipedia dumps, notes, and launch-only fields.
const LIST_GAME_COLS: &str = "id, name, store, launch_target, install_path, cover_url, cover_path,
    cover_source, favorite, hidden, missing, playtime_minutes, last_played_at, date_added,
    steam_app_id, genre, NULL as notes, developer, publisher, release_year, NULL as description,
    genres_json, NULL as hltb_main, NULL as hltb_extra, NULL as hltb_completionist,
    NULL as logo_path, NULL as launch_args, NULL as working_dir, run_as_admin, NULL as config_path,
    NULL as mod_manager_path, save_folder, pinned, preferred_copy_id";

pub struct Database {
    conn: Connection,
}

fn parse_genres(genres_json: Option<String>, genre: Option<String>) -> Vec<String> {
    if let Some(raw) = genres_json {
        if let Ok(list) = serde_json::from_str::<Vec<String>>(&raw) {
            return list
                .into_iter()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
    }
    genre
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn normalize_dup_name(name: &str) -> String {
    crate::playnite::normalize_name(name)
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
        db.compact_long_text()?;
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

    fn compact_long_text(&self) -> Result<()> {
        let mut stmt = self.conn.prepare(
            "SELECT id, description, notes FROM games
             WHERE length(IFNULL(description, '')) > 800
                OR length(IFNULL(notes, '')) > 4000",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })?;
        let mut updates = Vec::new();
        for row in rows {
            let (id, description, notes) = row?;
            updates.push((id, clip_text(description, 800), clip_text(notes, 4000)));
        }
        drop(stmt);
        for (id, description, notes) in updates {
            self.conn.execute(
                "UPDATE games SET description = ?1, notes = ?2 WHERE id = ?3",
                params![description, notes, id],
            )?;
        }
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
                cover_source TEXT,
                favorite INTEGER NOT NULL DEFAULT 0,
                hidden INTEGER NOT NULL DEFAULT 0,
                missing INTEGER NOT NULL DEFAULT 0,
                playtime_minutes INTEGER NOT NULL DEFAULT 0,
                last_played_at TEXT,
                date_added TEXT NOT NULL,
                steam_app_id TEXT,
                path_override INTEGER NOT NULL DEFAULT 0,
                genre TEXT,
                notes TEXT,
                developer TEXT,
                publisher TEXT,
                release_year INTEGER,
                description TEXT,
                genres_json TEXT,
                hltb_main INTEGER,
                hltb_extra INTEGER,
                hltb_completionist INTEGER,
                logo_path TEXT,
                launch_args TEXT,
                working_dir TEXT,
                run_as_admin INTEGER NOT NULL DEFAULT 0,
                config_path TEXT,
                mod_manager_path TEXT,
                save_folder TEXT,
                import_playtime INTEGER NOT NULL DEFAULT 1
            );

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                game_id TEXT NOT NULL,
                started_at TEXT NOT NULL,
                ended_at TEXT
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

            CREATE TABLE IF NOT EXISTS game_tags (
                game_id TEXT NOT NULL,
                tag TEXT NOT NULL,
                PRIMARY KEY (game_id, tag)
            );
            CREATE TABLE IF NOT EXISTS game_links (
                source_id TEXT PRIMARY KEY,
                game_id TEXT NOT NULL
            );
            "#,
        )?;
        // Migrations for existing DBs
        let _ = self.conn.execute("DROP TABLE IF EXISTS fps_samples", []);
        let _ = self.conn.execute(
            "ALTER TABLE games ADD COLUMN path_override INTEGER NOT NULL DEFAULT 0",
            [],
        );
        let _ = self
            .conn
            .execute("ALTER TABLE games ADD COLUMN genre TEXT", []);
        for col in [
            "ALTER TABLE games ADD COLUMN notes TEXT",
            "ALTER TABLE games ADD COLUMN developer TEXT",
            "ALTER TABLE games ADD COLUMN publisher TEXT",
            "ALTER TABLE games ADD COLUMN release_year INTEGER",
            "ALTER TABLE games ADD COLUMN description TEXT",
            "ALTER TABLE games ADD COLUMN genres_json TEXT",
            "ALTER TABLE games ADD COLUMN hltb_main INTEGER",
            "ALTER TABLE games ADD COLUMN hltb_extra INTEGER",
            "ALTER TABLE games ADD COLUMN hltb_completionist INTEGER",
            "ALTER TABLE games ADD COLUMN logo_path TEXT",
            "ALTER TABLE games ADD COLUMN launch_args TEXT",
            "ALTER TABLE games ADD COLUMN working_dir TEXT",
            "ALTER TABLE games ADD COLUMN run_as_admin INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE games ADD COLUMN config_path TEXT",
            "ALTER TABLE games ADD COLUMN mod_manager_path TEXT",
            "ALTER TABLE games ADD COLUMN save_folder TEXT",
            "ALTER TABLE games ADD COLUMN import_playtime INTEGER NOT NULL DEFAULT 1",
            "ALTER TABLE games ADD COLUMN cover_source TEXT",
            "ALTER TABLE games ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE games ADD COLUMN preferred_copy_id TEXT",
            "ALTER TABLE sessions ADD COLUMN last_seen_at TEXT",
            "ALTER TABLE sessions ADD COLUMN source_id TEXT",
        ] {
            let _ = self.conn.execute(col, []);
        }
        // Preserve both old collections as one pinned set. Keep the legacy field in sync
        // so older imports/backups and API clients cannot recreate a separate favorites list.
        self.conn.execute_batch(
            r#"
            UPDATE games SET pinned=MAX(pinned,favorite), favorite=MAX(pinned,favorite)
              WHERE pinned <> favorite;
            CREATE TRIGGER IF NOT EXISTS game_pin_insert AFTER INSERT ON games
              WHEN NEW.pinned <> NEW.favorite
              BEGIN UPDATE games SET pinned=MAX(NEW.pinned,NEW.favorite), favorite=MAX(NEW.pinned,NEW.favorite) WHERE id=NEW.id; END;
            CREATE TRIGGER IF NOT EXISTS game_pin_update AFTER UPDATE OF pinned,favorite ON games
              WHEN NEW.pinned <> NEW.favorite
              BEGIN UPDATE games SET pinned=MAX(NEW.pinned,NEW.favorite), favorite=MAX(NEW.pinned,NEW.favorite) WHERE id=NEW.id; END;
            "#,
        )?;
        let _ = self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS game_tags (
                game_id TEXT NOT NULL,
                tag TEXT NOT NULL,
                PRIMARY KEY (game_id, tag)
            );
            "#,
        );
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
        let genre: Option<String> = row.get("genre")?;
        let genres_json: Option<String> = row.get("genres_json")?;
        Ok(Game {
            id: row.get("id")?,
            name: row.get("name")?,
            store: Store::from_str(&row.get::<_, String>("store")?).unwrap_or(Store::Manual),
            launch_target: row.get("launch_target")?,
            install_path: row.get("install_path")?,
            cover_url: row.get("cover_url")?,
            cover_path: row.get("cover_path")?,
            cover_source: row.get("cover_source").unwrap_or(None),
            favorite: row.get::<_, i64>("favorite")? != 0,
            pinned: row.get::<_, i64>("pinned").unwrap_or(0) != 0,
            copies: Vec::new(),
            preferred_copy_id: row.get("preferred_copy_id").unwrap_or(None),
            hidden: row.get::<_, i64>("hidden")? != 0,
            missing: row.get::<_, i64>("missing")? != 0,
            playtime_minutes: row.get("playtime_minutes")?,
            last_played_at: row.get("last_played_at")?,
            date_added: row.get("date_added")?,
            steam_app_id: row.get("steam_app_id")?,
            genre: genre.clone(),
            tags: Vec::new(),
            notes: row.get("notes")?,
            developer: row.get("developer")?,
            publisher: row.get("publisher")?,
            release_year: row.get("release_year")?,
            description: row.get("description")?,
            genres: parse_genres(genres_json, genre),
            hltb_main: row.get("hltb_main")?,
            hltb_extra: row.get("hltb_extra")?,
            hltb_completionist: row.get("hltb_completionist")?,
            logo_path: row.get("logo_path")?,
            launch_args: row.get("launch_args")?,
            working_dir: row.get("working_dir")?,
            run_as_admin: row.get::<_, i64>("run_as_admin").unwrap_or(0) != 0,
            config_path: row.get("config_path")?,
            mod_manager_path: row.get("mod_manager_path")?,
            save_folder: row.get("save_folder")?,
        })
    }

    pub fn list_games(&self, include_hidden: bool) -> Result<Vec<Game>> {
        let sql = if include_hidden {
            format!("SELECT {LIST_GAME_COLS} FROM games WHERE id NOT IN (SELECT source_id FROM game_links) ORDER BY name COLLATE NOCASE")
        } else {
            format!("SELECT {LIST_GAME_COLS} FROM games WHERE hidden = 0 AND id NOT IN (SELECT source_id FROM game_links) ORDER BY name COLLATE NOCASE")
        };
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], Self::map_game)?;
        let mut games = Vec::new();
        for g in rows {
            games.push(g?);
        }
        self.attach_tags(&mut games)?;
        for g in &mut games {
            self.attach_copies(g)?;
            g.description = None;
            g.notes = None;
            g.hltb_main = None;
            g.hltb_extra = None;
            g.hltb_completionist = None;
            g.config_path = None;
            g.mod_manager_path = None;
        }
        Ok(games)
    }

    pub fn get_game(&self, id: &str) -> Result<Option<Game>> {
        let sql = format!("SELECT {GAME_COLS} FROM games WHERE id = ?1");
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query_map(params![id], Self::map_game)?;
        let mut game = rows.next().transpose()?;
        if let Some(g) = game.as_mut() {
            g.tags = self.tags_for(&g.id)?;
            self.attach_copies(g)?;
            g.description = clip_text(g.description.take(), 800);
        }
        Ok(game)
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
                "SELECT favorite, hidden, playtime_minutes, last_played_at, date_added, cover_url, cover_path, COALESCE(path_override, 0), COALESCE(import_playtime, 1) FROM games WHERE id = ?1",
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
                        row.get::<_, i64>(8)?,
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
                    import_playtime,
                )) => {
                    let mut playtime = playtime;
                    if import_playtime != 0 {
                        if let Some(imported) = g.playtime_minutes {
                            if imported > playtime {
                                playtime = imported;
                            }
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
            "UPDATE games SET pinned = CASE pinned WHEN 1 THEN 0 ELSE 1 END, favorite = CASE pinned WHEN 1 THEN 0 ELSE 1 END WHERE id = ?1",
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
        cover_source: Option<&str>,
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
        if let Some(source) = cover_source {
            self.conn.execute(
                "UPDATE games SET cover_source = ?1 WHERE id = ?2",
                params![source, id],
            )?;
        }
        Ok(())
    }

    pub fn clear_cover_path(&self, id: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE games SET cover_path = NULL WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn clear_all_stats(&self) -> Result<()> {
        self.conn.execute("DELETE FROM sessions", [])?;
        self.conn.execute(
            "UPDATE games SET playtime_minutes = 0, last_played_at = NULL, import_playtime = 0",
            [],
        )?;
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
        self.conn.execute(
            "DELETE FROM game_group_members WHERE game_id = ?1",
            params![id],
        )?;
        self.conn
            .execute("DELETE FROM games WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn record_launch(&self, id: &str, source_id: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("UPDATE games SET last_played_at=?1 WHERE id=?2 OR id=?3", params![now, id, source_id])?;
        tx.execute("INSERT INTO sessions (game_id, source_id, started_at, last_seen_at) VALUES (?1,?2,?3,?3)", params![id, source_id, now])?;
        tx.commit()?;
        Ok(())
    }

    pub fn heartbeat_session(&self, id: &str) -> Result<()> {
        self.conn.execute("UPDATE sessions SET last_seen_at=?1 WHERE game_id=?2 AND ended_at IS NULL", params![Utc::now().to_rfc3339(), id])?;
        Ok(())
    }

    /// Persist only time observed by the backend, never time inferred from window focus.
    pub fn finish_session(&self, id: &str) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        let mut stmt = tx.prepare("SELECT id, started_at, COALESCE(last_seen_at, started_at), COALESCE(source_id, game_id) FROM sessions WHERE game_id=?1 AND ended_at IS NULL")?;
        let rows: Vec<(i64,String,String,String)> = stmt.query_map(params![id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?.collect::<rusqlite::Result<_>>()?;
        drop(stmt);
        for (session, started, ended, source) in rows {
            let minutes = session_duration_minutes(&started, Some(&ended));
            tx.execute("UPDATE sessions SET ended_at=?1 WHERE id=?2 AND ended_at IS NULL", params![ended, session])?;
            tx.execute("UPDATE games SET playtime_minutes=playtime_minutes+?1 WHERE id=?2", params![minutes, source])?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn recover_sessions(&self) -> Result<()> {
        let mut stmt = self.conn.prepare("SELECT DISTINCT game_id FROM sessions WHERE ended_at IS NULL")?;
        let ids: Vec<String> = stmt.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
        drop(stmt);
        for id in ids { self.finish_session(&id)?; }
        Ok(())
    }

    pub fn set_pinned(&self, id: &str, pinned: bool) -> Result<Game> {
        self.conn.execute("UPDATE games SET pinned=?1, favorite=?1 WHERE id=?2", params![pinned, id])?;
        self.get_game(id)?.ok_or_else(|| anyhow::anyhow!("game not found"))
    }

    pub fn set_preferred_copy(&self, id: &str, source_id: &str) -> Result<Game> {
        let game = self.get_game(id)?.ok_or_else(|| anyhow::anyhow!("game not found"))?;
        anyhow::ensure!(game.copies.iter().any(|c| c.id == source_id), "That copy does not belong to this game");
        self.conn.execute("UPDATE games SET preferred_copy_id=?1 WHERE id=?2", params![source_id, id])?;
        self.get_game(id)?.ok_or_else(|| anyhow::anyhow!("game not found"))
    }

    fn attach_copies(&self, game: &mut Game) -> Result<()> {
        let mut stmt = self.conn.prepare("SELECT id, store, missing, install_path, playtime_minutes, last_played_at FROM games WHERE id=?1 OR id IN (SELECT source_id FROM game_links WHERE game_id=?1) ORDER BY store,id")?;
        let rows = stmt.query_map(params![game.id], |r| Ok((GameCopy {
            id:r.get(0)?, store:Store::from_str(&r.get::<_,String>(1)?).unwrap_or(Store::Manual),
            missing:r.get::<_,i64>(2)? != 0, install_path:r.get(3)?, playtime_minutes:r.get(4)?,
        }, r.get::<_,Option<String>>(5)?)))?;
        let mut copies = Vec::new();
        let mut baseline = 0;
        let mut tracked_total = 0;
        for row in rows {
            let (copy, last) = row?;
            if last > game.last_played_at { game.last_played_at = last; }
            let mut sessions = self.conn.prepare("SELECT started_at, ended_at FROM sessions WHERE COALESCE(source_id,game_id)=?1 AND ended_at IS NOT NULL")?;
            let times = sessions.query_map(params![copy.id], |r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?;
            let mut tracked = 0;
            for time in times { let (start,end) = time?; tracked += session_duration_minutes(&start, Some(&end)); }
            // Imported store histories may overlap. Keep the largest baseline; local sessions are distinct.
            baseline = baseline.max((copy.playtime_minutes - tracked).max(0));
            tracked_total += tracked;
            copies.push(copy);
        }
        game.missing = copies.iter().all(|c| c.missing);
        game.playtime_minutes = baseline + tracked_total;
        game.copies = copies;
        Ok(())
    }

    pub fn snapshot(&self, dest: &Path) -> Result<()> {
        self.conn.execute("VACUUM INTO ?1", params![dest.to_string_lossy().to_string()])?;
        let snapshot = Connection::open(dest)?;
        snapshot.execute_batch("PRAGMA secure_delete=ON")?;
        snapshot.execute("DELETE FROM settings WHERE key IN ('steam_grid_db_api_key','twitch_client_id','twitch_client_secret')", [])?;
        snapshot.execute_batch("VACUUM")?;
        Ok(())
    }

    pub fn relocate_art(&self, root: &Path) -> Result<()> {
        for column in ["cover_path", "logo_path"] {
            let mut stmt = self.conn.prepare(&format!("SELECT id,{column} FROM games WHERE {column} IS NOT NULL"))?;
            let paths = stmt.query_map([], |r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
            drop(stmt);
            for (id,path) in paths {
                let name = path.rsplit(['/', '\\']).next().unwrap_or("");
                anyhow::ensure!(!name.is_empty() && !name.contains(':') && name!="..", "Invalid artwork path in backup");
                self.conn.execute(&format!("UPDATE games SET {column}=?1 WHERE id=?2"),params![root.join("covers").join(name).to_string_lossy().to_string(),id])?;
            }
        }
        Ok(())
    }

    pub fn clear_art(&self, custom: bool) -> Result<Vec<String>> {
        let condition = if custom { "cover_source IN ('Custom','Local')" } else { "COALESCE(cover_source,'') NOT IN ('Custom','Local','Install folder')" };
        let mut stmt = self.conn.prepare(&format!("SELECT cover_path FROM games WHERE {condition} AND cover_path IS NOT NULL"))?;
        let paths = stmt.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<Vec<String>>>()?;
        drop(stmt);
        self.conn.execute(&format!("UPDATE games SET cover_path=NULL, cover_url=NULL, cover_source=NULL WHERE {condition}"), [])?;
        Ok(paths)
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
            let duration = session_duration_minutes(&started, ended.as_deref());
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
                    *slot += s.duration_minutes;
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
                "translucency" => settings.translucency = v.parse::<f64>().ok(),
                "card_scale" => {
                    if let Ok(n) = v.parse::<f64>() {
                        settings.card_scale = Some(n);
                    }
                }
                "library_order" => settings.library_order = Some(v),
                "show_titles" => settings.show_titles = Some(v == "1" || v == "true"),
                "show_store_labels" => settings.show_store_labels = Some(v == "1" || v == "true"),
                "grid_density" => settings.grid_density = Some(v),
                "cover_corners" => settings.cover_corners = Some(v),
                "cover_shape" => settings.cover_shape = Some(v),
                "reduce_motion" => settings.reduce_motion = Some(v == "1" || v == "true"),
                "start_with_windows" => settings.start_with_windows = Some(v == "1" || v == "true"),
                "close_to_tray" => settings.close_to_tray = Some(v == "1" || v == "true"),
                "start_in_background" => {
                    settings.start_in_background = Some(v == "1" || v == "true")
                }
                "hide_on_game_launch" => {
                    settings.hide_on_game_launch = Some(v == "1" || v == "true")
                }
                "twitch_client_id" => settings.twitch_client_id = Some(v),
                "twitch_client_secret" => settings.twitch_client_secret = Some(v),
                "show_tools" => settings.show_tools = Some(v == "1"),
                "automatic_backups" => settings.automatic_backups = Some(v == "1"),
                "library_layout" => settings.library_layout = Some(v),
                "saved_views" => settings.saved_views = Some(v),
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
        if let Some(value) = settings.translucency {
            self.conn.execute(
                "INSERT INTO settings (key, value) VALUES ('translucency', ?1) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![value.clamp(0.0, 70.0).to_string()],
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
        self.upsert_bool_setting("show_titles", settings.show_titles)?;
        self.upsert_bool_setting("show_store_labels", settings.show_store_labels)?;
        self.upsert_bool_setting("reduce_motion", settings.reduce_motion)?;
        if let Some(v) = &settings.grid_density {
            self.conn.execute(
                "INSERT INTO settings (key, value) VALUES ('grid_density', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![v],
            )?;
        }
        if let Some(v) = &settings.cover_corners {
            self.conn.execute(
                "INSERT INTO settings (key, value) VALUES ('cover_corners', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![v],
            )?;
        }
        if let Some(v) = &settings.cover_shape {
            self.conn.execute(
                "INSERT INTO settings (key, value) VALUES ('cover_shape', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![v],
            )?;
        }
        self.upsert_bool_setting("start_with_windows", settings.start_with_windows)?;
        self.upsert_bool_setting("close_to_tray", settings.close_to_tray)?;
        self.upsert_bool_setting("start_in_background", settings.start_in_background)?;
        self.upsert_bool_setting("hide_on_game_launch", settings.hide_on_game_launch)?;
        self.upsert_bool_setting("show_tools", settings.show_tools)?;
        self.upsert_bool_setting("automatic_backups", settings.automatic_backups)?;
        self.upsert_optional_setting("library_layout", settings.library_layout.as_deref())?;
        self.upsert_optional_setting("saved_views", settings.saved_views.as_deref())?;
        self.upsert_optional_setting("twitch_client_id", settings.twitch_client_id.as_deref())?;
        self.upsert_optional_setting(
            "twitch_client_secret",
            settings.twitch_client_secret.as_deref(),
        )?;
        Ok(())
    }

    fn upsert_bool_setting(&self, key: &str, value: Option<bool>) -> Result<()> {
        if let Some(v) = value {
            self.conn.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, if v { "1" } else { "0" }],
            )?;
        }
        Ok(())
    }

    fn upsert_optional_setting(&self, key: &str, value: Option<&str>) -> Result<()> {
        if let Some(v) = value.map(str::trim).filter(|s| !s.is_empty()) {
            self.conn.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, v],
            )?;
        } else {
            self.conn.execute("DELETE FROM settings WHERE key = ?1", params![key])?;
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

    fn tags_for(&self, game_id: &str) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT tag FROM game_tags WHERE game_id = ?1 ORDER BY tag COLLATE NOCASE")?;
        let rows = stmt.query_map(params![game_id], |row| row.get::<_, String>(0))?;
        let mut tags = Vec::new();
        for r in rows {
            tags.push(r?);
        }
        Ok(tags)
    }

    fn attach_tags(&self, games: &mut [Game]) -> Result<()> {
        let mut stmt = self.conn.prepare("SELECT game_id, tag FROM game_tags")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut map: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        for row in rows {
            let (id, tag) = row?;
            map.entry(id).or_default().push(tag);
        }
        for g in games.iter_mut() {
            if let Some(tags) = map.remove(&g.id) {
                g.tags = tags;
            }
        }
        Ok(())
    }

    pub fn set_notes(&self, id: &str, notes: &str) -> Result<Game> {
        self.conn.execute(
            "UPDATE games SET notes = ?1 WHERE id = ?2",
            params![notes, id],
        )?;
        self.get_game(id)?
            .ok_or_else(|| anyhow::anyhow!("game not found"))
    }

    pub fn set_tags(&self, id: &str, tags: &[String]) -> Result<Game> {
        if self.get_game(id)?.is_none() {
            anyhow::bail!("game not found");
        }
        self.conn
            .execute("DELETE FROM game_tags WHERE game_id = ?1", params![id])?;
        let mut seen = std::collections::HashSet::new();
        for tag in tags {
            let t = tag.trim();
            if t.is_empty() {
                continue;
            }
            let key = t.to_lowercase();
            if !seen.insert(key) {
                continue;
            }
            self.conn.execute(
                "INSERT OR IGNORE INTO game_tags (game_id, tag) VALUES (?1, ?2)",
                params![id, t],
            )?;
        }
        self.get_game(id)?
            .ok_or_else(|| anyhow::anyhow!("game not found"))
    }

    pub fn set_launch_options(
        &self,
        id: &str,
        launch_args: Option<&str>,
        working_dir: Option<&str>,
        run_as_admin: bool,
        save_folder: Option<&str>,
    ) -> Result<Game> {
        self.conn.execute(
            r#"UPDATE games SET
                launch_args = ?1,
                working_dir = ?2,
                run_as_admin = ?3,
                save_folder = ?4
            WHERE id = ?5"#,
            params![
                launch_args,
                working_dir,
                run_as_admin as i64,
                save_folder,
                id
            ],
        )?;
        self.get_game(id)?
            .ok_or_else(|| anyhow::anyhow!("game not found"))
    }

    pub fn apply_metadata(
        &self,
        id: &str,
        meta: &crate::metadata::GameMetadata,
    ) -> Result<Game> {
        let existing = self
            .get_game(id)?
            .ok_or_else(|| anyhow::anyhow!("game not found"))?;
        let developer = meta.developer.clone().or(existing.developer);
        let publisher = meta.publisher.clone().or(existing.publisher);
        let release_year = meta.release_year.or(existing.release_year);
        let description = clip_text(meta.description.clone().or(existing.description), 800);
        let genres =         if meta.genres.is_empty() {
            existing.genres.clone()
        } else {
            meta.genres.clone()
        };
        let genres_json = serde_json::to_string(&genres).unwrap_or_default();
        let genre = if genres.is_empty() {
            existing.genre.clone()
        } else {
            Some(genres.join(", "))
        };
        let steam_id = meta.steam_app_id.clone().or(existing.steam_app_id);

        self.conn.execute(
            r#"UPDATE games SET
                developer = ?1,
                publisher = ?2,
                release_year = ?3,
                description = ?4,
                genres_json = ?5,
                genre = ?6,
                steam_app_id = COALESCE(steam_app_id, ?7)
            WHERE id = ?8"#,
            params![
                developer,
                publisher,
                release_year,
                description,
                genres_json,
                genre,
                steam_id,
                id
            ],
        )?;
        self.get_game(id)?
            .ok_or_else(|| anyhow::anyhow!("game not found"))
    }

    #[allow(dead_code)]
    pub fn set_logo_path(&self, id: &str, path: &str) -> Result<Game> {
        self.conn.execute(
            "UPDATE games SET logo_path = ?1 WHERE id = ?2",
            params![path, id],
        )?;
        self.get_game(id)?
            .ok_or_else(|| anyhow::anyhow!("game not found"))
    }

    pub fn apply_playnite_fields(
        &self,
        id: &str,
        playtime_minutes: i64,
        favorite: bool,
        hidden: bool,
        notes: Option<&str>,
        tags: &[String],
        developer: Option<&str>,
        publisher: Option<&str>,
        release_year: Option<i64>,
        description: Option<&str>,
        genres: &[String],
        steam_app_id: Option<&str>,
    ) -> Result<Game> {
        let existing = self
            .get_game(id)?
            .ok_or_else(|| anyhow::anyhow!("game not found"))?;
        let playtime = existing.playtime_minutes.max(playtime_minutes);
        let fav = existing.favorite || favorite;
        let hid = existing.hidden || hidden;
        let notes = notes
            .map(|s| s.to_string())
            .or(existing.notes.clone());
        let developer = developer
            .map(|s| s.to_string())
            .or(existing.developer.clone());
        let publisher = publisher
            .map(|s| s.to_string())
            .or(existing.publisher.clone());
        let release_year = release_year.or(existing.release_year);
        let description = description
            .map(|s| s.to_string())
            .or(existing.description.clone());
        let genres = if genres.is_empty() {
            existing.genres.clone()
        } else {
            genres.to_vec()
        };
        let genres_json = serde_json::to_string(&genres).unwrap_or_default();
        let genre = if genres.is_empty() {
            existing.genre.clone()
        } else {
            Some(genres.join(", "))
        };
        self.conn.execute(
            r#"UPDATE games SET
                playtime_minutes = ?1,
                favorite = ?2,
                hidden = ?3,
                notes = COALESCE(?4, notes),
                developer = COALESCE(?5, developer),
                publisher = COALESCE(?6, publisher),
                release_year = COALESCE(?7, release_year),
                description = COALESCE(?8, description),
                genres_json = ?9,
                genre = ?10,
                steam_app_id = COALESCE(steam_app_id, ?11)
            WHERE id = ?12"#,
            params![
                playtime,
                fav as i64,
                hid as i64,
                notes,
                developer,
                publisher,
                release_year,
                description,
                genres_json,
                genre,
                steam_app_id,
                id
            ],
        )?;
        let mut merged_tags = existing.tags;
        for t in tags {
            if !merged_tags.iter().any(|x| x.eq_ignore_ascii_case(t)) {
                merged_tags.push(t.clone());
            }
        }
        self.set_tags(id, &merged_tags)
    }

    pub fn insert_game_full(&self, game: &Game) -> Result<Game> {
        let genres_json = serde_json::to_string(&game.genres).unwrap_or_default();
        self.conn.execute(
            r#"INSERT OR REPLACE INTO games (
                id, name, store, launch_target, install_path, cover_url, cover_path, cover_source,
                favorite, hidden, missing, playtime_minutes, last_played_at, date_added,
                steam_app_id, genre, notes, developer, publisher, release_year, description,
                genres_json, hltb_main, hltb_extra, hltb_completionist, logo_path,
                launch_args, working_dir, run_as_admin, config_path, mod_manager_path, save_folder,
                pinned, preferred_copy_id, path_override, import_playtime
            ) VALUES (
                ?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,
                ?21,?22,?23,?24,?25,?26,?27,?28,?29,?30,?31,?32,?33,?34,
                COALESCE((SELECT path_override FROM games WHERE id=?1),0),
                COALESCE((SELECT import_playtime FROM games WHERE id=?1),1)
            )"#,
            params![
                game.id,
                game.name,
                game.store.as_str(),
                game.launch_target,
                game.install_path,
                game.cover_url,
                game.cover_path,
                game.cover_source,
                game.favorite as i64,
                game.hidden as i64,
                game.missing as i64,
                game.copies.iter().find(|c| c.id == game.id).map(|c| c.playtime_minutes).unwrap_or(game.playtime_minutes),
                game.last_played_at,
                game.date_added,
                game.steam_app_id,
                game.genre,
                game.notes,
                game.developer,
                game.publisher,
                game.release_year,
                game.description,
                genres_json,
                game.hltb_main,
                game.hltb_extra,
                game.hltb_completionist,
                game.logo_path,
                game.launch_args,
                game.working_dir,
                game.run_as_admin as i64,
                game.config_path,
                game.mod_manager_path,
                game.save_folder,
                game.pinned,
                game.preferred_copy_id,
            ],
        )?;
        self.set_tags(&game.id, &game.tags)
    }

    pub fn finalize_remove(&self, id: &str) -> Result<()> {
        if self.get_game(id)?.is_some() {
            return Ok(());
        }
        self.conn.execute("UPDATE sessions SET game_id=COALESCE(source_id,game_id) WHERE game_id=?1 AND source_id != ?1", params![id])?;
        self.conn.execute("DELETE FROM game_links WHERE game_id=?1 OR source_id=?1", params![id])?;
        self.conn
            .execute("DELETE FROM sessions WHERE game_id = ?1", params![id])?;
        self.conn
            .execute("DELETE FROM game_tags WHERE game_id = ?1", params![id])?;
        Ok(())
    }

    /// Link installations without deleting store identities or launch targets.
    pub fn merge_games(&self, keep_id: &str, source_ids: &[String]) -> Result<Game> {
        anyhow::ensure!(self.get_game(keep_id)?.is_some(), "Game not found");
        let tx = self.conn.unchecked_transaction()?;
        let linked: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM game_links WHERE source_id=?1)", params![keep_id], |r| r.get(0))?;
        anyhow::ensure!(!linked, "Choose the main game card");
        for sid in source_ids {
            if sid == keep_id { continue; }
            let exists: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM games WHERE id=?1)", params![sid], |r| r.get(0))?;
            anyhow::ensure!(exists, "A selected copy no longer exists");
            let linked: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM game_links WHERE source_id=?1)", params![sid], |r| r.get(0))?;
            anyhow::ensure!(!linked, "That copy is already linked");
            tx.execute("UPDATE games SET favorite=MAX(favorite,(SELECT favorite FROM games WHERE id=?2)), pinned=MAX(pinned,(SELECT pinned FROM games WHERE id=?2)), notes=CASE WHEN (SELECT notes FROM games WHERE id=?2) IS NULL THEN notes WHEN notes IS NULL THEN (SELECT notes FROM games WHERE id=?2) ELSE notes || char(10) || char(10) || (SELECT notes FROM games WHERE id=?2) END WHERE id=?1", params![keep_id,sid])?;
            tx.execute("INSERT OR IGNORE INTO game_tags(game_id,tag) SELECT ?1,tag FROM game_tags WHERE game_id=?2", params![keep_id,sid])?;
            tx.execute("INSERT OR IGNORE INTO game_group_members(group_id,game_id,sort_order) SELECT group_id,?1,sort_order FROM game_group_members WHERE game_id=?2", params![keep_id,sid])?;
            tx.execute("DELETE FROM game_group_members WHERE game_id=?1", params![sid])?;
            tx.execute("UPDATE sessions SET source_id=COALESCE(source_id,game_id),game_id=?1 WHERE game_id=?2", params![keep_id,sid])?;
            tx.execute("UPDATE game_links SET game_id=?1 WHERE game_id=?2", params![keep_id,sid])?;
            tx.execute("INSERT INTO game_links(source_id,game_id) VALUES (?1,?2)", params![sid,keep_id])?;
        }
        tx.commit()?;
        self.get_game(keep_id)?.ok_or_else(|| anyhow::anyhow!("game not found"))
    }

    pub fn suggest_duplicates(&self) -> Result<Vec<DuplicateGroup>> {
        let games = self.list_games(true)?;
        let mut buckets: std::collections::BTreeMap<String, Vec<Game>> =
            std::collections::BTreeMap::new();
        for g in games {
            let key = normalize_dup_name(&g.name);
            if key.len() < 4 {
                continue;
            }
            buckets.entry(key).or_default().push(g);
        }
        Ok(buckets
            .into_iter()
            .filter(|(_, list)| list.len() >= 2)
            .map(|(key, games)| DuplicateGroup { key, games })
            .collect())
    }

    pub fn library_overview(&self) -> Result<LibraryOverview> {
        let games = self.list_games(true)?;
        let mut sess_stmt = self
            .conn
            .prepare("SELECT game_id, started_at, ended_at FROM sessions ORDER BY id ASC")?;
        let sess_rows = sess_stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })?;

        let now = Utc::now();
        let today = now.date_naive();
        let week_ago = now - Duration::days(7);
        let year = now.year();
        let mut minutes_this_week = 0i64;
        let mut days_with_play: std::collections::BTreeSet<chrono::NaiveDate> =
            std::collections::BTreeSet::new();
        let mut year_minutes = 0i64;
        let mut monthly = [0i64; 12];
        let mut per_game_year: std::collections::HashMap<String, i64> =
            std::collections::HashMap::new();

        for row in sess_rows {
            let (game_id, started, ended) = row?;
            let Ok(start) = chrono::DateTime::parse_from_rfc3339(&started) else {
                continue;
            };
            let start = start.with_timezone(&Utc);
            let duration = session_duration_minutes(&started, ended.as_deref());
            if duration <= 0 {
                continue;
            }
            if start >= week_ago {
                minutes_this_week += duration;
            }
            days_with_play.insert(start.date_naive());
            if start.year() == year {
                year_minutes += duration;
                monthly[(start.month0() as usize).min(11)] += duration;
                *per_game_year.entry(game_id).or_default() += duration;
            }
        }

        let mut streak = 0i64;
        let mut cursor = today;
        // allow streak to start yesterday if no play today yet
        if !days_with_play.contains(&today) {
            cursor = today - Duration::days(1);
        }
        while days_with_play.contains(&cursor) {
            streak += 1;
            cursor -= Duration::days(1);
        }

        let most_played = games
            .iter()
            .filter(|g| g.playtime_minutes > 0)
            .max_by_key(|g| g.playtime_minutes)
            .map(|g| TopPlayedGame {
                game_id: g.id.clone(),
                name: g.name.clone(),
                minutes: g.playtime_minutes,
            });

        let mut top: Vec<TopPlayedGame> = per_game_year
            .into_iter()
            .filter_map(|(id, minutes)| {
                games.iter().find(|g| g.id == id).map(|g| TopPlayedGame {
                    game_id: id,
                    name: g.name.clone(),
                    minutes,
                })
            })
            .collect();
        top.sort_by(|a, b| b.minutes.cmp(&a.minutes));
        top.truncate(8);

        let monthly_pts: Vec<DailyPlaytime> = (1..=12)
            .map(|m| DailyPlaytime {
                day: format!("{year}-{m:02}"),
                minutes: monthly[(m - 1) as usize],
            })
            .collect();

        let total_playtime_minutes = games.iter().map(|g| g.playtime_minutes).sum();
        let games_played = games.iter().filter(|g| g.playtime_minutes > 0).count();
        let minutes_this_week = minutes_this_week.min(total_playtime_minutes);

        Ok(LibraryOverview {
            hours_this_week: minutes_this_week as f64 / 60.0,
            minutes_this_week,
            most_played,
            streak_days: streak,
            year_in_review: YearInReview {
                year,
                total_minutes: year_minutes,
                monthly: monthly_pts,
                top_games: top,
            },
            total_playtime_minutes,
            games_played,
        })
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

/// Wipe app data if the user requested a full reset on the previous run.
pub fn apply_pending_reset() {
    let dir = app_data_dir();
    if dir.join(".reset").exists() {
        let _ = std::fs::remove_dir_all(&dir);
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn library() -> Database {
        let db = Database { conn: Connection::open_in_memory().unwrap() };
        db.migrate().unwrap();
        db
    }

    #[test]
    fn old_favorites_and_pins_merge_without_repinning_after_restart() {
        let db = library();
        for id in ["favorite", "pin", "neither"] { db.add_manual_game(id, id, "game.exe", None).unwrap(); }
        db.conn.execute_batch("DROP TRIGGER game_pin_insert; DROP TRIGGER game_pin_update;
          UPDATE games SET favorite=1 WHERE id='favorite';
          UPDATE games SET pinned=1 WHERE id='pin';").unwrap();
        db.migrate().unwrap();
        for id in ["favorite", "pin"] {
            let game = db.get_game(id).unwrap().unwrap();
            assert!(game.pinned && game.favorite);
        }
        assert!(!db.get_game("neither").unwrap().unwrap().pinned);
        db.set_pinned("favorite", false).unwrap();
        db.migrate().unwrap();
        let game = db.get_game("favorite").unwrap().unwrap();
        assert!(!game.pinned && !game.favorite);
        // Legacy import updates and the old toggle endpoint refer to the same collection.
        db.conn.execute("UPDATE games SET favorite=1 WHERE id='neither'", []).unwrap();
        assert!(db.get_game("neither").unwrap().unwrap().pinned);
        let game = db.toggle_favorite("neither").unwrap();
        assert!(!game.pinned && !game.favorite);
    }

    #[test]
    fn translucency_persists_and_legacy_settings_remain_readable() {
        let db = library();
        assert_eq!(db.get_settings().unwrap().translucency, None);
        let mut settings = db.get_settings().unwrap();
        settings.translucency = Some(45.0);
        db.save_settings(&settings).unwrap();
        assert_eq!(db.get_settings().unwrap().translucency, Some(45.0));
        settings.translucency = Some(120.0);
        db.save_settings(&settings).unwrap();
        assert_eq!(db.get_settings().unwrap().translucency, Some(70.0));
    }

    #[test]
    fn interrupted_sessions_use_last_observation_and_finish_once() {
        let db = library();
        db.add_manual_game("a", "Game", "game.exe", None).unwrap();
        db.record_launch("a", "a").unwrap();
        db.conn.execute("UPDATE sessions SET started_at='2026-01-01T10:00:00Z', last_seen_at='2026-01-01T10:07:00Z'", []).unwrap();
        db.recover_sessions().unwrap();
        db.finish_session("a").unwrap();
        assert_eq!(db.get_game("a").unwrap().unwrap().playtime_minutes, 7);
        let end: String = db.conn.query_row("SELECT ended_at FROM sessions", [], |r|r.get(0)).unwrap();
        assert_eq!(end, "2026-01-01T10:07:00Z");
    }

    #[test]
    fn linked_copies_survive_rescan_and_keep_distinct_local_sessions() {
        let db = library();
        let discovered = |id: &str, store: Store, target: &str| DiscoveredGame {
            id:id.into(), name:"Example".into(), store, launch_target:target.into(),
            install_path:None, steam_app_id:None, playtime_minutes:Some(100),
        };
        db.upsert_discovered(&[discovered("a", Store::Steam, "123"), discovered("b", Store::Epic, "epic://game")]).unwrap();
        db.set_pinned("b", true).unwrap();
        db.set_tags("b", &["Co-op".into()]).unwrap();
        db.set_notes("b", "Keep this note").unwrap();
        db.record_launch("b", "b").unwrap();
        db.conn.execute("UPDATE sessions SET started_at='2026-01-01T10:00:00Z', last_seen_at='2026-01-01T10:10:00Z'", []).unwrap();
        db.finish_session("b").unwrap();
        let linked = db.merge_games("a", &["b".into()]).unwrap();
        assert_eq!(linked.playtime_minutes, 110);
        assert_eq!(linked.copies.len(), 2);
        assert!(linked.pinned);
        assert_eq!(linked.tags, vec!["Co-op"]);
        assert_eq!(linked.notes.as_deref(), Some("Keep this note"));
        db.set_preferred_copy("a", "b").unwrap();
        assert!(db.set_preferred_copy("a", "unrelated").is_err());
        db.upsert_discovered(&[discovered("b", Store::Epic, "epic://game")]).unwrap();
        let games = db.list_games(false).unwrap();
        assert_eq!(games.len(), 1);
        assert!(!games[0].missing);
        assert_eq!(games[0].preferred_copy_id.as_deref(), Some("b"));
        assert!(games[0].copies.iter().find(|c|c.id=="a").unwrap().missing);
        assert_eq!(db.get_game("b").unwrap().unwrap().launch_target, "epic://game");
        assert_eq!(db.game_stats("a").unwrap().sessions.len(), 1);
    }

    #[test]
    fn invalid_link_batch_rolls_back_all_changes() {
        let db = library();
        for id in ["a", "b"] { db.add_manual_game(id, id, "game.exe", None).unwrap(); }
        assert!(db.merge_games("a", &["b".into(), "missing".into()]).is_err());
        assert_eq!(db.list_games(false).unwrap().len(), 2);
        assert_eq!(db.get_game("a").unwrap().unwrap().copies.len(), 1);
    }

    #[test]
    fn refreshing_downloads_preserves_custom_art() {
        let db = library();
        for id in ["custom", "download", "picked", "installed"] { db.add_manual_game(id, id, "game.exe", None).unwrap(); }
        db.conn.execute("UPDATE games SET cover_source='Custom',cover_path='custom.png' WHERE id='custom'", []).unwrap();
        db.conn.execute("UPDATE games SET cover_source='Steam',cover_path='download.png' WHERE id='download'", []).unwrap();
        db.conn.execute("UPDATE games SET cover_source='Local',cover_path='picked.png' WHERE id='picked'", []).unwrap();
        db.conn.execute("UPDATE games SET cover_source='Install folder',cover_path='installed.png' WHERE id='installed'", []).unwrap();
        assert_eq!(db.clear_art(false).unwrap(), vec!["download.png"]);
        assert_eq!(db.get_game("custom").unwrap().unwrap().cover_path.as_deref(), Some("custom.png"));
        assert_eq!(db.get_game("picked").unwrap().unwrap().cover_path.as_deref(), Some("picked.png"));
        let mut removed = db.clear_art(true).unwrap();
        removed.sort();
        assert_eq!(removed, vec!["custom.png", "picked.png"]);
        assert_eq!(db.get_game("installed").unwrap().unwrap().cover_path.as_deref(), Some("installed.png"));
    }
}

pub fn covers_dir() -> PathBuf {
    app_data_dir().join("covers")
}
