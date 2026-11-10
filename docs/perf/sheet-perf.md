# BlueSheet — hiệu năng bảng lớn (SHEET-PERF-001)

Mục tiêu: lưới đủ 1 048 576 hàng × 16 384 cột (XFD1048576); pivot trên 1 triệu hàng < 3 s;
sắp xếp / lọc 1 triệu hàng < 2 s; mở được sổ 10 triệu ô. Số đo bản **release** trên máy dev
(Windows 11, 16 luồng logic, 32 GB RAM, MSVC). Máy dùng chung với các agent khác: tải CPU nền
dao động 0–100 %, nên mỗi số là lần chạy yên nhất; khi máy bận, thời gian có thể gấp ~2 lần.

## Cách đo

```bash
# lõi bo-sheet: danh sách 1 048 576 hàng × 6 cột (vùng, sản phẩm, năm, số lượng, đơn giá, mã duy nhất)
cargo bench -p bo-sheet --bench big_sheet
BO_BENCH_ONCE=1 cargo bench -p bo-sheet --bench big_sheet          # mỗi ca chạy 1 lần, in thời gian
BO_BENCH_ONCE=1 BO_BENCH_ONLY=pivot cargo bench -p bo-sheet --bench big_sheet

# qua SheetEngine (lệnh JSON, gồm ảnh chụp undo + đồng bộ bộ tính công thức)
cargo bench -p bo-engine --bench sheet_big                          # + mở .xlsx 10 485 760 ô (sinh trong bench)
BO_BENCH_ONCE=1 BO_BENCH_ONLY=engine cargo bench -p bo-engine --bench sheet_big

# kiểm thử giới hạn lưới + mở 10 triệu ô
cargo test -p bo-engine --test sheet_limits
cargo test --release -p bo-engine --test sheet_limits -- --ignored --nocapture
```

Không commit tệp lớn: sổ 10 triệu ô được sinh trong bench / test (`bo_xlsx::write_xlsx`, ~65 MB).

## Kết quả lõi (`bo-sheet`, `BO_BENCH_ONCE=1`)

| Ca (1 048 575 bản ghi) | Trước | Sau (máy yên) | Sau (tải nền ~50 %) | Mục tiêu |
|---|---|---|---|---|
| Sắp xếp 1 khoá số | 13,3 s | 1,05 s | 1,46 s | < 2 s |
| Sắp xếp 2 khoá (văn bản tiếng Việt + số giảm dần) | 57,1 s | 1,26 s | 1,44 s | < 2 s |
| AutoFilter: danh sách giá trị + điều kiện số | 6,5 s | 0,67 s | 0,82 s | < 2 s |
| Tạo pivot (2 trường hàng, 1 trường cột, Sum) | 21,9 s | 2,43 s | 2,47 s | < 3 s |
| Làm mới pivot | 24,7 s | 2,74 s | 2,34 s | < 3 s |
| Nạp lại bộ tính (`LiveCalc::rebuild`, 6,3 triệu ô) | — | — | 2,41 s | (tham khảo) |

`size_of::<Cell>()` = 152 byte.

## Kết quả qua `SheetEngine` (danh sách 1 048 576 × 4)

| Ca | Trước | Sau | Ghi chú |
|---|---|---|---|
| Mở `.xlsx` 10 485 760 ô (7 cột số, 3 cột chuỗi dùng chung, 65 MB) | — | 13,0–14,0 s | mở được (`ten_million_cell_workbook_opens` release: 13,0 s) |
| Lệnh `sort` 2 khoá | (lõi 57 s + …) | ~4,0 s | ảnh chụp undo 0,6 s + sắp xếp 1,2 s + nạp lại bộ tính 2,3 s |
| `toggleFilter` trên 1 ô của danh sách 1 triệu hàng | treo (vùng hiện hành O(n²)) | < 1 s | `current_region` sửa |
| Lệnh `setFilter` | treo (O(hàng ẩn × ô)) | ~1,7 s* | không nạp lại bộ tính (`Recalc::HiddenRows`) |
| Lệnh `pivotCreate` | — | ~6–14 s* | ảnh chụp undo + nạp lại bộ tính chiếm phần lớn |

\* đo khi máy bận (tải nền ~50–100 %); xem bảng criterion bên dưới cho số ổn định khi có.

## SHEET-PERF-002: undo nhẹ + cập nhật bộ tính theo phần thay đổi

`BO_BENCH_ONCE=1 BO_BENCH_ONLY=engine cargo bench -p bo-engine --bench sheet_big`, mỗi ca 3 lần,
lấy trung vị; bản "trước" (main) và "sau" chạy xen kẽ trên cùng máy (dùng chung, có tải nền).

| Ca (qua `SheetEngine::exec_json`) | Trước (3 lần → trung vị) | Sau (3 lần → trung vị) | Mục tiêu |
|---|---|---|---|
| `sort` 2 khoá, 1 048 575 bản ghi × 4 cột | 2,89 · 3,66 · 3,18 → **3,18 s** | 1,63 · 1,84 · 2,52 → **1,84 s** | < 2 s |
| `setFilter` (danh sách giá trị) | 1,54 · 1,83 · 1,85 → **1,83 s** | 0,86 · 1,19 · 1,01 → **1,01 s** | < 2 s |
| `pivotCreate` (1 trường hàng, 1 cột, Sum) | 4,03 · 4,88 · 4,90 → **4,88 s** | 1,97 · 2,83 · 2,38 → **2,38 s** | < 3 s |

Phân rã lệnh `sort` sau khi sửa (một lần chạy): ảnh chụp "vỏ" sổ 0,06 ms; sắp xếp lõi + lưu công thức
~1,0 s; đồng bộ bộ tính ~0,5–0,6 s (trước: ảnh chụp cả sổ ~0,6 s + `LiveCalc::rebuild` ~2,3 s).
Pivot: tạo báo cáo lõi ~2,2 s + đồng bộ vùng đích ~0,07 s. Lọc: ẩn hàng lõi ~0,8 s + `SUBTOTAL` ~0,1 s.

Thay đổi:

- **Bước undo nhẹ** (`bo-engine/src/sheet/light_undo.rs`, `UndoEntry::Light`, `SheetEngine::edit_light`):
  thay vì `wb.clone()` (mọi ô), bước giữ sổ **không có ô và bộ đệm pivot** ("vỏ": style, thiết lập
  trang tính, hàng ẩn, AutoFilter/`sortState`, định nghĩa pivot, biểu đồ…) và:
  - **sort**: hoán vị hàng (`RowPerm`: hàng `start + n` nhận hàng `start + perm[n]`) + văn bản gốc
    của các công thức bị di chuyển (dịch tham chiếu khi đổi hàng không luôn đảo được: tham chiếu
    ra ngoài lưới thành `#REF!`). Undo = hoán vị ngược không dịch + khôi phục văn bản công thức;
    redo = áp lại hoán vị có dịch (tất định, trùng kết quả sort).
  - **pivotCreate / pivotRefresh**: nội dung cũ của **hình chữ nhật đích đã đổi** (lưu góc phần tư từ
    ô trái-trên của báo cáo trước khi sửa, sau khi sửa chỉ giữ hình bao các ô đổi) + bộ đệm pivot:
    thêm (`Truncate`/`Append`) hoặc bị làm mới thay (`Swap`, bộ đệm cũ được **chuyển** sang bước
    undo qua `bo_sheet::pivot::refresh_pivot_keeping`, không clone).
  - **toggleFilter / setFilter / showAllData**: chỉ "vỏ" (hàng ẩn), đồng bộ `SUBTOTAL` bằng
    `LiveCalc::sync_hidden_rows`.
  Áp bước = đổi chỗ trạng thái hiện tại với trạng thái lưu, trả về bước ngược (undo/redo chính xác).
  Sổ có trang tính được bảo vệ, sắp xếp trái → phải: giữ đường cũ (ảnh chụp cả sổ, kiểm ô khoá).
- **Bộ tính sau sort** (`LiveCalc::permute_rows` → `bo_formula::Workbook::permute_rows`): di chuyển
  hằng trong engine theo hoán vị (khối không có công thức: giá trị đổi chỗ theo chu trình ngay trong
  các mục của bảng băm, một lượt duyệt), đặt lại công thức bị di chuyển từ mô hình, đánh dấu ô di
  chuyển là đã sửa → chỉ công thức đọc chúng được tính lại. Nạp lại toàn bộ khi: chế độ tính tay,
  "precision as displayed", công thức chia sẻ / mảng trong hàng di chuyển, vùng tràn (spill) chạm khối.
- **Bộ tính sau pivot**: `LiveCalc::update` trên các ô của vùng đổi; thêm trang tính mới (pivot không
  có `dest`) vẫn nạp lại toàn bộ.

Kiểm thử đúng: `sheet::light_undo::tests` (sổ ngẫu nhiên có công thức tương đối / tuyệt đối / tới hàng
trên / ngoài vùng / trang khác / `SUBTOTAL`: sort nhiều lần, lọc, pivot tạo / làm mới / tạo trên trang
mới — sau lệnh trùng nạp lại bộ tính đầy đủ; undo → mô hình **giống hệt** trước; redo → giống hệt sau),
`live::tests::permuted_rows_recalculate_like_a_full_reload` (chỉ tính lại công thức liên quan),
`bo-formula/tests/proptest_permute.rs` (hoán vị có / không có công thức trong khối = sổ mới với đầu
vào đã hoán vị). Cộng tác: lệnh không đổi đường đồng bộ (`collab_local` so mô hình), test collab xanh.

## Điểm nóng đã sửa

- **Sắp xếp** (`bo_sheet::sort::sort_lines`): mỗi mức khoá quy về một **thứ hạng u32 cho mỗi dòng**
  (theo cột): giá trị phân biệt chuẩn bị một lần (khoá đối chiếu NFD chỉ tính một lần cho mỗi chuỗi
  phân biệt), sắp hoán vị bằng `sort_by_key` ổn định trên số nguyên (2 khoá gói vào `u64`). Trước đây
  mỗi phép so sánh gọi `text_key` (NFD + cấp phát) và tra BTreeMap. Hàng được **hoán vị tại chỗ**
  (`permute_rows_in_place`): lấp chỗ trống bằng ô đánh dấu, đổi chỗ ô theo chu trình hoán vị, không
  dựng lại cây; danh sách quá thưa dùng đường cũ (cắt các hàng ra một lần rồi dựng lại cây hàng loạt).
  `text_key` có đường tắt ASCII. Thứ tự số / ngày ISO cùng hạng giờ là thứ tự toàn phần (số trước).
- **AutoFilter** (`hidden_rows_at`): định dạng số phân tích một lần cho mỗi style (trước: mỗi ô),
  chuỗi hiển thị chỉ tính khi điều kiện cần, danh sách giá trị hạ chữ thường một lần (HashSet),
  không clone giá trị ô.
- **Pivot**: đọc nguồn và định dạng nguồn bằng một lần duyệt dải (không tra cây từng ô); mục phân
  biệt của trường (`cache_fields`) khử trùng chuỗi mượn rồi sắp khoá có tiền tố 8 byte, các trường
  chạy song song bằng `std::thread::scope` trên native (WASM đơn luồng); chỉ số mục chỉ dựng cho
  trường trục / bộ lọc; khoá mục văn bản nhớ theo chuỗi (không tạo khoá chữ thường mỗi bản ghi);
  tổng hợp trước theo lá (khoá mượn, không cấp phát mỗi bản ghi) rồi gộp lên tổng phụ / tổng chung;
  làm mới không clone trường cũ, bản ghi cũ giải phóng ở luồng nền. Giới hạn nguồn nâng 4 M → 16 M ô.
- **Engine**: `current_region` (Ctrl+A / sort / filter / pivot từ 1 ô) trước O(hàng²) và bị chặn
  100 000 vòng → nay mỗi bước cột duyệt dải một lần, không giới hạn kích thước danh sách.
  Nạp bộ tính: hàng ẩn nạp hàng loạt (`set_row_hidden` đánh dấu mọi công thức *mỗi hàng*).
  Lọc / bật lọc / hiện tất cả chỉ đồng bộ tập hàng ẩn (`LiveCalc::sync_hidden_rows`,
  `bo_formula::Workbook::set_hidden_rows`). Mở tệp không còn chụp cả sổ cho bước chuyển mã
  TCVN3/VNI khi không có phông cũ (`bo_encoding::workbook_has_legacy_fonts`). Bộ tính cũ giải phóng ở
  luồng nền khi nạp lại.

Song song: không thêm `rayon`; chỉ `std::thread` có `cfg(not(target_arch = "wasm32"))`, WASM giữ
đơn luồng (`pnpm build:wasm` xanh).

## Kiểm thử đúng

- `sort::tests::prepared_sort_matches_naive_stable_sort`: 3000 dòng giá trị hỗn hợp (số, văn bản
  có dấu / hoa thường, bool, lỗi, rich text, ngày ISO, ô trống), 5 bộ khoá (giảm dần, phân biệt hoa
  thường, danh sách tuỳ chỉnh) — kết quả trùng sắp xếp ổn định từng ô theo `compare_values`.
- `sort::sparse_tests::sparse_and_dense_rows_sort_alike`: hàng đầy, có lỗ (hoán vị tại chỗ) và rất
  thưa (dựng lại cây); ô ngoài vùng giữ nguyên.
- `sort::tests::rows_move_inside_the_range_only`: vùng kết thúc ở hàng 1 048 576, công thức đi theo hàng.
- `filter::tests::hidden_rows_match_naive_evaluation`: 4000 hàng, định dạng số, danh sách giá trị,
  điều kiện số / ký tự đại diện, AND/OR — trùng đánh giá từng ô bằng `format_cell`.
- `pivot::tests::report_cells_equal_naive_aggregates` (Sum/Count/Min/Max mọi ô, tổng hàng, cột,
  chung) và `nested_subtotals_equal_naive_sums_and_refresh`.
- `bo-engine/tests/sheet_limits.rs`: XFD1048576 trong tham chiếu (`=XFD1048576*2`, `SUM(XFD1:XFD1048576)`,
  `COUNT(XFD:XFD)`, `XFE1` không hợp lệ), Go To, vùng hiện hành ở góc, sắp xếp ở mép, vẽ khi cuộn quá
  cuối lưới, chèn / xoá hàng-cột ở mép; `ten_million_cell_workbook_opens` (`#[ignore]`, chạy release).

## Còn lại

- ~~Ảnh chụp undo cả sổ + nạp lại bộ tính sau sort / pivot~~: xong ở SHEET-PERF-002 (mục trên).
  Vẫn chụp cả sổ: sắp xếp trái → phải, sổ có trang tính bảo vệ, pivot update / group / move / đổi
  nguồn / làm mới tất cả, Advanced Filter, Subtotal. Pivot tạo trên **trang mới** vẫn nạp lại bộ tính
  (engine chưa thêm trang tăng dần). Undo của sort vẫn đồng bộ bộ tính bằng hoán vị ngược (~0,5 s).
- Mở 10 triệu ô ~14 s, bộ nhớ ~4 GB (mỗi `Cell` 152 byte trong BTreeMap + bản sao trong bộ tính).
  Hướng tiếp: lưu ô theo cột / chuỗi dùng chung dạng chỉ số, `Box` công thức trong `Cell`.
