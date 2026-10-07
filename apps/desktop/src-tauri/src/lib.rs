pub mod commands;
pub mod db;
pub mod models;
pub mod watcher;
pub mod preview;

use std::sync::Arc;
use tauri::{Emitter, Manager};
use tokio::sync::Mutex;
use tracing_subscriber::EnvFilter;

use db::Database;
use watcher::LibraryWatcher;

pub struct AppState {
    pub db: Database,
    pub watcher: Mutex<LibraryWatcher>,
    pub app_handle: tauri::AppHandle,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| "opendam=debug,info".parse().unwrap()),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data directory");

            std::fs::create_dir_all(&app_data_dir)?;

            let db_path = app_data_dir.join("opendam.sqlite");

            let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
            let db = rt.block_on(async {
                Database::new(db_path.to_str().unwrap())
                    .await
                    .expect("Failed to initialize database")
            });

            let watcher = LibraryWatcher::new();
            let state = Arc::new(AppState {
                db,
                watcher: Mutex::new(watcher),
                app_handle: app.handle().clone(),
            });

            app.manage(state.clone());
            
            // Start the background preview worker queue
            crate::preview::start_preview_worker(state.clone());

            // Re-index all existing libraries on startup in background
            let startup_state = state.clone();
            tauri::async_runtime::spawn(async move {
                let rows = sqlx::query_as::<_, (String, String)>(
                    "SELECT id, root_paths FROM libraries"
                )
                .fetch_all(&startup_state.db.master_pool)
                .await;

                if let Ok(libraries) = rows {
                    for (id, root_paths_json) in libraries {
                        let paths: Vec<String> = serde_json::from_str(&root_paths_json).unwrap_or_default();
                        let _ = watcher::index_paths(&startup_state.db, &id, &paths).await;
                    }
                    let _ = startup_state.app_handle.emit("library:updated", ());
                }
            });

            // ── Cross-machine NAS change detection ───────────────────────────────
            // When another machine writes to the shared library.sqlite, its mtime
            // changes on the NAS. We poll every 30s and emit library:updated when
            // we detect a newer mtime, causing the frontend to re-query automatically.
            let poll_state = state.clone();
            tauri::async_runtime::spawn(async move {
                use std::collections::HashMap;
                use std::time::SystemTime;

                // seed: record the initial mtime for each known library DB
                let mut last_seen: HashMap<String, SystemTime> = HashMap::new();

                loop {
                    tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;

                    let libs = poll_state.db.get_all_libraries().await;
                    let mut changed = false;

                    for lib in &libs {
                        let db_path = lib.root_path.join(".opendam").join("library.sqlite");
                        if let Ok(meta) = std::fs::metadata(&db_path) {
                            if let Ok(mtime) = meta.modified() {
                                let prev = last_seen.entry(lib.id.clone()).or_insert(mtime);
                                if mtime > *prev {
                                    *prev = mtime;
                                    changed = true;
                                    tracing::debug!(
                                        "NAS poll: library '{}' DB updated by another machine",
                                        lib.name
                                    );
                                }
                            }
                        }
                    }

                    if changed {
                        let _ = poll_state.app_handle.emit("library:updated", ());
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::library::add_library,
            commands::library::list_libraries,
            commands::library::remove_library,
            commands::library::refresh_library,
            commands::library::refresh_all_libraries,
            commands::asset::search_assets,
            commands::asset::get_asset,
            commands::asset::update_asset,
            commands::asset::delete_asset,
            commands::asset::delete_assets,
            commands::asset::rename_asset,
            commands::tag::list_tags,
            commands::tag::create_tag,
            commands::tag::delete_tag,
            commands::collection::list_collections,
            commands::collection::create_collection,
            commands::collection::add_to_collection,
            commands::collection::remove_from_collection,
            commands::collection::delete_collection,
            commands::preview::request_preview,
            commands::preview::get_preview_jobs,
            commands::shell::reveal_in_explorer,
            commands::shell::open_with_default,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
