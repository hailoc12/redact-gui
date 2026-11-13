# Roadmap

## MVP (v0.1) — xong
- CLI redact pattern VN: cccd, cmnd, mst, sdt, email, tendn
- text/CSV/JSON/PDF, custom rules, mapping opt-in, dry-run
- Cross-compile macOS aarch64 + Windows x86_64 qua GitHub Actions

## v0.2 — GUI + Unredact — xong
- GUI Tauri (Rust-native), branding Lộc Đặng: 2 tab Redact / Unredact
- Redact: chọn file + mức (1 Liên hệ · 2 Nhận diện · 3 Tối đa) → file redacted + dict
- Unredact: file đã qua AI + dict → file khôi phục (*.restored.*)
- 3 file ví dụ bundle trong app, bấm là chạy thử được ngay
- CLI: subcommand `unredact`, flag `--level`

## Phase tiếp theo
- Chế độ AI-assist (`--ai-assist`): gửi bản ĐÃ redact lên LLM API (key user tự cấp),
  cảnh báo an toàn tường minh trước khi chạy
- Placeholder stable-hash (khớp cross-run, phục vụ so khớp nhiều batch)
- OCR PDF scan (tesseract binding, vẫn offline)
- Ký code-sign cho macOS (bỏ bước chuột phải → Open) và Windows (bỏ SmartScreen)
