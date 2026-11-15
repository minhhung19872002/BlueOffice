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
