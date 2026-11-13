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

Đã chụp thêm tài liệu hành chính mẫu trên bản trước thay đổi kích thước trang:
[50%](screens/before-doc-125/doc-light-50.png), [100%](screens/before-doc-125/doc-light-100.png),
[200%](screens/before-doc-125/doc-light-200.png), [400%](screens/before-doc-125/doc-light-400.png)
ở light và [100% dark](screens/before-doc-125/doc-dark-100.png); manifest liệt kê đủ tám ảnh.
[Manifest](screens/before-doc-125/manifest.json) ghi SHA của executable và tài liệu,
Windows 125%, DPR 1,375 và viewport. Mỗi ảnh cuộn về đầu trang và đưa
điểm chèn đầu tài liệu vào vùng nhìn, để ảnh 400% có chữ. Đây mới là một tài liệu và một
DPI, chưa có đối chiếu Office.
Hai lần chụp liên tiếp cùng executable và môi trường cho chênh lệch không quá 0,001% ở tám ảnh;
điều này chưa chứng minh độ ổn định trên runner CI khác. Đã thêm ca này vào workflow CI;
lần đầu hoặc khi thiếu artifact `main` vẫn chỉ cảnh báo, chưa có kết quả CI xác nhận.

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

## Độ phân giải canvas: thay đổi đang kiểm tra

Kích thước CSS của trang trước đây được làm tròn trước, trong khi kích thước bộ đệm canvas
được làm tròn riêng theo DPR. Trang hiện giữ kích thước CSS phân số theo kích thước tài liệu
và zoom, giảm một lần làm tròn trước khi WebView hiển thị canvas. Kiểm thử đơn vị đã kiểm tra
kích thước bộ đệm với DPR 1/1,25/1,5/1,75/2 và zoom 50/100/200/400%; 52 kiểm thử
canvas-host và typecheck đã qua.
Ảnh trước/sau cùng tài liệu trên Windows 125%: [trước light 100%](screens/before-doc-125/doc-light-100.png) /
[sau](screens/after-doc-125/doc-light-100.png), [trước dark 100%](screens/before-doc-125/doc-dark-100.png) /
[sau](screens/after-doc-125/doc-dark-100.png); [manifest sau](screens/after-doc-125/manifest.json)
có đủ tám ảnh. Bản sau SHA-256
`d1bc2f4cfdd24a9eb6f820f82a4afa3ba5720b8334084450dbe9a3596c5472c9` cũng chứa các thay đổi
ribbon và tùy chọn dán đã có trong workspace. Ở zoom 100%, chiều rộng CSS trước/sau là
794 / 793,727 px, bộ đệm đều 1.091 px, DPR 1,375. Kiểm thử số học và một bộ ảnh này chưa
chứng minh khả năng chống nhòe trên các màn hình khác hay khi đổi màn hình.

So ảnh trước/sau vượt ngưỡng 0,1% ở light/dark 100% và 200% (tối đa 0,706%); đây là các
khác biệt chưa thể quy riêng cho sửa canvas vì executable còn thay đổi khác. Nhãn “Giọn...”
trong ảnh trước đã thành “Giọng nói” đầy đủ ở ảnh BlueDoc sau. Chưa coi visual regression
là xanh hoặc checklist là sạch.

| Theme | Zoom | Trước | Sau |
|---|---:|---|---|
| Light | 50% | [Ảnh](screens/before-doc-125/doc-light-50.png) | [Ảnh](screens/after-doc-125/doc-light-50.png) |
| Light | 100% | [Ảnh](screens/before-doc-125/doc-light-100.png) | [Ảnh](screens/after-doc-125/doc-light-100.png) |
| Light | 200% | [Ảnh](screens/before-doc-125/doc-light-200.png) | [Ảnh](screens/after-doc-125/doc-light-200.png) |
| Light | 400% | [Ảnh](screens/before-doc-125/doc-light-400.png) | [Ảnh](screens/after-doc-125/doc-light-400.png) |
| Dark | 50% | [Ảnh](screens/before-doc-125/doc-dark-50.png) | [Ảnh](screens/after-doc-125/doc-dark-50.png) |
| Dark | 100% | [Ảnh](screens/before-doc-125/doc-dark-100.png) | [Ảnh](screens/after-doc-125/doc-dark-100.png) |
| Dark | 200% | [Ảnh](screens/before-doc-125/doc-dark-200.png) | [Ảnh](screens/after-doc-125/doc-dark-200.png) |
| Dark | 400% | [Ảnh](screens/before-doc-125/doc-dark-400.png) | [Ảnh](screens/after-doc-125/doc-dark-400.png) |

Trên cùng executable sau, 10 lượt khởi động/mở và 10 phiên gõ/cuộn (270 phím):
[dữ liệu đo](baselines/dpi-ryzen7-7435hs.json). CPU đầu/cuối 13% / 16%, so với
25% / 9% ở mốc cũ, nên số chênh lệch không chứng minh riêng tác động của sửa DPI.

| Chỉ số | Trước, trung vị | Sau, trung vị | Mục tiêu |
|---|---:|---:|---|
| Khung hiện | 504 ms | 445,5 ms | < 500 ms |
| Editor sẵn sàng | 969,6 ms | 902,8 ms | < 1.500 ms |
| Phím tới khung hình | 34,9 ms | 33,1 ms | < 16 ms |
| Phím tới khung hình, p95 gộp | 44,4 ms | 44,5 ms | < 20 ms |
| Cuộn | 142,3 FPS | 142,1 FPS | ≥ 60 FPS |

Phép so benchmark tự động không vượt ngưỡng cảnh báo của nó; gõ vẫn chưa đạt. Cuộn chỉ được
đo trong kịch bản hiện có, chưa xác minh zoom/kéo chọn và mọi nội dung đồ họa.
E2E desktop trên executable này qua **16/16**, gồm hai nhóm cộng tác và so ảnh trang với web.
