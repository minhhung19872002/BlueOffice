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
