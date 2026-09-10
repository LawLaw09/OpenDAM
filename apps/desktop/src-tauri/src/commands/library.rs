use std::sync::Arc;
use std::path::PathBuf;
use tauri::{State, Emitter};
use uuid::Uuid;
use chrono::Utc;
use serde_json;

use crate::AppState;
use crate::models::{Library, AddLibraryPayload};
use crate::watcher;

type Result<T> = std::result::Result<T, String>;

#[tauri::command]
pub async fn add_library(
    payload: AddLibraryPayload,
    state: State<'_, Arc<AppState>>,
) -> Result<Library> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().timestamp_millis();
    let root_paths_json = serde_json::to_string(&payload.root_paths).map_err(|e| e.to_string())?;

    let first_path = payload.root_paths.first().ok_or("No root path provided")?;
    let root_path = PathBuf::from(first_path);

    // Register library in memory & open its .opendam/library.sqlite with NAS pragmas
    let _handle = state.db.register_library(&id, &payload.name, &root_path)
        .await
        .map_err(|e| e.to_string())?;

    // Record in master database
    sqlx::query(
        "INSERT INTO libraries (id, name, root_paths, created_at, asset_count) VALUES (?, ?, ?, ?, 0)"
    )
    .bind(&id)
    .bind(&payload.name)
    .bind(&root_paths_json)
    .bind(now)
    .execute(&state.db.master_pool)
    .await
    .map_err(|e| e.to_string())?;

    // Kick off background NAS-efficient indexing
    let state_clone = Arc::clone(&state);
    let paths = payload.root_paths.clone();
    let lib_id = id.clone();
    tokio::spawn(async move {
        if let Err(e) = watcher::index_paths(&state_clone.db, &lib_id, &paths).await {
            tracing::error!("Failed to index library {}: {}", lib_id, e);
        } else {
            let _ = state_clone.app_handle.emit("library:updated", ());
        }
    });

    Ok(Library {
        id,
        name: payload.name,
        root_paths: payload.root_paths,
        created_at: now,
        asset_count: 0,
    })
}

#[tauri::command]
pub async fn list_libraries(state: State<'_, Arc<AppState>>) -> Result<Vec<Library>> {
    let rows = sqlx::query_as::<_, (String, String, String, i64, i64)>(
        "SELECT id, name, root_paths, created_at, asset_count FROM libraries ORDER BY created_at ASC"
    )
    .fetch_all(&state.db.master_pool)
    .await
    .map_err(|e| e.to_string())?;

    rows.into_iter()
        .map(|(id, name, root_paths_json, created_at, asset_count)| {
            let root_paths: Vec<String> =
                serde_json::from_str(&root_paths_json).unwrap_or_default();
            Ok(Library { id, name, root_paths, created_at, asset_count })
        })
        .collect()
}

#[tauri::command]
pub async fn remove_library(
    id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<()> {
    let _ = state.db.unregister_library(&id).await;

    sqlx::query("DELETE FROM libraries WHERE id = ?")
        .bind(&id)
        .execute(&state.db.master_pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn refresh_library(
    id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<()> {
    let row = sqlx::query_as::<_, (String,)>(
        "SELECT root_paths FROM libraries WHERE id = ?"
    )
    .bind(&id)
    .fetch_optional(&state.db.master_pool)
    .await
    .map_err(|e| e.to_string())?;

    if let Some((root_paths_json,)) = row {
        let paths: Vec<String> = serde_json::from_str(&root_paths_json).unwrap_or_default();
        watcher::index_paths(&state.db, &id, &paths).await.map_err(|e| e.to_string())?;
        let _ = state.app_handle.emit("library:updated", ());
    }
    Ok(())
}

#[tauri::command]
pub async fn refresh_all_libraries(
    state: State<'_, Arc<AppState>>,
) -> Result<()> {
    let rows = sqlx::query_as::<_, (String, String)>(
        "SELECT id, root_paths FROM libraries"
    )
    .fetch_all(&state.db.master_pool)
    .await
    .map_err(|e| e.to_string())?;

    for (id, root_paths_json) in rows {
        let paths: Vec<String> = serde_json::from_str(&root_paths_json).unwrap_or_default();
        let _ = watcher::index_paths(&state.db, &id, &paths).await;
    }

    let _ = state.app_handle.emit("library:updated", ());
    Ok(())
}
