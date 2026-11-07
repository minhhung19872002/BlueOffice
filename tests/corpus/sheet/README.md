# Corpus BlueSheet (`tests/corpus/sheet`)

Toàn bộ file trong thư mục này do BlueOffice **tự sinh** từ SpreadsheetML viết tay
(theo ECMA-376 Part 1, §18) trong `crates/bo-xlsx/tests/corpus.rs` — không chứa dữ liệu
khách hàng, không tạo bằng phần mềm của bên thứ ba. Sinh lại:

```bash
BO_WRITE_CORPUS=1 cargo test -p bo-xlsx --test corpus generate_corpus
```

Kiểm thử round-trip: `cargo test -p bo-xlsx --test corpus corpus_roundtrip -- --nocapture`.

| File | Tính năng | Nguồn gốc |
|---|---|---|
| `values-1.xlsx` | numbers, booleans, errors, dates (t="d", 1904 system), normal/shared/array formulas with cached values (biến thể 1) | tự sinh, `corpus.rs::values` |
| `values-2.xlsx` | numbers, booleans, errors, dates (t="d", 1904 system), normal/shared/array formulas with cached values (biến thể 2) | tự sinh, `corpus.rs::values` |
| `values-3.xlsx` | numbers, booleans, errors, dates (t="d", 1904 system), normal/shared/array formulas with cached values (biến thể 3) | tự sinh, `corpus.rs::values` |
| `sst-1.xlsx` | shared strings with rich-text runs, xml:space, _xHHHH_ escapes (biến thể 1) | tự sinh, `corpus.rs::sst` |
| `sst-2.xlsx` | shared strings with rich-text runs, xml:space, _xHHHH_ escapes (biến thể 2) | tự sinh, `corpus.rs::sst` |
| `sst-3.xlsx` | shared strings with rich-text runs, xml:space, _xHHHH_ escapes (biến thể 3) | tự sinh, `corpus.rs::sst` |
| `styles-1.xlsx` | numFmts, fonts, pattern/gradient fills, borders, cellXfs, cellStyleXfs, cellStyles, dxfs (biến thể 1) | tự sinh, `corpus.rs::styles` |
| `styles-2.xlsx` | numFmts, fonts, pattern/gradient fills, borders, cellXfs, cellStyleXfs, cellStyles, dxfs (biến thể 2) | tự sinh, `corpus.rs::styles` |
| `styles-3.xlsx` | numFmts, fonts, pattern/gradient fills, borders, cellXfs, cellStyleXfs, cellStyles, dxfs (biến thể 3) | tự sinh, `corpus.rs::styles` |
| `layout-1.xlsx` | column widths, row heights, hidden rows/cols, outline levels, merges (biến thể 1) | tự sinh, `corpus.rs::layout` |
| `layout-2.xlsx` | column widths, row heights, hidden rows/cols, outline levels, merges (biến thể 2) | tự sinh, `corpus.rs::layout` |
| `layout-3.xlsx` | column widths, row heights, hidden rows/cols, outline levels, merges (biến thể 3) | tự sinh, `corpus.rs::layout` |
| `panes-1.xlsx` | frozen rows, frozen rows+cols, split panes, selections, zoom, hidden gridlines/headers (biến thể 1) | tự sinh, `corpus.rs::panes` |
| `panes-2.xlsx` | frozen rows, frozen rows+cols, split panes, selections, zoom, hidden gridlines/headers (biến thể 2) | tự sinh, `corpus.rs::panes` |
| `panes-3.xlsx` | frozen rows, frozen rows+cols, split panes, selections, zoom, hidden gridlines/headers (biến thể 3) | tự sinh, `corpus.rs::panes` |
| `names-1.xlsx` | defined names (workbook and sheet scope), print area, print titles (biến thể 1) | tự sinh, `corpus.rs::names` |
| `names-2.xlsx` | defined names (workbook and sheet scope), print area, print titles (biến thể 2) | tự sinh, `corpus.rs::names` |
| `names-3.xlsx` | defined names (workbook and sheet scope), print area, print titles (biến thể 3) | tự sinh, `corpus.rs::names` |
| `page-1.xlsx` | printOptions, pageMargins, pageSetup, headerFooter variants, row/col breaks (biến thể 1) | tự sinh, `corpus.rs::page` |
| `page-2.xlsx` | printOptions, pageMargins, pageSetup, headerFooter variants, row/col breaks (biến thể 2) | tự sinh, `corpus.rs::page` |
| `page-3.xlsx` | printOptions, pageMargins, pageSetup, headerFooter variants, row/col breaks (biến thể 3) | tự sinh, `corpus.rs::page` |
| `links-1.xlsx` | external (relationship) and internal hyperlinks, tooltips (biến thể 1) | tự sinh, `corpus.rs::links` |
| `links-2.xlsx` | external (relationship) and internal hyperlinks, tooltips (biến thể 2) | tự sinh, `corpus.rs::links` |
| `links-3.xlsx` | external (relationship) and internal hyperlinks, tooltips (biến thể 3) | tự sinh, `corpus.rs::links` |
| `comments-1.xlsx` | comments part with rich text + VML legacy drawing (biến thể 1) | tự sinh, `corpus.rs::comments` |
| `comments-2.xlsx` | comments part with rich text + VML legacy drawing (biến thể 2) | tự sinh, `corpus.rs::comments` |
| `comments-3.xlsx` | comments part with rich text + VML legacy drawing (biến thể 3) | tự sinh, `corpus.rs::comments` |
| `validation-1.xlsx` | data validation: list, whole, decimal, date, textLength, custom (biến thể 1) | tự sinh, `corpus.rs::validation` |
| `validation-2.xlsx` | data validation: list, whole, decimal, date, textLength, custom (biến thể 2) | tự sinh, `corpus.rs::validation` |
| `validation-3.xlsx` | data validation: list, whole, decimal, date, textLength, custom (biến thể 3) | tự sinh, `corpus.rs::validation` |
| `cf-1.xlsx` | conditional formatting: cellIs, expression, colorScale, dataBar, iconSet, top10, text, x14 ext (biến thể 1) | tự sinh, `corpus.rs::cf` |
| `cf-2.xlsx` | conditional formatting: cellIs, expression, colorScale, dataBar, iconSet, top10, text, x14 ext (biến thể 2) | tự sinh, `corpus.rs::cf` |
| `cf-3.xlsx` | conditional formatting: cellIs, expression, colorScale, dataBar, iconSet, top10, text, x14 ext (biến thể 3) | tự sinh, `corpus.rs::cf` |
| `tables-1.xlsx` | tables (ListObject) with autoFilter, totals row, calculated columns, style info (biến thể 1) | tự sinh, `corpus.rs::tables` |
| `tables-2.xlsx` | tables (ListObject) with autoFilter, totals row, calculated columns, style info (biến thể 2) | tự sinh, `corpus.rs::tables` |
| `tables-3.xlsx` | tables (ListObject) with autoFilter, totals row, calculated columns, style info (biến thể 3) | tự sinh, `corpus.rs::tables` |
| `charts-1.xlsx` | drawing + chartSpace parts (bar, bar+line combo, pie) (biến thể 1) | tự sinh, `corpus.rs::charts` |
| `charts-2.xlsx` | drawing + chartSpace parts (bar, bar+line combo, pie) (biến thể 2) | tự sinh, `corpus.rs::charts` |
| `charts-3.xlsx` | drawing + chartSpace parts (bar, bar+line combo, pie) (biến thể 3) | tự sinh, `corpus.rs::charts` |
| `ext-1.xlsx` | extLst, mc:AlternateContent, x14ac attributes, sheet autoFilter + sortState, controls, customXml (biến thể 1) | tự sinh, `corpus.rs::ext` |
| `ext-2.xlsx` | extLst, mc:AlternateContent, x14ac attributes, sheet autoFilter + sortState, controls, customXml (biến thể 2) | tự sinh, `corpus.rs::ext` |
| `ext-3.xlsx` | extLst, mc:AlternateContent, x14ac attributes, sheet autoFilter + sortState, controls, customXml (biến thể 3) | tự sinh, `corpus.rs::ext` |
| `writer-format-1.xlsx` | BlueSheet writer: cell formats (vi-VN number formats, fonts, fills, borders, alignment) and a gallery cell style (biến thể 1) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_format` |
| `writer-format-2.xlsx` | BlueSheet writer: cell formats (vi-VN number formats, fonts, fills, borders, alignment) and a gallery cell style (biến thể 2) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_format` |
| `writer-format-3.xlsx` | BlueSheet writer: cell formats (vi-VN number formats, fonts, fills, borders, alignment) and a gallery cell style (biến thể 3) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_format` |
| `writer-sheet-1.xlsx` | BlueSheet writer: column width / row height, freeze panes, merge, AutoFilter, conditional formats, list validation, page setup + print titles (biến thể 1) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_sheet` |
| `writer-sheet-2.xlsx` | BlueSheet writer: column width / row height, freeze panes, merge, AutoFilter, conditional formats, list validation, page setup + print titles (biến thể 2) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_sheet` |
| `writer-sheet-3.xlsx` | BlueSheet writer: column width / row height, freeze panes, merge, AutoFilter, conditional formats, list validation, page setup + print titles (biến thể 3) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_sheet` |
| `writer-charts-1.xlsx` | BlueSheet writer: inserted charts (column, column+line combo, doughnut, scatter) with axis titles and legend (biến thể 1) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_charts` |
| `writer-charts-2.xlsx` | BlueSheet writer: inserted charts (column, column+line combo, doughnut, scatter) with axis titles and legend (biến thể 2) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_charts` |
| `writer-charts-3.xlsx` | BlueSheet writer: inserted charts (column, column+line combo, doughnut, scatter) with axis titles and legend (biến thể 3) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_charts` |
| `writer-pivots-1.xlsx` | BlueSheet writer: pivot tables (rows + Sum; rows × columns with a report filter on another sheet; two row fields, four value fields, multi-item filter) (biến thể 1) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_pivots` |
| `writer-pivots-2.xlsx` | BlueSheet writer: pivot tables (rows + Sum; rows × columns with a report filter on another sheet; two row fields, four value fields, multi-item filter) (biến thể 2) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_pivots` |
| `writer-pivots-3.xlsx` | BlueSheet writer: pivot tables (rows + Sum; rows × columns with a report filter on another sheet; two row fields, four value fields, multi-item filter) (biến thể 3) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_pivots` |
| `writer-pivot-totals-1.xlsx` | BlueSheet writer: pivot report settings (tabular with repeated labels + blank rows and column subtotals; outline with bottom subtotals, sort by value, top-N + label filter, medium style; compact with number format, % of total / parent, rank, value filter, hidden item, dark style) (biến thể 1) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_pivot_totals` |
| `writer-pivot-totals-2.xlsx` | BlueSheet writer: pivot report settings (tabular with repeated labels + blank rows and column subtotals; outline with bottom subtotals, sort by value, top-N + label filter, medium style; compact with number format, % of total / parent, rank, value filter, hidden item, dark style) (biến thể 2) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_pivot_totals` |
| `writer-pivot-totals-3.xlsx` | BlueSheet writer: pivot report settings (tabular with repeated labels + blank rows and column subtotals; outline with bottom subtotals, sort by value, top-N + label filter, medium style; compact with number format, % of total / parent, rank, value filter, hidden item, dark style) (biến thể 3) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_pivot_totals` |
| `writer-pivot-groups-1.xlsx` | BlueSheet writer: pivot grouping and calculations (dates by years / quarters / months; numeric ranges × columns; years + months with a calculated field, difference from previous and running total) (biến thể 1) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_pivot_groups` |
| `writer-pivot-groups-2.xlsx` | BlueSheet writer: pivot grouping and calculations (dates by years / quarters / months; numeric ranges × columns; years + months with a calculated field, difference from previous and running total) (biến thể 2) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_pivot_groups` |
| `writer-pivot-groups-3.xlsx` | BlueSheet writer: pivot grouping and calculations (dates by years / quarters / months; numeric ranges × columns; years + months with a calculated field, difference from previous and running total) (biến thể 3) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_pivot_groups` |
