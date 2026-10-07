-- OpenDAM SQLite Schema — Migration 001: Initial

PRAGMA journal_mode=WAL;
PRAGMA foreign_keys=ON;

-- ── Libraries ──────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS libraries (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    root_paths  TEXT NOT NULL,   -- JSON array of absolute paths
    created_at  INTEGER NOT NULL,
    asset_count INTEGER NOT NULL DEFAULT 0
);

-- ── Tags ───────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS tags (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL UNIQUE,
    color      TEXT,            -- color label enum string
    parent_id  TEXT REFERENCES tags(id) ON DELETE SET NULL
);

-- ── Assets ─────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS assets (
    id               TEXT PRIMARY KEY,
    file_path        TEXT NOT NULL UNIQUE,
    file_name        TEXT NOT NULL,
    extension        TEXT NOT NULL,
    kind             TEXT NOT NULL,
    size_bytes       INTEGER NOT NULL,
    modified_at      INTEGER NOT NULL,
    indexed_at       INTEGER NOT NULL,
    rating           INTEGER NOT NULL DEFAULT 0,
    color_label      TEXT NOT NULL DEFAULT 'none',
    is_favorite      INTEGER NOT NULL DEFAULT 0,
    description      TEXT NOT NULL DEFAULT '',
    thumbnail_path   TEXT,
    preview_status   TEXT NOT NULL DEFAULT 'none',
    metadata         TEXT NOT NULL DEFAULT '{}'  -- JSON object
);

CREATE INDEX IF NOT EXISTS idx_assets_kind         ON assets(kind);
CREATE INDEX IF NOT EXISTS idx_assets_rating       ON assets(rating);
CREATE INDEX IF NOT EXISTS idx_assets_color_label  ON assets(color_label);
CREATE INDEX IF NOT EXISTS idx_assets_is_favorite  ON assets(is_favorite);
CREATE INDEX IF NOT EXISTS idx_assets_modified_at  ON assets(modified_at);
CREATE INDEX IF NOT EXISTS idx_assets_extension    ON assets(extension);

-- ── Asset ↔ Tag join table ──────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS asset_tags (
    asset_id  TEXT NOT NULL REFERENCES assets(id)  ON DELETE CASCADE,
    tag_id    TEXT NOT NULL REFERENCES tags(id)    ON DELETE CASCADE,
    PRIMARY KEY (asset_id, tag_id)
);

-- ── Collections ────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS collections (
    id           TEXT PRIMARY KEY,
    name         TEXT NOT NULL,
    description  TEXT NOT NULL DEFAULT '',
    parent_id    TEXT REFERENCES collections(id) ON DELETE CASCADE,
    is_smart     INTEGER NOT NULL DEFAULT 0,
    filter_spec  TEXT,
    created_at   INTEGER NOT NULL
);

-- ── Collection ↔ Asset join table ─────────────────────────────────────────
CREATE TABLE IF NOT EXISTS collection_assets (
    collection_id  TEXT NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    asset_id       TEXT NOT NULL REFERENCES assets(id)      ON DELETE CASCADE,
    PRIMARY KEY (collection_id, asset_id)
);

-- ── Preview jobs ──────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS preview_jobs (
    id            TEXT PRIMARY KEY,
    asset_id      TEXT NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    status        TEXT NOT NULL DEFAULT 'queued',
    progress      INTEGER,
    error_message TEXT,
    created_at    INTEGER NOT NULL
);

-- ── Full-text search (FTS5) ───────────────────────────────────────────────
CREATE VIRTUAL TABLE IF NOT EXISTS assets_fts USING fts5(
    id UNINDEXED,
    file_name,
    description,
    metadata,
    content='assets',
    content_rowid='rowid'
);

-- Triggers to keep FTS in sync
CREATE TRIGGER IF NOT EXISTS assets_ai AFTER INSERT ON assets BEGIN
    INSERT INTO assets_fts(rowid, id, file_name, description, metadata)
    VALUES (new.rowid, new.id, new.file_name, new.description, new.metadata);
END;

CREATE TRIGGER IF NOT EXISTS assets_ad AFTER DELETE ON assets BEGIN
    INSERT INTO assets_fts(assets_fts, rowid, id, file_name, description, metadata)
    VALUES ('delete', old.rowid, old.id, old.file_name, old.description, old.metadata);
END;

CREATE TRIGGER IF NOT EXISTS assets_au AFTER UPDATE ON assets BEGIN
    INSERT INTO assets_fts(assets_fts, rowid, id, file_name, description, metadata)
    VALUES ('delete', old.rowid, old.id, old.file_name, old.description, old.metadata);
    INSERT INTO assets_fts(rowid, id, file_name, description, metadata)
    VALUES (new.rowid, new.id, new.file_name, new.description, new.metadata);
END;
