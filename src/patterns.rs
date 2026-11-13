//! Built-in redaction patterns cho dữ liệu nhạy cảm Việt Nam.
//!
//! Thứ tự trong `BUILT_IN` quyết định độ ưu tiên khi nhiều pattern
//! khớp cùng một vị trí (regex crate chọn alternative trái nhất trước),
//! nên pattern dài/cụ thể hơn phải đứng trước pattern ngắn/chung hơn.

use serde::Deserialize;

/// Cách chuẩn hoá giá trị trước khi tra mapping (để "0901 234 567"
/// và "0901234567" maps về cùng một placeholder).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Normalize {
    /// Giữ nguyên giá trị match
    #[default]
    None,
    /// Chỉ lấy chữ số
    Digits,
    /// Chuẩn hoá số điện thoại: bỏ dấu +, chuyển đầu số 84 → 0, bỏ separator
    Phone,
}

#[derive(Clone, Debug)]
pub struct Pattern {
    /// id dùng trên CLI (--patterns cccd,sdt,...)
    pub id: &'static str,
    /// nhãn trong placeholder [LABEL_n]
    pub label: &'static str,
    /// tên hiển thị
    pub name: &'static str,
    pub description: &'static str,
    pub regex: &'static str,
    pub enabled: bool,
    pub normalize: Normalize,
}

pub const BUILT_IN: &[Pattern] = &[
    Pattern {
        id: "cccd",
        label: "CCCD",
        name: "CCCD (thẻ căn cước 12 số)",
        description: "12 chữ số, có thể ngăn cách bằng dấu chấm/khoảng trắng/gạch ngang",
        regex: r"(?x)
            \b
            0\d{3}[\s.-]?\d{4}[\s.-]?\d{4}
            \b",
        enabled: true,
        normalize: Normalize::Digits,
    },
    Pattern {
        id: "cmnd",
        label: "CMND",
        name: "CMND (9 số liền, mẫu cũ)",
        description: "9 chữ số liền nhau — cố ý KHÔNG nhận dạng có separator để tránh bắt nhầm số tiền (1.234.567.890). Có thể bắt nhầm số hiệu văn bản, cân nhắc tắt nếu nhiễu",
        regex: r"(?x)
            \b
            \d{9}
            \b",
        enabled: true,
        normalize: Normalize::Digits,
    },
    Pattern {
        id: "sdt",
        label: "SDT",
        name: "Số điện thoại VN",
        description: "Đầu số 0/84/+84, ngách 2-9 (di động 10 số, cố định 10-11 số)",
        regex: r"(?x)
            (?: \+84 | \b84 | \b0 )
            [\s.-]?
            [2-9]\d
            (?: [\s.-]? \d ){7,8}
            \b",
        enabled: true,
        normalize: Normalize::Phone,
    },
    Pattern {
        id: "mst",
        label: "MST",
        name: "Mã số thuế",
        description: "10 chữ số, có thể kèm hậu tố -XXX (mã đơn vị con). Lưu ý: MST trùng dạng số điện thoại sẽ bị gán nhãn SDT — vẫn được redact đủ",
        regex: r"(?x)
            \b
            \d{10} (?: - \d{3} )?
            \b",
        enabled: true,
        normalize: Normalize::Digits,
    },
    Pattern {
        id: "email",
        label: "EMAIL",
        name: "Địa chỉ email",
        description: "Địa chỉ email chuẩn RFC-lite",
        regex: r"(?x)
            \b
            [A-Za-z0-9._%+-]+ @ [A-Za-z0-9.-]+ \. [A-Za-z]{2,}
            \b",
        enabled: true,
        normalize: Normalize::None,
    },
    Pattern {
        id: "tendn",
        label: "TENDN",
        name: "Tên doanh nghiệp / tổ chức",
        description: "Chuỗi bắt đầu bằng Công ty/Cty/DNTN/HTX... đến hết cụm (tối đa 80 ký tự)",
        regex: r"(?x)
            \b
            (?: Công \s ty | Cty | CTy | Doanh \s nghiệp \s tư \s nhân | DNTN | Hợp \s tác \s xã | HTX )
            [^\n,;]{0,80}",
        enabled: true,
        normalize: Normalize::None,
    },
];

/// Rule tùy chỉnh load từ file JSON (mảng các object).
#[derive(Deserialize, Debug)]
pub struct CustomRule {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub name: Option<String>,
    pub regex: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matches(id: &str, text: &str) -> Option<String> {
        let p = BUILT_IN.iter().find(|p| p.id == id).unwrap();
        let re = regex::Regex::new(&format!("(?x){}", p.regex)).ok()?;
        re.find(text).map(|m| m.as_str().to_string())
    }

    #[test]
    fn cccd_matches_12_digits() {
        assert_eq!(
            matches("cccd", "CCCD 012345678901 ok").as_deref(),
            Some("012345678901")
        );
        assert_eq!(
            matches("cccd", "0123 4567 8901").as_deref(),
            Some("0123 4567 8901")
        );
        // 13 chữ số: không phải CCCD
        assert!(matches("cccd", "0123456789012").is_none());
    }

    #[test]
    fn cmnd_matches_9_digits() {
        assert_eq!(
            matches("cmnd", "CMND 012345678").as_deref(),
            Some("012345678")
        );
        assert!(matches("cmnd", "0123456789").is_none());
        // số tiền có separator ngàn KHÔNG được bắt nhầm là CMND
        assert!(matches("cmnd", "1.234.567.890 đồng").is_none());
        assert!(matches("cmnd", "1 234 567 890 đồng").is_none());
    }

    #[test]
    fn mst_matches_10_digits_with_optional_suffix() {
        assert_eq!(
            matches("mst", "MST 0101234567").as_deref(),
            Some("0101234567")
        );
        assert_eq!(
            matches("mst", "0101234567-001").as_deref(),
            Some("0101234567-001")
        );
    }

    #[test]
    fn sdt_matches_vn_phone_formats() {
        assert_eq!(matches("sdt", "0901234567").as_deref(), Some("0901234567"));
        assert_eq!(
            matches("sdt", "0901 234 567").as_deref(),
            Some("0901 234 567")
        );
        assert_eq!(
            matches("sdt", "+84901234567").as_deref(),
            Some("+84901234567")
        );
        assert_eq!(
            matches("sdt", "024.3826.1234").as_deref(),
            Some("024.3826.1234")
        );
        // 10 số nhưng ngách 0/1: không phải SĐT VN
        assert!(matches("sdt", "0101234567").is_none());
    }

    #[test]
    fn email_matches_basic() {
        assert_eq!(
            matches("email", "lienhe: nguyen.van@domain.com.vn").as_deref(),
            Some("nguyen.van@domain.com.vn")
        );
        assert!(matches("email", "khong-co-at-sign").is_none());
    }

    #[test]
    fn tendn_matches_company_prefix() {
        assert!(matches("tendn", "Công ty TNHH Linh An Solutions").is_some());
        assert!(matches("tendn", "Công ty Cổ phần Đầu tư ABC").is_some());
        assert!(matches("tendn", "Doanh nghiệp tư nhân Hoa Hồng").is_some());
        // chỉ dừng ở newline / dấu câu câu
        let m = matches("tendn", "Công ty TNHH ABC, Hà Nội").unwrap();
        assert_eq!(m, "Công ty TNHH ABC");
    }
}
