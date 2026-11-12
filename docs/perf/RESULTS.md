# Desktop — kết quả chương trình hiệu năng

Mốc và phương pháp: [BASELINE.md](BASELINE.md). Máy Ryzen 7 7435HS, Windows 11,
màn hình 125% (WebView DPR 1,375), WebView2 157.0.4341.46. Bản đo là release với
`updater,webdriver`: 5 lượt khởi động/mở và 3 phiên gõ/cuộn, CPU tải 7% trước và 27%
sau. Baseline trước thay đổi chạy với tải 41–94%, nên mức cải thiện chỉ để tham khảo.
[Dữ liệu bản đo](baselines/release-ryzen7-7435hs.json) ·
[dữ liệu bản trung gian](baselines/intermediate-ryzen7-7435hs.json).

| Chỉ số release | Trước | Trung gian | Bản đo | Mục tiêu |
|---|---:|---:|---:|---:|
| Khởi động tới editor | 1.558 ms | 1.115,9 ms | 960,5 ms | < 1.500 ms |
| Shell hiện | 1.215 ms | 643,4 ms | 532,4 ms | < 500 ms |
| Mở .docx 100 trang | 1.674 ms | 1.091,9 ms | 1.047,6 ms | < 2.000 ms |
| Phím tới glyph, trung vị / p95 | 71,1 / 95,4 ms | 35,4 / 47,3 ms | 34,4 / 47,5 ms | < 16 ms |
| Cuộn | 62,2 FPS | 127,2 FPS | 133,2 FPS | 60 FPS |
| RAM một tài liệu / sau gõ và cuộn | 291 / 491 MB | 301,4 / 486,6 MB | 297,8 / 476,1 MB | < 400 MB |
| Render ribbon mỗi phím | 489 | 0 | 0 | 0 |

## Đợt 2 — mốc thống nhất 10 lượt

Đo 10 lượt khởi động/mở và 10 phiên WebDriver (270 phím), cùng một bản release
`updater,webdriver`, SHA-256 `b0024503e3e77c73cc59d1ecbc43032a198e887ec46a6fa91666db3b9763e690`; CPU 7% trước và 10% sau. Máy Windows 11,
Ryzen 7 7435HS, màn hình 125%, WebView2 157.0.4341.46. p95 là nearest-rank, tính trên
10 phiên; độ trễ phím lấy trên toàn bộ 270 phím. [Số liệu và mẫu](baselines/round2-10runs-ryzen7-7435hs.json).

| Chỉ số | Số mẫu | Trung vị | p95 | Mục tiêu |
|---|---:|---:|---:|---:|
| Khởi động tới editor | 10 | 943,6 ms | 981,4 ms | < 1.500 ms |
| Shell | 10 | 512,5 ms | 529,2 ms | < 500 ms |
| Mở tài liệu 100 trang | 10 | 993,1 ms | 1.010,1 ms | < 2.000 ms |
| Phím → khung hình | 270 | 33,8 ms | 47,2 ms | < 16 / p95 < 20 ms |
| Cuộn | 10 | 138,9 FPS | 144 FPS | ≥ 60 FPS |
| RAM sau mở | 10 | 299,9 MB | 301,5 MB | < 400 MB |
| RAM sau gõ/cuộn | 10 | 483 MB | 497,6 MB | < 400 MB |
| Render ribbon / status mỗi phím | 10 | 0 / 0 | 0 / 0 | 0 |

Phân rã 270 phím, trung vị/p95: trước IPC 1/1,3 ms; IPC lệnh 7,8/10,8 ms
(trong đó native 5/7,7 ms); từ kết quả đến yêu cầu trang 0,4/0,5 ms; IPC trang
6,8/8 ms (native 0,2/0,3 ms); sau trang tới vẽ 0,6/7,4 ms; vẽ 2,1/3,1 ms;
sau vẽ tới khung hình 14,1/26,3 ms. Tổng 3 IPC trung vị, p95 4. Thời gian dàn trang
riêng chưa được tách khỏi lệnh native. FPS p95 là percentile tốc độ, không phải
trường hợp xấu nhất; p95 thời gian khung hình theo phiên là 13,8 ms.

Desktop E2E đầy đủ trên cùng executable: 16/16 ca qua, bao gồm hai nhóm cộng tác.
Lưu ý: lượt đo này dùng thư mục khôi phục/offline mặc định của app. Sau đó phát hiện
bản tự lưu benchmark; thao tác benchmark có thể đã ghi đè bản khôi phục trước đó. Không
xóa dữ liệu hiện có. Các lượt tiếp theo yêu cầu bản WebDriver xác nhận cache tạm riêng.
[CI nhánh lần 1](https://github.com/minhhung19872002/BlueOffice/actions/runs/43225468559)
và [lần 2](https://github.com/minhhung19872002/BlueOffice/actions/runs/43265642208)
đã chạy thật; lần 2 hiện có lỗi fidelity và web, các job khác còn chạy. Chưa có bằng chứng CI xanh.
Hai ảnh fidelity khác baseline cũng tái hiện trên CI `main`; chưa tự ý cập nhật baseline.

## Các bước và bằng chứng

1. Đã tích hợp bộ đo release và baseline. CI lưu kết quả và so với artifact lần `main` thành
   công trước đó nếu còn; nếu thiếu artifact thì chỉ ghi báo cáo. Các chỉ số bị thiếu khiến
   job thất bại để tránh kết quả xanh giả.
2. Profile release chuyển `opt-level=3`, fat LTO, một codegen unit, `panic=abort` và tắt
   source map desktop. Desktop dùng engine native qua Tauri cho tài liệu và bảng tính; lỗi
   khởi tạo được báo thay vì âm thầm chuyển engine soạn thảo sang WASM. WASM vẫn được dùng
   cho replica cộng tác và các tính năng web khác. Đã build và đo bản release có `updater,webdriver`. Thử nghiệm giải phóng bộ đệm canvas đã được đo trong ba phiên riêng (xem dưới).
3. Layout đã có cache và cập nhật tăng dần; `DocumentView` vẽ lại theo trang dirty và dùng
   DPR. Chưa có GPU surface native, dirty rectangle, texture scrolling hay DirectWrite. Độ nét
   100/125/150/200% và chuyển màn hình chưa được xác nhận. Xem [ADR-0017](../adr/0017-desktop-document-rendering.md).
4. Đã tích hợp tải editor theo nhu cầu, cập nhật state trì hoãn và memo hóa chrome. Playwright
   trên web qua 3 ca gõ: BlueDoc không render lại ribbon, tiêu đề, status; BlueSheet và
   BlueSlide không render lại ribbon, tiêu đề. Phép đo desktop ghi 0 render ribbon, status và title mỗi phím; các component khác là 8 và pane là 1.
   Khung HTML tĩnh hiện trước khi JS khởi tạo; engine được tạo ngoài đường setup cửa sổ. Bản đo đã có các thay đổi này.
5. Đã tích hợp Mica, caption, snap layouts, token và font hệ thống; xem
   [ADR-0016](../adr/0016-desktop-window-chrome.md). Bản đo qua 12 ca desktop E2E (gồm phóng to/khôi phục, nhập công thức BlueSheet và parity desktop/web); hai nhóm cộng tác không chạy. Chưa xác nhận Windows 10, macOS hay Linux.

Chưa thể kết luận đã đạt các mục tiêu ở bước 6. Cần chạy lại phép đo release, desktop E2E và
kiểm tra trực quan các tỷ lệ màn hình, rồi thực hiện renderer native và đo lại.

## Phân rã độ trễ bản đo

Trung vị: trước lệnh 1 ms, IPC lệnh 8,1 ms (native 5,2 ms), từ kết quả tới yêu cầu
trang 0,4 ms, IPC trang 7,1 ms (native 0,2 ms), vẽ 2,2 ms, chờ khung hình sau vẽ
14,9 ms. Ba IPC mỗi phím. Điều này cho thấy cần giảm số lượt IPC/chờ trình bày, bên cạnh
renderer; cần đo lại sau các thay đổi đó. WebView báo `native`, không tải WASM
trong phiên đo; DPR 1,375 và canvas/CSS 1,374.

## Thử giải phóng bộ đệm canvas

Bản release sau khi xoá ngay backing store của canvas ngoài vùng nhìn được đo lại 3 phiên
WebDriver, CPU 9% trước và 8% sau. [Dữ liệu](baselines/canvas-release-ryzen7-7435hs.json):
gõ trung vị/p95 34,1/46,6 ms; cuộn 131,4 FPS, không có khung trống; RAM sau gõ/cuộn
478,6 MB so với 476,1 MB ở bản đo trước. Chênh lệch nhỏ và không chứng minh giảm RAM;
chưa có profile phân bổ để xác định phần tăng. Lượt này chỉ đo tương tác, không đo lại
khởi động, mở tệp hay RAM ngay sau mở. Test đơn vị xác nhận backing store thực sự được
giải phóng, nhưng không suy ra bộ nhớ tiến trình sẽ giảm tương ứng.

## Mục tiêu chưa đạt và việc còn lại

- Shell 532,4 ms: mốc `page-load-started` đã là 444,3 ms, nên thời gian tạo Tauri/WebView2
  chiếm phần lớn ngân sách 500 ms; cần profile khởi tạo native trên máy ít tải và thử giảm nó.
- Gõ 34,1 ms trung vị / 46,6 ms p95 ở bản thử canvas: còn ba IPC mỗi phím và chờ trình bày;
  cần giảm round-trip, thử renderer native và đo lại thay vì suy đoán kết quả.
- RAM sau thao tác 478,6 MB: thử giải phóng canvas không chứng minh cải thiện; cần phân tích
  heap, vùng nhớ native và WebView2/GPU trước khi chọn tối ưu tiếp.
- Chữ sắc nét ở 100/125/150/200% và khi chuyển màn hình: mới đo tỷ lệ backing store ở
  125%, chưa kiểm tra thực tế cả bốn mức hay DirectWrite. Renderer native và các lớp chữ
  theo nền tảng trong ADR-0017 chưa được triển khai.

Các lý do này là trạng thái kỹ thuật chưa hoàn tất, không phải xác nhận rằng các mục tiêu
không thể đạt.

## Đợt 2 — thử RAM liên tục trên bản Fluent

Bản release `updater,webdriver`, SHA-256 `319c3831c59c6197ad87f714ba253d9ea944c4fcf4edfe42b7d2f2cdf91d5d12`,
cache khôi phục cách ly; cùng máy và Windows 125% như trên. Đo private working set của
cây tiến trình app và WebView2 mỗi khoảng 30 giây. Trong 602,8 giây có 491 lần gõ, cuộn
và 52 chu kỳ chèn ảnh 256×256/undo; đỉnh **442,6 MB**, cao hơn mục tiêu 400 MB.
Từ mẫu một phần ba thời gian tới mẫu cuối tăng **87,2 MB**: bài thử đánh dấu có xu hướng
tăng và trả mã lỗi. Đây là một phiên, không phải 10 lượt hay bằng chứng xác định nguyên
nhân rò rỉ. [Mẫu và môi trường](baselines/round2-soak-ryzen7-7435hs.json).

Chưa có heap profile Rust hoặc snapshot WebView hợp lệ để quy phần tăng cho undo, cache
hay canvas. Chưa chứng minh mọi cache có giới hạn; chưa đạt tiêu chí RAM.
