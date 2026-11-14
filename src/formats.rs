//! Dispatch theo định dạng file:
//! - text/CSV/markdown… → regex thuần
//! - JSON                → redact từng value (giữ key, output valid JSON)
//! - XML                 → redact text node, giữ nguyên cấu trúc
//! - PDF                 → trích text → redact (xuất .txt, không ghi lại PDF)
//! - DOCX/XLSX/PPTX      → mở zip, redact text node XML bên trong, GIỮ ĐỊNH DẠNG
//! - DOC/XLS/PPT legacy  → từ chối kèm gợi ý chuyển đổi
//!
//! File zip-based tự ghi output (không trả String) — caller phân biệt qua
//! `Processed::Written` (đã ghi sẵn) và `Processed::Text` (text cần ghi).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::Value;

use crate::office;
use crate::redactor::Redactor;
use crate::unredact::Unredactor;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    Text,
    Json,
    Xml,
    Pdf,
    Ooxml,  // docx | xlsx | pptx (zip)
    Legacy, // doc | xls | ppt — không hỗ trợ giữ format
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
        Some("xml") => Format::Xml,
        Some("docx") | Some("xlsx") | Some("pptx") => Format::Ooxml,
        Some("doc") | Some("xls") | Some("ppt") => Format::Legacy,
        _ => Format::Text,
    }
}

/// Đuôi file hỗ trợ redact giữ format (dùng để lọc khi quét thư mục).
pub fn is_supported(path: &Path) -> bool {
    !matches!(detect_format(path), Format::Legacy)
}

/// Đường dẫn output cho input path:
/// - Pdf → `<stem>.redacted.txt` (chỉ xuất text đã redact)
/// - other → `<stem>.redacted.<ext>`
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
        Value::String(s) => *s = redactor.redact(s),
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

#[derive(Debug)]
pub enum Processed {
    /// Nội dung đã redact, caller ghi ra file
    Text(String),
    /// File zip đã được ghi trực tiếp vào `dst`
    Written(PathBuf),
}

/// Đọc file, redact theo đúng định dạng. Zip formats tự ghi output.
pub fn redact_file_to(path: &Path, redactor: &mut Redactor) -> Result<Processed> {
    match detect_format(path) {
        Format::Json => {
            let raw = read_text(path)?;
            match serde_json::from_str::<Value>(&raw) {
                Ok(mut value) => {
                    redact_json_value(&mut value, redactor);
                    Ok(Processed::Text(serde_json::to_string_pretty(&value)?))
                }
                Err(_) => Ok(Processed::Text(redactor.redact(&raw))),
            }
        }
        Format::Xml => {
            let raw = read_text(path)?;
            let out = office::redact_xml_text(&raw, &mut |s| redactor.redact(s))?;
            Ok(Processed::Text(out))
        }
        Format::Ooxml => {
            let dst = output_path(path);
            let mut cb = |s: &str| redactor.redact(s);
            office::redact_ooxml(path, &dst, &mut cb)?;
            Ok(Processed::Written(dst))
        }
        Format::Legacy => {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("doc")
                .to_ascii_lowercase();
            anyhow::bail!("{}", office::legacy_hint(&ext));
        }
        Format::Pdf => {
            let text = pdf_extract::extract_text(path)
                .with_context(|| format!("trích text từ PDF {}", path.display()))?;
            Ok(Processed::Text(redactor.redact(&text)))
        }
        Format::Text => Ok(Processed::Text(redactor.redact(&read_text(path)?))),
    }
}

/// Dry-run: chỉ đọc + đếm, không ghi file nào.
pub fn dry_run_file(path: &Path, redactor: &mut Redactor) -> Result<()> {
    match detect_format(path) {
        Format::Json => {
            let raw = read_text(path)?;
            if let Ok(mut value) = serde_json::from_str::<Value>(&raw) {
                redact_json_value(&mut value, redactor);
            } else {
                redactor.count_only(&raw);
            }
        }
        Format::Xml => {
            let raw = read_text(path)?;
            let _ = office::redact_xml_text(&raw, &mut |s| redactor.redact(s))?;
        }
        Format::Ooxml => {
            // chạy như redact thật nhưng vào file tạm (cần bóc zip để đếm)
            let tmp = std::env::temp_dir().join(format!(
                "redact-dryrun-{}-{}",
                std::process::id(),
                path.file_name().and_then(|n| n.to_str()).unwrap_or("f")
            ));
            let mut cb = |s: &str| redactor.redact(s);
            office::redact_ooxml(path, &tmp, &mut cb)?;
            let _ = std::fs::remove_file(&tmp);
        }
        Format::Legacy => { /* đếm là bỏ qua — caller đã lọc */ }
        Format::Pdf => {
            let text = pdf_extract::extract_text(path)
                .with_context(|| format!("trích text từ PDF {}", path.display()))?;
            redactor.count_only(&text);
        }
        Format::Text => {
            let raw = read_text(path)?;
            redactor.count_only(&raw);
        }
    }
    Ok(())
}

/// Unredact mọi định dạng (thay placeholder bằng dữ liệu gốc từ dict).
/// Zip formats ghi trực tiếp; còn lại trả text. Trả về (kết quả, số placeholder khôi phục).
pub fn unredact_file_to(path: &Path, unredactor: &Unredactor) -> Result<(Processed, usize)> {
    match detect_format(path) {
        Format::Ooxml => {
            let dst = unredact_output_path(path);
            let mut count = 0usize;
            {
                let mut cb = |s: &str| {
                    let (out, n) = unredactor.unredact(s);
                    count += n;
                    out
                };
                office::redact_ooxml(path, &dst, &mut cb)?;
            }
            Ok((Processed::Written(dst), count))
        }
        _ => {
            let raw = read_text(path)?;
            let (out, n) = unredactor.unredact(&raw);
            Ok((Processed::Text(out), n))
        }
    }
}

/// boocao.redacted.docx → boocao.restored.docx (strip .redacted như CLI text path)
pub fn unredact_output_path(path: &Path) -> PathBuf {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("out")
        .strip_suffix(".redacted")
        .unwrap_or_else(|| path.file_stem().and_then(|s| s.to_str()).unwrap_or("out"))
        .to_string();
    match path.extension().and_then(|e| e.to_str()) {
        Some(e) => path.with_file_name(format!("{stem}.restored.{e}")),
        None => path.with_file_name(format!("{stem}.restored")),
    }
}

fn read_text(path: &Path) -> Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("đọc {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patterns::BUILT_IN;
    use std::io::{Read, Write};

    fn write_tmp(name: &str, content: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("redact-cli-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(name);
        std::fs::write(&p, content).unwrap();
        p
    }

    fn all_patterns() -> Vec<crate::patterns::Pattern> {
        BUILT_IN.iter().filter(|p| p.enabled).cloned().collect()
    }

    #[test]
    fn json_values_redacted_keys_kept() {
        let mut r = Redactor::new(all_patterns()).unwrap();
        let p = write_tmp(
            "t.json",
            r#"{"ho_ten":"Nguyễn Văn A","sdt":"0901234567","tuoi":30}"#,
        );
        let out = match redact_file_to(&p, &mut r).unwrap() {
            Processed::Text(t) => t,
            _ => panic!("phải là Text"),
        };
        let v: Value = serde_json::from_str(&out).unwrap();
        assert!(v.get("ho_ten").is_some());
        assert_eq!(v["sdt"], "[SDT_1]");
        assert_eq!(v["tuoi"], 30);
    }

    #[test]
    fn text_file_redacted() {
        let mut r = Redactor::new(all_patterns()).unwrap();
        let p = write_tmp("t.txt", "Email: a@b.com");
        let out = match redact_file_to(&p, &mut r).unwrap() {
            Processed::Text(t) => t,
            _ => panic!("phải là Text"),
        };
        assert_eq!(out.trim(), "Email: [EMAIL_1]");
    }

    #[test]
    fn xml_text_nodes_redacted_structure_kept() {
        let mut r = Redactor::new(all_patterns()).unwrap();
        let xml = r#"<?xml version="1.0"?><hocso><can_bo sdt_goc="0901234567">Liên hệ 0901 234 567</can_bo><id>012345678901</id></hocso>"#;
        let p = write_tmp("t.xml", xml);
        let out = match redact_file_to(&p, &mut r).unwrap() {
            Processed::Text(t) => t,
            _ => panic!("phải là Text"),
        };
        // text node redacted
        assert!(out.contains("[SDT_1]"), "sđt trong text: {out}");
        assert!(out.contains("[CCCD_1]"), "cccd trong text: {out}");
        // attribute KHÔNG bị đổi + cấu trúc giữ nguyên
        assert!(
            out.contains(r#"sdt_goc="0901234567""#),
            "attr giữ nguyên: {out}"
        );
        assert!(out.contains("<can_bo") && out.contains("</hocso>"));
    }

    #[test]
    fn ooxml_docx_roundtrip() {
        // dựng docx tối thiểu: zip chứa [Content_Types].xml + word/document.xml
        let dir = std::env::temp_dir().join(format!("redact-cli-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let docx = dir.join("t.docx");
        {
            let f = std::fs::File::create(&docx).unwrap();
            let mut w = zip::ZipWriter::new(f);
            w.start_file(
                "[Content_Types].xml",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            w.write_all(b"<Types/>").unwrap();
            w.start_file(
                "word/document.xml",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            w.write_all(br#"<?xml version="1.0"?><w:document><w:body><w:p><w:r><w:t>Ho so cua ong Nguyen Van A, SDT 0901 234 567, MST 0101234567</w:t></w:r></w:p></w:body></w:document>"#)
                .unwrap();
            w.finish().unwrap();
        }
        // redact giữ format
        let mut r = Redactor::new(all_patterns()).unwrap();
        let out_path = match redact_file_to(&docx, &mut r).unwrap() {
            Processed::Written(p) => p,
            _ => panic!("phải là Written"),
        };
        assert_eq!(out_path, dir.join("t.redacted.docx"));
        // mở lại zip đọc nội dung
        let zf = std::fs::File::open(&out_path).unwrap();
        let mut z = zip::ZipArchive::new(zf).unwrap();
        let mut xml = String::new();
        z.by_name("word/document.xml")
            .unwrap()
            .read_to_string(&mut xml)
            .unwrap();
        assert!(
            xml.contains("[SDT_1]") && xml.contains("[MST_1]"),
            "text redacted: {xml}"
        );
        assert!(!xml.contains("0901 234 567"));
        assert!(xml.contains("<w:document>"), "cấu trúc XML giữ nguyên");
        // part khác copy nguyên vẹn
        let mut ct = String::new();
        z.by_name("[Content_Types].xml")
            .unwrap()
            .read_to_string(&mut ct)
            .unwrap();
        assert_eq!(ct, "<Types/>");

        // unredact trả về text gốc
        let u = Unredactor::from_dict_json(&serde_json::to_string(&r.mapping).unwrap()).unwrap();
        let (processed, _n) = unredact_file_to(&out_path, &u).unwrap();
        let restored = match processed {
            Processed::Written(p) => p,
            _ => panic!("phải là Written"),
        };
        let zf = std::fs::File::open(&restored).unwrap();
        let mut z = zip::ZipArchive::new(zf).unwrap();
        let mut xml2 = String::new();
        z.by_name("word/document.xml")
            .unwrap()
            .read_to_string(&mut xml2)
            .unwrap();
        assert!(
            xml2.contains("0901 234 567") && xml2.contains("0101234567"),
            "khôi phục: {xml2}"
        );
    }

    #[test]
    fn legacy_rejected_with_hint() {
        let mut r = Redactor::new(all_patterns()).unwrap();
        let p = write_tmp("t.doc", "binary");
        let err = redact_file_to(&p, &mut r).unwrap_err().to_string();
        assert!(err.contains("docx"), "phải gợi ý chuyển docx: {err}");
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
        assert_eq!(
            output_path(Path::new("hoso.docx")).file_name().unwrap(),
            "hoso.redacted.docx"
        );
        assert_eq!(
            unredact_output_path(Path::new("hoso.redacted.docx"))
                .file_name()
                .unwrap(),
            "hoso.restored.docx"
        );
    }
}
