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

## Vòng 11 — tab Transitions / Animations đo lại (`b7-*.png` so với `pp-Transitions.png`, `ppsel-Animations.png`)

Chữ trong các ô điều khiển của tham chiếu (tên ô thư viện, Sound/Duration/After, Start…)
lớn hơn nhãn nhóm khoảng 8 % (cao chữ hoa 13–14 px so với 11): `control_style` 13 logical.

| Phần (thiết bị, 125 %) | Tham chiếu | BlueSlide vòng 9 | BlueSlide giờ |
|---|---|---|---|
| Đường chia sau Preview | x 92 | 102 | 94 → 92 (Preview 48,8 logical) |
| Hộp thư viện / ô đầu | x 104 / 106 | 114 / 115 | 104 / 106 |
| Bước ô thư viện | 124 | 127 | 124 (ô 121 + khe 3) |
| Ô đang chọn | y 110–193, khung 136, nền 210 | y 107–196 | y 110–193 |
| Hình ô (thẻ trang chiếu) | 50 × 45 tại y 117, viền 151/196, nền (201,217,234) | hình phẳng 48 px | 50 × 45 tại y 117, cùng màu viền/nền, mũi tên xám 114 viền trắng, dải màu đậm dần (hình vẽ riêng) |
| Cột "thêm" | x 851–886, khung 128 trên nền 243 | khung 210 | x 851–886, khung 128 |
| Effect Options | trong nhóm thư viện, 2 dòng, mờ khi không có biến thể | nhóm riêng, 1 dòng | như tham chiếu; đường chia sau nhóm 985 → 990 |
| Sound (hộp thả) | x 1088–1229, y 104–135 | không có | x 1086–1227, y 104–135 |
| Duration (ô số) | x 1113–1212, y 138–169, "02.00" | nút menu | x 1112–1211, y 138–169, bước 0,25 s, gõ được |
| Đường giữa hai cột | x 1235, màu 210 | — | x 1236 |
| On Mouse Click / After | ô đánh dấu 18 px tại x 1245; After x 1322–1421 | — | x 1246; After x 1321–1420 |
| Chữ "Sound:" / "Advance Slide" | rộng 54 / 111, đỉnh y 113 | — | 49 / 109, y 112 |
| Tên ô "None" | rộng 41, đỉnh y 170 | 35, y 173 | 40, y 168 → 170 |

Animations (có hình được chọn): 4 ô sao như tham chiếu, sao màu xanh lục ngả lam (104,164,144)
khi có lựa chọn, xám khi không; nhóm Advanced Animation (Add Animation 2 dòng, Animation Pane,
Trigger, Animation Painter); nhóm Timing hai cột (Start thả xuống x 1126, Duration/Delay ô số
x 1160–1259; Reorder Animation, Move Earlier/Later). Đường chia nhóm: tham chiếu 92 / 737 / 1040 /
1438, BlueSlide 92 / 740 / 1029 → (Add Animation rộng thêm 12 px) / 1430.

## Vòng 11 — trình chiếu

- Xoay (spin, §19.5.5 animRot và khoá `r`/`style.rotation`): `Painter::page_part_rotated` (Direct2D
  `SetTransform` quanh tâm hình), mặc định vẽ không xoay ở backend khác.
- Bộ lọc §19.5.3 ngoài lau: blinds, checkerboard, randombar, dissolve, barn, box, circle, diamond,
  plus, strips, wedge, wheel, slide — `slide_reveal::Reveal` trả về các hình chữ nhật (phân số của
  hộp hình) cần vẽ ở tiến độ q; hình tròn/thoi/chữ thập/nan quạt quét lưới 96 × 96 gộp theo hàng
  (≤ 96 lệnh vẽ mỗi khung), dissolve 48 × 27 ô theo thứ tự xáo trộn cố định.
- Chuyển trang cùng họ (blinds, checker, comb, randomBar, circle, diamond, plus, wedge, wheel,
  strips, dissolve; random/vortex/shred/… như dissolve), flash (qua trắng), newsflash (xoay 2 vòng
  + phóng), reveal, các hiệu ứng 3D (flip, cube, gallery, …) phát dạng lật phẳng.
- Theo đoạn (`LayerOut::parts`) và theo từ / chữ (§19.5 iterate, `LayerOut::text`): mỗi lớp giữ 2
  ảnh (thân + mọi mảnh gộp một ảnh), mỗi mảnh vẽ qua hộp của nó với thời điểm riêng
  (`start + i × text_step_ms`, đảo khi `backwards`).
- Morph ([MS-PPTX] p159:morph): 6 ảnh (nền cũ/mới, cặp hình cũ/mới, hình chỉ có ở trang cũ/mới);
  cặp hình dời, co giãn, xoay từ hộp cũ sang hộp mới, đường viền đổi điểm theo điểm
  (`MorphOutline`), nền và hình lẻ mờ dần.

- Lỗi tìm ra khi chạy thật (không lộ ra ở kiểm thử headless): `Frame::draw_page_part` đưa phần
  nguồn theo điểm ảnh thiết bị cho `DrawBitmap`, trong khi ảnh trang / lớp được tạo ở DPI
  96 × px/pt nên Direct2D hiểu là DIP — mọi lần vẽ một phần (chữ theo từ, lau, phóng, theo đoạn,
  morph) lấy sai vùng (trước đây chỉ vẽ cả lớp nên trông đúng). Sửa: `source_rect` đổi sang DIP
  theo `GetSize()` của ảnh.
- Nhịp khung (`BLUEOFFICE_NATIVE_PERF`, Reading view, `fx-slow.pptx`: tệp do ứng dụng tham
  chiếu tạo qua COM — blinds, bay vào, bay vào theo từ — làm chậm 10 lần, trang 2 Morph 8 s;
  màn hình 144 Hz): 3368 khung, trung vị 6,95 ms, p95 7,23 ms, lớn nhất 32,6 ms, 1 khung > 25 ms
  (dựng ảnh lớp / Morph của trang mới) — bám nhịp màn hình. Phím sang bước: 0,86 ms (trung vị).
  Ảnh chụp giữa hiệu ứng: `fs-blinds.png` (6 dải), `fs-fly.png`, `fs-words.png`, `fs-morph.png`
  (tiêu đề dời, thu hẹp, xoay 15°).

## Vòng 11 — ngăn, bình luận, bảng tạm, Draw

- Animation Pane: danh sách hiệu ứng (số lần bấm, sao màu theo loại, tên hình, thanh thời gian),
  chọn (chọn luôn hình), lên/xuống (MoveAnimation), Play From / Play All, Delete xoá.
- Bình luận: dấu (bong bóng) trên góc trang cho mỗi luồng, bấm mở ngăn ở luồng đó (khung màu nhấn);
  ô "Reply..." dưới mỗi luồng (trả lời theo luồng); tab Review: New, Delete, Previous, Next,
  Show Comments.
- Bảng tạm giữa hai cửa sổ BlueSlide (hai tiến trình): dạng nội bộ mang token + mô tả (trang /
  hình) + tệp trình chiếu base64; cửa sổ kia mở tệp trong engine tạm, chép lại rồi chuyển bảng tạm
  sang (`Engine::copy_from_deck`), dán ra đúng trang / hình với ảnh, bố cục, chủ đề.
- Tab Draw (giữa Insert và Design như tham chiếu): Tools (Eraser, Select), Pens (đen, đỏ, bút đánh
  dấu); nét vẽ trực tiếp trên trang, lưu bằng lệnh ink của engine (mỗi nét một bước hoàn tác).
- Slide Show: Hide Slide; View: Normal / Slide Sorter / Reading View; Insert: Comment, Text.

## Còn lại (sau vòng 11)

- Sao hoạt hình và hình ô chuyển trang còn phẳng hơn tham chiếu (tham chiếu tô chuyển màu trong
  hình sao); hình Morph / Fade của ta là hình vẽ riêng.
- Home / Insert / Slide Show / View còn thiếu nhiều nhóm của tham chiếu (Tables, Images,
  Illustrations, Links, Symbols, Media; Rehearse / Record; Master Views; Color/Grayscale; Window).
- Design: tham chiếu có 6 thẻ chủ đề + thư viện Variants; ta 5 thẻ + Colours / Fonts.
- Dấu bình luận đặt ở góc trang (engine chưa đọc vị trí neo của bình luận hiện đại).
- Draw: chưa có thước, lasso, chuyển nét thành hình / chữ / toán (engine đã có lệnh).
- Dán chéo cửa sổ mang cả tệp trình chiếu (base64) trên bảng tạm: tệp lớn chép chậm hơn.
- Trigger / Animation Painter chưa mờ khi không có hiệu ứng (luật bật/tắt nút của ribbon không
  thấy trạng thái trang chiếu).

## Vòng 12 — nhóm còn thiếu so với tham chiếu (`b1-*.png`, `b3-*.png` so với `pp-*.png`, 1500 × 1102, 125 %)

- Insert: Slides, Tables (lưới 10 × 8 như tham chiếu, đầu đề "Insert Table" → "3x2 Table"), Images
  (Pictures: host mở hộp thoại chọn tệp, PNG/JPEG/SVG), Illustrations (Shapes: thư viện hình dựng sẵn
  theo nhóm, vẽ bằng hình học §20.1.9.18 của `bo-drawingml`; Icons: thư viện biểu tượng riêng; Chart),
  Comments, Text (Text Box, Header & Footer — hộp thoại ngày giờ / số trang / chân trang / không hiện
  trên trang tiêu đề, Date & Time, Slide Number), Symbols, Media (video / âm thanh từ tệp).
- Home: Format Painter; Drawing (Shapes, Arrange, Quick Styles, tô / viền / hiệu ứng dạng biểu tượng có
  mũi tên như tham chiếu ở bề rộng này); Editing (Find / Replace / Select All, hộp thoại tìm thay theo
  cả bản trình bày, không phân biệt hoa thường).
- Tab ngữ cảnh sau các tab cố định, hiện ngay khi chèn: Shape Format (3 ô kiểu "Abc" theo màu chủ đề,
  tô / viền / hiệu ứng, Arrange, Size), Picture Format (Reset, viền, cắt theo hình / tỉ lệ / Fill / Fit),
  Table Design (ô đánh dấu Header Row / Banded Rows đọc từ `a:tblPr`, Shading), Table Layout (chèn /
  xoá hàng cột, gộp / tách ô).

| Đo (thiết bị) | Tham chiếu | BlueSlide |
|---|---|---|
| Size của hình chữ nhật 150 × 100 pt | 1.39" / 2.08" | 1.39" / 2.08" |
| Size của ảnh 120 × 90 pt | 1.25" / 1.67" | 1.25" / 1.67" |
| Ô số Size | rộng 112, cao 32 | 112 × 32 |

- Design: 6 thẻ chủ đề (thẻ đầu là chủ đề của bản trình bày, luôn được đánh dấu) + cột "thêm";
  Customize (Variants, Colors, Fonts, Effects, Background Styles); Slides (Layout áp cho trang đang
  chọn — trước đây thêm trang mới, sai so với tham chiếu; Slide Size + hộp thoại cỡ tuỳ chỉnh 1–56 inch);
  ngăn Format Background (tô đặc theo bảng màu chủ đề, Hide background graphics, Apply to All, Reset).
- Slide Show: Custom Slide Show (chạy / định nghĩa), hộp thoại Set Up Slide Show (khoảng trang, lặp đến
  Esc, không thuyết minh, không hoạt hình, dùng thời gian), Rehearse Timings (thanh thời gian góc trên;
  kết thúc hỏi giữ thời gian → `SetTimings`), ô đánh dấu Play Narrations / Use Timings / Show Media
  Controls / Use Presenter View, Presenter View (trang hiện tại 62 % bên trái, trang kế, ghi chú, đồng
  hồ, "Slide n of N"). Trang ẩn bị bỏ qua khi trình chiếu (trước đây vẫn chiếu).
- View: Outline View (dàn ý thay ngăn hình thu nhỏ, bấm để sang trang), Notes Page, Slide Master (tab
  riêng + Close Master View), Gridlines (chấm mỗi 1/2 inch), Guides (đường giữa), Zoom.
- Draw: Lasso Select, bút xanh dương / xanh lá, Ink to Shape.
- Trigger / Animation Painter mờ khi lựa chọn không có hiệu ứng (cờ trạng thái qua `FormatState.style`).
- Dấu bình luận đặt tại `p188:pos` của luồng (bo-pptx giờ đọc và ghi lại vị trí này).
- Bảng tạm giữa hai cửa sổ: chỉ mang một bản trình bày nhỏ chứa đúng các trang / hình đã chép
  (`Engine::slide_clip_deck`); trang 2 của `slide-album-photos.pptx`: nhỏ hơn cả tệp (kiểm thử).

## Còn lại (sau vòng 12)

- Thư viện Table Styles, WordArt Styles, Alt Text, Selection Pane; Picture Format chưa có Remove
  Background / Corrections / Color / Artistic Effects trên ribbon (engine có `PictureRemoveBackground`).
- Presenter View vẽ trong cùng cửa sổ (chưa mở cửa sổ thứ hai trên màn hình phụ); Record chưa có.
- Ruler (View và Draw), Ink to Math, Grayscale / Black and White, Handout / Notes Master.
- Dấu bình luận chưa neo theo hình (`ac:deMkLst`); bình luận cũ (`p:cm`) chưa đọc.
- Hình ô chuyển trang / sao hoạt hình vẫn phẳng hơn tham chiếu; dấu thả xuống của nút lớn là "▾" trên
  cùng dòng, tham chiếu là chữ v mảnh ở dòng riêng.

Đo lại Design (`b5-Design.png`): 6 thẻ chủ đề x 27 / 167 / … / 727 trùng tham chiếu, cột "thêm" x 863–897, đường chia Customize x 910 (tham chiếu 912), nhóm Slides hiện đủ (trước: tràn, có nút cuộn).


Vòng 12b (`slide-07/08/09`): Insert thêm Photo Album, 3D Models, SmartArt, Links, Text Styles; dấu thả xuống của nút lớn một từ xuống dòng riêng như tham chiếu (hết chồng nhãn "Variants ▾ Colors" ở Design); ô phông 211 px (tham chiếu 213), ô cỡ 67 px.


## Vòng 13 — thư viện, Picture Format, chế độ xem, bình luận, phần (`slide-10…17`, 1500 × 1102, 125 %)

- Table Design: 6 ô tuỳ chọn kiểu bảng (`a:tblPr` firstRow / lastRow / firstCol / lastCol / bandRow /
  bandCol, lệnh engine `setTableFlags`), thư viện 5 kiểu dựng sẵn vẽ như bảng nhỏ (5 × 5, hàng đầu màu
  nhấn, kẻ dòng mảnh, viền màu nhạt, ô đang dùng có khung xám + nền trắng như tham chiếu) + cột "thêm";
  Shading / Borders / Effects; nhóm Text Styles; Draw Borders (ô kiểu bút / độ dày vẽ chính nét, Pen
  Color, Draw Table kéo ra bảng, Eraser gộp ô với ô bên phải; lệnh `setTableBorders`).
- Shape Format: cột "thêm" của thư viện kiểu, nhóm Text Styles (kiểu chữ nhanh, tô / viền chữ, biến
  dạng §20.1.10.76), Accessibility (Alt Text), Arrange có Selection Pane, Align / Group / Rotate dạng
  biểu tượng như tham chiếu.
- Selection Pane (nền 230, nút khung 148, hàng chọn 198 khung 128 — đo trên tham chiếu): hình từ
  trước ra sau, con mắt ẩn / hiện (`cNvPr@hidden`), Show All / Hide All (một bước hoàn tác), lên /
  xuống thứ tự (cả hình ẩn), nhấp đúp đổi tên. Alt Text: ô mô tả, "Mark as decorative"
  ([MS-ODRAWXML] `adec:decorative`). Lệnh engine mới: `slide_edit::shape_props` (có kiểm thử khứ hồi).
- Picture Format: Remove Background (tab Background Removal, xem trước graph-cut tô vùng bỏ đi đè lên
  ảnh, khung đứt nét không tay nắm, nét đánh dấu giữ / bỏ, Keep / Discard), Corrections / Color /
  Transparency là thư viện ảnh xem trước thật của ảnh đang chọn (backend vẽ id ảnh dẫn xuất
  `media#fx:`), Compress Pictures (hộp thoại + kết quả), Change Picture, Reset, Quick Styles (khung,
  mép mờ, phản chiếu, phối cảnh). Artistic Effects của tham chiếu không làm (engine không có).
- View: chế độ đang dùng hiện nhấn; Show (Ruler / Gridlines / Guides rồi Notes); Color / Grayscale /
  Black and White (danh sách hiển thị đổi màu cho vùng soạn và hình thu nhỏ, trình chiếu vẫn màu);
  Handout Master / Notes Master (tab riêng: hướng, số trang mỗi tờ, chỗ dành sẵn, Close); Ruler (inch
  tính từ tâm trang); Window (New Window cùng tệp, Arrange All, Cascade, Switch Windows).
- Bình luận: đọc bình luận cũ (§19.4 `p:cmLst`, `p:cmAuthorLst`) — chỉ xem, phần giữ nguyên từng byte;
  luồng neo vào hình (`ac:deMkLst` / `ac:spMk`) đọc / ghi, bình luận mới trên một hình đang chọn được
  neo vào hình đó, dấu bình luận ở góc trên phải của hình.
- Home > Slides như tham chiếu (New Slide lớn; Layout, Reset, Section nhỏ); menu Section (thêm, đổi
  tên, xoá, xoá hết, thu gọn / mở rộng hết); tiêu đề phần trong ngăn hình thu nhỏ (mũi tên, tên, số
  trang khi thu gọn; nhấp để thu gọn, nhấp đúp đổi tên).
- Sao hoạt hình tô chuyển màu (sáng trên, tối dưới, viền đậm hơn); mặt thẻ chuyển trang chuyển màu.

| Tốc độ (bản release, `deck-300.pptx` từ `bo-cli sample-big-deck`, 42 MB, 300 trang) | BlueSlide |
|---|---|
| Mở tệp trong engine | 280 ms |
| Từ khởi động tiến trình đến khung đầu | 756 ms (tham chiếu mở cùng tệp qua COM: 784 ms, chưa tính khởi động) |
| Cuộn ngăn hình thu nhỏ (60 nấc bánh xe) | trung vị 2,7 ms, p95 3,7 ms mỗi nấc |
| Gõ phím trong tiêu đề, trước sửa | trung vị 23,2 ms (chuẩn bị ảnh 18,8 ms: vẽ lại cả hình thu nhỏ của trang) |
| Gõ phím, sau sửa (hình thu nhỏ vẽ khi rảnh) | trung vị 11,8 ms (p95 13,3; chuẩn bị 8,3 ms); khung đầu 655 ms |

Còn lại (sau vòng 13): Presenter View trên cửa sổ thứ hai / màn hình phụ và ô Monitor (host chỉ có một
cửa sổ mỗi tiến trình); thư viện Corrections / Color của tham chiếu không mở được bằng COM nên chỉ so
nút; khung trang phát (handout) chưa vẽ ô trang chiếu trong chế độ Handout Master; Eraser chỉ gộp với
ô bên phải.

## Vòng 14 — hành vi: kéo hình, chữ, trình chiếu, khung trang phát (`slide-19…23`, 1500 × 1102, 125 %)

Tham chiếu được điều khiển bằng COM và `PostMessage` chuột thẳng vào cửa sổ `mdiClass` của nó (không có
phím / chuột thật); kéo hình, khung chọn, đổi cỡ chạy được như vậy (kéo hình thu nhỏ thì không: OLE drag-drop).

| Đo (thiết bị) | Tham chiếu | BlueSlide |
|---|---|---|
| Khung chọn (marquee) | viền 1 px xám 115, trong xám 115 phủ 50 % (trắng → 185, 46/117/182 → 81/116/149) | như vậy |
| Đường gióng khi kéo | nét đứt 1 px 209/139/121, chu kỳ 7 px, vượt hai hình ~10 px | như vậy (trước: liền, 210/74/38) |
| Khoảng hút gióng | hút khoảng hở 5,5 px (4,4 pt ở 75 %) | 6 px logic trên màn hình (trước: 4 pt cố định) |
| Kéo hình | hình đi theo con trỏ (bản gốc biến mất) | như vậy (áp dụng trực tiếp, một bước hoàn tác) |
| Xoá nền: vùng bỏ | 200/0/200 (trời), 168/0/168 (cỏ), 252/0/252 (trắng) | 201/0/201, 167/0/167 (`0,6 × độ sáng + 0,4 × 250`, kênh lục 0) |
| Gõ chữ tiêu đề (nội bộ, 60 phím) | — | tổng p50 5,3 ms, p95 6,3 ms (vòng 13: 11,8 ms) |

- Cạnh trên hình được hút vào tâm phụ đề như tham chiếu (cạnh ↔ tâm). Lệch dọc của chữ do số đo phông:
  BlueSlide desktop dùng Calibri hệ thống với `hhea` 0,75 / 0,25 em; tham chiếu dùng `OS/2.usWinAscent /
  usWinDescent` (0,952 / 0,269 em). Bộ thử tự dựng (Calibri 24 / 54 pt, giãn dòng 100 / 90 / 80 %, neo trên /
  dưới): dòng đầu cao hơn 5–6 px (24 pt), 12–13 px (54 pt); sau sửa lệch 0–2 px, neo dưới 0–1 px; phụ đề /
  tiêu đề trùng từng hàng mực (628 / 532). Phông có cờ USE_TYPO_METRICS giữ số đo typo; Carlito / Liberation /
  Tinos đóng gói có `hhea` = win nên web và ảnh chụp kiểm thử không đổi.
- Hình: Shift + kéo đi thẳng, Ctrl + kéo chép (một bước hoàn tác), nhấp lần hai vào hình đang chọn vào chữ khi
  nhả (kéo thì di chuyển; trước đây vào chữ ngay), Shift + nhấp hình đang chọn bỏ nó khỏi lựa chọn khi nhả.
  Shift / Ctrl lấy từ cờ của thông điệp chuột (`win32::mouse_mods`).
- Chữ: nhấp ba lần chọn đoạn (kèm dấu đoạn); Tab giữa đoạn gõ ký tự tab, ở đầu đoạn hạ cấp (thử trên tham
  chiếu: `Mục |tiêu` → `Mục \ttiêu`, đầu đoạn → cấp 2); gạch sóng đỏ / xanh dưới lỗi chính tả / ngữ pháp của
  trang hiện tại (lệnh `slideSpelling`, kiểm lại ở nhịp kế sau mỗi thay đổi).
- Trình chiếu: B / . màn đen, W / , màn trắng (phím / nhấp bất kỳ trở lại, không sang bước); số + Enter tới
  trang; Ctrl+P bút đỏ, Ctrl+I bút đánh dấu, Ctrl+L laser, Ctrl+A / Esc về mũi tên (Esc lần đầu chỉ bỏ bút),
  E xoá nét của trang. Máy chỉ có một màn hình nên Presenter View vẫn trong cửa sổ.
- Handout Master: chỉ còn trang (không ngăn hình thu nhỏ / ghi chú), ô trang chiếu chấm từ
  `PageMasterOut.slots`, chữ gợi ý "Header" / "Footer" 12 pt. Eraser xoá cạnh ô gần con trỏ nhất, bốn phía.
- Slide Sorter có thu phóng riêng (Ctrl + bánh xe, thanh trạng thái): ảnh 320 px logic × thu phóng.
- Web: `pnpm build:wasm`, `@blueoffice/platform` 185 và `@blueoffice/slide-editor` 310 kiểm thử qua sau thay
  đổi bình luận neo hình của vòng 13, không cần sửa.

Còn lại (sau vòng 14): ngày giờ trong chỗ dành sẵn Handout Master và thanh trạng thái "Handout Master";
đổi cỡ của tham chiếu vẽ bóng mờ 50 % (ta vẽ hình thật); hỏi giữ nét bút khi kết thúc trình chiếu; Presenter
View trên màn hình phụ.

## Vòng 15 — phần còn lại, định dạng chữ, biểu đồ, bảng (`slide-24…`, 1500 × 1102, 125 %)

Ảnh chụp: cửa sổ của ta được đặt *topmost* (không kích hoạt) trong lúc chụp — các cửa sổ của tác tử khác che
lên thì swap chain bị *occluded* và `PrintWindow` trả khung cũ (vòng trước tưởng là lỗi nhấp chuột).

| Đo (tham chiếu qua COM / PostMessage) | Tham chiếu | BlueSlide |
|---|---|---|
| Handout Master: ô ngày | trường `datetimeFigureOut` 11/16/2026, căn phải | như vậy (ngày theo giờ máy, `MM/DD/YYYY`) |
| Handout Master: ô số trang | `‹#›`, khung đứt nét | như vậy (trước: "1", không khung) |
| Handout Master: thanh trạng thái | "Handout Master", không Notes | như vậy; không thanh cuộn |
| Handout Master: ribbon | Handout Orientation, Slide Size, Slides Per Page; Edit Theme; Background | như vậy (Hide Background Graphics mờ) |
| Bóng đổi cỡ | hình mới mờ 60 % (46/117/182 trên trắng → 130/172/211), bản gốc ẩn, không tay nắm | như vậy (trước: hình thật) |
| Increase Font Size từ 18 | 20 24 28 32 36 40 44 48 54 60 66 72 80 88 96 115 138 166 199 239 287 344 | trùng (Ctrl+] / Ctrl+Shift+>, nút ribbon; trước: +2 pt) |
| Decrease Font Size từ 18 | 16 14 12 11 10,5 10 9 8 7 6 5 4 3 2 | trùng |
| Cỡ lẫn 12 / 30, tăng | 14 / 32 (mỗi đoạn chạy theo cỡ riêng) | trùng |
| Shrink text on overflow (thân 828 × 342,6 pt, 28 pt, N dòng) | 9 dòng: lnSpcReduction 10 %; 10: 92,5 % + 20 %; 11: 85; 12: 77,5; 13: 62,5; 14–15: 55; 16: 47,5; 17: 40; 18: 32,5 | bước như vậy: giãn dòng −10 %, −20 % rồi cỡ chữ từng 7,5 % (trước: ×0,9 và +5 % cùng lúc) |

- Kết thúc trình chiếu có nét bút / bút đánh dấu: hộp "Do you want to keep your ink annotations?"
  (Keep / Discard) như tham chiếu; Keep thêm nét vào đúng trang (lệnh engine `keepInk`, một bước hoàn tác).
- Kiểm thử bố cục cho số đo dòng Windows (`slide_line_metrics`): Calibri hệ thống, hộp neo trên, dòng đầu ở
  `tIns + usWinAscent` (24 / 54 pt, ±0,5 pt); bỏ qua khi máy không có Calibri.
- Home: Clear All Formatting, Character Spacing (Very Tight −3 … Very Loose +6 pt), Change Case (Sentence
  case., lowercase, UPPERCASE, Capitalize Each Word, tOGGLE cASE, chữ vẫn được chọn), Line Spacing (1.0 … 3.0,
  dấu chọn ở mức hiện tại, đường kẻ trên Line Spacing Options…), hộp Paragraph (Before text, First line âm =
  treo, Before / After pt, giãn dòng bội số hoặc "N pt" chính xác; giá trị đầu là giá trị hiệu lực qua
  master — `FormatState.paragraph`).
- Format Painter trên chữ: chép định dạng ký tự của vùng chọn, quét chọn chữ khác (hoặc nhấp một từ) là dán,
  dùng một lần như tham chiếu.
- Biểu đồ: Insert > Chart mở danh sách loại; biểu đồ chọn hiện tab Chart Design / Format của BlueSheet (Add
  Chart Element, Quick Layout, Change Colors, Chart Styles, Change Chart Type, Shape Fill / Outline) trên lệnh
  `setChart` của engine.
- Bảng: Tab sang ô kế (chọn hết chữ), Shift+Tab ô trước, Tab ở ô cuối thêm hàng mới; gõ chữ trong ô giờ ghi
  vào ô (trước: lệnh `setText` thiếu `cell`, chữ không vào ô).
- Home > Font / Paragraph như bố cục tham chiếu: hàng 2 B I U S abc AV▾ Aa▾ | tô sáng▾ màu chữ▾; Text Shadow
  (`a:effectLst/a:outerShdw` blurRad 38 100, dist 38 100, dir 45°, đen 43 % như tham chiếu ghi; engine vẽ bản
  bóng lệch dưới chữ); Paragraph thêm Columns▾ (1–3 cột, `bodyPr@numCol`), cột phải Text Direction▾ (`vert`)
  và Align Text▾ (`anchor`) — lệnh engine `TextFormat::Body`. Biểu tượng Clear Formatting là tẩy hồng tự vẽ.
- Tốc độ (bản release, `deck-300.pptx`, máy đang biên dịch song song 6 tiến trình rustc): mở 297 ms, khung
  đầu 774 ms; cuộn ngăn hình thu nhỏ trung vị 2,4 ms / p95 3,2 ms mỗi nấc (không chặn, không cần luồng
  riêng); gõ chữ tiêu đề tổng trung vị 12,9 ms (chuẩn bị ảnh 8,9 ms) — bản vòng 14 đo cùng lúc 13,0 ms, nên
  không thụt lùi; số vòng 14 (5,3 ms) đo lúc máy rảnh.

## Vòng 16 — menu chuột phải, ngăn Spelling, Insert / Review, chữ đậm giả lập (`slide-28…`, 1500 × 1102, 125 %)

Chụp lại cả mười tab cạnh tham chiếu (`slide-28…30`). Cửa sổ của ta mở bằng `BLUEOFFICE_SHOW_NOACTIVATE`
đôi khi nằm ở trạng thái thu nhỏ: kịch bản chụp khôi phục bằng `ShowWindow(SW_SHOWNOACTIVATE)` rồi đổi cỡ
có `SWP_SHOWWINDOW | SWP_FRAMECHANGED` và `RedrawWindow` — không thì `PrintWindow` trả khung cũ cỡ thu nhỏ.

| Đo (tham chiếu qua COM / UIA / PostMessage) | Tham chiếu | BlueSlide |
|---|---|---|
| Insert: nhóm thu về một nút (Links, Symbols, Media; Home > Editing) | không có nhãn nhóm | như vậy (trước: nhãn "Links", "Symbols"…) |
| Insert: dấu thả xuống | Pictures▾, Photo Album▾, 3D Models▾; SmartArt không | như vậy, kèm menu This Device… / New Photo Album… |
| Preview (Transitions / Animations), Delete comment | mờ khi trang không có hiệu ứng / bình luận | như vậy |
| Ngăn Spelling (thiết bị px từ đỉnh ngăn) | từ in đậm 20 px ở 61; nút cao 35 ở 92, cách lề 15, cách nhau 14; danh sách ở 144, cao 202, viền 148, hàng chọn 198 viền 148, hàng 40 px; Change / Change All dưới 10 px | như vậy |
| Chữ đậm giả lập (Calibri Light / Gabriola, b=1), bề rộng ký tự | 0,980–0,984 bề rộng thường (`n` 31,125 → 30,625 pt ở 60 pt; `W` 52,875 → 51,875) | ×0,982; nét dày thêm 0,032 em (thân chữ 4,4 → 7 px ở 75 px/em) |
| Tiêu đề "Báo cáo quý IV – đã chỉnh sửa" (Calibri Light đậm 60 pt, khung trong 705,6 pt) | một dòng, rộng 704,3 pt | một dòng (trước: hai dòng, 717,4 pt theo bề rộng thường) |

- Chuột phải: trên từ sai chính tả — tối đa năm gợi ý, Ignore All, Add to Dictionary, rồi Cut / Copy / Paste
  (sửa là một bước hoàn tác, lệnh engine mới `Engine::slide_spell_fix`); trên vùng chọn — Group / Ungroup,
  Bring to Front / Send to Back, New Comment; trên trang trống — Layout (menu mở tại con trỏ), Reset Slide,
  Format Background…; trên hình thu nhỏ — New / Duplicate / Delete / Hide Slide. Cut / Copy mờ khi không
  chọn gì. Trước vòng này BlueSlide không có menu chuột phải nào.
- Review > Proofing > Spelling (F7): ngăn Spelling bên phải đi qua lỗi của cả bản trình bày từ trang hiện tại,
  chọn từ bị đánh dấu trong hình của nó; Ignore Once / Ignore All / Add, danh sách gợi ý, Change / Change All
  (mọi chỗ cùng từ, một bước hoàn tác); hết lỗi thì "Spell check complete. You're good to go!".
- TypeScript: `SlideTextFormat` thêm `sizeStep`, `paragraph`, `shadow`, `body`; `SlideFormatState.shadow`.
- Insert (góp ý vòng này): biểu tượng tab Insert vẽ lại kiểu tô màu đặc như BlueDoc (`icons.rs` SOLID: xanh
  nhạt, vàng ấm, xanh, xanh lá; hình tự vẽ — đầu bảng xanh, dải đầu / chân trang vàng, chữ A xanh, dấu cộng
  xanh lá trên New Slide và Comment); Screenshot▾ trên Photo Album▾ (mờ: host chưa có ảnh các cửa sổ); cột
  nhỏ của nhóm Text có ba nút như tham chiếu — Date & Time, Slide Number (mở hộp Header and Footer), Object
  (sổ làm việc nhúng mới hoặc từ tệp, lệnh engine `OleInsert`); New Slide có dấu ▾ ở dòng hai. Nhãn
  Shapes / Icons / Comment cùng cách hàng nhãn nhóm 54 px như tham chiếu (lệch chung của cả dải là do thanh
  tiêu đề, đo ở vòng 8).
- Tốc độ (bản release, `deck-300.pptx`, máy đang chạy 5 tiến trình rustc của tác tử khác; đo cùng lúc với
  bản vòng 15): mở 311 ms (vòng 15: 297), khung đầu 683 ms (686); cuộn ngăn hình thu nhỏ trung vị 3,2 ms /
  p95 4,1 ms mỗi nấc (3,2 / 4,0); gõ chữ tiêu đề trung vị 15,3 ms, chuẩn bị ảnh 10,3 ms (15,9 / 10,9) — không
  thụt lùi; hình thu nhỏ vẫn không chặn luồng giao diện nên chưa cần luồng riêng.
- Review như tham chiếu (`slide-34`): Proofing (Spelling — biểu tượng tô màu tự vẽ; Thesaurus mờ), Accessibility
  (Check Accessibility▾: ngăn kiểm tra tự viết — trang thiếu tiêu đề, ảnh / biểu đồ / SmartArt / media / đối
  tượng thiếu văn bản thay thế, nhấp hàng tới hình), Language (Translate mờ; Language▾ đặt `rPr@lang` hoặc
  `noProof`, lệnh engine `TextFormat::Lang`), Comments có vạch ngăn trước Show Comments, Ink (Hide Ink mờ như
  tham chiếu khi bản trình bày không có mực).
- Ngăn Spelling (`slide-35`): nút ▾ cạnh ✕, chữ phím tắt gạch dưới (Alt+I / G / A / C / L), phần dưới: từ gợi ý
  in đậm và nút loa đọc bằng giọng Windows (mô-đun `reader_speech.rs` của BlueReader). Chưa có: nghĩa từ,
  hộp ngôn ngữ.
- Home > Paragraph (`slide-36`): cột phải ba nút Text Direction / Align Text / Convert to SmartArt (mỗi đoạn một
  nút sơ đồ, thay hộp chữ tại chỗ); cột "late" của ribbon được ba hàng cạnh hai hàng thường.
- Còn lại: Screenshot cần host chụp các cửa sổ khác (đang mờ); Thesaurus / Translate chưa có engine; Hide Ink
  chưa bật / tắt được (engine có `RenderOptions.hide_ink`, chưa nối vào trang hiển thị); menu chuột phải chưa
  có ảnh so với tham chiếu (menu bật lên là cửa sổ riêng); Draw: Undo, Ruler, Ink to Math, Ink Help; cửa sổ
  ta mở không kích hoạt đôi khi bị thu nhỏ / đóng giữa các lần chụp (không thấy sự kiện lỗi; có thể do kịch
  bản khác) — kịch bản chụp đã chịu được.

## Vòng 17 — Draw, Hide Ink, Animations, Slide Show, View, Spelling (`slide-37…`, 1500 × 1102, 125 %)

Tham chiếu được điều khiển qua COM / UI Automation (Invoke, Expand) trên bản sao; hộp thoại modal mở bằng
`ExecuteMso` trong một job nền rồi chụp bằng `PrintWindow` và đóng bằng `IDCANCEL`; thư viện thả xuống của
tham chiếu là cửa sổ lớp (layered) nên chụp màn hình vùng của nó (không nhập liệu). Kịch bản của ta
(`ours2.ps1`) đặt cỡ cửa sổ một lần rồi gửi nhấp / phím — kịch bản cũ đổi cỡ khi tưởng khung cũ và làm
đóng menu đang mở.

| Đo (thiết bị) | Tham chiếu | BlueSlide |
|---|---|---|
| Draw: nhóm | Tools (Eraser▾, Lasso Select), Undo, Pens (7 bút + Add▾), Stencils (Ruler), Convert (Ink to Shape, Ink to Math), Help | như vậy (6 bút tự vẽ: đen, đỏ, bút chì, đánh dấu vàng, xanh, xanh lá; Ink to Math mờ — không có bộ nhận dạng công thức), thêm nhóm Replay (Ink Replay); Ink Help mờ |
| Bút đang dùng, nhấp lần hai | bảng độ dày / màu | menu 5 độ dày (0,25–1 mm; bút đánh dấu 2–10 mm) + Pen Color (bảng màu) |
| Ruler | thước mờ 45° vượt ra ngoài trang, vạch hai cạnh, mặt số góc ở giữa | như vậy (hình tự vẽ): rộng 72 pt, dài 1,6 × đường chéo trang, vạch mm / 5 mm / cm, vòng tròn ghi góc; kéo để dời, bánh xe xoay 1° (hút 0 / 45 / 90°); nét bút bắt đầu cách cạnh ≤ 14 px chạy dọc cạnh |
| Review > Hide Ink | mờ khi không có mực; bật: mực biến khỏi trang và hình thu nhỏ, nút nhấn; menu Hide Ink / Delete All Ink on Slide / in Presentation | như vậy (`Engine::set_slide_hide_ink`, không lưu; trình chiếu cũng ẩn) |
| Thư viện hiệu ứng (More) | 5 cột, ô 124 × 84, mục None / Entrance / Emphasis / Exit / Motion Paths, dòng More … Effects, OLE Action Verbs mờ | như vậy; sao mỗi hiệu ứng có dấu riêng (tự vẽ), xanh lục xám #5A9E8C / vàng / đỏ; Emphasis đủ 19 như tham chiếu — 13 hiệu ứng mới trong catalog (`colorPulse`, `desaturate`, `darken`, `lighten`, `objectColor`, `complementary`, `lineColor`, `brushColor`, `fontColor`, `underline`, `boldFlash`, `boldReveal`, `waveText`: §19.5.2 animClr, §19.5.66 set, §19.5.1 anim; kiểm thử khứ hồi `gallery_emphasis_effects_round_trip`); Exit đủ 13 tên của tham chiếu (Disappear, Fly Out, Float Out, Shrink & Turn…); Motion Paths: Lines, Arcs, Turns, Shapes, Loops, Custom Path (Custom Path chưa vẽ được); các hiệu ứng còn lại của catalog trong hộp More … Effects (Basic / Subtle / Moderate / Exciting) |
| Animation Pane | hàng chọn có nút ▾ (Start On Click / With / After Previous, Remove) | như vậy (cả chuột phải trên hàng) |
| Số thứ tự trên hình (tab Animations) | ô 32 × 22 bên trái góc trên của hình, xếp chồng | như vậy, không vẽ khi thư viện / menu / hộp thoại mở (kiểm thử) |
| Notes Page | trang dọc, khung ghi chú nét đứt 46–92 % trang, "Click to add text", thanh trạng thái "Notes 1 of 4" | như vậy (ngăn hình thu nhỏ vẫn hiện) |
| Set Up Show | 702 × 587; Show type, Show options (bút, laser), Show slides, Advance slides, Multiple monitors | cùng kích thước và vị trí ô; nút chọn / ô đánh dấu kiểu Windows 11 màu nhấn BlueSlide, chữ lớn hơn 10 %, phím tắt gạch dưới, nút màu có thùng sơn tự vẽ; mặc định narration bật, Use Presenter View bật |
| Custom Shows / Define Custom Show | 500 × 260; New / Edit / Remove / Copy / Close / Show | như vậy; Define: tên, hai danh sách, Add >>, lên / xoá / xuống |
| Record ▾ | ghi từ trang hiện tại / từ đầu, Clear | Clear Timing on Current Slide / on All Slides; ghi âm mờ |
| Zoom | 280 × 359; Fit, 400…33 %, Percent | như vậy |
| Grid and Guides (nút ↘ nhóm Show) | Snap objects to grid, Spacing 1/12", Display grid, guides, smart guides | như vậy; kéo hình hút góc trên trái vào lưới |
| Outline View | Tab / Shift+Tab hạ / nâng cấp | Tab, Shift+Tab, Alt+Shift+→/←: đoạn xuống / lên cấp; đoạn cấp 1 nâng thành tiêu đề trang mới; tiêu đề hạ thành đoạn của trang trước (một bước hoàn tác, `applyOutline`) |
| Spelling (phần dưới) | nghĩa của từ gợi ý, ngôn ngữ ở chân ngăn | 3 nghĩa + từ đồng nghĩa (dữ liệu Open English WordNet sẵn có `assets/thesaurus/en`), tên ngôn ngữ kiểm tra |

- Kiểm thử: `slide_ink.rs` (Hide Ink), `slide_round17_tests.rs` (bút, Hide Ink, thứ tự vẽ thư viện / số thứ
  tự, Set Up Show, lưới), `slide_ruler.rs`, `slide_outline8.rs`, `slide_view8.rs`, `slide_thesaurus.rs`.
- Khứ hồi `p:timing` (Start / Duration / Delay, thứ tự, motion path) đã có kiểm thử trong
  `crates/bo-engine/tests/slide_show.rs`; vòng này không đổi engine hoạt hình.
- Máy chỉ có một màn hình: Presenter View vẫn trong cửa sổ.
- Còn lại: Notes Page / Reading View chưa so ảnh vòng này; Outline View của ta hẹp bằng ngăn hình thu nhỏ
  (tham chiếu rộng hơn); Split / Wheel… vẽ bằng catalog engine; hiệu ứng màu phát lại bằng màu đích cố định (không theo HSL);
  Custom Path chưa có công cụ vẽ; Record chỉ có Clear; Thesaurus trên Review vẫn mờ (dữ liệu đã có, chưa có ngăn).

## Vòng 18 — Notes Page, Outline View, Thesaurus (`slide-47…`, 1500 × 1102, 125 %)

Tiếp tục vòng 8 của loạt (agent trước bị ngắt): phần chưa commit của vòng 17 được commit, sửa lint của
clippy 1.99, gộp `work/desktop-performance` (BlueSheet vòng 10). Tham chiếu chụp qua COM trên bản sao
(`th.pptx` = bản sao `pp8.pptx` thêm hộp chữ tiếng Anh), của ta qua `PostMessage`.

| Đo (thiết bị) | Tham chiếu | BlueSlide |
|---|---|---|
| Notes Page: ngăn | không có ngăn hình thu nhỏ, không có ngăn ghi chú | như vậy (`SlideView::set_full_page`) |
| Notes Page: trang | 600 × 800, cách mép trên vùng 11 px; hình trang 480 × 270 ở 8,4 %; khung ghi chú nét đứt 46–92 % | 608 × 811, 11 px; 8,4 %; như vậy |
| Notes Page: thanh trạng thái | "Notes 1 of 4", 67 % | "Notes 1 of 4", 68 % (độ rộng trang / 7,5 in) |
| Notes Page: khung hình trang, chữ | viền 1 px đen; "Click to add text" đen, chữ hoa 625–633 | viền đen; chữ đen, 624–633 |
| Outline View: bố cục | dàn bài chiếm cả cửa sổ (không có trang bên cạnh), nền xám 230 | như vậy |
| Outline View: dòng | số trang x = 17, biểu tượng trang 27 × 23 viền 5 px (viền màu nhấn ở trang hiện tại, xám 210/213/216 ở trang khác), tiêu đề đậm x = 66; đoạn thân cùng x, mỗi cấp thụt 19 px, không có dấu đầu dòng nhưng giữ số tự động ("1.", "a)") và đậm / nghiêng / gạch dưới của run; chữ xám 68 (điểm tối nhất đo được); dòng 28–29 px, giữa hai trang thêm 11 px; đoạn rỗng không liệt kê | như vậy (`slide_outline_paras` của engine: số theo bộ đếm của bố cục chữ, run b/i/u; chữ 1,3 × cỡ giao diện, dòng 1,45 ×; viền màu nhấn BlueSlide) |
| Outline View: thanh trạng thái | "Outline" | "Outline" |
| Animation Pane | hàng của hình đang chọn trên trang được chọn (tô màu), tên hàng "Title 1: Báo cáo…" | như vậy |
| Thesaurus (Review, Shift+F7) | ngăn 245 px; tiêu đề + ∨ + ×; nút lùi 34 × 34 và hộp tìm có kính lúp; danh sách khung 350–775, hàng 27 px: "từ đồng nghĩa đầu (n.)" có ∨ gập, các từ đồng nghĩa thụt cùng cột; hộp ngôn ngữ "English (United States)" ở chân; hàng "decrease (Antonym)" | như vậy (dữ liệu Open English WordNet có sẵn, giấy phép CC BY 4.0 + thông báo Princeton trong `THIRD_PARTY_NOTICES.md`; nghĩa cùng tiêu đề gộp lại; nghĩa động từ chỉ có từ trái nghĩa vẫn liệt kê); nhấp một từ: tra từ đó (nút lùi quay lại); nhấp đúp: thay từ ở con trỏ; từ đồng nghĩa khác tham chiếu vì nguồn dữ liệu khác |

- Web: 13 hiệu ứng nhấn mạnh mới của catalog (vòng 17) thêm vào `SlideAnimEffect` và danh sách Emphasis của
  `AnimationPane` (nhãn `slideEditor.effect.*`).
- Kiểm thử: `slide_window_tests.rs` (Notes Page / Outline toàn cửa sổ, zoom trang ghi chú),
  `slide_round17_tests.rs` (Thesaurus: tra, lùi, gập), `slide_thespane.rs` (từ ở con trỏ, UTF-16).
- Outline View làm mờ Ruler / Gridlines / Guides / Notes và nhóm Color/Grayscale như tham chiếu.
- Còn lại: Outline View chưa sửa chữ tại chỗ và chỉ đánh số theo `buAutoNum` đặt trên đoạn (không theo
  kế thừa master); thanh trạng thái của ta chưa có mục ngôn ngữ / Accessibility (cả ở Normal); Thesaurus chưa
  có menu Insert / Copy khi rê chuột và chưa cuộn danh sách dài; Reading View chưa so ảnh.

## Vòng 19 — sửa dàn bài tại chỗ, thanh trạng thái, ngăn Accessibility, Reading View, tốc độ (`slide-54…`, 1500 × 1102, 125 %)

Tham chiếu điều khiển qua COM / UI Automation (Invoke) trên bản sao (`ol.pptx`, `a11y.pptx`, `rd.pptx`,
`anim.pptx`, `pp300.pptx`); của ta qua `PostMessage`. Không gửi nhập liệu thật.

| Đo (thiết bị) | Tham chiếu | BlueSlide |
|---|---|---|
| Outline: run b="1" trong đoạn thân | vẽ thường (bỏ đậm), giữ nghiêng / gạch dưới; tiêu đề đậm + nghiêng của run | như vậy (trước: "3. Bước ba" đậm) |
| Outline: số kế thừa từ bố cục (`buAutoNum` trên placeholder của layout) | "1.One" (không cách) | như vậy (`outline::outline_paras`: chồng kiểu danh sách slide → layout → master) |
| Outline: chữ sau số | tại max(độ thụt treo × 19 px / 0,5", bề rộng số): "1. Bước" (treo 0,5") / "1.One" (treo 0,25") | như vậy |
| Outline: sửa tại chỗ | con trỏ, gõ, Enter, Backspace, Tab | nhấp đặt con trỏ gần nhất; gõ; Enter tách đoạn (giữ cấp) / tách tiêu đề thành trang mới nhận các đoạn sau (đầu tiêu đề: trang trống trước); Backspace / Delete nối dòng (tiêu đề nối vào dòng cuối trang trước); ←→↑↓ Home End; mỗi thay đổi một bước hoàn tác (`applyOutline`) |
| Thanh trạng thái (sau "Slide 1 of 4") | biểu tượng kiểm lỗi +29, ngôn ngữ +23, "Accessibility: Investigate" đậm sau biểu tượng +23; phải: Notes, Comments kết thúc 25 px trước nút chế độ xem | như vậy; không sửa chữ: ngôn ngữ của giao diện ("English (United States)" như tham chiếu); đang sửa: `lang` của run; Outline / Notes Page / Sorter cũng có (không Notes / Comments) |
| Nhấp ngôn ngữ | hộp Language 454 × 416: Update What? Document / Slide / Selected Text, danh sách, ghi chú từ điển, Do not check spelling, Default… / OK / Cancel | như vậy (Default… mờ); OK đặt `rPr@lang` / `noProof` cho vùng chọn, trang hoặc cả bản (một bước hoàn tác) |
| Ngăn Accessibility | 400 px; thẻ "Keep going!" màu nhấn của ứng dụng; công tắc "Show only issues on current slide"; Color and Contrast / Media and Illustrations / Tables / Document Structure / Document Access, hàng 55 px; thanh cuộn cổ điển x 371 | như vậy (màu nhấn BlueSlide); kiểm tra trong engine `slide_edit::a11y_check` (12 quy tắc) |
| Kết quả trên `pp8` | Missing alt text = 1 (hình elip không chữ; mũi tên có chữ "Tiếp", bảng, hộp chữ không tính); tương phản đạt | như vậy sau sửa: hình vẽ không chữ, không phải placeholder cần alt; nền của chữ lấy `fillRef` của style (trước: coi nền trắng → chữ trắng trên mũi tên bị báo tương phản) |
| Animation Pane | ngăn xám 275 px, Play From 125 × 40, nút lên / xuống 40 × 40 (khung nhạt khi mờ), hàng 31 px, thanh thời gian: hộp 23 px viền đậm hơn 1 px, 20 px / giây từ 184 px trong danh sách; "After Previous" có vạch dọc tại điểm bắt đầu | như vậy; menu hàng: Start On Click / With / After Previous, Effect Options…, Timing… (hộp Start / Delay / Duration / Repeat), Hide / Show Advanced Timeline, Remove; Shift+nhấp: "Play Selected" |
| Thesaurus | rê chuột trên từ: dải sáng + nút ▾ (Insert / Copy); danh sách dài cuộn | như vậy: bánh xe 3 hàng, thanh cuộn (mũi tên, trang) |
| Reading View | thanh trạng thái: "Slide 1 of 4", ◀ ☰ ▶ tâm 126 / 82 / 38 px trước nút xem, không thu phóng, nút xem kết thúc cách mép phải 17 px | như vậy (lệch ≤ 1 px); ☰: Next / Previous / Edit View |
| Nút Slide Show / Fit (thanh trạng thái) | màn chiếu trên chân / khung | màn chiếu tự vẽ; khung với bốn mũi tên nhỏ hướng ra |

Tốc độ (`deck-300.pptx`, bản release, máy còn tiến trình rustc của tác tử khác):

| | Tham chiếu (COM) | BlueSlide |
|---|---|---|
| Mở | 1310 ms (Presentations.Open có cửa sổ) | mở 267 ms, khung đầu 746 ms |
| Cuộn ngăn hình thu nhỏ | — | trung vị 3,0 ms / p95 3,5 ms mỗi nấc |
| Gõ chữ tiêu đề (phím → hiện) | — (không gửi phím thật) | trung vị 6,9 ms / p95 16,6 ms (3 rustc khác đang chạy); sau khi bỏ kiểm tra giữa lúc gõ: 9,1 / 11,1 ms với 8 rustc khác (xử lý 2,7 ms, dải GPU 2,0 ms). Trước: 33,3 / 37,1 ms |
| Trình chiếu | bắt đầu 327 ms; Next trung vị 4,7 ms | phím tiến trung vị 1,4 ms |

- Hai chỗ chậm khi gõ, tìm bằng bảng chia thời gian mới của `BLUEOFFICE_NATIVE_PERF` (`"ensure"`: danh
  sách vẽ / dải thay đổi / GPU / bản giữ lại):
  1. kiểm tra trợ năng của thanh trạng thái chạy lại cả bản trình bày sau mỗi phím (≈ 15 ms trên 300
     trang): nay chỉ chạy khi bản trình bày không đổi trong 0,8 s;
  2. vẽ lại dải chữ của trang (11,3 ms) giải mã lại ảnh của trang mỗi phím: `bo-gpu-win` bỏ qua ảnh nằm
     ngoài dải (như các dòng chữ), host BlueSlide giữ ảnh đã giải mã theo id — dải còn 2,0 ms.
- Kiểm thử: `slide_round19_tests.rs` (gõ / Enter / Backspace / hoàn tác trong Outline, thanh trạng thái,
  ngăn và thanh cuộn, hộp Language), `slide_outline9.rs`, `slide_lang9.rs`, `slide_timing9.rs`,
  `bo-engine/tests/outline_paras.rs` (số kế thừa), `slide_edit::a11y_check` (tương phản, alt text, tiêu đề trùng).
- Còn lại: hộp Language chưa có biểu tượng từ điển trên hàng và danh sách chỉ 14 ngôn ngữ; Default… chưa
  đổi ngôn ngữ mặc định; chi tiết vấn đề trong ngăn Accessibility chưa có ô nhập mô tả tại chỗ (nút mở ngăn
  Alt Text); Play Selected phát từ hàng đầu tiên được chọn; dòng thời gian chưa có thước giây ở chân ngăn;
  Reading View vẫn ở trong cửa sổ chính (tham chiếu mở cửa sổ riêng "Slide Show"); menu Thesaurus và
  hộp Timing chưa so ảnh (cần rê chuột / hộp thoại nhiều trang của tham chiếu).
