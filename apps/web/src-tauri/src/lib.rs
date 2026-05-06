use std::path::{Path, PathBuf};
use std::time::Instant;

use base64::{engine::general_purpose, Engine as _};

use serde::Serialize;
use tauri::menu::{Menu, MenuBuilder, SubmenuBuilder};
use tauri::{AppHandle, Runtime};
use tauri::ipc::Channel;
use tauri_plugin_dialog::DialogExt;

use pixiekit_core::{
    anim_preview, atlas_pack, audio, batch, bg_remove, nine_slice, optimize, preset, recent,
    scale, svg_optimize, trim_pad, vectorize, video_to_sprite,
};

#[derive(Debug, Clone, Serialize)]
pub struct ProgressPayload {
    pub index: u32,
    pub total: u32,
    pub current_file: String,
    pub duration_ms: u64,
}

#[tauri::command]
async fn pick_folder(app: AppHandle) -> Result<Option<String>, String> {
    let folder = app.dialog().file().blocking_pick_folder();
    match folder {
        Some(path) => Ok(Some(path.to_string())),
        None => Ok(None),
    }
}

#[tauri::command]
async fn pick_file(app: AppHandle, filters: Vec<String>) -> Result<Option<String>, String> {
    let mut dialog = app.dialog().file();
    if !filters.is_empty() {
        let filter_refs: Vec<&str> = filters.iter().map(|s| s.as_str()).collect();
        dialog = dialog.add_filter("Input Files", &filter_refs);
    }
    let file = dialog.blocking_pick_file();
    match file {
        Some(path) => Ok(Some(path.to_string())),
        None => Ok(None),
    }
}

// --- Tool Commands ---

#[tauri::command]
async fn run_bg_remove(
    input: String,
    output: String,
    options: bg_remove::Options,
    on_progress: Channel<ProgressPayload>,
) -> Result<serde_json::Value, String> {
    let input_path = PathBuf::from(input);
    let output_path = PathBuf::from(output);

    tokio::task::spawn_blocking(move || {
        let files = batch::list_images(&input_path, false, &["png", "jpg", "jpeg", "webp"])
            .map_err(|e| e.to_string())?;

        if files.is_empty() {
            return Err("No images found in input path".to_string());
        }

        if !output_path.exists() {
            std::fs::create_dir_all(&output_path).map_err(|e| e.to_string())?;
        }

        let total = files.len() as u32;
        let start_time = Instant::now();

        for (i, file) in files.iter().enumerate() {
            let index = (i + 1) as u32;
            let file_name = file
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();

            let _ = on_progress.send(ProgressPayload {
                index,
                total,
                current_file: file_name.clone(),
                duration_ms: start_time.elapsed().as_millis() as u64,
            });

            let img = image::open(file).map_err(|e| e.to_string())?.to_rgba8();
            let processed = bg_remove::process(&img, &options);

            let out_file = output_path.join(format!(
                "{}.png",
                file.file_stem().unwrap().to_str().unwrap()
            ));
            processed.save(out_file).map_err(|e| e.to_string())?;
        }

        Ok(serde_json::json!({
            "status": "ok",
            "files_processed": total,
            "total_duration_ms": start_time.elapsed().as_millis()
        }))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn run_vectorize(
    input: String,
    output: String,
    options: vectorize::Options,
    on_progress: Channel<ProgressPayload>,
) -> Result<serde_json::Value, String> {
    let input_path = PathBuf::from(input);
    let output_path = PathBuf::from(output);

    tokio::task::spawn_blocking(move || {
        let files = batch::list_images(&input_path, false, &["png", "jpg", "jpeg", "webp"])
            .map_err(|e| e.to_string())?;

        if files.is_empty() {
            return Err("No images found in input path".to_string());
        }

        if !output_path.exists() {
            std::fs::create_dir_all(&output_path).map_err(|e| e.to_string())?;
        }

        let total = files.len() as u32;
        let start_time = Instant::now();

        for (i, file) in files.iter().enumerate() {
            let index = (i + 1) as u32;
            let file_name = file
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();

            let _ = on_progress.send(ProgressPayload {
                index,
                total,
                current_file: file_name.clone(),
                duration_ms: start_time.elapsed().as_millis() as u64,
            });

            let out_file = output_path.join(format!(
                "{}.svg",
                file.file_stem().unwrap().to_str().unwrap()
            ));
            vectorize::process(file, &out_file, &options).map_err(|e| e.to_string())?;
        }

        Ok(serde_json::json!({
            "status": "ok",
            "files_processed": total,
            "total_duration_ms": start_time.elapsed().as_millis()
        }))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn run_video_to_sprite(
    input: String,
    output: String,
    options: video_to_sprite::Options,
    on_progress: Channel<ProgressPayload>,
) -> Result<serde_json::Value, String> {
    let input_path = PathBuf::from(input);
    let output_path = PathBuf::from(output);

    tokio::task::spawn_blocking(move || {
        let files = batch::list_images(&input_path, false, &["mp4", "mov", "webm", "m4v"])
            .map_err(|e| e.to_string())?;

        if files.is_empty() {
            return Err("No videos found in input path".to_string());
        }

        if !output_path.exists() {
            std::fs::create_dir_all(&output_path).map_err(|e| e.to_string())?;
        }

        let total = files.len() as u32;
        let start_time = Instant::now();

        for (i, file) in files.iter().enumerate() {
            let index = (i + 1) as u32;
            let file_name = file
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();

            let _ = on_progress.send(ProgressPayload {
                index,
                total,
                current_file: file_name.clone(),
                duration_ms: start_time.elapsed().as_millis() as u64,
            });

            video_to_sprite::process(file, &output_path, &options).map_err(|e| e.to_string())?;
        }

        Ok(serde_json::json!({
            "status": "ok",
            "files_processed": total,
            "total_duration_ms": start_time.elapsed().as_millis()
        }))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn run_atlas_pack(
    input: String,
    output: String,
    options: atlas_pack::Options,
    on_progress: Channel<ProgressPayload>,
) -> Result<serde_json::Value, String> {
    let input_path = PathBuf::from(input);
    let output_path = PathBuf::from(output);

    tokio::task::spawn_blocking(move || {
        let files = batch::list_images(&input_path, false, &["png"])
            .map_err(|e| e.to_string())?;

        if files.is_empty() {
            return Err("No PNG images found in input path".to_string());
        }

        if !output_path.exists() {
            std::fs::create_dir_all(&output_path).map_err(|e| e.to_string())?;
        }

        let total = files.len() as u32;
        let start_time = Instant::now();

        let _ = on_progress.send(ProgressPayload {
            index: 0,
            total,
            current_file: format!("Packing {} sprites…", total),
            duration_ms: 0,
        });

        let report = atlas_pack::process(&files, &output_path, &options).map_err(|e| e.to_string())?;

        let _ = on_progress.send(ProgressPayload {
            index: total,
            total,
            current_file: format!("Atlas {}x{}", report.atlas_size.0, report.atlas_size.1),
            duration_ms: start_time.elapsed().as_millis() as u64,
        });

        Ok(serde_json::json!({
            "status": "ok",
            "packed": report.packed,
            "total": report.total,
            "atlas_size": report.atlas_size,
            "efficiency": report.efficiency
        }))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn run_optimize(
    input: String,
    output: String,
    options: optimize::Options,
    on_progress: Channel<ProgressPayload>,
) -> Result<serde_json::Value, String> {
    let input_path = PathBuf::from(input);
    let output_path = PathBuf::from(output);

    tokio::task::spawn_blocking(move || {
        let files = batch::list_images(&input_path, false, &["png", "jpg", "jpeg", "webp"])
            .map_err(|e| e.to_string())?;

        if files.is_empty() {
            return Err("No images found in input path".to_string());
        }

        if !output_path.exists() {
            std::fs::create_dir_all(&output_path).map_err(|e| e.to_string())?;
        }

        let total = files.len() as u32;
        let start_time = Instant::now();

        for (i, file) in files.iter().enumerate() {
            let index = (i + 1) as u32;
            let file_name = file
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();

            let _ = on_progress.send(ProgressPayload {
                index,
                total,
                current_file: file_name.clone(),
                duration_ms: start_time.elapsed().as_millis() as u64,
            });

            let out_file = output_path.join(file.file_name().unwrap());
            optimize::process(file, &out_file, &options).map_err(|e| e.to_string())?;
        }

        Ok(serde_json::json!({
            "status": "ok",
            "files_processed": total,
            "total_duration_ms": start_time.elapsed().as_millis()
        }))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn run_scale(
    input: String,
    output: String,
    options: scale::Options,
    on_progress: Channel<ProgressPayload>,
) -> Result<serde_json::Value, String> {
    let input_path = PathBuf::from(input);
    let output_path = PathBuf::from(output);

    tokio::task::spawn_blocking(move || {
        let files = batch::list_images(&input_path, false, &["png", "jpg", "jpeg", "webp"])
            .map_err(|e| e.to_string())?;

        if files.is_empty() {
            return Err("No images found in input path".to_string());
        }

        if !output_path.exists() {
            std::fs::create_dir_all(&output_path).map_err(|e| e.to_string())?;
        }

        let total = files.len() as u32;
        let start_time = Instant::now();

        for (i, file) in files.iter().enumerate() {
            let index = (i + 1) as u32;
            let file_name = file
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();

            let _ = on_progress.send(ProgressPayload {
                index,
                total,
                current_file: file_name.clone(),
                duration_ms: start_time.elapsed().as_millis() as u64,
            });

            let out_file = output_path.join(file.file_name().unwrap());
            scale::process(file, &out_file, &options).map_err(|e| e.to_string())?;
        }

        Ok(serde_json::json!({
            "status": "ok",
            "files_processed": total,
            "total_duration_ms": start_time.elapsed().as_millis()
        }))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn run_audio(
    input: String,
    output: String,
    options: audio::Options,
    on_progress: Channel<ProgressPayload>,
) -> Result<serde_json::Value, String> {
    let input_path = PathBuf::from(input);
    let output_path = PathBuf::from(output);

    tokio::task::spawn_blocking(move || {
        let files = batch::list_images(&input_path, false, &["mp3", "wav", "ogg", "m4a", "flac"])
            .map_err(|e| e.to_string())?;

        if files.is_empty() {
            return Err("No audio files found in input path".to_string());
        }

        if !output_path.exists() {
            std::fs::create_dir_all(&output_path).map_err(|e| e.to_string())?;
        }

        let total = files.len() as u32;
        let start_time = Instant::now();

        for (i, file) in files.iter().enumerate() {
            let index = (i + 1) as u32;
            let file_name = file
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();

            let _ = on_progress.send(ProgressPayload {
                index,
                total,
                current_file: file_name.clone(),
                duration_ms: start_time.elapsed().as_millis() as u64,
            });

            let out_file = output_path.join(format!(
                "{}.{}",
                file.file_stem().unwrap().to_str().unwrap(),
                options.target_format.extension()
            ));
            audio::process(file, &out_file, &options).map_err(|e| e.to_string())?;
        }

        Ok(serde_json::json!({
            "status": "ok",
            "files_processed": total,
            "total_duration_ms": start_time.elapsed().as_millis()
        }))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn run_trim_pad(
    input: String,
    output: String,
    options: trim_pad::Options,
    on_progress: Channel<ProgressPayload>,
) -> Result<serde_json::Value, String> {
    let input_path = PathBuf::from(input);
    let output_path = PathBuf::from(output);

    tokio::task::spawn_blocking(move || {
        let files = batch::list_images(&input_path, false, &["png", "jpg", "jpeg", "webp"])
            .map_err(|e| e.to_string())?;

        if files.is_empty() {
            return Err("No images found in input path".to_string());
        }

        if !output_path.exists() {
            std::fs::create_dir_all(&output_path).map_err(|e| e.to_string())?;
        }

        let total = files.len() as u32;
        let start_time = Instant::now();

        for (i, file) in files.iter().enumerate() {
            let index = (i + 1) as u32;
            let file_name = file
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();

            let _ = on_progress.send(ProgressPayload {
                index,
                total,
                current_file: file_name.clone(),
                duration_ms: start_time.elapsed().as_millis() as u64,
            });

            let out_file = output_path.join(file.file_name().unwrap());
            trim_pad::process(file, &out_file, &options).map_err(|e| e.to_string())?;
        }

        Ok(serde_json::json!({
            "status": "ok",
            "files_processed": total,
            "total_duration_ms": start_time.elapsed().as_millis()
        }))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn run_svg_optimize(
    input: String,
    output: String,
    options: svg_optimize::Options,
    on_progress: Channel<ProgressPayload>,
) -> Result<serde_json::Value, String> {
    let input_path = PathBuf::from(input);
    let output_path = PathBuf::from(output);

    tokio::task::spawn_blocking(move || {
        let files = batch::list_images(&input_path, false, &["svg"])
            .map_err(|e| e.to_string())?;

        if files.is_empty() {
            return Err("No SVG files found in input path".to_string());
        }

        if !output_path.exists() {
            std::fs::create_dir_all(&output_path).map_err(|e| e.to_string())?;
        }

        let total = files.len() as u32;
        let start_time = Instant::now();

        for (i, file) in files.iter().enumerate() {
            let index = (i + 1) as u32;
            let file_name = file
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();

            let _ = on_progress.send(ProgressPayload {
                index,
                total,
                current_file: file_name.clone(),
                duration_ms: start_time.elapsed().as_millis() as u64,
            });

            let out_file = output_path.join(file.file_name().unwrap());
            svg_optimize::process(file, &out_file, &options).map_err(|e| e.to_string())?;
        }

        Ok(serde_json::json!({
            "status": "ok",
            "files_processed": total,
            "total_duration_ms": start_time.elapsed().as_millis()
        }))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn run_nine_slice(
    input: String,
    output: String,
    options: nine_slice::Options,
    on_progress: Channel<ProgressPayload>,
) -> Result<serde_json::Value, String> {
    let input_path = PathBuf::from(input);
    let output_path = PathBuf::from(output);

    tokio::task::spawn_blocking(move || {
        let files = batch::list_images(&input_path, false, &["png", "jpg", "jpeg", "webp"])
            .map_err(|e| e.to_string())?;

        if files.is_empty() {
            return Err("No images found in input path".to_string());
        }

        if !output_path.exists() {
            std::fs::create_dir_all(&output_path).map_err(|e| e.to_string())?;
        }

        let total = files.len() as u32;
        let start_time = Instant::now();

        for (i, file) in files.iter().enumerate() {
            let index = (i + 1) as u32;
            let file_name = file
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();

            let _ = on_progress.send(ProgressPayload {
                index,
                total,
                current_file: file_name.clone(),
                duration_ms: start_time.elapsed().as_millis() as u64,
            });

            nine_slice::process(file, &output_path, &options).map_err(|e| e.to_string())?;
        }

        Ok(serde_json::json!({
            "status": "ok",
            "files_processed": total,
            "total_duration_ms": start_time.elapsed().as_millis()
        }))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn run_anim_preview(
    input: String,
    output: String,
    options: anim_preview::Options,
    on_progress: Channel<ProgressPayload>,
) -> Result<serde_json::Value, String> {
    let input_path = PathBuf::from(input);
    let output_path = PathBuf::from(output);

    tokio::task::spawn_blocking(move || {
        let files = batch::list_images(&input_path, false, &["png", "jpg", "jpeg", "webp"])
            .map_err(|e| e.to_string())?;

        if files.is_empty() {
            return Err("No images found in input path".to_string());
        }

        if !output_path.exists() {
            std::fs::create_dir_all(&output_path).map_err(|e| e.to_string())?;
        }

        let total = files.len() as u32;
        let start_time = Instant::now();

        for (i, file) in files.iter().enumerate() {
            let index = (i + 1) as u32;
            let file_name = file
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();

            let _ = on_progress.send(ProgressPayload {
                index,
                total,
                current_file: file_name.clone(),
                duration_ms: start_time.elapsed().as_millis() as u64,
            });

            anim_preview::process(file, &output_path, &options).map_err(|e| e.to_string())?;
        }

        Ok(serde_json::json!({
            "status": "ok",
            "files_processed": total,
            "total_duration_ms": start_time.elapsed().as_millis()
        }))
    })
    .await
    .map_err(|e| e.to_string())?
}

// --- Preview Commands ---

#[tauri::command]
async fn preview_bg_remove(path: String, options: bg_remove::Options) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let img = image::open(path).map_err(|e| e.to_string())?.to_rgba8();
        let processed = bg_remove::process(&img, &options);
        let mut buffer = std::io::Cursor::new(Vec::new());
        processed
            .write_to(&mut buffer, image::ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        Ok(general_purpose::STANDARD.encode(buffer.into_inner()))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn preview_vectorize(path: String, options: vectorize::Options) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let tmp = tempfile::NamedTempFile::new().map_err(|e| e.to_string())?;
        vectorize::process(Path::new(&path), tmp.path(), &options).map_err(|e| e.to_string())?;
        let svg = std::fs::read_to_string(tmp.path()).map_err(|e| e.to_string())?;
        Ok(svg)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn preview_trim_pad(path: String, options: trim_pad::Options) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let tmp = tempfile::Builder::new()
            .suffix(".png")
            .tempfile()
            .map_err(|e| e.to_string())?;
        trim_pad::process(Path::new(&path), tmp.path(), &options).map_err(|e| e.to_string())?;
        let bytes = std::fs::read(tmp.path()).map_err(|e| e.to_string())?;
        Ok(general_purpose::STANDARD.encode(bytes))
    })
    .await
    .map_err(|e| e.to_string())?
}

// --- Preset Commands ---

#[tauri::command]
async fn list_presets() -> Result<Vec<preset::Preset>, String> {
    let names = preset::list().map_err(|e| e.to_string())?;
    let mut presets = Vec::new();
    for name in names {
        if let Ok(p) = preset::load(&name) {
            presets.push(p);
        }
    }
    Ok(presets)
}

#[tauri::command]
async fn get_preset(name: String) -> Result<Option<preset::Preset>, String> {
    match preset::load(&name) {
        Ok(p) => Ok(Some(p)),
        Err(pixiekit_core::Error::PresetNotFound { .. }) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
async fn save_preset(
    name: String,
    tool: String,
    options: serde_json::Value,
) -> Result<preset::Preset, String> {
    preset::save(&name, &tool, options).map_err(|e| e.to_string())?;
    preset::load(&name).map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_preset(name: String) -> Result<(), String> {
    preset::delete(&name).map_err(|e| e.to_string())?;
    Ok(())
}

// --- Recent Paths (M12.7) ---

#[tauri::command]
async fn list_recent_paths() -> Result<recent::RecentPaths, String> {
    Ok(recent::load())
}

#[tauri::command]
async fn add_recent_path(kind: recent::Kind, path: String) -> Result<recent::RecentPaths, String> {
    recent::add(kind, path).map_err(|e| e.to_string())
}

#[tauri::command]
async fn clear_recent_paths(kind: recent::Kind) -> Result<recent::RecentPaths, String> {
    recent::clear(kind).map_err(|e| e.to_string())
}

/// Native menu bar (M12.1): File / Edit / View / Window / Help.
/// Each submenu uses Tauri's predefined items so we get OS-native shortcuts
/// (Cmd+Q, Cmd+C, Cmd+W, etc.) on macOS without manual key handling.
fn build_app_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let file = SubmenuBuilder::new(app, "File")
        .close_window()
        .quit()
        .build()?;

    let edit = SubmenuBuilder::new(app, "Edit")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;

    let view = SubmenuBuilder::new(app, "View")
        .fullscreen()
        .build()?;

    let window = SubmenuBuilder::new(app, "Window")
        .minimize()
        .maximize()
        .build()?;

    let help = SubmenuBuilder::new(app, "Help")
        .about(Some(tauri::menu::AboutMetadata {
            name: Some("Pixiekit".into()),
            ..Default::default()
        }))
        .build()?;

    MenuBuilder::new(app)
        .items(&[&file, &edit, &view, &window, &help])
        .build()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .menu(build_app_menu)
        .invoke_handler(tauri::generate_handler![
            pick_folder,
            pick_file,
            run_bg_remove,
            run_vectorize,
            run_video_to_sprite,
            run_atlas_pack,
            run_optimize,
            run_scale,
            run_audio,
            run_trim_pad,
            run_svg_optimize,
            run_nine_slice,
            run_anim_preview,
            preview_bg_remove,
            preview_vectorize,
            preview_trim_pad,
            list_presets,
            get_preset,
            save_preset,
            delete_preset,
            list_recent_paths,
            add_recent_path,
            clear_recent_paths
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
