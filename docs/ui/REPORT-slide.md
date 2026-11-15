# BlueSlide giao diện Rust tự vẽ — so với ứng dụng trình chiếu tham chiếu (N6)

`blueslide-native.exe` (apps/native-win, bin `blueslide-native`): `bo_ui::SlideWindow` + `SlideView` trên
`bo-gpu-win` (Direct2D/DirectWrite, bitmap trang vẽ lại theo dải), gọi thẳng `Engine::slide_command`
(lệnh có kiểu, không JSON). Đo hộp đen trên Windows 125 % DPI, cỡ chữ Accessibility 110 %, cửa sổ
1500 × 1102 tại (40, 40) chụp bằng PrintWindow (`cap2.ps1`), cùng tệp `tests/corpus/slide/slide-text-bullets.pptx`
(bản sao) mở trong ứng dụng tham chiếu qua COM.

## Vòng 1 — bố cục vùng soạn

| Phần | Tham chiếu (px thiết bị, 125 %) | BlueSlide |
|---|---|---|
| Nền canvas / ngăn hình thu nhỏ | 230 xám | 230 (`SLIDE_CANVAS`) |
| Ngăn hình thu nhỏ | rộng 234, đường chia 1 px màu 191 | 234 (187 logic), 191 |
| Hình thu nhỏ | ảnh 156 × 88, viền 1 px (200,198,196), cách mép trái 49, bước 115, ảnh đầu cách đỉnh 25 | giống hệt (kiểm thử `layout_matches_the_reference_at_125_percent`) |
| Hình thu nhỏ đang chọn | khung 3 px màu thương hiệu, hở 1 px | khung 3 px, hở 1 px (đo: x 54–56 cả hai), màu nhấn BlueSlide `#C2185B` (không chép màu thương hiệu) |
| Số trang chiếu | chữ số cao 16 px, mép phải cách ảnh 22 px | trước: cao 11 px, lệch 6 px → sửa cỡ 16,8 logic, mép phải −22 px (vòng 3) |
| Canvas | 1227 × 769 | 1227 × 769 |
| Thu phóng vừa cửa sổ | 75 % (làm tròn xuống số nguyên, lề ≈ 8 logic) | 75 %, cùng công thức |
| Trang chiếu | ảnh tại (258, 290), viền 1 px màu 198 | (258, 290), viền 198 |
| Ngăn ghi chú | cao 45, đường chia 191, chữ gợi ý xám | 45 (36 logic), 191 |
| Thanh trạng thái | cao 32, đường 198, nền 243 | 32, 198, 243 |

## Vòng 2 — chọn hình và sửa chữ

| Phần | Tham chiếu | BlueSlide |
|---|---|---|
| Khung chọn | 1 px xám 97 | 1 px xám 97 |
| Tay nắm | tròn, ngoài 14 px, vòng 2 px xám 97, ruột trắng | như vậy |
| Tay nắm xoay | tâm cách mép trên 43 px, đường nối, mũi tên vòng | 43 px (34,4 logic), hình vẽ riêng |
| Khi gõ chữ | khung chấm 2 bật / 1 tắt, vẫn có tay nắm | như vậy |
| Vùng chữ chọn | xám 185 trên nền trắng | alpha 70 → 185 |

Chênh lệch còn lại: khung hình tiêu đề lệch 1 px dọc (hộp do engine tính); thân chữ dòng đầu lệch 6 px dọc
(bố cục chữ của renderer, không phải giao diện).

## Vòng 3 — thanh trạng thái

Tham chiếu: "Slide n of N" trái; bên phải nút Ghi chú (không tô nền khi bật), Bình luận, bốn nút chế độ xem
(nút hiện hành nền xám), thanh trượt thu phóng rãnh xám đậm, phần trăm, nút vừa cửa sổ. Đã sửa: bỏ nền tô
của nút Ghi chú, thêm bốn nút xem (nút cuối chạy trình chiếu từ trang hiện tại), rãnh thanh trượt 96,
biểu tượng vừa cửa sổ màu chữ. Chưa có: nút Bình luận, ngôn ngữ, trợ năng.

## Tốc độ

Gõ chữ vào tiêu đề trang 1 (phím thật qua `keybd_event`, đo đổi điểm ảnh màn hình bằng `CopyFromScreen`, 23–30 lượt):

| | trung vị | trung bình | min–max |
|---|---|---|---|
| Tham chiếu | 43,2 ms | 41,3 ms | 20,9–48,9 |
| BlueSlide | 43,3 ms | 44,3 ms | 29,8–56,2 |

Cả hai bị chặn bởi nhịp hợp thành màn hình/chụp màn hình (≈ 2–3 khung). Nội bộ BlueSlide
(`BLUEOFFICE_NATIVE_PERF`): xử lý phím 0,9–1,4 ms (lệnh `SetText` gộp hoàn tác), vẽ lại dải trang + hình thu
nhỏ 0,3–0,7 ms, vẽ giao diện 0,8–1,1 ms, tổng đến khi trình bày 2,5–3,4 ms.

Chuyển trang (PageDown/PageUp, `slide-edit-slides.pptx`): trang kề đã được vẽ trước lúc rảnh, `prepare_ms`
0,003–0,005 ms, tổng 1,3–3,2 ms mỗi phím. Ảnh giữ theo id trang chiếu nên thêm/xoá/chuyển trang chỉ vẽ
trang mới (kiểm thử `new_slide_and_navigation_reuse_kept_images`).

## Còn lại

- Chữ gợi ý trong chỗ dành sẵn rỗng ("Bấm để thêm tiêu đề"): engine chưa dựng; hiện chỉ có viền chấm.
- Hình thu nhỏ vẽ trực tiếp ở tỉ lệ nhỏ nên chữ đậm hơn tham chiếu (tham chiếu trông như thu nhỏ ảnh lớn).
- Từ dài vượt khung chữ không bị ngắt (bố cục engine).
- Hiệu ứng chuyển trang/hoạt hình trong trình chiếu, chế độ xem sắp xếp/đọc, bình luận, kéo thả thứ tự trang.
- Ribbon dùng khung của BlueDoc (dải tab trắng), không phải dải tab màu của tham chiếu (giữ màu riêng).
