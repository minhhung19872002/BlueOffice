# Việc cần con người (Parity)

Mỗi mục: ID · việc cần · vì sao · phương án tạm.

| ID | Việc cần | Vì sao | Phương án tạm |
|---|---|---|---|
| FID-REF-001 | Xuất ảnh/PDF từng trang của các file trong `tests/corpus` bằng phần mềm office thương mại có bản quyền, đặt vào `tests/fidelity/reference/<app>/<file>/page-N.png` (xem `REFERENCE_GUIDE.md`) | Clean-room: chỉ người dùng có bản quyền mới được tạo ảnh mốc | So với ảnh mốc tự sinh + kiểm tra số học (ngắt trang `w:lastRenderedPageBreak`, giá trị cache công thức) |
| CORE-CHART-006 | Chọn nguồn dữ liệu ranh giới hành chính Việt Nam cấp tỉnh có license phù hợp (đơn vị hành chính hiện hành) | Dữ liệu bản đồ phải có license rõ và cập nhật theo sắp xếp đơn vị hành chính | Map chart giữ nguyên XML khi lưu |
| CORE-LANG-001/002 | Duyệt model dịch/nhận dạng giọng nói cục bộ (license, kích thước triển khai) | Model AI cần kiểm tra license và hạ tầng | Provider gọi API cấu hình được; provider giả trong test |
| CORE-PIC-002 | Duyệt model phân đoạn ảnh (xoá nền) chạy cục bộ | Như trên | Chưa bật tính năng |
