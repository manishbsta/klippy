pub const CREATE_CLIPS_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS clips (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  content TEXT NOT NULL CHECK (length(content) > 0),
  content_type TEXT NOT NULL CHECK (content_type IN ('text', 'url', 'code')),
  pinned INTEGER NOT NULL DEFAULT 0 CHECK (pinned IN (0, 1)),
  hash TEXT NOT NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
"#;

pub const CREATE_SETTINGS_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS settings (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  history_limit INTEGER NOT NULL DEFAULT 200,
  tracking_paused INTEGER NOT NULL DEFAULT 0 CHECK (tracking_paused IN (0, 1)),
  max_clip_bytes INTEGER NOT NULL DEFAULT 262144,
  restore_clipboard_after_paste INTEGER NOT NULL DEFAULT 1 CHECK (restore_clipboard_after_paste IN (0, 1)),
  denylist_bundle_ids TEXT NOT NULL
);
"#;

pub const CREATE_INDEX_CREATED_AT: &str =
    "CREATE INDEX IF NOT EXISTS idx_created_at ON clips(created_at DESC);";
pub const CREATE_INDEX_PINNED: &str = "CREATE INDEX IF NOT EXISTS idx_pinned ON clips(pinned);";
pub const CREATE_INDEX_HASH: &str = "CREATE INDEX IF NOT EXISTS idx_hash ON clips(hash);";
