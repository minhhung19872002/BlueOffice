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
