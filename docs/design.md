# Design — redact-tool

## Kiến trúc

```
src/                    core library + CLI
├── main.rs      CLI (clap): redact (mặc định) + subcommand unredact, --level 1-3
├── lib.rs       export module cho test + cho GUI dùng lại
├── patterns.rs  pattern có sẵn + custom rule JSON + ids_for_level()
├── redactor.rs  engine: combined regex, mapping (MapEntry serialize), stats
├── unredact.rs  engine khôi phục: dict JSON → regex union placeholder (dài trước)
├── formats.rs   dispatch text/CSV · JSON · PDF → output
└── brand.rs     màu CLI theo design system Linh An v2.1
gui/                    Tauri v2 app (redact-tool-gui)
├── src/main.rs  commands: redact_file / unredact_file / load_example / list_examples
├── ui/          vanilla HTML/CSS/JS — branding Lộc Đặng (Manrope/Inter, palette P2)
└── tauri.conf.json — bundle resources: assets/examples (3 file ví dụ)
assets/examples/       3 file ví dụ (txt · json · pdf) load thử ngay trong GUI
```

GUI không chứa logic xử lý — mọi thao tác gọi sang lib `redact-tool` (một nguồn
truth, test bằng cargo test như CLI).

## Quyết định chính

| Quyết định | Lựa chọn | Lý do |
|---|---|---|
| Ghép pattern thành 1 regex `(p1)|(p2)|...` | regex crate leftmost-first | cùng vị trí → pattern cụ thể hơn thắng (CCCD trước SDT, không ăn nhầm substring) |
| Placeholder `[LABEL_n]` | counter per-label + map theo key chuẩn hoá | cùng giá trị → cùng placeholder; "0901 234 567" ≡ "+84901234567" ≡ "0901234567" |
| Chuẩn hoá key | digits-only / phone-normalize | tránh cùng 1 số bị 2 placeholder chỉ vì khác cách viết |
| JSON | walk serde_json::Value, chỉ redact value | giữ key + valid JSON output |
| PDF | pdf-extract trích text, xuất .txt | ghi lại PDF là phức tạp và dễ hỏng layout; text redact là 99% use case |
| Mapping file opt-in `--map` | cảnh báo rõ | file chứa dữ liệu nhạy cảm |
| Không dependency network | audit thẳng Cargo.toml | selling point "zero telemetry" phải kiểm chứng được |

## Pattern & priority

Thứ tự trong `BUILT_IN` = priority (CCCD → CMND → MST → SDT → email → tên DN).
Pattern dài/ng-specific đứng trước để leftmost-first chọn đúng.

## Edge cases đã xử lý
- 13+ chữ số liền → không match CCCD (word boundary).
- SĐT viết `+84`/`84`/`0` → một placeholder sau normalize.
- JSON hỏng → fallback redact như text.
- PDF lỗi extract → bỏ qua file, báo lỗi, không crash cả batch.
- `--in-place` + `--output` → từ chối chạy (mâu thuẫn rõ ràng).
- CMND 9 số có thể bắt nhầm số hiệu văn bản → README ghi rõ, `--patterns` cho phép tắt.

## Phát triển thêm (sau MVP)
- Chế độ AI-assist: flag `--ai-assist` tường minh + cảnh báo + API key user tự cung cấp.
- GUI Tauri: wrap lib.rs hiện tại, không đổi core.
- Stable hash placeholder (cross-run) nếu cần khớp giữa nhiều batch.
