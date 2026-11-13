# PRD — redact-tool (rút gọn)

## Bối cảnh
Nghị định/quy định cấm cán bộ, công chức đưa dữ liệu mật lên hệ thống AI, nhưng
công việc vẫn cần AI hỗ trợ. Bộ kit redact (phần 1) đã đóng gói phương pháp; tool
này là phần (2): **đóng phương pháp thành binary chạy local, ai cũng dùng được**.

## Người dùng
- Cán bộ/công chức cần đưa tài liệu lên AI an toàn (chính).
- Kiểm toán viên, kế toán xử lý hồ sơ chứa CCCD/MST/SĐT.
- Người dùng phổ thông muốn redact trước khi chia sẻ tài liệu.

## Yêu cầu chức năng (MVP)
1. Redact bằng pattern regex VN: CCCD, CMND, MST, SĐT, email, tên DN → placeholder nhất quán.
2. Định dạng: text/CSV (regex thuần), JSON (giữ key, output valid), PDF (trích text → txt).
3. 2 chế độ: (a) script thuần — an toàn tuyệt đối, không network; (b) AI-assist —
   mô tả trong README, implement sau (kết thúc M3).
4. 100% local, zero telemetry — selling point, phải audit được từ source.
5. Cross-platform: macOS aarch64 + Windows x86_64, build release bằng GitHub Actions.
6. CLI trước; GUI (Tauri) là phase sau.

## Success metrics
- Dry-run cho biết chính xác số match từng pattern từng file.
- Binary chạy offline hoàn toàn (rút mạng vẫn đủ chức năng).
- Cùng giá trị → cùng placeholder trong 1 lần chạy.
- File gốc không bao giờ bị sửa trừ khi `--in-place`.

## Non-goals (MVP)
- OCR PDF scan.
- Chế độ AI-assist (chỉ mô tả).
- GUI.
