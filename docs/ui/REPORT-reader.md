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
