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

Token font UI của desktop Linux đã chuyển sang font dự án như web; kiểm thử token 28/28 qua.
Chưa có ảnh Linux để đánh giá chữ và dấu tiếng Việt; thay đổi token này không nằm trong bản
Windows đã đo/chụp ở trên.

## Thử nghiệm chữ DirectWrite độc lập

Trên Windows, [spike](../../tools/dwrite-spike/README.md) nạp byte Tinos vào bộ nhớ, dùng ID,
advance và offset từ `bo-text` rồi nhận mặt nạ ClearType cho “Việt Nam” với DPR 1/1,25/1,5/
1,75/2 truyền vào API. Ở DPR 1,25, dạng dựng
sẵn và dạng dấu tổ hợp cho cùng kích thước 97 × 23, cùng 2.165 mẫu khác 0 và tổng alpha
332.830. Các số tổng này không chứng minh hai ảnh giống pixel, dấu đặt đúng, độ nét hay hiệu
năng hay độ nét trên màn hình vật lý. Spike chưa tích hợp vào ứng dụng hoặc surface GPU. Theo
[tài liệu DirectWrite](https://learn.microsoft.com/en-us/windows/win32/api/dwrite_3/nf-dwrite_3-idwriteinmemoryfontfileloader-createinmemoryfontfilereference),
API có thể sao chép byte font khi không truyền owner; loader phải được giữ trong thời gian
font được dùng. Những ràng buộc surface/IME/DPI của ADR-0017 vẫn còn nguyên.

Kiểm tra từ spike cho thấy Caladea thiếu glyph “ạ” và dấu chấm dưới kết hợp. Ca đơn vị với
chuỗi `a` + U+0323 đã tái hiện glyph thiếu trong nhánh chọn font cũ. Mã hiện chọn fallback
cho cả cụm dấu khi có font phù hợp, giữ phạm vi byte của văn bản gốc; kiểm thử dấu đơn và
dấu kép qua. Với trường hợp Caladea thiếu dấu, kiểm thử còn xác nhận dạng dựng sẵn và dạng
tổ hợp chọn cùng font và cùng kết quả shaping. Thay đổi này chưa nằm trong bản Windows đã
chụp/đo ở trên. Bộ [ảnh sau sửa font](screens/after-font-125/manifest.json) của bản SHA-256
`e3326b55b2fdb86b4eb2a7472d1bfb6de5a5b7dea0c3b2fb17434d17da214235` khác tối đa 0,001%
so với bộ sau sửa DPI trên cùng tài liệu. Tài liệu đó không chứa ca dấu tổ hợp đang sửa, nên
phép so chỉ kiểm tra hồi quy trên mẫu hiện có. Bộ dàn trang đã qua.

Trên cùng executable sau sửa font, 10 lượt khởi động/mở và 10 phiên gõ/cuộn (270 phím):
[dữ liệu](baselines/font-ryzen7-7435hs.json). CPU đầu/cuối 16% / 3%; phép so tự động không
báo suy giảm vượt ngưỡng, nhưng tải máy khác lần trước nên không quy chênh lệch cho sửa font.

| Chỉ số | Trước sửa font | Sau sửa font | Mục tiêu |
|---|---:|---:|---|
| Khung hiện, trung vị | 445,5 ms | 428,7 ms | < 500 ms |
| Editor sẵn sàng, trung vị | 902,8 ms | 870,1 ms | < 1.500 ms |
| Phím tới khung hình, trung vị | 33,1 ms | 32,5 ms | < 16 ms |
| Phím tới khung hình, p95 gộp | 44,5 ms | 44,1 ms | < 20 ms |
| Cuộn | 142,1 FPS | 142,2 FPS | ≥ 60 FPS |

Gõ vẫn chưa đạt mục tiêu. Kịch bản benchmark hiện có không bao phủ riêng đoạn dấu tổ hợp,
zoom hoặc kéo chọn với tài liệu đồ họa nặng.
E2E desktop trên bản sửa font qua **16/16**, gồm cộng tác và so trang với web.

## Ribbon: rà ảnh web bước đầu

Theo ảnh BlueDoc người dùng cung cấp, các icon lệnh thường dùng màu chữ trung tính,
nhóm có khoảng cách gọn hơn và các ô kiểu chữ rộng hơn để không cắt “Bình thường”.
[Ảnh sau](screens/ribbon-web-1394.png) chụp Chromium tại viewport 1394 CSS px,
deviceScaleFactor 1,375; ảnh có cả thay đổi nhãn nhóm đang có trong workspace. Kiểm tra
DOM cho ba tên kiểu cho thấy không bị cắt tại kích thước này. Đây là ảnh web, chưa phải
ảnh desktop Mica hay kiểm tra đầy đủ các tab/theme/DPI; cần chụp desktop và đối chiếu trước/sau.
