use std::sync::Arc;
use std::path::Path;
use tauri::State;
use serde_json;

use crate::AppState;
use crate::models::{Asset, SearchPayload, SearchResult, AssetPatch};
use crate::db::{resolve_path, make_relative_path};

type Result<T> = std::result::Result<T, String>;

/// Build an Asset from a raw SQLite row, resolving relative paths to absolute paths
fn row_to_asset(
    root: Option<&Path>,
    id: String, file_path: String, file_name: String, extension: String,
    kind: String, size_bytes: i64, modified_at: i64, indexed_at: i64,
    rating: i64, color_label: String, description: String,
    thumbnail_path: Option<String>, preview_status: String,
    metadata_json: String, tags_json: Option<String>,
    collections_json: Option<String>,
) -> Asset {
    let metadata = serde_json::from_str(&metadata_json).unwrap_or_default();
    let tags: Vec<String> = tags_json
        .and_then(|j| serde_json::from_str(&j).ok())
        .unwrap_or_default();
    let collections: Vec<String> = collections_json
        .and_then(|j| serde_json::from_str(&j).ok())
        .unwrap_or_default();

    let resolved_file_path = match root {
        Some(r) => resolve_path(r, &file_path),
        None => file_path,
    };
    let resolved_thumb_path = match (root, thumbnail_path) {
        (Some(r), Some(t)) => Some(resolve_path(r, &t)),
        (_, t) => t,
    };

    Asset {
        id, file_path: resolved_file_path, file_name, extension, kind, size_bytes,
        modified_at, indexed_at, rating, color_label, description,
        tags, collections, thumbnail_path: resolved_thumb_path, preview_status, metadata,
    }
}

#[tauri::command]
pub async fn search_assets(
    payload: SearchPayload,
    state: State<'_, Arc<AppState>>,
) -> Result<SearchResult> {
    let handle_opt = state.db.get_library_handle(payload.query.library_id.as_deref()).await;
    let (pool, root_opt) = match &handle_opt {
        Some(h) => (h.pool.clone(), Some(h.root_path.clone())),
        None => (state.db.get_active_pool().await, None),
    };

    let offset = payload.page * payload.page_size;
    let limit = payload.page_size;

    let has_fts = !payload.query.text.trim().is_empty();
    let f = &payload.query.filter;

    let mut conditions: Vec<String> = Vec::new();
    let mut binds: Vec<String> = Vec::new();

    if has_fts {
        conditions.push(
            "a.id IN (SELECT id FROM assets_fts WHERE assets_fts MATCH ?)".into(),
        );
        binds.push(format!("{}*", payload.query.text.trim()));
    }

    if let Some(kinds) = &f.kinds {
        if !kinds.is_empty() {
            let placeholders = kinds.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            conditions.push(format!("a.kind IN ({})", placeholders));
            binds.extend(kinds.clone());
        }
    }

    if let Some(color_labels) = &f.color_labels {
        if !color_labels.is_empty() {
            let placeholders = color_labels.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            conditions.push(format!("a.color_label IN ({})", placeholders));
            binds.extend(color_labels.clone());
        }
    }

    if let Some(rating) = &f.rating {
        conditions.push("a.rating >= ? AND a.rating <= ?".into());
        binds.push(rating.min.to_string());
        binds.push(rating.max.to_string());
    }

    if let Some(exts) = &f.extensions {
        if !exts.is_empty() {
            let placeholders = exts.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            conditions.push(format!("a.extension IN ({})", placeholders));
            binds.extend(exts.clone());
        }
    }

    // Tag filter: supports logic "and" (has all tags), "or" (has any tag), "and_or" (has primary tag AND any of remaining), "none" (untagged)
    let tag_logic = f.tag_logic.as_deref().unwrap_or("or");
    if tag_logic == "none" {
        conditions.push("a.id NOT IN (SELECT asset_id FROM asset_tags)".into());
    } else if let Some(tags) = &f.tags {
        if !tags.is_empty() {
            if tag_logic == "and" {
                let placeholders = tags.iter().map(|_| "?").collect::<Vec<_>>().join(",");
                conditions.push(format!(
                    "a.id IN (SELECT asset_id FROM asset_tags WHERE tag_id IN ({}) GROUP BY asset_id HAVING COUNT(DISTINCT tag_id) = ?)",
                    placeholders
                ));
                binds.extend(tags.clone());
                binds.push(tags.len().to_string());
            } else if tag_logic == "and_or" {
                if tags.len() <= 1 {
                    conditions.push("a.id IN (SELECT asset_id FROM asset_tags WHERE tag_id = ?)".into());
                    binds.push(tags[0].clone());
                } else {
                    let primary_tag = &tags[0];
                    let remaining_tags = &tags[1..];
                    let placeholders = remaining_tags.iter().map(|_| "?").collect::<Vec<_>>().join(",");
                    conditions.push(format!(
                        "a.id IN (SELECT asset_id FROM asset_tags WHERE tag_id = ?) AND a.id IN (SELECT asset_id FROM asset_tags WHERE tag_id IN ({}))",
                        placeholders
                    ));
                    binds.push(primary_tag.clone());
                    binds.extend(remaining_tags.iter().cloned());
                }
            } else {
                let placeholders = tags.iter().map(|_| "?").collect::<Vec<_>>().join(",");
                conditions.push(format!(
                    "a.id IN (SELECT asset_id FROM asset_tags WHERE tag_id IN ({}))",
                    placeholders
                ));
                binds.extend(tags.clone());
            }
        }
    }

    // Collection filter: supports multi-collection, subcollections, logic "and" (in all), "or" (in any), "and_or" (primary AND any remaining), "none" (unorganized)
    let mut target_collections: Vec<String> = Vec::new();
    if let Some(col_id) = payload.query.collection_id.as_ref().or(f.collection_id.as_ref()) {
        if !col_id.is_empty() && !target_collections.contains(col_id) {
            target_collections.push(col_id.clone());
        }
    }
    if let Some(cols) = &f.collection_ids {
        for cid in cols {
            if !cid.is_empty() && !target_collections.contains(cid) {
                target_collections.push(cid.clone());
            }
        }
    }

    let col_logic = f.collection_logic.as_deref().unwrap_or("or");
    let include_sub = f.include_subcollections.unwrap_or_else(|| payload.query.include_subcollections.unwrap_or(true));

    if col_logic == "none" || f.unorganized == Some(true) {
        conditions.push("a.id NOT IN (SELECT asset_id FROM collection_assets)".into());
    } else if !target_collections.is_empty() {
        if col_logic == "and" {
            for cid in &target_collections {
                if include_sub {
                    conditions.push(
                        "a.id IN (
                            WITH RECURSIVE col_tree AS (
                                SELECT id FROM collections WHERE id = ?
                                UNION ALL
                                SELECT c.id FROM collections c JOIN col_tree ct ON c.parent_id = ct.id
                            )
                            SELECT asset_id FROM collection_assets WHERE collection_id IN (SELECT id FROM col_tree)
                        )".into()
                    );
                } else {
                    conditions.push(
                        "a.id IN (SELECT asset_id FROM collection_assets WHERE collection_id = ?)".into()
                    );
                }
                binds.push(cid.clone());
            }
        } else if col_logic == "and_or" {
            if target_collections.len() <= 1 {
                let cid = &target_collections[0];
                if include_sub {
                    conditions.push(
                        "a.id IN (
                            WITH RECURSIVE col_tree AS (
                                SELECT id FROM collections WHERE id = ?
                                UNION ALL
                                SELECT c.id FROM collections c JOIN col_tree ct ON c.parent_id = ct.id
                            )
                            SELECT asset_id FROM collection_assets WHERE collection_id IN (SELECT id FROM col_tree)
                        )".into()
                    );
                } else {
                    conditions.push(
                        "a.id IN (SELECT asset_id FROM collection_assets WHERE collection_id = ?)".into()
                    );
                }
                binds.push(cid.clone());
            } else {
                let primary_col = &target_collections[0];
                let remaining_cols = &target_collections[1..];
                if include_sub {
                    conditions.push(
                        "a.id IN (
                            WITH RECURSIVE col_tree AS (
                                SELECT id FROM collections WHERE id = ?
                                UNION ALL
                                SELECT c.id FROM collections c JOIN col_tree ct ON c.parent_id = ct.id
                            )
                            SELECT asset_id FROM collection_assets WHERE collection_id IN (SELECT id FROM col_tree)
                        )".into()
                    );
                    binds.push(primary_col.clone());

                    let rem_placeholders = remaining_cols.iter().map(|_| "?").collect::<Vec<_>>().join(",");
                    conditions.push(format!(
                        "a.id IN (
                            WITH RECURSIVE col_tree AS (
                                SELECT id FROM collections WHERE id IN ({})
                                UNION ALL
                                SELECT c.id FROM collections c JOIN col_tree ct ON c.parent_id = ct.id
                            )
                            SELECT asset_id FROM collection_assets WHERE collection_id IN (SELECT id FROM col_tree)
                        )",
                        rem_placeholders
                    ));
                    binds.extend(remaining_cols.iter().cloned());
                } else {
                    conditions.push(
                        "a.id IN (SELECT asset_id FROM collection_assets WHERE collection_id = ?)".into()
                    );
                    binds.push(primary_col.clone());

                    let rem_placeholders = remaining_cols.iter().map(|_| "?").collect::<Vec<_>>().join(",");
                    conditions.push(format!(
                        "a.id IN (SELECT asset_id FROM collection_assets WHERE collection_id IN ({}))",
                        rem_placeholders
                    ));
                    binds.extend(remaining_cols.iter().cloned());
                }
            }
        } else {
            if include_sub {
                let placeholders = target_collections.iter().map(|_| "?").collect::<Vec<_>>().join(",");
                conditions.push(format!(
                    "a.id IN (
                        WITH RECURSIVE col_tree AS (
                            SELECT id FROM collections WHERE id IN ({})
                            UNION ALL
                            SELECT c.id FROM collections c JOIN col_tree ct ON c.parent_id = ct.id
                        )
                        SELECT asset_id FROM collection_assets WHERE collection_id IN (SELECT id FROM col_tree)
                    )",
                    placeholders
                ));
                binds.extend(target_collections);
            } else {
                let placeholders = target_collections.iter().map(|_| "?").collect::<Vec<_>>().join(",");
                conditions.push(format!(
                    "a.id IN (SELECT asset_id FROM collection_assets WHERE collection_id IN ({}))",
                    placeholders
                ));
                binds.extend(target_collections);
            }
        }
    }

    // Smart filter conditions
    if let Some(dir) = &f.directory {
        if !dir.trim().is_empty() {
            conditions.push("a.file_path LIKE ?".into());
            binds.push(format!("%{}%", dir.trim().replace('\\', "/")));
        }
    }

    if let Some(prefix) = &f.name_prefix {
        if !prefix.trim().is_empty() {
            conditions.push("a.file_name LIKE ?".into());
            binds.push(format!("{}%", prefix.trim()));
        }
    }

    if let Some(suffix) = &f.name_suffix {
        if !suffix.trim().is_empty() {
            conditions.push("a.file_name LIKE ?".into());
            binds.push(format!("%{}", suffix.trim()));
        }
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let sort_col = match payload.query.sort.field.as_str() {
        "name"   => "a.file_name",
        "size"   => "a.size_bytes",
        "rating" => "a.rating",
        "kind"   => "a.kind",
        _        => "a.modified_at",
    };
    let sort_dir = if payload.query.sort.order == "asc" { "ASC" } else { "DESC" };

    let count_sql = format!(
        "SELECT COUNT(*) FROM assets a {}",
        where_clause
    );
    let data_sql = format!(
        r#"SELECT
            a.id, a.file_path, a.file_name, a.extension, a.kind,
            a.size_bytes, a.modified_at, a.indexed_at, a.rating,
            a.color_label, a.description, a.thumbnail_path,
            a.preview_status, a.metadata,
            (SELECT json_group_array(tag_id) FROM asset_tags WHERE asset_id = a.id) AS tags,
            (SELECT json_group_array(collection_id) FROM collection_assets WHERE asset_id = a.id) AS collections
           FROM assets a
           {}
           ORDER BY {} {}
           LIMIT ? OFFSET ?"#,
        where_clause, sort_col, sort_dir
    );

    // Execute count
    let mut count_query = sqlx::query_scalar::<_, i64>(&count_sql);
    for b in &binds {
        count_query = count_query.bind(b.clone());
    }
    let total = count_query.fetch_one(&pool).await.map_err(|e| e.to_string())?;

    // Execute data fetch
    let mut data_query = sqlx::query_as::<_, (
        String, String, String, String, String,
        i64, i64, i64, i64,
        String, String, Option<String>,
        String, String,
        Option<String>,
        Option<String>,
    )>(&data_sql);
    for b in &binds {
        data_query = data_query.bind(b.clone());
    }
    data_query = data_query.bind(limit).bind(offset);

    let rows = data_query.fetch_all(&pool).await.map_err(|e| e.to_string())?;

    let root_ref = root_opt.as_deref();
    let assets = rows
        .into_iter()
        .map(|(id, fp, fn_, ext, kind, sb, ma, ia, r, cl, desc, tp, ps, meta, tags, cols)| {
            row_to_asset(root_ref, id, fp, fn_, ext, kind, sb, ma, ia, r, cl, desc, tp, ps, meta, tags, cols)
        })
        .collect();

    Ok(SearchResult {
        assets,
        total,
        page: payload.page,
        page_size: payload.page_size,
    })
}

#[tauri::command]
pub async fn get_asset(
    id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<Asset> {
    let libs = state.db.get_all_libraries().await;

    // Search across open libraries
    for lib in libs {
        let row = sqlx::query_as::<_, (
            String, String, String, String, String,
            i64, i64, i64, i64,
            String, String, Option<String>,
            String, String,
            Option<String>,
            Option<String>,
        )>(
            r#"SELECT a.id, a.file_path, a.file_name, a.extension, a.kind,
               a.size_bytes, a.modified_at, a.indexed_at, a.rating,
               a.color_label, a.description, a.thumbnail_path,
               a.preview_status, a.metadata,
               (SELECT json_group_array(tag_id) FROM asset_tags WHERE asset_id = a.id) AS tags,
               (SELECT json_group_array(collection_id) FROM collection_assets WHERE asset_id = a.id) AS collections
               FROM assets a WHERE a.id = ?"#,
        )
        .bind(&id)
        .fetch_optional(&lib.pool)
        .await
        .map_err(|e| e.to_string())?;

        if let Some((id, fp, fn_, ext, kind, sb, ma, ia, r, cl, desc, tp, ps, meta, tags, cols)) = row {
            return Ok(row_to_asset(Some(&lib.root_path), id, fp, fn_, ext, kind, sb, ma, ia, r, cl, desc, tp, ps, meta, tags, cols));
        }
    }

    // Fallback to active/default pool
    let fallback_pool = state.db.get_active_pool().await;
    let row = sqlx::query_as::<_, (
        String, String, String, String, String,
        i64, i64, i64, i64,
        String, String, Option<String>,
        String, String,
        Option<String>,
        Option<String>,
    )>(
        r#"SELECT a.id, a.file_path, a.file_name, a.extension, a.kind,
           a.size_bytes, a.modified_at, a.indexed_at, a.rating,
           a.color_label, a.description, a.thumbnail_path,
           a.preview_status, a.metadata,
           (SELECT json_group_array(tag_id) FROM asset_tags WHERE asset_id = a.id) AS tags,
           (SELECT json_group_array(collection_id) FROM collection_assets WHERE asset_id = a.id) AS collections
           FROM assets a WHERE a.id = ?"#,
    )
    .bind(&id)
    .fetch_optional(&fallback_pool)
    .await
    .map_err(|e| e.to_string())?;

    match row {
        Some((id, fp, fn_, ext, kind, sb, ma, ia, r, cl, desc, tp, ps, meta, tags, cols)) => {
            let root_opt = state.db.get_library_root(None).await;
            Ok(row_to_asset(root_opt.as_deref(), id, fp, fn_, ext, kind, sb, ma, ia, r, cl, desc, tp, ps, meta, tags, cols))
        }
        None => Err(format!("Asset not found: {}", id)),
    }
}

#[tauri::command]
pub async fn update_asset(
    id: String,
    patch: AssetPatch,
    state: State<'_, Arc<AppState>>,
) -> Result<Asset> {
    let libs = state.db.get_all_libraries().await;
    let mut pools_to_update: Vec<(sqlx::Pool<sqlx::Sqlite>, Option<std::path::PathBuf>)> = Vec::new();

    for lib in &libs {
        let exists: Option<(String,)> = sqlx::query_as("SELECT id FROM assets WHERE id = ?")
            .bind(&id)
            .fetch_optional(&lib.pool)
            .await
            .unwrap_or(None);

        if exists.is_some() {
            pools_to_update.push((lib.pool.clone(), Some(lib.root_path.clone())));
        }
    }

    let master_exists: Option<(String,)> = sqlx::query_as("SELECT id FROM assets WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.db.master_pool)
        .await
        .unwrap_or(None);

    if master_exists.is_some() || pools_to_update.is_empty() {
        pools_to_update.push((state.db.master_pool.clone(), state.db.get_library_root(None).await));
    }

    let active_pool = state.db.get_active_pool().await;
    pools_to_update.push((active_pool, state.db.get_library_root(None).await));

    for (pool, root) in &pools_to_update {
        if let Some(rating) = patch.rating {
            let _ = sqlx::query("UPDATE assets SET rating = ? WHERE id = ?")
                .bind(rating).bind(&id).execute(pool).await;
        }
        if let Some(ref color_label) = patch.color_label {
            let _ = sqlx::query("UPDATE assets SET color_label = ? WHERE id = ?")
                .bind(color_label).bind(&id).execute(pool).await;
        }
        if let Some(ref description) = patch.description {
            let _ = sqlx::query("UPDATE assets SET description = ? WHERE id = ?")
                .bind(description).bind(&id).execute(pool).await;
        }
        if let Some(ref thumbnail_path) = patch.thumbnail_path {
            let rel_thumb = match root {
                Some(r) => make_relative_path(r, Path::new(thumbnail_path)),
                None => thumbnail_path.clone(),
            };
            let _ = sqlx::query("UPDATE assets SET thumbnail_path = ? WHERE id = ?")
                .bind(rel_thumb).bind(&id).execute(pool).await;
        }
        if let Some(ref preview_status) = patch.preview_status {
            let _ = sqlx::query("UPDATE assets SET preview_status = ? WHERE id = ?")
                .bind(preview_status).bind(&id).execute(pool).await;
        }
        if let Some(ref tags) = patch.tags {
            let _ = sqlx::query("DELETE FROM asset_tags WHERE asset_id = ?")
                .bind(&id).execute(pool).await;
            for tag_id in tags {
                let tag_def = sqlx::query_as::<_, (String, String, Option<String>, Option<String>)>(
                    "SELECT id, name, color, parent_id FROM tags WHERE id = ?"
                )
                .bind(tag_id)
                .fetch_optional(&state.db.master_pool).await.unwrap_or(None);

                if let Some((tid, tname, tcolor, tparent)) = tag_def {
                    let _ = sqlx::query("INSERT OR IGNORE INTO tags (id, name, color, parent_id) VALUES (?, ?, ?, ?)")
                        .bind(&tid).bind(&tname).bind(&tcolor).bind(&tparent)
                        .execute(pool).await;
                } else {
                    let _ = sqlx::query("INSERT OR IGNORE INTO tags (id, name) VALUES (?, ?)")
                        .bind(tag_id).bind(tag_id)
                        .execute(pool).await;
                }

                let _ = sqlx::query("INSERT OR IGNORE INTO asset_tags (asset_id, tag_id) VALUES (?, ?)")
                    .bind(&id).bind(tag_id).execute(pool).await;
            }
        }
    }

    get_asset(id, state).await
}
