# Roadmap

## MVP (v0.1) — xong
- CLI redact pattern VN: cccd, cmnd, mst, sdt, email, tendn
- text/CSV/JSON/PDF, custom rules, mapping opt-in, dry-run
- Cross-compile macOS aarch64 + Windows x86_64 qua GitHub Actions

## Phase 2
- Chế độ AI-assist (`--ai-assist`): gửi bản ĐÃ redact lên LLM API (key user tự cấp),
  cảnh báo an toàn tường minh trước khi chạy
- Placeholder stable-hash (khớp cross-run, phục vụ so khớp nhiều batch)
- OCR PDF scan (tesseract binding, vẫn offline)

## Phase 3
- GUI Tauri (Rust-native): kéo-thả file, xem diff trước/sau, chọn pattern bằng checkbox
- Giao diện tiếng Việt + English
