# BlueReader — so sánh với trình đọc PDF trên desktop

Đo black-box với trình đọc PDF đã cài trên máy (Windows 125 %, cỡ chữ hệ thống 110 %), cùng tệp, cùng kích thước
cửa sổ; không dùng tên, icon hay màu thương hiệu của sản phẩm đó (ADR-0006 bổ sung 2026-11-15).

## Vòng 1 (2026-11-15)

**Lõi `bo-pdfread`** (dùng chung với BlueEditor): xref / object stream, tệp mã hoá mật khẩu rỗng, kế thừa tài
nguyên trang, CropBox / Rotate, nhãn trang, outline, link; đường vẽ, clip, không gian màu, ảnh (JPEG, soft mask),
shading, pattern, Form XObject, Optional Content, alpha hằng; font nhúng TrueType / OpenType / CFF đóng gói lại cho
DirectWrite, Type1 / Type3 vẽ bằng đường viền, 14 font chuẩn thay bằng font đóng gói tương thích; chữ kèm vị trí
(tìm, chọn, sao chép; ligature và tiếng Việt đúng); vẽ appearance stream của chú thích, ghi chú thích mới bằng cập
nhật nối thêm. 200 tệp đột biến không panic; giới hạn thời gian / bộ nhớ mỗi trang.

**Cửa sổ `bluereader-native.exe`**: thanh tiêu đề (truy cập nhanh, ô tìm), ribbon File / Home / Comment / View /
Help, nhiều tài liệu dạng tab + tab Bắt đầu (tệp gần đây), cột trái bookmark / ảnh trang / danh sách chú thích,
bốn kiểu bố cục trang, vừa chiều rộng / vừa trang / 100 %, thanh zoom, xoay, công cụ bàn tay, chọn chữ, link,
in, lưu / lưu thành.

| Hạng mục | Bản đối chiếu | BlueReader |
| --- | --- | --- |
| Chiều cao, màu xám khung (title, ribbon, tab, cột, cuộn, trạng thái) | #FBFBFD, #C7CCD9, #F0F1F5, #F5F6FA, viền trang #77787A | như nhau |
| Nhãn tab / điều khiển thanh trạng thái | — | lệch 1–5 px / ±2 px |
| Zoom mặc định (tệp mẫu) | 98,29 %, trang 1 ở x 286–1260 | 98,21 %, cùng vị trí |
| Zoom mặc định (sổ tay 1 202 trang, mở cột bookmark) | 108,48 % | 108,35 % |
| Độ phủ mực chữ trang | 34,7 | 31,9 (nhạt hơn ~8 %) |

Tốc độ (release): tệp mẫu 5 trang mở 22 ms, trang đầu 43 ms; sổ tay 10 MB 1 202 trang mở 214 ms (trước khi phân
tích song song 1 518 ms), trang đầu 224 ms, vẽ một trang trung vị 1,3–2,6 ms; cuộn 80 nấc chuột trung vị 1,3 ms,
p95 12,7 ms, không có trang trắng.

Còn lại: blend mode / soft mask (tô sáng chưa nhân màu), JPEG 2000 / JBIG2 / CCITT (ô xám), mesh shading, trang rất
lớn ở zoom cao (cần vẽ theo ô), hộp thoại in riêng, chế độ đọc toàn màn hình, sửa / xoá chú thích có sẵn, chữ trang
nhạt hơn ~8 %.

## Vòng 2 (2026-11-15)

Gộp nhánh BlueEditor (Look / Profile của cửa sổ, sửa / xoá chú thích, đường ghi PDF) rồi làm tiếp trên đó;
phần thêm của BlueReader nằm ở `reader_extra.rs` (mô-đun con của `reader_window`), `reader_print.rs`,
`bo-pdfread/src/{annot_edit,codecs,mesh}.rs`, chỉ móc vài dòng vào mã chung.

**Chữ trang PDF.** Bản đối chiếu vẽ chữ trang bằng ClearType (có viền màu; trang có nhóm trong suốt thì xám).
Đo độ phủ mực từng dòng (tệp thử: Helvetica / Times chuẩn 14, Arial / Times New Roman TrueType nhúng, font CFF
nhúng; cỡ 9 / 11 / 14 pt), tỉ số BlueReader / bản đối chiếu:

| Cách vẽ | 100 % | 139 % |
| --- | --- | --- |
| ClearType mặc định hệ thống (vòng 1) | 0,957 (0,87–1,03) | — |
| ClearType gamma 1,8, tương phản 1,0 (mới) | **1,001** (0,91–1,07) | **0,98** |

Lưới thử gamma 1,0–2,2 × tương phản 0,5–3: 1,4/0,5 → 1,013; 1,8/1,0 → 1,001; 2,2/2 → 1,019. Chữ BlueDoc
không đổi (biến thể `TextRendering::Tuned` chỉ dùng cho trang PDF; `BO_PDF_TEXT` để so).

**Blend mode và soft mask.** Nhóm `PushGroup { blend, mask }` mới trong display list; Direct2D: soft mask là
layer có opacity brush (LuminanceToAlpha trên nền /BC), blend mode chép nền dưới nhóm rồi trộn bằng hiệu ứng
Blend. Tệp thử 16 chế độ × 8 điểm màu so với MuPDF: chênh trung bình 1,0, lớn nhất 2 mỗi kênh (ColorDodge /
ColorBurn ở giá trị biên: 10); so với bản đối chiếu (5 điểm): lệch ≤ 2. Tô sáng nhân màu như bản đối chiếu.

**Ảnh.** JPEG 2000, JBIG2 (cả JBIG2Globals), CCITT G3/G4 giải mã bằng hayro (MIT / Apache-2.0): tệp thử ba
ảnh giống MuPDF. Shading lưới (kiểu 4–7, patch tensor vẽ như Coons) và shading hàm (kiểu 1) thành tam giác
màu phẳng. Trang rất lớn ở zoom cao (A4 1600 % ≈ 16 000 × 22 000 px) chỉ vẽ phần quanh vùng nhìn.

**In, chế độ đọc, toàn màn hình.** Hộp thoại In bố cục như bản đối chiếu (đo ở 125 %: 1535 × 870 px, cả hai
bằng nhau): Name / Copies / Collate / Print as grayscale; Print Range (trang hiện tại, tất cả, danh sách trang
“/ N” kèm ví dụ, tập con tất cả / lẻ / chẵn, đảo thứ tự); Print Handling (None, Fit / Reduce to printer margins);
Auto-Rotate / Auto-Center; xem trước từng trang với nút chuyển trang; OK / Cancel. In qua GDI (StartDoc /
StretchDIBits, trang vẽ bằng Direct2D tới 300 dpi, tự xoay, theo bộ hoặc không, thang xám). Chưa có: Current
view, in hai mặt, Tile / nhiều trang mỗi tờ / booklet, Custom scale, Print What.
Read Mode (tab View, ngay sau Select như bản đối chiếu; Ctrl+H): đo hàng ở 125 % — hàng tab ribbon giữ nguyên,
dải tab tài liệu y 100–143, mép trang 151, không còn thân ribbon, cột điều hướng, thanh trạng thái: **trùng
từng hàng** với bản đối chiếu. Full Screen (F11): đo ở 1920 × 1080 — nền đen, trang x 580–1340 (trùng), y 1–1077
(bản đối chiếu 2–1077), khung cửa sổ 1 px như bản đối chiếu; phím / nhấp / cuộn lật trang, Esc thoát.
**Chú thích.** Nhấp chọn (khung + tay nắm), kéo để di chuyển, nhấp đúp sửa nội dung, chuột phải: sửa, trả lời,
đổi màu (vẽ lại appearance), xoá; phím Delete; danh sách chú thích hiện luồng trả lời (/IRT); trả lời không vẽ
trên trang. Tất cả là bước hoàn tác của nhật ký sửa và lưu bằng cập nhật nối thêm.

Tốc độ (bản release, máy đang chạy song song bản build của các agent khác): sổ tay 1 202 trang mở 300 ms, trang
đầu 312 ms, dựng display list trung vị 1,0 ms, vẽ GPU trung vị 2,7 ms (60 trang đầu); trang thử blend (16 nhóm
trộn + tô sáng) vẽ 22 ms; trang thử lưới shading (8 708 tam giác) 50 ms.

Còn lại: các tuỳ chọn in nâng cao (hai mặt, nhiều trang mỗi tờ, booklet, Current view), Night Mode / Reflow /
AutoScroll của tab View, công cụ vẽ (Pencil, Callout, Stamp, Measure) của tab Comment, kéo đổi kích thước chú thích
bằng tay nắm, blend mode bên trong vùng cắt hình dạng (vẽ như Normal), patch tensor dùng điểm trong.

## Vòng 3 (2026-11-15)

Phần thêm nằm ở mô-đun riêng: `reader_impose.rs` (sắp trang in), `reader_viewx.rs` + `reader_viewx_win.rs`
(tab View), `reader_draw_win.rs` (công cụ vẽ), `bo-pdfread/src/annot_draw.rs` (chú thích vẽ); mã chung chỉ thêm móc.
Ảnh so sánh: thư mục `scratchpad/show/reader-NN-*.png` của phiên (bản đối chiếu trên / trái).

**Web.** Display list `pushGroup` / `popGroup` (blend mode, soft mask) có kiểu TS; canvas vẽ blend bằng
`globalCompositeOperation`, soft mask bỏ qua (vẽ như nội dung thường). `pnpm build:wasm` xanh trở lại (sửa
`clip_xfer.rs` không có `bundled-fonts`).

**In.** Hộp thoại thêm Current view (vùng đang thấy, in đứng, vừa khổ), in hai mặt (lật cạnh dài / ngắn, đặt qua
DEVMODE), ba nút Print Handling đo theo bản đối chiếu (123 × 51 px ở 125 %, chọn: viền 0,84,153 nền 204,228,247):
Scale (None / Fit / Reduce / Custom %), Multiple Pages Per Sheet (2 / 4 / 6 / 9 / 16, bốn thứ tự, viền trang; 2 và
6 trang xoay tờ ngang), Booklet (thứ tự gập giữa, gáy trái / phải, cả hai mặt / mặt trước / mặt sau). Xem trước
hiện ô trang trên tờ. Kiểm bằng in thật qua "Microsoft Print to PDF" (`bluereader-native --print in.pdf
"<máy in>" out.pdf --nup 4 --border | --booklet | --scale 50 | --view x,y,w,h`), dựng lại bằng MuPDF: 5 trang →
2 tờ (4-up), 3 tờ (2-up), 4 mặt (booklet), đúng thứ tự (`reader-01-print-layouts.png`). Chưa so được bản in của
bản đối chiếu: không mở được hộp thoại In của nó mà không dùng thao tác thật.

**Tab View** (đo lại thứ tự theo ribbon bản đối chiếu, `reader-05-view-tab.png`): Read Mode, Reflow, Reverse View,
Night Mode | Rotate View | lưới bố cục trang 3 × 2 (thêm trang bìa riêng) | AutoScroll | Word Count | View Setting
(zoom, vừa trang, bảng điều hướng, thước, lưới, toàn màn hình). Night Mode đo trên bản đối chiếu: nền trang
(25, 25, 25), mọi chữ (127, 127, 127), trắng / đen của hình vẽ theo nền / chữ, màu khác và ảnh giữ nguyên, nền
canvas không đổi (`reader-07-night-mode.png`). Reflow: chữ trang dàn lại theo bề rộng cửa sổ, cuộn sang trang kế.
AutoScroll: phím lên / xuống đổi tốc độ, Esc dừng (nhịp 100 ms).

**Tab Comment** (`reader-06-comment-tab.png`): Note | Typewriter, Text Box, Callout | Drawing (chữ nhật, bầu dục,
đường, mũi tên, đa giác, đường gấp khúc), Pencil | Measure (khoảng cách, chu vi, diện tích theo inch, `/Measure`
§12.9) | Stamp (8 con dấu tên chuẩn Table 181, hình vẽ của ta) | Properties (màu, độ mờ, độ dày, cỡ chữ). Kéo
tay nắm góc để đổi kích thước. Hình học đọc lại từ khoá chuẩn (`/InkList`, `/L`, `/Vertices`, `/CL`, `/RD`) nên
đổi kích thước / thuộc tính dựng lại appearance. Tệp có đủ 12 loại mở trong bản đối chiếu trùng hình
(`reader-08-drawn-comments.png`); lệch duy nhất (con dấu không đậm) là lỗi bộ nhớ đệm font từ điển trực tiếp,
đã sửa.

**Vẽ trang.** Chạy 15 tệp (5 PDF có sẵn trong phần mềm đã cài, 4 tệp vòng 2, 5 tệp khó tự sinh, tệp mẫu) so với
MuPDF ở 100 % (125 %), chênh trung bình mỗi kênh / % điểm ảnh lệch > 48:

| Tệp | Trước | Sau | Sửa |
| --- | --- | --- | --- |
| misc (chế độ vẽ chữ, mẫu, màu) | 12,74 / 6,42 % | 3,20 / 2,30 % | chữ Tr 1/2/5/6 viền nét, Tr 4–7 thành vùng cắt |
| clipblend (blend trong vùng cắt tròn) | 8,02 / 7,26 % | 0,39 / 0,23 % | đóng / mở lại layer quanh nhóm blend |
| tensor (patch kiểu 7) | 3,07 / 0,14 % | 0,64 / 0,14 % | dùng 4 điểm trong |
| type3 | 0,43 | 0,10 | (theo chế độ vẽ chữ) |
| license (AES-256, mật khẩu người dùng rỗng) | không mở | 5,47 (khác ClearType) | /O /U đệm quá 48 byte, /Encrypt trực tiếp |

Ảnh nhỏ phóng to không có `/Interpolate` vẽ theo điểm ảnh như bản đối chiếu. Các tệp còn lại 0,4–3,8 (khác cách
khử răng cưa chữ). Knockout group: 0,71 (chưa cài knockout, chênh nhỏ). Overprint chưa làm.

**Mở tệp.** Sổ tay 1 202 trang (bản release, máy vẫn chạy 9 tiến trình biên dịch của agent khác, CPU 44–49 %):
mở 216–220 ms, trang đầu 226–232 ms (vòng 2: 300 / 312 ms; mục tiêu vòng 1: 214 ms). Đo tách: phân tích
lopdf (song song) 204–209 ms, quét cây trang 6 ms, nhóm nội dung ẩn < 0,1 ms — phần còn lại là font; chưa đo
được trên máy rảnh.

**So hộp thoại In** (`reader-09-print-dialog.png`): các nhóm, nút Print Handling và Current view trùng vị trí bản
đối chiếu. Còn thiếu: Tile Large Pages, nhóm Orientation / Print What / Output, Properties / Advanced, Print as
image, xem trước tờ nhiều trang vẽ ô thay vì ảnh trang.

Còn lại: Text Viewer, Read (đọc to), chia đôi cửa sổ (Split), Eraser / Area Highlight / Custom Stamp của tab
Comment, chữ vẽ bằng viền (Tr 1/2) trong Night Mode vẫn giữ màu, AutoScroll nhịp 100 ms (giật hơn bản đối chiếu),
knockout group, overprint, mở 1 202 trang ≤ 210 ms đo trên máy rảnh.

## Vòng 4 (2026-11-16)

Phần thêm ở mô-đun riêng: `reader_view4_win.rs` (Text Viewer, chia khung, đọc to, AutoScroll mượt),
`reader_tabs4_win.rs` (tab Form / Protect, Eraser, Area Highlight, Custom Stamp, Search & Highlight, SnapShot,
sắp xếp / lọc danh sách chú thích), `bo-pdfread/src/{cmyk,annot_image,color_tests}.rs`; host:
`reader_speech.rs` (SAPI), `reader_clip.rs` (ảnh lên clipboard). Ảnh so sánh: `scratchpad/show/reader-10…16-*.png`.

**Vẽ trang.** Tệp thử 16 trường hợp Separation / DeviceN hỏng mở trong bản đối chiếu (`reader-10`): hàm
PostScript trả ít kết quả hơn số thành phần → mọi thành phần 0 (trắng với CMYK, đen với RGB), trả nhiều hơn →
lấy n giá trị trên cùng; thiếu hàm / số đầu ra khác không gian thay thế → `cs` bị bỏ qua (giữ không gian trước);
không gian thay thế không đọc được → không vẽ. misc.pdf: ô xanh nhạt nay trắng như bản đối chiếu. Khớp 15/16 ô
(lệch: toán tử lạ `foo` trong hàm PostScript). CMYK → RGB: bảng 5⁴ = 625 ô đo trên cửa sổ bản đối chiếu, nội suy
tứ tuyến (cyan 0,173,238; magenta 236,2,141; K 36,32,33): lệch 0 tại cả 625 ô (trước: công thức 1−c, cyan
0,255,255). Knockout group (`/K true`): Direct2D primitive blend Copy trong một layer — vùng chồng chỉ còn đối
tượng sau như bản đối chiếu (`reader-11`). Overprint: bản đối chiếu không mô phỏng trên màn hình (đo), ta cũng
không; khi in có "Simulate Overprinting" (mực nhân nhau). Night Mode: chữ vẽ viền (Tr 1/2) nay xám như chữ khác
(màu chữ ghi đè trong trình thông dịch).

**In** (`reader-12`, `reader-13`, đo ô điều khiển của hộp thoại bản đối chiếu qua danh sách cửa sổ con ở 125 %):
Properties (hộp thoại driver máy in, giữ DEVMODE theo máy in), Print as image (150 dpi), Tile Large Pages (Page
Zoom, Overlap inch, Cut Marks, Labels — kiểm bằng kế hoạch: trang 17 × 22 inch → 3 × 3 tờ), Orientation (Auto /
Portrait / Landscape, đặt vào DEVMODE), Print What (Document / and markups / and stamps / Form fields only /
Annotations only; khi in chỉ chú thích có cờ Print), Output (Simulate Overprinting), Choose paper source by PDF
page size, danh sách thả xuống thật thay cho bấm vòng. Collate tắt khi 1 bản, Reverse tắt khi All pages như bản
đối chiếu. Xem trước: giấy thật của máy in (Microsoft Print to PDF 8.5 × 11 inch), ảnh trang thật cho mọi kiểu
(n-up, booklet, tile), Zoom (A4 lên Letter: 94,06 % — bản đối chiếu 93,73 %), thanh trượt. Advanced, Page
Setting, Summarize Comments, Bleed Marks: hiện mờ.

**View** (`reader-14`, `reader-15`): Text Viewer — chữ cả tài liệu thành dòng chữ thường giữ cột (đơn vị cột 0,8
bề rộng chữ, dòng trống khi khoảng cách dòng > 1,25 lần), trang giấy trắng viền xám như bản đối chiếu. Chia khung
dọc / ngang / kiểu bảng tính: mỗi khung một vị trí cuộn, cùng zoom, ảnh trang dùng chung; bánh xe / nhấp chuyển
khung (bản đối chiếu chỉ hiện ô kéo chia ở đầu thanh cuộn, ta chia đôi ngay). Read: đọc trang / đọc từ trang
hiện tại, tạm dừng, dừng, chậm / nhanh qua giọng đọc Windows (SAPI, chỉ tạo khi bấm). AutoScroll: đo bản đối chiếu
2,7 px/giây ở 125 % (11 px sau 4 giây); ta cuộn theo thời gian mỗi khung hình (7 mức, ↑/↓ đổi).

**Comment / Home / Form / Protect** (`reader-16`): lưới đánh dấu chữ 3 × 2 (thêm Replace Text: gạch xoá rồi hỏi chữ
thay; Insert Text: ghi chú), File (mờ), Eraser (xoá nét bút chì dưới con trỏ), Area Highlight, Search & Highlight
(tô sáng mọi kết quả), Custom Stamp từ ảnh JPEG / PNG (Stamp có appearance ảnh), Summarize / Import / Export (mờ);
danh sách chú thích sắp theo trang / tác giả / ngày / loại và lọc theo loại. Home: SnapShot (vùng kéo chép vào
clipboard dạng ảnh, 2× màn hình 100 %), Reflow, Fill & Sign (mờ). Form: điền trường chữ (hộp nhập), ô chọn, nút
radio, danh sách bằng công cụ Hand; Reset Form; Form to sheet (CSV); Tool Settings › Highlight Fields. Protect:
Validate kiểm chữ ký chỉ đọc bằng `bo-sign` (`verify_pdf`); Sign / Time Stamp / Fill & Sign mờ (thuộc BlueEditor).

**Mở tệp.** Sổ tay 1 202 trang (bản release, máy có 5 tiến trình `rustc` của agent khác): mở 259 / 281 / 299 ms,
trang đầu 270–311 ms, dựng display list trung vị 1,1 ms, vẽ GPU trung vị 1,9 ms. Chưa đo được trên máy rảnh;
phần lớn là `lopdf::Document::load_mem` (vòng 3: 204–209 ms).

Còn lại: mở ≤ 210 ms trên máy rảnh (cần nạp đối tượng lười thay vì `load_mem`), trang Bắt đầu chưa so (bản đối
chiếu hiện tệp gần đây của chủ máy — không chụp), Advanced / Page Setting / Summarize Comments / Import / Export /
đính kèm File, Insert Text thật (chú thích Caret), chia khung kéo được, đọc to theo từng câu có tô sáng, toán tử
lạ trong hàm PostScript.

## Vòng 5 (2026-11-16)

Phần thêm ở mô-đun riêng: `reader_tabs5_win.rs` (Điền & Ký, ký số, dấu thời gian, trạng thái bật / tắt, tóm tắt /
nhập / xuất chú thích, File), `reader_bookmarks5_win.rs` (cột bookmark), `reader_props5_win.rs` (Thuộc tính tài
liệu), `reader_backstage5_win.rs` (trang File); lõi: `bo-pdfread/src/{exchange,viewer}.rs`. Ảnh so sánh:
`scratchpad/show/reader-19…25-*.png` (bản đối chiếu trên / trái).

**Trạng thái bật / tắt như bản đối chiếu** (`reader-19`). Home / Protect: Fill & Sign mở tab ngữ cảnh "Điền & Ký"
(thêm chữ, dấu tích, dấu chéo, chấm, gạch, ảnh chữ ký — đều là chú thích, lưu nối thêm); Sign & Certify ký PAdES
bằng tệp .p12 / .pfx qua `bo-sign` (khung kéo trên trang, chữ ký hiện, dấu thời gian tuỳ chọn; bản đã ký mở tab
mới); Time Stamp Document thêm dấu thời gian tài liệu (`add_document_timestamp`). Validate mờ tới khi nhấp chọn một
chữ ký trên trang; Reset Form mờ khi tệp không có biểu mẫu. Comment: cột ba hàng Summarize Comments / Import /
Export (đo: hàng cách nhau 29 px ở 125 %, như bản đối chiếu) và cột ba biểu tượng nhỏ; Summarize tạo PDF tóm tắt
(tiêu đề, theo trang: số, tác giả, chủ đề, ngày, nội dung; chữ là đường viền font đóng gói nên tiếng Việt đúng);
Import nhận FDF / XFDF (FDF chép nguyên chú thích kèm appearance, XFDF dựng lại; tên `/NM` đã có thì bỏ qua);
Export ra FDF hoặc XFDF; File đính kèm tệp tại chỗ nhấp. Home: Clipboard (Cut / Paste mờ, Copy mờ khi chưa chọn,
Select All) và Page Fit Option như bản đối chiếu, bỏ nút Find. Lệnh mờ không chạy khi nhấp.

**Biểu tượng đánh dấu chữ** (`reader-21`): hình là của ta (dòng chữ có dải tô, chữ "A" không chân với gạch chân /
lượn sóng / gạch ngang, mũi tên cho Replace, dấu cộng cho Insert), chỉ giữ ý nghĩa màu vàng cam của bản đối chiếu
(đo 249, 177, 10); các biểu tượng Summarize / Import / Export / danh sách / tuỳ chọn / ghim cũng vẽ lại theo bộ của
ta (bản `reader-20` bị thay vì quá giống hình của bản đối chiếu).

**Hành vi** (đưa sự kiện vào cửa sổ bản đối chiếu bằng PostMessage, cùng tệp thử `beh.pdf` tự sinh):

| Thao tác (125 %) | Bản đối chiếu | BlueReader trước → sau |
| --- | --- | --- |
| Mũi tên xuống | 54 px | 50 → 54 px |
| Một nấc bánh xe | 60 px | 75 → 60 px |
| PgDn | 728 px (khung 803 px) | 0,9 khung (≈ như nhau) |
| Phải / Trái / Home / End | trang sau / trước / đầu / đầu trang cuối | như nhau |
| Danh sách zoom | 6400 … 1 %, Actual Size, Fit Page, Fit Width | như nhau (trước: 10 … 1600 %) |
| Ctrl + bánh xe | (không đưa được Ctrl bằng PostMessage) | bước theo danh sách trên |
| Cột bookmark | mở khi `/PageMode /UseOutlines`; mục `/Count > 0` mở sẵn | trước: luôn mở, mọi mục đóng → như nhau |
| Link web | hộp cảnh báo bảo mật | trước: mở thẳng → hỏi Allow / Block |

Cột bookmark (`reader-22`): hàng công cụ (mở / thu tất cả, tìm bookmark hiện tại, chữ to / nhỏ), ô tìm lọc
bookmark, đường nối chấm, bookmark của trang hiện tại tô màu nhấn; hàng đầu tiên cùng toạ độ y với bản đối chiếu
(lệch 1 px), thụt mỗi cấp 22 px. Bản đối chiếu bỏ qua chuột kéo, nhấp đúp / ba và Ctrl khi đưa bằng PostMessage
(không chọn chữ được), nên chọn chữ, Ctrl+A, Ctrl+F chưa so được — không dùng thao tác thật.

**File và Thuộc tính** (`reader-24`, `reader-25`): File mở trang File toàn cửa sổ cùng dạng với BlueEditor
(Open: Recent Documents của chính BlueReader, Computer; Properties; Save mờ khi chưa sửa; Save As; Print; Revert;
Close). Chỉ chụp cột trái trang File của bản đối chiếu (cột phải là tệp gần đây của chủ máy — không chụp). Hộp
Thuộc tính tài liệu: Description (tệp, tiêu đề, tác giả, ngày, ứng dụng, phiên bản PDF, vị trí, cỡ tệp, khổ, số
trang, có thẻ, fast web view), Security (phương thức, quyền in / sửa / chép / chú thích / điền / sắp trang), Fonts
(tên, loại, bảng mã, nhúng hay không), Advanced (URL gốc, ngôn ngữ, trapped, đóng gáy, co giãn khi in, in hai mặt,
số bản).

**Mở tệp.** Sổ tay 1 202 trang, bản release, 5 lần (`--bench`): mở 351 / 384 / 407 / 510 / 689 ms, trang đầu
376–752 ms — máy không lúc nào rảnh (10 tiến trình biên dịch của agent khác, CPU 63–100 % trong cả 25 phút chờ),
nên số này không so được với mục tiêu 210 ms. Nạp đối tượng lười chưa làm: `lopdf` 0.38 không cho đọc một đối tượng
theo offset từ ngoài (`Reader::read_object` riêng tư), cần thay bộ đọc đối tượng của chính ta.

Còn lại: nạp lười để mở ≤ 210 ms (đo trên máy rảnh), so chọn chữ / tìm (Ctrl+F) với bản đối chiếu (cần thao tác
thật), danh sách zoom và menu dùng chung cỡ chữ lớn hơn bản đối chiếu, Batch Print / Share / Preferences của trang
File, Insert Text thật (Caret), chia khung kéo được, Advanced / Page Setting trong hộp In.

## Vòng 6 (2026-11-16)

Phần thêm ở mô-đun riêng: `bo-pdfread/src/{store,annot_caret}.rs`, `reader_prefs6_win.rs` (Tùy chọn, In hàng loạt,
Chèn chữ, cuộn mượt), `reader_print6.rs` (Nâng cao / Thiết lập trang của hộp In). Ảnh so sánh:
`scratchpad/show/reader-26…34-*.png` (bản đối chiếu trên / trái).

**Bộ đọc đối tượng của chính ta** (`store.rs`, `reader-26`). Mở tệp chỉ đọc bảng xref / xref stream (`/Prev`,
`/XRefStm`) và trailer; đối tượng được phân tích khi dùng lần đầu (bộ phân tích riêng trên lexer nội dung, `/Length`
gián tiếp, object stream giải nén một lần rồi giữ, offset sai thì tìm theo đầu `n g obj`). Sửa, lưu và các hàm của
lopdf đi qua `Deref` / `DerefMut`, nạp `lopdf::Document` đầy đủ một lần như trước — đường ghi của BlueEditor không
đổi (test xanh). Tệp mã hoá hoặc xref hỏng nạp đầy đủ khi mở. Mở sổ tay 1 202 trang trong cửa sổ không nạp toàn bộ
(kiểm bằng `BO_PDF_LOAD_TRACE`).

| Sổ tay 1 202 trang, `--bench`, bản release | Vòng 5 | Vòng 6 |
| --- | --- | --- |
| Mở (máy bận: 7 tiến trình biên dịch, CPU 26–43 %) | 351–689 ms | 33–49 ms |
| Trang đầu (cùng điều kiện) | 376–752 ms | 43–59 ms |
| Bản cuối, 5 lần sau 25 phút chờ máy rảnh (không lúc nào rảnh: 10 trình biên dịch, CPU 43–60 %) | — | mở 36–74 ms, trang đầu 47–86 ms (nhỏ nhất 47,1 ms; mục tiêu ≤ 210 ms) |

**Menu** (`reader-27`, `reader-28`). Đo menu của bản đối chiếu ở 125 %: hàng 30 px, chữ 13 px (chữ số cao 9 px),
nhãn cách khung 41 px, biểu tượng 20 px cách 10 px, khung 195 trên nền 251, 251, 253, đường kẻ trước Actual Size.
`popup::MenuLook` mới (mặc định như cũ cho BlueDoc / BlueSheet / BlueSlide / BlueEditor); menu của BlueReader:
hàng 30 px (trước 33), chữ số 9 px (trước 13), nhãn cách khung 42 px, menu Clipboard 131 × 129 px (bản đối chiếu
131 × 131).

**Trang File** (`reader-29`, `reader-30`). Đo lại cột trái ở cùng 125 % / cùng cỡ cửa sổ: cỡ chữ vốn đã bằng nhau
("Properties" rộng 74 px cả hai — ảnh vòng 5 bị thu nhỏ); sửa màu cột 245, 246, 250 + viền 199, 204, 217, hàng
cách 46 px từ 14 px dưới tab, đường kẻ 230, 233, 240 thêm 7 px, ô lệnh đang chọn góc 4 px; nhãn lệch ≤ 2 px. Thêm
Batch Print (bản đối chiếu hiện bật): chọn các tài liệu đang mở rồi một hộp In, host in từng tài liệu (mọi trang)
với cùng thiết lập. Preferences: thu phóng mặc định, bố cục trang, tác giả chú thích, đơn vị đo (in / mm / cm / pt
/ pica, cho công cụ đo), cuộn mượt; host lưu `reader-prefs.txt`. Share, Skins không làm. Hộp Preferences và các
hộp con của hộp In bản đối chiếu không mở được nếu không thao tác thật (UIA Invoke bị bỏ qua) nên bố cục là của ta.

**Chèn chữ, chia khung, hộp In** (`reader-31`, `reader-32`). Insert Text đặt chú thích Caret thật (§12.5.6.11,
`/Sy /None`, nội dung là chữ cần chèn) ở mép ký tự gần nhất. Thanh chia khung kéo được (con trỏ mũi tên hai đầu;
mỗi khung giữ ít nhất 1/10). Hộp In: Advanced (in thành ảnh 72 / 150 / 300 / 600 dpi; màu, thang xám, đen trắng)
và Page Setting (giấy của máy in hoặc Letter / Legal / A3 / A4 / A5; lề mm, vùng in co lại theo lề).

**Chữ sao chép** (`reader-33`). `bluereader-native --text` in ra đúng chữ Select All + Copy; so với MuPDF 1.27
(chữ của bản đối chiếu không đọc được: mẫu Text của UIA chỉ có khi tài liệu nhận focus thật):

| Tệp | Kết quả |
| --- | --- |
| Sổ tay 1 202 trang | 585 524 từ, 100,00 % cùng thứ tự và khoảng trắng |
| Hai cột viết xen dòng (tự sinh) | trước: L0 R0 L1 R1… như MuPDF; nay đọc hết cột trái rồi cột phải |
| Tiếng Việt, Arial nhúng, từng từ đặt riêng | giống MuPDF trừ 4 khoảng trắng MuPDF bỏ sót sau ủ / ự / ở / ợ / ả |

Thứ tự đọc: cắt XY các dòng; chỉ cắt cột khi rãnh ≥ 1 em và mỗi bên có ≥ 4 dòng chữ rộng ≥ 15 em, phần lớn dài
≥ 60 % cột — bảng (trang 26, 32 của sổ tay) và trang một cột giữ thứ tự nội dung.

Toàn cửa sổ (`reader-34`): cùng tệp, bố cục ribbon / tab / cột bookmark / trang / thanh trạng thái như vòng 5.

Còn lại: xem trước của hộp In chưa theo giấy / lề của Page Setting; hình học caret và màu xanh chưa đo được trên
bản đối chiếu; menu Clipboard chưa có biểu tượng; Share / Skins; so chọn chữ bằng chuột với bản đối chiếu (cần thao
tác thật).

## Vòng 7 (2026-11-16)

Phần thêm ở mô-đun riêng: `reader_keys7_win.rs` (phím tắt ribbon), `reader_print7.rs` (thứ tự Tab của hộp In);
lõi: `bo-pdfread` (`MeasureUnit`, `mesh::radial`, bảng mã Symbol / ZapfDingbats); host: `--search`, `--render a-b`,
`win32::enable_ribbon_keys` / `enable_full_client` (chỉ BlueReader bật), `Device::trim` (bo-gpu-win, thêm mới).
Ảnh so sánh: `scratchpad/show/reader-35…41-*.png`.

**Phần còn của vòng 6.** Xem trước của hộp In theo giấy và lề của Page Setting (giấy chọn giữ mép không in được
của máy in, trang vừa vùng trong lề, lệch theo lề trên / trái như khi in thật, `reader-38`). Menu Clipboard có biểu
tượng riêng (cắt, chép, dán, chọn tất cả — hình của ta, `Menu::with_icon_fn` thêm mới, các app khác không đổi,
`reader-39`). Đơn vị đo của Tùy chọn nay dùng cho cả chú thích đo đã lưu: nhãn, `/Contents` và `/Measure` (§12.9
Table 261 / 263: `/U`, `/C` = 1 / số point mỗi đơn vị, `/R` "1 mm = 1 mm"); sửa / đổi cỡ giữ đơn vị riêng của chú
thích (đọc lại từ `/Measure`).

**Vẽ trang: tập 58 PDF** (17 tệp khó tự sinh của các vòng trước, 2 tài liệu cài sẵn, 5 tệp font mới: chuẩn 14 không
nhúng, CFF / CID nhúng (Nimbus, Droid CJK), OpenType CFF, TrueType symbolic, Identity-V; 31 tệp `bo-cli convert` từ
corpus docx / pptx / xlsx), mọi trang (tối đa 30 mỗi tệp) so với MuPDF 1.27 ở 100 % / 125 %, chênh trung bình mỗi
kênh. 52 tệp ≤ 3 (khác cách khử răng cưa chữ ClearType); ba lỗi thật, đã sửa (`reader-36`):

| Tệp | Trước | Sau | Nguyên nhân |
| --- | --- | --- | --- |
| fox.pdf (nền gradient) | 4,80 / 11,95 % điểm lệch > 48 | 0,57 / 0,00 % | shading tròn (type 3) qua ma trận không đồng dạng (ellipse) vẽ như hình tròn → nay thành vòng tam giác, có Extend |
| TrueType symbolic không có `/Widths` | 2,49 (chữ chồng một chỗ) | 0,78 | dùng hmtx của font nhúng |
| Symbol / ZapfDingbats chuẩn 14 không nhúng | 9,21 (Latin / trống) | đúng hình (chênh còn ~9 do Nimbus ≠ Arial / Times) | bảng mã dựng sẵn Annex D.5 / D.6; Symbol thay bằng mặt chữ Unicode có chân |

CFF CID, OpenType CFF, Type 3: 0,94 / 0,12; Identity-V (dọc): 0,42. `sep.pdf` 15,3 là cố ý (theo bản đối chiếu, vòng 4).

**Tài liệu lớn** (sổ tay 1 202 trang, bản release, máy có 9–13 tiến trình biên dịch của agent khác):

| Đo | BlueReader | Bản đối chiếu |
| --- | --- | --- |
| Bánh xe 80 nấc (đầu vào → khung hình) | p50 1,6 ms, p95 2,2–7,1 ms | — (không đo được độ trễ) |
| PageDown 40 lần | p50 2,0 ms, p95 15–23 ms | — |
| Cột ảnh trang: 550 nấc | p50 1,2 ms, p95 5,6 ms; ảnh trang chỉ vẽ hàng đang thấy | 60 px / nấc |
| Ảnh trang trong cột | trước 150 px, viền 1 px → 126 px, viền 2 px (207, 207, 208), cách 200 px, 60 px / nấc | 126 px, 2 px, 200 px, 60 px |
| Tìm "snapshot" (354 kết quả) | kết quả đầu 15–41 ms, cả tài liệu 1,44 s (đọc theo nhịp, cửa sổ không đứng) | — |
| Bộ nhớ riêng sau mở / sau End | 112 / 107 MB (trước: 157 / 198 MB) | 48 / 71 MB |

Bộ nhớ: cửa sổ trống đã 51 MB (D3D / Direct2D); trước đây bộ đệm ảnh đã giải mã tới 384 MB (tìm hết tài liệu giải
mã mọi ảnh), trang lân cận ±2, bộ đệm trang 48 trang, bộ nhớ driver không trả lại. Nay: ảnh 64 MB, bộ đệm trang
~32 MB, lân cận ±1, `IDXGIDevice3::Trim` sau khi bỏ ảnh trang. Tìm kiếm: kết quả đầu sau trang hiện tại hiện ngay,
phần còn lại đọc theo nhịp 60 ms (thanh trạng thái "k / n (trang đã đọc / tổng)"), không đẩy trang đang thấy ra khỏi
bộ đệm.

**Khung cửa sổ và zoom vừa trang** (`reader-37`). Bản đối chiếu không có viền đổi cỡ ẩn: cùng cửa sổ 1518 × 1036,
vùng client của nó rộng 1518, của ta 1500 → zoom vừa rộng 110,29 % so với 108,55 %. BlueReader nay để client phủ cả
cửa sổ khi không phóng to (mép đổi cỡ 5 px bên trong, góc 20 px), phóng to như cũ: 110,37 %. Tệp mẫu: 99,57 % / 99,63 %
(`reader-41`). Lưu ý đo: exe mở ở trạng thái phóng to; các vòng trước đổi cỡ cửa sổ phóng to bằng SetWindowPos —
nay chụp với `BLUEOFFICE_SHOW_NOACTIVATE=1` (cửa sổ thường).

**Bàn phím, độ tương phản** (`reader-40`). Alt (nhấn rồi thả) hoặc F10: chữ gợi ý trên các tab ribbon; gõ chữ mở tab
và hiện gợi ý trên mọi lệnh của tab; ←/→ chuyển tab, ↓ / Enter vào lệnh, Tab / Shift+Tab / mũi tên chuyển vòng
focus, Enter chạy, Esc lùi một bậc; nhấp chuột thoát. Bản đối chiếu bỏ qua Alt đưa bằng PostMessage nên chữ gợi ý
của nó không chụp được (không dùng thao tác thật). Hộp In: Tab / Shift+Tab qua mọi điều khiển theo thứ tự đọc (cột
trái, cột giữa, xem trước, Page Setting / OK / Cancel), bỏ qua điều khiển mờ, ô chữ nhận gõ, Space bấm. Test tương
phản WCAG 2.2: chữ ≥ 4,5 : 1 trên mọi nền (kể cả hover / pressed), vòng focus, chữ gợi ý, viền trang ≥ 3 : 1.

**Mở tệp trên máy rảnh.** Chờ 60 phút một khoảng 2 phút không có cargo / rustc / link (`idle7.ps1`): máy không
lúc nào rảnh (khoảng yên lặng dài nhất 0 s; 9–13 tiến trình biên dịch của các agent khác). 5 lần đo ngay sau
đó (CPU 40–54 %): mở 38–48 ms, trang đầu 50–63 ms (vòng 6: 36–74 / 47–86 ms; mục tiêu ≤ 210 ms).

Còn lại: chữ gợi ý và thứ tự Tab của bản đối chiếu (cần thao tác thật), bộ nhớ còn ~1,5 × bản đối chiếu (nền D3D),
mở 1 202 trang đo trên máy rảnh, Share / Skins.

## Vòng 8 (2026-11-16)

Phần thêm ở mô-đun riêng: `reader_panels8_win.rs` (cột Lớp, Tệp đính kèm, danh sách chú thích), `reader_skins8_win.rs`
(Chia sẻ, Giao diện của trang File); lõi: `bo-pdfread/src/navdata.rs` (lớp, tệp đính kèm, trạng thái chú thích); host:
`ReaderRequest::OpenAttachment`, `host.reveal:`, cắt bộ nhớ driver khi rảnh, `BO_READER_MEMSTEPS`. Ảnh so sánh:
`scratchpad/show/reader-42…50-*.png` (bản đối chiếu trái).

**Cột trái** (`reader-42` … `reader-48`, đo ở 125 %). Tệp đính kèm: công cụ mở / lưu / thêm, cột Tên / Mô tả / Cỡ rộng
125 px, dải tiêu đề 24 px nền 245, 246, 250, hàng 21 px, cỡ làm tròn xuống KB; tài liệu có tệp nhúng mở sẵn cột này
(như bản đối chiếu), chấm đỏ 226, 63, 54 trên biểu tượng. Mở tệp: PDF mở thành tab; tài liệu / ảnh hỏi trước rồi mở bản
tạm bằng chương trình của Windows; chương trình / script chỉ được lưu. Biểu tượng cột đang chọn là "tab" màu nền cột
(viền 199, 204, 217, cao 56 px từ x 15), cột có cạnh trái 1 px. Chú thích: công cụ mở / thu tất cả, trước / sau, sắp
xếp (trang, loại, tác giả, ngày, trạng thái, đánh dấu, màu), công cụ thứ sáu mở nội dung; ô tìm, nút lọc (hộp Lọc: trạng
thái, tác giả, loại, đánh dấu), "Total Amount", nhóm 34 px nền 230, 233, 240, thẻ viền 199, 204, 217; tác giả đậm 700,
ngày canh phải (giờ nếu là hôm nay; thiếu `/M` thì dùng `/CreationDate`), bong bóng số trả lời; thẻ đang chọn: phần đầu
xám, nút ⋮, ô Add Reply (Enter thêm trả lời). Đặt trạng thái (Accepted / Rejected / Cancelled / Completed) và đánh dấu
kiểm là trả lời trạng thái ẩn (§12.5.6.3). Lớp: bản đối chiếu ẩn cột Lớp tới khi bật ở menu chuột phải của cột (menu này
bỏ qua nhấp đưa bằng PostMessage, không chụp được); BlueReader hiện biểu tượng khi tệp có lớp, ô mắt bật / tắt lớp và vẽ
lại trang. Thẻ cao bằng bản đối chiếu ±1 px (thẻ Bob 464–552 so với 463–553).

**Chia sẻ, Giao diện** (trang File). Chia sẻ: e-mail (`mailto:` ghi tên tệp), chép đường dẫn, mở thư mục chứa (Explorer
chọn sẵn tệp). Giao diện: Cổ điển (màu đo), Sáng, Xanh — lưu trong tuỳ chọn; Tối để mờ vì các ngăn / hộp thoại còn vẽ chữ
màu tối cố định. Ô tìm trên thanh tiêu đề có nút tuỳ chọn: Chỉ nguyên từ, Phân biệt hoa thường.

**Hành vi** (đưa sự kiện bằng PostMessage). F11 của bản đối chiếu nhận phím đưa vào: nền đen, trang vừa chiều cao, cùng
vị trí với của ta (x 580–1339 so với 580–1340, `reader-49`); nút thoát là cửa sổ con riêng (31 × 29 px, 94 px từ mép
phải), luôn hiện (kiểm 1 / 5 / 9 s và sau di chuột), chữ xám 180 không nền — của ta cùng chỗ, cùng cỡ, nền chỉ khi
hover (`reader-50`). Ctrl + bánh xe / Ctrl+= / Ctrl+-, Alt và menu thả của bản đối chiếu vẫn bỏ qua thông điệp đưa vào
(như các vòng trước) nên chưa so được; phím trang đã so ở vòng 5.

**Bộ nhớ** (sổ tay 1 202 trang, bản release, bộ nhớ riêng):

| Bước khởi động (`BO_READER_MEMSTEPS`) | Phần cứng | `D3D11_CREATE_DEVICE_SINGLETHREADED` | WARP (`BO_GPU_WARP=1`) |
| --- | --- | --- | --- |
| Cửa sổ | 4,2 MB | 4,1 MB | 4,0 MB |
| Sau D3D11CreateDevice | 42,7 | 43,2 | 4,9 |
| Sau thiết bị + ngữ cảnh D2D | 43,3 | 43,8 | 5,2 |
| Sau DirectWrite (FaceCache) | 43,4 | 43,9 | 5,4 |
| Sau swap chain (2 bộ đệm flip, cỡ client, không MSAA) | 43,5 | 44,0 | 5,6 |
| Cửa sổ trống đã hiện | 49,5 | 49,9 | 10,8 |

~38 MB là driver chế độ người dùng của hãng GPU nạp trong `D3D11CreateDevice` — ngoài tầm của ta; factory D2D vốn đơn
luồng, không có WIC. Thử thêm: tắt font hệ thống −1 MB; ngân sách ảnh giải mã 64 → 16 MB không đổi (ảnh của sổ tay nhỏ).

| Sổ tay 1 202 trang | Vòng 7 | Vòng 8 (phần cứng) | WARP | Bản đối chiếu |
| --- | --- | --- | --- | --- |
| Sau mở / sau End | 112 / 107 MB | 101–103 / 97–98 MB | 59–61 / 58 MB | 46 / 46 MB |
| Sau 80 nấc bánh xe + 40 PgDn, rồi 6 s | — | 158 → 146 MB; bộ đệm trang 12 MB: 145 → 132 MB | 107 MB | — |
| Độ trễ bánh xe / phím p50 (p95) | — | 1,4 (11) / 2,3 (17) ms | 1,8 (7,8) / 2,4 (16) ms | — |
| Trang ảnh, 400 %, 40 nấc: CPU / bộ nhớ riêng | — | 1 % / 246 MB | 3 % / 199 MB | — |

Giữ: cắt bộ nhớ driver 1 s sau khung cuối (−10 MB lúc mở), chỉ vẽ trước một trang theo hướng cuộn (−8 MB), bộ đệm trang
32 → 12 MB (−14 MB sau cuộn dài, độ trễ như cũ). Phần còn giữ sau cuộn: ảnh trang trên GPU của trang đang thấy + trang
kế, cộng bộ nhớ heap đã giải phóng nhưng chưa trả cho hệ điều hành. Thiết bị phần cứng vẫn là mặc định (chủ dự án duyệt);
WARP bật bằng `BO_GPU_WARP=1`.

**Mở tệp.** Chờ 15 phút một khoảng 60 s không có trình biên dịch: không có (9–12 tiến trình biên dịch của agent khác suốt
lúc đo). 5 lần `--bench` (CPU 51–71 %): mở 34–55 ms, trang đầu 44–66 ms (vòng 7: 38–48 / 50–63 ms; mục tiêu ≤ 210 ms).

Còn lại: giao diện Tối (cần màu chữ theo giao diện ở mọi ngăn), cột Lớp của bản đối chiếu (menu chuột phải không nhận
nhấp đưa vào), so Ctrl + bánh xe / tìm với tuỳ chọn trên bản đối chiếu (cần thao tác thật), bộ nhớ nền của driver GPU.

## Vòng 9 (2026-11-16)

Phần thêm ở mô-đun riêng: `reader_dark9_win.rs` (giao diện Tối, chú giải công cụ), `reader_menus9_win.rs` (menu chuột
phải trên trang), `reader_tabs9_win.rs` (tab Chia sẻ, tab Trợ giúp, cách ngắt nhãn ribbon), `reader_search9_win.rs` (Tìm
kiếm nâng cao), `reader_marks9_win.rs` (sửa dấu trang, cỡ ảnh trang); thêm chung (chỉ cộng thêm): `Painter::raw_colours`,
`Menu::with_keys` (cột phím tắt canh phải + khoảng cách đường kẻ), `PrintSettings::bleed`, `PrintAction::Summarize`.
Ảnh so sánh: `scratchpad/show/reader-52…*.png` (bản đối chiếu trái / trên).

**Giao diện Tối** (`reader-52`, `reader-53`). Mọi bề mặt (ribbon, cột trái, trang File, menu, hộp thoại, thanh trạng
thái, chú giải) vẽ qua một hàm bảng màu (`dark_ink`): độ sáng OKLab lật theo một đường cong chọn để chữ ≥ 4,5 : 1 trên
mọi nền; màu nhấn (độ bão hoà > 0,08) giữ sắc độ ở L 0,76 để vừa làm chữ trên nền tối vừa mang chữ tối; đường kẻ 1–2 px
sáng giữ ≥ 1,5 : 1 (`dark_line`). Trang, ảnh trang, xem trước khi in, mẫu giao diện vẽ giữa `raw_colours(true/false)`
nên giữ màu thật (bản đối chiếu cũng giữ trang trắng). Test đi qua mọi lệnh của mọi tab (menu / hộp thoại / ngăn mở ra),
các cột, trang File, hộp In, menu chuột phải, chú giải: 6 854 chữ, thấp nhất 4,66 : 1 (chữ của lệnh mờ được miễn theo
WCAG 1.4.3). Giao diện Tối của bản đối chiếu chỉ đặt được trong registry HKCU của nó (không có tệp hồ sơ để chép) nên
không chụp; ảnh là Cổ điển / Tối của ta.

**Các tab** (`reader-54`). Xem / Biểu mẫu / Bảo vệ / Chia sẻ / Trợ giúp đặt cạnh nhau. Sửa: nhãn hai dòng ngắt ở dấu cách
làm hai dòng cân nhất, dòng dài trước khi hoà ("Command / Line Help", "Time Stamp / Document", "Read / Mode"); Export có
mũi tên thả; Tool Settings mờ khi tài liệu không có trường biểu mẫu; nút menu của lưới hiển thị trang có mũi tên. Tab
Chia sẻ mới: E-mail, Chép đường dẫn tệp, Mở thư mục chứa (dịch vụ đám mây / duyệt của bản đối chiếu không làm). Tab Trợ
giúp theo thứ tự của bản đối chiếu: User Manual, Help Center (mờ, chú giải nói lý do: bản này chưa kèm hướng dẫn / địa
chỉ hỗ trợ), Command Line Help (hộp liệt kê tham số dòng lệnh), Set to Default PDF Reader (mở Default apps của
Windows), About. Chú giải công cụ sau 0,5 s đứng yên trên một lệnh.

**Menu chuột phải** (`reader-55`). Đo menu của bản đối chiếu ở 125 %: phím tắt canh phải cách mép 21 px, đường kẻ thêm
8 px, rộng theo hàng rộng nhất (nhãn + 40 px + phím tắt; menu vùng trống 249 px). Vùng trống / nền: Dán, Dán tại chỗ (mờ)
| Read Mode, Full Screen | Zoom In / Out | Actual Size, Fit Page, Fit Width (đánh dấu cái đang dùng) | Rotate Right |
Reverse View | Print | Rulers, Guides | Hide Navigation Panels. Chữ đang chọn: Copy | Select All | Highlight,
Strikeout, Underline, Squiggly Underline, Replace Text | Word Count. Chú thích: Reply | Delete | Set Status, Mark with
Checkmark | Open Popup Note, Reset Popup Note Location, Open All Popups, Close All Popups (`/Open` và `/Rect` của chú thích Popup, §12.5.6.14) | Properties, Set Current Properties as Default (màu của chú thích thành màu mặc định của loại đó); Set Status có mũi tên mở bốn trạng thái; chữ đang chọn thêm Look Up "<từ>" (mở trình duyệt tìm từ). Menu chuột phải hàng 28 px như bản đối chiếu (menu thả của ribbon vẫn 30 px).

**Tìm kiếm nâng cao** (`reader-56`). Ngăn bên phải (Ctrl+Shift+F hoặc nút tuỳ chọn của ô tìm): tài liệu hiện tại hoặc mọi
PDF trong một thư mục (Chọn... lấy thư mục của tệp được chọn), chỉ nguyên từ, phân biệt hoa thường, gồm dấu trang, gồm
chú thích; kết quả theo tài liệu → trang, mỗi dòng có ngữ cảnh và chữ tìm thấy tô nền; nhấp đi tới trang và chọn đúng
đoạn; đọc trang theo nhịp 45 ms mỗi tick. Ngăn của bản đối chiếu không mở được bằng nhấp đưa vào (danh sách thả của ô tìm
bỏ qua PostMessage, UIA không có mục), bố cục theo hướng dẫn sử dụng công khai của nó. Sổ tay 1 202 trang, "snapshot",
bản release, máy bận (CPU 45 %): 354 kết quả, kết quả đầu 17–55 ms, đọc hết 1,70–1,81 s (`--search`, cùng đường đọc
chữ).

**Dấu trang và ảnh trang** (`reader-57`). Dấu trang: thêm (Ctrl+B, tên là chữ đang chọn, trỏ tới trang hiện tại), đổi
tên tại chỗ (F2, Enter / Esc), xoá (Del khi cột đang nhận phím), nâng / hạ cấp, kéo thả lên một dấu trang khác để chuyển
tới trước nó, menu chuột phải; ghi `/Outlines` rồi File › Save nối thành cập nhật tăng dần (test: byte gốc còn nguyên là
phần đầu tệp, mở lại thấy dấu trang). Ảnh trang: hàng công cụ và menu chuột phải như bản đối chiếu (Enlarge / Reduce Page
Thumbnails | Print Pages, Properties — bản đối chiếu là trình đọc nên không có xoay / xoá trang), Ctrl + bánh xe trên cột
đổi cỡ (7 bậc 0,6–2,5 ×), khung đỏ phần trang đang thấy trên ảnh trang hiện tại.

**Hộp In** (`reader-58`). So lại với bản mới nhất: Scale (None / Fit to printer margins / Reduce to printer margins /
Custom scale / Choose paper source by PDF page size), Tile Large Pages, Multiple Pages Per Sheet, Booklet đã khớp. Thêm:
Bleed Marks (dấu góc ở mép mọi trang in, xem trước và in thật), Summarize Comments bật khi có chú thích (đóng hộp In, mở
tóm tắt chú thích); ô Pages điền sẵn trang hiện tại như bản đối chiếu. Các nút chế độ in của bản đối chiếu bỏ qua nhấp đưa vào nên chỉ so được chế độ Scale.

Còn lại: giao diện Tối của bản đối chiếu (registry), ngăn Tìm kiếm nâng cao của bản đối chiếu (cần thao tác thật), cửa sổ
ghi chú bật lên vẽ trên trang (mới ghi `/Open`), thanh công cụ nổi khi chọn chữ, Dán (ảnh / chữ thành chú thích), User Manual /
Help Center (chưa có hướng dẫn / địa chỉ hỗ trợ đi kèm).
## Ghi chú từ BlueEditor vòng 7 (2026-11-16)

Bộ nhớ đệm trang (`reader_view.rs`, dùng chung): một trang được làm mới liên tục (gõ Typewriter / Text Box, kéo
ghi chú, điền trường) bị đẩy vào danh sách dùng gần đây mỗi lần mà không bỏ bản cũ; quá 48 mục thì trang vừa dựng
bị bỏ ngay (“not rendered”), cửa sổ đứng hình trang cũ. Đã sửa (làm mới trang thì bỏ khỏi danh sách, mỗi trang
một mục); test `reader_window_draws_a_page_changed_sixty_times` (làm mới trang 60 lần trong cửa sổ BlueReader, lần
nào cũng vẽ) và `two_hundred_typed_characters_are_all_drawn_and_saved` (BlueEditor). `BLUEOFFICE_DRAW_LOG=<tệp>` ghi
lỗi dựng / vẽ trang.

## Vòng 10 (2026-11-16)

Phần thêm ở mô-đun riêng: `reader_popups10_win.rs` (ghi chú bật lên trên trang, thanh công cụ nổi khi chọn chữ, Dán);
lõi: `bo-pdfread/src/annot_popup.rs` (`popup_notes` / `set_popup_note` / `add_popup_note`, §12.5.6.14),
`annot_paste.rs` (`paste_annotation`), `RenderExtra::no_zoom` (cờ NoZoom §12.5.3); thêm chung (chỉ cộng thêm): menu
chú thích có Cut / Copy / Paste và Open / Close Popup Note, mặc định công cụ kiểu bản đối chiếu (`Draw::reader_defaults`),
host `host.paste10`, `BO_READER_STARTUP=<tệp>` (mốc khởi động). Ảnh so sánh: `scratchpad/show/reader-62…*.png`.

**Ghi chú bật lên** (`reader-62`). Đo ở 125 %: cửa sổ ghi chú 180 × 120 pt vẽ 300 × 200 px ở mọi zoom (cỡ `/Rect` ở
96 dpi), góc trên trái ở chỗ của `/Rect` trên trang, bị đẩy vào trong vùng xem (ghi chú Bob ở mép dưới như bản đối
chiếu). Viền 1 px 133; tiêu đề màu chú thích ngả 30 % về trắng (vàng → 255, 255, 77), vùng chữ 244, 244, 190; ghi chú
đang chọn: màu thật 255, 255, 0, viền 80, vùng chữ 240 khi đang gõ. Hai hàng 25 px (biểu tượng — chỉ ghi chú dán, ngày
"11/16/2026 1:54:32 PM", ô đóng 18 px cách viền 2 px; tác giả, Options ▾), vùng chữ từ 51 px, dải dưới 17 px có hai tay
nắm đổi cỡ, chữ Arial 17 px / 11 px (dòng 13 px). Chú thích không có `/M` hiện giờ lúc mở tài liệu (bản đối chiếu cũng
vậy). Kéo tiêu đề để dời, kéo tay nắm để đổi cỡ (ghi `/Rect`), nhấp vùng chữ để gõ (Esc / nhấp ra ngoài ghi
`/Contents`), ô đóng ghi `/Open false`; nhấp đúp chú thích mở ghi chú (thêm `/Popup` nếu chưa có); Options và menu chuột
phải trên ghi chú mở menu chú thích (Reply | Cut, Copy, Paste | Delete | Set Status, Mark | Open / Close Popup Note,
Reset, Open / Close All Popups | Properties, Set Default — như menu Options đo được). Open / Close All Popups vẽ lại ngay.

**Biểu tượng ghi chú dán trên trang** (`reader-65`). Chú thích có cờ NoZoom (PyMuPDF / bản đối chiếu ghi `/F 28`) giữ cỡ
trên màn hình: bản đối chiếu vẽ ghi chú 16 pt thành 21 × 20 px ở 137 % và 20 × 20 px ở 91 % (1 pt = 1 px giao diện);
trước đây ta phóng theo zoom (36 px, viền 1 pt thành 3 px). Nay `RenderExtra::no_zoom` = 1 / (zoom × 4/3): hình
co về góc trên trái, 20 px ở mọi zoom (test `no_zoom_notes_keep_their_screen_size`). Còn lại: vùng nhấp / khung chọn
của chú thích vẫn theo `/Rect` đầy đủ.

**Thanh công cụ nổi khi chọn chữ** (`reader-63`). Bản đối chiếu: cửa sổ 324 × 52 px, viền 199, 204, 217, nền 251, 251, 253,
8 biểu tượng 20 px cách 35 px, lề 25 px, vạch ngăn 1 px (230, 233, 240) sau biểu tượng thứ tư (thêm 8 px); góc dưới trái
ở đúng chỗ thả chuột, cao hơn 2 px (đo hai lần chọn khác nhau). Ta cùng số đo, 9 biểu tượng của ta (bỏ các mục AI):
Copy, Look Up, Word Count, More (menu chữ đang chọn) | Highlight, Strikeout, Underline, Squiggly, Replace Text (358 px);
ẩn khi nhấp chỗ khác, gõ phím, cuộn, zoom hoặc bỏ chọn.

**Dán.** Menu trang: Paste bật, Paste in Place bật khi đã chép một chú thích. Dán: hình trên bộ nhớ tạm thành dấu
(stamp) ảnh ở 96 dpi (thu vào trang), chữ thành chú thích máy chữ (FreeText) góc trên trái ở chỗ nhấp chuột phải, chú
thích vừa chép (Ctrl+C / Ctrl+X hoặc Cut / Copy của menu) thành bản sao ở chỗ nhấp; Paste in Place đặt bản sao trùng chỗ
cũ; Ctrl+V / Ctrl+Alt+V dán ở con trỏ. Không thử trên bản đối chiếu vì cần ghi đè bộ nhớ tạm của chủ máy; hành vi theo
hướng dẫn công khai của nó (test `paste_text_picture_and_comments`, `paste_at_a_point_and_in_place`).

**Công cụ tab Comment** (`reader-64`, nhấp / kéo đưa vào trên bản sao). Mặc định đo được: Drawing (hình chữ nhật) và
Pencil đỏ 255, 0, 0, nét 2 pt; Highlight 255, 255, 0; Typewriter chữ xanh 0, 0, 255 cỡ 9 pt; Note mở ngay ghi chú để gõ
(kèm ngăn Format bên phải). Ta trước: đỏ 229, 57, 53 nét 1 pt, highlight 255, 230, 0, máy chữ đỏ 12 pt → nay như bản
đối chiếu (chỉ BlueReader). Area Highlight / Measure / Stamp của bản đối chiếu không nhận kéo đưa vào nên chưa so.

**Tay nắm của chú thích đang chọn** (`reader-66`, `reader_handles10_win.rs`). Bản đối chiếu: viền chấm, tám chấm tròn
12 px (51, 142, 244, mép trắng) ở góc và giữa cạnh, tay xoay trên cuống 2 px (38, 128, 235) cách cạnh trên 28 px; đường
thẳng chỉ có hai chấm ở hai đầu. Ta vẽ như vậy (hình mũi tên xoay của ta) thay cho ô vuông cũ: kéo tay nắm đổi cỡ (Shift
giữ tỉ lệ, góc đối diện đứng yên), kéo đầu đường thẳng / đo khoảng cách, kéo tay xoay để xoay quanh tâm (Shift: bước
15°) — nét chì, đường gấp khúc, đa giác, số đo giữ loại; hình chữ nhật / elip thành đa giác (`rotate_drawing`, vì
`/Square` `/Circle` luôn đứng thẳng); hộp chữ, chú thích gọi ra, dấu không xoay (không có tay xoay). Mỗi thao tác một
bước hoàn tác, File › Save ghi lại (test `handles_resize_turn_and_move_line_ends`, `corners_keep_the_ratio_with_shift`).

**Khởi động lạnh tới trang đầu** (PDF 100 trang, `startup.py`: chạy exe, dò cửa sổ mỗi 15 ms rồi PrintWindow tới khi giữa
trang có chữ; 5–6 lần, lần đầu sau khi chép exe bị quét nên bỏ):

| | Cửa sổ hiện | Trang đầu vẽ xong | CPU cả máy lúc đo (trước khi đo) |
| --- | --- | --- | --- |
| Bản đối chiếu | 567–690 ms | 851–980 ms | 59–77 % (50–56 %) |
| BlueReader | 71–91 ms | 259–352 ms | 40–60 % (34–46 %) |

Mốc của ta (`BO_READER_STARTUP`): mở tài liệu 41–53 ms, cửa sổ 50–64 ms, tạo thiết bị D3D11 + D2D thêm 146–208 ms, swap
chain 5 ms, khung đầu 20–25 ms. Thử tạo thiết bị trước trên luồng phụ (nạp driver song song khi mở tài liệu): thiết bị
của cửa sổ vẫn mất 136 ms, trang đầu chậm hơn (435–507 ms) → bỏ.

Còn lại: ngăn Format khi gõ ghi chú, vùng nhấp của chú thích NoZoom, Area Highlight / Measure /
Stamp của bản đối chiếu (không nhận thao tác đưa vào), tạo thiết bị D3D chiếm ~60 % thời gian tới trang đầu.

## Vòng 11 (2026-11-16 – 17)

Phần thêm ở mô-đun riêng: `reader_fill11_win.rs` (+ `reader_fill11_tests.rs`: công cụ chú thích, Fill & Sign, điền
biểu mẫu, Read Aloud, tên cho công nghệ hỗ trợ), lõi `bo-pdfread/src/forms11.rs` (`add_signature_stamp`,
`flatten_fill_sign`, `/BOFillSign`, thứ tự Tab, `AFSimple_Calculate`, `SignatureInk::Shape`, `typed_outline_in`);
thêm chung (chỉ cộng thêm): `RenderExtra::field_tint`, nút thả xuống của hộp combo khi tô trường, ZapfDingbats /
Symbol không nhúng giữ bảng mã có sẵn kể cả khi từ điển font ghi `/Encoding` tên (§9.6.6.1). Ảnh so sánh:
`scratchpad/show/reader-67…73-*.png` (bản đối chiếu trên).

**Công cụ chú thích** (đo trên bản đối chiếu). Hình vừa vẽ được chọn ngay; Rectangle nét 2 pt; ghi chú NoZoom nhận
nhấp và khung chọn theo biểu tượng vẽ ra (khung chấm, không tay nắm). Bản đối chiếu không mở ngăn Format khi gõ ghi chú
mới nên ta không thêm.

**Tab Fill & Sign** (`reader-67`, sau góp ý `reader-69`). Như bản đối chiếu: Hand, Select | thư viện chữ ký 506 px (ô 145 × 68 px,
"+" tạo chữ ký / chữ ký tắt), Manage Signatures (không mũi tên), Keep Tool Selected (xám tới khi chọn một công cụ),
Apply All Signatures (xám tới khi đặt chữ ký; áp các mục Fill & Sign vào trang, một bước hoàn tác) | Add / Text (gõ tại
chỗ, khung 1 px 31, 123, 204), Comb Field (mỗi ký tự một ô 14 pt), Predefined Text (tên, ngày M/D/YYYY) | lưới dấu
3 × 2 (check, cross, chấm tròn 14 px; line, box; bước 44 px như bản đối chiếu 44–45 px) | Close. Dấu đặt cỡ 18 pt,
chọn bằng một tay nắm góc giữ tỉ lệ. Sửa lỗi: thư viện chữ ký nới rộng làm dời cả nút của thanh trạng thái (mũi tên
trang, zoom, kiểu trang biến mất ở chế độ Fill & Sign) — nay chỉ các lệnh ribbon sau nó dời (test
`fill_and_sign_keeps_the_status_bar_in_place`).

**Create Signature** (`reader-68`, sau góp ý `reader-70`). Hộp 1 086 × 604 px như bản đối chiếu: Type / Draw / Import / Paste
(Online bỏ), ô chữ ký, danh sách font, chấm màu (đen, xanh, đỏ), xem trước lấp chiều ngang, Options dưới trái, OK /
Cancel. Font gõ: font viết tay của hệ thống đứng đầu (Segoe Script mặc định, Ink Free, Segoe Print, Lucida Handwriting
nếu có — host chỉ đọc khi hộp mở lần đầu), sau đó bốn font nghiêng đóng gói; repo không có font script OFL nào. Chữ ký
gõ bằng font hệ thống lưu thành đường viền tô (`SignatureInk::Shape`, hộp đơn vị + tỉ lệ) nên tệp PDF không nhúng
chương trình font của bên thứ ba. Paste lấy ảnh từ bộ nhớ tạm (`host.sigpaste11`). Options (hộp 502 × 438 px như bản
đối chiếu): đổi sang đen trắng và giữ cỡ gốc (96 dpi) cho ảnh, bỏ chữ ký khi đóng chương trình (không ghi
`reader-signatures.txt`), mật khẩu: xám (chưa làm). Chữ ký lưu cho lần sau, đặt bằng một nhấp, đổi cỡ bằng góc, làm
phẳng khi lưu nếu chọn (Manage Signatures › Flatten on Save).

**Điền biểu mẫu** (`reader-71`, `reader-72`, `reader-73`). Trường tô 221, 228, 255 dưới hình hiện (Highlight Fields bật mặc định
như bản đối chiếu, Form › Highlight Fields tắt / bật), gõ tại chỗ với khung chấm 1 px ngoài trường, Tab / Shift+Tab theo
`/Tabs` của trang (một điểm dừng cho mỗi nhóm radio), Space cho hộp kiểm / radio, hàng của hộp danh sách, Alt+↓ / nhấp
mở combo, Reset Form, trường tính từ `/CO` với tập con `AFSimple_Calculate` (SUM, PRD, AVG, MIN, MAX; không chạy
script; 5 / 3 → 1.666667 như bản đối chiếu), một bước hoàn tác cho giá trị và phép tính kéo theo, lưu giá trị. Mới ở
vòng này: hộp combo có nút thả xuống trên màn hình (đo trên trường 20 pt: rộng 9,3 pt, khung đen, mặt 217, 192, 219,
mũi tên giữa; không in); dấu kiểm của PyMuPDF (`/ZapfDingbats` + `/WinAnsiEncoding`, mã "3") trước hiện thành chữ số
"3", nay là ✓ như bản đối chiếu (test `zapf_dingbats_ignores_a_named_base_encoding`). Vào trường chữ bằng Tab chọn cả giá trị
(gõ là thay), nhấp chuột đặt con trỏ — như bản đối chiếu (test `tab_into_a_filled_field_selects_its_value`). Nút
radio của PyMuPDF ghi `/FT null /T null` trên widget: trước bị coi là trường riêng không kiểu nên nhấp không được và
Tab nhảy về trường đầu; nay null = không có khoá (§7.3.9), cả nhóm là một điểm dừng Tab (test
`null_field_keys_are_inherited_from_the_parent`).

**Trợ năng.** Read Aloud tô từ đang đọc (trạng thái SAPI đọc theo nhịp), Reflow đọc bằng Space, ←/→, Home/End; mọi lệnh
ribbon có tên cho công nghệ hỗ trợ (`accessible_names11`, tooltip cho lệnh chỉ có biểu tượng). UIA thật cần
`IRawElementProviderSimple` (COM, `unsafe`) trong host Win32 — để vòng sau; hiện tên đọc được qua tooltip và tiêu đề.
Kiểm độ tương phản (WCAG 2.2 SC 1.4.3) nay chạy trên cả bốn skin (Classic, Light, Blue, Dark) cho mọi bề mặt (mỗi tab,
mỗi lệnh, panel, trang File, hộp thoại, menu, tooltip; ~6 900 chữ mỗi skin): xám 150, 150, 151 đo từ bản đối chiếu cho
chữ gợi ý ("Search", "No bookmarks") và nhãn Properties chỉ đạt 2,96 : 1 → 112, 112, 113 (4,8 : 1 trên nền 251). Không
bật chế độ High Contrast của Windows để thử vì đổi cả máy của chủ dự án.

**Tốc độ** (sổ tay 1 202 trang, bản release sau khi gộp mọi nhánh; máy bận: 7–8 tiến trình `rustc` của agent khác,
CPU 72–81 %):

| | Vòng 11 | Trước (vòng 8–10) |
| --- | --- | --- |
| Mở / trang đầu (`--bench`, 5 lần) | 33–69 / 44–84 ms | 34–55 / 44–66 ms |
| Tìm "snapshot": kết quả đầu / hết 1 202 trang | 17–27 ms / 1,80–2,17 s (354 kết quả) | 17–55 ms / 1,70–1,81 s |
| Bánh xe 80 nấc (đầu vào → khung) | p50 2,1–2,2 ms, p95 8,1–8,6 ms | p50 1,4 ms, p95 11 ms |
| Bộ nhớ riêng sau mở / sau 80 nấc | 117 / 115–117 MB | 101–103 / — MB |
| Khởi động lạnh (PDF 100 trang): cửa sổ / trang đầu | 104–124 / 368–435 ms | 71–91 / 259–352 ms |

Các số chậm hơn đi cùng CPU cả máy cao hơn (72–81 % so với 40–60 %); không có khoảng máy rảnh để đo lại.

Còn lại: UIA thật (COM provider), mật khẩu cho chữ ký, trang Online của Create Signature, đo lại tốc độ khi máy rảnh;
chữ của hình hiện trường ta sinh lùi 2 pt từ mép trái (quy ước chung, dùng cả ở BlueEditor; bản đối chiếu 0 pt), dấu ✓
ZapfDingbats của bản đối chiếu đậm hơn glyph thay thế của ta.

## Vòng 12 (2026-11-17)

Phần thêm ở mô-đun riêng: `reader_r12_win.rs` (+ `reader_r12_tests.rs`: số đo ribbon, menu Select / Assistant, cuối tab
Comment / Share, mật khẩu chữ ký, danh sách phần tử cho công nghệ hỗ trợ), `reader_prefs12_win.rs` (hộp Preferences),
host `reader_uia.rs` (UI Automation, mô-đun con của `reader_app` có `unsafe` riêng — `win32.rs` không đổi); thêm chung
(chỉ cộng thêm): `PdfWindow::accessibility` / `invoke` (mặc định rỗng, BlueEditor không đổi), `bo_ui::{AccessItem,
AccessRole}`, feature `Win32_UI_Accessibility` / `Win32_System_Ole` / `Win32_System_Variant` và `windows-core` (đã có
trong cây) cho `#[implement]`. Ảnh so sánh: `scratchpad/show/reader-74…83-*.png` (bản đối chiếu trên / trái).

**Phủ so sánh** (mọi tab / trang File / hộp thoại của bản đối chiếu; cột "Vòng" = vòng đã so có ảnh):

| Phần của bản đối chiếu | Vòng | Ghi chú |
| --- | --- | --- |
| Thanh tiêu đề, truy cập nhanh, ô tìm | 1, 7, 8 | |
| Tab Home | 3, 5, 12 | vòng 12: số đo nút lớn, Select ▾ |
| Tab Comment | 3, 4, 5, 10, 11, 12 | vòng 12: Summarize / Import / Export thành nút lớn, cột FDF via Email / Manage Comments / Keep Tool Selected |
| Tab View | 3, 4, 9, 12 | vòng 12: Assistant ▾ (Marquee; Loupe / Magnifier mờ) |
| Tab Form | 4, 9, 11, 12 | vòng 12: Form to sheet ▾ |
| Tab Protect | 4, 5, 9, 12 | |
| Tab eSign (dịch vụ tài khoản của hãng) | — | không làm; chữ ký cục bộ ở Fill & Sign |
| Tab Share | 9, 12 | vòng 12: Shared Files, Manage Shared / Email Review, Tracker mờ (cần dịch vụ trực tuyến); không dùng tên dịch vụ của hãng khác |
| Tab Help | 9 | Support Center / Plug-Ins / Register là dịch vụ của hãng: không làm |
| Tab AI Assistant | — | không làm |
| Tab ngữ cảnh Fill & Sign | 5, 11 | |
| Trang File (Open … Skins, Preferences) | 5, 6, 8, 12 | vòng 12: thứ tự / đường kẻ như bản đối chiếu (Share mở khối thứ hai, Skins trước Preferences) |
| Hộp Print + Page Setting / Advanced / Tile / Multiple Pages / Booklet | 2, 3, 4, 9, 12 | vòng 12: các chế độ của bản đối chiếu mở được bằng `BM_CLICK` (thông điệp, không phải thao tác thật) |
| Hộp Preferences | 6, 12 | hộp của bản đối chiếu không mở được (xem dưới) |
| Document Properties | 5 | |
| Create Signature / Options | 11, 12 | vòng 12: mật khẩu |
| Menu chuột phải, menu thả ribbon | 6, 9, 10, 12 | |
| Cột trái (Bookmarks / Pages / Comments / Attachments / Layers) | 5, 7, 8, 9 | |
| Full Screen, Read Mode | 2, 8 | |
| Ghi chú bật lên, thanh công cụ khi chọn chữ | 10 | |
| Tìm kiếm nâng cao | 9 | ngăn của bản đối chiếu không mở được bằng nhấp đưa vào |
| Giao diện Tối | 9 | bản đối chiếu chỉ đặt được trong registry |

**Ribbon** (`reader-74`, `reader-75`, sau góp ý `reader-79`, `reader-81`). Đo ở 125 %: biểu tượng lớn của bản đối chiếu
cao 31 px từ y 112, nhãn từ y 154 (của ta 146: thấp hơn 8 px), dòng hai cách 19 px (của ta 16), nút = nhãn + 10 px, cách
2 px (Hand 52 px), lưới nút nhỏ 44 px mỗi cột. Trước đây các nút dồn sang phải dần (Fill & Sign 695 so với 666). Nay
(`Look::READER`: icon 28 / y 4, nhãn y 35,2, đệm 8; `big_min12`, `label_pitch12`, `grid12`): tâm nhãn tab Home lệch
0–2,5 px tới nút cuối (Zoom 290 / 290, Typewriter 528 / 529,5, Fill & Sign 661,5 / 659); tab Comment Note 313,5 / 310,
nút cuối 1 274 / 1 257 (nhãn của ta rộng hơn 1–3 px mỗi nhãn); tab View lệch về trái 5–15 px. Select có ▾: phần biểu
tượng chọn công cụ, phần nhãn mở Select Text and Image / Select Annotation (Select Annotation: nhấp ngoài chú thích không
chọn chữ). Assistant ▾: Marquee (kéo khung → zoom vừa khung, nhấp → một bậc zoom; Esc / công cụ khác thoát), Loupe /
Magnifier mờ. Cuối tab Comment như bản đối chiếu ở 1 518 px: Summarize Comments, Import, Export ▾ thành nút lớn, rồi cột
FDF via Email (lưu FDF rồi mở thư mới để đính kèm), Manage Comments ▾, Keep Tool Selected. Menu Select / Assistant:
`reader-76` (bản đối chiếu có biểu tượng trong menu, của ta chưa).

**Preferences** (`reader-77`, sau góp ý `reader-82`). Hộp Preferences của bản đối chiếu không mở được không cần thao
tác thật: nhấp đưa vào trang File bị bỏ qua, các mục của trang File không có trong cây UI Automation (chỉ các pane),
Ctrl+K cần trạng thái phím thật. Hộp của ta dựng theo bố cục của nó (hướng dẫn công khai): hộp lớn 950 × 700 px có thanh
tiêu đề và nút đóng, danh sách 26 mục bên trái theo thứ tự chữ cái của bản đối chiếu, bên phải các nhóm có khung. Tuỳ
chọn ta làm thật và lưu (`reader-prefs.txt`): Forms › Highlight all fields; Full Screen › Escape key exits, nhấp trái /
phải lật trang; General › Smooth scrolling; History › số tài liệu gần đây (5–30), Clear History; Identity › tác giả;
Measure › đơn vị; Page Display › bố cục, zoom mặc định; Search › nguyên từ, phân biệt hoa thường (ô tìm bắt đầu với
chúng); Speech › tốc độ đọc; File Associations › Set to Default PDF Reader. Các tuỳ chọn khác của bản đối chiếu hiện mờ
(xám của lệnh mờ, được miễn WCAG 1.4.3) nên không trang nào trống. Test `round12_preferences_change_the_window`.

**Hộp In** (`reader-83`, đối chiếu trên bản sao thử, chỉ Cancel / WM_CLOSE, không in). Khác bản mới nhất của bản đối
chiếu, đã sửa: Pages điền "1 - 5" (trước: trang hiện tại), Reverse pages bật cả khi All pages, Multiple Pages Per Sheet
mặc định 2 × 2, Bleed Marks chỉ bật với Scale, Print on both sides mờ khi Tile / Booklet. Còn khác: ô Margins (inch) của
N-up / Booklet, hàng × cột tuỳ ý ("Custom…"), Booklet đặt giấy ngang trong xem trước, thứ tự Binding / Booklet subset.

**Mật khẩu chữ ký** (`reader-78`). Options của Create Signature như bản đối chiếu: Require password, Password, Confirm
Password (ô mờ tới khi tick; chấm thay chữ; Tab chuyển ô; OK từ chối khi hai ô khác nhau hoặc rỗng). Chỉ lưu
`salt$hash`: SHA-256 lặp 10 000 lần với salt 16 byte mới mỗi lần (`hash_password12`), so khớp không thoát sớm; dòng
`reader-signatures.txt` ghi `flags;pw=…` (tệp cũ vẫn đọc được). Đặt chữ ký có mật khẩu: hỏi mật khẩu trước, sai thì xoá
ô và báo, Esc huỷ. Test `signature_passwords_are_kept_as_salted_hashes`, `a_protected_signature_asks_for_its_password`.

**UI Automation** (`reader-80`). Host bắt `WM_GETOBJECT` bằng subclass cửa sổ trong `reader_uia.rs`: gốc là fragment
root trên provider của HWND, con là tab ribbon (TabItem), mọi lệnh ribbon / thanh tiêu đề / cột trái / thanh trạng thái
(Button: tên, ô, bật / mờ) và vùng trang (Document "tệp, page n of m"); Invoke chạy lệnh trên luồng giao diện (xếp hàng +
một tick). Danh sách chỉ được cập nhật khi đã có client. Kiểm bằng TreeWalker của UIA: 45 phần tử với tên / ô đúng;
Invoke `tab:3` mở tab View, Invoke `cmd:view.night` bật Night Mode (không thao tác thật). `BO_UIA_LOG=<tệp>` ghi các yêu
cầu. Còn thiếu: focus bàn phím (`HasKeyboardFocus`, sự kiện), mẫu Text cho chữ trang, menu / hộp thoại tự vẽ.

**Tốc độ** (bản release cuối, sổ tay 1 202 trang; máy gần rảnh: CPU 21 % trước khi đo, 2–3 tiến trình `rustc` của agent
khác vẫn chạy):

| | Vòng 12 | Vòng 11 (CPU 72–81 %) |
| --- | --- | --- |
| Mở / trang đầu (`--bench`, 5 lần) | 30–39 / 39–52 ms | 33–69 / 44–84 ms |
| Tìm "snapshot": kết quả đầu / hết 1 202 trang | 15,7 ms / 1,56 s (354 kết quả) | 17–27 ms / 1,80–2,17 s |
| Bánh xe 80 nấc (đầu vào → khung) | p50 1,8 ms, p95 5,6 ms | p50 2,1–2,2 ms, p95 8,1–8,6 ms |
| Bộ nhớ riêng sau 80 nấc | 116 MB | 115–117 MB |
| Khởi động lạnh (PDF 100 trang): cửa sổ / trang đầu | 70–105 / 275–339 ms (CPU 37–65 %) | 104–124 / 368–435 ms |

Còn lại: hộp Preferences thật của bản đối chiếu (không mở được), Loupe / Magnifier, biểu tượng trong menu Select /
Assistant, ô Margins và "Custom…" của N-up, giấy ngang khi Booklet, tab View lệch 5–15 px và nhãn của ta rộng hơn 1–3 px,
UIA: focus / mẫu Text / menu và hộp thoại tự vẽ.

## Vòng 13 (2026-11-17)

Mục tiêu chính: độ trung thực khi vẽ trang. Bộ thử 32 tệp PDF tự sinh (`scratchpad/r13/mkcorpus.py`: font chuẩn 14 không
nhúng, trạng thái chữ Tc/Tw/Tz/Ts/Tr, TrueType nhúng + tiếng Việt, CID CJK không nhúng, CJK dọc (UniGB-UCS2-V), Type 3,
CFF nhúng, shading kiểu 1–7, tiling / shading pattern, soft mask, nhóm trong suốt (isolated / knockout), 16 blend mode,
ảnh JPX / CCITT G4 / JBIG2 (MMR) / DCT RGB-xám-CMYK / 16 bpc / stencil / colour key, Indexed / Lab / ICC / CalRGB /
CalGray / Separation / DeviceN, chú thích có appearance, trang xoay 90 / 270 + CropBox, trang 200 × 100 inch, trang danh
thiếp, nét / nối / đứt / even-odd / clip, optional content, tài liệu HTML tiếng Việt, form XObject lồng + ảnh inline). Mỗi
tệp có `/OpenAction /Fit`; bản đối chiếu mở từng tệp trong phiên riêng của ta (không có phiên nào khác đang chạy), cửa sổ
1500 × 1102, PrintWindow, cắt trang theo khung 1 px; BlueReader vẽ trang đầu bằng `--render` (cùng Direct2D của ứng dụng)
ở đúng mức zoom đo được; điểm = % điểm ảnh lệch > 48 (sau làm mờ 1 px) và lệch trung bình (`score.py`).

| Tệp | Điểm ảnh lệch % trước | sau | Lệch TB trước | sau |
| --- | ---: | ---: | ---: | ---: |
| 01-std14 | 1.40 | 1.40 | 2.53 | 2.53 |
| 02-text-state | 0.11 | 0.11 | 0.58 | 0.58 |
| 03-truetype-vietnamese | 0.33 | 0.33 | 1.56 | 1.56 |
| 04-cjk-cid | 2.56 | 0.61 | 3.26 | 1.07 |
| 05-vertical-cjk | 2.03 | 0.03 | 2.17 | 0.39 |
| 06-type3 | 0.46 | 0.00 | 0.73 | 0.10 |
| 07-embedded-cff | 0.20 | 0.20 | 1.08 | 1.08 |
| 08-shading1-function | 0.42 | 0.42 | 1.52 | 1.52 |
| 09-shading2-axial | 12.66 | 0.38 | 22.50 | 1.45 |
| 10-shading3-radial | 0.01 | 0.01 | 1.07 | 1.07 |
| 11-shading4-free-triangles | 0.17 | 0.17 | 1.00 | 1.00 |
| 12-shading5-lattice | 0.06 | 0.06 | 1.08 | 1.08 |
| 13-shading6-coons | 0.00 | 0.00 | 0.78 | 0.78 |
| 14-shading7-tensor | 0.00 | 0.00 | 0.74 | 0.74 |
| 15-patterns | 27.10 | 10.84 | 34.70 | 14.31 |
| 16-softmask-alpha | 0.59 | 0.33 | 1.17 | 0.76 |
| 17-transparency-groups | 8.51 | 2.95 | 11.37 | 1.70 |
| 18-blend-modes | 0.47 | 0.47 | 2.68 | 2.68 |
| 19-image-jpx | 0.14 | 0.14 | 0.38 | 0.38 |
| 20-image-ccitt | 0.01 | 0.01 | 0.58 | 0.58 |
| 21-image-jbig2 | 0.03 | 0.03 | 0.66 | 0.66 |
| 22-image-types | 3.74 | 0.40 | 5.58 | 0.95 |
| 23-colour-spaces | 0.76 | 0.76 | 2.03 | 1.55 |
| 24-annotations | 0.39 | 0.39 | 1.35 | 1.35 |
| 25-rotate-90 | 0.05 | 0.05 | 0.20 | 0.20 |
| 26-rotate-270-cropbox | 0.01 | 0.01 | 0.18 | 0.18 |
| 27-huge-page | 0.30 | 0.30 | 1.19 | 1.19 |
| 28-small-page | 0.42 | 0.42 | 1.25 | 1.25 |
| 29-strokes-clip | 1.78 | 0.04 | 2.91 | 0.61 |
| 30-optional-content | 0.00 | 0.00 | 0.18 | 0.18 |
| 31-html-document | 0.03 | 0.03 | 0.68 | 0.68 |
| 32-forms-inline-images | 0.77 | 0.77 | 0.87 | 0.87 |
| **Trung bình** | 2.05 | 0.68 | | |

Ảnh: `reader-84-render-worst-before.png` (6 tệp tệ nhất trước), `reader-85`, `reader-86`, `reader-88-render-final.png`
(sau), `reader-87-loupe-magnifier.png`, `reader-89-annot-interop.png` (bản đối chiếu trái / trên).

Đã sửa trong `bo-pdfread` (test `render13_tests.rs`, `evenodd.rs`, `stroke_outline.rs`, `openaction.rs`):
- Tiling pattern không màu (`/PaintType 2`): ô vẽ bằng màu `scn` trong không gian nền; trước đây ô tự tô lại bằng chính
  pattern, đệ quy tới giới hạn và **bỏ cả phần còn lại của trang**. Ô pattern có màu bắt đầu từ đen.
- Shading trục theo `/Extend` (trước: luôn kéo dài cả hai đầu).
- JPEG xám giải mã một kênh (trước: đọc như RGB → ba bản thu nhỏ + đen).
- Font CJK không nhúng: CMap Unicode định sẵn (`Uni*-UCS2-*`, `Uni*-UTF16-*`) cho mã = ký tự (đọc, tìm, chép được), độ
  rộng ASCII theo CID 1–95; mặt chữ cài sẵn theo Ordering / tên (SimSun, JhengHei, MS Gothic / Mincho, Malgun Gothic…), độ
  đậm theo `/FontWeight`, `/StemV` và quy tắc đo trên bản đối chiếu (CNS1 / Japan1 / Korea1 không có cờ Serif và chưa cài
  → đậm; font đã cài giữ độ đậm của nó). Chữ dọc dùng dạng trình bày dọc của dấu câu (。：「 …).
- Alpha của nhóm trong suốt áp cho cả nhóm, nội dung bắt đầu ở alpha 1; nhóm knockout bỏ blend giữa các đối tượng; nhóm
  isolated có blend bên trong được đánh dấu (`PushGroup.isolated`, trường serde mới, mặc định false) và Direct2D vẽ trong
  layer riêng (không trộn với nền trang).
- Tô / clip even-odd (`f*` `B*` `b*` `W*`): đường con lồng nhau đổi chiều theo độ sâu; đường tự cắt (ngôi sao một nét)
  thay bằng biên của vùng even-odd (cắt cạnh tại giao điểm, giữ cạnh có vùng ở một phía). Có hiệu lực cho cả `--render`
  lẫn ứng dụng (cùng display list).
- CalGray / CalRGB theo gamma + ma trận → sRGB; tài nguyên ColorSpace tên `/G` `/RGB` `/CMYK` không bị nhầm là viết tắt.
- Chữ tô bằng pattern (shading / tiling) vẽ đường viền glyph tô pattern (trước: đen); nét vẽ bằng pattern tô vùng nét (độ
  rộng, đầu, nối, đứt) bằng ô pattern (trước: xám).
- `/OpenAction` (Fit / FitH / XYZ zoom, trang) được theo khi mở tệp như bản đối chiếu.
- Chú thích ta lưu mở trong bản đối chiếu: 15 chú thích đủ tác giả, nội dung, ngày, trả lời trên ghi chú. Sửa: squiggly vẽ
  sóng trong appearance (trước: đường thẳng, bản đối chiếu hiện như gạch dưới), chữ của stamp nằm trong khung (trước:
  đường chân chữ đặt ở đáy chữ hoa, chữ tràn ra ngoài).

Còn khác (có lý do): 15-patterns 10,8 % là pha ô pattern (của ta khớp MuPDF, bản đối chiếu neo khác); 17 nhóm isolated
không knockout: các đối tượng trong nhóm chồng nhau theo Normal (bản đối chiếu Multiply) — cần đọc backdrop trong layer;
01 ZapfDingbats: bảng mã đúng §D.5 (kiểm bằng `--text`), khác hình do font thay thế (Segoe UI Symbol); 23 ảnh Indexed
2-bit lệch nhỏ.

**Loupe / Magnifier** (View ▸ Assistant, `reader_r13_win.rs`, test `reader_r13_tests.rs`). Bản đối chiếu mở trong phiên
riêng bằng phím gửi vào menu bật lên (không thao tác thật). Loupe Tool: khung 518 × 522 px góc trái vùng trang, thanh
tiêu đề + đóng, vùng xem phóng trang quanh chỗ nhấp (mặc định 200 %, 100–1 600 %), ô zoom (menu các mức), −, thanh
trượt, +, Lock (khoá chỗ); vùng đang xem đóng khung trên trang; kéo được khung. Magnifier: kính tròn 280 px theo con trỏ,
phóng 2,25 × zoom xem, tay cầm tự vẽ (không chép hình của hãng). Cả hai vẽ từ ảnh trang render ở đúng tỉ lệ của chúng (ô
ảnh của xem trước in) nên chữ nét.

**Chiều ngược (bản đối chiếu → ta)**: bản đối chiếu không tạo chú thích từ thao tác kéo gửi bằng thông điệp (cần thao tác
thật), nên phần này dựa vào tệp của trình ghi khác (tệp 24, 0,39 %).

Còn lại: ribbon tab View lệch 5–15 px và nhãn rộng hơn 1–3 px (đo lại: lệch dồn từ Hand: Read Mode −5, Rotate −10, View
Setting −14 px), biểu tượng trong menu Select / Assistant, phần in (Margins / "Custom…" của N-up, giấy ngang khi Booklet,
viền trang khi in), nhóm isolated có blend đầy đủ.

## Vòng 14 (2026-11-17)

Phần thêm ở mô-đun riêng `reader_r14_win.rs` (+ `reader_r14_tests.rs`); `bo-pdfread/src/repair.rs` (dựng lại bảng
xref). Sửa chung chỉ cộng thêm: `ReaderView::stored_keys`, `Menu::has_icons`, `PdfDocument::annot_subject`. Ảnh:
`scratchpad/show/reader-90…9x-*.png` (bản đối chiếu trên / trái).

**Ribbon tab View** (`reader-90`). Đo lại 1:1 (tâm nhãn): Select rộng thêm 4 px, Reflow 4, Word Count / View Setting 5,
khoảng trước Read Mode +2, nhóm lưới +7,5, trước Assistant −7,5 (chỉ tab View; Home giữ số đo vòng 12). Nay mọi nhãn
lệch ≤ 1,5 px (AutoScroll 673,5 / 673, View Setting 951 / 949,5). **Trạng thái mờ** (đo trên bản sao 1 trang và 12
trang): tài liệu một trang mờ Reverse View, Continuous, Two Page, Continuous Two Page; Single, trang bìa riêng và Split
vẫn bật — làm như vậy (test `one_page_documents_grey_the_multi_page_layouts`).

**Menu Select / Assistant** (`reader-91`, chụp 1:1). Hàng 30 px, nhãn cách khung 41–42 px, độ rộng nhãn trùng (Marquee
51 px) — menu của bản đối chiếu không lớn hơn; khác duy nhất là biểu tượng (mực 20 px từ 10 px; của ta trước 17 px).
Biểu tượng tự vẽ (I-beam / ghi chú gấp góc + con trỏ, khung đứt + kính lúp, cửa sổ nhỏ có kính lúp, kính lúp có dấu +)
ở ô 24 px.

**Hộp In** (`reader-92`, `reader-93`; chỉ mở / Cancel ở bản đối chiếu, không in). Multiple Pages Per Sheet: mục
"Custom..." cuối danh sách, hiện ô cột × hàng có nút tăng giảm; Margins (ô đánh dấu, mặc định bật, 0.200 inch) là
khoảng giữa các trang (tắt: các trang sát nhau); Print Page Border. Booklet: thứ tự Binding, Booklet subset ("Both"),
Margins (lề ngoài mỗi nửa, hai trang vẫn gặp nhau ở nếp gấp); xem trước trên tờ ngang (Paper 11.7 × 8.3 inch), trang
đứng, và lệnh in đặt khổ ngang; "Print on both sides" đánh dấu và mờ; Collate một bản: đánh dấu và mờ (xám 195, dấu
trắng); bộ đếm "Page n of m" cho mọi chế độ. Test `custom_grid_margins_and_landscape_booklet` (bố cục + hộp).

**Bộ PDF "đời thực"** (`scratchpad/r14/mkcorpus14.py`, 15 tệp tự sinh; so trang đầu như vòng 13, `score.py` nay cắt
trang đầu trong chế độ cuộn liên tục và bỏ thanh cuộn). Mở = `--bench` (bản release, máy còn biên dịch song song).

| Tệp | Kiểu | Lệch % trước | sau | Mở / trang đầu (ms) |
| --- | --- | ---: | ---: | --- |
| 33-boexport-tagged | bo-pdf (Word-like, tagged, bookmarks) | 0.61 | 0.66 | 29 / 52 |
| 34-story-html-export | PyMuPDF Story (HTML → PDF) | 0.14 | 0.14 | |
| 35-reportlab-invoice | reportlab (bảng, ảnh) | 0.22 | 0.22 | |
| 36-form-heavy | 60 trường có appearance | 0.75 | 0.75 | 30 / 52 |
| 37-scanned-ocr-layer | JPEG quét + chữ ẩn Tr 3 | 0.00 | 0.00 | 24 / 75 |
| 38-linearized | /Linearized đầu tệp | 1.31 | 1.31 | |
| 39-incremental-xrefstm | 3 bản cập nhật, xref stream | 1.46 | 1.46 | |
| 40-object-streams | object stream + xref nén | 1.32 | 1.32 | |
| 41-broken-xref | offset sai, startxref sai | **không mở được** | 1.31 | 25 / 36 |
| 42 / 43 / 44 encrypt | RC4-128 / AES-128 / AES-256, mật khẩu người dùng rỗng | 1.35 / 0.42 / 1.35 | = | 28 / 46 (AES-256) |
| 45-pages-5000 | 5 000 trang nhỏ | 0.43 | 0.43 | 72 / 82 |
| 46-huge-page-200in | trang 200 × 200 inch | 0.77 (lệch TB 5.09) | 0.75 (1.26) | |
| 47-annots-1000 | 1 000 chú thích (300 ở trang 1) | 3.84 | 3.84 | 26 / 70 |

Đã sửa: (1) **dựng lại bảng xref** khi offset / `startxref` hỏng (`repair.rs`: quét `N G obj` đầu dòng, bản sau thắng,
lấy `/Root /Info /Encrypt /ID` từ trailer cuối hoặc từ xref stream, không có thì đối tượng `/Type /Catalog`) — trước
đó BlueReader báo lỗi, bản đối chiếu mở được; (2) nét rất mảnh ở mức zoom rất nhỏ (trang 200 inch vừa cửa sổ, 3,3 %)
vẽ 1 px với độ phủ thật như bản đối chiếu (lưới 4 pt nhạt, trước đây đậm); ở 60 % nét 0,25 pt vẫn đậm như bản đối chiếu
(kiểm lại bộ 32 tệp vòng 13: trung bình 0,677 → 0,677). Còn khác: 47 (biểu tượng ghi chú của ta là hình riêng), 38–44
1,3 % là chữ Helvetica thay thế, 36 khi `--render` không có tô trường (ứng dụng có).

**Chú thích do bản đối chiếu tạo → BlueReader** (`reader-94`). Trên bản sao thử, phiên riêng của bản đối chiếu nhận
nhấp và `WM_CHAR` gửi bằng thông điệp (không thao tác thật): Note (gõ nội dung vào ghi chú bật lên), Typewriter,
Highlight / Squiggly / Underline / StrikeOut (nhấp đúp chọn từ rồi nút đánh dấu), Replace (StrikeOut + Caret nhóm
`/IRT`), Insert Text (Caret), lưu bằng nút Save. Mở trong BlueReader: cả 9 chú thích hiện đúng chỗ, màu, ngày, tác
giả. Sửa theo so sánh: tiêu đề ghi chú bật lên hiện chủ đề `/Subj` ở mép trái ("Insert Text", "Note"; ngày bên phải
trước ×); ghi chú không đang dùng trong suốt 77 % như bản đối chiếu (đo trên ô đầu bảng tối: 189, 96, 172 / trên
trắng 241, 141, 211); ghi chú `/F NoZoom` giữ 1 pt mỗi pixel giao diện (ghi chú 20 pt: 25 px ở 125 %, bóng vàng 21 px như bản đối chiếu)
— trước đây danh sách vẽ của trang làm ở mức zoom vừa trang của cửa sổ đầu tiên (nhỏ hơn) không được làm lại khi zoom vừa
trang đổi, nên ghi chú to gấp 1,4 lần; nay zoom vừa trang đổi thì dựng lại danh sách như `set_zoom` (cả nét mảnh). Trả lời / trạng thái qua menu chuột phải của bản đối
chiếu không làm được (menu bật lên là cửa sổ riêng, phím gửi vào không chọn mục).

**Bộ nhớ** (sổ tay 1 202 trang, bản release, Page Down gửi bằng thông điệp tới trang cuối, đóng thẻ bằng UI Automation
`doc:close` mới):

| | Bộ nhớ riêng (MB) |
| --- | ---: |
| Mở | 98 |
| Lật hết 1 202 trang (mẫu mỗi 400 lần) | 188 – 221 (có giới hạn) |
| Đóng thẻ | 90 |
| 10 thẻ mở / sau 300 Page Down / đóng hết | 300 / 406 / 101 |

Lỗi rò đã sửa: đóng thẻ không trả ảnh trang GPU của thẻ (`PdfStore` giữ khoá cho tới khi thoát); nay
`close_tab_now` ghi lại khoá (`ReaderView::stored_keys`) và `prepare` xoá chúng (test
`closing_a_tab_frees_its_page_images`).

Còn lại: nhóm isolated có blend đầy đủ (vòng 13), trả lời / trạng thái tạo trong bản đối chiếu, UIA: focus / mẫu Text /
menu và hộp thoại tự vẽ, `--bench` không đo được trang 200 inch ở 100 %.

## Vòng 15 (2026-11-22)

Bàn phím đo trên bản đối chiếu (cửa sổ 1500 × 1102 ở 125 %, phím gửi bằng thông điệp `WM_KEYDOWN` vào
`FoxitDocWnd` của bản sao thử, trạng thái đọc từ ô zoom / ô trang / thanh cuộn; không thao tác thật). Cửa sổ của ta
không có khung hệ thống nhưng có viền thay đổi kích thước vô hình 9 px: muốn thấy cùng 1500 px phải đặt 1518 px rồi cắt
(trước đó so ở 1500 px làm vùng tài liệu của ta hẹp hơn 18 px, fit width 133,53 % thay vì 135,10 %).

| Phím | Bản đối chiếu | Trước | Nay |
| --- | --- | --- | --- |
| ↓ / ↑ | 54 px | 54 px | = |
| Page Down / Up, Space | chiều cao vùng xem − 41 px (773 / 814) | 0,9 × chiều cao | = (Shift+Space: xem dưới) |
| → / ← | đầu trang sau / trước | = | = |
| Home / End, Ctrl+Home / End | trang đầu / đầu trang cuối | = | = |
| Ctrl+wheel, Ctrl + / − | 1 · 8,33 · 12,5 · 25 · 33,33 · 50 · 66,67 · 75 · 100 · 125 · 150 · 200 · **300** · 400 · 600 · 800 · **1200** · 1600 · 3200 · 6400 % và các mức vừa trang / vừa rộng / vừa nội dung của trang hiện tại (60,38 · 135,10 · 188,77) | thiếu 300, 1200 và các mức vừa | = |
| Ctrl+0 / 1 / 2 | vừa trang / 100 % / vừa rộng | = | = |
| Ctrl+3 | **Fit Visible** 188,77 % | không có | 188,99 % |
| Alt+← / Alt+→ | khung nhìn trước / sau (mỗi phím, zoom, nhấp làm dời khung nhìn đều được ghi) | không có (nút thanh trạng thái mờ) | = (mỗi thẻ một lịch sử, nút thanh trạng thái dùng được) |
| F4 | ẩn / hiện cột điều hướng (fit width thành 141,67 %) | không có | = |
| F8, F9 | không làm gì | — | = |
| Ctrl+D | File › Properties | không có | = |
| Ctrl+K | hộp Preferences | không có | = |
| Ctrl+G | ô trang được chọn toàn bộ "1 / 12" để gõ | không có | = |
| Ctrl+F | hộp Search nhỏ ở góc trên phải vùng tài liệu | tiêu điểm ô tìm thanh tiêu đề | = |
| Ctrl+Shift+N | công cụ Note (bảng Format) | — | chưa làm |

**Fit Visible** (`reader_ink15.rs`), đo trên 4 trang: zoom = fit width × rộng trang / rộng *mực* của nội dung (hộp
đường nét glyph, không phải bề rộng tiến: 82,2 pt cho hộp tiến 83,75 pt; đường vẽ cộng nửa nét). Tỉ số với fit width,
bản đối chiếu / ta: tài liệu 12 trang 1,3973 / 1,3964; khung chữ nhật nét 2 pt 1,1930 / 1,1953; chữ 150–380 pt
2,6842 / 2,6778; chữ hẹp 84 pt 7,4452 / 7,3990. Phần lệch còn lại của chữ là font Helvetica thay thế (mực của bản đối
chiếu hẹp hơn ~0,5 pt mỗi dòng). Sau Ctrl+3 bản đối chiếu cuộn không theo quy tắc gọn (đỉnh khung nhìn cách nội dung
12–60 pt, mép trái 11–30 pt, có khi cắt mất bên phải); ta đặt mép trái nội dung cách 7,5 px và đỉnh 12,4 pt trên mực
(khớp tài liệu thử).

**Hộp Search (Ctrl+F)** (`reader-99`, `reader-100`): 304 × 123 px, sát trái thanh cuộn 4 px, đỉnh ở chân hàng thẻ;
dải tiêu đề (235, 246, 249) "Search" và ×; ô Find có nút tuỳ chọn (bánh răng của ta); Previous / Next (nền 230, viền
173, chữ xám khi chưa có chuỗi). Chụp trên màn hình: cửa sổ bật lên Windows 11 bo góc 8 px, viền mảnh (174 trên nền
trắng), bóng mềm ~15 px hai bên, ~30 px phía dưới (vạch đen 3 px trong ảnh PrintWindow là viền vô hình, không có thật).
Enter / F3 / Shift+F3 đi tới / lui; chỉ kết quả hiện tại được tô, màu (176, 206, 225) trên nền trắng như bản đối chiếu;
Esc / × đóng hộp và bỏ tô.

**Ô trang khi sửa (Ctrl+G)** (`reader-98`): viền xám 100, vòng trong 2 px (251, 251, 253), phần mũi tên (230, 233, 240),
chữ chọn (0, 120, 212); gõ số thay toàn bộ, Enter đi tới trang, Esc huỷ. Ô zoom ghi "100%" / "135.10%" như bản đối chiếu
(trước "100.00%"). Xám biểu tượng thanh trạng thái đo lại: trùng (175 mờ, 51 thường).

Ảnh: `reader-96` (Ctrl+3), `reader-97` (F4), `reader-98` (Ctrl+G), `reader-99` / `reader-100` (Ctrl+F). Test:
`reader_r15_tests.rs` (10, một test `#[ignore]` đo Fit Visible của tệp thật qua `BO_FV_PDFS`).

Còn lại: Shift+Space (đến dưới dạng văn bản, không biết Shift; hiện cuộn xuống), Ctrl+Shift+N; chuột (nhấp đúp / ba
lần, Shift+nhấp, nút giữa, menu chuột phải) — nhấp gửi bằng thông điệp tới `FoxitDocWnd` không chọn chữ ở công cụ Hand,
cần cách đo khác; nhóm isolated có blend; UIA sâu (focus, mẫu Text).
