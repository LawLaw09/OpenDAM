use anyhow::Result;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Pool, Sqlite};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct LibraryHandle {
    pub id: String,
    pub name: String,
    pub root_path: PathBuf,
    pub pool: Pool<Sqlite>,
}

pub struct Database {
    pub master_pool: Pool<Sqlite>,
    pub libraries: Arc<RwLock<HashMap<String, LibraryHandle>>>,
    pub pool: Pool<Sqlite>,
}

/// Connect to a per-library SQLite database with NAS-optimized Pragmas
pub async fn connect_library_sqlite(db_path: &Path) -> Result<Pool<Sqlite>> {
    let options = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(std::time::Duration::from_secs(10));

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .connect_with(options)
        .await?;

    // In-memory temp store and 64MB cache in RAM for limited NAS read/write
    sqlx::query(
        r#"
        PRAGMA temp_store = MEMORY;
        PRAGMA cache_size = -64000;
        PRAGMA foreign_keys = OFF;
        "#
    )
    .execute(&pool)
    .await?;

    // Execute the per-library schema
    sqlx::query(include_str!("../migrations/002_library_schema.sql"))
        .execute(&pool)
        .await?;

    // Backward compatibility: ensure dir_mtimes table exists
    let _ = sqlx::query(
        "CREATE TABLE IF NOT EXISTS dir_mtimes (dir_path TEXT PRIMARY KEY, mtime INTEGER NOT NULL)"
    )
    .execute(&pool)
    .await;

    // Backward compatibility migration for subcollections
    let _ = sqlx::query(
        "ALTER TABLE collections ADD COLUMN parent_id TEXT REFERENCES collections(id) ON DELETE CASCADE"
    )
    .execute(&pool)
    .await;

    // Purge any existing Revit backup files from database
    if let Ok(assets) = sqlx::query_as::<_, (String, String)>(
        "SELECT id, file_name FROM assets WHERE extension IN ('rfa', 'rvt')"
    )
    .fetch_all(&pool)
    .await
    {
        let mut purged = false;
        for (id, name) in assets {
            if crate::watcher::is_revit_backup(&name) {
                let _ = sqlx::query("DELETE FROM assets WHERE id = ?")
                    .bind(&id)
                    .execute(&pool)
                    .await;
                purged = true;
            }
        }
        if purged {
            let _ = sqlx::query("UPDATE libraries SET asset_count = (SELECT COUNT(*) FROM assets)")
                .execute(&pool)
                .await;
        }
    }

    Ok(pool)
}

impl Database {
    pub async fn new(master_db_url: &str) -> Result<Self> {
        let master_path = Path::new(master_db_url);
        let master_options = SqliteConnectOptions::new()
            .filename(master_path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(std::time::Duration::from_secs(10));

        let master_pool = SqlitePoolOptions::new()
            .max_connections(5)
            .acquire_timeout(std::time::Duration::from_secs(10))
            .connect_with(master_options)
            .await?;

        // Ensure libraries table exists in master DB
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS libraries (
                id          TEXT PRIMARY KEY,
                name        TEXT NOT NULL,
                root_paths  TEXT NOT NULL,
                created_at  INTEGER NOT NULL,
                asset_count INTEGER NOT NULL DEFAULT 0
            );
            "#
        )
        .execute(&master_pool)
        .await?;

        // Also ensure library schema tables exist on master_pool as fallback
        let _ = sqlx::query(include_str!("../migrations/002_library_schema.sql"))
            .execute(&master_pool)
            .await;

        let mut open_libraries = HashMap::new();
        let mut first_pool: Option<Pool<Sqlite>> = None;

        // Load all registered libraries and open their per-library SQLite pools
        if let Ok(rows) = sqlx::query_as::<_, (String, String, String)>(
            "SELECT id, name, root_paths FROM libraries"
        )
        .fetch_all(&master_pool)
        .await
        {
            for (id, name, root_paths_json) in rows {
                let paths: Vec<String> = serde_json::from_str(&root_paths_json).unwrap_or_default();
                if let Some(first_path) = paths.first() {
                    let root_path = PathBuf::from(first_path);
                    if root_path.exists() {
                        let opendam_dir = root_path.join(".opendam");
                        let thumbs_dir = opendam_dir.join("thumbs");
                        let _ = tokio::fs::create_dir_all(&thumbs_dir).await;

                        let lib_db_file = opendam_dir.join("library.sqlite");
                        match connect_library_sqlite(&lib_db_file).await {
                            Ok(lib_pool) => {
                                if first_pool.is_none() {
                                    first_pool = Some(lib_pool.clone());
                                }
                                open_libraries.insert(
                                    id.clone(),
                                    LibraryHandle {
                                        id,
                                        name,
                                        root_path,
                                        pool: lib_pool,
                                    },
                                );
                            }
                            Err(e) => {
                                tracing::error!("Failed to open library DB at {:?}: {}", lib_db_file, e);
                            }
                        }
                    }
                }
            }
        }

        let fallback_pool = first_pool.unwrap_or_else(|| master_pool.clone());
        let libraries = Arc::new(RwLock::new(open_libraries));

        let db = Self {
            master_pool,
            libraries,
            pool: fallback_pool,
        };

        // Sync metadata across all open libraries
        for lib in db.get_all_libraries().await {
            db.sync_library_metadata(&lib.pool).await;
        }

        Ok(db)
    }

    pub async fn sync_library_metadata(&self, lib_pool: &Pool<Sqlite>) {
        // 1. Sync tags from master_pool to lib_pool
        if let Ok(tags) = sqlx::query_as::<_, (String, String, Option<String>, Option<String>)>(
            "SELECT id, name, color, parent_id FROM tags"
        ).fetch_all(&self.master_pool).await {
            for (id, name, color, parent_id) in tags {
                let _ = sqlx::query("INSERT OR IGNORE INTO tags (id, name, color, parent_id) VALUES (?, ?, ?, ?)")
                    .bind(&id).bind(&name).bind(&color).bind(&parent_id)
                    .execute(lib_pool).await;
            }
        }

        // 2. Sync tags from lib_pool to master_pool
        if let Ok(tags) = sqlx::query_as::<_, (String, String, Option<String>, Option<String>)>(
            "SELECT id, name, color, parent_id FROM tags"
        ).fetch_all(lib_pool).await {
            for (id, name, color, parent_id) in tags {
                let _ = sqlx::query("INSERT OR IGNORE INTO tags (id, name, color, parent_id) VALUES (?, ?, ?, ?)")
                    .bind(&id).bind(&name).bind(&color).bind(&parent_id)
                    .execute(&self.master_pool).await;
            }
        }

        // 3. Sync collections from master_pool to lib_pool
        if let Ok(cols) = sqlx::query_as::<_, (String, String, String, Option<String>, bool, Option<String>, i64)>(
            "SELECT id, name, description, parent_id, is_smart, filter_spec, created_at FROM collections"
        ).fetch_all(&self.master_pool).await {
            for (id, name, desc, parent_id, is_smart, filter_spec, created_at) in cols {
                let _ = sqlx::query("INSERT OR IGNORE INTO collections (id, name, description, parent_id, is_smart, filter_spec, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
                    .bind(&id).bind(&name).bind(&desc).bind(&parent_id).bind(is_smart).bind(&filter_spec).bind(created_at)
                    .execute(lib_pool).await;
            }
        }

        // 4. Sync collections from lib_pool to master_pool
        if let Ok(cols) = sqlx::query_as::<_, (String, String, String, Option<String>, bool, Option<String>, i64)>(
            "SELECT id, name, description, parent_id, is_smart, filter_spec, created_at FROM collections"
        ).fetch_all(lib_pool).await {
            for (id, name, desc, parent_id, is_smart, filter_spec, created_at) in cols {
                let _ = sqlx::query("INSERT OR IGNORE INTO collections (id, name, description, parent_id, is_smart, filter_spec, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
                    .bind(&id).bind(&name).bind(&desc).bind(&parent_id).bind(is_smart).bind(&filter_spec).bind(created_at)
                    .execute(&self.master_pool).await;
            }
        }
    }

    pub async fn register_library(&self, id: &str, name: &str, root_path: &Path) -> Result<LibraryHandle> {
        let opendam_dir = root_path.join(".opendam");
        let thumbs_dir = opendam_dir.join("thumbs");
        tokio::fs::create_dir_all(&thumbs_dir).await?;

        let lib_db_file = opendam_dir.join("library.sqlite");
        let lib_pool = connect_library_sqlite(&lib_db_file).await?;

        // Sync metadata immediately
        self.sync_library_metadata(&lib_pool).await;

        let handle = LibraryHandle {
            id: id.to_string(),
            name: name.to_string(),
            root_path: root_path.to_path_buf(),
            pool: lib_pool,
        };

        let mut libs = self.libraries.write().await;
        libs.insert(id.to_string(), handle.clone());

        Ok(handle)
    }

    pub async fn unregister_library(&self, id: &str) -> Result<()> {
        let mut libs = self.libraries.write().await;
        libs.remove(id);
        Ok(())
    }

    pub async fn get_library_handle(&self, id: Option<&str>) -> Option<LibraryHandle> {
        let libs = self.libraries.read().await;
        if let Some(target_id) = id {
            if let Some(h) = libs.get(target_id) {
                return Some(h.clone());
            }
        }
        libs.values().next().cloned()
    }

    pub async fn get_library_root(&self, id: Option<&str>) -> Option<PathBuf> {
        self.get_library_handle(id).await.map(|h| h.root_path)
    }

    pub async fn get_pool(&self, id: Option<&str>) -> Pool<Sqlite> {
        if let Some(h) = self.get_library_handle(id).await {
            h.pool
        } else {
            self.pool.clone()
        }
    }

    pub async fn get_active_pool(&self) -> Pool<Sqlite> {
        self.get_pool(None).await
    }

    pub async fn get_all_libraries(&self) -> Vec<LibraryHandle> {
        let libs = self.libraries.read().await;
        libs.values().cloned().collect()
    }
}

/// Helper to resolve a stored relative path to an absolute path for the frontend
pub fn resolve_path(root: &Path, rel_or_abs: &str) -> String {
    let p = Path::new(rel_or_abs);
    if p.is_absolute() {
        rel_or_abs.to_string()
    } else {
        root.join(p).to_string_lossy().to_string()
    }
}

/// Helper to convert an absolute path to a portable relative path for storage
pub fn make_relative_path(root: &Path, p: &Path) -> String {
    if let Ok(rel) = p.strip_prefix(root) {
        rel.to_string_lossy().replace('\\', "/")
    } else {
        p.to_string_lossy().replace('\\', "/")
    }
}
