//! Dispatch theo định dạng file: text/CSV redact như text thuần,
//! JSON redact từng value (giữ key + giữ valid JSON), PDF trích text
//! rồi redact (xuất .txt — không ghi lại PDF).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::Value;

use crate::redactor::Redactor;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    Text,
    Json,
    Pdf,
}

pub fn detect_format(path: &Path) -> Format {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("json") => Format::Json,
        Some("pdf") => Format::Pdf,
        _ => Format::Text,
    }
}

/// Đường dẫn output cho input path:
/// - Pdf  → `<stem>.redacted.txt` (chỉ xuất text đã redact)
/// - other → `<stem>.redacted<ext>` (ext rỗng thì không có đuôi)
pub fn output_path(path: &Path) -> PathBuf {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("out");
    match detect_format(path) {
        Format::Pdf => path.with_file_name(format!("{stem}.redacted.txt")),
        _ => {
            let ext = path.extension().and_then(|e| e.to_str());
            let name = match ext {
                Some(e) => format!("{stem}.redacted.{e}"),
                None => format!("{stem}.redacted"),
            };
            path.with_file_name(name)
        }
    }
}

fn redact_json_value(value: &mut Value, redactor: &mut Redactor) {
    match value {
        Value::String(s) => {
            let redacted = redactor.redact(s);
            *s = redacted;
        }
        Value::Array(items) => {
            for item in items {
                redact_json_value(item, redactor);
            }
        }
        Value::Object(map) => {
            for (_, v) in map.iter_mut() {
                redact_json_value(v, redactor);
            }
        }
        _ => {}
    }
}

/// Đọc file, redact theo đúng định dạng, trả về nội dung output.
pub fn redact_file(path: &Path, redactor: &mut Redactor) -> Result<String> {
    match detect_format(path) {
        Format::Json => {
            let raw =
                std::fs::read_to_string(path).with_context(|| format!("đọc {}", path.display()))?;
            match serde_json::from_str::<Value>(&raw) {
                Ok(mut value) => {
                    redact_json_value(&mut value, redactor);
                    Ok(serde_json::to_string_pretty(&value)?)
                }
                // JSON hỏng → fallback text, vẫn redact được
                Err(_) => Ok(redactor.redact(&raw)),
            }
        }
        Format::Pdf => {
            let text = pdf_extract::extract_text(path)
                .with_context(|| format!("trích text từ PDF {}", path.display()))?;
            Ok(redactor.redact(&text))
        }
        Format::Text => {
            let raw =
                std::fs::read_to_string(path).with_context(|| format!("đọc {}", path.display()))?;
            Ok(redactor.redact(&raw))
        }
    }
}

/// Dry-run: chỉ đọc + đếm, không cần xuất nội dung.
pub fn dry_run_file(path: &Path, redactor: &mut Redactor) -> Result<()> {
    match detect_format(path) {
        Format::Json => {
            let raw =
                std::fs::read_to_string(path).with_context(|| format!("đọc {}", path.display()))?;
            if let Ok(value) = serde_json::from_str::<Value>(&raw) {
                let mut v = value;
                // đếm bằng cách redact vào void: dùng clone stats? đơn giản: redact string values
                count_json(&mut v, redactor);
            } else {
                redactor.count_only(&raw);
            }
        }
        Format::Pdf => {
            let text = pdf_extract::extract_text(path)
                .with_context(|| format!("trích text từ PDF {}", path.display()))?;
            redactor.count_only(&text);
        }
        Format::Text => {
            let raw =
                std::fs::read_to_string(path).with_context(|| format!("đọc {}", path.display()))?;
            redactor.count_only(&raw);
        }
    }
    Ok(())
}

fn count_json(value: &mut Value, redactor: &mut Redactor) {
    match value {
        Value::String(s) => {
            let out = redactor.redact(s);
            *s = out;
        }
        Value::Array(items) => {
            for item in items {
                count_json(item, redactor);
            }
        }
        Value::Object(map) => {
            for (_, v) in map.iter_mut() {
                count_json(v, redactor);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patterns::BUILT_IN;

    fn write_tmp(name: &str, content: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("redact-tool-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(name);
        std::fs::write(&p, content).unwrap();
        p
    }

    #[test]
    fn json_values_redacted_keys_kept() {
        let patterns: Vec<_> = BUILT_IN.iter().filter(|p| p.enabled).cloned().collect();
        let mut r = Redactor::new(patterns).unwrap();
        let p = write_tmp(
            "t.json",
            r#"{"ho_ten":"Nguyễn Văn A","sdt":"0901234567","tuoi":30}"#,
        );
        let out = redact_file(&p, &mut r).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert!(v.get("ho_ten").is_some());
        assert_eq!(v["sdt"], "[SDT_1]");
        assert_eq!(v["tuoi"], 30);
    }

    #[test]
    fn text_file_redacted() {
        let patterns: Vec<_> = BUILT_IN.iter().filter(|p| p.enabled).cloned().collect();
        let mut r = Redactor::new(patterns).unwrap();
        let p = write_tmp("t.txt", "Email: a@b.com");
        let out = redact_file(&p, &mut r).unwrap();
        assert_eq!(out.trim(), "Email: [EMAIL_1]");
    }

    #[test]
    fn output_path_naming() {
        assert_eq!(
            output_path(Path::new("/tmp/hop-dong.pdf"))
                .file_name()
                .unwrap(),
            "hop-dong.redacted.txt"
        );
        assert_eq!(
            output_path(Path::new("a/b.csv")).file_name().unwrap(),
            "b.redacted.csv"
        );
        assert_eq!(
            output_path(Path::new("b/report.txt")).file_name().unwrap(),
            "report.redacted.txt"
        );
        assert_eq!(
            output_path(Path::new("noext")).file_name().unwrap(),
            "noext.redacted"
        );
    }
}
