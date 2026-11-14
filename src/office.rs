//! Redact file Office Open XML (docx/xlsx/pptx) và XML thuần — GIỮ NGUYÊN ĐỊNH DẠNG.
//!
//! docx/xlsx/pptx là ZIP chứa nhiều part XML. Mở zip, redact text node của các
//! part chứa nội dung (word/document.xml, xl/sharedStrings.xml, ppt/slides/*),
//! copy phần còn lại nguyên vẹn, ghi zip mới. XML parse bằng quick-xml nên chỉ
//! text bị đổi — cấu trúc, định dạng, công thức giữ nguyên.
//!
//! Định dạng legacy .doc/.xls/.ppt (OLE binary) KHÔNG hỗ trợ giữ format —
//! trả lỗi rõ ràng kèm gợi ý chuyển đổi (trung thực hơn là làm hỏng file).

use std::io::{Read, Write};
use std::path::Path;

use anyhow::{bail, Context, Result};
use quick_xml::events::Event;
use quick_xml::Reader;
use quick_xml::Writer;

/// Danh sách part XML trong OOXML chứa text người dùng cần redact.
fn is_content_part(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    // word/document.xml, xl/sharedStrings.xml, xl/worksheets/sN.xml (inlineStr),
    // ppt/slides/slideN.xml, ppt/notesSlides/* — bỏ qua theme/style để an toàn
    n.starts_with("word/document")
        || n.starts_with("word/header")   // đầu trang thường chứa số hiệu
        || n.starts_with("word/footer")
        || n.starts_with("xl/sharedstrings")
        || (n.starts_with("xl/worksheets/") && n.ends_with(".xml"))
        || n.starts_with("ppt/slides/")
        || n.starts_with("ppt/notesSlides/")
        || n.starts_with("ppt/tags/")
}

/// Redact text node của 1 XML string, giữ nguyên toàn bộ markup.
/// Trả về (xml mới, không cần — stats do caller cập nhật qua redactor).
pub fn redact_xml_text(xml: &str, redactor: &mut dyn FnMut(&str) -> String) -> Result<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false); // giữ whitespace đúng như gốc
    let mut writer = Writer::new(Vec::new());
    let mut buf = Vec::new();
    let mut skip_depth: usize = 0; // trong <w:instrText/> (field code) không redact
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let local = e.local_name();
                if skip_depth == 0
                    && (local.as_ref() == b"instrText" || local.as_ref() == b"script")
                {
                    skip_depth = 1; // bắt đầu vùng bỏ qua
                }
                writer.write_event(Event::Start(e))?;
            }
            Ok(Event::End(e)) => {
                skip_depth = skip_depth.saturating_sub(1);
                writer.write_event(Event::End(e))?;
            }
            Ok(Event::Text(t)) => {
                // bytes từ reader VẪN còn escaped (&amp;…) — từ chối escape lần 2
                let raw = t.into_inner();
                let text = String::from_utf8_lossy(&raw).into_owned();
                let out_text = if skip_depth == 0 {
                    redactor(&text)
                } else {
                    text
                };
                writer.write_event(Event::Text(quick_xml::events::BytesText::from_escaped(
                    out_text,
                )))?;
            }
            Ok(Event::CData(t)) => writer.write_event(Event::CData(t))?,
            Ok(Event::Comment(t)) => writer.write_event(Event::Comment(t))?,
            Ok(Event::Decl(t)) => writer.write_event(Event::Decl(t))?,
            Ok(Event::PI(t)) => writer.write_event(Event::PI(t))?,
            Ok(Event::DocType(t)) => writer.write_event(Event::DocType(t))?,
            Ok(Event::Empty(e)) => writer.write_event(Event::Empty(e))?,
            Ok(Event::GeneralRef(e)) => writer.write_event(Event::GeneralRef(e))?,
            Ok(Event::Eof) => break,
            Err(e) => return Err(e).context("parse XML"),
        }
        buf.clear();
    }
    Ok(String::from_utf8(writer.into_inner())?)
}

/// Redact 1 file OOXML (zip) → ghi zip mới giữ nguyên các part khác.
pub fn redact_ooxml(
    src: &Path,
    dst: &Path,
    redactor: &mut dyn FnMut(&str) -> String,
) -> Result<usize> {
    let file = std::fs::File::open(src).with_context(|| format!("mở {}", src.display()))?;
    let mut zip_in =
        zip::ZipArchive::new(file).context("đọc zip (file hỏng hoặc không phải OOXML?)")?;
    let out = std::fs::File::create(dst).with_context(|| format!("tạo {}", dst.display()))?;
    let mut zip_out = zip::ZipWriter::new(out);
    let mut touched = 0usize;

    for i in 0..zip_in.len() {
        let mut entry = zip_in.by_index(i)?;
        let name = entry.name().to_string();
        let opts: zip::write::SimpleFileOptions =
            zip::write::SimpleFileOptions::default().compression_method(entry.compression());
        zip_out
            .start_file(name.clone(), opts)
            .context("ghi zip entry")?;
        if is_content_part(&name) {
            let mut xml = String::new();
            entry.read_to_string(&mut xml).context("đọc XML part")?;
            let redacted = redact_xml_text(&xml, redactor)?;
            if redacted != xml {
                touched += 1;
            }
            zip_out.write_all(redacted.as_bytes())?;
        } else {
            std::io::copy(&mut entry, &mut zip_out)?;
        }
    }
    zip_out.finish().context("đóng zip")?;
    Ok(touched)
}

/// Gợi ý chuyển đổi cho định dạng legacy.
pub fn legacy_hint(ext: &str) -> String {
    format!(
        "định dạng legacy .{ext} (OLE binary) chưa hỗ trợ giữ format — \
         hãy chuyển sang .{} (mở bằng Word/Excel/PowerPoint rồi Save As, \
         hoặc LibreOffice: soffice --convert-to {}) rồi redact lại",
        modern(ext),
        modern(ext)
    )
}

fn modern(ext: &str) -> &'static str {
    match ext {
        "doc" => "docx",
        "xls" => "xlsx",
        "ppt" => "pptx",
        _ => "docx",
    }
}

/// Bắt các đuôi legacy trả lỗi gợi ý.
pub fn reject_legacy(ext: &str) -> Result<()> {
    if matches!(ext, "doc" | "xls" | "ppt") {
        bail!("{}", legacy_hint(ext));
    }
    Ok(())
}
