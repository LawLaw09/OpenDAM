use std::sync::Arc;
use tauri::State;
use uuid::Uuid;

use crate::AppState;
use crate::models::Tag;

type Result<T> = std::result::Result<T, String>;

#[tauri::command]
pub async fn list_tags(state: State<'_, Arc<AppState>>) -> Result<Vec<Tag>> {
    let rows = sqlx::query_as::<_, (String, String, Option<String>, Option<String>)>(
        "SELECT id, name, color, parent_id FROM tags ORDER BY name ASC"
    )
    .fetch_all(&state.db.pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows.into_iter().map(|(id, name, color, parent_id)| Tag { id, name, color, parent_id }).collect())
}

#[tauri::command]
pub async fn create_tag(
    name: String,
    parent_id: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<Tag> {
    let id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO tags (id, name, parent_id) VALUES (?, ?, ?)")
        .bind(&id).bind(&name).bind(&parent_id)
        .execute(&state.db.pool).await.map_err(|e| e.to_string())?;
    Ok(Tag { id, name, color: None, parent_id })
}

#[tauri::command]
pub async fn delete_tag(id: String, state: State<'_, Arc<AppState>>) -> Result<()> {
    sqlx::query("DELETE FROM tags WHERE id = ?")
        .bind(&id).execute(&state.db.pool).await.map_err(|e| e.to_string())?;
    Ok(())
}
