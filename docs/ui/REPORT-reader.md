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
