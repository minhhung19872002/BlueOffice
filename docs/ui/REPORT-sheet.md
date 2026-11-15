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
