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

## Vòng 4 — đo lại sau sửa (`bs-3.png` so với `pp-1.png`)

| Phần | Tham chiếu | BlueSlide trước | BlueSlide sau |
|---|---|---|---|
| Số "2" cạnh hình thu nhỏ: x mực | 27–36, cao 15 | 20–29, cao 11 | 27–36, cao 15 |
| Chữ gợi ý ghi chú: cách đường chia | 13 px, cao 16, rộng 168 | 17 px, cao 15, rộng 163 | 13 px; cỡ chữ ×1,33 (vòng 5) |
| Nút xem hiện hành: nền | 198, bắt đầu x 985 | 216 (không có nút) | 198, x 986 |
| Rãnh thanh trượt | màu 38, dài 152 px | 96, 134 px | 38, 152 px |
| Đường trên thanh trạng thái | cách đáy vùng khách 34 px | 32 px | 34 px (vòng 5) |

Còn lệch do khung chung: ribbon của BlueDoc thấp hơn ribbon tham chiếu 8 px, nên canvas và trang chiếu
bắt đầu cao hơn 4–8 px (trang chiếu y 286 so với 290); kích thước trang, thu phóng, lề ngang trùng khớp.

Độ nét chữ trang chiếu: cả hai đều khử răng cưa xám, không viền màu (độ lệch chuẩn kênh màu = 0 trên vùng
phụ đề). Mật độ mực vùng phụ đề: tham chiếu 936 điểm < 128, trung bình 233,8; BlueSlide 776, 237,4 — nét của
tham chiếu đậm hơn khoảng 20 % (gamma riêng); chưa chỉnh để không đổi chế độ chữ chung với BlueDoc.

## Vòng 5 — đo lại (`bs-4.png`)

Đường trên thanh trạng thái y 1059 (tham chiếu 1059). Chữ gợi ý ghi chú: x 259–429, cách đường chia 12 px,
cao 16 (tham chiếu 259–427, 13 px, 16). Số trang chiếu và nút xem như vòng 4.

## Tốc độ chuyển trang

Tệp `slide-album-photos.pptx` (5 trang, có ảnh), PageDown/PageUp xen kẽ:

| | |
|---|---|
| Tham chiếu (phím thật, đo màn hình, 30 lượt) | trung vị 27,7 ms, trung bình 34,4 ms |
| BlueSlide nội bộ (`BLUEOFFICE_NATIVE_PERF`, 12 lượt) | xử lý 0,09–0,13 ms, `prepare` 0,004 ms (ảnh đã vẽ trước), tổng 1,1–1,6 ms đến khi trình bày |

Đo màn hình cho BlueSlide không lặp lại được trong vòng này: chủ máy đang dùng máy (cửa sổ khác ở trên),
nên đã dừng mọi phép đo bằng phím toàn cục (`keybd_event`) để không gõ nhầm vào ứng dụng của người dùng;
các phép đo sau chỉ gửi thông điệp thẳng vào cửa sổ BlueSlide (`PostMessage`) và chụp bằng `PrintWindow`.

## Còn lại

- Chữ gợi ý trong chỗ dành sẵn rỗng ("Bấm để thêm tiêu đề"): engine chưa dựng; hiện chỉ có viền chấm.
- Hình thu nhỏ vẽ trực tiếp ở tỉ lệ nhỏ nên chữ đậm hơn tham chiếu (tham chiếu trông như thu nhỏ ảnh lớn).
- Từ dài vượt khung chữ không bị ngắt (bố cục engine).
- Hiệu ứng chuyển trang/hoạt hình trong trình chiếu, chế độ xem sắp xếp/đọc, bình luận, kéo thả thứ tự trang.
- Ribbon dùng khung của BlueDoc (dải tab trắng), không phải dải tab màu của tham chiếu (giữ màu riêng).

Vòng 6 (phiên chính): chữ gợi ý ở khung trống (`placeholder_kinds` mới trong kết quả lệnh slide, chuỗi i18n slideEditor.prompt*): tiêu đề trang tiêu đề 54 pt sát đáy khung, tiêu đề thường 44 pt giữa khung, tiêu đề phụ 24 pt sát đỉnh, nội dung 28 pt góc trên trái, màu đen như bản đối chiếu. Đo trên tệp do bản đối chiếu tạo (trang tiêu đề trống): tiêu đề hàng 556–606 so với 556–607, tiêu đề phụ 649–668 trùng, lệch ngang ≤ 9 px.

## Vòng 7 — bố cục chữ (engine)

Đo trên ứng dụng tham chiếu qua COM (hộp chữ 200 pt, Calibri 18 pt, `TextRange.Lines`):
"Supercalifragilisticexpialidociousxyzabcdefghij end" → `Supercalifragilisticexpiali` /
`dociousxyzabcdefghij ` / `end`: từ dài sang dòng riêng rồi ngắt giữa ký tự ở chỗ tràn, chữ sau nối
tiếp phần cuối. `bo-pptx` (`split_to_fit`) làm y vậy (kiểm thử `slide_text_fit`: 3 dòng). Tự co
(`spAutoFit` chiều cao, `normAutofit` tỉ lệ cỡ chữ) chạy lại cả sau khi đổi kích thước hình và đổi
cỡ/phông, không chỉ khi gõ. Chữ gợi ý riêng của bố cục (`hasCustomPrompt`) thay chữ gợi ý mặc định
(`placeholder_prompts` trong kết quả lệnh slide).

## Vòng 8 — đo lại khung (1500 × 1102, 125 %, `pp-Home.png` / `b2-Home.png`)

Vòng 1 ghi "ribbon tham chiếu cao hơn BlueDoc 8 px"; đo lại thì thân ribbon tham chiếu chạy y 99–235,
chỉ cao hơn ribbon BlueDoc 1 px. Tuỳ chọn theo cửa sổ `Ribbon::with_band_extra` (BlueDoc không đổi)
đặt +1 px cho BlueSlide; thanh trạng thái 33 px (trước 34), ngăn ghi chú 46 px (trước 45).

| Mốc (y thiết bị) | Tham chiếu | BlueSlide vòng 1 | BlueSlide giờ |
|---|---|---|---|
| Đường dưới ribbon | 235 | 234 | 235 |
| Canvas bắt đầu | 236 | 235 | 236 |
| Viền trên / dưới ảnh trang chiếu | 285 / 961 | 284 / 960 | 285 / 961 |
| Đường chia ghi chú | 1013 | 1013 | 1013 |
| Đường trên thanh trạng thái | 1060 | 1059 | 1060 |
| Ảnh trang chiếu x | 258–1457 | 258–1457 | 258–1457 |

## Vòng 9 — Design / Transitions / Animations / Review

Design: 5 chủ đề có sẵn của engine vẽ bằng phông tiêu đề và 6 màu nhấn của chủ đề, biến thể
màu / phông, cỡ trang 4:3 / 16:9, màu nền. Transitions: 6 ô + cột "thêm" (menu mọi hiệu ứng),
Effect Options theo `dirs` của engine, thời lượng, áp dụng cho mọi trang, Preview. Animations:
6 ô vào / nhấn / ra (chọn ô thay hiệu ứng của hình đang chọn, như tham chiếu), Effect Options.
Review: bình luận mới, ngăn bình luận. Chọn một chuyển trang / hoạt hình là chạy thử ngay trên
canvas như tham chiếu.

| Phần (thiết bị, 125 %) | Tham chiếu | BlueSlide |
|---|---|---|
| Hộp thư viện chuyển trang (khung 210, nền trắng) | y 106–197, x từ 104 | y 106–197, x từ 114 |
| Ô thư viện | rộng 124, ô chọn khung 136 nền 210 | 124, khung 136 nền 210 |
| Thư viện chủ đề: hộp / thẻ | hộp x 20, y 106–197; thẻ 130 × 74 khung 100, x 27 / 167, y 115–188 | trùng từng điểm ảnh |
| Dải màu nhấn của thẻ | y 172–179 | y 172–179 |
| Ngăn bình luận | rộng 342, nền 230, đường chia 191 | 342, 230, 191 |

Khác còn lại: thư viện chuyển trang lệch 10 px ngang (nhóm Preview của ta hẹp hơn); hình minh
hoạ ô là hình vẽ riêng phẳng; "Duration" là nút menu thay vì ô số; Effect Options không mờ khi
chưa có hiệu ứng.

## Vòng 10 — trình chiếu, chế độ xem, bảng tạm, bình luận

- Trình chiếu phát chuyển trang (mờ, qua đen, đẩy, lau, tách, che, kéo, phóng; loại khác phát
  như mờ) và hoạt hình chuỗi chính từ lớp + hành vi engine mô tả (hiện/ẩn, độ mờ, vị trí, kích
  thước, đường chuyển động, bộ lọc lau). Lớp vẽ trên ảnh nền trong suốt (Direct2D premultiplied);
  `Painter::page_part` vẽ một phần ảnh có dời / co / độ mờ. Bấm khi đang chạy thì xong bước đó; tự
  sang trang theo `advTm`.
- Nhịp khung (`BLUEOFFICE_NATIVE_PERF`, Reading view, `slide-anim-transitions.pptx`, màn hình
  144 Hz): 403 khung, trung vị 6,95 ms, p95 7,56 ms, lớn nhất 15,5 ms, không khung nào > 25 ms —
  bám nhịp màn hình (≥ 60 fps). Phím sang trang: xử lý 0,9 ms (trung vị).
- Slide Sorter so với `pps-View.png` (tham chiếu, `slide-album-photos.pptx`): trùng từng số đo —
  ảnh 400 × 225, x 48 / 494 / 940, viền trên y 260, bước dọc 296, khung chọn 3 px cách 1 px, mực
  số "1" x 50–55 y 501–516, "4" x 48–58 y 797–812; thu phóng hiện 100 %.
- Reading view: trình chiếu trong cửa sổ (không ribbon), thanh trạng thái đếm trang đang chiếu.
- Kéo hình thu nhỏ đổi thứ tự (ngăn trái và sorter, một bước hoàn tác), Shift chọn dải, xoá nhiều
  trang.
- Bảng tạm hệ thống: chép hình / trang đưa chữ, ảnh (HTML img PNG) và mã cửa sổ lên bảng tạm; dán
  chữ của chương trình khác thành hộp chữ.
- Ngăn bình luận: thêm (Enter), giải quyết / mở lại, xoá; bình luận mới ghi giờ UTC.

## Còn lại (sau vòng 10)

- Hình minh hoạ thư viện chuyển trang / hoạt hình chưa có gradient như tham chiếu.
- Xoay (spin), bộ lọc khác lau (blinds, dissolve, …) phát như mờ; hiệu ứng theo đoạn / theo chữ phát
  cả lớp; morph chưa phát.
- Dán trang chiếu sang cửa sổ BlueSlide khác (bảng tạm chỉ mang chữ + ảnh giữa hai tiến trình).
- Bình luận: chưa có dấu bình luận trên trang, chưa trả lời theo luồng trong ngăn.
- Animation Pane chỉ là lệnh; tab Draw chưa làm.
