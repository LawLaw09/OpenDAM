use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};
use image::imageops::FilterType;
use std::sync::Arc;
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::io::{AsyncWriteExt, AsyncBufReadExt, BufReader};
use serde_json::json;
use tauri::Emitter;
use crate::AppState;
use crate::db::{make_relative_path, resolve_path};

pub const THUMB_SIZE: u32 = 512;

/// Checks if an extension is a standard raster image that Rust can decode directly.
pub fn is_native_image(ext: &str) -> bool {
    matches!(
        ext.to_lowercase().as_str(),
        "jpg" | "jpeg" | "png" | "bmp" | "gif" | "tif" | "tiff" | "tga" | "webp"
    )
}

/// Blazingly fast in-process image downsampling using the native Rust `image` crate.
/// Uses Triangle (bilinear) filtering for 4x–8x faster performance than Lanczos3.
pub fn generate_image_thumb(source: &Path, output: &Path) -> Result<()> {
    let img = image::open(source)?;
    let thumb = img.resize(THUMB_SIZE, THUMB_SIZE, FilterType::Triangle);

    if let Some(parent) = output.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let is_jpg = output
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.eq_ignore_ascii_case("jpg") || s.eq_ignore_ascii_case("jpeg"))
        .unwrap_or(false);

    if is_jpg {
        let rgb = thumb.to_rgb8();
        rgb.save_with_format(output, image::ImageFormat::Jpeg)?;
    } else {
        thumb.save(output)?;
    }
    Ok(())
}

/// Persistent Python worker process communicating over JSON-RPC on stdin/stdout.
/// Stays alive across requests to eliminate Python cold-start overhead (200-500ms per file).
struct PythonWorker {
    child: Child,
    stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
}

impl PythonWorker {
    async fn spawn(py_exec: &Path, worker_py: &Path) -> Result<Self> {
        let mut cmd = Command::new(py_exec);
        cmd.arg(worker_py)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .env("PYTHONUNBUFFERED", "1")
            .env("PYTHONIOENCODING", "utf-8");

        #[cfg(windows)]
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

        let mut child = cmd.spawn()?;
        let stdin = child.stdin.take().ok_or_else(|| anyhow!("Failed to take python stdin"))?;
        let stdout = child.stdout.take().ok_or_else(|| anyhow!("Failed to take python stdout"))?;
        let reader = BufReader::new(stdout);

        Ok(Self { child, stdin, reader })
    }

    async fn execute(&mut self, req: &serde_json::Value) -> Result<serde_json::Value> {
        let req_str = req.to_string() + "\n";
        self.stdin.write_all(req_str.as_bytes()).await?;
        self.stdin.flush().await?;

        let mut line = String::new();
        loop {
            line.clear();
            let bytes_read = self.reader.read_line(&mut line).await?;
            if bytes_read == 0 {
                return Err(anyhow!("Python worker closed stdout unexpectedly"));
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if trimmed.starts_with('{') {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                    return Ok(val);
                }
            }
        }
    }

    async fn kill(&mut self) {
        let _ = self.child.kill().await;
    }
}

pub fn start_preview_worker(state: Arc<AppState>) {
    tauri::async_runtime::spawn(async move {
        // Dynamically resolve python worker and script paths
        let current_dir = std::env::current_dir().unwrap_or_default();
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| current_dir.clone());

        let mut search_roots = vec![current_dir.clone(), exe_dir.clone()];
        let mut curr = current_dir.clone();
        for _ in 0..4 {
            if let Some(parent) = curr.parent() {
                search_roots.push(parent.to_path_buf());
                curr = parent.to_path_buf();
            } else {
                break;
            }
        }
        let mut curr_exe = exe_dir.clone();
        for _ in 0..4 {
            if let Some(parent) = curr_exe.parent() {
                search_roots.push(parent.to_path_buf());
                curr_exe = parent.to_path_buf();
            } else {
                break;
            }
        }

        let py_exec = if let Ok(custom) = std::env::var("OPENDAM_PYTHON") {
            PathBuf::from(custom)
        } else {
            let mut found_py = None;
            for root in &search_roots {
                let candidates = [
                    root.join("packages").join("preview-engine").join("venv").join("Scripts").join("pythonw.exe"),
                    root.join("packages").join("preview-engine").join("venv").join("Scripts").join("python.exe"),
                    root.join("packages").join("preview-engine").join("venv").join("bin").join("python"),
                    root.join("venv").join("Scripts").join("pythonw.exe"),
                    root.join("venv").join("Scripts").join("python.exe"),
                    root.join("venv").join("bin").join("python"),
                ];
                if let Some(p) = candidates.into_iter().find(|p| p.exists()) {
                    found_py = Some(p);
                    break;
                }
            }
            found_py.unwrap_or_else(|| {
                let fallbacks = [PathBuf::from("pythonw"), PathBuf::from("python")];
                fallbacks.into_iter().find(|p| p.exists()).unwrap_or_else(|| PathBuf::from("python"))
            })
        };

        let worker_py = if let Ok(custom) = std::env::var("OPENDAM_WORKER") {
            PathBuf::from(custom)
        } else {
            let mut found_worker = None;
            for root in &search_roots {
                let candidates = [
                    root.join("packages").join("preview-engine").join("worker.py"),
                    root.join("worker.py"),
                ];
                if let Some(p) = candidates.into_iter().find(|p| p.exists()) {
                    found_worker = Some(p);
                    break;
                }
            }
            found_worker.unwrap_or_else(|| PathBuf::from("worker.py"))
        };

        tracing::info!("Preview worker using Python: {:?}, Script: {:?}", py_exec, worker_py);

        // Crash-recovery: Reset any orphaned jobs left in 'processing' ONCE at startup
        for lib in state.db.get_all_libraries().await {
            let _ = sqlx::query("UPDATE preview_jobs SET status = 'queued' WHERE status = 'processing'")
                .execute(&lib.pool)
                .await;
        }

        let mut py_worker: Option<PythonWorker> = None;

        loop {
            let libs = state.db.get_all_libraries().await;
            if libs.is_empty() {
                tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
                continue;
            }

            let mut processed_any = false;

            for lib in libs {
                let pool = &lib.pool;
                let root_path = &lib.root_path;
                let thumb_dir = root_path.join(".opendam").join("thumbs");
                let _ = tokio::fs::create_dir_all(&thumb_dir).await;

                // Poll batch of up to 10 jobs to minimize SQLite round-trip overhead
                let jobs: Vec<(String, String, String)> = sqlx::query_as(
                    "SELECT p.id, p.asset_id, a.file_path FROM preview_jobs p JOIN assets a ON p.asset_id = a.id WHERE p.status = 'queued' OR p.status = 'pending' LIMIT 10"
                ).fetch_all(pool).await.unwrap_or_default();

                for (job_id, asset_id, file_path) in jobs {
                    processed_any = true;

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

                    let ext = full_source
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_lowercase();

                    // Check if thumbnail already exists on disk
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
                        continue;
                    }

                    // ── 1. Fast Native Rust Path for Standard Raster Images ──
                    if is_native_image(&ext) {
                        let thumb_file_name = if ext == "png" || ext == "webp" {
                            format!("{}_thumb.png", file_stem)
                        } else {
                            format!("{}_thumb.jpg", file_stem)
                        };
                        let thumb_dest = thumb_dir.join(&thumb_file_name);
                        let rel_thumb = format!(".opendam/thumbs/{}", thumb_file_name);

                        match generate_image_thumb(&full_source, &thumb_dest) {
                            Ok(()) => {
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
                                continue;
                            }
                            Err(e) => {
                                tracing::warn!("Native Rust image thumbnail failed for {}: {}. Falling back to Python.", full_source.display(), e);
                            }
                        }
                    }

                    // ── 2. Persistent Python Worker Path for 3D/CAD/DCC Formats ──
                    if py_worker.is_none() {
                        match PythonWorker::spawn(&py_exec, &worker_py).await {
                            Ok(w) => py_worker = Some(w),
                            Err(e) => {
                                tracing::error!("Failed to spawn Python preview worker: {}", e);
                            }
                        }
                    }

                    let req = json!({
                        "id": &job_id,
                        "method": "model_meta",
                        "params": {
                            "source": full_source.to_string_lossy().to_string(),
                            "output_dir": thumb_dir.to_string_lossy().to_string()
                        }
                    });

                    let py_resp = if let Some(ref mut worker) = py_worker {
                        match tokio::time::timeout(std::time::Duration::from_secs(60), worker.execute(&req)).await {
                            Ok(Ok(val)) => Ok(val),
                            Ok(Err(e)) => {
                                tracing::error!("Python worker execute error: {}. Restarting worker.", e);
                                worker.kill().await;
                                py_worker = None;
                                Err(e)
                            }
                            Err(_) => {
                                tracing::error!("Python worker timed out after 60s processing {}. Killing worker.", file_stem);
                                worker.kill().await;
                                py_worker = None;
                                Err(anyhow!("Job timed out"))
                            }
                        }
                    } else {
                        Err(anyhow!("Python worker unavailable"))
                    };

                    let mut success = false;
                    let mut result_thumb = None;
                    let mut result_model = None;

                    if let Ok(resp) = py_resp {
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
                            tracing::error!("Python response missing result: {:?}", resp);
                        }
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

                    // Yield execution between items to allow other async runtime tasks to progress
                    tokio::task::yield_now().await;
                }
            }

            if !processed_any {
                tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
            }
        }
    });
}
