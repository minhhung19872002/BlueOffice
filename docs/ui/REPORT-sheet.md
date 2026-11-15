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