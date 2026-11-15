//! Redact GUI — Tauri v2. Toàn bộ xử lý nằm trong lib redact-cli (local, offline).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};

use redact_cli::patterns::{ids_for_level, BUILT_IN};
use redact_cli::redactor::{MapEntry, Redactor};
use redact_cli::unredact::Unredactor;
use serde::Serialize;
use tauri::Manager;

#[derive(Serialize)]
struct FileOutcome {
    input: String,
    output: String,
    total: usize,
    error: Option<String>,
}

#[derive(Serialize)]
struct RedactResult {
    input_path: String,
    redacted_path: String,
    dict_path: String,
    total: usize,
    per_pattern: Vec<(String, usize)>,
    is_folder: bool,
    files_ok: usize,
    files_skipped: usize,
    skipped: Vec<FileOutcome>,
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
fn redact_file(path: String, level: u8, doc_class: String) -> Result<RedactResult, String> {
    let file = PathBuf::from(&path);
    let ids = ids_for_level(level);
    let pattern_list: Vec<_> = BUILT_IN
        .iter()
        .filter(|p| ids.contains(&p.id))
        .cloned()
        .collect();
    let mut redactor = Redactor::new(pattern_list).map_err(err)?;

    if file.is_dir() {
        return redact_folder(&file, &mut redactor, &doc_class);
    }

    let processed = redact_cli::formats::redact_file_to(&file, &mut redactor).map_err(err)?;
    let (redacted_path, dict_path) = output_paths(&file);
    match processed {
        redact_cli::formats::Processed::Text(content) => {
            std::fs::write(&redacted_path, content).map_err(|e| err(format!("ghi output: {e}")))?;
        }
        redact_cli::formats::Processed::Written(p) => {
            // zip formats đã ghi cạnh file gốc — chuyển vào vị trí chuẩn
            if p != redacted_path {
                let _ = std::fs::rename(&p, &redacted_path);
            }
        }
    }

    let mapping: Vec<MapEntry> = redactor.mapping.clone();
    let dict_json = serde_json::to_string_pretty(&mapping).map_err(err)?;
    std::fs::write(&dict_path, dict_json).map_err(|e| err(format!("ghi dict: {e}")))?;

    redact_cli::orglog::log_local("redact", &file.display().to_string(), &doc_class);
    redact_cli::orglog::log_activity("redact", &file.display().to_string());

    let stats = redactor.stats();
    Ok(RedactResult {
        input_path: path,
        redacted_path: redacted_path.display().to_string(),
        dict_path: dict_path.display().to_string(),
        total: stats.total,
        per_pattern: stats.per_pattern,
        is_folder: false,
        files_ok: 1,
        files_skipped: 0,
        skipped: vec![],
    })
}

/// Redact cả thư mục (đệ quy): mirror cấu trúc vào <tên>_redacted/ + 1 dict chung.
fn redact_folder(
    root: &Path,
    redactor: &mut Redactor,
    doc_class: &str,
) -> Result<RedactResult, String> {
    let folder_name = root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("folder");
    let out_root = root.with_file_name(format!("{folder_name}_redacted"));
    std::fs::create_dir_all(&out_root).map_err(|e| err(format!("tạo thư mục output: {e}")))?;

    let mut files_ok = 0usize;
    let mut skipped: Vec<FileOutcome> = vec![];
    let mut walker = walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok());
    let mut entries: Vec<PathBuf> = walker
        .by_ref()
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().to_path_buf())
        .collect();
    entries.sort();

    for f in entries {
        if !redact_cli::formats::is_supported(&f) {
            let ext = f
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_string();
            skipped.push(FileOutcome {
                input: f.display().to_string(),
                output: String::new(),
                total: 0,
                error: Some(format!(
                    "định dạng legacy .{ext} — chuyển sang dạng mới rồi redact lại"
                )),
            });
            continue;
        }
        let rel = f.strip_prefix(root).unwrap_or(f.as_os_str().as_ref());
        let dst = out_root.join(rel);
        if let Some(parent) = dst.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match redact_cli::formats::redact_file_to(&f, redactor) {
            Ok(redact_cli::formats::Processed::Text(content)) => {
                if std::fs::write(&dst, content).is_ok() {
                    files_ok += 1;
                }
            }
            Ok(redact_cli::formats::Processed::Written(_tmp)) => {
                // zip đã ghi cạnh file gốc — move vào mirror
                let expected = redact_cli::formats::output_path(&f);
                let moved = std::fs::rename(&expected, &dst).is_ok();
                if moved {
                    files_ok += 1;
                } else {
                    let _ = std::fs::copy(&expected, &dst);
                    files_ok += 1;
                }
            }
            Err(e) => skipped.push(FileOutcome {
                input: f.display().to_string(),
                output: String::new(),
                total: 0,
                error: Some(format!("{e:#}")),
            }),
        }
    }

    let dict_path = out_root.join("dict.json");
    let dict_json = serde_json::to_string_pretty(&redactor.mapping).map_err(err)?;
    std::fs::write(&dict_path, dict_json).map_err(|e| err(format!("ghi dict: {e}")))?;

    redact_cli::orglog::log_local("redact_folder", &root.display().to_string(), doc_class);
    redact_cli::orglog::log_activity("redact_folder", &root.display().to_string());

    let stats = redactor.stats();
    Ok(RedactResult {
        input_path: root.display().to_string(),
        redacted_path: out_root.display().to_string(),
        dict_path: dict_path.display().to_string(),
        total: stats.total,
        per_pattern: stats.per_pattern,
        is_folder: true,
        files_ok,
        files_skipped: skipped.len(),
        skipped,
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

    redact_cli::orglog::log_local("unredact", &file.display().to_string(), "-");
    redact_cli::orglog::log_activity("unredact", &file.display().to_string());

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

#[tauri::command]
fn list_classes() -> Vec<redact_cli::compliance::ClassInfo> {
    redact_cli::compliance::CLASSES.to_vec()
}

#[derive(Serialize)]
struct ScanResult {
    markers: Vec<redact_cli::compliance::Marker>,
    files_scanned: usize,
}

/// Auto-scan dấu hiệu tài liệu mật (F3): quét file hoặc toàn thư mục (cap 200 file).
#[tauri::command]
fn scan_secret(path: String) -> ScanResult {
    let p = PathBuf::from(&path);
    let mut files: Vec<PathBuf> = Vec::new();
    if p.is_dir() {
        for e in walkdir::WalkDir::new(&p).into_iter().filter_map(|e| e.ok()) {
            if e.file_type().is_file() && files.len() < 200 {
                files.push(e.into_path());
            }
        }
    } else {
        files.push(p);
    }
    let mut markers = Vec::new();
    let mut scanned = 0usize;
    for f in &files {
        if let Ok(text) = redact_cli::compliance::extract_text_for_scan(f) {
            scanned += 1;
            for m in redact_cli::compliance::scan_markers(&text) {
                if !markers
                    .iter()
                    .any(|x: &redact_cli::compliance::Marker| x.phrase == m.phrase)
                {
                    markers.push(m);
                }
            }
            if !markers.is_empty() && markers.iter().any(|m| m.phrase.contains("MẬT")) {
                break; // dấu hiệu độ mật cao nhất — dừng sớm đủ bằng chứng
            }
        }
    }
    ScanResult {
        markers,
        files_scanned: scanned,
    }
}

#[derive(Serialize)]
struct LogInfo {
    entries: Vec<redact_cli::orglog::LocalEvent>,
    log_path: String,
    machine: String,
    org_feature: bool,
    org_endpoint: bool,
}

#[tauri::command]
fn get_log_info() -> LogInfo {
    let (feature, endpoint) = redact_cli::orglog::org_mode_status();
    LogInfo {
        entries: redact_cli::orglog::read_local_log(),
        log_path: redact_cli::orglog::local_log_path().display().to_string(),
        machine: redact_cli::orglog::machine_fingerprint(),
        org_feature: feature,
        org_endpoint: endpoint,
    }
}

#[tauri::command]
fn clear_log() -> bool {
    std::fs::remove_file(redact_cli::orglog::local_log_path()).is_ok()
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            redact_file,
            unredact_file,
            load_example,
            list_examples,
            list_classes,
            scan_secret,
            get_log_info,
            clear_log
        ])
        .run(tauri::generate_context!())
        .expect("khởi động Redact GUI");
}
