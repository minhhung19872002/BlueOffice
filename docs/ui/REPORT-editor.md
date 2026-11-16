# BlueEditor — so sánh với trình sửa PDF trên desktop

Đo black-box (Windows 125 %, chữ hệ thống 110 %), cùng tệp `phase1-sample.pdf`, cùng vùng client 1518 × 1036;
không dùng tên, icon hay màu thương hiệu của bản đối chiếu. Màu nhấn BlueEditor `#D96A0B` (chữ `#A84A05`),
thanh tiêu đề `#2F3E5C` — của riêng ta.

## Vòng 1 (2026-11-15)

Lõi `bo-pdfread`: lưu viết lại toàn bộ (nén, object stream tuỳ chọn) / lưu nối thêm; mã hoá lại bằng handler gốc,
mở tệp có mật khẩu, đặt/gỡ mật khẩu; chèn/xoá/di chuyển/đảo/xoay/trích/tách/gộp trang; hoàn tác mọi sửa đổi;
đối tượng trang (dòng chữ, ảnh, hình, form) — sửa chữ tại chỗ bằng font gốc hoặc font Unicode nhúng
(giữ serif/sans, đậm, nghiêng), di chuyển/co giãn/xoá, thêm chữ/ảnh; biểu mẫu AcroForm (điền, đặt lại, làm phẳng);
sửa/xoá chú thích; biên tập xoá đúng glyph dưới vùng; ký PAdES (.p12).

Cửa sổ `blueeditor-native.exe`: dùng chung cửa sổ/host với BlueReader (`Profile`, `Look`, `PdfWindow`); ribbon
Home/Convert/Edit/Organize/Comment/View/Form/Protect/Help; lưới trang ở tab Organize; Sửa chữ / Sửa đối tượng /
Thêm chữ / Thêm ảnh; điền trường, sửa chú thích, biên tập, ký; chuyển sang Word/Excel/PowerPoint/PNG/văn bản,
tạo PDF từ tệp BlueDoc/BlueSheet/BlueSlide.

| Hạng mục (125 %) | Bản đối chiếu | BlueEditor |
| --- | --- | --- |
| Thanh tiêu đề | 45 px, tiêu đề giữa | 45 px, `#2F3E5C`, tiêu đề giữa |
| Chữ tab “File” / nhãn “Hand” | y 57–68 / y 133–144 | y 57–69 / y 133–145 |
| Icon ribbon | y 90–120 | y 91–121 |
| Đường dưới ribbon / tab tài liệu / đỉnh trang | 180 / 190–224 / 232 | như nhau |
| Nền khung / đường kẻ / vùng trang / nút nhấn | trắng / `#E0E0E0` / `#F0F0F0` / `#E2E2E2` | như nhau |
| Zoom mặc định | 99,57 % | 99,63 % |
| Chữ thanh trạng thái | cao 13 px, y 1004 | cao 12 px, y 1010 (lệch 6 px) |
| Gõ chữ tại chỗ (25 ký tự) | — | p50 35,7 ms, p95 69 ms |
| Mở 5 trang / 1 202 trang | — | 37 ms / 370 ms (trang đầu 71 / 389 ms) |

Còn lại: ô “Tell me”, nút tài khoản; sửa chữ một dòng (chưa reflow); sang PowerPoint là ảnh trang; ký chưa có
dấu thời gian / hình chữ ký; tối ưu độ trễ gõ (phân tích trang 3–4 lần mỗi phím); chữ thanh trạng thái thấp 6 px.

## Vòng 2 (2026-11-15)

Ảnh so sánh (bản đối chiếu trên/trái, BlueEditor dưới/phải): `scratchpad/show/editor-01…05-*.png`.

**Gõ chữ.** Sửa chữ thành phiên theo đoạn (`bo_pdfread::BlockEdit`): phân tích trang một lần khi bấm vào đoạn,
các run của đoạn được vô hiệu trong bản sao nội dung (prefix), mỗi phím chỉ dàn lại đoạn (ngắt từ theo bề rộng,
giữ căn lề, thụt dòng đầu, khoảng cách dòng, kiểu từng ký tự) và ghi luồng không nén; nén khi kết thúc phiên
hoặc khi lưu. Bề rộng ký tự đo một lần cho mỗi (kiểu, ký tự); phông nhúng dự phòng lưu thô, nén khi lưu.

| WM_CHAR gửi bằng PostMessage, `phase1-sample.pdf`, 25 ký tự | Vòng 1 | Vòng 2 |
| --- | --- | --- |
| Tổng mỗi phím p50 / p95 | 35,7 / 69 ms | 9,4 / 15,9 ms |
| Xử lý phím (handle) p50 | ~28 ms | 1,6 ms |

**Đoạn văn.** Dòng cùng đường cơ sở gộp thành dòng nhìn thấy, các dòng cùng cỡ, bước dòng đều, mép trái/giữa/phải
thẳng hàng gộp thành đoạn với kiểu căn (trái, giữa, phải, đều hai bên, thụt dòng đầu). Trên mẫu: “Điều 1–4”
nhận đúng 4/4/4/3 dòng, căn đều; quốc hiệu căn giữa. Enter ngắt dòng, mũi tên lên/xuống, chọn bằng Shift/kéo,
Ctrl+B/I.

**Bảng Format** (khung phải như bản đối chiếu, rộng 437 px ở 125 %): Text Style (phông, cỡ, màu), B I U S chỉ số
trên/dưới, căn trái/giữa/phải/đều, tăng/giảm lề, gạch đầu dòng/đánh số, giãn dòng, tỉ lệ ký tự, giãn ký tự, giãn
từ (giãn đoạn chỉ hiển thị). Zoom vừa trang khi khung hiện: 68,42 % (đối chiếu 68,47 %). Bật Edit Text chuyển
sang tab Edit bố cục như bản đối chiếu (các lệnh chưa có hiện mờ).

**Ký số.** Kéo khung chữ ký trên trang bất kỳ; hình chữ ký là bố cục riêng (tên hoặc ảnh bên trái, “Ký số bởi”,
tên, ngày, lý do bên phải) vẽ bằng đường viền glyph nên không cần phông; dấu thời gian TSA (B-T) tuỳ chọn.
Bảng Chữ ký (cột điều hướng, Protect › Bảng chữ ký) kiểm `verify_pdf`: xanh = toàn vẹn, hổ phách = sửa sau khi ký,
đỏ = sai chữ ký; chi tiết người ký, TSA, thời điểm.

**Chuyển đổi.** PowerPoint: mỗi trang một slide, ảnh trang không có chữ phía sau + hộp chữ sửa được theo từng
đoạn (phông, cỡ, đậm, nghiêng, màu, căn). Word: đoạn văn với căn lề, thụt dòng đầu, bước dòng chính xác, khoảng
trước, kiểu run, khổ và lề trang, ngắt trang. Excel: ô theo cột dò từ mép trái.

**Ghi chú.** Typewriter, Callout, Drawing (chữ nhật, bầu dục, đường, mũi tên, đa giác, đám mây), Pencil, Stamp
(thiết kế và chữ riêng), tệp đính kèm; màu, độ mờ đục (/CA), độ dày (/BS) trong khung Format.

**Bảo vệ / Biểu mẫu / Thiết kế trang.** AES-256 (R6) mặc định hoặc AES-128, hộp quyền (in, sao chép, sửa,
ghi chú/điền, sắp trang); trường text, check, radio, ngày, combo, list, nút (kéo khung → thuộc tính); tab Organize:
Watermark, Background, Header & Footer (`<<page>>`, `<<pages>>`, `<<date>>`), Bates; Tìm & thay thế.

| Hạng mục (125 %) | Bản đối chiếu | BlueEditor vòng 2 |
| --- | --- | --- |
| Thanh trạng thái | 45 px, ô trang 28 px cách mép 4 px, chữ y +13 | 45 px, ô 28 px cách mép 4 px |
| Chữ tab tài liệu “Start” | x 24, y 204–214 | x 24, y 204–213 |
| Khung Format: Text Style / hộp phông / hàng B / hàng căn | y 250 / 275 / 350 / 405 | ±3 px |

Còn lại: Link & Join Text, chính tả, Reflow, Article Box, liên kết, bookmark, cross reference, image annotation,
audio/video, 3D, nhận dạng trường, barcode/ảnh/chữ ký trường, giãn đoạn; chọn chữ có công cụ ghi chú của Reader
đang bật vẫn đi qua công cụ ghi chú của Editor; hộp “Tell me”.

## Vòng 3 (2026-11-16)

Ảnh so sánh (bản đối chiếu trên, BlueEditor dưới): `scratchpad/show/editor-08…20-*.png` (18–20: bản cuối).

**Tab Edit vừa 1 536 px.** Số đo nút lớn đưa vào `Look` (`big_pad`, `big_pad_one`, `big_min`, `big_gap`,
`sep_space`, `arrow_w`; giá trị BlueReader giữ nguyên): nhãn 16,2 (chữ “Hand” 23–56 × 133–144 như bản đối
chiếu), nhãn hai dòng + 18 px, một dòng + 12 px, nút tối thiểu 50 px, sát nhau, 1 px trước / 8 px sau vạch
ngăn, mũi tên thả xuống cộng 8 px vào nút hai dòng. Nút bị tắt vẽ xám.

| Tab Edit (125 %, chữ 110 %) | Bản đối chiếu | Vòng 2 | Vòng 3 |
| --- | --- | --- | --- |
| Nút Hand | 14–65 | 13–70 | 13–64 |
| Vạch ngăn nhóm tab Edit | 123 / 470 / 677 / 748 / 843 / 1126 / 1437 | 153 / 542 / 757 / 843 / 957 / 1270 / tràn | 117 / 470 / 676 / 748 / 845 / 1131 / ≈1437 |
| Vạch ngăn nhóm tab Home | 363 / 612 / 743 / 916 / 1043 / 1226 | — | 359 / 627 / 754 / 923 / 1051 / 1238 |

**Mọi công cụ tab Edit hoạt động** (`editor_tools.rs`; lõi `bo-pdfread`: `links_edit`, `media_edit`,
`field_detect`, `optimize`): Link & Join Text (bấm các khối chữ theo thứ tự, Enter nối vào khối đầu, một bước
hoàn tác), Check Spelling (quy tắc tiếng Việt + từ điển tiếng Anh đóng gói; Thay / Thay tất cả / Bỏ qua),
Add Shapes (chữ nhật, bo góc, bầu dục, đường — path trong nội dung trang), Reflow Editing (hết sửa đoạn thì
phần bên dưới dịch theo), Article Box (`/Threads`, bead vòng), Web Links (tạo từ http/https/www/e-mail trong
chữ; gỡ mọi liên kết web), Link (địa chỉ web hoặc trang), Bookmark (thêm, thêm con, đổi tên, xoá, lên/xuống —
viết lại outline một bước; chọn trong khung Bookmarks), Cross Reference, Image Annotation (`Screen` có ảnh),
Audio & Video (`Screen` + rendition nhúng tệp), 3D (U3D/PRC; tệp khác báo rõ). Tab Form: nhận dạng trường
(gạch dưới / chấm dẫn → ô chữ, ô vuông rỗng → hộp chọn, khung rỗng → ô chữ), trường chữ ký, ảnh, mã vạch.

**Khác.** Giãn đoạn trong khung Format sửa được; chọn công cụ của Reader trên ribbon tắt mọi công cụ Editor
(lỗi vòng 2); ô tìm lệnh bên trái Find (gõ → danh sách lệnh, Enter chạy); trang File như bản đối chiếu
(Properties, Create, Open với tài liệu gần đây / Computer, Save — xám khi chưa sửa, Save As, PDF Optimizer,
Export, Print, Revert — đọc lại từ bản đã lưu, Close, Preferences — tên tác giả ghi chú, thu phóng và hiển thị
trang khi mở, đơn vị đo; cột trái 262 px `#EAEAEA`, mục cách 47 px, mục chọn hình viên thuốc màu nhấn, lệch ≤ 3 px);
PDF Optimizer giảm ảnh trên ngưỡng ppi (JPEG của `bo-raster`, Flate cho ảnh phẳng), bỏ đối tượng thừa, object
stream, báo kích thước trước/sau, hoàn tác được. Tab Home như bản đối chiếu (Clipboard ▾, Bookmark, Reflow,
Edit Object ▾, SnapShot của BlueReader vòng 4 sau khi gộp `work/desktop-performance`; From Scanner / Quick
Recognition hiện xám giữ chỗ); tab View dùng bố cục của bản đối
chiếu; Protect › Whiteout; Organize › Crop Pages (lề mm, `/CropBox`); Comment › Summarize Comments.
Mã vẽ ghi chú trùng giữa `markup` (Editor) và `annot_draw` (Reader) gộp một đường: các loại cả hai cùng vẽ đi
qua `annot_draw`; `markup` chỉ còn đám mây, dấu, tệp đính kèm.

**Tốc độ** (các agent khác build song song):

| Đo | Vòng 2 | Vòng 3 |
| --- | --- | --- |
| Gõ trong đoạn, tổng mỗi phím p50 / p95 | 9,4 / 15,9 ms | 6,6 / 8,9 ms |
| — vẽ trang (prepare) p50 | 6,1 ms | 3,7 ms (chỉ vẽ lại dải các dòng của đoạn: `PdfStore::update_band`) |
| Mở 1 202 trang / trang đầu (`--bench`) | 370 / 389 ms | 221 / 231 ms |
| Cuộn 1 202 trang, mỗi nấc WM_MOUSEWHEEL p50 / p95 | — | 1,5 / 1,7 ms |
| Lưu tệp 100 trang 6,8 MB sau khi xoay trang (ghi lại toàn bộ) | — | 13,4 ms |

Còn lại (so từng tab, ảnh 11–20): From Scanner, Quick Recognition;
Convert: From Scanner/Clipboard/Web Page, Form, PDF Portfolio, From Template, Export All Images, To HTML/Other,
OCR; Organize: Move/Duplicate/Replace/Swap/Interleaving, Resize Pages, Flatten, Format Page Numbers; Comment:
Area/Search & Highlight, Measure, Accounting Calculator, Eraser, Import/Export; View: Text Viewer, Ruler, Page
Transitions, Read, Compare; Form: Page Templates, XFA, Calculation Order, Tooltip, Form to sheet, Import/Export,
JavaScript; Protect: Hidden Data, Restrict Access, Batch Encrypt, Sensitivity, Time Stamp, Validate; File: Action
Wizard, Batch Print, Index, Share, Revert, Preferences, Skins; nút “⋮” và nút kiểu tìm cạnh Find.

## Vòng 4 (2026-11-16)

Ảnh so sánh (bản đối chiếu trên, BlueEditor dưới): `scratchpad/show/editor-21…34-*.png (28–34: bản cuối)`.

**Ribbon.** Nút hàng (`Kind::Row`, `editor_rows_win.rs`): icon nhỏ, nhãn bên phải, ba nút một cột như
Move / Duplicate / Replace của bản đối chiếu (hàng cách 29 px, icon 20 px cách mép cột 5 px, nhãn cách 32 px,
cột hết 8 px sau nhãn dài nhất). Mũi tên thả xuống của nhãn hai dòng chỉ nới nút khi dòng có mũi tên là dòng
dài hơn (lỗi làm nhóm Zoom … Rotate View rộng thêm 16 px). Mỗi tab kết thúc bằng một vạch ngăn như bản đối
chiếu. Lệnh chưa có (From Web Page, OCR, Form, PDF Portfolio, Preflight, Page Transitions, Page Templates, XFA,
Accounting Calculator, Rearrange) vẽ xám, rê chuột hiện chú giải. Icon trang (Background, Header & Footer,
Bates, Format Page Numbers) chuyển sang nét viền + một chi tiết màu nhấn, không tô khối.

| Vạch ngăn (125 %) | Bản đối chiếu | Vòng 3 | Vòng 4 |
| --- | --- | --- | --- |
| Home | 363 / 612 / 743 / 916 / 1043 / 1226 | 360 / 628 / 755 / 924 / 1052 / 1239 | 360 / 614 / 741 / 910 / 1033 / 1220 |
| Organize | 123 / 669 / 852 / 921 / 1396 | — | 118 / 665 / 853 / 920 / 1402 |
| Convert | 123 / 764 / 853 / 1108 / 1367 / 1448 | — | 118 / 769 / 859 / 1120 / 1388 / 1467 |
| Edit | 123 / 470 / 677 / 748 / 843 / 1126 | 117 / 470 / 676 / 748 / 845 / 1131 | 118 / 471 / 677 / 749 / 846 / 1132 |

**Organize** (`bo-pdfread::pages_edit`): Move (trang, trước trang N), Duplicate (bản sao đứng sau trang gốc,
ghi chú chép theo), Replace (trang chọn thay bằng trang của tệp khác), Swap, Interleaving (xen kẽ với tệp khác,
tuỳ chọn lấy ngược), Resize Pages (khổ mm, co giãn nội dung bằng `cm`, ghi chú dời theo), Flatten (vẽ ghi chú và
trường vào nội dung trang), Format Page Numbers (`/PageLabels`: 1, i, I, a, A, tiền tố, số đầu; trang sau giữ
nhãn). Mỗi lệnh một bước hoàn tác.

**Convert** (`extract`): Export All Images (JPEG giữ nguyên luồng, ảnh khác giải mã ra PNG, mỗi ảnh một lần),
To HTML (một tệp, mỗi trang một khung đúng khổ, dòng chữ và ảnh đặt đúng chỗ, ảnh `data:`), From Clipboard (host
đọc `CF_DIB` → trang PDF đúng cỡ ảnh ở 96 dpi), From Template (trang trắng A4/A3/A5/Letter/Legal/Tabloid), menu To
Office / To Image (PNG, JPEG) / To Other.

**Comment** (`exchange`): Import XFDF/FDF (ghi chú → chú thích mới, giá trị trường), Export XFDF hoặc đoạn được
tô sáng (.txt); Eraser, Area / Search & Highlight, Measure, Create (dấu từ ảnh) dùng công cụ của BlueReader —
chọn chúng tắt công cụ của Editor.

**View**: Text Viewer, Toggle Ruler, lưới hiển thị trang, Read (giọng Windows), Compare (chọn tệp thứ hai →
khác biệt chữ từng trang theo LCS, hai cột trái/phải đỏ/xanh, mở thành PDF mới qua BlueDoc).

**Protect** (`sanitize`): Hidden Data (hộp chọn có số lượng: siêu dữ liệu, tệp đính kèm, ghi chú, trường biểu
mẫu, lớp ẩn — nội dung `/OC … BDC … EMC` của nhóm OFF bị cắt, dấu trang, liên kết, JavaScript/hành động), menu
Secure, Restrict Access (chỉ mật khẩu quyền), Time Stamp Document (`/DocTimeStamp` RFC 3161 từ TSA), Validate.

**Form**: Calculation Order (`/CO`), Add Tooltip (`/TU`), Form to sheet (CSV / .xlsx qua BlueSheet), Import /
Export (XFDF, FDF, CSV), Actions (nút: đặt lại biểu mẫu, hiện/ẩn trường — chạy khi bấm nút lúc điền).

**Hành vi Edit Text** (so với quy ước của bản đối chiếu; bàn phím gửi bằng PostMessage không mang trạng thái
Ctrl nên phần này đối chiếu theo quy ước Windows): Ctrl+←/→ theo từ (Shift mở rộng chọn), Ctrl+Backspace/Delete
xoá theo từ, Ctrl+C / Ctrl+X / Ctrl+V (chữ thuần từ host, tab thành dấu cách, xuống dòng thành ngắt dòng),
Ctrl+Z / Ctrl+Y trong phiên lùi/tiến từng cụm gõ (ngắt ở dấu cách, xoá, Enter, dán), hết cụm thì hoàn tác của
tài liệu lấy lại cả phiên.

Còn lại: Page Transitions, OCR, From Web Page, PDF Portfolio, Preflight, Accounting Calculator, XFA, Page
Templates (xám); lưới nút nhỏ tab Comment hẹp hơn bản đối chiếu ~23 px; nhãn "Text Box" hai dòng (bản đối chiếu
"Textbox"); undo trong phiên của bản đối chiếu chưa đo được bằng input gửi qua PostMessage.

## Vòng 5 (2026-11-16)

Gộp BlueReader vòng 6 (bộ đọc đối tượng lười `store::Document`): mọi đường ghi của BlueEditor (sửa chữ / đối
tượng, lưu toàn bộ / nối thêm, mã hoá, tối ưu, sắp trang, mẫu trang) đi qua `DerefMut` như trước — 113 test
`bo-pdfread` và các test editor của `bo-ui` xanh sau khi gộp.

**Edit Object** (`editor_arrange.rs`, lõi `bo-pdfread::arrange`): kéo bám vào mép / giữa trang và mép / giữa đối
tượng khác trong 6 px màn hình, vẽ đường dẫn hồng; Shift giữ hướng ngang / dọc (chỉ trục tự do còn bám), Shift ở
góc giữ tỉ lệ; ←↑→↓ dời 1 pt, Shift 10 pt, đối tượng vẫn được chọn; Ctrl+kéo chép (bản gốc giữ nguyên, bản chép được
chọn: ảnh / path / form chép ngay sau bản gốc trong cùng trạng thái đồ hoạ, chữ chép với đủ trạng thái chữ); kéo
khung trên chỗ trống chọn mọi đối tượng chạm khung (Shift thêm vào); Ctrl+A chọn cả trang; khung Format khi Edit
Object hiện mục **Arrange** như bản đối chiếu: Page Center (dọc, ngang, cả hai), Alignment (trái, giữa, phải,
trên, giữa, dưới — cần ≥ 2 đối tượng), Distribute (ngang, dọc — cần ≥ 3), thêm Rotate & Flip (xoay ±90°, lật
ngang / dọc quanh tâm vùng chọn); chuột phải mở menu cùng các lệnh. Mỗi lần kéo / dời / sắp xếp là một bước hoàn
tác (`transform_each` ghi mọi đối tượng một lượt).

**Organize**: Ctrl+bấm thêm / bớt một trang; thanh trượt cỡ hình thu nhỏ ở góc dưới phải lưới (50–250 %) và
Ctrl+lăn chuột; Ctrl+C / Ctrl+X / Ctrl+V chép, cắt, dán trang (dán sau trang chọn, sang được tài liệu khác đang
mở); kéo trang thả lên tab của tài liệu khác chép trang sang cuối tài liệu đó. Kéo đổi thứ tự với vạch chèn, mũi
tên / Shift / Home / End / Delete đã có từ vòng 1.

**Lệnh xám đã làm**: Form › Page Templates (đặt tên trang hiện tại làm mẫu `/Names /Pages`, tạo trang từ mẫu ngay
sau trang hiện tại — một bước hoàn tác, xoá mẫu; mẫu giữ qua lưu), Convert › PDF Portfolio (danh sách tệp nhúng
`/EmbeddedFiles` có cỡ và mô tả, chỉ đọc: Open mở PDF thành tab, tệp khác lưu ra; Extract lưu các tệp chọn),
Protect › Secure › Batch Encrypt (chọn các tài liệu đang mở và thêm tệp, một mật khẩu mở / quyền, AES-256 hoặc
128, lưu bản `-protected.pdf` cạnh nhau; tệp đã có mật khẩu bị bỏ qua và báo tên). Accounting Calculator,
Preflight, Sensitivity không làm (theo yêu cầu); OCR vẫn xám.

**Tốc độ sau bộ đọc lười** (bản release, máy đang chạy song song build của các agent khác):

| Đo | Vòng 3–4 | Vòng 5 |
| --- | --- | --- |
| Mở sổ tay 1 202 trang / trang đầu (`--bench`, 3 lần) | 221 / 231 ms | 37–58 / 48–70 ms |
| Cuộn 1 202 trang, mỗi nấc p50 / p95 (60 nấc) | 1,5 / 1,7 ms | 1,6 / 2,6 ms |
| Gõ trong đoạn, tổng mỗi phím p50 / p95 (25 ký tự) | 6,6 / 8,9 ms | 8,0 / 18,0 ms (xử lý phím p50 1,7 ms) |
| Xoá một trang rồi Lưu (ghi lại toàn bộ): 100 trang 6,8 MB / 1 202 trang 10 MB | 13,4 ms (100 trang) | 15,7 / 63,4 ms |
| Bộ nhớ sau khi mở sổ tay (working set / private) | 126 / 148 MB | 90 / 129 MB |

Lưu lần đầu sau khi sửa nạp `lopdf::Document` đầy đủ một lần (`DerefMut`), nằm trong 63 ms của tệp 1 202 trang.

**Khung Format theo loại đối tượng** (`editor-35`, `editor-38`): chọn khối chữ bằng Edit Object hiện Text Style,
Split (bốn nút, xám — chưa có tính năng), Arrange theo thứ tự và khoảng cách của bản đối chiếu (125 %: Split +467,
Arrange +566 px từ đỉnh khung; tiêu đề mục x 27 có đường kẻ tới mép); khung cuộn bằng lăn chuột; ảnh / path chỉ hiện
Arrange. Hàng Arrange: Page Center bước 90 px, Alignment / Distribute bước 60 px, tiêu đề phụ +26 / +119 / +201 px
(lệch ≤ 1 px).

**Portfolio** (`editor-39`): mở PDF portfolio hiện danh sách chi tiết như bản đối chiếu (đường dẫn “Home ▸”, hàng
tiêu đề màu nhấn của ta, cột Name / Description / Modified / Size đo từ mép phải −358 / −210 / −85 px, hàng 46 px);
nhấp đúp mở PDF thành tab hoặc lưu tệp khác; Convert › PDF Portfolio đổi sang trang bìa.

**So từng tab** (`editor-40…47`, bản đối chiếu trên): cột nút hàng Organize Move / Split lệch ≤ 5 px (icon 457 / 558
so với 459 / 558, nhãn 471 / 574 so với 476 / 576) nên giữ bố cục cột chung của BlueReader; Page Templates mở hộp
Name / Add / Spawn / Delete / Go to / Close (bản đối chiếu: Add, Replace, Rename, Delete, Goto).

Còn lại: Text Style trong khung Edit Object chưa hiện phông / cỡ của khối được chọn (bản đối chiếu hiện); mục ảnh
của khung khi chọn ảnh; Split chưa làm; Rename / Replace mẫu trang; nút thả xuống của Select; Page Transitions,
OCR, From Web Page, XFA, Accounting Calculator, Preflight vẫn xám.

## Vòng 6 (2026-11-16)

Ảnh so sánh (bản đối chiếu trái / trên, BlueEditor phải / dưới): `scratchpad/show/editor-50…*.png`.
Gộp `work/desktop-performance` (BlueReader vòng 7: giới hạn bộ nhớ, khung cửa sổ `enable_full_client`).

**Khung Format khi Edit Object** (`editor_objtext.rs`, lõi `bo-pdfread::text_ops`): mục Text Style hiện phông, cỡ,
màu, đậm / nghiêng, co giãn, giãn ký tự của khối chữ được chọn như bản đối chiếu và thay đổi áp cho cả khối; các
điều khiển bản đối chiếu không cho dùng ở chế độ này vẽ xám (`#B1B1B1`, giá trị `#6D6D6D`). Mục Split: Merge
Text (nối các khối theo thứ tự đọc), Split Text (mỗi dòng một khối), Remove Kerning (bỏ số kern trong `TJ`),
Convert to Shapes (chữ thành path) — nút xám khi không áp dụng được. Chọn ảnh / path hiện Arrange rồi Effect
(thay ảnh, xoay, lật, độ mờ qua `/ExtGState /ca`).

**Mẫu trang**: Replace (mẫu được chọn chỉ sang trang hiện tại), Rename — đủ Add / Replace / Rename / Spawn / Delete /
Go to như bản đối chiếu.

**Select**: menu thả xuống có đúng hai mục như bản đối chiếu (Select Text and Image, Select Annotation); lệnh
riêng `ed.selectMenu` của Editor (lệnh `tool.select` của Reader không có menu).

**Typewriter gõ tại chỗ** (`editor_typewriter.rs`): bấm công cụ rồi bấm lên trang là có con trỏ, gõ thẳng (Enter
xuống dòng, Backspace), bấm chỗ khác hoặc Esc thì thành ghi chú — như bản đối chiếu (trước đây hỏi qua hộp
thoại). Mặc định lấy từ tệp bản đối chiếu lưu ra (`/DA (/Helvetica 9 Tf)`, chữ `#0000FF`, không viền), khung
khi gõ 1 px `#759DB8` ôm sát chữ (100,9 %: khung 103 × 19 px, chữ lùi 3 px — bản đối chiếu 99,6 %: 103 × 19,
3 px), con trỏ nhấp nháy ở cuối chữ. Typewriter ở tab Home và tab Comment là cùng một công cụ.

**Hỏi lưu khi đóng** (`editor_close.rs`): đóng tài liệu đã sửa (nút ✕ của tab, Ctrl+W, File › Close, nút đóng
cửa sổ — host thêm `Handler::close_requested`) hiện hộp “Do you want to save changes to '…' before closing?”
như bản đối chiếu: Yes lưu rồi đóng, No đóng không lưu, Cancel. Hộp vẽ theo số đo hộp thông báo của bản đối
chiếu ở 125 %: 793 × 193 px, thanh tiêu đề 32 px `#F3F3F3` (tên ứng dụng xám `#919191`, ✕), dấu thông tin
32 px (hình tròn của ta, màu nhấn của ta) ở x 35, chữ `#222222` từ x 88, nút viên thuốc 100 × 35 px cách 13 px, Yes
tô màu nhấn chữ trắng, No / Cancel nền trắng viền `#D5D5D5`. Nhiều tài liệu sửa: hỏi lần lượt từng cái.

**Khung cửa sổ**: vùng client phủ tới mép cửa sổ như bản đối chiếu (PrintWindow: màu thanh tiêu đề từ x = 0),
dùng `win32::enable_full_client` của BlueReader.

**Tốc độ gõ** (bản release, cùng tệp mẫu, 67 ký tự, máy vẫn đang build song song; lấy lần tốt nhất của 3 lần):

| Đo | Vòng 5 | Vòng 6 trước sửa | Vòng 6 |
| --- | --- | --- | --- |
| Gõ trong đoạn, tổng mỗi phím p50 / p95 | 8,0 / 18,0 ms | 9,6 / 12,0 ms | 5,3 / 8,0 ms |
| — xử lý phím (handle) p50 | 1,7 ms | 2,2 ms | 1,6 ms |
| — chuẩn bị trang (prepare) p50 / p95 | — | 5,5 / 7,1 ms | 2,1 / 3,5 ms |
| Cuộn 1 202 trang, mỗi nấc p50 / p95 | 1,6 / 2,6 ms | — | 1,8 / 2,6 ms (vẽ khung 1,5 ms, prepare 0,01 ms) |

Nguyên nhân: mỗi phím diễn giải lại cả trang (3,1 ms trên trang mẫu, đo bằng test `typing_cost`). Nay
`bo-pdfread::typing_fast` giữ display list của phần trang ngoài đoạn đang gõ (prefix của phiên sửa khối, kiểm
tra trạng thái đồ hoạ trở về như đầu) và mỗi phím chỉ diễn giải đoạn đó; test so khớp với diễn giải cả trang.
Cuộn: không còn chênh so với vòng 5 — phần lớn là vẽ khung cửa sổ.

Trang File (`editor-55`): bố cục cột trái khớp; bản đối chiếu có thêm Action Wizard, Batch Print, Index, Share,
Skins (ta chưa có).

Còn lại: đoạn gõ dài ra đè lên đoạn dưới khi Reflow Editing tắt; các mục File còn thiếu ở trên; Page Transitions, OCR, From Web Page, XFA,
Accounting Calculator, Preflight vẫn xám; thanh tiêu đề hộp thoại chung của BlueEditor (Dialog) vẫn kiểu cũ.

## Vòng 7 (2026-11-16)

Ảnh so sánh (bản đối chiếu trái / trên, BlueEditor phải / dưới): `scratchpad/show/editor-56…*.png`.

**Đoạn gõ dài ra khi Reflow Editing tắt** (`bo-pdfread::paragraph_push`). Đo trên bản sao: bản đối chiếu
gom các đoạn liền nhau cùng cột (cách nhau dưới khoảng hai dòng) thành một khung Edit Text; gõ làm đoạn thêm dòng
thì các đoạn sau trong khung bị đẩy xuống ngay khi gõ (bớt dòng thì kéo lên), còn đường kẻ, ảnh, đoạn ở xa sau một
khoảng lớn đứng yên và bị đè (`editor-56`). BlueEditor làm như vậy: phiên sửa khối ghi lại các đoạn theo sau của
khung (chữ chép lại cuối phần đầu nội dung như lệnh dời đối tượng), dời đúng số dòng × khoảng dòng mỗi khi số dòng
đổi; Reflow Editing bật thì dời mọi thứ bên dưới (cả hình) — trước đây chỉ dời khi kết thúc phiên, lúc gõ vẫn đè.
Một bước hoàn tác cho cả đoạn và những gì dời theo. Lối gõ nhanh (`typing_fast`) vẫn dùng được: phần đầu chỉ đổi khi
số dòng đổi. Khung khi gõ như khung hộp chữ của bản đối chiếu: bao cả đoạn và các đoạn theo sau trong khung (vị trí
hiện tại), nét 1 px `#99C1DA`, tám ô vuông 8 px (nền trắng, viền `#B3D1E4`) ở góc và giữa cạnh; kéo cạnh phải đặt
lại bề rộng xuống dòng (`BlockEdit::set_wrap_width`).

**Comment › Text Box** (`editor_textbox.rs`, `bo-pdfread::textbox_annot`). Lỗi: lệnh đi vào công cụ của BlueReader,
hộp hỏi chữ của nó chưa được đặt chỗ nên hiện ở góc trên trái cửa sổ (“Ex / Cancel”). Nay hộp hỏi của BlueReader
được đặt chỗ ngay khi mở, và Text Box của BlueEditor gõ tại chỗ như bản đối chiếu: bấm lên trang là có khung
trắng viền đỏ, dòng đầu canh giữa theo điểm bấm, chữ xuống dòng ở bề rộng cố định, khung dài ra theo chữ; bấm chỗ
khác / Esc thì thành ghi chú. Mặc định đọc từ tệp bản đối chiếu lưu ra: FreeText `/Subj (Textbox)`, vùng chữ rộng
100 pt, viền 1 pt `#FF0000` ngoài vùng chữ (`/Rect` 102 × 12,044 pt cho một dòng), nền trắng `/C [1 1 1]`,
chữ Helvetica 9 pt `#0000FF` (`/RC`), dòng cách 1,116 em, đường chân chữ đầu 0,905 em dưới mép trên.

**Trang File** (`editor_backstage.rs`, `editor_file7.rs`, `editor_skin7_win.rs`): đủ 16 mục theo thứ tự bản đối chiếu.
Save / Revert xám khi chưa sửa (đã có). Batch Print: danh sách tài liệu đang mở có ô chọn, Add Files… mở thêm
PDF vào tab, Print… mở hộp Print một lần rồi host in từng tài liệu đã chọn cả tệp (`take_batch` cho BlueEditor).
Share và Skins dùng trang của BlueReader vòng 8; skin áp lên bảng màu riêng của BlueEditor (Classic giữ nguyên số đo).
Action Wizard và Index xám, rê chuột hiện lý do; Open › Add a place bỏ qua.

**Hộp thoại** (`dialog_caption7.rs`, tuỳ chọn `Dialog::with_caption`, BlueDoc không đổi). Đo hộp thoại thường của bản
đối chiếu (Batch Encrypt, Page Transitions, 125 %): thanh tiêu đề 32 px `#F2F3F3`, tiêu đề chữ thường đen từ x 7,
dấu ✕; thân trắng; nút Windows 11 cao 33 px, rộng tối thiểu 134 px, bo 4 px, mặt `#FDFDFD`, viền `#D0D0D0`, cạnh
dưới `#BABABA`, nút mặc định viền `#0078D4`. Mọi hộp thoại của BlueEditor (Save As / mật khẩu / Optimize / Header &
Footer / Watermark / Bates / Batch… — đều dựng bằng `Dialog`) nay vẽ kiểu này; trường danh sách (`with_choices`:
bấm hoặc ↑↓ để chọn, có mũi tên thả) cho các hộp cần chọn.

**Lệnh xám đã làm**: View › Page Transitions (hộp theo bố cục cổ điển của bản đối chiếu, 530 × 470 px ở 125 %: nhóm
“Page Transition” — nhãn canh phải bên trái danh sách 302 × 27 px mặt `#E2E2E2`, xám `#CCCCCC` khi tắt, viền `#999999`; Direction / Speed
tắt khi No Transition —, nhóm “Page Range” — nút chọn Selected Pages (chỉ khi có trang chọn ở Organize) / All Pages /
Pages From [ô số] To [ô số] of N —, OK / Cancel 91 × 28 px góc dưới phải; Transition — No Transition và 12 kiểu PDF
Split / Blinds / Box / Wipe / Dissolve / Glitter / Push / Cover / Uncover / Fade / Replace / Fly —, Direction theo
kiểu, Speed Slow / Medium / Fast = 2 / 1 / 0,5 s, trang: tất cả hoặc khoảng; ghi `/Trans` từng trang, ISO 32000-1
§12.4.4, một bước hoàn tác). Form › XFA: biểu mẫu XFA tĩnh (có trường AcroForm) bỏ gói XFA thành biểu mẫu thường,
giữ trường và giá trị; XFA động thì báo không làm được. From Web Page, Preflight bỏ qua (theo yêu cầu).

**Lỗi gõ dừng hiển thị sau ~46 phím** (có từ trước, thấy rõ ở `typed.png` vòng 6): mỗi phím làm mới trang 0 và
`ReaderView::ensure` đẩy trang vào danh sách dùng gần đây mà không bỏ bản cũ; quá 48 mục thì vòng dọn bộ nhớ đệm bỏ
đúng trang vừa vẽ (“not rendered”), cửa sổ giữ hình trang lúc ~46 ký tự dù chữ vẫn được gõ và lưu đủ. Sửa: làm mới
trang thì bỏ khỏi danh sách, mỗi trang một mục. Test gõ 200 ký tự kiểm tra mỗi phím đều vẽ và tệp lưu đủ (bỏ sửa thì
hỏng ở phím 47). `BLUEOFFICE_DRAW_LOG=<tệp>` ghi lỗi dựng / vẽ trang để chẩn đoán.
BlueReader dùng chung bộ nhớ đệm này (gõ Typewriter, kéo ghi chú, điền trường): test
`reader_window_draws_a_page_changed_sixty_times` giữ cho đường đó (ghi cả ở `REPORT-reader.md`).

**Lỗi khởi động**: khung đầu tiên vẽ khi cửa sổ còn rất nhỏ làm `thumb()` của thanh cuộn gọi `clamp(px(20), track)` với
track âm (panic `min > max`, thấy ngẫu nhiên khi chạy bản release); nay track tối thiểu 20 px, test `a_tiny_window_paints`.

**Text Box cùng mức thu phóng** (`editor-57b`): bản đối chiếu 141 % / BlueEditor 142,8 %: khung 392–630 × 286–312 so với
391–632 × 286–313 px, chữ cao 292–306 px ở cả hai — cỡ 9 pt đúng (ảnh `editor-57` lệch vì khung Format thu nhỏ trang về 98,6 %).

Còn lại: OCR, From Web Page, Preflight, Accounting Calculator, Action Wizard, Index vẫn xám; Format pane mở khi chọn công cụ
ghi chú (bản đối chiếu không mở) làm trang thu nhỏ theo bề rộng.
