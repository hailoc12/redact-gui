//! Redact GUI — Tauri v2. Toàn bộ xử lý nằm trong lib redact-cli (local, offline).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};

use redact_cli::patterns::{ids_for_level, BUILT_IN};
use redact_cli::redactor::{MapEntry, Redactor};
use redact_cli::unredact::Unredactor;
use serde::Serialize;
use tauri::Manager;

#[derive(Serialize)]
struct RedactResult {
    input_path: String,
    redacted_path: String,
    dict_path: String,
    total: usize,
    per_pattern: Vec<(String, usize)>,
}

#[derive(Serialize)]
struct UnredactResult {
    input_path: String,
    restored_path: String,
    restored_count: usize,
    dict_size: usize,
}

fn output_paths(file: &Path) -> (PathBuf, PathBuf) {
    let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("out");
    let redacted = match file.extension().and_then(|e| e.to_str()) {
        Some("pdf") => file.with_file_name(format!("{stem}.redacted.txt")),
        Some(e) => file.with_file_name(format!("{stem}.redacted.{e}")),
        None => file.with_file_name(format!("{stem}.redacted")),
    };
    let dict = file.with_file_name(format!("{stem}.dict.json"));
    (redacted, dict)
}

fn err<E: std::fmt::Display>(e: E) -> String {
    format!("{e:#}")
}

#[tauri::command]
fn redact_file(path: String, level: u8) -> Result<RedactResult, String> {
    let file = PathBuf::from(&path);
    let ids = ids_for_level(level);
    let pattern_list: Vec<_> = BUILT_IN
        .iter()
        .filter(|p| ids.contains(&p.id))
        .cloned()
        .collect();
    let mut redactor = Redactor::new(pattern_list).map_err(err)?;

    let content = redact_cli::formats::redact_file(&file, &mut redactor).map_err(err)?;
    let (redacted_path, dict_path) = output_paths(&file);
    std::fs::write(&redacted_path, &content).map_err(|e| err(format!("ghi output: {e}")))?;

    let mapping: Vec<MapEntry> = redactor.mapping.clone();
    let dict_json = serde_json::to_string_pretty(&mapping).map_err(err)?;
    std::fs::write(&dict_path, dict_json).map_err(|e| err(format!("ghi dict: {e}")))?;

    let stats = redactor.stats();
    Ok(RedactResult {
        input_path: path,
        redacted_path: redacted_path.display().to_string(),
        dict_path: dict_path.display().to_string(),
        total: stats.total,
        per_pattern: stats.per_pattern,
    })
}

#[tauri::command]
fn unredact_file(path: String, dict_path: String) -> Result<UnredactResult, String> {
    let file = PathBuf::from(&path);
    let dict_raw =
        std::fs::read_to_string(&dict_path).map_err(|e| err(format!("đọc dict: {e}")))?;
    let unredactor = Unredactor::from_dict_json(&dict_raw).map_err(err)?;

    let raw = std::fs::read_to_string(&file).map_err(|e| err(format!("đọc file: {e}")))?;
    let (restored, count) = unredactor.unredact(&raw);

    let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("out");
    let stem = stem.strip_suffix(".redacted").unwrap_or(stem);
    let out = match file.extension().and_then(|e| e.to_str()) {
        Some(e) => file.with_file_name(format!("{stem}.restored.{e}")),
        None => file.with_file_name(format!("{stem}.restored")),
    };
    std::fs::write(&out, restored).map_err(|e| err(format!("ghi output: {e}")))?;

    Ok(UnredactResult {
        input_path: path,
        restored_path: out.display().to_string(),
        restored_count: count,
        dict_size: unredactor.dict_size,
    })
}

/// Liệt kê 3 file ví dụ bundle trong app; copy vào ~/redact-gui-vi-du/ để ghi output được.
#[tauri::command]
fn load_example(app: tauri::AppHandle, name: String) -> Result<String, String> {
    let resource_dir = app.path().resource_dir().map_err(err)?;
    let src = resource_dir.join("examples").join(&name);
    let work_dir = dirs_home().join("redact-gui-vi-du");
    std::fs::create_dir_all(&work_dir).map_err(|e| err(format!("tạo thư mục: {e}")))?;
    let dst = work_dir.join(&name);
    std::fs::copy(&src, &dst).map_err(|e| err(format!("copy ví dụ: {e}")))?;
    Ok(dst.display().to_string())
}

#[tauri::command]
fn list_examples(app: tauri::AppHandle) -> Vec<String> {
    let resource_dir = match app.path().resource_dir() {
        Ok(d) => d,
        Err(_) => return vec![],
    };
    let dir = resource_dir.join("examples");
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| e.path().is_file())
                .filter_map(|e| e.file_name().into_string().ok())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

fn dirs_home() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            redact_file,
            unredact_file,
            load_example,
            list_examples
        ])
        .run(tauri::generate_context!())
        .expect("khởi động Redact GUI");
}
