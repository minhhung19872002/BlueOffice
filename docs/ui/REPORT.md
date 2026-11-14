# Báo cáo thị giác desktop BlueOffice

Trạng thái: **đang thực hiện, checklist chưa sạch**. Mục tiêu, phạm vi và các ngoại lệ được
cập nhật tại đây; số đo hiệu năng nằm trong [RESULTS.md](../perf/RESULTS.md).

## Nền tảng đang chuyển đổi

- Đã thêm Fluent UI React v9 và Fluent System Icons, dùng giấy phép MIT; xem
  [THIRD_PARTY_NOTICES.md](../../THIRD_PARTY_NOTICES.md). Logo và icon ứng dụng BlueOffice giữ nguyên.
- Đã tạo theme Fluent riêng theo màu nhấn của từng ứng dụng cho light/dark và theme tương phản
  cao; giữ font/token BlueOffice ở gốc. Thanh truy cập nhanh đã dùng Fluent Button và ba icon
  Fluent. Ribbon, dialog, menu và các điều khiển khác **chưa** được chuyển đầy đủ.
- [Ảnh trước, 125%](screens/before-125/manifest.json): ba app, light/dark, các tab ribbon và
  màn hình Tệp trên WebView2 thật. DPR trong WebView là 1,375 vì thang Windows 125% và zoom
  WebView. Các ảnh dùng tài liệu mới trống; chưa phải bộ ảnh kiểm tra chữ tài liệu mẫu.
- Bộ [ảnh sau, 125%](screens/after-125/manifest.json) đã chụp lại với thư mục khôi phục
  cách ly. Ví dụ: BlueDoc [trước](screens/before-125/doc-light-home.png) /
  [sau](screens/after-125/doc-light-home.png), BlueSheet
  [trước](screens/before-125/sheet-light-home.png) / [sau](screens/after-125/sheet-light-home.png),
  BlueSlide [trước](screens/before-125/slide-light-home.png) /
  [sau](screens/after-125/slide-light-home.png). Ảnh sau ban đầu có banner khôi phục không được sử dụng.

## Rà soát ban đầu

| Kiểm tra | Kết quả hiện tại |
|---|---|
| Nền, font và màu thương hiệu | Đã sửa Fluent không ghi đè font web và nền BlueOffice; đã chụp lại trên bản desktop mới; cần rà đầy đủ các màn hình. |
| Viền và độ sắc ở 100/125/150/200% | Chưa xác nhận; hiện mới chụp 125%. Chưa có màn hình thứ hai để thử kéo qua DPI khác. |
| Trang tài liệu | Đã sửa dải đen ở đầu trang: con trỏ có `position: static` và rộng bằng trang do CSS lớp phủ tạo động không được áp dụng trong desktop. Đưa toàn bộ CSS lớp phủ vào stylesheet đóng gói; [ảnh sau sửa](screens/fix-caret-125/doc-light-home.png) trên release Windows 125% không còn dải đen. Đã thêm kiểm thử kích thước/vị trí con trỏ vào desktop E2E. |
| Khoảng cách và cắt chữ | Một số nhãn nhóm ribbon thu gọn như “Giọn…”; chưa hoàn thành rà tất cả tab và sửa. |
| Dialog, menu, tooltip | Đã chụp 10 ảnh đại diện light/dark ở 125%: [manifest](screens/dialogs-125/manifest.json), ví dụ [BlueDoc](screens/dialogs-125/doc-light-dialog-font-effects.png), [BlueSheet](screens/dialogs-125/sheet-light-dialog-chart.png), [BlueSlide](screens/dialogs-125/slide-light-menu-table.png). Bộ mới đã chụp các lệnh đại diện bị thu gọn; tooltip và các dialog còn lại chưa được bao phủ. |
| DirectWrite và renderer GPU native | Chưa triển khai; xem [ADR-0017](../adr/0017-desktop-document-rendering.md). |
| Visual regression CI | Đã thêm lệnh chụp/so sánh tab, màn hình Tệp và một số dialog/menu; chỉ so khi có artifact baseline CI cùng viewport/DPR. Lần đầu hoặc thiếu artifact chỉ báo cáo; nhiều dialog/menu, tooltip và DPI khác chưa được bao phủ. |

Script [capture-desktop-ui.mjs](../../scripts/capture-desktop-ui.mjs) xác nhận tab được chọn
trước mỗi ảnh và ghi DPR vào manifest. [compare-desktop-ui.mjs](../../scripts/compare-desktop-ui.mjs)
từ chối so sánh khi kích thước hoặc DPR khác. Không dùng ảnh mô phỏng làm bằng chứng cho DPI thật.

## Kiểm tra tiếp theo

- Màn hình Tệp BlueSheet: nền hover được làm tối để chữ trắng đạt tương phản; bài kiểm tra
  accessibility BlueSheet đã qua trên bản web build lại. Test chuyển theme chờ trạng thái
  sáng ổn định trước khi đổi lần tiếp theo.
- Ảnh sửa con trỏ được chụp với executable SHA-256
  `4629ac6dd15d4ae8dbe7ea93348b04522ec5860de9baeb0bf3f31aec1331ba5c`.
  Bộ ảnh này mới có BlueDoc light 125%; không thay thế bộ ảnh đủ màn hình và DPI.

Script chụp có thêm các dialog/menu đại diện và báo cảnh báo khi không mở được điều khiển.
Ở bộ ban đầu, Giãn dòng và Thay thế của BlueDoc cùng Định dạng ô của BlueSheet bị thu gọn;
bộ mới đã mở được chúng qua nhóm ribbon và phím tắt. Hộp thoại hiệu ứng/giãn cách ký tự
không đại diện toàn bộ hộp thoại phông chữ.

## Ảnh bản release đo 10 lượt

[Bộ 80 ảnh](screens/after-immediate-125/manifest.json) được chụp ở Windows 125% trên
cùng SHA-256 `75e9508742bbb0f8fdaab4de7a2f8b7bc0d9e5dca759697d118820b222acda56`
như phép đo hiệu năng: ba app, light/dark, các tab và một số dialog/menu. Trước mỗi màn
hình, script đặt lại lựa chọn trong profile WebDriver tạm để trạng thái ngăn của app trước
không lọt sang ảnh sau. Ảnh BlueSlide dark đã được chụp lại sau khi phát hiện ngăn hiệu ứng
còn mở từ phiên trước. Chưa thể kết luận checklist sạch: vẫn thiếu nhiều dialog/menu, tooltip, tài liệu chữ mẫu,
các DPI khác và so sánh màn hình thứ hai.

Một thay đổi tiếp theo cho nhãn nhóm ribbon đã qua kiểm tra web trên các tab của ba app ở
độ rộng 720, 900, 1.164 và 1.366 CSS px; không còn nhãn nhóm bị cắt trong phép thử này.
Đây là kiểm tra trình duyệt, chưa xác nhận ảnh desktop mới hay các DPI vật lý.

Kiểm tra lại ngày 2026-11-14: build web và hai bài E2E Chromium cho nhãn ribbon và
menu tùy chọn dán đều đạt. Kiểm thử đơn vị mô phỏng đổi chế độ dán bất đồng bộ và
mở lại menu trong lúc chờ; E2E xác nhận luồng mở lại menu sau khi chọn chế độ.

## Độ nét giao diện Rust tự vẽ so với trình soạn thảo desktop (2026-11-14)

So sánh black-box `bluedoc-native.exe` với một trình soạn thảo văn bản desktop mở cùng
`tests/corpus/doc/phase1-sample.docx`, cùng máy (Windows 125%, ClearType bật), zoom 100%,
chụp cửa sổ 1500×1102 px vật lý rồi phóng 4× từng vùng.

| Hạng mục | Trước | Sau |
| --- | --- | --- |
| Icon ribbon | nét 1,67 px trải trên hai hàng pixel → nhòe | nét làm tròn xuống số pixel nguyên, căn vào tâm/biên pixel; chấm vẽ thành ô vuông đặc |
| B / I / U / abc / x₂ / x² / A▴ / A▾ | hình nét mảnh | chữ thật theo định dạng nó áp dụng, chỉ số và mũi tên màu nhấn |
| Ô phông chữ / cỡ chữ | không có mũi tên | tam giác ▾ bên phải |
| Chữ trong trang | ClearType giảm màu (độ phủ mực 0,115, có viền màu) | xám + khớp lưới dọc: độ phủ mực 0,120 so với 0,122 của bản đối chiếu, không viền màu |
| Chữ giao diện | ClearType hệ thống | ClearType hệ thống, chế độ GDI classic (`BLUEOFFICE_UI_TEXT=natural\|system` để so) |

`BLUEOFFICE_TEXT=light` giữ cách vẽ chữ trang cũ. Phép đo: tỉ lệ pixel tối/mờ và sắc độ
trên vùng thân văn bản (bỏ gạch đỏ chính tả của bản đối chiếu).

Vòng 2 (cùng ngày): thân ribbon nền xám trung tính `#f3f3f3` (ô nhập và ô kiểu trắng),
icon có màu nhấn theo bảng màu riêng (không mô phỏng icon của hãng khác), thêm Aa (đổi chữ
hoa/thường, menu), Tô nền đoạn (menu màu, §17.3.1.31 shd), Màu chữ, Tô sáng và ¶.
Đối chiếu thêm ở zoom 150%: độ phủ mực thân văn bản 0,1109 so với 0,1102 của bản đối
chiếu, tỉ lệ pixel đậm cao hơn (nét sắc hơn); ribbon ở DPI 100% và 125% đều khớp lưới pixel.
Vòng 3: thêm vào engine và ribbon viền đoạn văn (§17.3.1.24 pBdr, menu cạnh/hộp/lưới/bỏ)
và sắp xếp đoạn văn (thứ tự từ điển tiếng Việt, A→Z / Z→A, một bước hoàn tác); nhóm
Clipboard / Font / Paragraph của ribbon có đủ bộ điều khiển như bản đối chiếu. Icon vẫn là
thiết kế riêng (quy tắc clean-room: không chép icon của hãng khác).

Vòng 4: thêm danh sách đa cấp (4 mẫu, `defineList`) và hiệu ứng chữ (viền chữ, bóng, phát
sáng, phản chiếu); đã render bốn hiệu ứng trên trang qua `BO_WINDOW_COMMANDS`. So thêm trang 2
(`BO_WINDOW_WHEEL=1500`, cùng nội dung, zoom 100%): độ phủ mực 0,1051 so với 0,1073 của bản
đối chiếu, sắc độ 0 ở cả hai, tỉ lệ pixel đậm 0,649 so với 0,604. Tám tab ribbon render ở 125%
và tab Home ở 100% đều khớp lưới pixel.

Vòng 5: so từng tab ribbon với bản đối chiếu (8 tab chụp bằng KeyTips, cùng máy, 125%). Tỉ lệ
pixel đậm trong các pixel có mực (càng cao càng sắc; chỉ số thiên về icon nét đậm, icon
nhiều màu của bản đối chiếu cho giá trị thấp hơn):

| Tab | Đối chiếu | BlueDoc |
| --- | --- | --- |
| Home | 0,214 | 0,376 |
| Insert | 0,244 | 0,538 |
| Design | 0,238 | 0,546 |
| Layout | 0,231 | 0,622 |
| References | 0,347 | 0,505 |
| Mailings | 0,123 | 0,497 |
| Review | 0,325 | 0,480 |
| View | 0,185 | 0,468 |

Hiệu ứng chữ so ở 200% cùng chữ và tham số: bóng và viền chữ khớp; phát sáng chỉnh lại (nở nửa
bán kính, làm mờ nửa còn lại; màu halo 194,209,234 so với 194,208,233); phản chiếu lật quanh
đường cơ sở như bản đối chiếu.

Vòng 6: so cả 5 trang của `phase1-sample.docx` ở 100% (tỉ lệ pixel đậm trong pixel có mực,
trên khung chữ; sắc độ 0 ở cả hai bên mọi trang):

| Trang | Đối chiếu | BlueDoc |
| --- | --- | --- |
| 1 | 0,388 | 0,530 |
| 2 | 0,386 | 0,597 |
| 3 | 0,384 | 0,596 |
| 4 | 0,630 | 0,617 |
| 5 | 0,578 | 0,621 |

Trang 4–5 đo trên khung chữ tự dò (cùng kích thước ở hai bên: 786×121 và 983×64 px).
Icon chưa có màu chọn tay được tô nền nhạt cho khung chính (vẽ nền trước, nét sau).


Vòng 7 (2026-11-15): máy đối chiếu đặt cỡ chữ Windows 110% (Settings › Accessibility › Text size,
`HKCU\Software\Microsoft\Accessibility\TextScaleFactor`). Bản đối chiếu phóng chữ giao diện theo hệ số
này, BlueDoc thì chưa → chữ ribbon nhỏ hơn ~10%. `bluedoc-native.exe` giờ đọc hệ số (cập nhật khi đổi
cài đặt, `BLUEOFFICE_TEXT_SCALE=1.1` để thử) và nhân vào mọi cỡ chữ giao diện, nửa pixel làm tròn xuống.
Đo lại ở 125%: độ rộng 7 nhãn tab trùng từng pixel (42/52/51/81/63/52/37), nhãn nhóm "Clipboard"
66×16 px ở cả hai bên (trước: 64×14). Ghi chú: bản sao `out/desktop/bluedoc-native.exe` từ 14/11 sáng
chưa có các vòng chỉnh nét trước — đã thay bằng bản mới.
Vòng 8: thân ribbon theo hình học bản đối chiếu — dải lệnh cao 80 px logic (ba hàng nút nhỏ; nhóm
hai hàng như Font căn giữa dải, khoảng hàng 8 px), hàng nhãn nhóm theo chiều cao chữ đo được (giãn theo
cỡ chữ hệ thống). Ở 125%/110%: thân ribbon 133 px so với 135 px của bản đối chiếu (trước 115 px). Ô
kiểu trong gallery: chữ mẫu căn giữa phần trên, tên kiểu cỡ nhãn nhóm ở dòng dưới.
Vòng 9: đo theo cột trên bản cuối — thân ribbon 135 px (gồm viền) so với 136 px của bản đối chiếu.
Chụp lại 8 tab trên cả hai app: chữ và icon nét như nhau; khác biệt còn lại là số lệnh (bản đối chiếu
có nhiều lệnh hơn ở Insert/Layout, bố cục ba hàng nút nhỏ có nhãn) và hình icon (thiết kế riêng,
clean-room) — thuộc việc parity tính năng, không phải độ nét.

Vòng 10 (độ mịn chữ trang): ở zoom ≥ 140% chữ BlueDoc chỉ có 4 mức xám (0/59/119/182) — khi bảng
`gasp` của font tắt hinting, chế độ NATURAL_SYMMETRIC qua API cũ chỉ lấy mẫu 3× theo chiều ngang nên mép
cong thành bậc thang ("chưa mềm"). Bản đối chiếu có 16 mức (lấy mẫu 4×4). Chuyển sang
`DWRITE_RENDERING_MODE1_NATURAL_SYMMETRIC` (IDWriteFactory3): 17 mức ở mọi zoom. Đo trên thân văn bản
(cùng file, 125% DPI; mức xám / tỉ lệ pixel đậm / mép cứng dọc / mép cứng ngang):

| Zoom | Đối chiếu | BlueDoc |
| --- | --- | --- |
| 100% | 18 / 0,557 / 0,226 / 0,126 | 19 / 0,563 / 0,227 / 0,115 |
| 120% | 17 / 0,621 / 0,230 / 0,125 | 17 / 0,615 / 0,238 / 0,122 |
| 140% | 16 / 0,651 / 0,234 / 0,125 | 18 / 0,653 / 0,243 / 0,119 (trước: 4 / 0,708) |
| 160% | 16 / 0,686 / 0,222 / 0,113 | 29 / 0,697 / 0,223 / 0,110 |
| 200% | 31 / 0,756 / 0,231 / 0,118 | 29 / 0,766 / 0,231 / 0,110 |

`BO_GRAY_MODE=legacy` giữ cách cũ; `BLUEOFFICE_ZOOM=140` mở sẵn ở zoom đó để chụp so sánh.

Ribbon: thêm Hình (6 hình dạng), Hộp văn bản, Số trang (trường PAGE/NUMPAGES), Chữ hoa đầu đoạn,
Ngày & giờ (trường DATE), Dòng chữ ký (Insert); Số dòng, Gạch nối (Layout); Chèn chú thích (References).
Nhóm ba hàng nút nhỏ xếp kín dải lệnh như bản đối chiếu; tab Insert vừa cửa sổ 1500 px không cần cuộn.
