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
    let libs = state.db.get_all_libraries().await;
    let now = Utc::now().timestamp_millis();

    for asset_id in &asset_ids {
        let mut target_pool = None;
        for lib in &libs {
            let exists: Option<(String,)> = sqlx::query_as("SELECT id FROM assets WHERE id = ?")
                .bind(asset_id)
                .fetch_optional(&lib.pool)
                .await
                .unwrap_or(None);

            if exists.is_some() {
                target_pool = Some(lib.pool.clone());
                break;
            }
        }

        let pool = match target_pool {
            Some(p) => p,
            None => state.db.get_active_pool().await,
        };

        // Delete any existing preview job so a new one is queued cleanly
        let _ = sqlx::query("DELETE FROM preview_jobs WHERE asset_id = ?")
            .bind(asset_id)
            .execute(&pool)
            .await;

        let id = Uuid::new_v4().to_string();
        let _ = sqlx::query(
            "INSERT INTO preview_jobs (id, asset_id, status, created_at) VALUES (?, ?, 'queued', ?)"
        )
        .bind(&id).bind(asset_id).bind(now)
        .execute(&pool).await;

        // Mark asset as generating / pending
        let _ = sqlx::query("UPDATE assets SET preview_status = 'pending' WHERE id = ?")
            .bind(asset_id).execute(&pool).await;
    }
    Ok(())
}

#[tauri::command]
pub async fn get_preview_jobs(state: State<'_, Arc<AppState>>) -> Result<Vec<PreviewJob>> {
    let libs = state.db.get_all_libraries().await;
    let mut all_jobs = Vec::new();

    for lib in &libs {
        if let Ok(rows) = sqlx::query_as::<_, (String, String, String, Option<i64>, Option<String>, i64)>(
            "SELECT id, asset_id, status, progress, error_message, created_at FROM preview_jobs ORDER BY created_at DESC LIMIT 50"
        )
        .fetch_all(&lib.pool)
        .await {
            for (id, asset_id, status, progress, error_message, created_at) in rows {
                all_jobs.push(PreviewJob { id, asset_id, status, progress, error_message, created_at });
            }
        }
    }

    all_jobs.sort_by_key(|b| std::cmp::Reverse(b.created_at));
    all_jobs.truncate(100);
    Ok(all_jobs)
}
