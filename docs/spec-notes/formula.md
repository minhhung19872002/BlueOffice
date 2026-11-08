# Công thức (formula) — ghi chú đọc spec

Ghi chú clean-room: chỉ diễn giải và trích số mục, không chép văn bản spec. Nguồn: ECMA-376 Part 1 §18.17
(Formulas: §18.17.2 cú pháp, §18.17.3 tính toán, §18.17.4 ngày giờ, §18.17.7 hàm định nghĩa sẵn), §18.2.2 `calcPr`,
§18.2.28 `date1904`, §18.5.1 table; [MS-XLSX] (tiền tố `_xlfn.`, dynamic array); OpenFormula (ODF 1.3 Part 4)
§6.3 chỉ dùng làm nguồn test. Crate: `crates/bo-formula`.

## 1. Phạm vi đã cài

- **Lexer / parser** (§18.17.2): `src/lexer.rs`, `src/parser.rs` (Pratt parser, độ ưu tiên §18.17.2.2: toán tử tham
  chiếu `:` / khoảng trắng / `,` chặt nhất, rồi dấu âm, `%`, `^`, `* /`, `+ -`, `&`, so sánh; mọi toán tử hai ngôi
  kết hợp trái), AST `src/ast.rs`, formatter `src/format.rs` (`parse(format(e)) == e`).
- **Tham chiếu** (§18.17.2.3) — `src/reference.rs`: A1 và R1C1 (`R[-2]C`), `$` tuyệt đối/hỗn hợp, vùng, cả cột `A:C`,
  cả hàng `1:3`, sheet khác có nháy (`'Sheet name'!`), 3D `Sheet1:Sheet3!`, union/intersection, spill `A1#`,
  `@` implicit intersection (`src/eval.rs`), structured reference `Table[col]`, `[#This Row]` (§18.5.1, §18.5.1.2 —
  `src/eval.rs`, `src/workbook.rs`), array constant `{…}` (`parser.rs`). Lưới tối đa tới cột `XFD`.
- **Giá trị và lỗi** — `src/value.rs`: số, chữ, logic, lỗi, mảng, tham chiếu, lambda. 10 lỗi: `#NULL!`, `#DIV/0!`,
  `#VALUE!`, `#REF!`, `#NAME?`, `#NUM!`, `#N/A`, `#GETTING_DATA`, `#SPILL!`, `#CALC!` (mã `ERROR.TYPE` 1–9 và 14).
- **Ép kiểu** (§18.17.3) — `value.rs` `as_number`/`as_text`/`as_bool`, `compare_values`.
- **Ngày giờ** (§18.17.4, §18.17.4.1) — `src/date.rs`: hệ 1900 và 1904 (`DateSystem`, chênh 1462 ngày), `DATE`
  (§18.17.7) cộng 1900 cho năm 0–1899.
- **Tiêu chí** `COUNTIF`/`SUMIF(S)`/`AVERAGEIF(S)`/`MAXIFS`… (§18.17.7) — `src/criteria.rs`.
- **Dịch chuyển tham chiếu** (§18.17.2.3) — `src/shift.rs`: chèn/xoá hàng cột, copy/paste tương đối, cut/paste di chuyển khối.
- **Hàm** (§18.17.7) — `src/functions/{math,logical,lookup,text,datetime,stats,financial,info}.rs`, đăng ký ở
  `functions/mod.rs`: 276 hàm (đếm các lời gọi đăng ký `f`/`s`/`l`/`m`: 120 + 140 + 8 + 8), gồm LET, LAMBDA.
- **Volatile**: `NOW`, `TODAY`, `RAND`, `RANDBETWEEN`, `RANDARRAY`, `OFFSET`, `INDIRECT` (`.volatile()` trong
  `functions/*.rs`).
- **Workbook / tính lại** — `src/workbook.rs`: đồ thị phụ thuộc, tính tăng dần, thứ tự theo SCC (Tarjan), phát hiện
  vòng, tính lặp theo `calcPr` `iterate`/`iterateCount`/`iterateDelta` (§18.2.2), spill mảng động.
- **TEXT()** dùng bộ định dạng số rút gọn (§18.8.31) — `src/numfmt.rs`.
- Test: `crates/bo-formula/tests/engine.rs` (`comparisons`, `error_values`, `reference_forms`, `r1c1_parse_and_format`,
  `implicit_intersection`, `structured_references`, `spill_and_spill_errors`, `circular_references`,
  `iterative_calculation`, `volatile_functions_recalculate`), `tests/fn_*.rs`; mỗi dòng test phải ghi nguồn bắt đầu
  bằng `ECMA-376 §`, `OpenFormula `, `MS-XLSX _xlfn.` hoặc `hand: ` (`tests/common/mod.rs` `SOURCE_PREFIXES`).

## 2. Điểm mơ hồ & cách đã chọn

- **29/02/1900**: hệ 1900 giữ ngày không có thật làm serial 60; serial ≥ 61 lệch một ngày; serial 0 là "1900-01-00";
  serial 1 được coi là Chủ nhật cho nhất quán với ngày giả (`date.rs`, test `serials_1900`). Hệ 1904: serial 0 là
  1904-01-01. Serial lớn nhất 2 958 465 (9999-12-31).
- **15 chữ số có nghĩa**: so sánh số bằng `num_eq` — làm tròn 15 chữ số có nghĩa (`round_sig15`), có lối tắt khi chênh
  ≥ 1e-14 lần độ lớn; nên `=0.1+0.2=0.3` là TRUE (`value.rs`, test `comparisons`). `ROUND` cũng “chụp” về 15 chữ số trước
  khi làm tròn nửa xa số 0, để 2.675 làm tròn như viết (`functions/math.rs` `round_digits`). Đối số lượng giác > 2^27 bị từ
  chối vì mất độ chính xác (`math.rs`). Ép số nguyên khi |x| > 9e15 → `#NUM!` (`eval.rs`).
- **Chuyển số thành chữ** (General): tối đa 15 chữ số có nghĩa, ký hiệu khoa học ngoài khoảng số mũ −10..14
  (`value.rs` `number_to_text`).
- **Ép kiểu**: ô trống → 0 / "" / FALSE tuỳ vế còn lại; chữ dạng số (có `%`, dấu nghìn, số âm trong ngoặc `(5)`, chuỗi
  ngày) được parse, chữ khác → `#VALUE!`; chữ ↔ logic chỉ nhận `TRUE`/`FALSE`; so sánh chữ không phân biệt hoa thường;
  khác kiểu thì xếp số < chữ < logic (`value.rs` `type_rank`). Ép số từ chữ ngày dùng hệ 1900.
- **Độ ưu tiên dấu âm**: `-2^2` = `(-2)^2` (dấu âm chặt hơn `^`) (`parser.rs` test `precedence`).
- **Giới hạn parser**: 8192 ký tự, lồng tối đa 100, độ sâu cây 1024 (`parser.rs`).
- **Tên hàm**: viết hoa, bỏ tiền tố lưu trữ `_xlfn.`, `_xlws.`, `_xludf.` (`lexer.rs`); hàm/tên lạ → `#NAME?` (`eval.rs`).
- **Tham chiếu vòng**: không bật lặp thì mọi ô trong vòng là `#CALC!` (`workbook.rs`, test `circular_references`).
- **Tham chiếu động** (`INDIRECT`, `OFFSET`) đọc ô chưa tính thì hoãn ô đọc tới khi ô kia tính xong (`workbook.rs`).
- **Spill** bị chặn bởi ô không rỗng → `#SPILL!` (`workbook.rs`).
- **Dịch tham chiếu**: chèn bên trong vùng làm vùng nở; xoá trọn vùng → `#REF!`, xoá một phần làm vùng co; copy đẩy ra
  ngoài lưới → `#REF!`; cut/paste: tham chiếu (tuyệt đối lẫn tương đối) trỏ vào khối di chuyển đi theo khối
  (`shift.rs`). Table bị xoá hết hàng/cột thì bị gỡ (`workbook.rs`).
- **Tiêu chí**: `"="` đơn lẻ chỉ khớp ô thật sự trống; `*` `?` wildcard, `~` escape, không phân biệt hoa thường (`criteria.rs`).
- **Năm 2 chữ số** khi parse chữ ngày: 00–29 → 20xx, 30–99 → 19xx (`date.rs`).

## 3. Quan sát black-box

Không có quan sát black-box nào được ghi trong code/test của `bo-formula` (grep "black-box"/"observed" không có kết
quả). Kỳ vọng test lấy từ ví dụ trong spec, OpenFormula hoặc tự tính (`hand:`), không từ chạy phần mềm khác.

## 4. Chưa hỗ trợ

- Test hàm: một file mỗi **nhóm** thay vì mỗi hàm, nguồn chưa ghi theo từng dòng ở mọi nơi, chưa có bộ kỳ vọng do người
  kiểm thử cung cấp; `TEXT()` dùng bộ định dạng số rút gọn (`docs/REQUIREMENTS.md` P3.25, mục 37).
- `numfmt.rs` (cho `TEXT`) bỏ qua `[điều kiện]`/`[màu]`, literal đặt giữa placeholder được in sau số.
- WASM không tính song song (`eval_parallel` tắt trên wasm32) (`docs/REQUIREMENTS.md` mục 2).
- Công thức trong `.xls`/`.xlsb` (BIFF `Rgce`, [MS-XLS] §2.5.198, [MS-XLSB] §2.5.97) được dịch sang chữ A1 ở
  `crates/bo-binary/src/xls_formula.rs`; ptg lạ báo lỗi `unsupported ptg`. Chi tiết xem `legacy-binary.md`.
