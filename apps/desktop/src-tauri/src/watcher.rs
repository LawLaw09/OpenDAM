use anyhow::Result;
use chrono::Utc;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};
use uuid::Uuid;
use walkdir::WalkDir;

use crate::db::{make_relative_path, Database};
use crate::models::AssetKind;

// ── Advisory lock guard ─────────────────────────────────────────────────────

/// RAII guard: writes `.opendam/index.lock` on creation, removes it on drop.
/// On NAS, multiple machines may attempt to re-index simultaneously on startup.
/// The second machine detects the lock and skips indexing, preventing interleaved
/// batch writes that would corrupt the asset catalogue.
struct IndexLock(PathBuf);

impl IndexLock {
    /// Try to acquire the lock.
    ///
    /// Returns `None` if another instance acquired the lock within the last
    /// `stale_after` seconds (default: 120s). Returns `Some(guard)` on success.
    fn try_acquire(opendam_dir: &Path, stale_after: u64) -> Option<Self> {
        let lock_path = opendam_dir.join("index.lock");

        // If the lock file exists and is recent, another indexer is running.
        if let Ok(meta) = std::fs::metadata(&lock_path) {
            if let Ok(modified) = meta.modified() {
                let age = modified
                    .duration_since(UNIX_EPOCH)
                    .map(|t| {
                        let now = std::time::SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        now.saturating_sub(t.as_secs())
                    })
                    .unwrap_or(u64::MAX);
                if age < stale_after {
                    tracing::info!(
                        "index.lock is {}s old — another machine is indexing, skipping.",
                        age
                    );
                    return None;
                }
            }
        }

        // Write current Unix timestamp into the lock file.
        let ts = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs()
            .to_string();

        if let Err(e) = std::fs::write(&lock_path, &ts) {
            // Couldn't write — NAS may be read-only. Log and proceed anyway
            // (best-effort: don't block indexing just because we can't lock).
            tracing::warn!("Could not write index.lock at {:?}: {} — proceeding without lock", lock_path, e);
        }

        Some(IndexLock(lock_path))
    }
}

impl Drop for IndexLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

// Supported extensions
const SUPPORTED_EXTENSIONS: &[&str] = &[
    // 3D & CAD
    "fbx", "obj", "3ds", "dae", "gltf", "glb", "skp", "blend",
    "c4d", "3dm", "rfa", "rvt", "dwg", "dxf", "stl", "usd", "usdz", "max",
    "abc", "ply", "x3d", "wrl",
    // Materials
    "mat", "vrmat", "sbsar", "sbs", "mdl",
    // Textures
    "jpg", "jpeg", "png", "tga", "tif", "tiff", "bmp", "dds",
    "ktx", "ktx2", "webp", "avif",
    // HDRI/HDR
    "hdr", "exr", "hdri",
    // IES
    "ies", "ldt",
    // Video
    "mp4", "mov", "avi", "mkv", "webm", "m4v",
    // Image layered
    "psd", "psb", "ai", "svg", "gif",
    // Docs
    "pdf", "txt", "md",
];

pub fn is_supported(ext: &str) -> bool {
    SUPPORTED_EXTENSIONS.contains(&ext.to_lowercase().as_str())
}

/// Check whether a file is an Autodesk Revit backup file (e.g. `filename.0001.rfa` or `filename.0002.rvt`).
pub fn is_revit_backup(file_name: &str) -> bool {
    let lower = file_name.to_lowercase();
    if lower.ends_with(".rfa") || lower.ends_with(".rvt") {
        if let Some((stem, _ext)) = lower.rsplit_once('.') {
            if let Some((_base, suffix)) = stem.rsplit_once('.') {
                return (3..=5).contains(&suffix.len()) && suffix.chars().all(|c| c.is_ascii_digit());
            }
        }
    }
    false
}

/// Tracks directories the indexer could not read, as library-relative `/` paths.
#[derive(Default)]
struct UnreadableDirs {
    /// A whole root was unreachable: no asset may be considered deleted.
    whole_library: bool,
    dir_prefixes: Vec<String>,
}

impl UnreadableDirs {
    fn add_dir(&mut self, rel_dir: String) {
        let mut prefix = rel_dir.trim_end_matches('/').to_string();
        if prefix.is_empty() {
            self.whole_library = true;
            return;
        }
        prefix.push('/');
        self.dir_prefixes.push(prefix);
    }

    /// True if `rel_path` lies in a part of the tree we could not read.
    fn covers(&self, rel_path: &str) -> bool {
        self.whole_library
            || self.dir_prefixes.iter().any(|p| {
                rel_path.starts_with(p.as_str()) || rel_path == p.trim_end_matches('/')
            })
    }
}

struct PendingAsset {
    id: String,
    file_path: String,
    file_name: String,
    extension: String,
    kind_str: &'static str,
    size_bytes: i64,
    modified_at: i64,
    now: i64,
    preview_status: &'static str,
    thumbnail_path: Option<String>,
    needs_job: bool,
    is_update: bool,
}

/// Walk `paths` and index all found files as Assets into the database.
/// Highly optimized for NAS:
/// 1. Single initial query loads existing assets into RAM (0 queries during walk).
/// 2. Skips `.opendam` directory.
/// 3. Pre-checks `.opendam/thumbs/` to never re-render existing previews across network.
/// 4. Groups writes into chunks of 200 inside single transactions (cutting NAS I/O by 99%).
/// 5. Advisory index.lock prevents two machines from indexing the same library simultaneously.
pub async fn index_paths(db: &Database, library_id: &str, paths: &[String]) -> Result<()> {
    // Look the library up by exact id. Never fall back to another library: if this
    // library's root was offline at startup it is not open, and indexing it against a
    // different library's pool would mark every asset in that library as deleted.
    let (pool, root_path) = match db.get_library_handle_exact(library_id).await {
        Some(h) => (h.pool, h.root_path),
        None => {
            tracing::warn!("Library {} is not open (root offline?) — skipping index", library_id);
            return Ok(());
        }
    };

    // ── Advisory lock: skip if another machine is already indexing ────────
    let opendam_dir = root_path.join(".opendam");
    let _lock = match IndexLock::try_acquire(&opendam_dir, 120) {
        Some(guard) => guard,
        None => return Ok(()), // another instance is indexing — yield
    };


    let thumbs_dir = root_path.join(".opendam").join("thumbs");
    let now = Utc::now().timestamp_millis();

    // ── 1. Load existing assets in ONE single query into memory ──────────
    let existing_rows: Vec<(String, String, i64, i64, Option<String>)> = sqlx::query_as(
        "SELECT id, file_path, modified_at, size_bytes, thumbnail_path FROM assets"
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let mut existing_map: HashMap<String, (String, i64, i64, Option<String>)> = HashMap::with_capacity(existing_rows.len());
    for (id, file_path, modified_at, size_bytes, thumb) in existing_rows {
        // Normalize path separators to forward slash for reliable matching
        let norm = file_path.replace('\\', "/");
        existing_map.insert(norm, (id, modified_at, size_bytes, thumb));
    }

    let mut seen_paths: HashSet<String> = HashSet::with_capacity(existing_map.len() + 100);
    let mut pending_batch: Vec<PendingAsset> = Vec::with_capacity(200);
    // Parts of the tree we could not read. Assets under them must not be treated as
    // deleted: an offline NAS or a permission error is not the same as an empty folder.
    let mut unreadable = UnreadableDirs::default();

    // ── 2. Walk directory tree ──────────────────────────────────────────
    for root in paths {
        let root_dir = Path::new(root);
        if !root_dir.is_dir() {
            tracing::warn!("Library root {:?} is not reachable — skipping deletion pass", root_dir);
            unreadable.whole_library = true;
            continue;
        }
        let walker = WalkDir::new(root_dir)
            .follow_links(true)
            .into_iter()
            .filter_entry(|e| {
                // Skip hidden folders and .opendam internal folder
                let file_name = e.file_name().to_string_lossy();
                if e.file_type().is_dir() && file_name.starts_with('.') {
                    return false;
                }
                true
            });

        for entry in walker {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    match e.path() {
                        Some(p) if e.depth() > 0 => {
                            tracing::warn!("Could not read {:?} while indexing: {}", p, e);
                            unreadable.add_dir(make_relative_path(&root_path, p));
                        }
                        _ => {
                            tracing::warn!("Could not read library root {:?}: {}", root_dir, e);
                            unreadable.whole_library = true;
                        }
                    }
                    continue;
                }
            };
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            if !is_supported(&ext) {
                continue;
            }

            let file_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            // Skip Revit backup files (e.g. name.0001.rfa, name.0002.rvt)
            if is_revit_backup(&file_name) {
                continue;
            }

            let rel_path = make_relative_path(&root_path, path);
            seen_paths.insert(rel_path.clone());

            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            let size_bytes = meta.len() as i64;
            let modified_at = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as i64)
                .unwrap_or(now);

            // In-memory cache check: if unchanged, SKIP completely!
            if let Some((_, cached_mod, cached_size, _)) = existing_map.get(&rel_path) {
                if *cached_mod == modified_at && *cached_size == size_bytes {
                    continue; // 0 disk writes, 0 SQL queries!
                }
            }

            let kind = AssetKind::from_extension(&ext);
            let kind_str = kind.as_str();

            // Pre-check if thumbnail already exists in .opendam/thumbs/
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(&file_name);
            let thumb_png = thumbs_dir.join(format!("{}_thumb.png", stem));
            let thumb_jpg = thumbs_dir.join(format!("{}_thumb.jpg", stem));

            let mut preview_status = "none";
            let mut thumbnail_path = None;
            let mut needs_job = false;

            if thumb_png.exists() {
                preview_status = "done";
                thumbnail_path = Some(format!(".opendam/thumbs/{}_thumb.png", stem));
            } else if thumb_jpg.exists() {
                preview_status = "done";
                thumbnail_path = Some(format!(".opendam/thumbs/{}_thumb.jpg", stem));
            } else {
                if crate::preview::needs_preview(&ext) {
                    preview_status = "pending";
                    needs_job = true;
                }
            }

            let (id, is_update) = if let Some((existing_id, _, _, existing_thumb)) = existing_map.get(&rel_path) {
                if thumbnail_path.is_none() && existing_thumb.is_some() {
                    thumbnail_path = existing_thumb.clone();
                    preview_status = "done";
                    needs_job = false;
                }
                (existing_id.clone(), true)
            } else {
                (Uuid::new_v4().to_string(), false)
            };

            pending_batch.push(PendingAsset {
                id,
                file_path: rel_path,
                file_name,
                extension: ext,
                kind_str,
                size_bytes,
                modified_at,
                now,
                preview_status,
                thumbnail_path,
                needs_job,
                is_update,
            });

            // Flush chunk if batch reaches 200 items
            if pending_batch.len() >= 200 {
                commit_asset_batch(&pool, &pending_batch).await?;
                pending_batch.clear();
            }
        }
    }

    // Flush any remaining assets
    if !pending_batch.is_empty() {
        commit_asset_batch(&pool, &pending_batch).await?;
        pending_batch.clear();
    }

    // ── 3. Detect and remove deleted files in batch ───────────────────────
    let to_delete: Vec<String> = existing_map
        .into_iter()
        .filter(|(rel, _)| !seen_paths.contains(rel) && !unreadable.covers(rel))
        .map(|(_, (id, _, _, _))| id)
        .collect();

    if !to_delete.is_empty() {
        for chunk in to_delete.chunks(200) {
            let placeholders = chunk.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            // foreign_keys is OFF, so ON DELETE CASCADE does not fire: clean up the
            // join tables by hand or tag/collection counts keep the removed assets.
            for table in ["asset_tags", "collection_assets", "preview_jobs"] {
                let sql = format!("DELETE FROM {} WHERE asset_id IN ({})", table, placeholders);
                let mut query = sqlx::query(&sql);
                for id in chunk {
                    query = query.bind(id);
                }
                let _ = query.execute(&pool).await;
            }
            let sql = format!("DELETE FROM assets WHERE id IN ({})", placeholders);
            let mut query = sqlx::query(&sql);
            for id in chunk {
                query = query.bind(id);
            }
            let _ = query.execute(&pool).await;
        }
    }

    // ── 4. Queue previews that older builds never queued ─────────────────
    let include_python = crate::preview::python_worker_available();
    if let Err(e) = backfill_missing_previews(&pool, now, include_python).await {
        tracing::warn!("Preview backfill failed for library {}: {}", library_id, e);
    }

    // ── 5. Update asset count ───────────────────────────────────────────
    let count_row: Option<(i64,)> = sqlx::query_as("SELECT COUNT(*) FROM assets").fetch_optional(&pool).await?;
    let total_count = count_row.map(|(c,)| c).unwrap_or(0);

    let _ = sqlx::query("UPDATE libraries SET asset_count = ? WHERE id = ?")
        .bind(total_count)
        .bind(library_id)
        .execute(&pool)
        .await;

    let _ = sqlx::query("UPDATE libraries SET asset_count = ? WHERE id = ?")
        .bind(total_count)
        .bind(library_id)
        .execute(&db.master_pool)
        .await;

    tracing::info!("NAS-efficient indexing complete for library {}: {} total assets", library_id, total_count);
    Ok(())
}

/// Queue thumbnail jobs for assets stuck at `preview_status = 'none'`.
///
/// Unchanged files are skipped by the walk, so an asset indexed by a build that did not
/// yet preview its format (e.g. `.png`/`.jpg` before image previews existed) stayed at
/// `none` forever and never got a thumbnail. This finds those rows and queues them.
///
/// Formats rendered in-process by Rust are always backfilled; Python-rendered ones only
/// when `include_python` (a worker interpreter was found for this run).
/// Rows in `error` or `done` are left alone. Orphaned job rows are removed too.
/// Returns the number of jobs queued.
async fn backfill_missing_previews(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    now: i64,
    include_python: bool,
) -> Result<u64> {
    let mut exts: Vec<&str> = crate::preview::rust_preview_exts();
    if include_python {
        exts.extend_from_slice(crate::preview::PYTHON_PREVIEW_EXTS);
    }
    exts.sort_unstable();
    exts.dedup();
    let placeholders = exts.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let stuck = format!(
        "a.preview_status = 'none' AND a.thumbnail_path IS NULL AND lower(a.extension) IN ({})",
        placeholders
    );

    let mut tx = pool.begin().await?;

    sqlx::query("DELETE FROM preview_jobs WHERE asset_id NOT IN (SELECT id FROM assets)")
        .execute(&mut *tx)
        .await?;

    let insert_sql = format!(
        "INSERT INTO preview_jobs (id, asset_id, status, created_at)
         SELECT lower(hex(randomblob(16))), a.id, 'pending', ? FROM assets a
         WHERE {} AND NOT EXISTS (SELECT 1 FROM preview_jobs p WHERE p.asset_id = a.id)",
        stuck
    );
    let mut insert = sqlx::query(&insert_sql).bind(now);
    for e in &exts {
        insert = insert.bind(*e);
    }
    let queued = insert.execute(&mut *tx).await?.rows_affected();

    if queued > 0 {
        let update_sql = format!(
            "UPDATE assets SET preview_status = 'pending' WHERE id IN (
                SELECT a.id FROM assets a WHERE {}
                AND EXISTS (SELECT 1 FROM preview_jobs p WHERE p.asset_id = a.id AND p.status = 'pending'))",
            stuck
        );
        let mut update = sqlx::query(&update_sql);
        for e in &exts {
            update = update.bind(*e);
        }
        update.execute(&mut *tx).await?;
        tracing::info!("Queued {} missing previews", queued);
    }

    tx.commit().await?;
    Ok(queued)
}

/// Helper to execute a batch of assets inside a single database transaction
async fn commit_asset_batch(pool: &sqlx::Pool<sqlx::Sqlite>, batch: &[PendingAsset]) -> Result<()> {
    let mut tx = pool.begin().await?;

    for item in batch {
        if item.is_update {
            sqlx::query(
                r#"UPDATE assets SET
                    size_bytes = ?,
                    modified_at = ?,
                    preview_status = CASE WHEN preview_status = 'done' THEN 'done' ELSE ? END,
                    thumbnail_path = COALESCE(?, thumbnail_path)
                   WHERE id = ?"#
            )
            .bind(item.size_bytes)
            .bind(item.modified_at)
            .bind(item.preview_status)
            .bind(&item.thumbnail_path)
            .bind(&item.id)
            .execute(&mut *tx)
            .await?;

            // A changed file that still needs a preview gets marked 'pending' above;
            // without a job row the worker never picks it up and the spinner never ends.
            if item.needs_job {
                let _ = sqlx::query("DELETE FROM preview_jobs WHERE asset_id = ?")
                    .bind(&item.id)
                    .execute(&mut *tx)
                    .await;
                let _ = sqlx::query(
                    "INSERT INTO preview_jobs (id, asset_id, status, created_at) VALUES (?, ?, 'pending', ?)"
                )
                .bind(Uuid::new_v4().to_string())
                .bind(&item.id)
                .bind(item.now)
                .execute(&mut *tx)
                .await;
            }
        } else {
            sqlx::query(
                r#"INSERT INTO assets
                   (id, file_path, file_name, extension, kind, size_bytes,
                    modified_at, indexed_at, rating, color_label, is_favorite, description,
                    thumbnail_path, preview_status, metadata)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0, 'none', 0, '', ?, ?, '{}')"#
            )
            .bind(&item.id)
            .bind(&item.file_path)
            .bind(&item.file_name)
            .bind(&item.extension)
            .bind(item.kind_str)
            .bind(item.size_bytes)
            .bind(item.modified_at)
            .bind(item.now)
            .bind(&item.thumbnail_path)
            .bind(item.preview_status)
            .execute(&mut *tx)
            .await?;

            if item.needs_job {
                let job_id = Uuid::new_v4().to_string();
                let _ = sqlx::query(
                    "INSERT INTO preview_jobs (id, asset_id, status, created_at) VALUES (?, ?, 'pending', ?)"
                )
                .bind(&job_id)
                .bind(&item.id)
                .bind(item.now)
                .execute(&mut *tx)
                .await;
            }
        }
    }

    tx.commit().await?;
    Ok(())
}

/// LibraryWatcher placeholder
#[derive(Default)]
pub struct LibraryWatcher {}

impl LibraryWatcher {
    pub fn new() -> Self {
        Self {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unreadable_dirs_cover_only_their_subtree() {
        let mut u = UnreadableDirs::default();
        u.add_dir("models/locked".into());
        assert!(u.covers("models/locked/chair.max"));
        assert!(u.covers("models/locked/deep/chair.max"));
        assert!(!u.covers("models/locked2/chair.max"));
        assert!(!u.covers("models/chair.max"));
        assert!(!u.whole_library);
        u.add_dir(String::new());
        assert!(u.covers("anything.max"));
    }

    fn scratch_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("opendam-test-{}-{}", tag, Uuid::new_v4()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    async fn asset_count(db: &Database, lib_id: &str) -> i64 {
        let h = db.get_library_handle_exact(lib_id).await.unwrap();
        sqlx::query_scalar("SELECT COUNT(*) FROM assets").fetch_one(&h.pool).await.unwrap()
    }

    /// An unopened library id must not be indexed against another library's DB, and an
    /// unreachable root must not mark the library's assets as deleted.
    #[tokio::test]
    async fn offline_root_does_not_delete_assets() {
        let base = scratch_dir("offline");
        let lib_root = base.join("lib");
        std::fs::create_dir_all(lib_root.join("sub")).unwrap();
        std::fs::write(lib_root.join("sub").join("chair.obj"), b"v 0 0 0\n").unwrap();
        std::fs::write(lib_root.join("table.obj"), b"v 0 0 0\n").unwrap();

        let master = base.join("master.sqlite");
        let db = Database::new(master.to_str().unwrap()).await.unwrap();
        db.register_library("lib-a", "A", &lib_root).await.unwrap();
        let root_str = lib_root.to_string_lossy().to_string();

        index_paths(&db, "lib-a", std::slice::from_ref(&root_str)).await.unwrap();
        assert_eq!(asset_count(&db, "lib-a").await, 2);

        // A library that is registered in master but not open (its DB failed to open)
        // used to fall back to lib-a's pool, walk its own (readable, empty) root, see
        // none of lib-a's files, and delete them all.
        let other_root = base.join("other-lib");
        std::fs::create_dir_all(&other_root).unwrap();
        let other = other_root.to_string_lossy().to_string();
        index_paths(&db, "lib-not-open", &[other]).await.unwrap();
        assert_eq!(asset_count(&db, "lib-a").await, 2);
        let offline = base.join("offline-nas").to_string_lossy().to_string();

        // lib-a's own root becoming unreachable must not wipe it either.
        index_paths(&db, "lib-a", &[offline]).await.unwrap();
        assert_eq!(asset_count(&db, "lib-a").await, 2);

        // A file genuinely removed from a readable root is still cleaned up.
        std::fs::remove_file(lib_root.join("table.obj")).unwrap();
        index_paths(&db, "lib-a", &[root_str]).await.unwrap();
        assert_eq!(asset_count(&db, "lib-a").await, 1);

        drop(db);
        let _ = std::fs::remove_dir_all(&base);
    }

    fn write_png(path: &Path) {
        image::RgbImage::from_pixel(8, 8, image::Rgb([200, 40, 40])).save(path).unwrap();
    }

    async fn job_count(pool: &sqlx::Pool<sqlx::Sqlite>, file_name: &str) -> i64 {
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM preview_jobs p JOIN assets a ON a.id = p.asset_id WHERE a.file_name = ?",
        )
        .bind(file_name)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    async fn status_of(pool: &sqlx::Pool<sqlx::Sqlite>, file_name: &str) -> String {
        sqlx::query_scalar("SELECT preview_status FROM assets WHERE file_name = ?")
            .bind(file_name)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    /// Reproduces a library indexed by a build that didn't preview images yet: rows saved
    /// as 'none' with no job, and unchanged files skipped by every later index pass.
    #[tokio::test]
    async fn stuck_none_previews_are_backfilled_once() {
        let base = scratch_dir("backfill");
        let lib_root = base.join("lib");
        std::fs::create_dir_all(&lib_root).unwrap();
        for name in ["stuck.png", "failed.png", "done.png"] {
            write_png(&lib_root.join(name));
        }
        std::fs::write(lib_root.join("notes.txt"), b"hi").unwrap();
        std::fs::write(lib_root.join("model.obj"), b"v 0 0 0\n").unwrap();

        let db = Database::new(base.join("master.sqlite").to_str().unwrap()).await.unwrap();
        db.register_library("lib", "L", &lib_root).await.unwrap();
        let root = lib_root.to_string_lossy().to_string();
        index_paths(&db, "lib", std::slice::from_ref(&root)).await.unwrap();
        let pool = db.get_library_handle_exact("lib").await.unwrap().pool;

        // Rewind to the old build's state.
        sqlx::query("DELETE FROM preview_jobs").execute(&pool).await.unwrap();
        sqlx::query("UPDATE assets SET preview_status = 'none', thumbnail_path = NULL")
            .execute(&pool).await.unwrap();
        sqlx::query("UPDATE assets SET preview_status = 'error' WHERE file_name = 'failed.png'")
            .execute(&pool).await.unwrap();
        sqlx::query("UPDATE assets SET preview_status = 'done', thumbnail_path = '.opendam/thumbs/x.png' WHERE file_name = 'done.png'")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO preview_jobs (id, asset_id, status, created_at) VALUES ('orphan', 'gone', 'queued', 0)")
            .execute(&pool).await.unwrap();

        for _ in 0..2 {
            index_paths(&db, "lib", std::slice::from_ref(&root)).await.unwrap();
            assert_eq!(job_count(&pool, "stuck.png").await, 1);
            assert_eq!(status_of(&pool, "stuck.png").await, "pending");
        }
        assert_eq!(job_count(&pool, "failed.png").await, 0);
        assert_eq!(status_of(&pool, "failed.png").await, "error");
        assert_eq!(job_count(&pool, "done.png").await, 0);
        assert_eq!(status_of(&pool, "done.png").await, "done");
        assert_eq!(job_count(&pool, "notes.txt").await, 0);
        // Python-only formats are not backfilled (no worker in installed builds).
        assert_eq!(job_count(&pool, "model.obj").await, 0);
        assert_eq!(status_of(&pool, "model.obj").await, "none");
        let orphans: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM preview_jobs WHERE id = 'orphan'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(orphans, 0);

        // With a Python worker available, Python formats are backfilled too (once).
        for _ in 0..2 {
            backfill_missing_previews(&pool, 0, true).await.unwrap();
            assert_eq!(job_count(&pool, "model.obj").await, 1);
            assert_eq!(status_of(&pool, "model.obj").await, "pending");
        }
        assert_eq!(job_count(&pool, "notes.txt").await, 0);
        assert_eq!(status_of(&pool, "failed.png").await, "error");

        // The queued job is one the native worker path can complete.
        let thumb = base.join("stuck_thumb.png");
        crate::preview::generate_image_thumb(&lib_root.join("stuck.png"), &thumb).unwrap();
        assert!(thumb.exists());

        drop(pool);
        drop(db);
        let _ = std::fs::remove_dir_all(&base);
    }
}
