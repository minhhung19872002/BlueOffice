# SpreadsheetML (.xlsx) — ghi chú đọc spec

Ghi chú clean-room: chỉ diễn giải và trích số mục, không chép văn bản spec. Nguồn: ECMA-376 Part 1 §18
(SpreadsheetML), §20.5 / §21.2 (drawing, chart), [MS-XLSX] (phần mở rộng `x14`), [MS-ODRAWXML] (`c14`),
RFC 4180 (CSV). Mọi mục dưới đây đều đối chiếu được bằng grep `§18.` trong các file nêu tên.

## 1. Phạm vi đã cài

- **Workbook** (§18.2): `workbookPr` §18.2.28 (cờ `date1904`), `sheets`/`sheet` §18.2.20/§18.2.19,
  `definedNames`/`definedName` §18.2.6/§18.2.5 (hai phạm vi, tên dựng sẵn gắn sheet), `pivotCaches` §18.2.22/§18.2.21
  — `crates/bo-xlsx/src/read.rs`, `crates/bo-xlsx/src/write.rs`, model ở `crates/bo-model/src/sheet.rs`.
- **Worksheet** (§18.3.1): `sheetPr` §18.3.1.82 (tabColor, outlinePr, pageSetUpPr), `dimension` §18.3.1.35 (tính lại
  khi ghi), `sheetViews`/`pane`/`selection` §18.3.1.88/.66/.78, `sheetFormatPr` §18.3.1.81, `cols`/`col` §18.3.1.17/.13,
  `sheetData`/`row`/`c`/`v`/`f`/`is` §18.3.1.80/.73/.4/.96/.40/.53, `mergeCells` §18.3.1.55, `hyperlinks` §18.3.1.48,
  `printOptions`/`pageMargins`/`pageSetup`/`headerFooter` §18.3.1.70/.62/.63/.46, `rowBreaks`/`colBreaks` §18.3.1.74/.14,
  `tableParts` §18.3.1.95 — `crates/bo-xlsx/src/read.rs`, `write.rs`; hàng/cột/gộp ô ở `crates/bo-sheet/src/rowcol.rs`,
  `geometry.rs`.
- **Kiểu ô** §18.18.11 `ST_CellType` (`n`, `s`, `b`, `e`, `str`, `inlineStr`, `d`) — `read.rs`; chuỗi có escape `_xHHHH_`
  (§22.9.2.19 `ST_Xstring`) — `crates/bo-xlsx/src/xml.rs` `xstring_decode`.
- **Shared strings** (§18.4): `sst`/`si`, rich run `rPr` §18.4.7, gạch chân §18.4.13 — `write.rs` (`SharedStrings`),
  `styles.rs` (font dùng chung cho `font` và `rPr`).
- **Styles** (§18.8): `numFmts`/`numFmt` §18.8.31/.30, `font` §18.8.22, `fill`/`patternFill`/`gradientFill`
  §18.8.20/.32/.24, `border` §18.8.4, `alignment` §18.8.1, `protection` §18.8.33, `xf` §18.8.45, `cellStyleXfs` §18.8.9,
  `cellStyles` §18.8.8/.7, `dxf` §18.8.14/`dxfs` §18.8.15, màu `CT_Color` §18.3.1.15 / §18.8.19, bảng màu chỉ số
  §18.8.27 — `crates/bo-xlsx/src/styles.rs`, `crates/bo-sheet/src/color.rs`, `crates/bo-sheet/src/style.rs`.
- **Định dạng số** §18.8.30–31 (4 section, điều kiện, màu, phân số, khoa học, ngày giờ, elapsed, tag locale/tiền tệ,
  `[$-F800]`/`[$-F400]`) và bảng id dựng sẵn 0–49 — `crates/bo-sheet/src/numfmt/{mod,builtin,locale}.rs`;
  serial ngày §18.17.4 / §18.17.4.1 — `numfmt/date.rs`; nhận dạng ngày gõ vào — `numfmt/input.rs`.
- **Conditional formatting** §18.3.1.18 / `cfRule` §18.3.1.10 (`cellIs` §18.18.12, toán tử §18.18.15), `cfvo`
  §18.3.1.11 / §18.18.13, colorScale §18.3.1.16, dataBar §18.3.1.28, iconSet §18.3.1.49 / §18.18.42 —
  `crates/bo-sheet/src/cf.rs`, `icons.rs`.
- **Data validation** §18.3.1.32 (`type` §18.18.21, `operator` §18.18.20, `errorStyle` §18.18.18) —
  `crates/bo-sheet/src/validation.rs`.
- **AutoFilter** §18.3.1.2 + §18.3.2 (`filters`/`filter` .8/.6, `customFilters` .3/.2, `top10` .10, `dateGroupItem` .4,
  `dynamicFilter` .5 với §18.18.26, `colorFilter` .1, `filterColumn` .7) — đọc/ghi `crates/bo-xlsx/src/autofilter.rs`,
  đánh giá `crates/bo-sheet/src/filter.rs`. **Sort** `sortState`/`sortCondition` §18.3.1.92/.91, §18.18.79 —
  `crates/bo-sheet/src/sort.rs`.
- **Table** §18.5.1.2 / `tableColumns` §18.5.1.4; **comments** §18.7 (authors §18.7.2, comment §18.7.3) — `read.rs`, `write.rs`.
- **PivotTable** §18.10: cache definition §18.10.1.67, records §18.10.1.68, table definition §18.10.1.73, `sharedItems`
  §18.10.1.90, nhóm `fieldGroup`/`discretePr`/`rangePr` §18.10.1.27/.26/.77, trường tính toán `cacheField/@formula`
  §18.10.1.3, `location` §18.10.1.49, style §18.8.40; phần `x14` theo [MS-XLSX] — `crates/bo-xlsx/src/pivot.rs`,
  `pivot_write.rs`, `crates/bo-sheet/src/pivot/{mod,compute,group,calc,style}.rs`.
- **Chart**: drawing §20.5 → `c:chartSpace` §21.2 (bar, line, area, pie, doughnut, scatter, combo; `numCache`
  §21.2.2.120) — đọc `crates/bo-xlsx/src/chart.rs`, ghi `chart_write.rs`, vẽ `crates/bo-sheet/src/chart.rs`;
  nút trường pivot chart `c14:pivotOptions` ([MS-ODRAWXML]).
- **CSV** RFC 4180 — `crates/bo-xlsx/src/csv.rs`.

## 2. Điểm mơ hồ & cách đã chọn

- Ô thiếu thuộc tính `r`: lấy vị trí kế tiếp sau ô/hàng trước (`read.rs`, biến `next_col`/`next_row`).
- `t="s"` với chỉ số ngoài bảng sst → chuỗi rỗng; số không parse được hoặc không hữu hạn → `#VALUE!` khi đọc;
  khi ghi, số không hữu hạn thành ô lỗi `#NUM!` (`read.rs`, `write.rs`).
- Số ghi ra dạng `xsd:double` round-trip chính xác: số nguyên < 1e15 ghi không phần thập phân, còn lại dùng biểu diễn
  ngắn nhất của Rust (`xml.rs` `fmt_f64`).
- Ô `t="d"` (ISO 8601) giữ nguyên chuỗi, không chuyển sang serial (`CellValue::Date`); OpenXmlValidator chưa nhận dạng
  này nên được ghi ngoại lệ trong `tests/fidelity/known-issues.txt`.
- `calcChain` (§18.6) bị bỏ khi ghi vì là cache tuỳ chọn, sẽ cũ sau khi sửa ô (`read.rs`).
- Lossless: phần tử/part không mô hình hoá (theme, drawing, VML, `extLst`, `sheetProtection`, …) giữ nguyên byte và ghi lại
  đúng vị trí schema; tiêu chí autofilter không biểu diễn chính xác được giữ XML thô (`lib.rs`, `autofilter.rs`).
- `row/@s` chỉ được áp khi `customFormat` bật (`read.rs`).
- dxf solid fill: màu nằm ở `bgColor`, `fgColor` chỉ là dự phòng (`cf.rs`, `filter.rs`, viện dẫn §18.8.14).
- CF: rule áp theo `priority` tăng dần, thuộc tính đã đặt bởi rule trước không bị rule sau ghi đè, `stopIfTrue` dừng;
  ngưỡng percentile nội suy kiểu PERCENTILE.INC (`cf.rs`).
- Data validation: `showDropDown=1` nghĩa là **ẩn** mũi tên; list nội tuyến là chuỗi literal tách bằng dấu phẩy; khi nhiều
  rule phủ một ô, rule sau thắng; ngưỡng tính tương đối theo ô trên-trái (`validation.rs`).
- AutoFilter: danh sách giá trị so khớp **chuỗi hiển thị**, không phân biệt hoa thường; custom filter so số khi hai vế
  là số, ngược lại so chuỗi có wildcard `*`/`?` (`~` escape); tuần trong lọc ngày tương đối bắt đầu Chủ nhật (`filter.rs`).
- Sort: số < chữ < logic < lỗi, ô trống luôn cuối cả hai chiều, ổn định; khoá chữ bỏ dấu (NFD) và `đ`→`d` cho tiếng
  Việt; chữ thường trước chữ hoa khi bằng nhau (`sort.rs`).
- Định dạng số: parse dễ dãi (ký tự lạ thành literal); `[ColorN]` đánh số từ 1 và ánh xạ vào bảng màu chỉ số từ mục 8;
  tag locale trong mã chỉ đổi tên tháng/thứ và AM/PM, dấu phân cách lấy từ locale người dùng (§18.8.31); tag lạ
  (`[DBNum1]`…) bị bỏ qua (`numfmt/mod.rs`, `numfmt/locale.rs`).
- Hệ ngày 1900: serial 60 là 29/02/1900 không có thật (§18.17.4.1), serial 0 hiển thị "1900-01-00", serial âm/quá
  9999-12-31 hiển thị `#####` (`numfmt/date.rs`).
- Ngày gõ vào: thứ tự ngày/tháng theo locale (vi-VN `dd/mm/yyyy`, en-US `m/d/yyyy`); năm 2 chữ số 00–29 → 20xx,
  30–99 → 19xx; `1/2` để nguyên là chữ (`numfmt/input.rs`).
- CSV: mặc định `,`; vi-VN dùng `;`, thập phân `,` và ghi BOM UTF-8; số có số 0 đầu ("007") giữ là chữ; ghi giá trị
  cache, không ghi công thức (`csv.rs`).
- Pivot: thiết lập chỉ có ở schema 2010 (`fillDownLabels`, "show values as" mới) ghi bằng `x14`; đặt `refreshOnLoad`
  để ứng dụng khác dựng lại layout từ cache (`pivot_write.rs`).
- Chart/pivot đọc lỗi thì bỏ qua (không tóm tắt), part giữ nguyên byte (`chart.rs`, `pivot.rs`).

## 3. Quan sát black-box

Chưa có quan sát black-box nào được ghi trong code/test của `bo-xlsx`, `bo-sheet`, `bo-model/src/sheet.rs`
(grep "black-box"/"observed" không có kết quả). Các lựa chọn ở mục 2 dựa trên đọc spec và quy ước bảng tính phổ biến.

## 4. Chưa hỗ trợ

- Pivot: chưa có biểu đồ pivot, mục tính toán `calculatedItems`, UI nhóm thủ công, lọc theo ngày
  (`docs/PROGRESS.md` "Hạn chế đã biết" mục 4).
- Ngày gõ dạng chữ (tên tháng) chưa được nhận là ngày; chart neo tuyệt đối chưa kéo được (như trên; `docs/REQUIREMENTS.md` P3.34).
- Bảo vệ sheet/workbook: `sheetProtection` giữ opaque, không khoá ô, không UI; không mở/lưu `.xlsx` có mật khẩu
  (`docs/REQUIREMENTS.md` P5.6, mục 6 danh sách thiếu).
- Tiêu chí autofilter dạng XML thô không được đánh giá (hàng vẫn hiện) (`filter.rs`).
- Cộng tác: định dạng và chèn/xoá hàng cột BlueSheet chưa đồng bộ trực tiếp (`docs/PROGRESS.md` mục 1).
- Mở `.xlsx` 1 triệu ô trên WASM > 5 s (`docs/REQUIREMENTS.md` P3.38).
- OpenXmlValidator còn 3 lỗi đã biết cho xlsx (`docs/REQUIREMENTS.md` mục 39, `tests/fidelity/known-issues.txt`).
