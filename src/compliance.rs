//! Khai loại tài liệu & tra cứu quy định — nội dung nghiên cứu từ bài
//! "Che dữ liệu trước khi đưa lên AI thì luật cho phép đến đâu" (locdang.com,
//! research 14/11/2026, 10 văn bản QPPL + 6 chuẩn quốc tế).
//!
//! 5 nhóm tài liệu (Luật 117/2025, Luật Dữ liệu 60/2024, Luật 91/2025,
//! Luật SHTT, luật ngành) → mỗi nhóm một hành động: cấm / cần phê duyệt /
//! redact rồi dùng được.

use serde::Serialize;

/// Link bài nghiên cứu gốc (tóm tắt trọng yếu + nguồn đầy đủ).
pub const RESEARCH_URL: &str =
    "https://locdang.com/che-du-lieu-truoc-khi-ua-len-ai-thi-luat-cho-phep-en-au/";

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
pub enum DocClass {
    /// Bí mật nhà nước (Tuyệt mật / Tối mật / Mật) — Luật 117/2025/QH15
    SecretState,
    /// Dữ liệu cốt lõi, quan trọng, dùng riêng — Luật Dữ liệu 60/2024/QH15
    CoreData,
    /// Dữ liệu cá nhân (cơ bản + nhạy cảm) — Luật 91/2025/QH15 + NĐ 13/2023
    PersonalData,
    /// Bí mật kinh doanh / công nghệ — Luật SHTT Đ.84, 127; BLL Đ.21
    TradeSecret,
    /// Bí mật nghề nghiệp, đời tư — Luật KCB Đ.69, KTĐL Đ.12, BLDS Đ.38
    ProfessionalSecret,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
pub enum Action {
    /// Dừng tuyệt đối — không redact, không đưa lên AI
    Block,
    /// Cần phê duyệt cấp có thẩm quyền + cam kết trách nhiệm
    RequireApproval,
    /// Được dùng sau khi redact đạt chuẩn khử nhận dạng
    AllowedAfterRedact,
}

#[derive(Clone, Serialize)]
pub struct ClassInfo {
    pub id: &'static str,
    pub label: &'static str,
    pub legal_basis: &'static str,
    pub rule: &'static str,
    pub action: Action,
    pub action_label: &'static str,
    pub source: &'static str,
}

pub const CLASSES: &[ClassInfo] = &[
    ClassInfo {
        id: "secret_state",
        label: "Bí mật nhà nước (Tuyệt mật / Tối mật / Mật)",
        legal_basis: "Luật Bảo vệ bí mật nhà nước 117/2025/QH15 (thay Luật 29/2018 từ 01/3/2026) + Nghị định 63/2026",
        rule: "Không bao giờ đưa lên AI — kể cả đã che. Chưa giải mật chính thức (hội đồng, Điều 22) thì bản trích vẫn chứa bí mật nhà nước. Kể cả số hóa văn bản giấy đã cần thẩm quyền.",
        action: Action::Block,
        action_label: "DỪNG — ranh giới đỏ tuyệt đối",
        source: "https://thuvienphapluat.vn/van-ban/Bo-may-hanh-chinh/Luat-Bao-ve-bi-mat-nha-nuoc-2025-so-117-2025-QH15-685866.aspx",
    },
    ClassInfo {
        id: "core_data",
        label: "Dữ liệu cốt lõi, quan trọng, dùng riêng",
        legal_basis: "Luật Dữ liệu 60/2024/QH15",
        rule: "Chỉ theo danh mục hạn chế của cơ quan quản lý dữ liệu. Ngoài danh mục hạn chế thì xử lý như dữ liệu thường.",
        action: Action::RequireApproval,
        action_label: "Cần phê duyệt — hỏi đơn vị quản lý dữ liệu",
        source: "https://xaydungchinhsach.chinhphu.vn/toan-van-luat-du-lieu-119250226145839949.htm",
    },
    ClassInfo {
        id: "personal_data",
        label: "Dữ liệu cá nhân (cơ bản và nhạy cảm)",
        legal_basis: "Luật Bảo vệ dữ liệu cá nhân 91/2025/QH15 (hiệu lực 01/01/2026) + Nghị định 13/2023",
        rule: "Được dùng khi đã khử nhận dạng đạt chuẩn — khoản 1 Điều 2: \"dữ liệu cá nhân sau khi khử nhận dạng không còn là dữ liệu cá nhân\". Chuẩn cao hơn \"xoá tên\": không thể giúp xác định lại chủ thể. Ngoại lệ cá nhân: tự chuyển dữ liệu của chính mình (Đ.20 khoản 6).",
        action: Action::AllowedAfterRedact,
        action_label: "Redact đạt chuẩn khử nhận dạng rồi dùng được",
        source: "https://datafiles.chinhphu.vn/cpp/files/vbpq/2025/7/91qh.signed.pdf",
    },
    ClassInfo {
        id: "trade_secret",
        label: "Bí mật kinh doanh, bí mật công nghệ",
        legal_basis: "Luật Sở hữu trí tuệ (Điều 84, 127) + Bộ luật Lao động (khoản 2 Điều 21)",
        rule: "Theo chính sách nội bộ của doanh nghiệp — tổ chức tự thiết kế quy trình.",
        action: Action::RequireApproval,
        action_label: "Cần phê duyệt — theo chính sách nội bộ",
        source: "https://luatvietnam.vn/",
    },
    ClassInfo {
        id: "professional_secret",
        label: "Bí mật nghề nghiệp, bí mật đời tư",
        legal_basis: "Luật Khám chữa bệnh (Điều 69) + Luật Kiểm toán độc lập (Điều 12) + Bộ luật Dân sự (Điều 38) + Hiến pháp (Điều 21)",
        rule: "Khi có đồng ý bằng văn bản của người bệnh, khách hàng, người liên quan. Hồ sơ bệnh án lưu giữ và giữ bí mật theo luật ngành.",
        action: Action::RequireApproval,
        action_label: "Cần phê duyệt — đồng ý bằng văn bản",
        source: "https://datafiles.chinhphu.vn/cpp/files/vbpq/2023/02/15luat.signed.pdf",
    },
];

pub fn class_by_id(id: &str) -> Option<&'static ClassInfo> {
    CLASSES.iter().find(|c| c.id == id)
}

/// Hành động tương ứng với nhóm tài liệu — dùng cho gate trong GUI/CLI.
pub fn action_for(id: &str) -> Option<(Action, &'static str)> {
    class_by_id(id).map(|c| (c.action, c.action_label))
}

/// Dấu hiệu nhận diện tài liệu mật — quét text thô (đã trích từ file).
#[derive(Serialize, Debug, PartialEq)]
pub struct Marker {
    pub phrase: &'static str,
    pub note: &'static str,
}

const MARKERS: &[(&str, &str)] = &[
    (
        "TUYỆT MẬT",
        "Độ mật cao nhất theo Luật 117/2025 — DỪNG tuyệt đối",
    ),
    ("TỐI MẬT", "Độ mật theo Luật 117/2025 — DỪNG tuyệt đối"),
    ("CẤP TỐI MẬT", "Độ mật theo Luật 117/2025 — DỪNG tuyệt đối"),
    (
        "cấm chia sẻ",
        "Dấu hiệu hạn chế phổ biến — kiểm tra phê duyệt",
    ),
    (
        "Cấm sao chép",
        "Dấu hiệu hạn chế phổ biến — kiểm tra phê duyệt",
    ),
    (
        "bí mật nội bộ",
        "Chính sách nội bộ — cần phê duyệt theo quy định tổ chức",
    ),
    (
        "BÍ MẬT NỘI BỘ",
        "Chính sách nội bộ — cần phê duyệt theo quy định tổ chức",
    ),
    ("không được phổ biến", "Dấu hiệu hạn chế phổ biến"),
    ("lưu hành nội bộ", "Hạn chế phạm vi — kiểm tra phê duyệt"),
    ("CONFIDENTIAL", "International confidentiality marker"),
    ("TOP SECRET", "Highest classification marker — STOP"),
    ("INTERNAL ONLY", "Internal use restriction marker"),
];

/// Quét dấu hiệu mật trong text. Trả về danh sách dấu hiệu tìm thấy.
pub fn scan_markers(text: &str) -> Vec<Marker> {
    let mut found = Vec::new();
    for (phrase, note) in MARKERS {
        if text.to_lowercase().contains(&phrase.to_lowercase()) {
            found.push(Marker { phrase, note });
        }
    }
    found
}

/// Trích text thô từ file để scan dấu hiệu (đủ cho mọi format hỗ trợ).
pub fn extract_text_for_scan(path: &std::path::Path) -> anyhow::Result<String> {
    match crate::formats::detect_format(path) {
        crate::formats::Format::Ooxml => {
            // zip: nối text của các part nội dung
            let file = std::fs::File::open(path)?;
            let mut zip = zip::ZipArchive::new(file)?;
            let mut out = String::new();
            for i in 0..zip.len() {
                let mut entry = zip.by_index(i)?;
                let name = entry.name().to_string();
                if crate::office::is_content_part_pub(&name) {
                    use std::io::Read;
                    let mut s = String::new();
                    let _ = entry.read_to_string(&mut s);
                    out.push_str(&s);
                }
            }
            Ok(out)
        }
        crate::formats::Format::Pdf => Ok(pdf_extract::extract_text(path).unwrap_or_default()),
        _ => Ok(std::fs::read_to_string(path).unwrap_or_default()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classification_matrix() {
        assert_eq!(action_for("secret_state").unwrap().0, Action::Block);
        assert_eq!(action_for("core_data").unwrap().0, Action::RequireApproval);
        assert_eq!(
            action_for("personal_data").unwrap().0,
            Action::AllowedAfterRedact
        );
        assert_eq!(
            action_for("trade_secret").unwrap().0,
            Action::RequireApproval
        );
        assert_eq!(
            action_for("professional_secret").unwrap().0,
            Action::RequireApproval
        );
        assert!(action_for("không_tồn_tại").is_none());
        assert_eq!(CLASSES.len(), 5);
    }

    #[test]
    fn every_class_has_source_and_legal_basis() {
        for c in CLASSES {
            assert!(c.source.starts_with("http"), "{} thiếu nguồn", c.id);
            assert!(
                c.legal_basis.contains("Luật") || c.legal_basis.contains("Bộ luật"),
                "{} thiếu cơ sở pháp lý",
                c.id
            );
            assert!(!c.rule.is_empty());
        }
    }

    #[test]
    fn scan_finds_vn_markers() {
        let m = scan_markers("Văn bản này ghi rõ TUYỆT MẬT, không được phổ biến");
        assert!(m.iter().any(|x| x.phrase == "TUYỆT MẬT"));
        assert!(m.iter().any(|x| x.phrase == "không được phổ biến"));
    }

    #[test]
    fn scan_case_insensitive_and_clean_text_passes() {
        let m = scan_markers("Họp thường kỳ, biên bản công khai nội dung trao đổi");
        assert!(m.is_empty(), "văn bản thường không bị gắn cờ: {m:?}");
        let m2 = scan_markers("đánh dấu confidential ở footer");
        assert!(m2.iter().any(|x| x.phrase == "CONFIDENTIAL"));
    }

    #[test]
    fn scan_file_end_to_end() {
        let dir = std::env::temp_dir().join(format!("redact-comp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("vb.txt");
        std::fs::write(&p, "BIÊN BẢN\nTUYỆT MẬT\nSĐT 0901234567").unwrap();
        let text = extract_text_for_scan(&p).unwrap();
        assert!(scan_markers(&text).iter().any(|x| x.phrase == "TUYỆT MẬT"));
    }
}
