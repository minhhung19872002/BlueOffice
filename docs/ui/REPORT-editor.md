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

## Vòng 8 (2026-11-16)

Ảnh so sánh (bản đối chiếu trái / trên, BlueEditor phải / dưới): `scratchpad/show/editor-60…*.png`.

**Hộp thoại kiểu cổ điển** (`editor_form8.rs` — bộ điều khiển chung: nhóm có tiêu đề, nhãn bên trái, nút chọn, ô
đánh dấu, ô số có mũi tên, danh sách thả, liên kết, nút biểu tượng của ta; `editor_dlg8.rs`, `editor_dlg8b.rs`). Đo
trên hộp thoại của bản đối chiếu ở 125 % (PrintWindow; menu mở bằng phím gửi tới cửa sổ menu), vị trí từng điều khiển
theo số đo, cỡ hộp đúng bằng bản đối chiếu: Add Headers and Footers 1004 × 820, Add Watermark 1093 × 802, Add
Background 957 × 703, Add Bates Numbering 933 × 532, Password Protection 708 × 612, Insert Blank Page 479 × 370, Extract
Pages 444 × 491, Split Document 494 × 424 (`editor-60…67`). Các nút Header & Footer / Watermark / Background / Bates mở
menu Add / Remove All như bản đối chiếu (`PdfDocument::remove_page_marks`). Danh sách thả là danh sách combo của
Windows như bản đối chiếu (khung `#7A7A7A`, mục 21 px, mục chọn `#0078D4` chữ trắng, tối đa 11 mục có thanh cuộn) — cả
trong Page Transitions.

- Header & Footer: phông, cỡ, màu, kiểu chữ (gạch chân), lề trên / dưới / trái / phải (inch), sáu ô chữ, Macros +
  Insert, liên kết Appearance Options (dời phải / lên), Page number and date format (5 dạng số trang, 4 dạng ngày, số
  bắt đầu), Page Range Options (tất cả / từ–đến, chẵn / lẻ), xem trước có đường lề nét đứt; Save Settings / Delete lưu
  bộ thiết lập có tên vào Saved Settings, ghi ra `%LOCALAPPDATA%/BlueOffice/editor-presets.txt` (cả policy của
  Password Protection) và đọc lại ở phiên sau.
- Watermark: chữ hoặc ảnh (File: Browse chọn ảnh, Absolute Scale hoặc theo bề rộng trang), phông / cỡ (Auto) / màu /
  kiểu chữ, xoay −45 / 0 / 45 / tuỳ chỉnh, độ mờ (thanh trượt + ô số), co theo trang, lặp nhiều dòng với khoảng hàng /
  cột, sau / trên trang, vị trí (khoảng dọc / ngang, đơn vị, tính từ), Page Range Options, ba nút biểu tượng lưu / nạp /
  xoá bộ thiết lập (hình của ta); xem trước là ảnh trang thật (`ReaderView::want_preview`) với chữ hình mờ.
- Background: màu, độ mờ, co theo trang, vị trí, Page Range Options, xem trước trang thật phủ màu.
- Bates: danh sách tệp (tài liệu đang mở), OK sang hộp Bates Numbering Options (số chữ số, số bắt đầu, tiền tố, hậu tố).
- Password Protection: ô nhập luôn trắng, nhãn đen, gõ vào thì tự đánh dấu ô yêu cầu, cách 4 px trước mỗi ô;
  Permission… mở hộp quyền; Changes mặc định “Any except extracting pages”; AES 128 / AES 256 / ARC-FOUR 128
  (`Cipher::Rc4`, bộ xử lý bảo mật bản 3, `set_protection_with`); Don't encrypt metadata (`/EncryptMetadata false`);
  Save the settings as a new policy giữ lựa chọn (không giữ mật khẩu) cho lần sau.
- Insert Blank Page (trước / sau trang, đầu / cuối, số trang) và Insert Pages sau khi chọn tệp; Extract Pages (tất cả /
  khoảng, chẵn / lẻ, một PDF / nhiều PDF theo dấu phẩy / mỗi trang một PDF, lưu ngay hoặc mở thành tab, xoá sau khi
  trích); Split Document (theo số trang, cỡ tệp, khoảng trang, dấu trang cấp đầu, trang trắng).

Còn xám (có lý do): Select More Files / Output Options (tách / Bates nhiều tệp), Shrink document, phông khác Helvetica
(dấu trang vẽ bằng phông đóng gói), nguồn PDF cho hình mờ / ảnh nền, chữ động. Optimize và Save As: mục trang File của
bản đối chiếu không nhận cú bấm gửi bằng thông điệp nên chưa chụp được hộp của nó — giữ hộp hiện có.

**Edit Text** (`editor_join8.rs`): Backspace ở đầu đoạn nối đoạn vào đoạn ngay trên trong khung, chữ chạy tiếp sau ký
tự cuối, không thêm dấu cách; Delete ở cuối đoạn kéo đoạn dưới lên; ở đầu đoạn đầu tiên Backspace không làm gì — như
bản đối chiếu (`editor-68`). Khung Format khi bật Edit Text / Edit Object nay là dải thu gọn 18 px `#E2E2E2` có mũi tên
như bản đối chiếu, trang giữ mức thu phóng (trước: khung mở ngay, trang thu từ 142,8 % xuống 98,6 %); bấm dải thì mở.
Nhấp đúp chọn từ, nhấp ba chọn đoạn, Ctrl+mũi tên, Home / End, Shift chọn, Enter tách, dán chữ thường: đã có.

**Edit Object**: menu chuột phải của ảnh thêm Replace Image / Extract Image (JPEG giữ nguyên, khác thì PNG,
`PdfDocument::object_image`). Tay cầm xoay như bản đối chiếu (`editor_rot8.rs`): nút tròn phía trên giữa cạnh trên của vùng chọn, đường nét đứt
xuống tâm; kéo thì xoay vùng chọn quanh tâm (Shift: bước 15°), hiện góc khi kéo, một bước hoàn tác. Clip Path chưa làm.

**Sót từ vòng 7**: chọn công cụ ghi chú không mở khung Format, không đổi thu phóng (`cmt8.props` mở thuộc tính khi
cần); cột trái trang File 272 px tính từ mép cửa sổ như bản đối chiếu; menu của ribbon không còn bị lưới trang của
Organize che (trước bị cắt ở dải tab).

**Tốc độ** (bản release, máy đang build song song):

| Đo | Vòng 6–7 | Vòng 8 |
| --- | --- | --- |
| Mở sổ tay 1 202 trang / trang đầu (`--bench`, 3 lần) | 37–58 / 48–70 ms | 34–57 / 46–71 ms |
| Gõ trong đoạn, tổng mỗi phím p50 / p95 (67 ký tự, 2 lần) | 5,3 / 8,0 ms | 2,4 / 4,8–8,1 ms (xử lý phím 0,15 ms) |
| Lưu sau một ký tự gõ (nối thêm, +743 byte) | — | 2,6 ms |
| Xoá một trang rồi Lưu (ghi lại toàn bộ): 6 trang / 1 202 trang 10 MB | 15,7 (100 tr.) / 63,4 ms | 2,6 / 76 ms |
| Bộ nhớ sau khi mở sổ tay (working set / private) | 90 / 129 MB | 82 / 91 MB |

XFA tĩnh → biểu mẫu thường đã làm ở vòng 7; OCR vẫn xám (cần mô hình nhận dạng).

Còn lại: Clip Path của Edit Object; vùng chọn chữ của bản đối chiếu là nền `#1B3471` chữ trắng (ta vẫn
phủ xanh trong); Optimize / Save As chưa đối chiếu.

## Vòng 9 (2026-11-17)

Ảnh so sánh (bản đối chiếu trái / trên, BlueEditor phải / dưới): `scratchpad/show/editor-70…*.png`.

**Edit Object / Edit Text** (`editor_rot8.rs`, `editor_selhi9.rs`, `editor_edit.rs`). Vùng chọn đối tượng như bản đối
chiếu (đo ở 125 %): khung 1 px `#2E6DAD`, ô vuông đặc 6 px ở bốn góc và giữa cạnh (ô giữa cạnh kéo một chiều), dấu
xoay là vòng hở có mũi tên (hình của ta) trên cuống nét đứt 15/5 px xuống hình thoi ở tâm. Chữ được chọn trong Edit
Text là chữ trắng trên dải `#003371` vẽ vào ảnh trang (`ReaderView::set_text_mark9`); trong phiên gõ mỗi ký tự gõ / xoá
là một bước hoàn tác như Undo của bản đối chiếu. Nhấp đúp / nhấp ba, Ctrl+←/→, Home / End, Shift chọn, Delete cuối đoạn
nối đoạn, Enter tách, dán chữ thường, phông / cỡ / màu / đậm từ khung Format trên vùng chọn: đã có từ vòng 6–8, kiểm lại.

**Organize** (`editor_org9.rs`, `editor_pages9.rs`, `editor_conv9.rs`, `editor_crop9.rs`, `editor_pagenum9.rs`).
- Insert ▾ đủ hàng của bản đối chiếu (From File, From Folder, From Open Files, From Web Page, From Scanner, From
  Clipboard, Blank Page, Into File; Folder / Web Page / Scanner / Into File xám); Watermark / Background / Header &
  Footer ▾ có Add / Update / Remove All (Update mở lại hộp với giá trị đã áp và thay dấu cũ).
- Delete mở “Delete Pages” 453 × 364 px (ô Page điền sẵn trang chọn kèm dấu phẩy, of N, Sample, Delete: tất cả /
  chẵn / lẻ, Show Preview); phím Delete trong lưới trang vẫn xoá ngay. Swap mở “Swap Two Pages” 419 × 178 px (hai ô số
  Swap / For). Replace: chọn tệp rồi “Replace Pages” 763 × 311 px (From / To of N, in <tệp>, Select file, Browse…,
  Page of M; ô Page trống = mọi trang của tệp — đo trên bản đối chiếu: thay 1 trang bằng tệp 6 trang ra 11 trang);
  bản đối chiếu hỏi “cannot be undone”, ta hoàn tác được nên không hỏi.
- Rotate Pages ▾: Left / Right / Multi-page rotate → hộp “Rotate Pages” 481 × 469 px (hướng, khoảng trang, chẵn / lẻ,
  trang ngang / dọc).
- Crop Pages là công cụ như bản đối chiếu: trang hiện ra (cả khi đang ở Organize), kéo khung → “Set Page Dimensions”
  843 × 673 px điền lề theo khung (bản đối chiếu điền 0.99 / 8.58 / 0.80 / 2.55 in là do khung đã vẽ), nhấp → hộp
  với lề 0; Change Page Size dùng được (Fixed Size None / A4 / A3 / A5 / Letter / Legal / Tabloid, Custom rộng × cao,
  nội dung canh giữa); Format Page Numbers mở “Page Numbers” 691 × 538 px (`/PageLabels`).
- Ô nhập của các hộp cổ điển có đường đáy `#838383` như bản đối chiếu.

**Convert**: menu đúng hàng của bản đối chiếu — From Files ▾ (From File, From Multiple Files, Convert to PDF
Settings xám), To MS Office ▾ (Word, Word 97-2003 xám, Excel, XML Spreadsheet 2003 xám, PowerPoint, XPS xám), To
Image ▾ (PNG, JPEG, JPEG2000 xám, TIFF, BMP — TIFF 6.0 baseline RGB và BMP 24-bit tự ghi), To Other ▾ (Plain Text,
Rich Text — RTF 1.9 của chữ trang —, Accessible Text, XML 1.0 xám). Combine Files như bản đối chiếu: tab tài liệu
“Combine Files” + tab ribbon ngữ cảnh cùng tên (Add Files, Add Folder, Add WebPage, Add PDF from Scanner, Add From
Clipboard, Add Open Files, Remove | Settings ▾, Move Up, Move Down | nút Combine tô màu nhấn của ta và Cancel bo tròn
116 × 48 px), danh sách toàn bề rộng 9 cột (Name / Operation / Modified / Page Range / Size / Bookmark for File / Bates
Numbering / Warnings/Errors / Location; đầu 50 px chữ `#999999`, hàng 59 px chữ `#222222`, đường `#DEDEDE`); nhấp đúp
cột Page Range hoặc Enter để đặt khoảng trang. Folder / WebPage / Scanner / Settings xám (host chỉ chọn tệp).

**Optimize / Save As**: mục trang File của bản đối chiếu không nhận cú bấm gửi bằng thông điệp nhưng nhận phím ↓
gửi tới danh sách — mở được PDF Optimizer: Reduce File Size (hộp 544 × 262, chỉ “Make compatible with”) và Advanced
Optimization (“PDF Optimizer” 1158 × 795). File › PDF Optimizer của BlueEditor nay là hộp theo bố cục Advanced
Optimization: Predefined Settings, Current PDF version, Make compatible with, cột bảng có ô đánh dấu (Compress
Images, Unembed Fonts xám, Transparency xám, Discard Objects, Discard User Data, Clean Up), khung Compress Images
(giảm mẫu tới 150 ppi cho ảnh trên 225 ppi, JPEG, chất lượng Minimum…Maximum; Monochrome / JBIG2 xám; chỉ nén khi
nhỏ đi). Save As của bản đối chiếu là trang File (thư mục gần đây) rồi hộp chọn tệp của Windows — giữ hộp hiện có.

**Sau góp ý của chủ dự án** (cùng vòng 9). Combine Files ban đầu chưa chọn hàng (Remove / Move xám) như bản đối
chiếu. Add Folder chọn thư mục (hộp chọn thư mục của Windows, `win32::folder_dialog`) và thêm mọi PDF cùng tệp văn bản
/ bảng tính / trình chiếu trong đó (đổi sang PDF bằng engine BlueOffice). Settings ▾ có ba tuỳ chọn của bản đối chiếu:
trang mục lục mới (mỗi tệp một dòng với trang bắt đầu), giữ số trang logic (`/PageLabels` của từng tệp), đóng các tệp
đang mở đã lưu sau khi kết hợp. Add WebPage / Add PDF from Scanner vẫn xám (không tải web, không có thiết bị quét).
Convert: To XML Spreadsheet 2003 (bảng tính XML 2003 tự ghi, cùng ô như To Excel, BlueSheet đọc lại được), To XPS
(gói OpenXPS ECMA-388, mỗi trang một ảnh 150 dpi), To XML 1.0 (các dòng chữ với hộp, cỡ, màu), Convert to PDF Settings
(“Converting To PDF” 857 × 678 px: danh sách loại tệp — Văn bản / Bảng tính / Trình chiếu —, thiết lập, Add
bookmarks, Tagged PDF; áp dụng khi From File đổi tệp văn phòng sang PDF qua `export_pdf_with`). Còn xám vì không làm
được: To Word 97-2003 (BlueOffice không ghi .doc), JPEG2000 (không có bộ mã hoá), nút Edit… của thiết lập.
Ô nhập đầu tiên của các hộp được chọn sẵn cả chữ khi mở (và khi Tab tới) như bản đối chiếu, gõ là thay. Menu thả của
vòng 9: hàng 29 px, chữ nhỏ hơn từ x 35, viền 1 px `#E0E0E0`, bóng mờ đều bốn phía (`Menu::with_soft_shadow`) thay cho
dải bóng xám phải / dưới.

## Vòng 10 (2026-11-17)

Ảnh so sánh (bản đối chiếu trái / trên, BlueEditor phải / dưới): `scratchpad/show/editor-82…*.png`.

**PDF Optimizer đủ các trang** (`editor_opt10.rs`, `bo-pdfread::optimize10`, `ccitt_enc`, `font_subset`). Đo hộp Advanced
Optimization của bản đối chiếu ở 125 % (1158 × 795): cột trái là danh sách có ô đánh dấu, trang đang xem nền `#0078D7`
chữ trắng từ x 40 tới 240, hàng 24 px; khung các trang 1 px `#DCDCDC`.
- Compress Images: ảnh màu / xám giảm mẫu (Average / Bicubic — Catmull-Rom sau một bước trung bình / Subsampling) tới N
  ppi khi trên M ppi, nén Retain existing / JPEG / ZIP, chất lượng Minimum…Maximum (dòng Quality chỉ hiện khi chọn JPEG như
  bản đối chiếu); ảnh đơn sắc (1 bit, image mask) giảm mẫu rồi ghi JBIG2 (vùng generic MMR, không mất dữ liệu; cả hai mức
  Lossless / Lossy đều ghi MMR vì chưa có mã hoá ký hiệu), CCITT Group 3 / Group 4 (bộ mã T.4 / T.6 tự viết, kiểm bằng bộ
  giải mã), ZIP, Run Length. JPEG2000 và High Compression không có trong danh sách (không có bộ mã hoá).
- Unembed Fonts: biểu tượng cảnh báo (hình của ta), Do not unembed any font, hai danh sách Embedded fonts / Fonts to
  unembed với Unembed>> / <<Retain, Subset all embedded fonts — chương trình TrueType được rút còn các glyph trang dùng (giữ
  nguyên mã glyph nên `/W`, `/CIDToGIDMap`, `cmap` không đổi; bỏ GSUB / GPOS / kern…). Phông của giá trị mặc định biểu mẫu
  (`/DR`) giữ nguyên để còn gõ được.
- Transparency: ba mức Low / Medium / High Resolution với đúng các dòng thông số bản đối chiếu hiện (75 / 288 / 144,
  75 / 300 / 150, 100 / 1200 / 300). Làm phẳng: trang có trong suốt được host vẽ ở độ phân giải Gradient and Meshes thành
  ảnh, nội dung cũ giữ dưới một vùng cắt rỗng (vẫn tìm / chép được chữ), trang xoay để nguyên.
- Discard Objects / Discard User Data / Clean Up: các ô như bản đối chiếu; làm được: thao tác gửi / nhập / đặt lại biểu
  mẫu, làm phẳng trường, JavaScript, ảnh thay thế, ảnh thu nhỏ, thẻ, thiết lập in, dấu trang, ghi chú / biểu mẫu / đa
  phương tiện, tham chiếu tệp khác, siêu dữ liệu, đính kèm, dữ liệu riêng (`PieceInfo`), lớp ẩn; nén cả tệp / cấu trúc /
  bỏ nén / giữ nguyên, Flate cho luồng chưa nén, LZW → Flate, bỏ liên kết tới trang không còn, viết gọn luồng nội dung.
  Xám (không làm): đường mịn → cong, ghép mảnh ảnh, chỉ mục tìm kiếm, dữ liệu đối tượng, dấu trang hỏng, đích không dùng,
  Fast Web View.
- Predefined Settings: Standard, Mobile (đo trên bản đối chiếu: 96 ppi trên 144, JPEG High, đơn sắc 600 / 900 CCITT G4, PDF
  1.6) và các bộ đã lưu; biểu tượng xoá / lưu (hình của ta), hộp “Create PDF Optimizer settings” 376 × 175; lưu vào
  `editor-presets.txt` như các hộp vòng 8. Make compatible with: Retain existing / PDF 1.3…1.7 (dưới 1.5 không dùng
  object stream).
- Audit Space Usage (349 × 407 khi 6 dòng + Total): Description / Bytes / Percentage theo loại (Images, Bookmarks,
  Thumbnails, Comments, Forms, Fonts, Content Streams, Structure Info, Named Destinations, Document Overhead, Color Spaces,
  Cross Reference Table), chỉ các loại có byte, số có dấu phẩy nghìn; byte lấy theo vị trí đối tượng trong tệp (đối tượng
  trong object stream chia theo độ dài), tổng đúng bằng cỡ tệp.
- Trang File › PDF Optimizer như bản đối chiếu: ba hàng Reduce File Size / Advanced Optimization / Optimize Scanned PDF,
  bên phải tiêu đề `#666666`, mô tả `#999999`, ô 142 × 128 px viền `#E0E0E0` (Current File; Multiple Files xám); Reduce
  File Size mở hộp 544 × 262 (PDF Version Compatibility) và tối ưu theo Standard.

Cỡ tệp trên mẫu 3 trang (ảnh JPEG 2400 × 1800, bản quét xám 2400 × 3000, bản quét 1 bit 3600 × 4800, hai phông TrueType
nhúng đủ, một hình trong suốt), thiết lập Standard:

| Tệp | Gốc | Bản đối chiếu | BlueEditor |
| --- | --- | --- | --- |
| opt.pdf (byte) | 2 089 231 | 351 501 | 243 338 |

Khác biệt điểm ảnh so với bản gốc (vẽ lại 100 dpi xám; trung bình |Δ| / % điểm lệch > 32):

| Trang | Bản đối chiếu | BlueEditor |
| --- | --- | --- |
| 1 ảnh màu + chữ | 0,60 / 0,00 % | 0,40 / 0,00 % |
| 2 bản quét xám | 0,23 / 0,02 % | 0,57 / 0,07 % (nền 254: sửa, xem dưới) |
| 3 bản quét 1 bit | 1,20 / 1,50 % | 0 / 0 % (MMR không nhỏ hơn Flate gốc nên giữ nguyên) |

Bản đối chiếu rút phông (1,27 MB → 98 KB), giảm ảnh màu 939 × 704, ảnh xám thành JPEG 750 × 938, ảnh 1 bit JBIG2 có
mất dữ liệu 2250 × 3000 (1 100 byte). BlueEditor: phông 72 KB, ảnh màu 940 × 705, ảnh xám JPEG 750 × 938. Sau góp ý của
chủ dự án (nền xám nhạt sau bản quét): bộ mã JPEG làm trắng thành 254 — trọng số độ sáng cộng ra 254,99997 và hệ số DC của
khối trắng nằm đúng nửa bậc lượng tử ở chất lượng 50; nay làm tròn tới 1/1024 trước, trắng giữ 255 ở mọi chất lượng (test).
Ảnh 1 bit khi giảm mẫu giữ điểm đen từ một phần ba độ phủ (ngưỡng 50 % làm nét chữ mảnh đi).

**Sửa lỗi có từ vòng 4**: host trả trang đã vẽ dưới dạng PNG mà `codecs::decode` chỉ đọc GIF / BMP / TIFF, nên Convert ›
To Image › JPEG / TIFF / BMP không lưu được tệp nào; nay giải mã PNG (`png_rgba`), có test.

**Protect** (`editor_prot10.rs`, `bo-pdfread::security10`): nhóm và menu như bản đối chiếu — Mark for Redaction ▾ (Text &
Images, Pages, Search & Redact, rồi Apply Redactions, xám khi chưa đánh dấu), Whiteout, Protect Redactions ▾ (hai mục xám
như bản đối chiếu), Hidden Data ▾ (Sanitize Document, Remove Hidden Information), Secure Document ▾ (Password Protect,
Certificate Protect và Security Policies xám, Remove Security xám khi tệp không mã hoá, Security Properties), Restrict
Access, Batch Encrypt, Fill & Sign, Sign & Certify ▾ (Place Signature…; các mục chứng nhận / Digital IDs xám), Time Stamp,
Validate. Search & Redact đánh dấu mọi chỗ khớp (cả từ, phân biệt hoa thường), Pages đánh dấu cả trang; Apply xoá thật
glyph, ảnh, hình, ghi chú dưới vùng (có từ trước, kiểm lại). Security Properties đọc từ điển mã hoá (AES 256 / AES 128 /
RC4) và quyền `/P`.

**Hộp thoại**: nút không dùng được viền `#E9E9E9` mặt `#F9F9F9`, nút chọn không dùng được vòng `#C3C3C3` trên `#F9F9F9`,
ô nhập không dùng được viền `#ECECEC`, vành `#FBFBFB`, trong trắng, danh sách và khung trang `#DCDCDC` (đo trên PDF
Optimizer khi bỏ chọn Compress Images). Audit Space Usage của ta trên cùng tệp ra gần đúng số của bản đối chiếu (Images
814 435 / 814 429, Fonts 1 267 046 / 1 267 022, Color Spaces 4 878 / 4 870, Cross Reference Table 731 / 730).

**Edit** (`editor_edit10.rs`): ribbon đã đúng thứ tự (ảnh `editor-84`); menu Edit Object ▾ (All / Text / Image / Shape /
Shading — công cụ chỉ chọn loại đó), Add Images ▾ (Image From File; Draw New Image xám), Web Links ▾ đủ bốn mục (Append All
Web Links on Current Page, View Web Links là bảng trang / địa chỉ), Add Shapes theo kiểu menu vòng 9 (chưa có bộ hình dạng
lưới Add Path / Add Shading của bản đối chiếu). Khung khi gõ (Reflow Editing tắt): bản đối chiếu bao cả nhóm đoạn sát nhau,
kể cả các đoạn phía trên đoạn đang gõ; ta trước chỉ bao từ đoạn đang gõ trở xuống — nay bao từ đoạn đầu nhóm
(`paragraph_push::leaders_box`, `editor-96`). Gõ thêm dòng ở đoạn cuối nhóm: đường kẻ phía dưới bị đè như bản đối chiếu.

**Tốc độ** (bản release, máy đang build song song — rustc của các agent khác):

| Đo | Vòng 8 | Vòng 10 |
| --- | --- | --- |
| Mở sổ tay 1 202 trang / trang đầu (`--bench`, 3 lần) | 34–57 / 46–71 ms | 40–67 / 52–80 ms |
| Gõ trong Edit Text, xử lý phím / vẽ / gửi khung p50 (p95) | 0,15 / — / — ms | 0,16 (0,24) / 0,42 (0,53) / 0,98 (1,27) ms |
| Gõ, tổng công việc mỗi phím p50 / p95 (96 ký tự) | 2,4 / 4,8–8,1 ms | 2,2 / 2,7 ms |

Bản đối chiếu không đo được độ trễ phím bằng thông điệp gửi (không có nhật ký thời gian); cuộn ảnh thu nhỏ trang chưa đo.

Còn lại: hộp Properties của trường biểu mẫu (General / Appearance / Options / Actions / Format / Validate / Calculate) —
công cụ trường và Select Annotation của bản đối chiếu không nhận cú bấm gửi bằng thông điệp nên chưa chụp được để đo, ta vẫn
dùng hộp đơn giản; Tab order; Add Shapes dạng lưới (Add Path / Add Shading) và các hình mới (đa giác, mũi tên); khung mỏng
quanh các khối chữ khác khi bật Edit Text (bản đối chiếu vẽ); JBIG2 có mất dữ liệu (mã hoá ký hiệu), JPEG2000; Optimize
Scanned PDF mở hộp Advanced; Multiple Files; Certificate Protect / Security Policies / chứng nhận chữ ký; nút Edit… của
Converting To PDF; khung xem trước của Crop.

## Vòng 11 (2026-11-17)

Ảnh so sánh (bản đối chiếu trái / trên, BlueEditor phải / dưới): `scratchpad/show/editor-97…*.png`.

**Hộp Properties của trường biểu mẫu** (`editor_fields11.rs`, `bo-pdfread::field_props`). Lần này tới được hộp của
bản đối chiếu: bấm một công cụ trường (gửi thông điệp) đưa trang vào chế độ trường, nhấp đúp gửi bằng thông điệp chọn
trường, nhấp phải mở hộp “Text Field Properties”; các tab bấm được bằng thông điệp, danh sách thả đọc bằng
`CB_GETLBTEXT`, loại Format đổi bằng phím ↓ gửi tới hộp thả. Hộp đổi cỡ theo tab, đo ở 125 % (cả viền): General
660 × 452, Appearance 678 × 615, Position 834 × 495, Options 628 × 667, Format 625 × 755, Validate 675 × 540, Calculate
660 × 752, Actions 678 × 690; khung trang 1 px `#E0E0E0` từ (16, 72) tới (W − 31, H − 71), tab chọn là hộp `#E0E0E0` từ
y 45 tới khung, nhãn giữa y 58, ô Locked ở y H − 44, Close 91 × 27 ở (W − 110, H − 58) là nút mặc định viền `#0078D4`;
hộp thả trong hộp này mặt `#FDFDFD` viền `#D2D2D2`, danh sách hành động viền `#828790`. Hộp của BlueEditor ra đúng các
cỡ ấy (kiểm bằng ảnh chụp: 660 × 452 … 678 × 690) với đủ tab và điều khiển:
- General: tên, chú thích, Form Field (Visible / Hidden / Visible but doesn't print / Hidden but printable → cờ
  `/F`), hướng xoay (`/MK /R`), Read Only, Required (`/Ff`).
- Appearance: kiểu viền Solid / Dashed / Beveled / Inset / Underline (`/BS /S`), độ dày (`/BS /W`), màu viền / nền
  (`/MK /BC /BG`, ô “không màu” là hình của ta: bàn cờ xám có dấu “?”), cỡ chữ Auto…36, màu chữ, phông (14 phông
  chuẩn, `/DA`), hướng chữ.
- Position: đơn vị Points / Inches / Centimeters / Picas / Millimeters, Left / Bottom / Right / Top / Width / Height
  (đo từ gốc trang như bản đối chiếu: 1 in / 9.72 in…), giữ rộng cao khi dời.
- Options (trường chữ): canh lề `/Q`, giá trị mặc định `/DV`, Scroll long text, Rich Text, giới hạn ký tự `/MaxLen`,
  Password, File select, Check spelling, Comb of N, Multi-line; hộp thả / danh sách: mục, giá trị xuất, Up / Down /
  Delete, sắp xếp, cho nhập tự do, chọn nhiều, ghi ngay; ô đánh dấu / nút chọn: giá trị xuất, chọn sẵn; nút bấm: nhãn.
- Format: None / Number (số lẻ, kiểu phân cách, tiền tệ, ngoặc, chữ đỏ, ví dụ “4,123.03”) / Percentage
  (“412,303.45%”) / Date (18 mẫu) / Time (4 mẫu) / Special (mã bưu chính, điện thoại, an sinh, mặt nạ tự đặt) /
  Custom (script) — ghi `AFNumber_Format`, `AFPercent_Format`, `AFDate_FormatEx`, `AFTime_Format`,
  `AFSpecial_Format` vào `/AA /F /K` như các trình đọc khác hiểu; mô tả dưới khung là lời của ta.
- Validate: không / trong khoảng (chỉ dùng được khi Format là số, như bản đối chiếu) / script (`AFRange_Validate`).
- Calculate: không / tổng, tích, trung bình, nhỏ nhất, lớn nhất của các trường (Pick… mở hộp chọn trường) / ký hiệu
  rút gọn / script (`AFSimple_Calculate` đọc lại được bằng phép tính tự động có từ trước).
- Actions: Select Trigger (Mouse Up / Down / Enter / Exit, On Receiving / Losing Focus), Select Action (thực hiện mục
  menu, đi tới trang, nhập dữ liệu, mở liên kết web, mở tệp, đặt lại biểu mẫu, JavaScript, hiện / ẩn trường, gửi
  biểu mẫu), Add… mở hộp nhỏ theo loại, danh sách Up / Down / Edit / Delete.
Bản đối chiếu áp từng thay đổi ngay; ta áp khi Close (một bước hoàn tác), không đổi gì thì tài liệu giữ nguyên.

**Chế độ trường** (cùng module): khi một công cụ trường bật, mọi trường hiện khung và tên; nhấp chọn (Shift thêm),
kéo để dời, Delete xoá, nhấp đúp mở Properties; trường được chọn có khung đen, tám ô vuông đỏ 6 px và nhãn tên đỏ chữ
trắng như bản đối chiếu. Menu chuột phải: Properties…, Delete, Align ▸ (trái, phải, trên, dưới, tâm dọc, giữa
ngang), Center ▸ (ngang, dọc, cả hai — giữa trang), Distribute ▸ (dọc, ngang), Size ▸ (cao, rộng, cả hai theo trường
đầu), Set Tab Order ▸ (theo hàng / cột / cấu trúc → `/Tabs R C S`; tự đặt: bấm lần lượt các trường, số thứ tự hiện
trên trường, Enter lưu thứ tự `/Annots`), Highlight Fields. Tool Settings ▾ như bản đối chiếu: Highlight Fields (có
dấu khi bật) và Keep Tool Selected (xám). Bản đối chiếu có thêm ngăn Format (Arrange) khi chọn trường — ta dùng menu.

**Protect** (theo góp ý `editor-94`): Restrict Access ▾ (Unrestricted Access có dấu, xám; kết nối máy chủ quản lý
quyền, xám), Settings ▾ (Custom Templates xám, Security Watermark → hộp Add Watermark), Sensitivity ▾ (một dòng kết
nối dịch vụ bảo vệ thông tin, xám; tên dịch vụ của hãng khác không dùng), biểu tượng của ta. Nhãn hai dòng của nút
lớn cách nhau 23 px như bản đối chiếu (trước 18 px; đo: dòng 1 từ y 133, dòng 2 kết thúc y 167).

**Edit Text** (`editor_frames11.rs`): khi bật Edit Text, mọi khối chữ trên các trang đang hiện có khung 1 px `#C0C0C0`
cách chữ 2 px hai bên, 1 px trên dưới; khối gồm các đoạn cùng cột cách nhau không quá hai dòng (cùng quy tắc với
khung khi gõ) — ảnh `editor-101` khớp từng khung với bản đối chiếu. Khối đang gõ giữ khung xanh `#99C1DA` có ô vuông.
Bản đối chiếu không đổi khung khi rê chuột gửi bằng thông điệp nên chưa đo được màu khi rê.

**Add Shapes** (`editor_shapes11.rs`, `bo-pdfread::shapes11`): lưới như bản đối chiếu (326 × 129 px: “Add Path” với
chín hình 30 px bước 36 px — đường thẳng, chữ nhật, chữ nhật bo góc, elip, đường gấp khúc, đa giác, đường cong, mũi
tên, góc chữ L — rồi “Add Shading” với tám hình kín tô chuyển), hình vẽ là của ta. Khi công cụ hình bật, ngăn Format có
mục Shape: màu đường, màu tô (cả “không”), độ dày 0,5–6 pt, độ mờ 25–100 %, nét liền / đứt / chấm; hình ghi vào nội
dung trang với `/CA /ca`, mẫu nét `d`, tô chuyển là shading trục (`sh` trong vùng cắt). Ngăn Format của bản đối chiếu
chỉ hiện khi đã vẽ hình, mà kéo gửi bằng thông điệp không vẽ được — bố cục ngăn là của ta.

**Comment / View / Share** (`editor-103`): thêm tab Share như bản đối chiếu (Email ▾, Send for Shared Review ▾,
Manage Shared Review ▾, Send for Email Review — gửi tài liệu qua email —, Manage Email Review ▾, Tracker; các dịch vụ
duyệt trực tuyến xám; nút của dịch vụ ghi chú / quản lý tài liệu hãng khác không đưa vào vì là tên thương hiệu).
View: Rotate View và Toggle Ruler tách nhóm; Comment: nhãn Textbox một dòng.

**Tốc độ** (bản release, máy vẫn đang build song song): trang đầu ở lưới Organize của sổ 300 trang hiện sau 104–281 ms
tính từ cú bấm tab (bản đối chiếu không có lưới ở Organize — ảnh thu nhỏ ở ngăn Pages: 170–198 ms; cùng cách đo:
chụp PrintWindow lặp tới khi ô đầu có chữ, sai số một lần chụp ~100 ms). Lăn / Page Down trong lưới: xử lý mỗi bước
p50 1,4–1,6 ms, p95 2,0–2,2 ms (Page Down tới vòng này đi xuống trang ẩn sau lưới — nay cuộn lưới; lưới cũng nhận
lăn khi chuột chưa di chuyển). Bản đối chiếu không cho đo thời gian từng bước. Gõ trong Edit Text với khung các khối:
tổng mỗi phím p50 1,9 / p95 2,9 ms (vòng 10: 2,2 / 2,7).

| Đo | Vòng 10 | Vòng 11 |
| --- | --- | --- |
| Gõ trong Edit Text, tổng mỗi phím p50 / p95 | 2,2 / 2,7 ms | 1,9 / 2,9 ms |
| Lăn lưới Organize 300 trang, mỗi bước p50 / p95 | — | 1,4–1,6 / 2,0–2,2 ms |
| Ô đầu của lưới Organize sau khi bấm tab | — | 104–281 ms (bản đối chiếu, ngăn Pages: 170–198 ms) |

Còn lại: ngăn Format (Arrange) khi chọn trường như bản đối chiếu (ta dùng menu chuột phải), Create Multiple Copies /
Duplicate / dán trường; hộp Options riêng của hộp thả / ô đánh dấu / nút bấm chưa đo được trên bản đối chiếu (theo bố
cục của ta); ô số của Position chưa kèm đơn vị (“1 in”); màu khung khối chữ khi rê chuột (bản đối chiếu không đổi với
thông điệp gửi); ngăn Format cho hình đã vẽ (Edit Object) chưa có mục Shape; Assistant ▾ của tab View (trợ lý AI của
bản đối chiếu) không làm; các mục còn lại của vòng 10 (JBIG2 có mất dữ liệu, JPEG2000, Optimize Scanned PDF, Multiple
Files, Certificate Protect / Security Policies).

## Vòng 12 (2026-11-17 → 2026-11-22, làm tiếp sau khi tạm dừng)

Đã gộp `work/desktop-performance` (việc giải phóng ảnh trang khi đóng tab dùng bản của BlueReader vòng 14; test của ta
giữ lại). Ảnh so sánh `editor-106` … `editor-115` (bản đối chiếu bên trái).

**Menu chuột phải của trường** (`editor-106`): đủ các dòng, vạch ngăn và quy tắc bật / xám như bản đối chiếu
(Properties… | Cut / Copy / Paste | Delete | Align / Center / Distribute / Size ▸ | Create Multiple Copies… / Set Tab
Order / Duplicate… / Use Current Properties as New Defaults). Chữ nay đúng cỡ (đo: “Use Current Properties as New
Defaults” 271 px, bản đối chiếu 272 px; trước 256 px), menu rộng 359 px (đối chiếu 357), dòng 28 px. Phím tắt chữ như
bản đối chiếu (Proper_t_ies, Cu_t_, _C_opy, _P_aste, _D_elete, A_l_ign, Ce_n_ter, Distri_b_ute, Si_z_e, Create _M_ultiple,
Set Tab _O_rder, Duplicat_e_, _U_se): gõ chữ khi menu mở chạy dòng đó; Shift+F10 mở menu ở trường đang chọn. Gạch chân
chữ tắt (khi mở bằng bàn phím) chờ phần hỗ trợ chung của menu bật lên đang làm ở BlueSheet — vị trí từng gạch đã tính
sẵn. Set Tab Order xám khi chỉ chọn một trường như bản đối chiếu (bật từ hai trường; bản đối chiếu không nhận Shift+bấm
gửi bằng thông điệp nên chưa xem được trạng thái hai trường).

**Hộp thoại**: chữ tiêu đề lớn hơn chữ thân ~6 % như bản đối chiếu (“Create Multiple Copies of Fields” 222 / 209 px).
Create Multiple Copies 545 × 585 px khớp (`editor-107`); Split › Output Options 553 × 458 px, khung “Output Folder” dời
xuống đúng chỗ (`editor-114`).

**File › Properties** (`editor-108` … `editor-112`): đường dẫn dài bị cắt bên trái trong ô 413 px như bản đối chiếu; giá
trị cao hơn nhãn 3 px; “Custom” cách dòng cuối 74 px. Fonts: cây kiểu cổ điển (ô 11 px viền #919191, dấu trừ xanh đậm,
đường chấm đen), xếp theo tên, “Type:Type1”, “Encoding:Ansi”; font không nhúng có “Actual Font:” — mặt chữ mà chính ta
dùng để vẽ (cùng bộ chọn với trình vẽ trang, `PdfDocument::substitute_font`: Helvetica → Arial nếu máy có, không thì
Liberation Sans đóng gói) và “Actual Font Type:TrueType”. Security: ổ khoá cao 32 px như bản đối chiếu (hình của ta),
tam giác thả xuống 9 × 5 px; mũi tên combo 11 × 6 px xám. Ngày không đọc được hiện “—” (bản đối chiếu in số vô nghĩa).

**Comment**: Measure ▾ dùng kiểu menu của editor (dòng 29 px) với biểu tượng riêng cho Distance / Perimeter / Area
(`editor-113`); bảng Stamp (mẫu tem của ta) thu nhỏ chữ tiêu đề dài cho vừa khung (`editor-115`).

**Khác**: chú thích nút ribbon không còn hiện đè lên hộp thoại của editor.

| Đo (bản release) | Kết quả |
| --- | --- |
| Bộ nhớ riêng: trang Start / mở 10 tab sổ 300 trang / sau khi đóng hết | 51,8 / 172,4 / 55,7 MB |
| Gõ trong Edit Text, trang 1 000 đoạn, tổng mỗi phím p50 / p95 | 42,9 / 54,6 ms (xử lý 13,9 / 18,8; dựng ảnh 27,0 / 35,0) |

Còn lại: gạch chân chữ tắt của menu trường (nối vào hỗ trợ chung của menu); gõ trên trang 1 000 đoạn chậm vì 500 đoạn
cùng cột gộp thành một khung sửa (mỗi phím dàn lại và vẽ lại cả khung) — cần dàn lại / vẽ lại theo đoạn; ngăn Format
(Arrange) khi chọn trường; cột ba nút nhỏ cuối tab Comment (danh sách bình luận, tuỳ chọn hiển thị ▾, giữ công cụ) của
bản đối chiếu — ta còn nút lớn Comment List; Organize của bản đối chiếu giữ trang tài liệu (ảnh thu nhỏ ở ngăn Pages),
ta hiện lưới trang; kéo thả sắp trang, Insert từ tệp có hộp vị trí, Page Labels; các mục còn lại của vòng 11.
