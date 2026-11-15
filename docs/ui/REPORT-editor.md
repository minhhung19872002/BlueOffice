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
