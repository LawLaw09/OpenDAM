use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};
use image::imageops::FilterType;
use std::sync::{Arc, OnceLock};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::io::{AsyncWriteExt, AsyncBufReadExt, BufReader};
use serde_json::json;
use tauri::Emitter;
use crate::AppState;
use crate::db::{make_relative_path, resolve_path};

pub const THUMB_SIZE: u32 = 512;

/// Raster formats the Rust `image` crate decodes in-process (no Python needed).
/// `exr`/`hdr` are floating-point and get tone-mapped; `dds` covers BC1–BC3 textures.
pub const NATIVE_IMAGE_EXTS: &[&str] = &[
    "jpg", "jpeg", "png", "bmp", "gif", "tif", "tiff", "tga", "webp", "exr", "hdr", "dds",
];

/// Formats that need the Python preview worker to produce a thumbnail.
pub const PYTHON_PREVIEW_EXTS: &[&str] = &[
    "max", "fbx", "obj", "gltf", "glb", "dae", "stl", "3ds", "skp", "rfa", "rvt", "dwg", "3dm",
    "vrmat", "psd", "psb", "ai",
];

/// Checks if an extension is a standard raster image that Rust can decode directly.
pub fn is_native_image(ext: &str) -> bool {
    NATIVE_IMAGE_EXTS.contains(&ext.to_lowercase().as_str())
}

/// Formats Rust can make a thumbnail for without Python: plain/float images plus the
/// previews DCC apps embed in their files (see `embedded.rs`). Embedded-preview formats
/// still fall back to Python when a file has no usable embedded image.
pub fn rust_preview_exts() -> Vec<&'static str> {
    let mut v = NATIVE_IMAGE_EXTS.to_vec();
    v.extend_from_slice(crate::embedded::EMBEDDED_PREVIEW_EXTS);
    v
}

/// Whether OpenDAM generates a preview for this extension (natively or via Python).
pub fn needs_preview(ext: &str) -> bool {
    let ext = ext.to_lowercase();
    NATIVE_IMAGE_EXTS.contains(&ext.as_str()) || PYTHON_PREVIEW_EXTS.contains(&ext.as_str())
}

/// Blazingly fast in-process image downsampling using the native Rust `image` crate.
/// Uses Triangle (bilinear) filtering for 4x–8x faster performance than Lanczos3.
pub fn generate_image_thumb(source: &Path, output: &Path) -> Result<()> {
    write_thumb(image::open(source)?, output)
}

/// Thumbnail from the preview image a DCC/CAD file embeds (.max/.rvt/.rfa/.skp/.psd).
/// Returns `Ok(false)` when the file has no usable embedded preview.
pub fn generate_embedded_thumb(source: &Path, ext: &str, output: &Path) -> Result<bool> {
    let Some(img) = crate::embedded::extract_preview(source, ext)? else {
        return Ok(false);
    };
    // Embedded previews are often tiny (Revit: 128 px). Upscale smoothly and sharpen a
    // little so they don't look blocky in the grid, as the Python worker did.
    let img = if img.width().max(img.height()) < THUMB_SIZE {
        img.resize(THUMB_SIZE, THUMB_SIZE, FilterType::Lanczos3).unsharpen(1.2, 2)
    } else {
        img
    };
    write_thumb(img, output)?;
    Ok(true)
}

fn write_thumb(img: image::DynamicImage, output: &Path) -> Result<()> {
    let thumb = img.resize(THUMB_SIZE, THUMB_SIZE, FilterType::Triangle);
    // Float images (EXR/HDR) hold linear, unbounded light values. Writing them out as-is
    // clips everything above 1.0 to white and looks too dark below it, so tone-map first.
    let thumb = if matches!(thumb.color(), image::ColorType::Rgb32F | image::ColorType::Rgba32F) {
        image::DynamicImage::ImageRgb8(tone_map_to_srgb8(&thumb.to_rgb32f()))
    } else {
        thumb
    };

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

/// Reinhard tone mapping (`c / (1 + c)`) followed by sRGB-ish gamma (1/2.2).
/// NaN and negative values (both legal in EXR) map to black.
pub fn tone_map_to_srgb8(img: &image::Rgb32FImage) -> image::RgbImage {
    image::RgbImage::from_fn(img.width(), img.height(), |x, y| {
        let p = img.get_pixel(x, y);
        image::Rgb(p.0.map(|c| {
            let c = if c.is_finite() && c > 0.0 { c } else { 0.0 };
            let mapped = (c / (1.0 + c)).powf(1.0 / 2.2);
            (mapped * 255.0).round().clamp(0.0, 255.0) as u8
        }))
    })
}

/// Persistent Python worker process communicating over JSON-RPC on stdin/stdout.
/// Stays alive across requests to eliminate Python cold-start overhead (200-500ms per file).
struct PythonWorker {
    child: Child,
    stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
}

impl PythonWorker {
    async fn spawn(worker: &WorkerCommand) -> Result<Self> {
        let mut cmd = Command::new(&worker.python);
        // The bundled (embeddable) Python ignores PYTHON* environment variables, so
        // behaviour is set with flags instead:
        //   -I        isolated: no user site-packages, no env vars, no cwd on sys.path
        //   -u        unbuffered stdout (responses are line-delimited JSON)
        //   -X utf8   UTF-8 I/O regardless of the Windows code page (non-ASCII paths)
        //   -B        don't write __pycache__ into the install folder
        cmd.args(["-I", "-u", "-X", "utf8", "-B"])
            .arg(&worker.worker)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .env_remove("PYTHONHOME")
            .env_remove("PYTHONPATH");

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

/// The interpreter + script used for formats Rust can't render itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerCommand {
    pub python: PathBuf,
    pub worker: PathBuf,
}

static WORKER_COMMAND: OnceLock<Option<WorkerCommand>> = OnceLock::new();

/// Pick the preview worker's interpreter. Pure, so it can be tested with temp dirs.
///
/// Order: explicit `OPENDAM_PYTHON` + `OPENDAM_WORKER` override → the repo's dev venv
/// (`dev_roots`, only passed in debug builds) → the private Python bundled with the
/// installer under `<resources>/preview-engine/`.
///
/// It never falls back to `python` on PATH: that would be the user's own Python (or
/// the Microsoft Store stub), missing our packages, and not ours to run.
pub fn resolve_worker_command(
    env_python: Option<PathBuf>,
    env_worker: Option<PathBuf>,
    dev_roots: &[PathBuf],
    resource_dir: Option<&Path>,
) -> Option<WorkerCommand> {
    let valid = |python: PathBuf, worker: PathBuf| {
        (python.is_file() && worker.is_file()).then_some(WorkerCommand { python, worker })
    };

    if let (Some(python), Some(worker)) = (env_python, env_worker) {
        if let Some(cmd) = valid(python, worker) {
            return Some(cmd);
        }
    }

    for root in dev_roots {
        let engine = root.join("packages").join("preview-engine");
        for python in [
            engine.join("venv").join("Scripts").join("python.exe"),
            engine.join("venv").join("bin").join("python"),
        ] {
            if let Some(cmd) = valid(python, engine.join("worker.py")) {
                return Some(cmd);
            }
        }
    }

    let bundled = resource_dir?.join("preview-engine");
    valid(bundled.join("python").join("python.exe"), bundled.join("worker.py"))
}

/// Resolve the worker once at startup (before indexing or the preview loop start).
pub fn init_worker_command(app: &tauri::AppHandle) {
    use tauri::Manager;

    let mut dev_roots = Vec::new();
    if cfg!(debug_assertions) {
        // `pnpm dev` runs from apps/desktop/src-tauri; walk up to the repo root.
        for start in [std::env::current_dir().ok(), std::env::current_exe().ok()].into_iter().flatten() {
            dev_roots.extend(start.ancestors().take(6).map(Path::to_path_buf));
        }
    }
    let cmd = resolve_worker_command(
        std::env::var_os("OPENDAM_PYTHON").map(PathBuf::from),
        std::env::var_os("OPENDAM_WORKER").map(PathBuf::from),
        &dev_roots,
        app.path().resource_dir().ok().as_deref(),
    );
    match &cmd {
        Some(c) => tracing::info!("Preview worker: {:?} {:?}", c.python, c.worker),
        None => tracing::warn!(
            "No preview worker Python found; 3D/CAD previews will wait until one is available"
        ),
    }
    let _ = WORKER_COMMAND.set(cmd);
}

/// Whether Python-rendered formats can be processed in this run.
pub fn python_worker_available() -> bool {
    matches!(WORKER_COMMAND.get(), Some(Some(_)))
}

pub fn start_preview_worker(state: Arc<AppState>) {
    tauri::async_runtime::spawn(async move {
        let worker_cmd = WORKER_COMMAND.get().cloned().flatten();
        // Without a worker, only pick jobs Rust can render. Python-format jobs stay
        // queued (not 'error') so they complete once a worker is available.
        let native_list = rust_preview_exts()
            .iter()
            .map(|e| format!("'{}'", e))
            .collect::<Vec<_>>()
            .join(",");
        let job_filter = if worker_cmd.is_some() {
            String::new()
        } else {
            format!(" AND lower(a.extension) IN ({})", native_list)
        };
        let jobs_sql = format!(
            "SELECT p.id, p.asset_id, a.file_path, p.status FROM preview_jobs p JOIN assets a ON p.asset_id = a.id WHERE (p.status = 'queued' OR p.status = 'pending'){} LIMIT 10",
            job_filter
        );

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
                let jobs: Vec<(String, String, String, String)> = sqlx::query_as(&jobs_sql)
                    .fetch_all(pool).await.unwrap_or_default();

                for (job_id, asset_id, file_path, job_status) in jobs {
                    // 'queued' jobs come from an explicit user request (request_preview);
                    // the indexer creates 'pending' ones. A user request must re-render even
                    // when a thumbnail already exists, otherwise "Regenerate" is a no-op.
                    // The new render overwrites the old file only on success.
                    let force_render = job_status == "queued";
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

                    let existing_thumb = if force_render {
                        None
                    } else if thumb_png.exists() {
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

                    // ── 1. Native Rust path: images, and previews embedded in DCC files ──
                    let is_embedded = crate::embedded::EMBEDDED_PREVIEW_EXTS.contains(&ext.as_str());
                    if is_native_image(&ext) || is_embedded {
                        let thumb_file_name = if ext == "png" || ext == "webp" || is_embedded {
                            format!("{}_thumb.png", file_stem)
                        } else {
                            format!("{}_thumb.jpg", file_stem)
                        };
                        let thumb_dest = thumb_dir.join(&thumb_file_name);
                        let rel_thumb = format!(".opendam/thumbs/{}", thumb_file_name);

                        // Decoding can take a second or more (large EXR/PSD): keep it off
                        // the async runtime's worker threads.
                        let (src, dest, ext_c) = (full_source.clone(), thumb_dest.clone(), ext.clone());
                        let native = tauri::async_runtime::spawn_blocking(move || {
                            if is_embedded {
                                generate_embedded_thumb(&src, &ext_c, &dest)
                            } else {
                                generate_image_thumb(&src, &dest).map(|_| true)
                            }
                        })
                        .await
                        .map_err(|e| anyhow!("thumbnail task failed: {}", e))
                        .and_then(|r| r)
                        .and_then(|made| if made { Ok(()) } else { Err(anyhow!("no embedded preview")) });

                        match native {
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
                                tracing::info!("Native thumbnail unavailable for {}: {}. Trying Python.", full_source.display(), e);
                            }
                        }
                    }

                    // ── 2. Persistent Python Worker Path for 3D/CAD/DCC Formats ──
                    if let (None, Some(cmd)) = (&py_worker, &worker_cmd) {
                        match PythonWorker::spawn(cmd).await {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("opendam-preview-{}-{}", tag, uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Left half 0.18 (mid-grey in linear light), right half 8.0 (well above 1.0).
    fn hdr_test_image() -> image::Rgb32FImage {
        image::Rgb32FImage::from_fn(64, 32, |x, _| {
            let v = if x < 32 { 0.18 } else { 8.0 };
            image::Rgb([v, v, v])
        })
    }

    #[test]
    fn tone_map_keeps_detail_and_handles_bad_values() {
        let mut img = image::Rgb32FImage::new(4, 1);
        img.put_pixel(0, 0, image::Rgb([0.0, -1.0, f32::NAN]));
        img.put_pixel(1, 0, image::Rgb([0.18, 0.18, 0.18]));
        img.put_pixel(2, 0, image::Rgb([8.0, 8.0, 8.0]));
        img.put_pixel(3, 0, image::Rgb([f32::INFINITY, 1e30, 1.0]));
        let out = tone_map_to_srgb8(&img);
        assert_eq!(out.get_pixel(0, 0).0, [0, 0, 0]);
        let mid = out.get_pixel(1, 0).0[0];
        let bright = out.get_pixel(2, 0).0[0];
        assert!((90..140).contains(&mid), "mid-grey mapped to {}", mid);
        assert!(bright > mid && bright < 255, "8.0 mapped to {} (should not clip)", bright);
        assert_eq!(out.get_pixel(3, 0).0[0], 0, "inf treated as invalid");
        assert_eq!(out.get_pixel(3, 0).0[1], 255);
    }

    #[test]
    fn exr_and_hdr_thumbnails_are_generated_natively() {
        let dir = scratch("float");
        let src_img = image::DynamicImage::ImageRgb32F(hdr_test_image());
        for ext in ["exr", "hdr"] {
            let src = dir.join(format!("gradient.{}", ext));
            src_img.save(&src).unwrap();
            assert!(is_native_image(ext) && needs_preview(ext));
            for out_ext in ["jpg", "png"] {
                let out = dir.join(format!("{}_thumb.{}", ext, out_ext));
                generate_image_thumb(&src, &out).unwrap();
                let thumb = image::open(&out).unwrap().to_rgb8();
                let (l, r) = (thumb.get_pixel(4, 4).0[0], thumb.get_pixel(thumb.width() - 4, 4).0[0]);
                assert!(l > 60 && l < 160, "{} -> {}: dark side {}", ext, out_ext, l);
                assert!(r > l && r < 255, "{} -> {}: bright side {} vs {}", ext, out_ext, r, l);
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn touch(p: &Path) -> PathBuf {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, b"").unwrap();
        p.to_path_buf()
    }

    #[test]
    fn worker_resolution_order_and_no_path_fallback() {
        let dir = scratch("resolve");
        let res = dir.join("install");
        let repo = dir.join("repo");

        // Nothing present: no worker, and never a bare `python` from PATH.
        assert_eq!(resolve_worker_command(None, None, std::slice::from_ref(&repo), Some(&res)), None);
        assert_eq!(resolve_worker_command(None, None, &[], None), None);

        // Bundled interpreter only.
        let b_py = touch(&res.join("preview-engine").join("python").join("python.exe"));
        let b_worker = touch(&res.join("preview-engine").join("worker.py"));
        let bundled = WorkerCommand { python: b_py.clone(), worker: b_worker.clone() };
        assert_eq!(resolve_worker_command(None, None, std::slice::from_ref(&repo), Some(&res)), Some(bundled.clone()));

        // Dev venv wins over the bundle when a dev root is passed (debug builds only).
        let engine = repo.join("packages").join("preview-engine");
        let v_py = touch(&engine.join("venv").join("Scripts").join("python.exe"));
        let v_worker = touch(&engine.join("worker.py"));
        assert_eq!(
            resolve_worker_command(None, None, std::slice::from_ref(&repo), Some(&res)),
            Some(WorkerCommand { python: v_py, worker: v_worker })
        );
        // Release builds pass no dev roots, so they get the bundle.
        assert_eq!(resolve_worker_command(None, None, &[], Some(&res)), Some(bundled.clone()));

        // Explicit override wins, but only if both files exist.
        let o_py = touch(&dir.join("custom").join("python.exe"));
        let o_worker = touch(&dir.join("custom").join("worker.py"));
        assert_eq!(
            resolve_worker_command(Some(o_py.clone()), Some(o_worker.clone()), &[], Some(&res)),
            Some(WorkerCommand { python: o_py.clone(), worker: o_worker })
        );
        assert_eq!(
            resolve_worker_command(Some(o_py), Some(dir.join("missing.py")), &[], Some(&res)),
            Some(bundled)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn embedded_revit_preview_becomes_an_upscaled_thumbnail() {
        use std::io::Write;
        let dir = scratch("embedded");
        let src = dir.join("family.rfa");
        {
            let mut png = std::io::Cursor::new(Vec::new());
            image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(128, 128, image::Rgb([40, 90, 200])))
                .write_to(&mut png, image::ImageFormat::Png)
                .unwrap();
            let mut comp = cfb::create(&src).unwrap();
            let mut s = comp.create_stream("/RevitPreview4.0").unwrap();
            s.write_all(&[1, 2, 3, 4]).unwrap();
            s.write_all(png.get_ref()).unwrap();
        }
        let out = dir.join("family_thumb.png");
        assert!(generate_embedded_thumb(&src, "rfa", &out).unwrap());
        let thumb = image::open(&out).unwrap().to_rgb8();
        assert_eq!(thumb.dimensions(), (THUMB_SIZE, THUMB_SIZE));
        let px = thumb.get_pixel(THUMB_SIZE / 2, THUMB_SIZE / 2).0;
        assert!(px.iter().zip([40u8, 90, 200]).all(|(a, b)| (*a as i16 - b as i16).abs() <= 3), "{:?}", px);

        // A file without an embedded preview reports false (caller falls back to Python).
        let plain = dir.join("empty.skp");
        std::fs::write(&plain, vec![0u8; 2048]).unwrap();
        assert!(!generate_embedded_thumb(&plain, "skp", &dir.join("x.png")).unwrap());
        assert!(rust_preview_exts().contains(&"max") && rust_preview_exts().contains(&"exr"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_float_image_is_an_error_not_a_panic() {
        let dir = scratch("corrupt");
        let src = dir.join("broken.exr");
        std::fs::write(&src, b"v/1\x01 definitely not an exr file").unwrap();
        assert!(generate_image_thumb(&src, &dir.join("out.jpg")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod real_file_check {
    /// Manual check against real files: OPENDAM_FLOAT_SAMPLES=<dir> cargo test -- --ignored
    #[test]
    #[ignore]
    fn decode_real_float_images() {
        let Ok(dir) = std::env::var("OPENDAM_FLOAT_SAMPLES") else { return };
        let keep = std::env::var("OPENDAM_FLOAT_OUT").ok();
        let out = keep.clone().map(std::path::PathBuf::from).unwrap_or_else(|| std::env::temp_dir().join(format!("opendam-real-{}", uuid::Uuid::new_v4())));
        std::fs::create_dir_all(&out).unwrap();
        for entry in walkdir::WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
            let p = entry.path();
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
            if ext != "exr" && ext != "hdr" || p.to_string_lossy().contains(".opendam") {
                continue;
            }
            let dest = out.join(format!("{}.jpg", p.file_stem().unwrap().to_string_lossy()));
            let t = std::time::Instant::now();
            let r = super::generate_image_thumb(p, &dest);
            println!("{:?}: {:?} in {:?}", p.file_name().unwrap(), r.map(|_| "ok"), t.elapsed());
        }
        if keep.is_none() {
            let _ = std::fs::remove_dir_all(&out);
        }
    }
}
