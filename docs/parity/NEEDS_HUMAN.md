# Việc cần con người (Parity)

Mỗi mục: ID · việc cần · vì sao · phương án tạm.

| ID | Việc cần | Vì sao | Phương án tạm |
|---|---|---|---|
| FID-REF-001 | Xuất ảnh/PDF từng trang của các file trong `tests/corpus` bằng phần mềm office thương mại có bản quyền, đặt vào `tests/fidelity/reference/<app>/<file>/page-N.png` (xem `REFERENCE_GUIDE.md`) | Clean-room: chỉ người dùng có bản quyền mới được tạo ảnh mốc | So với ảnh mốc tự sinh + kiểm tra số học (ngắt trang `w:lastRenderedPageBreak`, giá trị cache công thức) |
| CORE-CHART-006 | Chọn nguồn dữ liệu ranh giới hành chính Việt Nam cấp tỉnh có license phù hợp (đơn vị hành chính hiện hành) | Dữ liệu bản đồ phải có license rõ và cập nhật theo sắp xếp đơn vị hành chính | Map chart giữ nguyên XML khi lưu |
| CORE-LANG-001/002 | Duyệt model dịch/nhận dạng giọng nói cục bộ (license, kích thước triển khai) | Model AI cần kiểm tra license và hạ tầng | Provider gọi API cấu hình được; provider giả trong test |
| CORE-PIC-002 | Duyệt model phân đoạn ảnh (xoá nền) chạy cục bộ | Như trên | Chưa bật tính năng |
| UI-BRAND-001 | Xác nhận mã màu thương hiệu Bluestar (đang dùng tạm `#1F6FEB`) | Chưa có `assets/brand/`; màu phải lấy từ logo chính thức | Token `--bo-brand: #1F6FEB` (`packages/ui/src/tokens`) |
| UI-BRAND-002 | Duyệt logo BlueOffice và icon 3 app (BlueDoc, BlueSheet, BlueSlide) chính thức | Nhận diện thương hiệu do chủ dự án quyết | Bản nháp SVG trong `assets/brand/drafts/` (UI-ICONS-004) |
| SLIDE-EXPORT-001 | Chọn bộ mã hoá video license hợp lệ cho máy chủ / CLI (không có trình duyệt): H.264 không có crate Rust license MIT/Apache/BSD; VP8 thuần Rust chưa có; ứng viên `rav1e` (AV1, BSD-2) nặng, cần duyệt kích thước + hiệu năng | Dự án cấm encoder GPL/LGPL (ffmpeg/x264); mã hoá H.264 phần mềm còn vướng bằng sáng chế | Web và desktop (webview có WebCodecs) xuất MP4/WebM qua encoder của trình duyệt/hệ điều hành; desktop native/CLI/server xuất GIF động (`bo-cli convert x.pptx --to gif`) và chuỗi PNG (.zip) |
