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
| `chart-anchors-1.xlsx` | charts in a oneCellAnchor (from + ext) and absoluteAnchor (pos + ext) (biến thể 1) | tự sinh, `corpus.rs::chart-anchors` |
| `chart-anchors-2.xlsx` | charts in a oneCellAnchor (from + ext) and absoluteAnchor (pos + ext) (biến thể 2) | tự sinh, `corpus.rs::chart-anchors` |
| `chart-anchors-3.xlsx` | charts in a oneCellAnchor (from + ext) and absoluteAnchor (pos + ext) (biến thể 3) | tự sinh, `corpus.rs::chart-anchors` |
| `ext-1.xlsx` | extLst, mc:AlternateContent, x14ac attributes, sheet autoFilter + sortState, controls, customXml (biến thể 1) | tự sinh, `corpus.rs::ext` |
| `ext-2.xlsx` | extLst, mc:AlternateContent, x14ac attributes, sheet autoFilter + sortState, controls, customXml (biến thể 2) | tự sinh, `corpus.rs::ext` |
| `ext-3.xlsx` | extLst, mc:AlternateContent, x14ac attributes, sheet autoFilter + sortState, controls, customXml (biến thể 3) | tự sinh, `corpus.rs::ext` |
| `smartart-1.xlsx` | SmartArt: dgm:relIds graphic frame, diagram parts and dsp:drawing fallback (biến thể 1) | tự sinh, `corpus.rs::smartart` |
| `smartart-2.xlsx` | SmartArt: dgm:relIds graphic frame, diagram parts and dsp:drawing fallback (biến thể 2) | tự sinh, `corpus.rs::smartart` |
| `smartart-3.xlsx` | SmartArt: dgm:relIds graphic frame, diagram parts and dsp:drawing fallback (biến thể 3) | tự sinh, `corpus.rs::smartart` |
| `threaded-1.xlsx` | threaded comments + replies + resolved thread ([MS-XLSX] threadedComments, persons) with legacy notes (biến thể 1) | tự sinh, `corpus.rs::threaded` |
| `threaded-2.xlsx` | threaded comments + replies + resolved thread ([MS-XLSX] threadedComments, persons) with legacy notes (biến thể 2) | tự sinh, `corpus.rs::threaded` |
| `threaded-3.xlsx` | threaded comments + replies + resolved thread ([MS-XLSX] threadedComments, persons) with legacy notes (biến thể 3) | tự sinh, `corpus.rs::threaded` |
| `writer-format-1.xlsx` | BlueSheet writer: cell formats (vi-VN number formats, fonts, fills, borders, alignment) and a gallery cell style (biến thể 1) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_format` |
| `writer-format-2.xlsx` | BlueSheet writer: cell formats (vi-VN number formats, fonts, fills, borders, alignment) and a gallery cell style (biến thể 2) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_format` |
| `writer-format-3.xlsx` | BlueSheet writer: cell formats (vi-VN number formats, fonts, fills, borders, alignment) and a gallery cell style (biến thể 3) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_format` |
| `writer-sheet-1.xlsx` | BlueSheet writer: column width / row height, freeze panes, merge, AutoFilter, conditional formats, list validation, page setup + print titles (biến thể 1) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_sheet` |
| `writer-sheet-2.xlsx` | BlueSheet writer: column width / row height, freeze panes, merge, AutoFilter, conditional formats, list validation, page setup + print titles (biến thể 2) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_sheet` |
| `writer-sheet-3.xlsx` | BlueSheet writer: column width / row height, freeze panes, merge, AutoFilter, conditional formats, list validation, page setup + print titles (biến thể 3) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_sheet` |
| `writer-charts-1.xlsx` | BlueSheet writer: inserted charts (column, column+line combo, doughnut, scatter) with axis titles and legend (biến thể 1) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_charts` |
| `writer-charts-2.xlsx` | BlueSheet writer: inserted charts (column, column+line combo, doughnut, scatter) with axis titles and legend (biến thể 2) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_charts` |
| `writer-charts-3.xlsx` | BlueSheet writer: inserted charts (column, column+line combo, doughnut, scatter) with axis titles and legend (biến thể 3) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_charts` |
| `writer-charts-components-1.xlsx` | BlueSheet writer: chart components (linear trend line, fixed error bars, drop lines; exponential and polynomial trend lines, percentage error bars, data table, log axis; moving average, standard-deviation error bars, high-low lines, up/down bars, date axis) (biến thể 1) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_charts_components` |
| `writer-charts-components-2.xlsx` | BlueSheet writer: chart components (linear trend line, fixed error bars, drop lines; exponential and polynomial trend lines, percentage error bars, data table, log axis; moving average, standard-deviation error bars, high-low lines, up/down bars, date axis) (biến thể 2) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_charts_components` |
| `writer-charts-components-3.xlsx` | BlueSheet writer: chart components (linear trend line, fixed error bars, drop lines; exponential and polynomial trend lines, percentage error bars, data table, log axis; moving average, standard-deviation error bars, high-low lines, up/down bars, date axis) (biến thể 3) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_charts_components` |
| `writer-charts-percent-1.xlsx` | BlueSheet writer: 100 % stacked column, bar and area charts (biến thể 1) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_charts_percent` |
| `writer-charts-percent-2.xlsx` | BlueSheet writer: 100 % stacked column, bar and area charts (biến thể 2) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_charts_percent` |
| `writer-charts-percent-3.xlsx` | BlueSheet writer: 100 % stacked column, bar and area charts (biến thể 3) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_charts_percent` |
| `writer-charts-bubble-1.xlsx` | BlueSheet writer: bubble charts (one and two series of X, Y and size; negative values) (biến thể 1) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_charts_bubble` |
| `writer-charts-bubble-2.xlsx` | BlueSheet writer: bubble charts (one and two series of X, Y and size; negative values) (biến thể 2) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_charts_bubble` |
| `writer-charts-bubble-3.xlsx` | BlueSheet writer: bubble charts (one and two series of X, Y and size; negative values) (biến thể 3) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_charts_bubble` |
| `writer-charts-radar-1.xlsx` | BlueSheet writer: radar charts (standard, with markers, filled) (biến thể 1) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_charts_radar` |
| `writer-charts-radar-2.xlsx` | BlueSheet writer: radar charts (standard, with markers, filled) (biến thể 2) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_charts_radar` |
| `writer-charts-radar-3.xlsx` | BlueSheet writer: radar charts (standard, with markers, filled) (biến thể 3) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_charts_radar` |
| `writer-charts-stock-1.xlsx` | BlueSheet writer: stock charts (high-low-close; open-high-low-close; with up/down bars) (biến thể 1) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_charts_stock` |
| `writer-charts-stock-2.xlsx` | BlueSheet writer: stock charts (high-low-close; open-high-low-close; with up/down bars) (biến thể 2) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_charts_stock` |
| `writer-charts-stock-3.xlsx` | BlueSheet writer: stock charts (high-low-close; open-high-low-close; with up/down bars) (biến thể 3) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_charts_stock` |
| `writer-charts-surface-1.xlsx` | BlueSheet writer: surface charts (3 series; wireframe; 5 series) (biến thể 1) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_charts_surface` |
| `writer-charts-surface-2.xlsx` | BlueSheet writer: surface charts (3 series; wireframe; 5 series) (biến thể 2) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_charts_surface` |
| `writer-charts-surface-3.xlsx` | BlueSheet writer: surface charts (3 series; wireframe; 5 series) (biến thể 3) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_charts_surface` |
| `writer-charts-ofpie-1.xlsx` | BlueSheet writer: pie of pie, bar of pie and an exploded pie (biến thể 1) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_charts_ofpie` |
| `writer-charts-ofpie-2.xlsx` | BlueSheet writer: pie of pie, bar of pie and an exploded pie (biến thể 2) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_charts_ofpie` |
| `writer-charts-ofpie-3.xlsx` | BlueSheet writer: pie of pie, bar of pie and an exploded pie (biến thể 3) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_charts_ofpie` |
| `writer-charts-3dbar-1.xlsx` | BlueSheet writer: 3-D columns (series one behind the other), 3-D cylinders, stacked 3-D bars (biến thể 1) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_charts_3dbar` |
| `writer-charts-3dbar-2.xlsx` | BlueSheet writer: 3-D columns (series one behind the other), 3-D cylinders, stacked 3-D bars (biến thể 2) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_charts_3dbar` |
| `writer-charts-3dbar-3.xlsx` | BlueSheet writer: 3-D columns (series one behind the other), 3-D cylinders, stacked 3-D bars (biến thể 3) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_charts_3dbar` |
| `writer-charts-3dline-1.xlsx` | BlueSheet writer: 3-D line, 3-D area, 100 % stacked 3-D area (biến thể 1) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_charts_3dline` |
| `writer-charts-3dline-2.xlsx` | BlueSheet writer: 3-D line, 3-D area, 100 % stacked 3-D area (biến thể 2) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_charts_3dline` |
| `writer-charts-3dline-3.xlsx` | BlueSheet writer: 3-D line, 3-D area, 100 % stacked 3-D area (biến thể 3) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_charts_3dline` |
| `writer-charts-3dpie-1.xlsx` | BlueSheet writer: 3-D pie, exploded 3-D pie, 3-D surface (biến thể 1) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_charts_3dpie` |
| `writer-charts-3dpie-2.xlsx` | BlueSheet writer: 3-D pie, exploded 3-D pie, 3-D surface (biến thể 2) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_charts_3dpie` |
| `writer-charts-3dpie-3.xlsx` | BlueSheet writer: 3-D pie, exploded 3-D pie, 3-D surface (biến thể 3) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_charts_3dpie` |
| `writer-chartex-waterfall-1.xlsx` | BlueSheet writer: chartex waterfall (with a total; ups and downs; with a total) (biến thể 1) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_chartex_waterfall` |
| `writer-chartex-waterfall-2.xlsx` | BlueSheet writer: chartex waterfall (with a total; ups and downs; with a total) (biến thể 2) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_chartex_waterfall` |
| `writer-chartex-waterfall-3.xlsx` | BlueSheet writer: chartex waterfall (with a total; ups and downs; with a total) (biến thể 3) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_chartex_waterfall` |
| `writer-chartex-treemap-1.xlsx` | BlueSheet writer: chartex treemap (2 levels; 3 levels; 2 levels, 5 points) (biến thể 1) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_chartex_treemap` |
| `writer-chartex-treemap-2.xlsx` | BlueSheet writer: chartex treemap (2 levels; 3 levels; 2 levels, 5 points) (biến thể 2) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_chartex_treemap` |
| `writer-chartex-treemap-3.xlsx` | BlueSheet writer: chartex treemap (2 levels; 3 levels; 2 levels, 5 points) (biến thể 3) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_chartex_treemap` |
| `writer-chartex-sunburst-1.xlsx` | BlueSheet writer: chartex sunburst (2 levels; 3 levels; 2 levels, 5 points) (biến thể 1) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_chartex_sunburst` |
| `writer-chartex-sunburst-2.xlsx` | BlueSheet writer: chartex sunburst (2 levels; 3 levels; 2 levels, 5 points) (biến thể 2) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_chartex_sunburst` |
| `writer-chartex-sunburst-3.xlsx` | BlueSheet writer: chartex sunburst (2 levels; 3 levels; 2 levels, 5 points) (biến thể 3) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_chartex_sunburst` |
| `writer-chartex-histogram-1.xlsx` | BlueSheet writer: chartex histogram (automatic bins; 6 bins; 15 values) (biến thể 1) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_chartex_histogram` |
| `writer-chartex-histogram-2.xlsx` | BlueSheet writer: chartex histogram (automatic bins; 6 bins; 15 values) (biến thể 2) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_chartex_histogram` |
| `writer-chartex-histogram-3.xlsx` | BlueSheet writer: chartex histogram (automatic bins; 6 bins; 15 values) (biến thể 3) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_chartex_histogram` |
| `writer-chartex-pareto-1.xlsx` | BlueSheet writer: chartex pareto (categories sorted by count, cumulative line) (biến thể 1) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_chartex_pareto` |
| `writer-chartex-pareto-2.xlsx` | BlueSheet writer: chartex pareto (categories sorted by count, cumulative line) (biến thể 2) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_chartex_pareto` |
| `writer-chartex-pareto-3.xlsx` | BlueSheet writer: chartex pareto (categories sorted by count, cumulative line) (biến thể 3) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_chartex_pareto` |
| `writer-chartex-box-1.xlsx` | BlueSheet writer: chartex box & whisker (one series; two series; an outlier) (biến thể 1) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_chartex_box` |
| `writer-chartex-box-2.xlsx` | BlueSheet writer: chartex box & whisker (one series; two series; an outlier) (biến thể 2) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_chartex_box` |
| `writer-chartex-box-3.xlsx` | BlueSheet writer: chartex box & whisker (one series; two series; an outlier) (biến thể 3) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_chartex_box` |
| `writer-chartex-funnel-1.xlsx` | BlueSheet writer: chartex funnel (4, 3 and 5 stages) (biến thể 1) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_chartex_funnel` |
| `writer-chartex-funnel-2.xlsx` | BlueSheet writer: chartex funnel (4, 3 and 5 stages) (biến thể 2) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_chartex_funnel` |
| `writer-chartex-funnel-3.xlsx` | BlueSheet writer: chartex funnel (4, 3 and 5 stages) (biến thể 3) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_chartex_funnel` |
| `writer-pivots-1.xlsx` | BlueSheet writer: pivot tables (rows + Sum; rows × columns with a report filter on another sheet; two row fields, four value fields, multi-item filter) (biến thể 1) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_pivots` |
| `writer-pivots-2.xlsx` | BlueSheet writer: pivot tables (rows + Sum; rows × columns with a report filter on another sheet; two row fields, four value fields, multi-item filter) (biến thể 2) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_pivots` |
| `writer-pivots-3.xlsx` | BlueSheet writer: pivot tables (rows + Sum; rows × columns with a report filter on another sheet; two row fields, four value fields, multi-item filter) (biến thể 3) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_pivots` |
| `writer-pivot-totals-1.xlsx` | BlueSheet writer: pivot report settings (tabular with repeated labels + blank rows and column subtotals; outline with bottom subtotals, sort by value, top-N + label filter, medium style; compact with number format, % of total / parent, rank, value filter, hidden item, dark style) (biến thể 1) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_pivot_totals` |
| `writer-pivot-totals-2.xlsx` | BlueSheet writer: pivot report settings (tabular with repeated labels + blank rows and column subtotals; outline with bottom subtotals, sort by value, top-N + label filter, medium style; compact with number format, % of total / parent, rank, value filter, hidden item, dark style) (biến thể 2) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_pivot_totals` |
| `writer-pivot-totals-3.xlsx` | BlueSheet writer: pivot report settings (tabular with repeated labels + blank rows and column subtotals; outline with bottom subtotals, sort by value, top-N + label filter, medium style; compact with number format, % of total / parent, rank, value filter, hidden item, dark style) (biến thể 3) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_pivot_totals` |
| `writer-pivot-groups-1.xlsx` | BlueSheet writer: pivot grouping and calculations (dates by years / quarters / months; numeric ranges × columns; years + months with a calculated field, difference from previous and running total) (biến thể 1) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_pivot_groups` |
| `writer-pivot-groups-2.xlsx` | BlueSheet writer: pivot grouping and calculations (dates by years / quarters / months; numeric ranges × columns; years + months with a calculated field, difference from previous and running total) (biến thể 2) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_pivot_groups` |
| `writer-pivot-groups-3.xlsx` | BlueSheet writer: pivot grouping and calculations (dates by years / quarters / months; numeric ranges × columns; years + months with a calculated field, difference from previous and running total) (biến thể 3) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_pivot_groups` |
| `writer-pivot-items-1.xlsx` | BlueSheet writer: calculated items (row field; column field in outline form; two items in compact form) (biến thể 1) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_pivot_items` |
| `writer-pivot-items-2.xlsx` | BlueSheet writer: calculated items (row field; column field in outline form; two items in compact form) (biến thể 2) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_pivot_items` |
| `writer-pivot-items-3.xlsx` | BlueSheet writer: calculated items (row field; column field in outline form; two items in compact form) (biến thể 3) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_pivot_items` |
| `writer-pivot-manual-1.xlsx` | BlueSheet writer: manual item groups (rows; two groups on a column field; group then ungroup one item) (biến thể 1) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_pivot_manual` |
| `writer-pivot-manual-2.xlsx` | BlueSheet writer: manual item groups (rows; two groups on a column field; group then ungroup one item) (biến thể 2) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_pivot_manual` |
| `writer-pivot-manual-3.xlsx` | BlueSheet writer: manual item groups (rows; two groups on a column field; group then ungroup one item) (biến thể 3) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_pivot_manual` |
| `writer-pivot-dates-1.xlsx` | BlueSheet writer: pivot date filters (dateBetween; Q2; dateNewerThanOrEqual in a 1904 workbook with a year grouping) (biến thể 1) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_pivot_dates` |
| `writer-pivot-dates-2.xlsx` | BlueSheet writer: pivot date filters (dateBetween; Q2; dateNewerThanOrEqual in a 1904 workbook with a year grouping) (biến thể 2) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_pivot_dates` |
| `writer-pivot-dates-3.xlsx` | BlueSheet writer: pivot date filters (dateBetween; Q2; dateNewerThanOrEqual in a 1904 workbook with a year grouping) (biến thể 3) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_pivot_dates` |
| `writer-pivot-datarows-1.xlsx` | BlueSheet writer: value fields on rows (tabular; compact × columns; outline with three values) (biến thể 1) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_pivot_datarows` |
| `writer-pivot-datarows-2.xlsx` | BlueSheet writer: value fields on rows (tabular; compact × columns; outline with three values) (biến thể 2) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_pivot_datarows` |
| `writer-pivot-datarows-3.xlsx` | BlueSheet writer: value fields on rows (tabular; compact × columns; outline with three values) (biến thể 3) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_pivot_datarows` |
| `writer-pivot-charts-1.xlsx` | BlueSheet writer: pivot charts (column; line following a hidden item; combo with field buttons) (biến thể 1) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_pivot_charts` |
| `writer-pivot-charts-2.xlsx` | BlueSheet writer: pivot charts (column; line following a hidden item; combo with field buttons) (biến thể 2) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_pivot_charts` |
| `writer-pivot-charts-3.xlsx` | BlueSheet writer: pivot charts (column; line following a hidden item; combo with field buttons) (biến thể 3) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_pivot_charts` |
| `writer-slicers-1.xlsx` | BlueSheet writer: table slicers (one slicer; filtered to one item; two slicers on two columns, both filtered) (biến thể 1) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_slicers` |
| `writer-slicers-2.xlsx` | BlueSheet writer: table slicers (one slicer; filtered to one item; two slicers on two columns, both filtered) (biến thể 2) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_slicers` |
| `writer-slicers-3.xlsx` | BlueSheet writer: table slicers (one slicer; filtered to one item; two slicers on two columns, both filtered) (biến thể 3) | tự sinh bằng `write_xlsx`, `corpus.rs::writer_slicers` |
| `timeline-1.xlsx` | Timeline (x15 timelineCacheDefinition + timelines part) over a pivot table's date field, filtered to Q2 2023 | tự sinh, `corpus.rs::timeline_fixture` |
