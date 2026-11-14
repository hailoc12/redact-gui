//! redact-cli (Redact GUI CLI) — redact thông tin nhạy cảm Việt Nam trước khi đưa tài liệu lên AI.
//!
//! 100% xử lý local, zero telemetry: thư viện này không chứa bất kỳ
//! tính năng network nào.

pub mod brand;
pub mod formats;
pub mod patterns;
pub mod redactor;
pub mod unredact;
