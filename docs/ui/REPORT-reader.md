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
