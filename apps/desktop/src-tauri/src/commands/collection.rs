use std::sync::Arc;
use tauri::State;
use uuid::Uuid;
use chrono::Utc;

use crate::AppState;
use crate::models::Collection;

type Result<T> = std::result::Result<T, String>;

#[tauri::command]
pub async fn list_collections(state: State<'_, Arc<AppState>>) -> Result<Vec<Collection>> {
    let pool = state.db.get_active_pool().await;
    let mut rows = sqlx::query_as::<_, (String, String, String, Option<String>, bool, Option<String>, i64)>(
        "SELECT id, name, description, parent_id, is_smart, filter_spec, created_at FROM collections ORDER BY created_at ASC"
    )
    .fetch_all(&state.db.master_pool).await.unwrap_or_default();

    if rows.is_empty() {
        rows = sqlx::query_as::<_, (String, String, String, Option<String>, bool, Option<String>, i64)>(
            "SELECT id, name, description, parent_id, is_smart, filter_spec, created_at FROM collections ORDER BY created_at ASC"
        )
        .fetch_all(&pool).await.unwrap_or_default();
    }

    let libs = state.db.get_all_libraries().await;
    let mut collections = Vec::new();
    for (id, name, description, parent_id, is_smart, filter_spec, created_at) in rows {
        let mut asset_ids_set = std::collections::HashSet::new();

        // Gather asset_ids from all open libraries
        for lib in &libs {
            if let Ok(a_rows) = sqlx::query_as::<_, (String,)>(
                "SELECT asset_id FROM collection_assets WHERE collection_id = ?"
            )
            .bind(&id).fetch_all(&lib.pool).await {
                for (aid,) in a_rows {
                    asset_ids_set.insert(aid);
                }
            }
        }
        if let Ok(a_rows) = sqlx::query_as::<_, (String,)>(
            "SELECT asset_id FROM collection_assets WHERE collection_id = ?"
        )
        .bind(&id).fetch_all(&state.db.master_pool).await {
            for (aid,) in a_rows {
                asset_ids_set.insert(aid);
            }
        }

        let asset_ids = asset_ids_set.into_iter().collect();
        collections.push(Collection { id, name, description, parent_id, asset_ids, is_smart, filter_spec, created_at });
    }
    Ok(collections)
}

#[tauri::command]
pub async fn create_collection(
    name: String,
    description: Option<String>,
    parent_id: Option<String>,
    is_smart: Option<bool>,
    filter_spec: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<Collection> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().timestamp_millis();
    let desc = description.unwrap_or_default();
    let smart = is_smart.unwrap_or(false);
    let pool = state.db.get_active_pool().await;
    sqlx::query(
        "INSERT INTO collections (id, name, description, parent_id, is_smart, filter_spec, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&id).bind(&name).bind(&desc).bind(&parent_id).bind(smart).bind(&filter_spec).bind(now)
    .execute(&pool).await.map_err(|e| e.to_string())?;

    // Also sync to all open library pools and master pool
    let libs = state.db.get_all_libraries().await;
    for lib in libs {
        let _ = sqlx::query(
            "INSERT OR IGNORE INTO collections (id, name, description, parent_id, is_smart, filter_spec, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&id).bind(&name).bind(&desc).bind(&parent_id).bind(smart).bind(&filter_spec).bind(now)
        .execute(&lib.pool).await;
    }
    let _ = sqlx::query(
        "INSERT OR IGNORE INTO collections (id, name, description, parent_id, is_smart, filter_spec, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&id).bind(&name).bind(&desc).bind(&parent_id).bind(smart).bind(&filter_spec).bind(now)
    .execute(&state.db.master_pool).await;

    Ok(Collection { id, name, description: desc, parent_id, asset_ids: vec![], is_smart: smart, filter_spec, created_at: now })
}

#[tauri::command]
pub async fn add_to_collection(
    collection_id: String,
    asset_ids: Vec<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<()> {
    let libs = state.db.get_all_libraries().await;

    // Fetch collection info from master_pool or active pool
    let col_info = sqlx::query_as::<_, (String, String, String, Option<String>, bool, Option<String>, i64)>(
        "SELECT id, name, description, parent_id, is_smart, filter_spec, created_at FROM collections WHERE id = ?"
    )
    .bind(&collection_id)
    .fetch_optional(&state.db.master_pool)
    .await
    .unwrap_or(None);

    for aid in &asset_ids {
        let mut target_pool: Option<sqlx::Pool<sqlx::Sqlite>> = None;
        for lib in &libs {
            let exists: Option<(String,)> = sqlx::query_as("SELECT id FROM assets WHERE id = ?")
                .bind(aid)
                .fetch_optional(&lib.pool)
                .await
                .unwrap_or(None);
            if exists.is_some() {
                target_pool = Some(lib.pool.clone());
                break;
            }
        }

        let pool = target_pool.unwrap_or_else(|| state.db.pool.clone());

        // Ensure collection exists in target pool before creating foreign key link
        if let Some((ref cid, ref cname, ref cdesc, ref cpid, csmart, ref cfspec, ctime)) = col_info {
            let _ = sqlx::query(
                "INSERT OR IGNORE INTO collections (id, name, description, parent_id, is_smart, filter_spec, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(cid).bind(cname).bind(cdesc).bind(cpid).bind(csmart).bind(cfspec).bind(ctime)
            .execute(&pool).await;
        }

        let _ = sqlx::query("INSERT OR IGNORE INTO collection_assets (collection_id, asset_id) VALUES (?, ?)")
            .bind(&collection_id)
            .bind(aid)
            .execute(&pool)
            .await;
    }

    Ok(())
}

#[tauri::command]
pub async fn remove_from_collection(
    collection_id: String,
    asset_ids: Vec<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<()> {
    let libs = state.db.get_all_libraries().await;
    for lib in libs {
        for aid in &asset_ids {
            let _ = sqlx::query("DELETE FROM collection_assets WHERE collection_id = ? AND asset_id = ?")
                .bind(&collection_id)
                .bind(aid)
                .execute(&lib.pool)
                .await;
        }
    }
    for aid in &asset_ids {
        let _ = sqlx::query("DELETE FROM collection_assets WHERE collection_id = ? AND asset_id = ?")
            .bind(&collection_id)
            .bind(aid)
            .execute(&state.db.master_pool)
            .await;
    }
    Ok(())
}

#[tauri::command]
pub async fn delete_collection(
    id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<()> {
    let pool = state.db.get_active_pool().await;
    // Recursively collect all descendant collection IDs
    let mut to_delete = vec![id.clone()];
    let mut idx = 0;
    while idx < to_delete.len() {
        let cur_id = &to_delete[idx];
        let sub_rows = sqlx::query_as::<_, (String,)>(
            "SELECT id FROM collections WHERE parent_id = ?"
        )
        .bind(cur_id).fetch_all(&pool).await.map_err(|e| e.to_string())?;

        for (sub_id,) in sub_rows {
            if !to_delete.contains(&sub_id) {
                to_delete.push(sub_id);
            }
        }
        idx += 1;
    }

    let libs = state.db.get_all_libraries().await;
    for cid in &to_delete {
        let _ = sqlx::query("DELETE FROM collection_assets WHERE collection_id = ?").bind(cid).execute(&pool).await;
        let _ = sqlx::query("DELETE FROM collections WHERE id = ?").bind(cid).execute(&pool).await;
        for lib in &libs {
            let _ = sqlx::query("DELETE FROM collection_assets WHERE collection_id = ?").bind(cid).execute(&lib.pool).await;
            let _ = sqlx::query("DELETE FROM collections WHERE id = ?").bind(cid).execute(&lib.pool).await;
        }
        let _ = sqlx::query("DELETE FROM collection_assets WHERE collection_id = ?").bind(cid).execute(&state.db.master_pool).await;
        let _ = sqlx::query("DELETE FROM collections WHERE id = ?").bind(cid).execute(&state.db.master_pool).await;
    }
    Ok(())
}
