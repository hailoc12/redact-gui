//! redact-cli (Redact GUI CLI) — redact thông tin nhạy cảm Việt Nam trước khi đưa tài liệu lên AI.
//!
//! 100% xử lý local, zero telemetry: thư viện này KHÔNG chứa tính năng
//! network nào trong bản build mặc định. Riêng bản tổ chức (org-mode) bật
//! cargo feature `org-logging` để gửi activity log về API nội bộ.

pub mod brand;
pub mod compliance;
pub mod formats;
pub mod office;
pub mod orglog;
pub mod patterns;
pub mod redactor;
pub mod unredact;
