# ADR-0003: Font đóng gói làm mặc định để hiển thị tất định

- Trạng thái: Chấp nhận · Ngày: 2026-11-05

## Bối cảnh
Prompt §3.4 cho phép desktop dùng thêm font hệ thống. Nhưng nếu desktop dùng Times New Roman thật còn web dùng
Tinos thì hình chữ khác nhau, vi phạm yêu cầu "web và desktop hiển thị giống nhau" và làm test visual không ổn định.

## Quyết định
- Phase 1: cả web và desktop dùng **cùng bộ font đóng gói** (Tinos, Liberation Sans, Carlito — OFL/Apache-2.0),
  metric-compatible với Times New Roman / Arial / Calibri. Bảng fallback nằm trong `bo-text`.
- `FontManager::add_fonts_from_dir` (native) có sẵn để nạp font hệ thống; việc bật nó cho desktop sẽ là tuỳ chọn
  người dùng ở Phase 2, và font hệ thống chỉ được dùng cho họ font **không có** trong bộ đóng gói.
- Caladea (thay Cambria) chưa đóng gói ở Phase 1 (fallback về Tinos); bổ sung ở Phase 2.

## Hệ quả
- Ngắt dòng/phân trang giống hệt trên mọi nền tảng và trong worker server.
- Bản cài tăng ~6,5 MB font (nén còn ~3,5 MB).

## Phương án đã loại
- Luôn ưu tiên font hệ thống: không tất định giữa các máy.
- Tải font từ CDN: vi phạm yêu cầu chạy offline và on-premise.
