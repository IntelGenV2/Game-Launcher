from pathlib import Path
p = Path('src-tauri/src/db.rs')
s = p.read_text(encoding='utf-8')
start = s.index('    pub fn record_launch(')
end = s.index('    pub fn game_stats(', start)
s = s[:start] + r'''    pub fn record_launch(&self, id: &str, source_id: &str) -> Result<()> {
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
        self.conn.execute("UPDATE games SET pinned=?1 WHERE id=?2", params![pinned, id])?;
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
        snapshot.execute("DELETE FROM settings WHERE key IN ('steam_grid_db_api_key','twitch_client_id','twitch_client_secret')", [])?;
        Ok(())
    }

    pub fn clear_art(&self, custom: bool) -> Result<Vec<String>> {
        let condition = if custom { "cover_source='Custom'" } else { "COALESCE(cover_source,'') != 'Custom'" };
        let mut stmt = self.conn.prepare(&format!("SELECT cover_path FROM games WHERE {condition} AND cover_path IS NOT NULL"))?;
        let paths = stmt.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<Vec<String>>>()?;
        drop(stmt);
        self.conn.execute(&format!("UPDATE games SET cover_path=NULL, cover_url=NULL, cover_source=NULL WHERE {condition}"), [])?;
        Ok(paths)
    }

''' + s[end:]
start = s.index('    pub fn merge_games(')
end = s.index('    pub fn suggest_duplicates(', start)
s = s[:start] + r'''    /// Link installations without deleting store identities or launch targets.
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

''' + s[end:]
s = s.replace('game.playtime_minutes,\n                game.last_played_at,','game.copies.iter().find(|c| c.id == game.id).map(|c| c.playtime_minutes).unwrap_or(game.playtime_minutes),\n                game.last_played_at,')
# Copies reappear as independent cards if the main card is permanently removed.
s = s.replace('    pub fn finalize_remove(&self, id: &str) -> Result<()> {','    pub fn finalize_remove(&self, id: &str) -> Result<()> {')
s = s.replace('        self.conn\n            .execute("DELETE FROM sessions WHERE game_id = ?1", params![id])?;', '        self.conn.execute("UPDATE sessions SET game_id=COALESCE(source_id,game_id) WHERE game_id=?1 AND source_id != ?1", params![id])?;\n        self.conn.execute("DELETE FROM game_links WHERE game_id=?1 OR source_id=?1", params![id])?;\n        self.conn\n            .execute("DELETE FROM sessions WHERE game_id = ?1", params![id])?;')
s = s.replace('*slot += s.duration_minutes.max(1);','*slot += s.duration_minutes;')
p.write_text(s, encoding='utf-8')
