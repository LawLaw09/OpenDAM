use std::sync::Arc;
use tauri::State;
use uuid::Uuid;
use chrono::Utc;

use crate::AppState;
use crate::models::PreviewJob;

type Result<T> = std::result::Result<T, String>;

#[tauri::command]
pub async fn request_preview(
    asset_ids: Vec<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<()> {
    let pool = state.db.get_active_pool().await;
    let now = Utc::now().timestamp_millis();
    for asset_id in &asset_ids {
        // Delete any existing preview job so a new one is queued cleanly
        let _ = sqlx::query("DELETE FROM preview_jobs WHERE asset_id = ?")
            .bind(asset_id)
            .execute(&pool)
            .await;

        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO preview_jobs (id, asset_id, status, created_at) VALUES (?, ?, 'queued', ?)"
        )
        .bind(&id).bind(asset_id).bind(now)
        .execute(&pool).await.map_err(|e| e.to_string())?;

        // Mark asset as generating / pending
        sqlx::query("UPDATE assets SET preview_status = 'pending' WHERE id = ?")
            .bind(asset_id).execute(&pool).await.map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn get_preview_jobs(state: State<'_, Arc<AppState>>) -> Result<Vec<PreviewJob>> {
    let pool = state.db.get_active_pool().await;
    let rows = sqlx::query_as::<_, (String, String, String, Option<i64>, Option<String>, i64)>(
        "SELECT id, asset_id, status, progress, error_message, created_at FROM preview_jobs ORDER BY created_at DESC LIMIT 100"
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows.into_iter().map(|(id, asset_id, status, progress, error_message, created_at)| {
        PreviewJob { id, asset_id, status, progress, error_message, created_at }
    }).collect())
}
