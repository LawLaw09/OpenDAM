use std::sync::Arc;
use tauri::State;
use uuid::Uuid;
use chrono::Utc;

use crate::AppState;
use crate::models::Collection;

type Result<T> = std::result::Result<T, String>;

#[tauri::command]
pub async fn list_collections(state: State<'_, Arc<AppState>>) -> Result<Vec<Collection>> {
    let pool = &state.db.pool;
    let rows = sqlx::query_as::<_, (String, String, String, Option<String>, bool, Option<String>, i64)>(
        "SELECT id, name, description, parent_id, is_smart, filter_spec, created_at FROM collections ORDER BY created_at ASC"
    )
    .fetch_all(pool).await.map_err(|e| e.to_string())?;

    let mut collections = Vec::new();
    for (id, name, description, parent_id, is_smart, filter_spec, created_at) in rows {
        let asset_id_rows = sqlx::query_as::<_, (String,)>(
            "SELECT asset_id FROM collection_assets WHERE collection_id = ?"
        )
        .bind(&id).fetch_all(pool).await.map_err(|e| e.to_string())?;
        let asset_ids = asset_id_rows.into_iter().map(|(aid,)| aid).collect();
        collections.push(Collection { id, name, description, parent_id, asset_ids, is_smart, filter_spec, created_at });
    }
    Ok(collections)
}

#[tauri::command]
pub async fn create_collection(
    name: String,
    description: Option<String>,
    parent_id: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<Collection> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().timestamp_millis();
    let desc = description.unwrap_or_default();
    sqlx::query(
        "INSERT INTO collections (id, name, description, parent_id, is_smart, created_at) VALUES (?, ?, ?, ?, 0, ?)"
    )
    .bind(&id).bind(&name).bind(&desc).bind(&parent_id).bind(now)
    .execute(&state.db.pool).await.map_err(|e| e.to_string())?;
    Ok(Collection { id, name, description: desc, parent_id, asset_ids: vec![], is_smart: false, filter_spec: None, created_at: now })
}

#[tauri::command]
pub async fn add_to_collection(
    collection_id: String,
    asset_ids: Vec<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<()> {
    let pool = &state.db.pool;
    for aid in &asset_ids {
        sqlx::query("INSERT OR IGNORE INTO collection_assets (collection_id, asset_id) VALUES (?, ?)")
            .bind(&collection_id).bind(aid).execute(pool).await.map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn remove_from_collection(
    collection_id: String,
    asset_ids: Vec<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<()> {
    let pool = &state.db.pool;
    for aid in &asset_ids {
        sqlx::query("DELETE FROM collection_assets WHERE collection_id = ? AND asset_id = ?")
            .bind(&collection_id).bind(aid).execute(pool).await.map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn delete_collection(
    id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<()> {
    let pool = &state.db.pool;
    // Recursively collect all descendant collection IDs
    let mut to_delete = vec![id.clone()];
    let mut idx = 0;
    while idx < to_delete.len() {
        let cur_id = &to_delete[idx];
        let sub_rows = sqlx::query_as::<_, (String,)>(
            "SELECT id FROM collections WHERE parent_id = ?"
        )
        .bind(cur_id).fetch_all(pool).await.map_err(|e| e.to_string())?;

        for (sub_id,) in sub_rows {
            if !to_delete.contains(&sub_id) {
                to_delete.push(sub_id);
            }
        }
        idx += 1;
    }

    for cid in &to_delete {
        sqlx::query("DELETE FROM collection_assets WHERE collection_id = ?")
            .bind(cid).execute(pool).await.map_err(|e| e.to_string())?;
        sqlx::query("DELETE FROM collections WHERE id = ?")
            .bind(cid).execute(pool).await.map_err(|e| e.to_string())?;
    }
    Ok(())
}
