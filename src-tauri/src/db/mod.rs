mod schema;

use std::fs;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::utils::hash::sha256_hex;

const DEFAULT_HISTORY_LIMIT: i64 = 200;
const DEFAULT_MAX_CLIP_BYTES: i64 = 262_144;

fn default_denylist() -> Vec<String> {
    vec![
        "com.1password.1password".to_string(),
        "com.agilebits.onepassword7".to_string(),
        "com.bitwarden.desktop".to_string(),
        "com.lastpass.LastPass".to_string(),
    ]
}

#[derive(Debug, Error)]
pub enum DbError {
    #[error("sqlite error: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("database lock poisoned")]
    LockPoisoned,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Clip {
    pub id: i64,
    pub content: String,
    pub content_type: String,
    pub pinned: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClipPage {
    pub items: Vec<Clip>,
    pub total: i64,
    pub next_offset: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub history_limit: i64,
    pub tracking_paused: bool,
    pub max_clip_bytes: i64,
    pub restore_clipboard_after_paste: bool,
    pub denylist_bundle_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    pub history_limit: Option<i64>,
    pub tracking_paused: Option<bool>,
    pub max_clip_bytes: Option<i64>,
    pub restore_clipboard_after_paste: Option<bool>,
    pub denylist_bundle_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LatestClip {
    pub content: String,
    pub hash: String,
}

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    fn conn(&self) -> Result<MutexGuard<'_, Connection>, DbError> {
        self.conn.lock().map_err(|_| DbError::LockPoisoned)
    }

    pub fn new(path: &Path) -> Result<Self, DbError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        Self::initialize(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    #[cfg(test)]
    pub fn new_in_memory() -> Result<Self, DbError> {
        let conn = Connection::open_in_memory()?;
        Self::initialize(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn initialize(conn: &Connection) -> Result<(), DbError> {
        conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA temp_store = MEMORY;
            PRAGMA foreign_keys = ON;
            ",
        )?;

        conn.execute_batch(schema::CREATE_CLIPS_TABLE)?;
        conn.execute_batch(schema::CREATE_SETTINGS_TABLE)?;
        conn.execute_batch(schema::CREATE_INDEX_CREATED_AT)?;
        conn.execute_batch(schema::CREATE_INDEX_PINNED)?;
        conn.execute_batch(schema::CREATE_INDEX_HASH)?;

        let denylist_json = serde_json::to_string(&default_denylist())?;
        conn.execute(
            "
            INSERT OR IGNORE INTO settings (
                id,
                history_limit,
                tracking_paused,
                max_clip_bytes,
                restore_clipboard_after_paste,
                denylist_bundle_ids
            ) VALUES (1, ?1, 0, ?2, 1, ?3)
            ",
            params![DEFAULT_HISTORY_LIMIT, DEFAULT_MAX_CLIP_BYTES, denylist_json],
        )?;

        Ok(())
    }

    pub fn list_clips(
        &self,
        query: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<ClipPage, DbError> {
        let limit = limit.max(1);
        let offset = offset.max(0);
        let conn = self.conn()?;

        let search = query
            .map(str::trim)
            .filter(|q| !q.is_empty())
            .map(|q| q.to_string());

        let (items, total) = if let Some(search) = search {
            let like = format!("%{}%", search.to_lowercase());
            let mut stmt = conn.prepare(
                "
                SELECT id, content, content_type, pinned, created_at
                FROM clips
                WHERE LOWER(content) LIKE ?1
                ORDER BY pinned DESC, created_at DESC, id DESC
                LIMIT ?2 OFFSET ?3
                ",
            )?;
            let rows = stmt.query_map(params![like, limit, offset], |row| {
                Ok(Clip {
                    id: row.get(0)?,
                    content: row.get(1)?,
                    content_type: row.get(2)?,
                    pinned: row.get::<_, i64>(3)? == 1,
                    created_at: row.get(4)?,
                })
            })?;

            let items = rows.collect::<Result<Vec<_>, _>>()?;
            let total: i64 = conn.query_row(
                "SELECT COUNT(*) FROM clips WHERE LOWER(content) LIKE ?1",
                params![format!("%{}%", search.to_lowercase())],
                |row| row.get(0),
            )?;
            (items, total)
        } else {
            let mut stmt = conn.prepare(
                "
                SELECT id, content, content_type, pinned, created_at
                FROM clips
                ORDER BY pinned DESC, created_at DESC, id DESC
                LIMIT ?1 OFFSET ?2
                ",
            )?;
            let rows = stmt.query_map(params![limit, offset], |row| {
                Ok(Clip {
                    id: row.get(0)?,
                    content: row.get(1)?,
                    content_type: row.get(2)?,
                    pinned: row.get::<_, i64>(3)? == 1,
                    created_at: row.get(4)?,
                })
            })?;

            let items = rows.collect::<Result<Vec<_>, _>>()?;
            let total: i64 = conn.query_row("SELECT COUNT(*) FROM clips", [], |row| row.get(0))?;
            (items, total)
        };

        let next_offset = if offset + limit < total {
            Some(offset + limit)
        } else {
            None
        };

        Ok(ClipPage {
            items,
            total,
            next_offset,
        })
    }

    pub fn latest_clip(&self) -> Result<Option<LatestClip>, DbError> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT content, hash FROM clips ORDER BY created_at DESC, id DESC LIMIT 1",
            [],
            |row| {
                Ok(LatestClip {
                    content: row.get(0)?,
                    hash: row.get(1)?,
                })
            },
        )
        .optional()
        .map_err(DbError::from)
    }

    pub fn insert_clip(&self, content: &str, content_type: &str) -> Result<Clip, DbError> {
        let hash = sha256_hex(content);
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO clips (content, content_type, pinned, hash) VALUES (?1, ?2, 0, ?3)",
            params![content, content_type, hash],
        )?;
        let id = conn.last_insert_rowid();
        Ok(self.get_clip_internal(&conn, id)?)
    }

    pub fn get_clip(&self, id: i64) -> Result<Option<Clip>, DbError> {
        let conn = self.conn()?;
        self.get_clip_internal(&conn, id).optional().map_err(DbError::from)
    }

    fn get_clip_internal(&self, conn: &Connection, id: i64) -> Result<Clip, rusqlite::Error> {
        conn.query_row(
            "SELECT id, content, content_type, pinned, created_at FROM clips WHERE id = ?1",
            params![id],
            |row| {
                Ok(Clip {
                    id: row.get(0)?,
                    content: row.get(1)?,
                    content_type: row.get(2)?,
                    pinned: row.get::<_, i64>(3)? == 1,
                    created_at: row.get(4)?,
                })
            },
        )
    }

    pub fn set_pinned(&self, id: i64, pinned: bool) -> Result<Option<Clip>, DbError> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE clips SET pinned = ?1 WHERE id = ?2",
            params![if pinned { 1 } else { 0 }, id],
        )?;
        self.get_clip_internal(&conn, id).optional().map_err(DbError::from)
    }

    pub fn delete_clip(&self, id: i64) -> Result<bool, DbError> {
        let conn = self.conn()?;
        let affected = conn.execute("DELETE FROM clips WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    pub fn delete_all_clips(&self) -> Result<usize, DbError> {
        let conn = self.conn()?;
        let affected = conn.execute("DELETE FROM clips", [])?;
        Ok(affected)
    }

    pub fn get_settings(&self) -> Result<Settings, DbError> {
        let conn = self.conn()?;
        let row: (i64, i64, i64, i64, String) = conn.query_row(
            "
            SELECT history_limit, tracking_paused, max_clip_bytes, restore_clipboard_after_paste, denylist_bundle_ids
            FROM settings
            WHERE id = 1
            ",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )?;

        Ok(Settings {
            history_limit: row.0,
            tracking_paused: row.1 == 1,
            max_clip_bytes: row.2,
            restore_clipboard_after_paste: row.3 == 1,
            denylist_bundle_ids: serde_json::from_str(&row.4).unwrap_or_else(|_| default_denylist()),
        })
    }

    pub fn update_settings(&self, patch: SettingsPatch) -> Result<Settings, DbError> {
        let current = self.get_settings()?;
        let next = Settings {
            history_limit: patch.history_limit.unwrap_or(current.history_limit).max(1),
            tracking_paused: patch.tracking_paused.unwrap_or(current.tracking_paused),
            max_clip_bytes: patch.max_clip_bytes.unwrap_or(current.max_clip_bytes).max(1),
            restore_clipboard_after_paste: patch
                .restore_clipboard_after_paste
                .unwrap_or(current.restore_clipboard_after_paste),
            denylist_bundle_ids: patch
                .denylist_bundle_ids
                .unwrap_or(current.denylist_bundle_ids),
        };

        let denylist = serde_json::to_string(&next.denylist_bundle_ids)?;
        let conn = self.conn()?;
        conn.execute(
            "
            UPDATE settings SET
              history_limit = ?1,
              tracking_paused = ?2,
              max_clip_bytes = ?3,
              restore_clipboard_after_paste = ?4,
              denylist_bundle_ids = ?5
            WHERE id = 1
            ",
            params![
                next.history_limit,
                if next.tracking_paused { 1 } else { 0 },
                next.max_clip_bytes,
                if next.restore_clipboard_after_paste { 1 } else { 0 },
                denylist
            ],
        )?;

        Ok(next)
    }

    pub fn set_tracking_paused(&self, paused: bool) -> Result<(), DbError> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE settings SET tracking_paused = ?1 WHERE id = 1",
            params![if paused { 1 } else { 0 }],
        )?;
        Ok(())
    }

    pub fn prune_excess(&self, history_limit: i64) -> Result<usize, DbError> {
        let history_limit = history_limit.max(1);
        let conn = self.conn()?;

        let total: i64 = conn.query_row("SELECT COUNT(*) FROM clips", [], |row| row.get(0))?;
        let overflow = total - history_limit;
        if overflow <= 0 {
            return Ok(0);
        }

        let deleted = conn.execute(
            "
            DELETE FROM clips
            WHERE id IN (
                SELECT id
                FROM clips
                WHERE pinned = 0
                ORDER BY created_at ASC, id ASC
                LIMIT ?1
            )
            ",
            params![overflow],
        )?;
        Ok(deleted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_query_order_prefers_pinned() {
        let db = Database::new_in_memory().expect("db init");
        let first = db.insert_clip("first", "text").expect("insert first");
        let second = db.insert_clip("second", "text").expect("insert second");
        db.set_pinned(first.id, true).expect("pin first");

        let page = db.list_clips(None, 10, 0).expect("list clips");
        assert_eq!(page.items.first().map(|x| x.id), Some(first.id));
        assert!(page.items.iter().any(|x| x.id == second.id));
    }

    #[test]
    fn prune_keeps_pinned() {
        let db = Database::new_in_memory().expect("db init");
        let pinned = db.insert_clip("pinned", "text").expect("insert pinned");
        db.set_pinned(pinned.id, true).expect("set pin");
        db.insert_clip("a", "text").expect("insert a");
        db.insert_clip("b", "text").expect("insert b");

        let deleted = db.prune_excess(1).expect("prune");
        assert_eq!(deleted, 2);
        let page = db.list_clips(None, 10, 0).expect("list");
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].id, pinned.id);
    }

    #[test]
    fn settings_patch_applies() {
        let db = Database::new_in_memory().expect("db init");
        let updated = db
            .update_settings(SettingsPatch {
                history_limit: Some(300),
                tracking_paused: Some(true),
                max_clip_bytes: Some(1000),
                restore_clipboard_after_paste: Some(false),
                denylist_bundle_ids: Some(vec!["com.example".to_string()]),
            })
            .expect("update settings");

        assert_eq!(updated.history_limit, 300);
        assert!(updated.tracking_paused);
        assert_eq!(updated.max_clip_bytes, 1000);
        assert!(!updated.restore_clipboard_after_paste);
        assert_eq!(updated.denylist_bundle_ids, vec!["com.example".to_string()]);
    }
}
