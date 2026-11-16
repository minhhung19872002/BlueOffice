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

Vòng 11: đo trên toàn bộ trang nhìn thấy (100/140/200%): số mức xám, tỉ lệ pixel đậm và mép cứng chênh bản đối chiếu ≤ 0,01. Chụp màn hình thật cả 6 menu mới: phát hiện và sửa lỗi menu mở ở góc trái trên cửa sổ (`static_id` thiếu id mới; thêm test mọi nút menu trên ribbon đều neo được menu). Đã áp dụng thử từ menu: chữ hoa đầu đoạn, hình oval.
Vòng 12: bấm thử trên giao diện cả 6 lệnh còn lại (Số trang, Ngày & giờ, Số dòng, Gạch nối, Chú thích, Dòng chữ ký) — chạy đúng. Phát hiện trường DATE hiện ngày UTC (14/11 lúc 5 giờ sáng 15/11 giờ Việt Nam); engine nhận độ lệch múi giờ từ host (`set_utc_offset_minutes`), DATE/TIME hiện giờ địa phương như bản đối chiếu, ngày của revision vẫn UTC.
Vòng 13: đo thêm trên tài liệu chữ không chân (Calibri 11, Arial 10, Calibri 14, tạo bằng bản đối chiếu). Phát hiện đoạn Arial bị vẽ bằng **Arial Narrow**: chỉ mục font hệ thống gom theo tên typographic (name id 16) nên Arial Narrow / Arial Black cũng là họ `Arial`, và việc chọn mặt chữ chỉ xét đậm/nghiêng. Sửa: mỗi mặt chữ đăng ký cả tên id 16 và id 1 (`Arial Narrow` tìm được bằng tên riêng), chọn mặt chữ ưu tiên độ rộng chuẩn (OS/2 usWidthClass 5) rồi độ đậm gần 400/700; bộ nhớ đệm chỉ mục lên phiên bản 2. Sau sửa: ba đoạn ngắt dòng trùng bản đối chiếu từng chữ; độ mịn 16 mức xám / pixel đậm 0,552 / mép cứng dọc 0,212 (đối chiếu 20 / 0,564 / 0,259).
Vòng 14: dò lưới gamma/contrast (7 bộ) và chế độ khử răng cưa (5 chế độ) trên cả tài liệu Times và tài liệu không chân, so cùng lúc với bản đối chiếu. Bộ mặc định (NATURAL_SYMMETRIC qua RenderingParams3, gamma 1,5, contrast 1,0) có tổng sai số nhỏ nhất (0,147; bộ tốt nhất trong lưới 0,138 — khác biệt nằm trong độ nhiễu đo); các chế độ khác lệch 0,23–0,55. Phần chênh còn lại ở chữ không chân (mép cứng dọc 0,21 so với 0,26) không đổi được bằng tham số DirectWrite công khai.
Thêm `BO_GRAY_MODE=gdinat|gdiclassic` (căn nét kiểu GDI): sai số 0,413 / 0,426, chỉ 9–13 mức xám — xa bản đối chiếu hơn chế độ mặc định (0,147).
Vòng 16: icon nhóm Paragraph của ribbon native vẽ lại theo thiết kế riêng (bảng `OVERRIDES` trong `icons.rs`, bộ JSON dùng chung với React không đổi): căn lề 4 dòng thay 3, chấm đầu dòng tròn đặc, mũi tên thụt lề tam giác đặc — đọc rõ ở 16–20 px như bản đối chiếu, không dùng hình của hãng khác.
Vòng 17: Dán và Sao chép vẽ lại có mảng màu đặc (bảng `SOLID` trong `icons.rs`, thiết kế riêng): bảng kẹp vàng ấm giữ trang trắng có dòng chữ; trang xanh nhạt chồng trang trắng. Khi nút bị vô hiệu chỉ vẽ nét.
Vòng 18: thêm hình vẽ có mảng màu (thiết kế riêng) cho Tìm, Bảng, Đầu trang, Chân trang, Ngắt trang, Lề, Hướng giấy, Cột; đã chụp tab Home / Insert / Layout trên bản build.
Vòng 19: bảng màu icon riêng chỉnh theo độ đậm/độ bão hoà đo trên ribbon bản đối chiếu (màu có sắc độ phổ biến nhất của nó: xanh 72,120,168 → 144,168,192; vàng 216,192,120). Mới: xanh nhấn #2b6ab8, xanh nhạt #b9d2ee (trước #d6e6fb, quá nhạt), vàng ấm #ecc67e. Sau đổi, các màu phổ biến nhất trên ribbon BlueDoc: 168,192,216 / 216,192,120 / 24,96,168. Thêm hình có mảng màu cho Hộp văn bản và Hình; chụp lại Home / Insert / Review / View.
Vòng 20: thử làm tròn cỡ chữ về pixel nguyên khi vẽ (giả thuyết cho chênh lệch mép chữ không chân) — không đổi số đo, đã gỡ. Thêm hình có mảng màu cho Mục lục, Chú thích cuối trang, Chú thích cuối tài liệu (tab References).
Vòng 21: hình có mảng màu cho Chọn người nhận, Hoàn tất trộn (Mailings), Đếm từ, Theo dõi thay đổi (Review), Chế độ xem (View). Tám tab ribbon đều đã chụp lại trên bản build: mọi nút lớn dùng kiểu mảng màu.
Vòng 22: so thêm 3 tài liệu corpus (cite-03-vn-thesis, fields-03-toc, lists-02-bullets) với bản đối chiếu ở 100%: bố cục, ngắt dòng, ký hiệu danh sách, thụt lề khớp. Khác biệt: chấm dẫn mục lục dừng sớm (khoảng trống tới 1,5 ô chấm trước số trang). Sửa: chấm tính ngược từ vị trí tab, chấm cuối kết thúc đúng chỗ số trang bắt đầu; đã chụp lại trên app — khớp.
Vòng 23: tham số vẽ chữ riêng cho font không chân (nhận biết qua `OS/2` PANOSE bSerifStyle 11–15, cờ `FaceInfo::sans`), đặt lại theo từng đoạn chữ khi vẽ: gamma 1,8 / contrast 3,0 (dò 12 bộ trên tài liệu không chân). Ở 100%: không chân — pixel đậm 0,588 / mép cứng dọc 0,251 / ngang 0,216 (đối chiếu 0,564 / 0,259 / 0,182; trước 0,552 / 0,212 / 0,140), tổng sai số 0,066 (trước ≈ 0,10); Times không đổi (sai số 0,019).
Vòng 24: tham số riêng chỉ áp cho chữ không chân nhỏ hơn 20 px (ở 120–140% bản đối chiếu vẽ mềm như chữ có chân; áp cho mọi cỡ làm chữ 140% quá cứng: mép dọc 0,269 so với 0,201). Sai số tổng (pixel đậm + mép cứng dọc + ngang) so với bản đối chiếu: Times 0,019 / 0,016 / 0,023, không chân 0,038 / 0,022 / 0,023 ở 100 / 120 / 140%.
Vòng 25: so thêm images-02-anchored-wrap (khớp), tables-03-nested-borders, layout-01-drop-caps-columns. Sửa: viền đôi (§17.18.2 `double`) vẽ hai nét dày bằng `w:sz`, cách một nét — trước chia 3 nên trên màn hình dính thành một nét đơn. Còn khác (chưa sửa): dòng trống bắt buộc sau bảng lồng gần như không cao trong bản đối chiếu; vị trí chữ hoa đầu đoạn và cách chia cột ở layout-01.
Vòng 26: chữ hoa đầu đoạn do bản đối chiếu tự tạo (thả 3 dòng và trong lề, Times 13) mở trong BlueDoc khớp: cùng vị trí chữ hoa, cùng bao chữ, ngắt dòng trùng từng chữ. Khác biệt ở layout-01 do file corpus tự sinh thiếu `w:spacing w:line/lineRule=exact` mà bản đối chiếu luôn ghi cho đoạn chữ hoa — không đổi engine. Bản web đã build lại (viền đôi), 185 test đối chiếu web/app đạt.
Vòng 27: hai mục còn treo kiểm tra lại bằng file do bản đối chiếu tự tạo — văn bản hai cột có đường kẻ giữa (8 đoạn Times 13) và bảng lồng trong ô: khớp (ngắt dòng, ngắt cột, đường kẻ, vị trí bảng con, chiều cao hàng). Khác biệt ở tables-03 / layout-01 chỉ xuất hiện với file corpus tự sinh. Tổng: 11 tài liệu đã so.
Vòng 28 (chủ dự án quyết định 2026-11-15: ribbon native làm giống bản đối chiếu về bảng màu, kiểu vẽ, số đo; hình icon vẫn tự vẽ, không sao chép): nét viền icon đổi từ màu chữ (#1b2430) sang xám #66696e — trung vị độ xám nét icon 106 so với 105 của bản đối chiếu (trước 30).
Vòng 29: thanh trạng thái theo bản đối chiếu — nền xám trung tính như thân ribbon (trước ánh xanh), chữ và nút zoom màu chữ chính (trước màu phụ), thêm ba nút chế độ xem (bố cục in / web / nháp, icon riêng) trước thanh zoom, nút đang dùng được làm nổi; đã bấm thử trên app (bố cục web: chế độ đổi, số trang 5 → 7).
Vòng 30: thước kẻ và nền quanh trang theo bản đối chiếu — nền thước và nền quanh trang 230,230,230 (trước 238,241,245 / 227,231,238 ánh xanh), vùng lề trên thước 198,198,198 (trước 211,217,226), vạch ¼ cm là chấm, ½ cm là vạch ngắn. Đo lại trên app: trùng từng giá trị.
Vòng 31: viền trang 198,198,198 (trước 195,202,214 ánh xanh) và vùng chọn chữ xám — 198 trên nền trắng như bản đối chiếu (trước xanh trong suốt ≈ 205,223,250); đo lại trên app sau khi nhấp đúp chọn từ: trùng. Không đổi: thanh tiêu đề giữ màu riêng của BlueDoc (màu xanh tiêu đề của bản đối chiếu là màu nhận diện thương hiệu, CLAUDE.md).
Vòng 32: nút đang bật trên ribbon (B, căn giữa, thước…) nền 198 + viền 1 px 128, góc vuông như bản đối chiếu (trước nền 216 không viền); đo lại trên app: trùng.
Vòng 33: gallery kiểu chữ thành một khung trắng chung viền 210 (trước mỗi ô một thẻ riêng), ô đang chọn viền ngoài 136 + dải trong 210 (trước viền màu nhấn), nút mở rộng cùng khung sau vạch ngăn — như bản đối chiếu.
Vòng 34: viền ô phông / cỡ chữ 148 (trước 211,217,226 gần như không thấy) và vạch ngăn nhóm ribbon 128 (trước 211,217,226) như bản đối chiếu; đo lại trên app: vạch ngăn 128.
Vòng 35: menu — mục đang chọn nền 198 + viền 128 + dấu tích (trước thanh màu nhấn), menu giãn dòng đủ 1.0 / 1.15 / 1.5 / 2.0 / 2.5 / 3.0 và thêm/bỏ khoảng trước/sau đoạn (12 pt) như bản đối chiếu; menu rộng theo nhãn dài nhất.
Vòng 36: thanh cuộn dọc như bản đối chiếu (rãnh 243, con trượt và nút mũi tên trắng viền 148, mũi tên 38; bấm rãnh lật một màn hình, kéo con trượt). Độ trễ gõ phím (`BLUEOFFICE_NATIVE_PERF` thêm prepare/paint/submit): xử lý 0,6 ms, vẽ lại trang 0,3 ms, vẽ giao diện 0,9 ms, chờ GPU + present ≈ 5 ms. Đo từ lúc gửi phím tới lúc vùng chữ đổi trên màn hình (chụp màn hình liên tục, cùng vùng 900×500, 30 lần): BlueDoc trung vị 38 ms, bản đối chiếu 63 ms. Bộ nhớ đệm layout chữ giao diện và hình icon (không đổi đáng kể thời gian khung hình).
Vòng 37: thước dọc bên trái (bật cùng thước ngang) như bản đối chiếu: dải 16 px, lề trên/dưới của trang hiện tại tô 198, vùng chữ trắng, số cm từ lề trên, chấm ¼ cm, vạch ½ cm. Chưa thêm nút mở hộp thoại ở góc nhóm (chưa có hộp thoại Phông chữ / Đoạn văn trong bản native).
Vòng 38: nút thiết lập ở góc hàng nhãn nhóm Font và Paragraph như bản đối chiếu (icon riêng), mở hộp thoại Phông chữ (phông, cỡ, màu) và Đoạn văn (thụt lề trái/phải/dòng đầu, khoảng trước/sau theo pt, giãn dòng theo dòng; nhận dấu phẩy thập phân) — dùng lại khoá chuỗi docStyles.dialog; test mở → gõ → Enter áp dụng.
Vòng 39: hộp thoại theo điều khiển chuẩn Windows như hộp thoại của bản đối chiếu — nền 240, ô nhập viền 122 (xanh 0,120,215 khi đang nhập), nút mặt 225 viền 173, nút mặc định viền xanh, rê chuột nền 229,241,251; nhãn màu chữ chính (trước: nền trắng, nút OK màu nhấn).
Vòng 40: hộp thoại Phông chữ thêm ô đánh dấu Đậm / Nghiêng / Gạch chân (kiểu điều khiển Windows, Space hoặc nhấp để bật) và khung xem trước cập nhật theo phông, cỡ, màu, kiểu đang nhập; test tích Đậm bằng Space rồi Enter.
Vòng 41: menu chuột phải như bản đối chiếu — cột icon bên trái (icon riêng), vạch ngăn giữa nhóm clipboard và nhóm chèn, Cắt / Sao chép mờ khi chưa chọn; chụp lại trên app (nhấp phải gửi bằng thông điệp, PrintWindow).
Vòng 42: màn hình File như bản đối chiếu — icon riêng cho từng mục, mục Info ở đầu cột được làm nổi, trang Info có cột Properties (số trang, số từ, tiêu đề, tác giả, người lưu cuối, ngày tạo, ngày sửa từ lệnh DocProperties; khoá backstage.props.*). Màu cột giữ màu nhấn BlueDoc (không dùng màu thương hiệu của bản đối chiếu). Hàm vẽ icon tách riêng để cửa sổ BlueSheet / BlueSlide không phải đổi.
Vòng 43 (2026-11-15, tính năng): tab ngữ cảnh như bản đối chiếu — con trỏ trong bảng hiện **Table Design** / **Table Layout** sau các tab thường, trên dải nền nhấn nhạt, nhãn màu nhấn (bản đối chiếu làm tối dải tab ~20 % sau tab ngữ cảnh: 43,87,154 → 34,70,123). Chèn bảng thì Table Design được chọn; nhấp vào bảng có sẵn chỉ hiện tab. Table Layout: Delete ▾ (ô / cột / hàng / bảng), Insert Row Above / Below, Insert Column Left / Right, Merge Cells (mờ khi chưa chọn nhiều ô) / Split Cells, AutoFit ▾, chia đều hàng / cột, Text Direction (xoay vòng như bản đối chiếu), lặp hàng tiêu đề, bảng → văn bản. Table Design: Shading ▾, Borders ▾. So hai tab cạnh bản đối chiếu: cùng nhóm, cùng thứ tự; chưa có gallery kiểu bảng, Select, View Gridlines, 9 nút căn ô (engine chưa có lệnh).
Vòng 44: đầu / chân trang — tab **Header & Footer** tự chọn khi mở (Page Number, Date & Time, Go to Header / Footer, Link to Previous, Different First Page, Different Odd & Even Pages, Close với icon riêng ô đỏ 216,99,68 dấu ✕ trắng như bản đối chiếu). Nhấp đúp vào lề trên / dưới mở đầu / chân trang của trang đó, Esc đóng. Thân văn bản mờ, đường gạch và nhãn "Header"/"Footer" đo trên bản đối chiếu: chữ thân 0 → 128 (đối chiếu 128), đường gạch 119 nét 3 px cách 3 px (trùng), nhãn nền 225 chữ 68 viền 119, 60×21 px ở 125 % (BlueDoc đo chữ thật). Chèn ảnh từ tệp (Insert › Pictures, hộp thoại của host; `BLUEOFFICE_PICTURE` cho chụp tự động) → tab **Picture Format** (Reset, Crop ▾: 1:1 / 4:3 / 3:2 / 16:9, oval / bo góc, Fill / Fit).
Vòng 45: **Navigation pane** (View › Navigation Pane, Ctrl+F như bản đối chiếu). Bản đối chiếu ở 125 %: rộng 437 px nền 230 viền phải 191, ô tìm trắng viền 171 cao 36 px, tab đang chọn đậm màu nhấn gạch 4 px, dòng hiện tại 194,213,242 viền 163,189,227. BlueDoc: 438 px, cùng màu nền / viền / ô tìm, tab Headings / Pages / Results, đề mục thụt theo cấp, nhấp đề mục / trang / kết quả để nhảy tới; Enter trong ô tìm liệt kê kết quả kèm ngữ cảnh, tô vàng trên trang, chọn kết quả đầu; vùng tài liệu và thước dọc lùi sang phải. Ctrl+H vẫn mở hộp thoại Thay thế.
Vòng 46: chính tả khi gõ — gạch sóng đỏ dưới từ sai, xanh dưới lỗi ngữ pháp, kiểm lại khi ngừng gõ 400 ms (engine giữ bộ nhớ đệm từng đoạn); so ảnh phóng 2× với bản đối chiếu: cùng vị trí, chiều cao, chu kỳ sóng. Nhấp phải vào từ sai: gợi ý ở đầu menu, Ignore All, Add to Dictionary, vạch ngăn, rồi Cut / Copy / Paste… như bản đối chiếu. Thanh trạng thái: icon sách xám 116 (đo trên bản đối chiếu) có ✕ khi còn lỗi, ✓ khi hết; các mục xếp theo chữ đo thật — số từ / sách / ngôn ngữ ở 127 / 262 / 309 px (đối chiếu 127 / 261 / 307; trước 167 / 261 / 314).
Vòng 47: **Proofing pane** (Review › Spelling & Grammar, F7, nhấp icon sách) bên phải như bản đối chiếu (438 px, nền 230): loại lỗi, câu chứa từ (đo và ngắt dòng thật, từ sai in đậm gạch màu), Suggestions (nhấp để thay), Ignore Once / Ignore All / Add to Dictionary; mỗi thao tác kiểm lại và chọn lỗi kế tiếp, hết lỗi báo "Nothing to correct was found". Thanh cuộn tài liệu nằm bên trái pane như bản đối chiếu.
Vòng 48: tab Review theo nhóm của bản đối chiếu — Spelling & Grammar + Word Count; New / Delete / Previous / Next comment; Track Changes với menu cách hiện thay đổi (Simple / All / No Markup / Original, có dấu tích) và menu dấu hiện (chú thích, chèn/xoá, định dạng, bong bóng); Accept ▾ / Reject ▾ (thay đổi này hoặc tất cả), Previous / Next. Còn thiếu so với bản đối chiếu: Thesaurus, Read Aloud, Accessibility, Language, Show Comments, Reviewing Pane, Compare, Protect.
Vòng 49: Insert › Table mở lưới 10 × 8 như bản đối chiếu — rê chuột tô ô (viền màu nhấn, nền cam nhạt), tiêu đề đổi thành "3x4 Table", nhấp là chèn bảng đúng cỡ và chuyển sang Table Design; dưới lưới: Convert Text to Table. Nhóm Pages của Insert thành hai hàng nút nhỏ có nhãn như bản đối chiếu nên tab Insert lại vừa cửa sổ 1500 px khi thêm Pictures (trước: phải cuộn, mất nhóm Symbols).
Vòng 50: tab View — Views có nút lớn cho từng bố cục (Print Layout / Web Layout / Draft, nút đang dùng được làm nổi), Zoom thêm One Page và Page Width (vừa trang theo chiều ngang hoặc cả trang trong vùng xem). Layout › Page Setup (nút góc nhóm, mục cuối menu Margins / Size như Custom Margins / More Paper Sizes): hộp thoại lề trên/dưới/trái/phải, khổ giấy, khoảng cách đầu/chân trang theo cm. Format Painter nhấp đúp giữ bật tới khi Esc như bản đối chiếu. Ctrl+G: Go To trang (số trang hoặc +n / −n).
Công cụ so sánh không gửi phím/chuột thật: `BLUEOFFICE_ACTIONS="insert.table.3x3;tab:tableLayout"` chạy lệnh giao diện lúc mở; bản đối chiếu đổi tab bằng UI Automation (`SelectionItemPattern.Select`), mở pane bằng `CommandBars.ExecuteMso`, luôn trên bản sao tài liệu.
Vòng 51 (2026-11-16, tốc độ gõ): đo lại bản `work/desktop-performance` trên máy này (phase1-sample, 100 %, gõ chữ thường + chuỗi VNI như Unikey gửi: xoá lùi + ký tự dựng sẵn, `BLUEOFFICE_NATIVE_PERF`): xử lý 0,69–0,89 ms, vẽ lại trang 0,44–0,58 ms, vẽ giao diện 1,32–1,55 ms mỗi phím (vòng 36: 0,6 / 0,3 / 0,9). Nguyên nhân tìm bằng đo: (1) cả cửa sổ vẽ lại mỗi phím — riêng thanh tiêu đề + ribbon ≈ 90 chữ, 165 path, 185 hình chữ nhật, mỗi lệnh vẽ tạo brush mới, mỗi path tạo stroke style mới; (2) thanh trạng thái đo lại hai chuỗi hai lần mỗi phím; (3) trong engine, mọi truy vấn theo đoạn chứa con trỏ (định dạng, ảnh, control, trường, form, section, cả lệnh gõ) lập danh sách đường dẫn của mọi đoạn (`body_paragraphs`) — tài liệu 300 trang: 4,4 ms chỉ để dựng kết quả một phím, lệnh gõ 5,8 ms; (4) chính tả khi ngừng gõ kiểm cả tài liệu (300 trang: 552 ms lần đầu, 16 ms mỗi lần sau). Sửa: thanh tiêu đề + ribbon và thanh trạng thái là hai lớp Direct2D lưu sẵn, chỉ vẽ lại khi băm nội dung vẽ đổi (`Painter::layer_begin/layer_end`); một brush đổi màu và một stroke style dùng chung; chuỗi thanh trạng thái chỉ đo khi đổi; tra đoạn theo id trực tiếp (`bo_model::index::para_path`), danh sách control khối lưu theo phiên bản và bỏ qua khi tài liệu không có; chính tả chỉ kiểm các trang đang xem ± 1, phần còn lại quét nền từng trang (ngân sách 3 ms mỗi nhịp, chỉ khi ngừng gõ); số kiểu bỏ dấu thanh (cũ / mới) đếm theo khối, khối đổi mới đếm lại, tối đa 8 khối mỗi lần kiểm. Kết quả (cùng máy, xen kẽ hai bản, mỗi lượt 146 phím):

| Mỗi phím (trung vị) | Trước | Sau | Vòng 36 |
| --- | --- | --- | --- |
| Xử lý | 0,69–0,89 ms | 0,61–0,65 ms | 0,6 |
| Vẽ lại trang | 0,44–0,58 ms | 0,44–0,49 ms | 0,3 |
| Vẽ giao diện | 1,32–1,55 ms | 0,37–0,39 ms | 0,9 |

Tài liệu 300 trang (`perf-big-300.docx`, đo không GPU): lệnh gõ trong engine 5,8 → 0,72 ms, xử lý một phím 4,0 → 0,9 ms; kiểm chính tả khi ngừng gõ 16,3 → 1,1 ms; quét nền cả tài liệu ~180 bước, bước dài nhất 6–8 ms (một trang dài), quét lại sau khi gõ 0,12 s. Lỗi gõ ở trang 250 khi đang xem trang 1: quét nền tìm thấy, biểu tượng sách hiện ✕ (test `sweep_finds_a_mistake_on_page_250`). Vẽ lại trang còn 0,44 ms (vòng 36: 0,3), chưa giảm thêm.
Thanh trạng thái: tài liệu không gắn ngôn ngữ (phase1-sample không có `w:lang`) hiện ngôn ngữ soạn thảo như bản đối chiếu — đọc qua COM trên bản sao: LanguageID 1033, "English (United States)"; BlueDoc hiện cùng chữ ở x 325 (đối chiếu 322).
Vòng 52: Table Design / Table Layout theo bản đối chiếu: Table Style Options sáu ô đánh dấu ba hàng hai cột; gallery kiểu bảng nhanh (6 kiểu riêng của Bluestar, ô xem trước vẽ bảng 5 × 5 theo tuỳ chọn đang bật, ô đang dùng viền 136 nền 210); Table Layout: nhóm Table với Select ▾ (ô / cột / hàng / bảng), nhóm Alignment chín nút căn ô (vAlign §17.4.84 + jc của đoạn trong ô) rồi Text Direction như bản đối chiếu. Engine: `tableEdit` thêm `look`, `cellAlign`, `select` (kiểu TS `TableEdit`). Còn thiếu: View Gridlines, Properties, Draw/Eraser, ô nhập cao/rộng ô, Cell Margins, Split Table, Border Styles / Pen / Border Painter.
Vòng 53: Header & Footer › Options là ô đánh dấu thật; chú thích nút ribbon có dòng tiêu đề đậm kèm phím tắt rồi một câu mô tả (lời riêng của Bluestar, khoá `docTips.*`, 49 lệnh). Ảnh so sánh: `scratchpad/show/doc-01…04-*.png`.
Vòng 54: Table Layout so cạnh bản đối chiếu lần hai — nhãn viết hoa đầu từ (Insert Row Below…), nút lớn ngắt nhãn hai dòng (Insert Row / Above, Text / Direction, Cell / Margins); nhóm Table: Select ▾, Properties (hộp thoại chiều rộng mong muốn, chiều cao hàng, chiều rộng cột); Merge thêm Split Table; Cell Size: ô số chiều cao / chiều rộng có mũi tên (bước 0,1 đơn vị, gõ thay giá trị đang chọn) cạnh hai nút chia đều, AutoFit ▾ bên dưới; Alignment: lưới 3 × 3 rồi Text Direction và Cell Margins (hộp thoại lề trái / phải ô); Data ▾ một nút lớn (lặp hàng tiêu đề, chuyển thành văn bản, sắp xếp A→Z / Z→A theo cột con trỏ). Đơn vị theo vùng như bản đối chiếu: đọc qua COM `Options.MeasurementUnit` = 0 (inch) trên máy có `iMeasure` = 1, `sDecimal` = "." → hiện 0.35" / 1.62" (đối chiếu 0.35" / 1.63"); máy hệ mét hiện cm với dấu thập phân của vùng; hàng tự động hiện chiều cao dàn trang đo khi ngừng gõ. Engine: `tableEdit` `rowHeight`, `columnWidth`, `splitTable`, `cellMargins`. Còn thiếu: View Gridlines, Draw Table / Eraser, Formula.
Vòng 55: Read Mode (nút đầu của ba nút chế độ xem trên thanh trạng thái, thứ tự Read Mode / Print Layout / Web Layout như bản đối chiếu; View › Read Mode; Esc thoát): bỏ ribbon và thước, chữ dàn lại theo bề ngang cửa sổ ở 140 % (zoom đọc của bản đối chiếu) với lề hai bên; đã chụp cạnh bản đối chiếu cùng tài liệu — dòng ngắt tương tự, cùng cỡ chữ.
Vòng 56: Navigation › Pages hiện ảnh thu nhỏ của trang hai cột như bản đối chiếu (ảnh do host vẽ ở tỉ lệ nhỏ, 4 ảnh mới mỗi khung hình, trang vừa sửa vẽ lại khi ngừng gõ), số trang dưới ảnh, trang đang xem viền màu nhấn; rộng ≈ 150 px ở 125 % như bản đối chiếu, trang lẻ cuối căn giữa. Ảnh so sánh: `scratchpad/show/doc-02-table-layout.png`, `doc-05-read-mode.png`, `doc-06-nav-pages.png`.
Vòng 57: Read Mode — trang màn hình nền trắng không viền, không gạch sóng chính tả (như bản đối chiếu), hai nút tròn lật màn hình ở giữa hai mép (Trái / Phải / PageUp / PageDown cũng lật), thanh trạng thái đếm "Screen n of m".
Vòng 58: **Comments pane** (Review › Show Comments; dùng chung bên phải với Proofing pane, mỗi lúc một pane) bố cục theo vùng markup của bản đối chiếu, đo ở 125 % trên cùng tài liệu có hai luồng (một trả lời): huy hiệu chữ tắt tròn 30 px màu 0,120,212, tên tác giả đậm cách mép huy hiệu 44 px (đối chiếu 44), chữ chú thích bằng phông của tài liệu rộng 309 px (đối chiếu 309), trả lời sau vạch xám 198 cách huy hiệu 72 px và tên cách vạch 14 px (trùng); khoảng dọc tên → chữ 18 px như đối chiếu. Luồng đang chọn (con trỏ trong vùng neo hoặc nhấp) có Reply (hộp thoại) / Resolve ↔ Reopen / Delete qua lệnh engine `replyComment` / `resolveComment` / `deleteComment`; luồng đã giải quyết xám kèm nhãn Resolved. Bản đối chiếu cài trên máy là kiểu chú thích cổ điển (không có ô trả lời trong thẻ) nên chưa đặt ô nhập tại chỗ.
Vòng 59: Insert › Symbol mở lưới 5 × 4 ký hiệu dùng gần đây (mới nhất trước, nhớ trong phiên) + More Symbols… (lưới 16 cột: Latin-1, chữ Việt Ă Đ Ơ Ư ₫, dấu câu, tiền tệ, mũi tên, toán, Hy Lạp, hình); ký hiệu khối U+2600–U+27BF vẽ bằng phông ký hiệu đơn sắc để cùng nét với ô khác. Gallery của bản đối chiếu không mở được bằng COM/UIA nên chỉ có ảnh BlueDoc.
Vòng 60: Design › Document Formatting: Themes (ảnh thu nhỏ "Aa" bằng phông tiêu đề trên sáu màu nhấn, chủ đề đang dùng viền màu nhấn), Colors (hàng tám ô màu + tên), Fonts (cặp phông tiêu đề / nội dung viết bằng chính phông đó), Paragraph Spacing (Compact / Balanced / Open / Double) — chủ đề, bảng màu, cặp phông là của BlueOffice (`applyTheme`, `applyStyleSet`). Find / Replace / Go To là ba tab của một hộp thoại "Find and Replace" như bản đối chiếu (Ctrl+PageUp/PageDown hoặc nhấp tab; chữ Find what giữ khi chuyển tab). Ảnh so sánh: `scratchpad/show/doc-07…12-*.png`.
