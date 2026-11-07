# Phase 3 — BlueSheet: đọc/ghi .xlsx và lõi bảng tính

Trạng thái: **phần I/O + lõi sheet xong, chờ duyệt** · Prompt gốc: [`BlueOffice-PROMPT.md`](../../BlueOffice-PROMPT.md)
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

## 4. Còn lại
- Render: conditional formatting (màu, data bar, icon), style mặc định của hàng/cột cho ô trống, viền nét đứt
  (đang vẽ liền), xoay chữ, vẽ chart; mô hình chart chỉ đọc → cần writer `c:chartSpace` để chỉnh sửa chart.
- Sort theo cột (trái → phải), sort theo màu; AutoFilter `dynamicFilter`/`colorFilter`/`dateGroupItem` mới giữ
  nguyên, chưa đánh giá.
- Tham chiếu công thức khi sort/chèn/xoá/autofill: chờ `bo-formula` cài `FormulaAdjuster`.
- Comments part: chỉ giữ authors + commentList (threaded comments vẫn opaque).
- Chưa có: WASM/engine binding cho BlueSheet, UI grid, OpenXmlValidator trong CI cho file xuất ra.
