# Báo cáo chất lượng hình ảnh BlueOffice

Trạng thái: **đang thực hiện; chưa đạt điều kiện kết thúc**. Mốc dưới đây là phép đo đã có,
không đại diện cho mọi DPI hay tài liệu trong phạm vi mới. Danh sách vấn đề theo dõi tại
[ISSUES.md](ISSUES.md); yêu cầu ảnh đối chiếu tại [NEEDS_HUMAN.md](../../NEEDS_HUMAN.md).

## Mốc hiện có trước chương trình

[80 ảnh desktop](../ui/screens/after-immediate-125/manifest.json), ba ứng dụng, light/dark,
các tab và một số hộp thoại/menu; Windows 125%, WebView DPR 1,375, tài liệu mới trống.
Ví dụ: [BlueDoc](../ui/screens/after-immediate-125/doc-light-home.png),
[BlueSheet](../ui/screens/after-immediate-125/sheet-light-home.png),
[BlueSlide](../ui/screens/after-immediate-125/slide-light-home.png). Đây không phải bộ ảnh
đầy đủ zoom/DPI và không chứng minh độ sắc của nội dung tài liệu. Ảnh trước các thay đổi
Fluent: [manifest](../ui/screens/before-125/manifest.json).

Bản release WebDriver SHA-256 `75e9508742bbb0f8fdaab4de7a2f8b7bc0d9e5dca759697d118820b222acda56`,
Windows 11, Ryzen 7 7435HS, 10 lượt; [dữ liệu](../perf/baselines/round3-immediate-10runs-ryzen7-7435hs.json)
và [phương pháp/giới hạn](../perf/RESULTS.md). Số không áp dụng cho các thay đổi chưa build.

| Chỉ số | Trung vị | p95 | Mục tiêu |
|---|---:|---:|---|
| Khung hiện | 504 ms | 537,4 ms | < 500 ms |
| Editor sẵn sàng | 969,6 ms | 1.015,1 ms | < 1.500 ms |
| Phím tới khung hình | 34,9 ms | 44,4 ms | < 16 ms, p95 < 20 ms |
| Cuộn | 142,3 FPS | — | ≥ 60 FPS |

Đây là tốc độ cuộn của kịch bản đo, chưa chứng minh zoom và kéo chọn đạt 60 FPS với mọi
tài liệu. Canvas theo DPR chưa chứng minh độ nét native hay độ đúng khi đổi màn hình.
Không có ảnh Office do người dùng cung cấp; không thể so sánh với Office.
