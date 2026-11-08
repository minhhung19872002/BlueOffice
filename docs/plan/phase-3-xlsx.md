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

**PivotTable** (`bo_model::sheet::{PivotCache, PivotTable}`, `bo-xlsx::pivot` / `pivot_write`,
`bo_sheet::pivot::{compute, group, calc, style}`, lệnh engine trong `bo-engine/src/sheet/pivot_cmd.rs`):
- Đọc: `workbook/pivotCaches` (§18.2.22) → `pivotCacheDefinition` (§18.10.1.67: nguồn `worksheetSource`, `cacheFields`
  + `sharedItems` (cả `d`), `fieldGroup` (§18.10.1.27) với `rangePr` (§18.10.1.77) / `discretePr` (§18.10.1.26) /
  `groupItems` (§18.10.1.37), trường tính toán `cacheField/@formula` + `databaseField="0"`, part records) và
  `pivotTableDefinition` của từng sheet (§18.10.1.73: `location`, `rowFields`/`colFields`, `rowItems`/`colItems`,
  `pageFields` + mục chọn, `pivotFields` (`defaultSubtotal`, `subtotalTop`, `insertBlankRow`, `sortType`,
  `autoSortScope`, mục ẩn `h`, `compact`/`outline`, `x14:pivotField/@fillDownLabels`), `dataFields` (`subtotal`,
  `showDataAs`, `baseField`, `baseItem`, `numFmtId`, `x14:dataField/@pivotShowAs`), `filters` (§18.10.1.35: lọc nhãn,
  giá trị, top-N qua `top10`/`customFilters`), `pivotTableStyleInfo`, `rowGrandTotals`/`colGrandTotals`). Part
  không sửa vẫn **opaque** → ghi lại byte-for-byte (lossless). Ô báo cáo là ô thường của sheet.
- Tính báo cáo (`bo_sheet::pivot::compute`): bản ghi qua bộ lọc báo cáo + mục ẩn → lọc nhãn/giá trị/top-N theo
  từng trường (trường ngoài trước, so trong cùng mục cha; tổng phụ/tổng cộng chỉ gồm mục còn hiện) → gộp mọi cặp
  tiền tố hàng × cột (mục, tổng phụ, tổng cộng) theo `ST_DataConsolidateFunction` → sắp xếp mục (thứ tự nguồn,
  tăng/giảm theo nhãn hoặc theo trường giá trị) → "hiển thị giá trị dạng": % tổng cộng / tổng cột / tổng hàng,
  % mục cơ sở, chênh lệch / % chênh lệch (mục cố định, trước, sau), lũy kế và % lũy kế, % tổng hàng/cột cha,
  % tổng cha (trường cơ sở), xếp hạng tăng/giảm, chỉ số → bố cục **thu gọn** (một cột nhãn thụt lề, "Nhãn
  hàng"/"Nhãn cột"), **phác thảo**, **dạng bảng**; tổng phụ bật/tắt, ở đầu/cuối nhóm (dạng bảng luôn ở cuối), lặp
  lại nhãn, dòng trống sau mục; tổng phụ cột; tổng cộng hàng/cột bật/tắt. `rowItems`/`colItems` sinh cùng ô.
- Nhóm (`group`): ngày theo năm / quý / tháng / ngày (mức nhỏ nhất nhóm trên chính trường, mức lớn hơn thành
  trường nhóm mới `base`/`par`, đặt trước trường trên trục), khoảng số (bắt đầu/kết thúc/bước, tự động theo dữ liệu
  khi bỏ trống), mục `<đầu`/`>cuối`/(trống); làm mới giữ nhóm. Nhóm thủ công (`discretePr`) chỉ đọc/ghi, chưa có UI.
- Trường tính toán (`calc`): công thức tên trường (trần hoặc `'có dấu cách'`) tính bằng `bo-formula` trên **tổng**
  của các trường trong từng ô báo cáo (kể cả tổng phụ/tổng cộng); từ chối tham chiếu ô/tên lạ.
- Định dạng: số của trường giá trị (`numFmtId`, mặc định theo cột nguồn; phần trăm `0.00%` cho phép tính tỉ lệ;
  không áp cho Count/xếp hạng/chỉ số), nhãn ngày theo định dạng cột nguồn; kiểu `PivotStyleLight|Medium|Dark1–28`
  (tên theo §18.8.40) vẽ bằng bảng màu riêng của BlueSheet: dòng tiêu đề, tổng phụ, tổng cộng (viền đôi),
  hàng/cột xen kẽ, tiêu đề hàng, cột tổng cộng.
- Thao tác: tạo, sửa vùng, làm mới (mọi pivot dùng chung cache; khớp trường theo tên, giữ nhóm/trường tính toán),
  **làm mới tất cả**, **đổi vùng nguồn**, **di chuyển** (cùng sheet hoặc sheet khác — quan hệ part theo sang),
  **xoá** (ô, part pivot đọc từ file, cache không còn dùng). Lệnh sửa lỗi giữa chừng khôi phục workbook.
- Ghi: `pivotCacheDefinition` (`refreshOnLoad="1"`, ngày dưới dạng `d`, `fieldGroup`) + `pivotCacheRecords` (chỉ
  trường gốc) + `pivotTableDefinition` đầy đủ thiết lập ở trên; pivot từ file khi sửa/làm mới được sinh lại part
  cùng tên.
- Engine: `pivotSource`, `pivotCreate`, `pivots` (trường + mục theo định dạng, nhóm, công thức, thiết lập),
  `pivotUpdate` (+ `layout`, `rowGrandTotals`, `colGrandTotals`, `fieldOptions`, `fieldFilters`, `style`, giá trị
  có `showAs`/`baseField`/`baseItem`/`numFmt`), `pivotGroup`, `pivotCalcField`, `pivotMove`, `pivotChangeSource`,
  `pivotDelete`, `pivotRefresh`, `pivotRefreshAll`; `presets.pivotStyles`.
- UI "Bảng tổng hợp (Pivot)": hộp thoại chèn, panel danh sách trường (tick — trường ngày vào Hàng —, kéo thả, nút
  cài đặt trên từng chip), hộp thoại cài đặt trường hàng/cột (tổng phụ, bố cục, sắp xếp, lọc, nhóm), cài đặt trường
  giá trị (hàm, hiển thị giá trị dạng, trường/mục cơ sở, định dạng số, tên), tùy chọn (bố cục, tổng cộng, kiểu),
  trường tính toán, di chuyển, đổi nguồn; tab ribbon "Bảng tổng hợp" với các lệnh `pivot.*` trong
  `CommandRegistry`. Chuỗi ở cả `vi-VN.json` và `en-US.json`.
- Kiểm thử: bảng kỳ vọng tính tay trong `crates/bo-sheet/src/pivot/tests.rs` (tổng phụ/tổng cộng, bố cục, nhóm,
  trường tính toán, mọi kiểu hiển thị giá trị, sắp xếp, lọc, thao tác, kiểu); `group.rs`/`calc.rs`/`style.rs`;
  engine `pivot_grouping_layout_show_as_and_management`. Corpus `writer-pivots-{1,2,3}`, `writer-pivot-totals-{1,2,3}`,
  `writer-pivot-groups-{1,2,3}.xlsx` (OpenXmlValidator hợp lệ, round-trip thiết lập trong
  `corpus_pivot_features_round_trip`); E2E `sheet-pivot.spec.ts` và `sheet-pivot-settings.spec.ts` (nhóm ngày theo
  tháng, bố cục phác thảo, % tổng cộng, lưu, mở lại, thiết lập còn nguyên, tính lại).

**Bổ sung (biểu đồ tổng hợp, mục tính toán, nhóm thủ công, lọc ngày, giá trị trên hàng)**
- Biểu đồ tổng hợp (Pivot): `Chart::pivot_source` ↔ `c:pivotSource` (§21.2.2.142: `c:name` dạng
  `Sheet!TênBảng`, `c:fmtId`), `c:pivotFmts` (§21.2.2.144) mỗi series một `pivotFmt`, nút trường tùy chọn qua
  `c14:pivotOptions` ([MS-ODRAWXML]). `bo_sheet::chart::pivot_series` dựng series từ `location` /
  `firstDataRow` / `firstDataCol` / `rowItems` / `colItems`: danh mục = mục hàng trong cùng (nhãn ngoài nối " / "),
  một series cho mỗi mục cột trong cùng × trường giá trị, bỏ tổng phụ/tổng cộng; tham chiếu ô khi điểm liền nhau,
  ngược lại ghi giá trị literal. Sau mỗi lần dựng báo cáo (làm mới, lọc, đổi bố cục, nhóm, mục tính toán…)
  `sync_pivot_charts` cập nhật series (biểu đồ từ file được sinh lại part); di chuyển bảng sang sheet khác đổi
  `pivotSource`, xoá bảng biến biểu đồ thành biểu đồ thường. Lệnh `pivotChart` (ribbon "Bảng tổng hợp" →
  "Biểu đồ tổng hợp", `pivot.chart`), đổi kiểu bằng công cụ biểu đồ có sẵn; vẽ bằng bộ vẽ biểu đồ của BlueSheet.
- Mục tính toán (`calculatedItems` §18.10.1.9 / `calculatedItem` §18.10.1.8, `item/@f`): công thức trên các mục
  khác của cùng trường (tên trần hoặc `'có dấu cách'`), tính trên **tổng** của các mục đó trong từng ô báo cáo
  (ngữ cảnh các trường khác giữ nguyên); tổng phụ/tổng cộng cộng cả mục tính toán (như mô hình của đặc tả);
  tên mục là một shared item của trường. Lệnh `pivotCalcItem` (thêm / sửa theo `old` / xoá), hộp thoại
  "Mục tính toán". Làm mới giữ mục tính toán.
- Nhóm thủ công (`discretePr` §18.10.1.26, `fieldGroup/@base` / `@par` §18.10.1.27): chọn ô nhãn mục →
  "Nhóm mục đã chọn" tạo trường nhóm mới (tên theo trường gốc, đặt trước trường gốc trên trục) với nhóm
  "Nhóm {n}"; chọn tiếp trên trường gốc hoặc trường nhóm để thêm nhóm; "Bỏ nhóm" trả mục về chính nó, trường nhóm
  bị xoá khi không còn nhóm. Lệnh `pivotGroupItems` (`ungroup`). Trường có mục tính toán không nhóm được.
- Lọc ngày (§18.18.60 `ST_PivotFilterType`): `dateEqual` … `dateNotBetween` (so theo ngày, giá trị là serial
  hoặc ISO `yyyy-mm-dd`), kỳ tương đối `today` / `thisWeek` (tuần bắt đầu Chủ nhật) / `thisMonth` / `thisQuarter`
  / `thisYear` / `yearToDate` và trước/sau, `Q1–Q4`, `M1–M12`. Ghi `customFilters` (serial) hoặc `dynamicFilter`
  (§18.3.2.5). Ngày hôm nay lấy từ đồng hồ engine (`PivotLabels::today`); chưa có đồng hồ thì kỳ tương đối giữ
  mọi mục. UI: chế độ "Lọc ngày" trong cài đặt trường ngày.
- Nhiều trường giá trị trên trục hàng (`dataOnRows`): mỗi dòng có giá trị lặp lại một lần cho từng trường giá
  trị; dạng bảng/phác thảo thêm cột "Giá trị" chứa tên trường, thu gọn: dòng mục rồi các tên giá trị thụt lề
  bên dưới; `rowItems` có chỉ số trường giá trị ở cuối (`i`), định dạng số theo dòng. Ribbon "Giá trị trên hàng".
- `date1904`: `startDate` / `endDate` của nhóm ngày và mục `d` được ghi/đọc đúng ngày lịch (dời 1462 ngày).
- Ngày gõ dạng chữ (`bo_sheet::numfmt::input`): `dd/mm/yyyy` (vi-VN) hoặc `m/d/yyyy` (en-US) theo thứ tự ngày
  của locale, ISO `yyyy-mm-dd[ T]hh:mm`, giờ `h:mm[:ss] [SA/CH|AM/PM]`, năm 2 chữ số (00–29 → 20xx) thành serial
  với định dạng dựng sẵn 14 / 18–22 khi ô đang General; ô định dạng chữ `@`, công thức, `'…` và dạng hai phần
  (`1/2`) không đổi; ngày không hợp lệ (`31/02/2024`, tháng 13) giữ là chữ. Thanh công thức hiện ngày.
  Nhờ đó cột ngày gõ tay nhóm theo ngày được trong bảng tổng hợp.
- Biểu đồ trong `absoluteAnchor` (§20.5.2.1: `pos` + `ext`) và `oneCellAnchor` (§20.5.2.24: `from` + `ext`):
  khi mở được quy về ô (góc làm tròn tới cạnh ô gần nhất); kéo / đổi cỡ ghi lại `pos` / `ext` (EMU) hoặc `from` /
  `ext` trong drawing opaque.
- Kiểm thử: `pivot/tests.rs` (`calculated_items_rows_and_columns`, `manual_groups_and_ungroup`, `date_filters`,
  `values_on_rows`), `numfmt::input`, engine `typed_dates_become_serials`, `pivot_charts_items_groups_date_filters`,
  `absolute_and_one_cell_charts_move_and_resize`; corpus `writer-pivot-{items,manual,dates,datarows,charts}-{1,2,3}`,
  `chart-anchors-{1,2,3}.xlsx` (OpenXmlValidator hợp lệ), `corpus_pivot_charts_round_trip`,
  `pivot_dates_in_1904_workbooks`; E2E `sheet-pivot-extras.spec.ts`.

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
- PivotTable: `rowItems`/`colItems` ghi đủ đường dẫn (`r="0"`); pivot từ file chỉ tính lại được khi nguồn là
  vùng/tên/bảng trong workbook; đổi tên sheet nguồn không cập nhật part cache opaque cho tới khi làm mới.
- Biểu đồ tổng hợp: chưa ghi `chartFormats` (§18.10.1.13) / `@chartFormat` của bảng; nút trường chỉ ghi
  (`c14:pivotOptions`), chưa vẽ; biểu đồ tổng hợp đọc từ file bị sinh lại part (mất định dạng riêng) khi bảng thay
  đổi; danh mục nhiều cấp được nối thành một nhãn (chưa `multiLvlStrRef`); nhãn ngày trong cache biểu đồ định dạng
  theo vi-VN.
- Mục tính toán: mọi hàm tổng hợp nhận đúng một số (Count của mục tính toán là 1); không hỗ trợ trên trường đã
  nhóm; kết quả lỗi công thức thành ô trống.
- Nhóm thủ công: không đặt tên nhóm tùy ý từ UI (dùng "Nhóm {n}"); chưa nhóm của nhóm (trường nhóm cấp 2).
- Lọc ngày bỏ qua trên trường đã nhóm theo ngày; kỳ tương đối của pivot đọc từ file chỉ áp dụng khi báo cáo được
  tính lại; ô chọn ngày của UI chỉ hiện giá trị ISO (filter đọc từ file dạng serial hiện trống).
- Ngày gõ: chưa nhận dạng tháng bằng chữ (`15 Thg 1 2024`, `Jan 15`), dạng hai phần `d/m` (năm hiện tại).
- Biểu đồ `absoluteAnchor` / `oneCellAnchor` hiển thị và kéo theo ô (bỏ phần lẻ trong ô, như `twoCellAnchor`).
- Cắt–dán sang sheet khác chưa thêm tên sheet nguồn cho tham chiếu tương đối không qualified bên trong khối cắt;
  chart/CF/DV chưa cập nhật theo ô bị di chuyển.
- `dynamicFilter` có `valIso`/`maxValIso`, `iconFilter` vẫn giữ nguyên XML, chưa đánh giá.
- Comments part: chỉ giữ authors + commentList (threaded comments vẫn opaque).
- Màu series lấy theo bảng màu BlueSheet, chưa đọc `c:spPr` của file.