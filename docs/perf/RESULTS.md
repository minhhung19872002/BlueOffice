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

Chưa có heap profile Rust hoặc snapshot WebView đầu/cuối bài thử 10 phút để quy phần tăng
cho undo, cache hay canvas. Chưa chứng minh mọi cache có giới hạn; chưa đạt tiêu chí RAM.

## Đợt 2 — đo lại sau Fluent và cách ly dữ liệu

Cùng executable SHA-256 `319c3831c59c6197ad87f714ba253d9ea944c4fcf4edfe42b7d2f2cdf91d5d12`
đã dùng cho bài thử RAM; 10 lượt khởi động/mở, 10 phiên tương tác gồm 270 phím. CPU
trước/sau 14% / 10%. p95 dùng nearest-rank; p95 phím gộp toàn bộ 270 phím.
[Dữ liệu từng lượt](baselines/round2-fluent-10runs-ryzen7-7435hs.json).

| Chỉ số | Trước: trung vị / p95 | Sau: trung vị / p95 | Kết luận |
|---|---:|---:|---|
| Editor sẵn sàng | 943,6 / 981,4 ms | 948,5 / 1.009,9 ms | đạt < 1.500 ms |
| Shell | 512,5 / 529,2 ms | 508,1 / 535,5 ms | chưa đạt < 500 ms |
| Mở 100 trang | 993,1 / 1.010,1 ms | 1.008,5 / 1.032,7 ms | đạt < 2.000 ms |
| Phím tới khung hình, 270 mẫu | 33,8 / 47,2 ms | 33,8 / 46,5 ms | chưa đạt < 16 / p95 < 20 ms |
| Cuộn, FPS theo phiên | 138,9 / 144 | 136,5 / 143,4 | đạt 60 FPS trong phép thử; p95 FPS không phải trường hợp xấu nhất |
| RAM sau mở | 299,9 / 301,5 MB | 300,8 / 303,2 MB | dưới 400 MB |
| RAM sau gõ/cuộn | 483 / 497,6 MB | 495,8 / 507,8 MB | chưa đạt < 400 MB |
| Render ribbon / status mỗi phím | 0 / 0 | 0 / 0 | đạt |

Phân rã 270 phím sau, trung vị/p95: nhận sự kiện → IPC 1 / 1,3 ms; IPC lệnh
7,5 / 10,5 ms (lõi 4,8 / 7,2 ms, trong đó dàn trang tăng dần 0,6 / 0,8 ms); kết quả
→ yêu cầu trang 0,4 / 0,5 ms; IPC trang 7 / 8,3 ms; trang → vẽ 0,6 / 7,5 ms; vẽ
2,1 / 3,3 ms; vẽ → trình bày 13 / 26,2 ms. Trung vị 3 IPC, p95 4. Các chặng được
đo riêng; không cộng p95 của từng chặng để suy ra p95 tổng.

Chưa có local echo, một IPC/diff hay vẽ native DirectWrite. IME hiện hiển thị vùng
composition riêng và chỉ commit khi composition kết thúc; chưa xác nhận Unikey/EVKey
trên các kiểu chữ và trường hợp sửa ký tự trước. Cần mở rộng hợp đồng vị trí/kiểu chữ và
đồng bộ kết quả shaping trước khi có thể thay chữ tạm bằng chữ chính thức không nhảy.
Điều này là việc còn làm, không phải bằng chứng không thể đạt.

Desktop E2E trên executable này: **16/16 qua, không bỏ qua**, bao gồm hai nhóm cộng tác.
Lượt đầu có một ca bị hủy vì bước xác nhận cache mới chạy trước khi Tauri khởi tạo; đã sửa
điều kiện chờ và chạy lại toàn bộ. Bài thử nhập tiếng Việt tự động không thay thế kiểm tra
Unikey/EVKey thực tế.

### Thử công cụ heap

Một phiên chẩn đoán riêng 15 giây (12 phím, một ảnh/undo) đã lấy hai V8 heap snapshot
từ đúng WebView, lưu cục bộ tại `out/perf/snapshot-diagnostic/` (không đưa snapshot vào
Git). Tổng kích thước shallow theo node trong snapshot tăng từ 32,66 lên 34,05 MiB;
`JSArrayBufferData` giữ nguyên 15.196.089 byte. Đây không phải heap snapshot trước/sau
bài thử 10 phút và không thể dùng để xác định nguyên nhân tăng private working set.
Thử bật Windows Performance Recorder heap tracing cho tên executable thử nghiệm riêng
bị Windows từ chối `0x80070005` (Access is denied); không có native heap profile.

[CI cho commit Fluent](https://github.com/minhhung19872002/BlueOffice/actions/runs/43269570854)
đã chạy và job performance đạt, nhưng job fidelity thất bại; các job còn lại đang chạy tại
thời điểm ghi. Chưa xác nhận CI toàn bộ xanh. Bản sửa điều kiện chờ E2E cần một lượt CI mới.

## Tiếp tục sửa lỗi desktop / accessibility

Dải đen đầu trang là con trỏ bị trải rộng vì CSS lớp phủ tạo động không áp dụng trên desktop;
đã đưa các quy tắc vào stylesheet đóng gói và chụp lại bản release (xem [báo cáo thị giác](../ui/REPORT.md)).
Đã sửa trường hợp truy vấn accessibility đọc nội dung đã sửa trước khi nhận kết quả con trỏ:
so sánh với bản trước sự kiện nhập; kiểm thử đơn vị cho thứ tự này đạt và E2E BlueDoc lặp 5/5 đạt.
Bản release sau các thay đổi này cần được đo lại trước khi kết luận có ảnh hưởng hiệu năng.

[CI gần nhất](https://github.com/minhhung19872002/BlueOffice/actions/runs/43273616492)
thất bại ở web E2E, desktop Windows và fidelity. Desktop Windows dừng do bước tải artifact
ảnh chuẩn thiếu nhưng để lại mã thoát lỗi; đã sửa xử lý trường hợp thiếu artifact. Trên web,
đã sửa và kiểm lại lỗi contrast BlueSheet và lỗi thông báo BlueDoc; những ca lỗi còn lại và
hai sai lệch fidelity vẫn cần được xác minh. Chưa tuyên bố CI xanh.

Các truy vấn nền `styleList`, `fontList`, `equationTemplates`, `notes` được xử lý trước
nhánh chỉnh sửa story, tránh đóng đầu trang/chú thích. Hai kiểm thử lõi cho đầu trang và
footnote đã qua; ba ca web từng lỗi cần chạy lại với WASM mới. Danh sách vẽ của trang xa
vùng nhìn hiện được loại khỏi cache cùng canvas, có kiểm thử; chưa đo lại RAM nên chưa
kết luận mức giảm hoặc đạt ngưỡng.

Sau khi build WASM mới, ba ca web từng thất bại về sửa đầu trang, chú thích và sửa đầu trang
có theo dõi thay đổi đã qua 3/3. Cache ảnh giải mã của vùng tài liệu BlueDoc hiện giới hạn
64 MiB / 256 mục sau khi vẽ; ảnh đang được các trang sử dụng được giữ đến khi vẽ xong,
bitmap bị loại được đóng. Các app khác chưa dùng cơ chế giới hạn này. Test đơn vị kiểm tra
không đóng ảnh đang được vẽ đồng thời; chưa có phép đo RAM sau thay đổi.

### Hai baseline fidelity cũ

Đã đối chiếu dữ liệu mẫu và lịch sử trước khi cập nhật đúng hai ảnh baseline từ artifact CI
Linux [lần 2](https://github.com/minhhung19872002/BlueOffice/actions/runs/43265642208):

- `doc-chart-2`: commit `a9dd751a` đổi `word/charts/chart4.xml` từ `holeSize=50` sang
  `holeSize=75`, nhưng không cập nhật ảnh. Vùng khác biệt là vòng biểu đồ.
- `slide-zoom-section`: commit `102ea928` tạo lại ảnh xem trước không có nền đích, theo
  thay đổi `showBg` ở `c6665b3e`; ảnh baseline cũ vẫn chứa nền trắng che nội dung phía sau.

Hai ảnh mới phản ánh nội dung mẫu hiện tại, không nới ngưỡng so sánh. Cần CI mới xác nhận
toàn bộ corpus; chưa coi CI là xanh chỉ vì đã cập nhật baseline.

`DocumentView.refresh` hiện bắt đầu cập nhật trang dirty ngay thay vì chờ animation frame;
trước khi vẽ kiểm tra lại request trang, zoom và DPI để không vẽ kết quả cũ. Kiểm thử đơn vị
bao gồm trường hợp kết quả đến ngược thứ tự; 6 ca web accessibility / render chrome đã qua.
Bản release trước thay đổi này đang chạy lại E2E và thử RAM 10 phút; cần build, đo 10 lượt
và thử RAM trên bản có thay đổi này trước khi đánh giá hiệu quả.

CI dùng nhãn Rust `stable` trôi từ 1.99.1 sang 1.100, khiến Clippy phát hiện các cảnh báo
`map_unwrap_or` / `needless_range_loop` trong `bo-formula`. Đã cố định toolchain CI ở
1.99.1, cùng phiên bản đo và phiên bản từng qua các job Rust, để kết quả có thể tái lập.
Việc xử lý cảnh báo để nâng CI lên Rust 1.100 vẫn còn; không coi thay đổi này là đã sửa
khả năng tương thích với Clippy mới.

### Thử lại RAM sau giới hạn cache, trước sửa vẽ tức thời

Bản release SHA-256 `20aadb6ae8087fd0fed57a0f0cc2172250962211b7417781b5355fc3423b4d70`
đã qua desktop E2E **16/16**, gồm cộng tác và kiểm tra con trỏ. Phép thử 602,4 giây:
488 lần gõ, 52 chu kỳ ảnh/undo, đỉnh **464,5 MB**, phần cuối tăng **70 MB**; không đạt.
Bản trước giới hạn cache đạt đỉnh 442,6 MB; hai phép thử đơn lẻ không chứng minh cache
là nguyên nhân làm RAM tăng hay giảm. [Dữ liệu mẫu](baselines/round3-soak-ryzen7-7435hs.json).
Một mẫu phân rã trong phiên: app và tiến trình phụ không thuộc WebView khoảng 126 MB,
WebView2 khoảng 255 MB (bao gồm renderer và GPU); không suy ra nguyên nhân từ một mẫu.
Trong lúc thử có một số kiểm tra ngắn chạy trên cùng máy, vì vậy tải nền không hoàn toàn tĩnh.
Bản thử này chưa gồm thay đổi vẽ ngay; bản mới đang build và cần đo lại.

### Đo 10 lượt sau sửa vẽ tức thời

Bản release `updater,webdriver`, SHA-256
`75e9508742bbb0f8fdaab4de7a2f8b7bc0d9e5dca759697d118820b222acda56`,
10 lượt khởi động/mở và 10 phiên, 270 phím; CPU đầu/cuối 25% / 9% (lượt trước
14% / 10%). Bản này đã qua desktop E2E 16/16, cả hai nhóm cộng tác.
[Dữ liệu](baselines/round3-immediate-10runs-ryzen7-7435hs.json).

| Chỉ số | Trước: trung vị / p95 | Sau: trung vị / p95 |
|---|---:|---:|
| Editor sẵn sàng | 948,5 / 1.009,9 ms | 969,6 / 1.015,1 ms |
| Shell | 508,1 / 535,5 ms | 504 / 537,4 ms |
| Mở 100 trang | 1.008,5 / 1.032,7 ms | 1.001,7 / 1.044,3 ms |
| Phím tới khung hình (270 phím) | 33,8 / 46,5 ms | 34,9 / 44,4 ms |
| Sau vẽ tới trình bày (270 phím) | 13 / 26,2 ms | 8 / 19,4 ms |
| RAM sau mở | 300,8 / 303,2 MB | 294,2 / 296,9 MB |
| RAM sau gõ/cuộn | 495,8 / 507,8 MB | 496,4 / 508,8 MB |
| Cuộn, FPS theo phiên | 136,5 / 143,4 | 142,3 / 143,8 |

Thời gian sau vẽ giảm trong phép đo, nhưng tổng độ trễ không đạt mục tiêu và tải máy khác
nhau; không thể quy toàn bộ chênh lệch cho thay đổi. IPC lệnh sau có trung vị/p95
12,8 / 16,2 ms; IPC trang 8 / 15,6 ms; dàn trang 0,6 / 0,8 ms; vẫn 3 IPC trung vị.
Shell, gõ và RAM sau thao tác đều chưa đạt; không có local echo hay giao thức diff một IPC.

Đã thêm feature thử nghiệm `heap-profile` dùng `stats_alloc` để đếm byte do allocator Rust
quản lý, cùng tùy chọn `--rust-heap` trong bài thử. Đây là bộ đếm tổng, không phải phân tích
stack từng allocation, và không dùng trong bản cài. Build này khác executable ở bảng trên;
số của nó chỉ dùng chẩn đoán, không thay thế phép đo release không instrumentation.

### Chẩn đoán heap 10 phút (có instrumentation)

Bản `updater,webdriver,heap-profile`, SHA-256
`e4ea97a3f390e8b67b381db112a8fde07fb2981c4fa4bf6ca77bcd0974549eca`;
492 lần gõ, 52 chu kỳ ảnh/undo, đỉnh **463,4 MB**, phần cuối tăng **60,4 MB**.
[Chuỗi mẫu](baselines/round3-heap-ryzen7-7435hs.json); hai WebView heap snapshot nằm
cục bộ tại `out/perf/round3-heap/`, không đưa vào Git.

Tại mẫu đầu / cuối: private working set app và tiến trình phụ **119,3 / 137,2 MB**;
WebView2 **177,5 / 326,2 MB**; byte còn dùng qua allocator Rust **96,09 / 108,73 MiB**.
Đây là các bộ đếm khác nhau, không cộng hoặc trừ chúng như những vùng nhớ độc lập.
Shallow size của các node V8 snapshot là **32,71 / 36,61 MiB**; số node
`blink::UndoStep` là **0 / 492**. Điều này cho thấy nơi cần điều tra thêm nhưng không chứng
minh undo của WebView gây ra toàn bộ phần tăng: snapshot shallow không đo đầy đủ bộ nhớ
native/GPU, và việc chụp snapshot có thể kích hoạt GC. Không suy số release chính thức từ
bản instrumentation.

### Thử RAM release cùng executable với bảng 10 lượt

Bản không instrumentation SHA-256
`75e9508742bbb0f8fdaab4de7a2f8b7bc0d9e5dca759697d118820b222acda56`: 602,5 giây,
489 lần gõ và 52 chu kỳ ảnh/undo; đỉnh **460 MB**, phần cuối tăng **63,8 MB**, không đạt.
Private working set app / WebView2 ở mẫu đầu: **121,2 / 187,3 MB**; mẫu cuối:
**137,5 / 322,5 MB**. [Các mẫu](baselines/round3-immediate-soak-ryzen7-7435hs.json).
Đây là cùng SHA với phép đo 10 lượt; binary được sao chép sang tên khác để giữ nguyên
trong khi build bản instrumentation.

### Direct2D + DirectWrite cho trang tài liệu (gpu-preview, 2026-11-13)

Bản release `--features gpu-preview`, Ryzen 7 7435HS, `phase1-sample.docx`, zoom 110 %:
`engine_gpu_page` lần đầu **169,6 ms** (gồm tạo thiết bị D3D11/Direct2D), các trang sau
**6,0 / 6,2 / 7,9 ms** (vẽ + readback trong Rust, chưa tính IPC và `putImageData`). Chưa đo
phím→khung hình hay cuộn với đường này; không suy ra đạt mục tiêu 16 ms.

So ảnh chụp cùng đoạn văn với trình soạn thảo đối chứng trên cùng màn hình (quan sát black-box):
ngắt dòng và vị trí chữ trùng; độ lệch màu trung bình |R−B| của vùng chữ: đối chứng **7,47**
(có cả gạch đỏ chính tả), BlueDoc xám **0**, BlueDoc ClearType hệ thống **17,11**. Mẫu
`cargo test -p bo-gpu-win text_modes -- --ignored`: hệ thống 10,81 · mức 0,35/contrast 2,0
**3,88** với độ đậm cao hơn bản xám 11 % (đối chứng đậm hơn bản xám của BlueDoc ~15 %). Đã chọn
mức 0,35/contrast 2,0 làm mặc định; chưa chụp lại ở DPI 100/125/200 %.

#### Độ nét: bám lưới pixel, canvas 1:1, ribbon đục (2026-11-13)

Người dùng thấy trang và ribbon "nhòe hơn" đối chứng (cùng Times New Roman thật: desktop nạp
font hệ thống trước bản thay thế). Đo trên ảnh chụp 125 %: tỉ lệ cạnh ngang sắc (hàng kề nhau
lệch > 200) trong vùng chữ trang: đối chứng **0,162**, BlueDoc **0** — chữ được khử răng cưa
cả chiều dọc (`NATURAL_SYMMETRIC`) và canvas bị trình duyệt co giãn vì khung trang có kích thước
lẻ; đường kẻ 1 px chia hai hàng xám (204 / 52). Sau khi đổi sang `DWRITE_RENDERING_MODE_NATURAL`
+ grid-fit, làm tròn baseline và đường kẻ/ô thẳng theo pixel thiết bị, và hiển thị canvas 1:1
(`DocumentView.alignToDevicePixels`): **0,353**, đường kẻ một hàng đen, viền màu 6,46 (đối chứng 7,47).

Ribbon trên Mica trong suốt 28 % nên Chromium vẽ chữ xám. Thân ribbon giờ đục trên lớp
compositor riêng; nhãn nhóm dùng màu gần màu chữ chính. Nhãn nhóm: cạnh ngang sắc 0,077 → 0,319
(đối chứng 0,372), điểm tối nhất 91 → 48 (đối chứng 38); tab có ClearType (độ lệch kênh 15 → 75,
đối chứng 68). Ô chọn phông vẫn xám (control riêng của WebView). Chưa đo DPI 100/150/200 %.

#### Điều chỉnh chữ tài liệu native (2026-11-14)

Vẫn giữ ghép bitmap trang 1:1, baseline bám hàng pixel và DirectWrite NATURAL có grid-fit.
Đổi mặc định ClearType từ gamma 1,8 / contrast 0 / level 0,4 sang 1,8 / 0,5 / 0,2:
mẫu Times New Roman tiếng Việt trong `text_modes_tnr` nhìn đậm hơn và bớt viền màu.
Trên pixel mẫu có ít nhất một kênh dưới 245, độ tối trung bình tăng 138,0 → 160,2;
độ lệch |R−B| trung bình giảm 33,5 → 17,8. Đây là phép đo ảnh mẫu offscreen,
không phải so sánh mới với Word; cần kiểm tra trực tiếp cùng font, zoom, DPI và màn hình
trước khi kết luận tương đương. Khi người dùng tắt ClearType vẫn dùng grayscale.

Tiếp tục kiểm tra: thông số vẽ chữ tùy chỉnh dùng thứ tự điểm ảnh do DirectWrite
báo thay vì luôn giả định RGB; điều này tránh đảo viền màu trên màn hình BGR.
Host native chọn thứ tự của màn hình chứa cửa sổ khi tạo và khi di chuyển, và
vẽ lại toàn bộ trang trong cache nếu thứ tự thay đổi; đường offscreen không gắn
cửa sổ dùng màn hình chính. Test kiểm tra việc làm mới cache; vẫn cần thử thực tế
khi kéo giữa hai màn hình. Các test GPU không chứng minh chữ đã giống đối chứng.

Đã chạy lại mẫu `text_modes_tnr` với font Times New Roman hệ thống, zoom 110%,
ở tỉ lệ DPI 100/125/150/200% (`BO_TEXT_MODES_DPI_SCALE=1.0|1.25|1.5|2.0`).
Với các pixel có ít nhất một kênh dưới 245, độ lệch |R−B| trung bình của cấu hình
hiện tại lần lượt là **19,0 / 17,8 / 16,1 / 13,9**, so với cấu hình trước
**36,1 / 33,5 / 31,0 / 27,2**. Đây là ảnh DirectWrite ngoài màn hình của cùng
một đoạn mẫu, không phải ảnh chụp Word hoặc xác nhận trên bốn màn hình vật lý.
Mẫu còn có các chế độ hệ thống và grayscale để lặp lại phép đối chiếu khi có ảnh
tham chiếu; không đổi tham số mặc định chỉ từ phép so nội bộ này.

Phép kiểm thử vẽ lại sau sửa được mở rộng từ một cấu hình lên DPI/zoom
**100%/100%, 125%/110%, 150%/75% và 200%/100%**. Chuỗi lệnh gồm chèn, xóa,
chữ Việt dựng sẵn, dấu Unicode kết hợp, undo và redo; sau mỗi lệnh, ảnh ghép dải
trùng từng pixel với ảnh vẽ lại toàn trang, dùng cấu hình chữ mặc định của tài liệu.
Đây là lệnh sửa mô phỏng, không phải
phép thử bộ gõ Telex hay xác nhận chất lượng chữ so với Word.

Ribbon native đã sửa phép cộng vị trí dọc: các điều khiển bám mép trên ribbon,
không phụ thuộc chiều cao vùng nút. Test kiểm tra vị trí và vùng nhấn ở 100, 125,
150 và 200% với mép trên thay đổi. Đo nhãn nút menu cũng dùng đúng ký tự mũi tên
được vẽ. Chưa có ảnh đối chiếu mới cho bản native sau sửa.

Khi thu hẹp cửa sổ, gallery kiểu chữ trên tab Home giảm dần số ô xem trước để ưu tiên
giữ các lệnh phía sau; nút mở danh sách kiểu vẫn còn. Test bố cục kiểm tra hai ngôn ngữ
và 100/125/150/200%, kể cả vùng nhấn của nút danh sách. Nếu vẫn không đủ chỗ,
hai nút mũi tên cho phép dịch vùng nhóm lệnh; phần ngoài vùng hiển thị bị cắt và
không nhận nhấn. Test kiểm tra đi tới lệnh cuối rồi trở lại ở hai ngôn ngữ và bốn
mức DPI. Thanh tab phía trên có điều hướng riêng khi tràn, giữ nút Tệp cố định;
khi đổi kích thước, tab đang chọn được đưa vào vùng nhìn thấy. Kiểm thử bao gồm
chọn tab cuối, quay lại và thu hẹp cửa sổ ở hai ngôn ngữ và bốn mức DPI. Chưa chụp
và đối chiếu giao diện thực tế sau thay đổi.

Ảnh cửa sổ hẹp 800 px cho thấy một phần icon của nhóm kế tiếp lộ ra ở mép vùng
cuộn. Các nút/tab bị cắt một phần nay được ẩn và chỉ nhận nhấn khi đã cuộn đủ vào
khung; nút rộng hơn chính khung vẫn cho phép cắt để còn thao tác được. Test headless
đi qua từng bước cuộn và xác nhận mọi lệnh/tab có thể nhấn tại 480 CSS px, hai ngôn
ngữ và 100–200% DPI; ảnh Direct2D 800 px dựng lại không còn nửa icon ở mép.
Mở rộng phép thử xuống 320 CSS px phát hiện bước cuộn nửa khung có thể bỏ qua
một nút rộng. Bước cuộn nhóm giờ dừng tại cạnh của điều khiển ẩn gần nhất; test
đã đi qua mọi nút/tab ở cả 320 và 480 CSS px, hai ngôn ngữ và bốn mức DPI.

Ảnh ribbon Home dựng ngoài màn hình bằng cùng `Frame` Direct2D/DirectWrite của
host (720 và 1.366 px ở 100%, 1.708 px ở 125%) cho thấy chuỗi xem trước kiểu
`AaBbĐđ` bị cắt ở một số ô. Đổi mẫu thành `AaĐđ`, vẫn có chữ tiếng Việt; dựng
lại và đo bằng DirectWrite cho năm kiểu của tài liệu thử cho thấy mẫu nằm trong ô.
Chạy lại bằng `BO_RIBBON_OUT=<thư mục> cargo test -p bo-gpu-win ribbon_offscreen -- --ignored`.
Ảnh là ribbon dựng riêng, không phải ảnh toàn cửa sổ hay so sánh với Word.
Với font khác, gallery đo chiều rộng mẫu theo font thật và chọn lần lượt `AaĐđ`,
`AĐ`, `Đ`; nếu ký tự đơn vẫn quá rộng, cỡ mẫu được giảm. Test headless bao gồm
các font giả lập rộng và kiểm tra lựa chọn cùng cỡ; chưa so tất cả font hệ thống.

Đã bổ sung phép dựng **toàn bộ cửa sổ** ngoài màn hình: tài liệu
`phase1-sample.docx`, phông hệ thống, giao diện tiếng Anh, zoom 100%, tỉ lệ 125%,
1.708 × 1.125 pixel. Ảnh gồm thanh tiêu đề, ribbon, thước, trang và thanh trạng thái;
backend dùng cùng `Frame` và bitmap trang như host. Lệnh dựng:
`BO_RIBBON_OUT=<thư mục> cargo test -p bo-gpu-win window_offscreen -- --ignored`.
Ảnh hữu ích để phát hiện lỗi cắt chữ và bố cục, nhưng không bao gồm compositor,
độ nét màn hình vật lý hay ảnh Word đối chứng; không dùng làm bằng chứng tương đương.

Ribbon Home native đã thêm tăng/giảm cỡ chữ theo các mốc của editor chung và xóa
định dạng, đặt cạnh hộp cỡ chữ. Danh sách cỡ bổ sung 13 và 48 pt cho khớp web.
Test nhấn nút kiểm tra 13 → 14 → 13 pt và xóa chữ đậm; test riêng kiểm tra các
biên 1, 8, 72 và 1.638 pt. Ảnh toàn cửa sổ ngoài màn hình được dựng lại sau sửa;
`cargo check -p bo-native-win` và test `bo-ui` đạt. Bản native đang mở chưa có
thay đổi này.

Nhóm Đoạn văn có thêm menu giãn dòng đơn, 1,5 và đôi, hiển thị lựa chọn hiện tại;
lệnh đi qua engine và hỗ trợ hoàn tác. Kiểm thử nhấn menu, áp dụng 1,5 và undo,
cùng kiểm tra chuỗi hai ngôn ngữ và nguồn icon, đều đạt. Ảnh Direct2D ngoài màn
hình đã dựng lại sau thay đổi; cửa sổ release đang mở chưa chứa menu này.

Menu native được đặt lại trong khung cửa sổ khi nút neo gần mép phải, và mở lên
trên nếu không đủ chỗ phía dưới. Kiểm thử vị trí và nhấn chọn qua 100–200% DPI;
ảnh toàn cửa sổ ngoài màn hình 800 × 600 với menu ngữ cảnh ở góc dưới phải cho
thấy toàn bộ menu nằm trong cửa sổ. Đây chưa phải thử tương tác trên màn hình thật.

Ảnh Direct2D của cả tám tab ở 125% cho thấy tab Xem lại có nút hàng thứ ba đè
lên nhãn nhóm. Các nút bình luận và thay đổi đã được sắp xếp trong hai hàng;
ảnh dựng lại không còn lỗi chồng lấn này. Test kiểm tra cạnh dưới mọi điều khiển
không vượt lên vùng nhãn nhóm, trên mọi tab, hai ngôn ngữ và bốn mức DPI. Đây là
kiểm tra bố cục BlueDoc, chưa phải đối chiếu với Word.

Trong ảnh tab Xem, nút ký hiệu định dạng và đặt lại thu phóng từng thiếu icon.
Đã ánh xạ chúng tới biểu tượng sẵn có của BlueOffice; ảnh dựng lại hiển thị cả hai.
Test kiểm tra mọi nút trong ribbon có biểu tượng tương ứng, bên cạnh nhãn và lệnh.

## Giao diện Rust tự vẽ — mốc N1 (ADR-0018, 2026-11-14)

`bluedoc-native.exe` (release, `apps/native-win`), `phase1-sample.docx`, 125 %, Ryzen 7 7435HS:
70 ký tự tiếng Việt gõ liên tục qua `SendInput`. Từ lúc cửa sổ nhận `WM_CHAR` tới khi
`Present` trả về: xử lý lệnh **0,63 / 0,80 ms** (trung vị / p95), vẽ dải thay đổi + present
**0,55 / 0,78 ms**, tổng **1,22 / 1,47 ms** (tối đa 5,0 ms). Bản Tauri cùng máy: phím→khung hình
trung vị ~35 ms. Khung hình còn chờ tới nhịp quét kế tiếp (≤ 16,7 ms ở 60 Hz) như mọi ứng dụng;
số đo chưa gồm phần này. `age_ms` (thời gian chờ trong hàng thông điệp) đo bằng `GetMessageTime`
có độ phân giải 10–16 ms nên ghi 0; chưa đo bằng camera / PresentMon.

