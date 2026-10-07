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
    let handle_opt = db.get_library_handle(Some(library_id)).await;
    let (pool, root_path) = match handle_opt {
        Some(h) => (h.pool, h.root_path),
        None => {
            let p = PathBuf::from(paths.first().map(|s| s.as_str()).unwrap_or("."));
            (db.pool.clone(), p)
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

    // ── 2. Walk directory tree ──────────────────────────────────────────
    for root in paths {
        let root_dir = Path::new(root);
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

        for entry in walker.filter_map(|e| e.ok()).filter(|e| e.file_type().is_file()) {
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
                let needs_preview = matches!(
                    ext.as_str(),
                    "max" | "fbx" | "obj" | "gltf" | "glb" | "dae" | "stl" | "3ds" | "skp" | "rfa" | "rvt" | "dwg" | "3dm" | "vrmat" | "psd" | "psb" | "ai" | "exr" | "hdr"
                    | "jpg" | "jpeg" | "png" | "tif" | "tiff" | "tga" | "bmp" | "webp"
                );
                if needs_preview {
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
        .filter(|(rel, _)| !seen_paths.contains(rel))
        .map(|(_, (id, _, _, _))| id)
        .collect();

    if !to_delete.is_empty() {
        for chunk in to_delete.chunks(200) {
            let placeholders = chunk.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let sql = format!("DELETE FROM assets WHERE id IN ({})", placeholders);
            let mut query = sqlx::query(&sql);
            for id in chunk {
                query = query.bind(id);
            }
            let _ = query.execute(&pool).await;
        }
    }

    // ── 4. Update asset count ───────────────────────────────────────────
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
