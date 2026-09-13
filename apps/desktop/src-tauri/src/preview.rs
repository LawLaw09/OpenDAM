use anyhow::Result;
use std::path::{Path, PathBuf};
use image::imageops::FilterType;
use std::sync::Arc;
use tokio::process::Command;
use tokio::io::{AsyncWriteExt, AsyncBufReadExt, BufReader};
use serde_json::json;
use tauri::Emitter;
use crate::AppState;
use crate::db::{make_relative_path, resolve_path};

pub const THUMB_SIZE: u32 = 1024;

pub fn generate_image_thumb(source: &Path, output: &Path) -> Result<()> {
    let img = image::open(source)?;
    let thumb = img.resize(THUMB_SIZE, THUMB_SIZE, FilterType::Lanczos3);
    thumb.save(output)?;
    Ok(())
}

pub fn is_native_image(ext: &str) -> bool {
    matches!(
        ext.to_lowercase().as_str(),
        "jpg" | "jpeg" | "png" | "bmp" | "gif" | "tif" | "tiff" | "webp"
    )
}

pub fn start_preview_worker(state: Arc<AppState>) {
    tauri::async_runtime::spawn(async move {
        // Dynamically resolve python worker and script paths
        let current_dir = std::env::current_dir().unwrap_or_default();
        let py_exec = if let Ok(custom) = std::env::var("OPENDAM_PYTHON") {
            PathBuf::from(custom)
        } else {
            let candidates = [
                current_dir.join("packages").join("preview-engine").join("venv").join("Scripts").join("python.exe"),
                current_dir.join("packages").join("preview-engine").join("venv").join("bin").join("python"),
                PathBuf::from("python"),
            ];
            candidates.into_iter().find(|p| p.exists()).unwrap_or_else(|| PathBuf::from("python"))
        };

        let worker_py = if let Ok(custom) = std::env::var("OPENDAM_WORKER") {
            PathBuf::from(custom)
        } else {
            let candidate = current_dir.join("packages").join("preview-engine").join("worker.py");
            if candidate.exists() {
                candidate
            } else if let Ok(exe_dir) = std::env::current_exe().map(|p| p.parent().unwrap_or(&current_dir).to_path_buf()) {
                exe_dir.join("worker.py")
            } else {
                PathBuf::from("worker.py")
            }
        };

        loop {
            // Get all currently open libraries
            let libs = state.db.get_all_libraries().await;
            if libs.is_empty() {
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                continue;
            }

            let mut processed_any = false;

            for lib in libs {
                let pool = &lib.pool;
                let root_path = &lib.root_path;
                let thumb_dir = root_path.join(".opendam").join("thumbs");
                let _ = tokio::fs::create_dir_all(&thumb_dir).await;

                // Reset stuck jobs
                let _ = sqlx::query("UPDATE preview_jobs SET status = 'queued' WHERE status = 'processing'")
                    .execute(pool)
                    .await;

                // Poll for 1 job in this library
                let job: Result<(String, String, String), sqlx::Error> = sqlx::query_as(
                    "SELECT p.id, p.asset_id, a.file_path FROM preview_jobs p JOIN assets a ON p.asset_id = a.id WHERE p.status = 'queued' OR p.status = 'pending' LIMIT 1"
                ).fetch_one(pool).await;

                if let Ok((job_id, asset_id, file_path)) = job {
                    processed_any = true;
                    tracing::info!("Processing job {} for asset {}", job_id, asset_id);

                    if let Err(e) = sqlx::query("UPDATE preview_jobs SET status = 'processing' WHERE id = ?")
                        .bind(&job_id)
                        .execute(pool)
                        .await
                    {
                        tracing::error!("Failed to update job status to processing: {}", e);
                        continue;
                    }

                    let full_source = if Path::new(&file_path).is_absolute() {
                        PathBuf::from(&file_path)
                    } else {
                        root_path.join(&file_path)
                    };

                    let file_stem = full_source
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("preview");

                    // ── NAS Efficiency Check: If thumbnail already exists on disk, SKIP Python! ──
                    let thumb_png = thumb_dir.join(format!("{}_thumb.png", file_stem));
                    let thumb_jpg = thumb_dir.join(format!("{}_thumb.jpg", file_stem));

                    let existing_thumb = if thumb_png.exists() {
                        Some(format!(".opendam/thumbs/{}_thumb.png", file_stem))
                    } else if thumb_jpg.exists() {
                        Some(format!(".opendam/thumbs/{}_thumb.jpg", file_stem))
                    } else {
                        None
                    };

                    if let Some(rel_thumb) = existing_thumb {
                        tracing::info!("Thumbnail already exists for {}, skipping generation", file_stem);
                        let _ = sqlx::query("UPDATE preview_jobs SET status = 'done' WHERE id = ?")
                            .bind(&job_id)
                            .execute(pool)
                            .await;
                        let _ = sqlx::query("UPDATE assets SET preview_status = 'done', thumbnail_path = ? WHERE id = ?")
                            .bind(&rel_thumb)
                            .bind(&asset_id)
                            .execute(pool)
                            .await;

                        let resolved_abs = resolve_path(root_path, &rel_thumb);
                        let _ = state.app_handle.emit("preview:done", json!({
                            "assetId": &asset_id,
                            "thumbnailPath": resolved_abs,
                        }));
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                        continue;
                    }

                    // Call Python preview worker
                    let mut cmd = Command::new(&py_exec);
                    cmd.arg(&worker_py)
                        .stdin(std::process::Stdio::piped())
                        .stdout(std::process::Stdio::piped())
                        .stderr(std::process::Stdio::piped());

                    #[cfg(windows)]
                    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

                    let mut child = match cmd.spawn() {
                        Ok(c) => c,
                        Err(e) => {
                            tracing::error!("Failed to spawn Python: {}", e);
                            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                            continue;
                        }
                    };

                    let mut stdin = child.stdin.take().expect("Failed to open stdin");
                    let stdout = child.stdout.take().expect("Failed to open stdout");

                    let req = json!({
                        "id": job_id,
                        "method": "model_meta",
                        "params": {
                            "source": full_source.to_string_lossy().to_string(),
                            "output_dir": thumb_dir.to_string_lossy().to_string()
                        }
                    });

                    let req_str = req.to_string() + "\n";
                    if let Err(e) = stdin.write_all(req_str.as_bytes()).await {
                        tracing::error!("Failed to write to python stdin: {}", e);
                    }
                    drop(stdin);

                    let mut reader = BufReader::new(stdout);
                    let mut line = String::new();
                    let mut success = false;
                    let mut result_thumb = None;
                    let mut result_model = None;

                    match reader.read_line(&mut line).await {
                        Ok(bytes) => {
                            tracing::info!("Read {} bytes from python: {}", bytes, line);
                            if let Ok(resp) = serde_json::from_str::<serde_json::Value>(&line) {
                                if let Some(res) = resp.get("result") {
                                    if res.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                                        success = true;
                                        if let Some(thumb) = res.get("thumbnail").and_then(|v| v.as_str()) {
                                            result_thumb = Some(thumb.to_string());
                                        }
                                        if let Some(model) = res.get("preview_model").and_then(|v| v.as_str()) {
                                            result_model = Some(model.to_string());
                                        }
                                    } else {
                                        tracing::error!("Python returned ok=false: {:?}", res);
                                    }
                                } else {
                                    tracing::error!("Python response missing result: {}", line);
                                }
                            } else {
                                tracing::error!("Failed to parse Python response: {}", line);
                            }
                        }
                        Err(e) => tracing::error!("Failed to read line from python: {}", e),
                    }

                    if success {
                        tracing::info!("Job {} succeeded, thumb: {:?}, model: {:?}", job_id, result_thumb, result_model);
                        let _ = sqlx::query("UPDATE preview_jobs SET status = 'done' WHERE id = ?")
                            .bind(&job_id)
                            .execute(pool)
                            .await;

                        let thumb_to_save_abs = result_thumb.clone().or_else(|| result_model.clone());
                        let rel_thumb = thumb_to_save_abs.as_ref().map(|abs| {
                            make_relative_path(root_path, Path::new(abs))
                        });

                        if let Some(ref model_path) = result_model {
                            let rel_model = make_relative_path(root_path, Path::new(model_path));
                            let _ = sqlx::query(
                                "UPDATE assets SET preview_status = 'done', thumbnail_path = ?, metadata = json_set(coalesce(nullif(metadata, ''), '{}'), '$.preview_model', ?) WHERE id = ?"
                            )
                            .bind(&rel_thumb)
                            .bind(&rel_model)
                            .bind(&asset_id)
                            .execute(pool)
                            .await;
                        } else if let Some(ref thumb_rel_val) = rel_thumb {
                            let _ = sqlx::query("UPDATE assets SET preview_status = 'done', thumbnail_path = ? WHERE id = ?")
                                .bind(thumb_rel_val)
                                .bind(&asset_id)
                                .execute(pool)
                                .await;
                        } else {
                            let _ = sqlx::query("UPDATE assets SET preview_status = 'done' WHERE id = ?")
                                .bind(&asset_id)
                                .execute(pool)
                                .await;
                        }

                        let resolved_display_thumb = rel_thumb.as_ref().map(|r| resolve_path(root_path, r));
                        let _ = state.app_handle.emit("preview:done", json!({
                            "assetId": &asset_id,
                            "thumbnailPath": &resolved_display_thumb,
                            "previewModel": &result_model,
                        }));
                    } else {
                        tracing::error!("Job {} failed", job_id);
                        let _ = sqlx::query("UPDATE preview_jobs SET status = 'error' WHERE id = ?")
                            .bind(&job_id)
                            .execute(pool)
                            .await;
                        let _ = sqlx::query("UPDATE assets SET preview_status = 'error' WHERE id = ?")
                            .bind(&asset_id)
                            .execute(pool)
                            .await;
                        let _ = state.app_handle.emit("preview:error", json!({
                            "assetId": &asset_id,
                            "jobId": &job_id,
                        }));
                    }

                    let _ = child.wait().await;

                    // NAS Courtesy Sleep: Polite 250ms delay between preview extractions
                    tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
                }
            }

            if !processed_any {
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            }
        }
    });
}
