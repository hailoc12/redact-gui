//! Brand styling cho CLI output — Linh An Solutions design system v2.1.
//!
//! Palette P2 Đỏ: đỏ #C22620 chỉ dùng làm điểm nhấn (heading/số liệu),
//! body là đen than #1F1F1F, meta là xám muted #5F6368, rule vàng #D4A017.
//! Tôn trọng NO_COLOR và tự tắt màu khi output không phải terminal.

use std::io::IsTerminal;

/// Đỏ Lộc Đặng #C22620 — heading, số liệu nổi
pub const RED: (u8, u8, u8) = (0xC2, 0x26, 0x20);
/// Đỏ Sẫm #9E1B16 — cảnh báo mức cao
pub const RED_DARK: (u8, u8, u8) = (0x9E, 0x1B, 0x16);
/// Xám Muted #5F6368 — meta, caption
pub const MUTED: (u8, u8, u8) = (0x5F, 0x63, 0x68);
/// Vàng Rule #D4A017 — kẻ phân cách
pub const YELLOW: (u8, u8, u8) = (0xD4, 0xA0, 0x17);

pub struct Style {
    enabled: bool,
}

impl Style {
    pub fn new(no_color: bool) -> Self {
        Self {
            enabled: !no_color
                && std::env::var_os("NO_COLOR").is_none()
                && std::io::stdout().is_terminal(),
        }
    }

    fn wrap(&self, text: &str, rgb: (u8, u8, u8), bold: bool) -> String {
        if !self.enabled || text.is_empty() {
            return text.to_string();
        }
        let (r, g, b) = rgb;
        let prefix = if bold {
            format!("\x1b[1;38;2;{r};{g};{b}m")
        } else {
            format!("\x1b[38;2;{r};{g};{b}m")
        };
        format!("{prefix}{text}\x1b[0m")
    }

    /// Heading / số liệu nổi — đỏ, đậm
    pub fn heading(&self, text: &str) -> String {
        self.wrap(text, RED, true)
    }

    /// Chữ phụ / meta — xám muted
    pub fn muted(&self, text: &str) -> String {
        self.wrap(text, MUTED, false)
    }

    /// Số thay thế — đỏ thường
    pub fn number(&self, text: &str) -> String {
        self.wrap(text, RED, false)
    }

    /// Cảnh báo — đỏ sẫm
    pub fn warning(&self, text: &str) -> String {
        self.wrap(text, RED_DARK, true)
    }

    /// Dòng kẻ rule — vàng (dùng tối đa 1 lần mỗi khối)
    pub fn rule(&self, width: usize) -> String {
        self.wrap(&"─".repeat(width), YELLOW, false)
    }
}
