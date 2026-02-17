mod schema;

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(test)]
use crate::utils::hash::sha256_hex;

const DEFAULT_HISTORY_LIMIT: i64 = 200;
const DEFAULT_MAX_CLIP_BYTES: i64 = 10_485_760;

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
    pub media_path: Option<String>,
    pub thumb_path: Option<String>,
    pub mime_type: Option<String>,
    pub byte_size: i64,
    pub pixel_width: Option<i64>,
    pub pixel_height: Option<i64>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LatestClip {
    pub content: String,
    pub content_type: String,
    pub hash: String,
}

pub struct Database {
    conn: Mutex<Connection>,
}

const CLIP_COLUMNS: &str = "
    id,
    content,
    content_type,
    pinned,
    created_at,
    media_path,
    thumb_path,
    mime_type,
    byte_size,
    pixel_width,
    pixel_height
";

struct NewClip<'a> {
    content: &'a str,
    content_type: &'a str,
    hash: &'a str,
    media_path: Option<&'a str>,
    thumb_path: Option<&'a str>,
    mime_type: Option<&'a str>,
    byte_size: i64,
    pixel_width: Option<i64>,
    pixel_height: Option<i64>,
}

pub struct ImageClipInsert<'a> {
    pub content: &'a str,
    pub hash: &'a str,
    pub media_path: &'a str,
    pub thumb_path: &'a str,
    pub mime_type: &'a str,
    pub byte_size: i64,
    pub pixel_width: i64,
    pub pixel_height: i64,
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

        ensure_clips_schema(conn)?;

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

        conn.execute(
            "UPDATE settings SET max_clip_bytes = ?1 WHERE id = 1 AND max_clip_bytes < ?1",
            params![DEFAULT_MAX_CLIP_BYTES],
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
            let mut stmt = conn.prepare(&format!(
                "
                SELECT {CLIP_COLUMNS}
                FROM clips
                WHERE LOWER(content) LIKE ?1
                ORDER BY pinned DESC, created_at DESC, id DESC
                LIMIT ?2 OFFSET ?3
                "
            ))?;
            let rows = stmt.query_map(params![like, limit, offset], clip_from_row)?;

            let items = rows.collect::<Result<Vec<_>, _>>()?;
            let total: i64 = conn.query_row(
                "SELECT COUNT(*) FROM clips WHERE LOWER(content) LIKE ?1",
                params![format!("%{}%", search.to_lowercase())],
                |row| row.get(0),
            )?;
            (items, total)
        } else {
            let mut stmt = conn.prepare(&format!(
                "
                SELECT {CLIP_COLUMNS}
                FROM clips
                ORDER BY pinned DESC, created_at DESC, id DESC
                LIMIT ?1 OFFSET ?2
                "
            ))?;
            let rows = stmt.query_map(params![limit, offset], clip_from_row)?;

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
            "SELECT content, content_type, hash FROM clips ORDER BY created_at DESC, id DESC LIMIT 1",
            [],
            |row| {
                Ok(LatestClip {
                    content: row.get(0)?,
                    content_type: row.get(1)?,
                    hash: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(DbError::from)
    }

    pub fn list_image_clips_desc(&self, limit: i64) -> Result<Vec<Clip>, DbError> {
        let limit = limit.max(1);
        let conn = self.conn()?;
        let mut stmt = conn.prepare(&format!(
            "
            SELECT {CLIP_COLUMNS}
            FROM clips
            WHERE content_type = 'image'
            ORDER BY created_at DESC, id DESC
            LIMIT ?1
            "
        ))?;
        let rows = stmt.query_map(params![limit], clip_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    #[cfg(test)]
    pub fn insert_clip(&self, content: &str, content_type: &str) -> Result<Clip, DbError> {
        let hash = sha256_hex(content);
        self.insert_new_clip(NewClip {
            content,
            content_type,
            hash: &hash,
            media_path: None,
            thumb_path: None,
            mime_type: None,
            byte_size: content.len() as i64,
            pixel_width: None,
            pixel_height: None,
        })
    }

    pub fn insert_text_clip(
        &self,
        content: &str,
        content_type: &str,
        hash: &str,
    ) -> Result<Clip, DbError> {
        self.insert_new_clip(NewClip {
            content,
            content_type,
            hash,
            media_path: None,
            thumb_path: None,
            mime_type: None,
            byte_size: content.len() as i64,
            pixel_width: None,
            pixel_height: None,
        })
    }

    pub fn insert_image_clip(&self, image: ImageClipInsert<'_>) -> Result<Clip, DbError> {
        self.insert_new_clip(NewClip {
            content: image.content,
            content_type: "image",
            hash: image.hash,
            media_path: Some(image.media_path),
            thumb_path: Some(image.thumb_path),
            mime_type: Some(image.mime_type),
            byte_size: image.byte_size,
            pixel_width: Some(image.pixel_width),
            pixel_height: Some(image.pixel_height),
        })
    }

    fn insert_new_clip(&self, new_clip: NewClip<'_>) -> Result<Clip, DbError> {
        let conn = self.conn()?;
        conn.execute(
            "
            INSERT INTO clips (
                content,
                content_type,
                pinned,
                hash,
                media_path,
                thumb_path,
                mime_type,
                byte_size,
                pixel_width,
                pixel_height
            ) VALUES (?1, ?2, 0, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ",
            params![
                new_clip.content,
                new_clip.content_type,
                new_clip.hash,
                new_clip.media_path,
                new_clip.thumb_path,
                new_clip.mime_type,
                new_clip.byte_size,
                new_clip.pixel_width,
                new_clip.pixel_height,
            ],
        )?;
        let id = conn.last_insert_rowid();
        Ok(self.get_clip_internal(&conn, id)?)
    }

    pub fn get_clip(&self, id: i64) -> Result<Option<Clip>, DbError> {
        let conn = self.conn()?;
        self.get_clip_internal(&conn, id)
            .optional()
            .map_err(DbError::from)
    }

    fn get_clip_internal(&self, conn: &Connection, id: i64) -> Result<Clip, rusqlite::Error> {
        conn.query_row(
            &format!("SELECT {CLIP_COLUMNS} FROM clips WHERE id = ?1"),
            params![id],
            clip_from_row,
        )
    }

    pub fn set_pinned(&self, id: i64, pinned: bool) -> Result<Option<Clip>, DbError> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE clips SET pinned = ?1 WHERE id = ?2",
            params![if pinned { 1 } else { 0 }, id],
        )?;
        self.get_clip_internal(&conn, id)
            .optional()
            .map_err(DbError::from)
    }

    pub fn delete_clip(&self, id: i64) -> Result<Option<Clip>, DbError> {
        let conn = self.conn()?;
        let clip = self
            .get_clip_internal(&conn, id)
            .optional()
            .map_err(DbError::from)?;
        if clip.is_none() {
            return Ok(None);
        }
        conn.execute("DELETE FROM clips WHERE id = ?1", params![id])?;
        Ok(clip)
    }

    pub fn delete_all_clips(&self) -> Result<Vec<Clip>, DbError> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(&format!("SELECT {CLIP_COLUMNS} FROM clips"))?;
        let rows = stmt.query_map([], clip_from_row)?;
        let clips = rows.collect::<Result<Vec<_>, _>>()?;
        conn.execute("DELETE FROM clips", [])?;
        Ok(clips)
    }

    pub fn delete_clips_by_ids(&self, ids: &[i64]) -> Result<Vec<Clip>, DbError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut conn = self.conn()?;
        let mut deleted = Vec::with_capacity(ids.len());
        let tx = conn.transaction()?;

        for id in ids {
            let clip = tx
                .query_row(
                    &format!("SELECT {CLIP_COLUMNS} FROM clips WHERE id = ?1"),
                    params![id],
                    clip_from_row,
                )
                .optional()?;
            if let Some(clip) = clip {
                tx.execute("DELETE FROM clips WHERE id = ?1", params![id])?;
                deleted.push(clip);
            }
        }

        tx.commit()?;
        Ok(deleted)
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
            denylist_bundle_ids: serde_json::from_str(&row.4)
                .unwrap_or_else(|_| default_denylist()),
        })
    }

    pub fn prune_excess(&self, history_limit: i64) -> Result<Vec<Clip>, DbError> {
        let history_limit = history_limit.max(1);
        let mut conn = self.conn()?;

        let total: i64 = conn.query_row("SELECT COUNT(*) FROM clips", [], |row| row.get(0))?;
        let overflow = total - history_limit;
        if overflow <= 0 {
            return Ok(Vec::new());
        }

        let clips = {
            let mut stmt = conn.prepare(&format!(
                "
                SELECT {CLIP_COLUMNS}
                FROM clips
                WHERE pinned = 0
                ORDER BY created_at ASC, id ASC
                LIMIT ?1
                "
            ))?;
            let rows = stmt.query_map(params![overflow], clip_from_row)?;
            rows.collect::<Result<Vec<_>, _>>()?
        };

        if clips.is_empty() {
            return Ok(Vec::new());
        }

        let tx = conn.transaction()?;
        for clip in &clips {
            tx.execute("DELETE FROM clips WHERE id = ?1", params![clip.id])?;
        }
        tx.commit()?;

        Ok(clips)
    }

    pub fn referenced_media_paths(&self) -> Result<HashSet<String>, DbError> {
        let conn = self.conn()?;
        let mut referenced = HashSet::new();
        let mut stmt = conn.prepare("SELECT media_path, thumb_path FROM clips")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
            ))
        })?;

        for row in rows {
            let (media_path, thumb_path) = row?;
            if let Some(path) = media_path {
                referenced.insert(path);
            }
            if let Some(path) = thumb_path {
                referenced.insert(path);
            }
        }

        Ok(referenced)
    }
}

fn clip_from_row(row: &Row<'_>) -> Result<Clip, rusqlite::Error> {
    Ok(Clip {
        id: row.get(0)?,
        content: row.get(1)?,
        content_type: row.get(2)?,
        pinned: row.get::<_, i64>(3)? == 1,
        created_at: row.get(4)?,
        media_path: row.get(5)?,
        thumb_path: row.get(6)?,
        mime_type: row.get(7)?,
        byte_size: row.get(8)?,
        pixel_width: row.get(9)?,
        pixel_height: row.get(10)?,
    })
}

fn ensure_clips_schema(conn: &Connection) -> Result<(), DbError> {
    let table_sql: Option<String> = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'clips'",
            [],
            |row| row.get(0),
        )
        .optional()?;

    if table_sql.is_none() {
        conn.execute_batch(schema::CREATE_CLIPS_TABLE)?;
        return Ok(());
    }

    if clips_schema_is_current(conn, table_sql.as_deref().unwrap_or_default())? {
        return Ok(());
    }

    migrate_clips_table(conn)?;
    Ok(())
}

fn clips_schema_is_current(conn: &Connection, table_sql: &str) -> Result<bool, DbError> {
    if !table_sql.contains("'image'") {
        return Ok(false);
    }

    let mut columns = HashSet::new();
    let mut stmt = conn.prepare("PRAGMA table_info(clips)")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for row in rows {
        columns.insert(row?);
    }

    let required = [
        "media_path",
        "thumb_path",
        "mime_type",
        "byte_size",
        "pixel_width",
        "pixel_height",
    ];

    Ok(required.iter().all(|column| columns.contains(*column)))
}

fn migrate_clips_table(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch("DROP TABLE IF EXISTS clips_v2;")?;
    conn.execute_batch(schema::CREATE_CLIPS_TABLE_V2)?;
    conn.execute_batch(
        "
        INSERT INTO clips_v2 (
            id,
            content,
            content_type,
            pinned,
            hash,
            byte_size,
            created_at
        )
        SELECT
            id,
            content,
            content_type,
            pinned,
            hash,
            length(CAST(content AS BLOB)),
            created_at
        FROM clips;
        ",
    )?;
    conn.execute_batch("DROP TABLE clips;")?;
    conn.execute_batch("ALTER TABLE clips_v2 RENAME TO clips;")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;

    use rusqlite::Connection;
    use uuid::Uuid;

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
        assert_eq!(deleted.len(), 2);
        let page = db.list_clips(None, 10, 0).expect("list");
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].id, pinned.id);
    }

    #[test]
    fn defaults_max_clip_size_to_ten_mb() {
        let db = Database::new_in_memory().expect("db init");
        let settings = db.get_settings().expect("settings");
        assert_eq!(settings.max_clip_bytes, 10_485_760);
    }

    #[test]
    fn inserts_image_clip_with_metadata() {
        let db = Database::new_in_memory().expect("db init");
        let clip = db
            .insert_image_clip(ImageClipInsert {
                content: "Image | PNG | 20x10 | 0.1 MB",
                hash: "abc",
                media_path: "/tmp/originals/a.png",
                thumb_path: "/tmp/thumbs/a.png",
                mime_type: "image/png",
                byte_size: 1234,
                pixel_width: 20,
                pixel_height: 10,
            })
            .expect("insert image");

        assert_eq!(clip.content_type, "image");
        assert_eq!(clip.media_path.as_deref(), Some("/tmp/originals/a.png"));
        assert_eq!(clip.thumb_path.as_deref(), Some("/tmp/thumbs/a.png"));
        assert_eq!(clip.mime_type.as_deref(), Some("image/png"));
        assert_eq!(clip.pixel_width, Some(20));
        assert_eq!(clip.pixel_height, Some(10));
    }

    #[test]
    fn migrates_v1_schema_to_v2() {
        let db_path = env::temp_dir().join(format!("klippy-migrate-{}.sqlite3", Uuid::new_v4()));
        let conn = Connection::open(&db_path).expect("open old db");
        conn.execute_batch(
            r#"
            CREATE TABLE clips (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              content TEXT NOT NULL CHECK (length(content) > 0),
              content_type TEXT NOT NULL CHECK (content_type IN ('text', 'url', 'code')),
              pinned INTEGER NOT NULL DEFAULT 0 CHECK (pinned IN (0, 1)),
              hash TEXT NOT NULL,
              created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE settings (
              id INTEGER PRIMARY KEY CHECK (id = 1),
              history_limit INTEGER NOT NULL DEFAULT 200,
              tracking_paused INTEGER NOT NULL DEFAULT 0 CHECK (tracking_paused IN (0, 1)),
              max_clip_bytes INTEGER NOT NULL DEFAULT 2097152,
              restore_clipboard_after_paste INTEGER NOT NULL DEFAULT 1 CHECK (restore_clipboard_after_paste IN (0, 1)),
              denylist_bundle_ids TEXT NOT NULL
            );
            INSERT INTO clips (content, content_type, pinned, hash) VALUES ('legacy text', 'text', 0, 'abc');
            INSERT INTO settings (id, history_limit, tracking_paused, max_clip_bytes, restore_clipboard_after_paste, denylist_bundle_ids)
            VALUES (1, 200, 0, 2097152, 1, '[]');
            "#,
        )
        .expect("seed old schema");
        drop(conn);

        let db = Database::new(&db_path).expect("open migrated db");
        let inserted = db
            .insert_image_clip(ImageClipInsert {
                content: "Image | PNG | 10x10 | 0.1 MB",
                hash: "hash-image",
                media_path: "/tmp/media.png",
                thumb_path: "/tmp/thumb.png",
                mime_type: "image/png",
                byte_size: 128,
                pixel_width: 10,
                pixel_height: 10,
            })
            .expect("insert image after migration");

        assert_eq!(inserted.content_type, "image");
        let list = db.list_clips(None, 10, 0).expect("list migrated rows");
        assert!(list.items.iter().any(|clip| clip.content == "legacy text"));
        assert!(list.items.iter().any(|clip| clip.content_type == "image"));

        let settings = db.get_settings().expect("settings after migration");
        assert_eq!(settings.max_clip_bytes, 10_485_760);

        let _ = fs::remove_file(&db_path);
    }
}
