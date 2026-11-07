# Phase 3 — BlueSheet: đọc/ghi .xlsx và lõi bảng tính

Trạng thái: **I/O + lõi sheet + giao diện & chỉnh sửa BlueSheet xong, chờ duyệt** · Prompt gốc: [`BlueOffice-PROMPT.md`](../../BlueOffice-PROMPT.md)
§6 Phase 3 ("Đọc/ghi .xlsx", "Định dạng số", "Giao diện & chỉnh sửa"). Engine công thức (`bo-formula`)
làm song song ở nhánh khác — ở đây **không** tính công thức.

## 1. Phạm vi đã cài

| Crate | Nội dung |
|---|---|
| `bo_model::sheet` | Mô hình workbook (ECMA-376 Part 1, §18): ô thưa (`BTreeMap<(row,col), Cell>`), styles, merges, cột/hàng, panes, defined names, print area/titles, page setup, hyperlink, comment, data validation, conditional formatting, table, **AutoFilter**, **Chart** (chỉ đọc), phần opaque. |
| `bo-xlsx` | `read_xlsx` / `write_xlsx` lossless; CSV (RFC 4180, BOM, dialect vi-VN `;`). |
| `bo-sheet` | Định dạng số, màu, hình học lưới, viewport + freeze, render ra display list, chèn/xoá hàng cột, sort, AutoFilter, AutoFill, ranh giới trait cho `bo-formula`. |

### .xlsx
- workbook, worksheets, sharedStrings (rich text, `_xHHHH_`), styles (numFmts, fonts, fills pattern/gradient,
  borders, cellStyleXfs, cellXfs, cellStyles, dxfs), merge cells, độ rộng cột / chiều cao hàng, ẩn hàng/cột,
  outline, freeze/split panes, defined names (workbook & sheet scope), print area / print titles, page setup
  (printOptions, pageMargins, pageSetup, headerFooter, breaks), hyperlink (ngoài qua relationship, trong qua
  `location`), comments, data validation, conditional formatting (cellIs, expression, colorScale, dataBar,
  iconSet, top10, …), tables (ListObject), AutoFilter của sheet (`filters`, `customFilters`, `top10`; loại khác giữ
  nguyên XML).
- **Chart**: drawing (§20.5) → `c:chartSpace` (§21.2) được tóm tắt vào `Worksheet::charts` (loại, hướng cột,
  grouping, tiêu đề, legend, series với tham chiếu + giá trị cache, anchor ô). Part drawing/chart vẫn **opaque**,
  ghi lại byte-for-byte; sửa `Chart` chưa được lưu (cần writer DrawingML ở bước sau).
- Lossless: part không hiểu giữ byte-for-byte; phần tử con không hiểu (`extLst`, `x14:*`, `mc:AlternateContent`,
  `sheetProtection`, `drawing`, `controls`, …) giữ nguyên XML và ghi lại đúng vị trí schema; relationship opaque giữ id.
- An toàn: giới hạn giải nén / tỉ lệ nén (zip bomb) và độ sâu XML của `bo-opc`; mini-DOM của chart giới hạn số node;
  không `unwrap` trên dữ liệu file.

### Định dạng số (`bo_sheet::numfmt`)
4 section, điều kiện `[>100]`, màu `[Red]` / `[ColorN]`, ngày giờ (`[h]`, `ss.00`, AM/PM, `bbbb`, hệ 1900 có
29/02/1900 và hệ 1904), %, phân số, khoa học, `@`, literal / `_x` / `*x`, tag locale `[$-42A]`, tiền tệ `[$₫-42A]`,
`[$-F800]`/`[$-F400]`, built-in id 0–49, dấu phân cách vi-VN (`.` nghìn, `,` thập phân). Test dạng bảng trong
`crates/bo-sheet/src/numfmt/tests.rs`.

### Corpus
`tests/corpus/sheet/`: 42 file tự sinh (14 tính năng × 3), README ghi nguồn gốc. Test
`cargo test -p bo-xlsx --test corpus corpus_roundtrip -- --nocapture` báo % lossless theo tính năng
(hiện **42/42 = 100 %**, ngưỡng 95 %). "Lossless" = model đọc lại giống hệt, part opaque giống từng byte, không
mất part nào (trừ `calcChain` cố ý bỏ), và mọi "fact" (đường dẫn phần tử + thuộc tính + giá trị, text; id
relationship so theo đích) của part sinh lại vẫn còn.

## 2. API chính

```rust
// bo-xlsx
read_xlsx(&[u8]) -> Result<Workbook>;            read_xlsx_with_limits(&[u8], &Limits)
write_xlsx(&Workbook) -> Result<Vec<u8>>;        read_csv / write_csv(…, &CsvOptions)

// bo-sheet
numfmt::{NumberFormat::parse_with(code, &Locale), format_value, format_number, format_code,
         builtin_format, Locale::{VI_VN, EN_US}, FormatOptions, Formatted}
color::resolve_color(&SheetColor, Option<&ThemeColors>) -> Option<Rgba>
geometry::{SheetGeometry::new(ws, styles, &mut FontManager) | with_metrics(ws, SheetMetrics),
           col_x / row_y / col_width / row_height / cell_rect / range_rect / hit_test}
viewport::{Viewport, ViewLayout::for_sheet(geom, ws, &vp) -> panes (Corner/Top/Left/Main),
           ViewLayout::hit_test -> Hit::{Cell, ColumnHeader, RowHeader, Corner, None}}
render::{render_viewport(wb, sheet, geom, &vp, &RenderOptions, &mut FontManager) -> PageList,
         format_cell(wb, cell, &Locale) -> Formatted}
ops::{insert_rows, delete_rows, insert_cols, delete_cols}(wb, sheet, at, count, &dyn FormulaAdjuster)
sort::{sort_range(ws, range, &[SortKey], has_header), compare_values, text_key}
filter::{hidden_rows, apply_auto_filter, column_values, wildcard_match}
fill::{fill_series(&[CellValue], n, &FillOptions), autofill(wb, sheet, src, target, &Locale, &dyn FormulaAdjuster)}
calc::{FormulaAdjuster { adjust, translate }, FormulaEngine { recalculate }, NoFormulas}
```

`PageList` (JSON qua `to_json`) là định dạng display list sẵn có của `bo-render` — canvas-host vẽ trực tiếp.

## 3. Ranh giới với `bo-formula`
Ô giữ `formula.text` (không có `=`) + giá trị cache. `bo-sheet` gọi `FormulaAdjuster::adjust` khi chèn/xoá
hàng cột (cell, defined names, print area/titles, DV, CF, cột tính toán của table) và `translate` khi AutoFill;
`FormulaEngine::recalculate` để tính lại. Mặc định `NoFormulas` giữ nguyên công thức và giá trị cache.

## 4. Giao diện & chỉnh sửa (Phase 3 "Giao diện & chỉnh sửa")

Mọi thao tác là command của `CommandRegistry` (`packages/sheet-editor/src/BlueSheetEditor.tsx`) và đi qua
cầu JSON `SheetEngine::exec_json` (`crates/bo-engine/src/sheet/cmd.rs`, ADR-0008) → WASM `WasmSheet::exec`,
Tauri `sheet_call("exec")`, TS `SheetHost.exec(SheetCommand)`. Mỗi lệnh thay đổi là một bước undo.

| Tính năng | Lõi (Rust) | UI |
|---|---|---|
| Định dạng ô | `bo_sheet::style`: `StylePatch` (mã số kể cả preset vi-VN `#,##0 [$₫-42A]`, `0%`, `dd/mm/yyyy`; font đậm/nghiêng/gạch/cỡ/màu/tên; nền; căn ngang/dọc, xuống dòng, thụt lề, xoay), viền (outline/all/inside/none/cạnh, kiểu nét), `cellXfs` khử trùng lặp; gallery kiểu ô (Normal/Good/Bad/Neutral/Title/Heading 1–2/Total/Note — `cellStyleXfs` + `cellStyles` với `builtinId` §18.8.7, màu riêng của BlueSheet); Format Painter; xoá định dạng | Ribbon Trang đầu; hộp thoại "Định dạng ô" (Radix Dialog, tab Số/Căn lề/Phông/Viền/Tô nền, Ctrl+1); bảng màu; menu kiểu ô |
| Hàng/cột | `bo_sheet::rowcol`: độ rộng (pt ↔ ký tự §18.3.1.13), chiều cao, ẩn/hiện, freeze (`pane` frozen), gộp/bỏ gộp (giữ ô trên-trái); AutoFit đo chữ bằng `render::measure_cell` | Kéo mép tiêu đề để đổi kích thước, nhấp đúp để AutoFit (hit test `ColumnEdge`/`RowEdge`), tab Xem: cố định hàng đầu / cột đầu / tại ô chọn |
| Sort & lọc | `sort_range_with` nhiều cấp (dịch công thức), `current_region` cho ô đơn; AutoFilter bật/tắt, giá trị cột, điều kiện (`customFilter`, có `*` cho chứa/bắt đầu/kết thúc), `apply_auto_filter` ẩn hàng | Hộp thoại sắp xếp nhiều cấp; nút lọc vẽ trên ô tiêu đề (`FilterButton` hit) mở menu: tìm giá trị, chọn/bỏ, điều kiện; điều hướng bàn phím bỏ qua hàng/cột ẩn |
| Điền | `fill::autofill` (chuỗi số/ngày/danh sách, dịch công thức qua `BoFormula`), Ctrl+D/Ctrl+R | Tay nắm ở góc vùng chọn, khung xem trước |
| Định dạng có điều kiện | `bo_sheet::cf`: cellIs, expression, containsText/…, top10, aboveAverage, duplicate/unique, colorScale, dataBar, iconSet (biểu tượng vector tự thiết kế — `icons.rs`), thứ tự `priority` + `stopIfTrue`; render vào display list | Hộp thoại "Thêm quy tắc"; xoá quy tắc trên vùng chọn |
| Xác thực dữ liệu | `bo_sheet::validation`: list (inline / vùng / tên), whole, decimal, date, time, textLength, custom; `errorStyle` stop chặn, warning/information báo | Mũi tên dropdown cạnh ô (Alt+↓), hộp thoại xác thực, thông báo từ chối |
| Biểu đồ | `bo_sheet::chart`: dữ liệu sống từ vùng tham chiếu (cache nếu không giải được), vẽ cột/thanh/đường/vùng/tròn/doughnut/scatter/combo tại anchor; `bo-xlsx::chart_write`: `c:chartSpace` + drawing (§20.5, §21.2) cho biểu đồ mới (thêm vào drawing có sẵn nếu có), sinh lại part khi sửa (`Chart::dirty`) | Chèn → Biểu đồ (theo loại), Sửa biểu đồ (tiêu đề, chú giải, tiêu đề trục, loại), chọn biểu đồ bằng chuột, nhấp đúp để sửa |
| Tìm/Thay, Đi tới, Dán đặc biệt | find/replace (phân biệt hoa thường, cả ô, trong công thức/giá trị, mọi sheet); `resolve` (A1, Sheet!A1:B2, tên); clipboard nội bộ bản ghi ô: all/values/formulas/formats, transpose, bỏ ô trống, cắt = di chuyển; tham chiếu tương đối dịch theo `BoFormula::translate` | Ctrl+F, Ctrl+H, Ctrl+G/F5, Ctrl+Alt+V; Ctrl+V dùng bản ghi nội bộ khi clipboard hệ thống là bản sao của chính BlueSheet, ngược lại dán TSV |
| Xuất PDF theo page setup | `sheet/print.rs`: khổ giấy, hướng, lề, scale / fitToWidth×Height, vùng in, hàng/cột tiêu đề lặp lại trên mọi trang, ngắt trang thủ công, căn giữa, in lưới; trang xếp "xuống rồi sang" | Tab Bố cục trang: hộp thoại thiết lập trang, đặt/xoá vùng in |
| Hiệu năng | `bo_sheet::live::LiveCalc`: một `bo_formula::Workbook` sống trong `SheetEngine`; sửa ô → chỉ tính lại phụ thuộc (`Workbook::last_recalculated`) | — |

Benchmark sửa 1 ô trong sheet 100 000 công thức (20 000 hàng × 5 công thức + SUM): **≈ 6 ms ở bản debug**
(`cargo test -p bo-engine live_recalc_touches_only_dependents -- --nocapture`, ngưỡng 100 ms, chỉ 6 lần
evaluate); bản release: `cargo bench -p bo-engine --bench sheet_edit`.

Corpus writer: `writer-format-*`, `writer-sheet-*`, `writer-charts-*` (sinh bằng `write_xlsx`,
`BO_WRITE_CORPUS=1`) — 51/51 lossless, OpenXmlValidator hợp lệ (`node scripts/fidelity-report.mjs`).
E2E: `tests/e2e/specs/sheet.spec.ts` (gõ số + SUM, đậm + tô nền, sort, lưu .xlsx, mở lại).

## 5. PivotTable (§18.10) và các tính năng bổ sung

**PivotTable** (`bo_model::sheet::{PivotCache, PivotTable}`, `bo-xlsx::pivot` / `pivot_write`, `bo_sheet::pivot`):
- Đọc: `workbook/pivotCaches` (§18.2.22) → `pivotCacheDefinition` (§18.10.1.67: nguồn `worksheetSource`, `cacheFields`
  + `sharedItems`, part records) và `pivotTableDefinition` của từng sheet (§18.10.1.73: `location`, `rowFields`,
  `colFields`, `pageFields` + mục chọn, `dataFields` + `subtotal`). Các part vẫn **opaque** → ghi lại byte-for-byte
  khi không sửa (lossless). Ô báo cáo là ô thường của sheet nên hiển thị nguyên trạng.
- Tạo/sửa/làm mới: cache dựng từ vùng nguồn (dòng tiêu đề = tên trường, item gom không phân biệt hoa thường, sắp
  số < chữ < bool < lỗi < trống); báo cáo dạng bảng (tabular, không subtotal), Hàng × Cột × Giá trị
  (Sum/Count/Average/Max/Min/Product/CountNums/StdDev(p)/Var(p)), bộ lọc báo cáo (một hoặc nhiều mục), tổng cộng
  hai chiều; định dạng số của cột nguồn cho ô giá trị. Ghi `pivotCacheDefinition` (`refreshOnLoad="1"`) +
  `pivotCacheRecords` (chỉ số `x`) + `pivotTableDefinition` (compact/outline tắt, `defaultSubtotal="0"`); pivot
  từ file khi sửa/làm mới được sinh lại part cùng tên. Làm mới tính lại mọi pivot dùng chung cache (khớp trường theo tên).
- Engine: `pivotSource`, `pivotCreate`, `pivots`, `pivotUpdate`, `pivotRefresh`; UI: Chèn → Bảng tổng hợp (hộp thoại
  vùng nguồn / trang tính mới hoặc ô đích), panel danh sách trường (tick trường, kéo thả giữa Bộ lọc/Cột/Hàng/Giá trị,
  chọn hàm tóm tắt, chọn mục lọc), lệnh làm mới.
- Corpus `writer-pivots-{1,2,3}.xlsx` (OpenXmlValidator hợp lệ, cả file pivot từ file sau khi làm mới); E2E
  `tests/e2e/specs/sheet-pivot.spec.ts` (tạo pivot, đổi hàm, lưu, mở lại, tính lại).

**Bổ sung khác**
- Biểu đồ: kéo để di chuyển, kéo góc để đổi cỡ (lệnh `moveChart`), xoá cả biểu đồ đọc từ file
  (`bo_xlsx::chart_write::{move_chart, remove_chart}` sửa anchor trong drawing opaque, bỏ relationship và part chart).
- Cắt–dán: công thức ở mọi sheet (và defined name) trỏ tới ô bị di chuyển được trỏ theo (`bo_formula::adjust_for_move`,
  `FormulaAdjuster::moved`); công thức của chính khối bị cắt giữ nguyên. Dán bản sao lặp lại khi vùng chọn là bội số.
- Sort trái → phải (`sortState/@columnSort`) và theo màu ô / màu chữ (`sortBy`): `bo_sheet::sort::sort_lines`.
- AutoFilter: `dateGroupItem`, `dynamicFilter` (ngày tương đối, M1–12, Q1–4, trên/dưới trung bình), `colorFilter`
  được mô hình hoá (lossless) và đánh giá; menu lọc có cây năm/tháng, danh sách lọc ngày, lọc theo màu.
- Render: chữ xoay `textRotation` 1–180 (transform) và 255 (xếp dọc); style mặc định của hàng/cột vẽ trên ô trống.

## 6. Còn lại
- PivotTable: chưa có subtotal, layout compact/outline, nhóm ngày/số, trường tính toán, "show values as", định dạng
  pivot; chưa xoá pivot; pivot từ file chỉ tính lại được khi nguồn là vùng/tên/bảng trong workbook; đổi tên sheet nguồn
  không cập nhật part cache opaque cho tới khi làm mới.
- Cắt–dán sang sheet khác chưa thêm tên sheet nguồn cho tham chiếu tương đối không qualified bên trong khối cắt;
  chart/CF/DV chưa cập nhật theo ô bị di chuyển.
- Biểu đồ `absoluteAnchor` chưa di chuyển được; `oneCellAnchor` chỉ di chuyển (không đổi cỡ).
- `dynamicFilter` có `valIso`/`maxValIso`, `iconFilter` vẫn giữ nguyên XML, chưa đánh giá.
- Comments part: chỉ giữ authors + commentList (threaded comments vẫn opaque).
- Màu series lấy theo bảng màu BlueSheet, chưa đọc `c:spPr` của file.