# BlueSheet giao diện Rust tự vẽ — so sánh với bảng tính desktop đối chứng

Mốc N6 (ADR-0018, `docs/plan/native-ui.md`): cửa sổ `bluesheet-native.exe` (`apps/native-win/src/sheet_main.rs`,
`sheet_app.rs`) trên `bo-ui` (`sheet_view.rs` lưới, `sheet_window.rs` khung) + `bo-gpu-win`, `SheetEngine` trong tiến trình.

Phương pháp như BlueDoc (`docs/ui/REPORT.md`): Windows 125 % DPI, cỡ chữ trợ năng 110 %; cùng một tệp (bản sao)
mở trong đối chứng qua COM và trong BlueSheet, chụp `PrintWindow` cùng kích thước cửa sổ 1500×1102 tại (40, 40),
đo màu / vị trí điểm ảnh bằng PIL. Đối chứng: bảng tính desktop cài trên máy (bản 2016/2019). Chỉ quan sát hộp đen —
không chép biểu tượng, phông, chuỗi; thanh tiêu đề giữ màu riêng của BlueSheet.

## Kiến trúc

- Lưới: `SheetEngine::render_viewport_with` (display list, không tiêu đề) → một bitmap GPU (`Device::draw_page`),
  vẽ lại **chỉ** khi ô / cuộn / kích thước / thu phóng đổi. Di chuyển vùng chọn, gõ trong ô không vẽ lại lưới
  (test `grid_image_is_redrawn_only_when_cells_or_scroll_change`).
- Tiêu đề cột/hàng, khung chọn, tô vùng chọn, ô đang sửa, thanh công thức, thẻ trang tính, thanh trạng thái vẽ
  mỗi khung bằng `Painter` (chữ DirectWrite kiểu GDI classic, ClearType như đối chứng).
- Chỉ hàng/cột nhìn thấy được dựng (`ViewLayout`); test 10 000 hàng: Ctrl+↓ tới hàng 10 000, display list < 2 000 lệnh.
- Hình học hiển thị: `SheetEngine::set_display_scale` → `SheetGeometry::for_display`: chiều cao hàng không đặt tay =
  dòng phông mặc định làm tròn điểm ảnh + 2 px ở tỉ lệ màn hình (Calibri 11 @120 dpi = 24 px = 14,4 pt — đúng giá trị
  đối chứng báo qua COM `RowHeight = 14.4`). In ấn / web giữ kích thước điểm của tệp.

## Vòng đo

### Vòng 1 — lưới (values-1.xlsx, 100 %)

| Hạng mục | Đối chứng | BlueSheet trước | BlueSheet sau |
|---|---|---|---|
| Đường lưới | #D4D4D4, 1 px | #DADCE0 (mặc định engine) | #D4D4D4 |
| Bước cột / hàng | 80 / 24 px | 80 / 25 px | 80 / 24 px |
| Nền tiêu đề / tiêu đề đang chọn | #E6E6E6 / #D2D2D2 | — | #E6E6E6 / #D2D2D2 |
| Viền tiêu đề–ô | #999999 | — | #999999 |
| Vạch giữa tiêu đề cột / hàng / cạnh vùng chọn | #C9C9C9 / #D0D0D0 / #B6B6B6 | — | như đối chứng |
| Chữ tiêu đề đang chọn | xanh #217346 | — | #217346 |
| Khung ô hiện hành | 2 px #217346 đè lên đường lưới bao quanh | thiếu cạnh trên/trái (tiêu đề vẽ đè) | khớp từng cột điểm ảnh |
| Cao dải tiêu đề cột / rộng cột số hàng | 24 / 34 px | 24 / 34 px | 24 / 34 px |
| Vị trí chữ tiêu đề cột | đỉnh chữ 6 px dưới mép dải | 7 px | 6 px |
| Đường cơ sở chữ ô (căn dưới) | 5 px trên đường lưới | 9 px | 5 px (bỏ khoảng line-gap của dòng cuối) |
| Số General quá rộng cột | bớt số lẻ (-0,00013), rồi dạng khoa học | `########` | như đối chứng (`general_fit`) |
| Chữ ô | ClearType | ClearType (`TextRendering::System`) | — |
| Thanh công thức | dải 54 px #E6E6E6, ô 30 px #FDFDFD viền #C6C6C6, hộp tên 127 px | dải 38 px | như đối chứng |

Khác biệt còn lại không thuộc giao diện: tệp mẫu có giá trị công thức lưu sẵn cũ — đối chứng tính lại khi mở
(C2 = 1235,5), engine giữ giá trị lưu (C2 = 1) theo quy tắc `fullCalcOnLoad`; ô kiểu ngày `t="d"` không định dạng
hiển thị chuỗi ISO thay vì số sê-ri. Ribbon dùng chung với BlueDoc (đo theo trình soạn thảo đối chứng) thấp hơn
ribbon bảng tính 1 px nên phần dưới lệch 1 px theo chiều dọc. Số dùng dấu phẩy thập phân (vi-VN) như đối chứng
trên máy này (`BLUEOFFICE_SHEET_LOCALE=en-US` để dùng dấu chấm).

### Vòng 2 — thẻ trang tính, thanh cuộn, thanh trạng thái, vùng chọn nhiều ô, độ trễ

| Hạng mục | Đối chứng | BlueSheet trước | BlueSheet sau |
|---|---|---|---|
| Thẻ đang chọn | trắng, cạnh #999999, rộng = nhãn + 22 px, chữ đậm xanh, vạch xanh 2 px | rộng nhãn + 44 px, chữ thấp 5 px | như đối chứng (nhãn cùng bề rộng mực 50 px) |
| Thẻ đầu tiên | x = 112 px | 85 px | 112 px |
| Thanh cuộn | cột 25 px #E6E6E6, hộp mũi tên 19 px trắng viền #949494, rãnh #F3F3F3, con trượt trắng viền #949494 | dải xám 18 px không mũi tên; thanh ngang bị dải thẻ vẽ đè | như đối chứng |
| Thanh trạng thái | "Ready" x = 15 px; nút xem 54 px (nút hiện hành #C6C6C6); − tâm 268 px, + tâm 91 px, % 65 px từ mép phải | x = 19 px; nút 38 px | như đối chứng |
| Tô vùng chọn | ô trắng → #C6C6C6, lưới → #A5A5A5, vòng trắng 1 px trong khung | #E0E0E0 / #BABABA, không vòng | #C6C6C6 / #A5A5A5, có vòng (khớp từng điểm ảnh hàng 3) |

Độ trễ phím (cùng tệp, 30 phím gửi `PostMessage` vào cửa sổ, đo bằng `PrintWindow` liên tục — mỗi lần chụp ~29 ms
nên độ phân giải ±29 ms; không dùng SendInput, không đổi cửa sổ đang có focus):

| Thao tác | Đối chứng trung vị / p95 | BlueSheet trung vị / p95 | BlueSheet nội bộ (`BLUEOFFICE_NATIVE_PERF`) |
|---|---|---|---|
| Gõ chữ vào ô (WM_CHAR) | 58,1 / 65,6 ms | 27,6 / 43,4 ms | xử lý 0,43 ms, vẽ 1,15 ms, phím→present trung vị 2,1 ms (p95 16,8 ms = chờ vsync); lưới không vẽ lại |
| Mũi tên ↓/↑ | 36,0 / 63,1 ms | 34,4 / 91,5 ms | xử lý 0,08 ms, vẽ 1,0 ms, trung vị 1,5 ms |

### Vòng 3 — styles-1.xlsx (phông, tô, viền, xoay chữ)

| Hạng mục | Đối chứng | BlueSheet trước | BlueSheet sau |
|---|---|---|---|
| Hàng không lưu chiều cao có chữ dọc / xuống dòng / phông lớn | tự giãn khi mở (hàng 4: 217 px) | 24 px, chữ bị cắt | tự giãn (`SheetGeometry::for_display` → `fit_rows`; chỉ quét khi có kiểu ô có thể làm hàng cao hơn) |
| Ngày không định dạng riêng | theo vùng Windows (M/d/yyyy) | theo locale số vi-VN (dd/mm/yyyy) | giữ (BlueSheet hiển thị kiểu Việt Nam) |

Còn khác (engine, chưa sửa): tô mẫu (pattern) vẽ thành màu đặc; tô chuyển sắc chỉ lấy điểm dừng đầu.

### Vòng 4 — tốc độ (10 000 hàng × 7 cột, cột E là công thức)

Cuộn bằng bánh xe (40 nấc, `WM_MOUSEWHEEL` gửi vào cửa sổ, 3 hàng/nấc, đo bằng `PrintWindow` ±25 ms):

| | Đối chứng trung vị / p95 | BlueSheet trung vị / p95 |
|---|---|---|
| Nấc bánh xe → khung đổi | 34,0 / 41,2 ms | 32,7 / 68,5 ms |

Nhật ký nội bộ của BlueSheet cho mỗi nấc: xử lý (cuộn + dựng tiêu đề) trung vị 2,7 ms, vẽ giao diện 1,0 ms,
vẽ lưới GPU + present ~10 ms; chỉ các hàng nhìn thấy được dựng (test `scrolling_ten_thousand_rows_lays_out_only_visible_rows`).
Đuôi p95 cao hơn đối chứng: lần vẽ lại lưới GPU (`draw_page` cả vùng) chưa vẽ theo dải — việc còn lại: dời ảnh lưới
và chỉ vẽ các hàng mới lộ ra khi cuộn.

Bản phát hành cuối (LTO, `out/desktop/bluesheet-native.exe`): gõ chữ vào ô 25 phím — màn hình đổi trung vị 25,8 ms /
p95 29,0 ms (đối chứng 58,1 / 65,6 ms, cùng phương pháp); nội bộ: xử lý 0,14 ms, vẽ 1,0 ms, phím→present 1,5 ms (p95 4,9 ms).

## Còn lại

- Vẽ lại lưới theo dải khi cuộn (dời ảnh cũ, chỉ vẽ hàng mới) để hạ đuôi p95 khi cuộn.
- Hộp định dạng số (General…), Styles, Editing (AutoSum, Fill, Clear, Sort & Filter, Find) của tab Home; các tab khác.
- Thanh công thức: hộp tên có mũi tên thả, sửa nhiều dòng; tham chiếu ô bằng chuột khi gõ công thức.
- Kéo tay nắm điền (fill handle), nhấp đúp mép cột để AutoFit, menu chuột phải, hộp thoại Format Cells.
- Engine: tính lại công thức khi tệp có giá trị lưu cũ như đối chứng, tô mẫu / chuyển sắc, ô `t="d"`.

## Vòng 2 (N6 tiếp) — tab, menu, tương tác lưới, engine

Nhánh `worktree-agent-a868e14eb3e9a35fe` (từ `work/desktop-performance` 046f18b5). Ảnh chụp và số đo trong
thư mục scratch của phiên (`sheet2/`): `fills-x/b.png`, `cf1-x/b.png`, `ribbon-cmp.png`, `numfmt.png`, `ctx.png`, `point.png`.

### Vòng 5 — cuộn bằng cách dời ảnh lưới

`GridStore::scroll` + `Device::copy_page_rects` (bo-gpu-win, thêm mới): khi chỉ cuộn đúng số nguyên điểm ảnh theo
một hướng, phần còn thấy của ảnh lưới được chép sang bitmap thứ hai trên GPU (`CopyFromBitmap`), chỉ dải hàng/cột mới
lộ ra được vẽ (`draw_page` theo dải); hàng/cột cố định giữ nguyên, dải 4 px quanh đường cố định vẽ lại. Kiểm bằng
rasterizer CPU (`bo-raster`): ghép ảnh sau 12 lần cuộn khác ảnh vẽ cả trang < 1/20 000 điểm ảnh (mép glyph nằm ở nửa
điểm ảnh), cả khi có cố định hàng/cột (`sheet_window_tests.rs`). `BLUEOFFICE_SHEET_BLIT=0` tắt để so.

Nhấp bánh xe trên `big10k.xlsx` (10 000 hàng), 40 nấc `WM_MOUSEWHEEL` gửi vào cửa sổ:

| Cách đo | Đối chứng trung vị / p95 | BlueSheet trước (vòng 1) | BlueSheet sau |
|---|---|---|---|
| `PrintWindow` 1500×1102 (như vòng 1) | 34,0 / 41,2 (vòng 1) | 30,5 / 80,7 | 28,2–28,8 / 55,6–82,6 |
| `PrintWindow` 900×700 (mỗi lần chụp ~15 ms) | 27,9 / 29,2 · 27,8 / 28,6 | 22,0 / 49,9 · 45,9 / 51,1 | 24,8 / 50,6 · 43,1 / 53,3 |
| Lấy mẫu màn hình đã ghép (BitBlt vùng 500×120, ~7 ms/mẫu) | — (không đưa được cửa sổ đối chứng mở qua COM lên trên) | 15,1 / 19,7 · 15,0 / 17,5 | 15,2 / 21,1 · 15,1 / 17,2 |
| Nội bộ (`BLUEOFFICE_NATIVE_PERF`): vẽ lưới CPU / phím→present | — | — | 3,1–3,5 ms / 6,6–7,3 ms (trung vị) |

Đọc số: trên màn hình thật BlueSheet đổi hình sau ~1 khung (15 ms trung vị, p95 ≤ 21 ms). `PrintWindow` phạt cửa sổ
swap chain flip: ảnh mới chỉ thấy được sau khi DWM ghép, nên kết quả hai mức (~22 ms hoặc ~46 ms = một lần chụp nữa);
đối chứng vẽ GDI nên lần chụp đầu đã thấy (28 ms). Dời ảnh không đổi thời gian CPU (3,1 ms với/không — phần lớn là dựng
display list và chuẩn bị glyph), chỉ bớt việc vẽ GPU; ở bảng trên với/không dời ảnh cho cùng số trong sai số đo.
Host: khi hệ thống đòi vẽ (chụp, lộ cửa sổ) và đang có khung chờ, khung đó được vẽ một lần (trước đây vẽ hai lần,
present hai lần) và được ghi log; `grid_ms` cộng cả thời gian vẽ lưới trong các lần vẽ đó.

### Vòng 6 — ribbon: Home đầy đủ và các tab Insert, Formulas, Data, View

`sheet_ribbon.rs` (tab, nhóm, menu) + `sheet_commands.rs` (lệnh). So `ribbon-cmp.png` (trên: đối chứng, dưới: BlueSheet):

| Nhóm | Đối chứng | BlueSheet |
|---|---|---|
| Number | hộp “General” 120 px + mũi tên; hàng $ % ,; hàng tăng/giảm số lẻ | hộp vẽ đè lên chỗ của mục có nhãn (152 px), danh sách General…Text + “Other number formats…”, dấu chọn ở loại hiện hành; 3 hàng như đối chứng |
| Styles | 3 hàng có nhãn + ▾ | 3 hàng có nhãn + ▾: quy tắc định dạng có điều kiện (trên/dưới trung bình, trùng, top/bottom 10, thanh dữ liệu, thang màu, bộ biểu tượng, xoá quy tắc), kiểu bảng, kiểu ô (`NAMED_STYLES`) |
| Cells | Insert ▾ / Delete ▾ / Format ▾ | như đối chứng (chèn/xoá hàng, cột, trang tính; AutoFit, ẩn/hiện, Format Cells…) |
| Editing | hai cột nút biểu tượng có ▾ (Σ, ↓, tẩy / sắp xếp, tìm) | như đối chứng: AutoSum (Sum/Average/Count/Max/Min, theo lệnh `autoSum` của engine), Fill (xuống/phải/lên/trái, chuỗi), Clear (tất cả/định dạng/nội dung), Sort & Filter, Find & Select (Find, Replace, Go To) |

Tab khác: Insert (bảng, biểu đồ cột/thanh, đường/vùng, tròn/vành khuyên), Formulas (chèn hàm, AutoSum, thư viện hàm theo
nhóm — chỉ hàm engine có, tính ngay), Data (A→Z, Z→A, lọc, tách văn bản theo , ; tab, loại trùng), View (đường lưới,
tiêu đề, thanh công thức, 100 %, thu phóng theo vùng chọn, cố định ô/hàng đầu/cột đầu/bỏ). Biểu tượng mới là hình vẽ
riêng (Σ, fx, máy tính, tách cột, tiêu đề, thanh công thức) hoặc biểu tượng sẵn có cùng nghĩa.
Khác còn lại: thứ tự tab chưa có Draw / Page Layout / Review / Help; nút “Calculation Options” (thủ công) chưa có vì
engine luôn tính lại; Text to Columns không có trình hướng dẫn.

### Vòng 7 — tương tác lưới

| Hạng mục | Kết quả (test headless trong `sheet_window_tests.rs`) |
|---|---|
| Kéo tay nắm điền | khung xám theo hướng kéo xa nhất, thả = `autofill` (1, 2 → … 5) |
| Nhấp đúp mép tiêu đề cột/hàng | `autofit` cột/hàng đó |
| Menu chuột phải | ô / cả hàng / cả cột: cắt, chép, dán · chèn, xoá, xoá nội dung · sắp xếp hoặc AutoFit/ẩn/hiện · Format Cells… (`ctx.png`) |
| Hộp thoại | Find (chọn ô khớp tiếp theo), Replace (Replace all), Go To (tham chiếu/tên qua `resolve`), Insert Function, Format Cells cơ bản (mã định dạng số, phông, cỡ, màu chữ, màu tô, căn ngang, đậm, nghiêng, xuống dòng, viền quanh) — dùng `Dialog` sẵn có |
| Trỏ ô khi gõ công thức | sau `=`, toán tử, `(`, `,`: nhấp chèn tham chiếu, kéo mở rộng thành vùng; khung màu cho từng tham chiếu (`point.png`: `=SUM(H4:H5)+D1`, khung xanh/cam) |

Màu khung tham chiếu chưa đo được trên đối chứng (gửi F2 bằng `PostMessage` không mở chế độ sửa khi cửa sổ không có
focus); bảng màu hiện là của BlueSheet.

### Vòng 8 — engine (thấy trên màn hình), `fills.xlsx` tự sinh + `cf-1.xlsx`

| Hạng mục | Đối chứng | BlueSheet trước | BlueSheet sau |
|---|---|---|---|
| Công thức có giá trị lưu cũ (`=SUM(H4:H5)` lưu 1) | tính lại khi mở: 1234,5 | 1 | 1234,5 (`set_recalc_on_open`, chỉ bản native; web giữ quy tắc `fullCalcOnLoad`) |
| Ô `t="d"` kiểu General | số sê-ri 45356 | chuỗi ISO | 45356; có định dạng ngày → ngày |
| Ngày định dạng dựng sẵn 14 | theo vùng Windows: 3/5/2024 | 05/03/2024 | 3/5/2024 (`set_region_dates` đọc `sShortDate`/`sTimeFormat`) |
| Tô mẫu (16 kiểu) | ảnh mẫu 4 px theo điểm ảnh | màu tiền cảnh đặc | xám/caro = mật độ trộn màu; sọc/lưới nhạt = đường 1–2 px mỗi 4 px trên nền |
| Tô chuyển sắc | tuyến tính theo độ, “path” toả từ tâm | điểm dừng đầu | tuyến tính / toả tròn như đối chứng |
| Bộ biểu tượng 5 phần tư | đĩa trắng viền #505050, phần tô #505050, 20 px, cách mép ô 2 px | đĩa xám không viền 16 px | như đối chứng (x 366–384 so với 365–384: ±1 px do khử răng cưa) |
| Thanh dữ liệu | x 284–343, y 1–20 (hàng 24 px) | x 285–343, y 2–21 | x 284–343, y 1–20 (khớp từng điểm ảnh) |
| Cột có độ rộng lưu (16 ký tự) | 144 px | 140 px (112 px × 1,25) | 144 px (`for_display` dùng độ rộng chữ số ở tỉ lệ màn hình, 9 px; 100 % và in/web không đổi) |

Độ dài thanh dữ liệu đã khớp từ trước (sai khác ghi ở phiên chính là do so khác hàng). Thay đổi trong `bo-sheet`
(tô mẫu, chuyển sắc, biểu tượng, `t="d"`) áp dụng cả bản web.

Còn khác: vạch mẫu chéo vẽ bằng đường khử răng cưa thay vì điểm ảnh; “dark grid” / “trellis” là màu trộn thay vì ô caro.

### Bản phát hành cuối vòng 2 (LTO, `out/desktop/bluesheet-native.exe`, `big10k.xlsx`, 30 lần mỗi phép)

| Thao tác | Đối chứng trung vị / p95 | BlueSheet trung vị / p95 |
|---|---|---|
| Gõ chữ vào ô, `PrintWindow` 900×700 | 42,1 / 62,5 ms | 26,7 / 49,0 ms |
| Nấc bánh xe, `PrintWindow` 900×700 | 27,9 / 33,6 ms | 23,5 / 42,0 ms |
| Nấc bánh xe, lấy mẫu màn hình | — | 15,4 / 21,7 ms |

## Còn lại (sau vòng 2)

- Đo độ trễ bánh xe của đối chứng trên màn hình thật (cửa sổ mở qua COM không lên trên được khi không có focus);
  `PrintWindow` không công bằng với swap chain flip.
- Màu khung tham chiếu khi sửa công thức chưa đo trên đối chứng; chế độ “Point” bằng phím mũi tên chưa có.
- Hộp định dạng số rộng 152 px (đối chứng 120 px) vì chiếm chỗ một mục có nhãn của ribbon dùng chung.
- Tab Draw / Page Layout / Review / Help; tính toán thủ công; trình hướng dẫn tách cột; hộp thoại Format Cells có thẻ
  (Number / Alignment / Font / Border / Fill) như đối chứng — hiện là một hộp thoại trường văn bản.
- Thanh công thức: hộp tên có mũi tên thả, sửa nhiều dòng.

## Vòng 3 — độ trễ đo trên màn hình thật, thanh công thức, ribbon theo bề rộng, các tab còn lại

Nhánh `worktree-agent-a835fa1460ab75a86` (từ `work/desktop-performance` eb3dac0c). Ảnh chụp và số đo trong thư mục
scratch của phiên (`sheet3/`): `cmp-refs.png` (khung tham chiếu, trên: đối chứng, dưới: BlueSheet), `cmp-w1.png` /
`cmp-w2.png` (Home ở 1920 px), `cmp-n.png` (Home ở 1500 px), `xh-all.png` (Home của đối chứng ở 1600/1700/1800 px),
`xw-*.png` (mọi tab của đối chứng ở 1920 px), `tab-*.png` (từng tab, đối chứng trên / BlueSheet dưới),
`xfb-3c.png` (thanh công thức mở rộng), `xfb-namesc.png` (danh sách tên), `xnote.png` (dấu chú thích).

Cách đo mới, không gửi input thật và không đổi cửa sổ đang có focus của người dùng (kiểm tra sau mỗi lần chạy):
- Đối chứng tạo bằng COM trong chính tiến trình đo (không `GetActiveObject`, chỉ đóng phiên bản đó). Cửa sổ của nó bỏ
  `HWND_TOPMOST` (`SetWindowPos` trả `true` nhưng kiểu cửa sổ không có `WS_EX_TOPMOST`), nên `thumb.ps1` dựng một cửa sổ
  nhỏ của phiên đo — luôn trên cùng, không bao giờ kích hoạt — hiển thị **ảnh thu nhỏ DWM sống** (`DwmRegisterThumbnail`)
  của vùng lưới cửa sổ đích, rồi lấy mẫu vùng màn hình dưới nó bằng BitBlt. Hai ứng dụng đi qua cùng một đường DWM.
- Chế độ sửa ô của đối chứng mở được bằng nhấp đúp gửi qua `PostMessage` vào cửa sổ lưới (F2 gửi vào thì không).
- Đổi tab ribbon của đối chứng: nhấp gửi vào cửa sổ `NetUIHWND` của ribbon.
- BlueSheet mở với `BLUEOFFICE_SHOW_NOACTIVATE=1` (cửa sổ hiện không lấy focus; thêm vào `win32.rs`, chỉ khi có biến).
- Máy: màn hình 144 Hz (một khung 6,9 ms) trong phần lớn phiên, GPU NVIDIA RTX 2050; cuối phiên màn hình về 60 Hz
  (các lần đo đó ghi riêng).

### Vòng 9 — độ trễ bánh xe (`big10k.xlsx`, `WM_MOUSEWHEEL` gửi vào cửa sổ, 30–40 nấc, 144 Hz)

| Cửa sổ / nhịp nấc | Đối chứng trung vị / p95 | BlueSheet vòng 2 | BlueSheet vòng 3 |
|---|---|---|---|
| 900×700, nấc cách 120 ms | 8,1 / 14,0 ms | 16,8 / 18,0 | 14,6 / 15,3 |
| 1500×1000, nấc cách 120 ms | 8,0 / 13,6 | 24,6 / 25,8 | 19,9 / 26,9 |
| 900×700, nấc cách 15 ms (cuộn liên tục) | 22,1 / 29,1 | — | 15,2 / 16,7 |

Ở 60 Hz (cuối phiên, mỗi mẫu tốn 16 ms nên độ phân giải ±16 ms): đối chứng 16,9 / 18,9 ms (900×700) và 17,1 / 22,2
(1500×1000); BlueSheet vòng 2 18,9 / 21,1, vòng 3 19,0 / 33,7 (một lần trễ 34–40 ms ở nấc thứ 3–4) và 21,0 / 23,0
(1500×1000); cuộn liên tục: đối chứng 50,6 / 66,7 ms (dồn nấc), BlueSheet 18,2 / 20,0.

Nhật ký nội bộ (`BLUEOFFICE_NATIVE_GPU=1` đợi GPU xong sau present — chỉ để đo, nó làm chậm luồng): ở 1500×1000 một khung
cuộn tốn 6,1 ms GPU, tăng theo số điểm ảnh (GPU ở xung nhịp thấp giữa các nấc, giới hạn bởi băng thông bộ nhớ); CPU
~4 ms (xử lý 2,4, lưới 1,0, vẽ giao diện 1,0). Đã làm:
- Thanh tiêu đề + ribbon vẽ vào một ảnh GPU riêng (`Device::create_layer` / `draw_layer`, thêm mới), chỉ vẽ lại khi
  băm các lệnh vẽ của chúng (`bo_ui::PaintHash`) đổi; vẽ giao diện CPU 1,0 → 0,3 ms.
- Không xoá toàn cửa sổ mỗi khung (lưới + ảnh ribbon + phần phủ đã che kín): GPU mỗi khung 6,1 → 3,9 ms, trung vị
  1500×1000 24,6 → 19,9 ms.
- Đã thử: sync interval 0 (`WindowSurface::set_sync_interval`, thêm mới) — trên màn hình như 1 (15,0 / 15,8 so với
  14,6 / 15,3 ms) nhưng `PrintWindow` khi đó có thể chụp một khung đang vẽ, nên mặc định giữ 1
  (`BLUEOFFICE_SHEET_SYNC=0` để thử); swap chain bit-block-transfer (16,0 / 23,4 ms ở 900×700) — không tốt hơn flip, bỏ.

Đọc số: đối chứng vẽ GDI trên CPU, DWM ghép ở lần vblank kế tiếp (~1 khung 144 Hz). BlueSheet cần CPU ~4 ms + GPU
2–4 ms rồi mới được ghép: trung vị ~2 khung; p95 gần đối chứng ở 900×700 (15,3 so với 14,0 ms), và khi cuộn liên tục
BlueSheet nhanh hơn rõ (đối chứng dồn các nấc: 22 / 29 ms ở 144 Hz, 51 / 67 ms ở 60 Hz). Việc còn lại để hạ thêm ở cửa
sổ lớn: ảnh lưới dạng vòng (khi cuộn không chép cả ảnh sang ảnh thứ hai — bước chép chiếm ~2 ms GPU ở 1500×1000).

### Vòng 10 — khung tham chiếu, trỏ ô bằng phím, thanh công thức

Khung tham chiếu đo trên đối chứng (`=A1+B2:C3+A5+D2+E6+F8+B10+D11+A3`, `cmp-refs.png`):

| Hạng mục | Đối chứng | BlueSheet trước | BlueSheet sau |
|---|---|---|---|
| Màu (theo thứ tự tham chiếu, lặp sau 7) | #326AC7 #C0353E #8157B7 #007C20 #B03E84 #B64900 #267392 | #4472C4 #ED7D31 #7030A0 #00B050 #C00000 #0070C0 #BF8F00 | như đối chứng |
| Khung | 2 px: đường lưới + 1 px ra ngoài | 2 px | như đối chứng |
| Bên trong | vòng trắng 1 px, phủ 10 % màu khung (cả đường lưới và chữ) | không | như đối chứng |
| Góc | ô vuông 5 px màu khung, viền trắng 1 px, ở cả 4 góc | không | như đối chứng |
| Chữ tham chiếu trong ô đang sửa | cùng màu khung; toán tử đen | một màu | như đối chứng |

- Gõ công thức, sau toán tử / `(` / `,`: mũi tên trỏ ô (lần đầu: ô cạnh ô đang sửa, sau đó dời ô được trỏ), Shift mở
  rộng thành vùng, gõ toán tử rồi mũi tên lại bắt đầu từ ô đang sửa; thanh trạng thái: Point / Enter / Edit.
- Hộp tên (`sheet_fbar.rs`): mũi tên ▾ 7×4 px #838688 cách mép phải 15 px liệt kê các tên đã định nghĩa (chọn → chọn
  vùng); nhấp vào hộp chọn sẵn chữ, gõ tham chiếu hoặc tên rồi Enter để tới, Esc thôi. Danh sách đối chứng: hộp trắng
  125 px, mỗi tên 25 px (`xfb-namesc.png`); BlueSheet dùng menu sẵn có.
- Ba chấm ngăn 3×3 px #B3B3B3 cách nhau 5 px; hộp 137 × 30 px quanh dấu huỷ / nhập / fx; nút mở rộng (chữ V đậm
  9×6 px #777777, cách mép phải 15 px): thanh công thức hiện 3 dòng, cao thêm 48 px (đo `FormulaBarHeight = 3`).
  So từng cột điểm ảnh ở 1500 px: hộp tên 15–141, hộp dấu 179–315, hộp nội dung 319–1484 — khớp đối chứng.

### Vòng 11 — Home theo bề rộng cửa sổ, tab Page Layout / Review, hộp thoại

Đối chứng thu gọn nhóm Home theo bề rộng (`xh-all.png`): từ ~1800 px hộp định dạng số rộng 196 px trên một hàng nút số,
Styles là ba nút lớn, Sort & Filter / Find & Select là nút lớn sau cột biểu tượng Σ / Fill / Clear, nhãn nút lớn hai
dòng; ở 1500 px hộp số 121 px, Styles ba hàng có nhãn, Editing hai cột biểu tượng. BlueSheet chọn cách bày rộng nếu vừa
cửa sổ, không thì bày gọn (`sheet_ribbon::tabs_wide` / `tabs`; ribbon thêm quy tắc chọn dùng `with_labeled_widths`,
`with_trailing`, `with_two_line_labels`, `set_tabs` — BlueDoc không đổi). Đo: hộp số 196 / 121 px như đối chứng.

| Tab | Đối chứng (1920 px) | BlueSheet |
|---|---|---|
| Thứ tự | Home, Insert, Draw, Page Layout, Formulas, Data, Review, View, Help | Home, Insert, Page Layout, Formulas, Data, Review, View |
| Page Layout | Themes; Margins, Orientation, Size, Print Area, Breaks, Background, Print Titles; Scale to Fit; Sheet Options; Arrange | Margins (Normal/Wide/Narrow), Orientation, Size (Letter/Legal/A3/A4/A5), Print Area (đặt/xoá), Breaks (chèn/bỏ/đặt lại); Width / Height / Scale; xem/in đường lưới và tiêu đề |
| Formulas | … Calculation Options ▾, Calculate Now, Calculate Sheet | Calculation Options (Automatic / Manual, lưu calcPr), Calculate Now / Sheet; F9 / Shift+F9; thanh trạng thái “Calculate” khi công thức chờ tính như đối chứng |
| Review | Proofing, Accessibility, Language; New Comment, Delete, Previous, Next, Show All; Protect Sheet / Workbook, Allow Edit Ranges | New Comment, Delete, Previous, Next (chú thích theo luồng); Protect Sheet (mật khẩu tuỳ chọn, ô khoá từ chối sửa) / Protect Workbook |

- Ô có chú thích: tam giác vuông đỏ #FF0000 cạnh 7 px trong góc trên-phải (đo trên đối chứng, `xnote.png`).
- Format Cells có 6 thẻ như đối chứng (Number, Alignment, Font, Border, Fill, Protection) — `Dialog::with_tabs`
  thêm dải thẻ cho hộp thoại bo-ui (tuỳ chọn; Ctrl+Tab / Ctrl+PageDown chuyển thẻ). Các trường vẫn là ô nhập / ô đánh
  dấu của hộp thoại chung (không có danh sách loại số, mẫu viền vẽ sẵn, bảng màu như đối chứng).
- Text to Columns: trình hướng dẫn 2 bước (kiểu dữ liệu, dấu phân cách / điểm ngắt, coi dấu liền nhau là một; rồi xem
  trước các hàng đầu do engine tách) thay cho tách thẳng theo , ; tab.

### Kiểm tra bản web (thay đổi engine vòng 2)

`pnpm build:wasm` (13 phút 41 giây), `pnpm --filter @blueoffice/sheet-editor test`: 44 tệp / 307 test đạt;
`pnpm --filter @blueoffice/platform test`: 27/28 tệp đạt ở lần đầu — `parity.test.ts` quá thời gian vì tự build
`bo-cli` trong thư mục `target` của worktree; build `bo-cli` trước rồi chạy lại: đạt (2/2). `@blueoffice/i18n`: đạt.

## Còn lại (sau vòng 3)

- Bánh xe ở cửa sổ lớn: ảnh lưới dạng vòng để không chép cả ảnh mỗi nấc; trung vị vẫn ~2 khung so với ~1 khung của
  đối chứng khi các nấc cách xa nhau.
- Format Cells: danh sách loại số và mẫu, ô chọn viền vẽ sẵn, bảng màu; Text to Columns: định dạng từng cột, đích khác.
- Hộp tên khi sửa công thức (đối chứng hiện danh sách hàm, vd SUM); danh sách tên vẽ như hộp của đối chứng.
- Tab Draw / Help; nhóm Defined Names / Formula Auditing của Formulas (engine có `defineName`, `createNames`,
  `traceArrows`); Page Layout: Themes, Background, Print Titles, Arrange; Review: Show All Comments, thẻ hiện chú thích
  khi rê chuột.
- Ô đánh dấu “View / Print” của Sheet Options đang là nút bật/tắt có nhãn.

## Vòng 4 — cuộn vòng, hộp thoại tự vẽ, Formulas / Data / Review / Help

Nhánh `worktree-agent-af1e12dc87f070ae8` (từ `work/desktop-performance` 8d99794b). Ảnh chụp và số đo trong thư mục
scratch của phiên (`sheet4/`): `xfc-0.png`, `xd-43-381.png`, `xd-45-84.png` (Format Cells của đối chứng: Number,
Alignment / Font, Border / Fill), `xnote-all.png` (thẻ chú thích), `ring-runs.txt` (độ trễ bánh xe), `cmp-*.png`
(đối chứng trên / BlueSheet dưới).

Cách mở hộp thoại của đối chứng không gửi input thật: tiến trình con tạo phiên bản COM riêng rồi gọi
`Application.Dialogs(id).Show()` (chặn trong tiến trình con); tiến trình đo tìm cửa sổ `bosa_sdm_XL9` của phiên bản đó,
chụp `PrintWindow`, đóng bằng `WM_CLOSE` (`xdlgs.ps1`, `xshow.ps1`). Nhấp gửi vào hộp thoại `bosa_sdm` không ổn định
(lần chụp lỡ khung đang vẽ) nên mỗi thẻ mở bằng id hộp thoại riêng (42 Number, 43 Alignment, 381 Font, 45 Border,
84 Fill, 46 Protection).

### Vòng 12 — bánh xe: ảnh lưới dạng vòng, chỉ chuẩn bị chữ trong dải, thanh cuộn lưu đệm

- `GridRing` (bo-ui): ảnh lưới lưu theo pha (px, py); cuộn chỉ đổi pha và vẽ dải mới lộ ra vào đúng chỗ trong vòng
  (`Device::draw_page_at`, bo-gpu-win, thêm mới: vẽ một dải display list dời đi số nguyên điểm ảnh), khi hiện ghép tối
  đa 4 mảnh (`Frame::draw_page_rect`). Không còn chép cả ảnh sang ảnh thứ hai mỗi nấc. Hàng/cột cố định theo trục cuộn
  vẫn dùng cách dời ảnh cũ. Test rasterizer CPU: 17 lần cuộn (bánh xe + cột) qua vòng = vẽ cả trang
  (`scrolling_turns_a_ring_image_without_copying`). `BLUEOFFICE_SHEET_RING=0` tắt để so.
- Vẽ theo dải trên GPU bỏ qua các lượt chữ nằm hẳn ngoài dải (trước đây bước 1 chuẩn bị glyph run DirectWrite cho mọi
  ô của trang dù chỉ vẽ 3 hàng).
- Thanh cuộn: `SheetEngine::content_size` duyệt mọi ô (2 lần mỗi trục, mỗi input); nay lưu đệm theo (cuộn, khung,
  trang tính, thu phóng, số lần sửa).

Đo thời gian CPU trong tiến trình (test `wheel_step_timing`, bản release, `big10k.xlsx`, 40 nấc, kho ảnh rỗng):

| Bước mỗi nấc | Trước | Sau |
|---|---|---|
| cuộn (`scroll_rows`) | 0,99–1,23 ms | 1,45 ms (dao động do máy tải) |
| thanh cuộn (`update_bars`) | 1,49–2,10 ms | 0,00 ms |
| display list | 0,72 ms | 0,78 ms |
| `handle(Wheel)` cộng lại | 2,32–3,41 ms | 1,50 ms |

Trên màn hình (`thumb.ps1`, 30 nấc, 144 Hz; máy đang chạy build của 4 phiên khác nên số dao động ±5 ms giữa các lần):

| Cửa sổ | vòng tắt (4 lần, trung vị) | vòng bật (4 lần, trung vị) |
|---|---|---|
| 1500×1000 | 21,0 · 18,6 · 14,8 · 14,8 ms | 21,5 · 18,8 · 15,2 · 15,9 ms |
| 900×700 | 15,7 · 20,9 · 14,6 · 15,2 ms | 20,8 · 14,6 · 15,2 · 15,1 ms |

Riêng ảnh vòng không đổi độ trễ thấy được (trong sai số). Sau khi lưu đệm kích thước nội dung (bản cuối, 3 lần mỗi cỡ,
`r4-runs.txt`; đối chứng đo cùng lúc, cùng cách):

| Cửa sổ | Đối chứng trung vị / p95 | BlueSheet vòng 3 | BlueSheet vòng 4 | nội bộ: xử lý / lưới / phím→present |
|---|---|---|---|---|
| 1500×1000 | 7,5 / 14,3 ms | 19,9 / 26,9 | 14,6–14,9 / 20,8–23,0 | 1,8–2,0 / 2,4–2,7 / 5,2–5,8 ms |
| 900×700 | 7,8 / 14,3 ms | 14,6 / 15,3 | 14,4–14,6 / 15,0–15,7 | 1,5–1,7 / 1,3 / 3,7–3,9 ms |

Đọc số: 1500×1000 xuống từ ~3 khung còn 2 khung 144 Hz (p95 20,8–23 so với 26,9); 900×700 giữ 2 khung với p95 sát
trung vị. Đối chứng vẫn nhanh hơn một khung: nó vẽ GDI và được ghép ở vblank kế tiếp; BlueSheet sau present cần thêm
một vblank của swap chain flip. Việc còn lại: present sớm hơn trong khung (waitable object + đo vblank), dựng display
list chỉ cho dải mới lộ ra (hiện dựng cả khung nhìn 0,8 ms rồi chỉ vẽ dải).

### Vòng 13 — Format Cells tự vẽ (`sheet_form.rs`, `sheet_fcells.rs`)

Bộ điều khiển hộp thoại riêng của BlueSheet (không đổi `dialog.rs` dùng chung): list box, drop-down, ô số có mũi tên,
ô đánh dấu, ô màu, nút, vùng do chủ vẽ, thẻ trang. Đo trên đối chứng (`xfc-0.png`):

| Hạng mục | Đối chứng | BlueSheet |
|---|---|---|
| Cửa sổ / thanh tiêu đề | 692×636, 38 px #F3F3F3, mặt #F0F0F0 | như đối chứng |
| Trang thẻ | x 18–671, y 74–575, #F9F9F9 viền #E5E5E5 | như đối chứng |
| List box | viền #828790, hàng 17 px, chọn #0078D7 | như đối chứng |
| Nút OK / Cancel | 112×28 tại (434, 588) / (561, 588), #FDFDFD, viền #0078D4 / #D0D0D0 | như đối chứng |
| Chữ | Segoe UI, "Number" rộng 47 px, chữ hoa 10 px | 13,5 px: 47–48 px (trước sửa 15 px: 53 px) |

Các thẻ (`cmp-fc0..5.png`, đối chứng trái / BlueSheet phải): Number — danh sách 12 loại, Sample sống (định dạng giá
trị ô hiện hành bằng `bo_sheet::numfmt`), số chữ số thập phân, dấu phân cách, ký hiệu, kiểu số âm (danh sách mẫu
−1234,10), danh sách Type cho Date / Time / Fraction / Special / Custom (Custom có ô mã); Alignment — Horizontal /
Vertical, Indent, Justify distributed (mờ), Wrap / Shrink / Merge, Text direction, Orientation (cột chữ dọc, mặt
đồng hồ bấm đặt góc, ô độ); Font — ô + danh sách phông / kiểu / cỡ, Underline, Color, Normal font, Strikethrough /
Superscript / Subscript, Preview với đường nền hai bên; Border — 14 kiểu nét theo thứ tự đối chứng, Color, Presets
None / Outline / Inside, 8 nút cạnh có hình cạnh, hộp xem trước bấm gần cạnh nào thì bật cạnh đó; Fill — No Color,
bảng màu chủ đề 10×6 (sắc độ tính từ theme của sổ) + 10 màu chuẩn ở đúng vị trí, Pattern Color / Style, Sample;
Protection — Locked / Hidden + ghi chú. Engine: `StylePatch` thêm `fillPattern` / `patternColor` (§18.8.32) và
`vertAlign` (§18.4.14). Mở bằng menu chuột phải → Format cells… (chụp bằng nhấp gửi vào cửa sổ, `bcap.ps1`).

Còn khác: màu chữ / màu hoa văn là danh sách tên màu (đối chứng: bảng màu thả xuống); Fill Effects / More Colors mờ;
viền chéo vẽ trong hộp nhưng engine chưa có cạnh chéo; Text to Columns chưa có định dạng từng cột.

### Vòng 14 — Formulas và Data

- Formulas: Insert Function (tìm, nhóm, danh sách hàm, chữ ký đối số kiểu BlueOffice), nhóm Defined Names (Name
  Manager: danh sách, thêm / sửa / xoá qua `defineName` / `deleteName`; Create from Selection), nhóm Formula Auditing
  (mũi tên dò ô được tham chiếu / phụ thuộc từ `traceArrows`, mỗi lần bấm thêm một mức; khung quanh vùng; chấm ở
  nguồn, đầu mũi tên ở đích; Remove Arrows; Show Formulas; Error Checking chọn ô lỗi kế tiếp; Evaluate Formula liệt kê
  các bước của `evaluateFormula`).
- Data: Sort nhiều mức (Add / Delete / Copy Level, “My data has headers” đoán bằng `sortRegion`, cột theo tên tiêu
  đề, A→Z / Z→A), Data Validation 3 thẻ (Settings / Input Message / Error Alert, `setValidation`), Goal Seek (`goalSeek`,
  áp giá trị tìm được).

### Vòng 15 — Review, Page Layout, Help

- Thẻ chú thích (đo `xnote-all.png`): hộp 193×119 px, nền #FFFFE1, viền đen 1 px, lệch phải 15 px / lên 8 px so với góc
  trên-phải ô, chữ cách mép 6 px, đường nối từ góc ô; hiện khi rê chuột lên ô có chú thích; Show All Comments (Review)
  giữ mọi thẻ. Đối chứng (2016/2019) chỉ có chú thích kiểu note nên chú thích theo luồng của BlueSheet vẽ như note
  (tên tác giả đậm, các trả lời nối sau).
- Page Layout: Print Titles (hàng lặp ở đầu trang, `setPageSetup.titleRows`); Gridlines / Headings (View, Print) và
  Formula Bar là ô đánh dấu vẽ đè lên biểu tượng như đối chứng, không còn là nút bật/tắt tô nền.
- Thẻ Help: Help (giới thiệu, định dạng tương thích) và Keyboard Shortcuts.

## Còn lại (sau vòng 4)

- Bánh xe: vẫn ~2 khung 144 Hz so với ~1 khung của đối chứng (CPU 4–6 ms + một vblank của swap chain flip).
- Tab Draw (engine bảng tính chưa có nét mực); Page Layout: Themes, Background, Arrange.
- Insert: thư viện biểu đồ, PivotTable, Table Design, sparkline; Data: hộp lọc thả xuống có tìm kiếm và danh sách giá trị.
- Format Cells: bảng màu thả xuống cho màu chữ / hoa văn; Text to Columns định dạng từng cột; hộp tên hiện danh sách
  hàm khi đang sửa công thức.

## Vòng 5 — lọc tự động, Insert, màu Format Cells, thư viện kiểu ô

Nhánh `worktree-agent-aa38ae3d481a9744f` (từ `work/desktop-performance` 250ef53b). Ảnh so sánh (đối chứng trái/trên,
BlueSheet phải/dưới) trong thư mục scratch của phiên: `show/sheet-01-format-cells-number.png`,
`show/sheet-02-insert-tab.png`, `show/sheet-03-filter-dropdown.png` (chỉ BlueSheet), `show/sheet-04-cell-styles.png`,
`show/sheet-05-format-as-table.png`, `show/sheet-06-color-palette.png`.

Cách mở menu thả xuống của đối chứng không gửi input thật: nhấp gửi vào cửa sổ `NetUIHWND` của ribbon mở được thư viện
Cell Styles (cửa sổ `Net UI Tool Window` 991×639 px); ảnh ghép cửa sổ chính với các cửa sổ bật lên cùng tiến trình
(`xrib.ps1`, `xgrid.ps1`). Hộp lọc thả xuống của đối chứng **không** mở được bằng nhấp / Alt+↓ gửi vào cửa sổ lưới
(`EXCEL7` cần trạng thái chuột thật), nên hộp lọc dựng theo bố cục đã biết của đối chứng và chỉ có ảnh BlueSheet.

### Vòng 16 — sửa lỗi Format Cells, hộp lọc thả xuống

- Format Cells / Number: mô tả “General format cells have no specific number format.” bắt đầu bên phải danh sách
  Category (x 225, y 163 như đối chứng), không vẽ đè lên danh sách nữa; tiêu đề “Format Cells”; nút “?” cách mép phải
  96 px; danh sách Category luôn có thanh cuộn mờ (LBS_DISABLENOSCROLL như đối chứng).
- Lọc tự động (`sheet_filter.rs`): nhấp nút lọc ở ô tiêu đề mở hộp 300 px (ở 125 %) canh trái theo ô: Sort A→Z / Z→A
  (số: Smallest/Largest, ngày: Oldest/Newest), Sort by Color (mờ), Clear Filter From "…", Filter by Color (mờ),
  Text / Number / Date Filters ▸ (menu con: Equals, Does Not Equal, Begins / Ends With, Contains, Greater / Less Than,
  Between, Top 10, Above / Below Average, Today / This Week / … Year to Date, Custom Filter), ô Search có kính lúp,
  danh sách giá trị có (Select All) (ba trạng thái), cây năm / tháng cho cột ngày, (Blanks), OK / Cancel, chấm kéo
  cỡ. Custom AutoFilter (hai điều kiện, And / Or, ký tự đại diện) và Top 10 AutoFilter là form riêng. Engine:
  `filterValues` / `setFilter`; thanh trạng thái “x of y records found” như đối chứng, xoá khi nhấp lại vào lưới.

### Vòng 17 — tab Insert theo đối chứng

Nhóm như đối chứng ở 1920 px: Tables (PivotTable, Recommended PivotTables, Table), Illustrations (Pictures, Shapes),
Charts (Recommended Charts, khối 3 × 3 nút loại biểu đồ có menu: cột/thanh, phân cấp, thác nước/phễu/chứng khoán/bề
mặt, đường/vùng, thống kê, radar, tròn/vành khuyên, phân tán/bong bóng, 3-D; Maps, PivotChart), Sparklines (Line,
Column, Win/Loss), Filters (Slicer), Links (Link), Text, Symbols (Symbol). Biểu tượng mới là hình vẽ riêng
(`Icons::add_solid`).

- Create Table (vùng + “My table has headers”; không có tiêu đề thì chèn hàng Column1…), Create PivotTable (vùng,
  trang mới / có sẵn, chọn trường Rows và Sum of — chưa có ngăn PivotTable Fields), PivotChart (bảng tổng hợp đề xuất
  + biểu đồ cột), Create Sparklines (Data Range / Location Range → `sparklineInsert`), Slicer (cột bảng của ô hiện
  hành; ngoài bảng thì thanh trạng thái nhắc), Insert Hyperlink (ghi công thức `HYPERLINK`), Header/Footer (trái /
  giữa / phải → `setPageSetup.headerFooter`), Symbol (lưới 16 × 8, chọn trang ký tự, mã U+).
- Ảnh và hình: engine thêm lệnh `insertDrawing` (hình dạng preset hoặc ảnh PNG/JPEG/GIF/BMP base64, chữ trong hình) —
  một `xdr:grpSp` trong `SheetShapeGroup`, bộ ghi xlsx đặt vào phần drawing kèm quan hệ ảnh; có test lưu / mở lại.
  Pictures mở hộp chọn tệp của Windows (`Request::PickPicture`, `BLUEOFFICE_PICTURE` để chạy kịch bản).
- Chưa làm: Icons, 3D Models, SmartArt, Screenshot, Timeline (cần bảng tổng hợp có trường ngày), Equation (chưa có
  phương trình trong bảng tính), Text Box.

### Vòng 18 — viền chéo, bảng màu, hộp tên khi gõ công thức

- Viền chéo: `BorderPreset` thêm `diagonalDown` / `diagonalUp` / `noDiagonal` (§18.8.4); Format Cells / Border đọc và
  ghi hai nút chéo; test lưu / mở lại xlsx.
- Màu chữ, màu viền, màu hoa văn của Format Cells là nút màu mở bảng như đối chứng: Automatic, Theme Colors 10 × 6
  (sắc độ tính từ theme), Standard Colors, More Colors… (form Colors: Red / Green / Blue, Hex, ô mới / hiện tại; form
  Format Cells chờ bên dưới rồi nhận màu).
- Hộp tên khi đang gõ công thức hiện tên hàm đang gõ (mặc định SUM) và mũi tên liệt kê SUM, AVERAGE, IF, HYPERLINK,
  COUNT, MAX, SIN, SUMIF, PMT, STDEV, More Functions… như đối chứng; chọn hàm chèn `TÊN(` tại con trỏ.

### Vòng 19 — thư viện Cell Styles và Format as Table

Đo trên đối chứng (`xcs-1.png`): hộp 991 px, sáu ô mỗi hàng, ô 153 × 36 px cách 8 px, tiêu đề nhóm 30 px nền xám.
Engine thêm các kiểu dựng sẵn còn thiếu (Calculation, Check Cell, Explanatory Text, Input, Linked Cell, Output,
Warning Text, Heading 3–4, 20 % / 40 % / 60 % – Accent1–6, Accent1–6; builtinId theo §18.8.7) với màu đo được
(vd 20 % – Accent1 #D9E1F2, Accent1 #4472C4, Accent6 #70AD47). Thư viện vẽ mỗi kiểu bằng phông, màu, nền, viền dưới
của nó; Format as Table vẽ bảng nhỏ (hàng tiêu đề, dải màu) cho 21 kiểu Light / Medium / Dark.

### Bánh xe — đo lại sau khi gộp thay đổi page.rs của BlueReader

`thumb.ps1`, `big10k.xlsx`, 30 nấc cách 120 ms, 144 Hz:

| Cửa sổ | Đối chứng trung vị / p95 | BlueSheet vòng 4 | BlueSheet vòng 5 |
|---|---|---|---|
| 1500×1000 | 7,5 / 13,8 ms | 14,6–14,9 / 20,8–23,0 | 15,3 / 16,6 |
| 900×700 | 7,6 / 13,5 ms | 14,4–14,6 / 15,0–15,7 | 14,6 / 15,7 (sync 0: 14,7 / 15,9) |

Nội bộ (900×700): xử lý 1,2–1,4 ms, lưới 0,9–1,3 ms, vẽ giao diện 0,2–0,4 ms, gửi 0,4 ms — từ input tới khi
`Present` trả về ~3 ms; với `BLUEOFFICE_NATIVE_GPU=1` GPU xong sau 1,2–2,0 ms (tổng 4,8–6,6 ms). Khung vẫn hiện sau
hai vblank: nấc được gửi ngay sau một vblank (cách đo), đối chứng vẽ GDI xong trong ~1 ms và kịp lần ghép của DWM ở
vblank kế tiếp; BlueSheet xong sau ~5 ms thì trễ lần ghép đó. Sync interval 0 không đổi gì. Muốn còn một khung cần
cả CPU + GPU dưới ~1–2 ms (dựng display list chỉ cho dải mới lộ ra, bỏ bước `refresh` toàn khung) hoặc đường
DirectComposition — chưa làm trong vòng này; p95 ở 1500×1000 đã sát trung vị (16,6 so với 20,8–23 ms).

## Còn lại (sau vòng 5)

- Bánh xe vẫn ~2 khung 144 Hz (đối chứng ~1 khung): xem phân tích trên.
- Page Layout: Themes / Colors / Fonts (engine có `applyThemeFile` nhưng chưa có bộ theme dựng sẵn), Background,
  Arrange; tab Draw (engine bảng tính chưa có nét mực).
- Conditional Formatting: menu con theo đối chứng và Manage Rules; Fill Effects (gradient); Text to Columns định dạng
  từng cột; ngăn PivotTable Fields; chọn / di chuyển hình và ảnh trên lưới; tab Chart Design / Format, Table Design.
- Kiểu số (Comma, Currency, Percent) trong thư viện Cell Styles; ô thư viện chưa vẽ khung bốn cạnh (Calculation,
  Output, Check Cell) và chữ đậm tiêu đề nhóm; Format as Table mới có 21 kiểu (đối chứng 60: Light 21, Medium 28,
  Dark 11) và ô nhỏ hơn (61 × 46 so với 77 × 56 px).

## Vòng 6 — thư viện kiểu ô, Insert, Data / Formulas / View, biểu đồ, phím tắt

Nhánh `worktree-agent-a42d4a0783243dd39` (từ `work/desktop-performance` 296728d7). Ảnh so sánh (đối chứng trên,
BlueSheet dưới) trong `show/sheet-07…` của thư mục scratch phiên.

### Vòng 20 — thư viện Cell Styles đo lại

Đo trên `xcs-1.png` (đối chứng, 125 %):

| Hạng mục | Đối chứng | BlueSheet vòng 5 | Vòng 6 |
|---|---|---|---|
| Ô kiểu | 153 × 34 px, cách 161 × 42 px, cách mép 5 px | 153 × 36, cách 8 px | như đối chứng |
| Dải tiêu đề nhóm | 34 px, #EBEBEB, chữ đậm xám cách mép 12 px | 30 px #F0F0F0 | như đối chứng |
| Viền hộp | #949494 | #C6C6C6 | #949494 |
| Calculation / Input / Output / Note / Check Cell | khung 4 cạnh (Check Cell: kép #3F3F3F) | chỉ gạch dưới | khung 4 cạnh, kép = 2 nét 1 px cách 2 px |
| Heading 1 / 2 / 3 | nét dưới 3 px #4472C4 / 3 px #A2B8E1 / 2 px #8EA9DB | 2 px cùng màu | như đối chứng (engine: thick / thick / medium) |
| Good / Bad / Neutral | #C6EFCE / #FFC7CE / #FFEB9C | màu riêng | như đối chứng (chữ 006100 / 9C0006 / 9C5700) |
| Title | Calibri Light 18 pt thường, #44546A | đậm | phông tiêu đề của theme (scheme major) |
| Chữ trong ô | phông của kiểu, cao 'l' 16 px (≈ 1,2 × cỡ trên lưới) | phông giao diện | phông của kiểu × 1,2; tên dài kết thúc "…" |
| Nhóm Number Format, New Cell Style…, Merge Styles… | có | thiếu | có (lệnh engine `newCellStyle`, `mergeStyles`) |

Kiểu số (Comma 43, Comma [0] 41, Currency 44, Currency [0] 42, Percent 9) chỉ đổi định dạng số, giữ phần còn lại của ô
như đối chứng. New Cell Style… mở hộp Style đo trên đối chứng (404 × 459 px; OK / Cancel ở (146, 411) / (274, 411)),
mô tả "Style includes" lấy từ ô hiện hành; nhóm Custom hiện các kiểu riêng của sổ. Merge Styles… chọn một sổ
`.xlsx` (đối chứng liệt kê các sổ đang mở; BlueSheet mỗi sổ một tiến trình).

### Vòng 21 — Insert: Illustrations, Filters, Symbols

- Illustrations như đối chứng: Pictures lớn, hai cột nút nhỏ Shapes / Icons / 3D Models và SmartArt / Screenshot.
  Shapes và Icons mở thư viện hình (bộ chọn của BlueSlide, thư viện biểu tượng tự vẽ `packages/ui/src/iconLibrary`);
  biểu tượng chèn dưới dạng PNG vẽ từ SVG (engine `insertDrawing` nhận SVG). SmartArt: menu 7 họ bố cục
  (`insertSmartArt`). 3D Models, Screenshot mờ (chưa có).
- Filters: Slicer, Timeline (bảng tổng hợp có trường ngày; không có thì thanh trạng thái nhắc như đối chứng).
- Symbols: Equation ▾ (Area of Circle, Binomial Theorem, Pythagorean Theorem, Quadratic Formula, Insert New Equation —
  hộp chữ dạng tuyến tính vì bảng tính của engine chưa có vùng toán) và Symbol.

### Vòng 22 — Formulas, Data, View theo đối chứng ở 1500 px

Đối chứng chụp bằng nhấp gửi vào thẻ ribbon (`xtabs.ps1`, `xt6-1..3c.png`).

- Formulas: Insert Function, AutoSum, rồi ba cột nút nhỏ (Recently Used / Financial / Logical, Text / Date & Time /
  Lookup & Reference, Math & Trig / More Functions); Defined Names gộp một menu; Formula Auditing + Watch Window (mờ);
  Calculation Options + hai nút biểu tượng.
- Data: Get & Transform Data (Get Data ▾, From Text/CSV qua `bo-query`: `querySource` → `queryInspect` → `querySave`
  lên trang mới, From Table/Range); Queries & Connections (Refresh All, danh sách truy vấn); Sort & Filter (Clear);
  Data Tools (Flash Fill, Consolidate, Remove Duplicates, Data Validation); Forecast (What-If ▾: Goal Seek, Data Table);
  Outline ▾ (Group, Ungroup, Subtotal, Show / Hide Detail, Auto Outline, Clear Outline).
- View: Workbook Views (Normal, Page Break Preview / Page Layout mờ, Custom Views: hiện / thêm / xoá qua
  `customViews`), Show (Ruler mờ), Zoom (hộp Zoom 200 / 100 / 75 / 50 / 25 %, vừa vùng chọn, tuỳ chỉnh), Window
  (Freeze Panes; New Window, Split, Arrange, Hide / Unhide, Switch Windows mờ), Macros (mờ).
- Nhãn riêng của giao diện native theo đối chứng (`sheetNative.*`: Text to Columns, Math & Trig, …) không đổi nhãn
  của trình sửa web.

### Vòng 23 — biểu đồ trên lưới

Nhấp chọn biểu đồ (khung xám #A6A6A6, 8 tay nắm tròn trắng viền #848484), kéo thân để dời, kéo tay nắm để đổi cỡ
(theo ô, `moveChart`; khung xanh xem trước khi kéo), Delete xoá, Esc bỏ chọn. Chèn biểu đồ thì biểu đồ mới được chọn và
thẻ ngữ cảnh Chart Design hiện như đối chứng. Chart Design: Add Chart Element ▾ (tiêu đề, tiêu đề trục, nhãn dữ liệu,
bảng dữ liệu, chú giải), Quick Layout ▾, Change Colors ▾, Chart Styles ▾ (`chartDesign`), Change Chart Type ▾
(`editChart`); Switch Row/Column, Select Data, Move Chart mờ. Format: Reset to Match Style, Shape Fill / Outline của
vùng biểu đồ, Delete Chart.

### Vòng 24 — phím tắt

So với hành vi của đối chứng: F4 khi sửa công thức đổi tham chiếu ở con trỏ $A$1 → A$1 → $A1 → A1 (cả vùng A1:B2);
Ctrl+D / Ctrl+R điền xuống / sang phải; Ctrl+1 mở Format Cells; Ctrl+; ngày hôm nay, Ctrl+Shift+; giờ (giờ địa
phương); Alt+= AutoSum; Alt+Enter xuống dòng trong ô (trước đây Windows gửi Alt+phím dạng phím hệ thống nên không tới
BlueSheet; nay chỉ BlueSheet bật `win32::enable_sheet_keys`, BlueDoc không đổi). Ctrl+mũi tên, Ctrl+Shift+mũi tên, F2,
Tab / Enter trong vùng chọn đã có từ vòng trước (test `keyboard_shortcuts_as_the_reference`).

### Vòng 25 — góp ý của chủ dự án về Chart Design / Data

- Chart Styles là thư viện ô xem trước nằm ngay trên ribbon (5 ô 130 px + nút More) như đối chứng; ô là hình vẽ riêng
  (cột, lưới, sắc độ, nền tối) và áp `chartDesign.style`.
- Khi chọn biểu đồ, vùng nguồn được đóng khung trên lưới như đối chứng (đo `xch2-1.png`): danh mục #8157B7, tên chuỗi
  #C0353E, giá trị #326AC7, nét 2 px, nền nhạt, tay nắm vuông ở góc; hộp tên hiện "Chart 1". Tay nắm biểu đồ 13 px
  viền 2 px #616161 (trước 10 px viền 1 px #848484).
- Thẻ ngữ cảnh vẽ như thẻ thường (gạch dưới xanh khi chọn; `Ribbon::with_plain_contextual`, BlueDoc không đổi).
- Data › Sort & Filter: A→Z / Z→A, Sort, Filter, rồi cột Clear / Reapply / Advanced như đối chứng (ribbon thêm
  `with_late_rows`: mục hàng đặt sau nút lớn cuối nhóm). Outline: một nút lớn, không chú thích nhóm.
- Cỡ chữ biểu đồ đo lại: tiêu đề "Sales" đối chứng 46 × 16 px, BlueSheet 40 × 14 px (nhỏ hơn chút, không lớn hơn);
  chưa đổi bộ vẽ biểu đồ của engine trong vòng này.

### Bánh xe — đo lại vòng 6

`thumb.ps1`, `big10k.xlsx`, 900 × 700, 30 nấc cách 120 ms, 144 Hz: đối chứng trung vị 7,4 / p95 13,4 ms; BlueSheet
14,5 / 16,1 ms (vòng 5: 14,6 / 15,7). Không đổi: vẫn hai vblank so với một của đối chứng. Phân tích ở vòng 5 vẫn đúng
(CPU + GPU ~5 ms sau input làm lỡ lần ghép DWM kế tiếp); muốn một khung cần dựng display list chỉ cho dải mới lộ ra và
đường DirectComposition — chưa làm vòng này.

## Còn lại (sau vòng 6)

- Bánh xe: một khung như đối chứng (xem trên).
- Insert ở 1500 px vẫn tràn (mũi tên cuộn ribbon): nhóm Tables / Sparklines rộng hơn đối chứng ~50 px; nhãn một từ
  có ▾ chưa xuống dòng riêng như đối chứng ("Illustrations ▾").
- Biểu đồ: Switch Row/Column, Select Data, Move Chart (engine chưa có lệnh); Format tab chưa có Shape Styles gallery,
  WordArt, Size; ô kiểu biểu đồ là hình vẽ chung chứ chưa là ảnh xem trước thật của từng kiểu (`chartGallery` trả SVG).
- Format as Table 60 kiểu (engine mới 21) và Table Design; Conditional Formatting menu con + Manage Rules; Page Layout
  Themes; Page Break Preview / Page Layout view; Split, New Window, Watch Window, Macros (mờ).
- Merge Styles chọn tệp thay vì danh sách sổ đang mở (mỗi sổ một tiến trình).
