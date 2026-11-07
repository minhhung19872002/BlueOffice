# Phase 3 — Engine công thức `bo-formula`

Trạng thái: **hoàn thành phần engine công thức — chờ tích hợp** · Prompt gốc:
[`BlueOffice-PROMPT.md`](../../BlueOffice-PROMPT.md) §6 Phase 3 "Engine công thức", "Hàm đợt 1".
Cài đặt clean-room từ ECMA-376 Part 1 §18.17 (công thức), §18.5 (bảng), §18.2.2 calcPr,
§18.8.31 numFmt và OpenFormula (ODF 1.3 Part 4). Không có dependency runtime mới
(chỉ `proptest`, `criterion` ở dev, đã có trong `THIRD_PARTY_NOTICES.md`).

## 1. Đã cài đặt

**Cú pháp** (`parse`, `parse_with`, `format`, `format_with`; round-trip ổn định):
A1 và R1C1 (tương đối theo ô gốc), tham chiếu tương đối/tuyệt đối/hỗn hợp, range, cột/hàng nguyên
(`A:C`, `1:3`), union `(A1,B2)`, intersection (dấu cách), sheet khác `'Sheet 1'!A1`, 3D
`Sheet1:Sheet3!A1`, defined name (workbook & sheet scope), structured reference
(`T[Col]`, `T[[#This Row],[Col]]`, `[@Col]`, `#All/#Data/#Headers/#Totals`, `[A]:[B]`, tên bảng trần),
array constant, `@` (implicit intersection), toán tử spill `A1#`, `LAMBDA(..)(..)`, tiền tố `_xlfn.`.
Giới hạn chống input độc hại: ≤ 8192 ký tự, lồng ≤ 100 cấp, cây ≤ 1024 cấp (không tràn stack).

**Giá trị & ngữ nghĩa**: number (f64), text, boolean, mọi lỗi (`#NULL! #DIV/0! #VALUE! #REF! #NAME?
#NUM! #N/A #SPILL! #CALC! #GETTING_DATA`), array, reference, lambda. Ép kiểu ngầm định (chuỗi số/ngày,
boolean, ô trống; trong range text/bool bị bỏ qua), so sánh số 15 chữ số có nghĩa, text không phân biệt
hoa thường, thứ tự number < text < boolean. Broadcasting mảng, LET, LAMBDA (closure), MAP/REDUCE/SCAN/
BYROW/BYCOL/MAKEARRAY. Ngày serial hệ 1900 (có 29/02/1900 giả = 60) và 1904.

**Workbook / dependency graph** (`Workbook`): precedents tĩnh (kể cả qua name và structured ref),
ô nhỏ index từng ô, range lớn index theo lưới bucket (`rangeindex.rs`); tính lại tăng dần (dirty
propagation) + thứ tự topo bằng Tarjan SCC; tham chiếu động (INDIRECT/OFFSET) tự hoãn tới khi
precedent được tính; phát hiện vòng lặp (`#CALC!` + `circular_references()`) hoặc tính lặp
(`IterativeConfig`: iterateCount/iterateDelta); hàm volatile (NOW, TODAY, RAND, RANDBETWEEN,
RANDARRAY, OFFSET, INDIRECT); dynamic array spill, `#SPILL!` khi bị chặn/tràn lưới, `A1#`;
hàng ẩn cho SUBTOTAL 1xx/AGGREGATE.

**Chỉnh sửa cấu trúc**: `adjust_for_change` / `Workbook::apply_structural_change` (chèn/xoá hàng/cột:
dịch, mở rộng/thu hẹp range, tham chiếu bị xoá → `#REF!`; cập nhật cả defined name và bảng);
`offset_relative` / `Workbook::copy_cell` (copy-paste điều chỉnh tham chiếu tương đối).

**Hàm**: 276 hàm built-in (`function_names()`), đủ toàn bộ danh sách "Hàm đợt 1" và thêm các biến thể
(`.S/.P/.INC/.EXC`, CEILING/FLOOR.PRECISE, ISO.CEILING, DAYS360, SYD, EFFECT, NOMINAL, CUMIPMT,
CUMPRINC, HSTACK/VSTACK/TAKE/DROP/CHOOSECOLS/CHOOSEROWS, ISFORMULA, FORMULATEXT, SHEET(S), …).
TEXT dùng bộ định dạng số riêng (`format_number`: 4 section, điều kiện/màu bỏ qua, ngày giờ, %, khoa học,
phân số `# ?/?`, `@`).

## 2. API chính

```rust
use bo_formula::{Workbook, Value, StructuralChange, CellId};
let mut wb = Workbook::with_sheets(&["Sheet1"]);
wb.set_a1(0, "A1", "2")?;               // nhập như người dùng: số / TRUE / #N/A / text / =công thức
wb.set_formula(0, 1, 0, "=A1*21")?;      // (sheet, row, col) 0-based
wb.recalculate();                         // chỉ tính các ô bị ảnh hưởng + volatile
assert_eq!(wb.value_a1(0, "A2"), Value::Number(42.0));
wb.apply_structural_change(0, StructuralChange::InsertRows { at: 0, count: 1 });
wb.copy_cell(CellId::new(0, 2, 0), CellId::new(0, 3, 0));
```

Khác: `define_name`, `add_table(TableInfo)`, `set_date_system`, `set_now`, `seed_random`,
`set_iterative`, `set_row_hidden`, `spill_range`, `formula`, `evaluate` (công thức tạm),
`circular_references`, `last_recalc_evaluations`. Đánh giá trên nguồn dữ liệu khác: trait
`CellProvider` + `evaluate(expr, provider, sheet, row, col, &EvalConfig)`.

## 3. Kiểm thử

- `tests/fn_*.rs`: bảng (công thức, input, kết quả kỳ vọng) cho từng nhóm hàm — math, logical, info,
  text, datetime, stats, financial, lookup; mỗi bảng ghi nguồn (ECMA-376 §18.17.7 / OpenFormula /
  định nghĩa hàm công bố) và ghi chú chỗ mơ hồ. Mọi hàm đã đăng ký đều có ít nhất một dòng test.
- `tests/engine.rs`: cú pháp, ép kiểu, so sánh 15 chữ số, tham chiếu, spill, name, bảng, tính lại
  tăng dần (đếm số lần evaluate), vòng lặp, tính lặp, volatile, chèn/xoá hàng cột, copy-paste,
  input độc hại.
- `tests/proptest_roundtrip.rs`: parse → format → parse (A1 và R1C1), biến đổi tham chiếu, text bất kỳ
  không panic.
- Benchmark `benches/recalc.rs` (xem §4).

## 4. Benchmark (`cargo bench -p bo-formula --bench recalc`)

Máy dev Windows (release, criterion, trung vị). Workbook 10 000 hàng × 100 cột: 900 000 hằng số +
100 000 công thức (số học, SUM/AVERAGE/MAX 10 ô, IF, COUNTIF, VLOOKUP chính xác vào bảng 1 000 hàng,
running total nối chuỗi 10 000 ô).

| Kịch bản | Thời gian | Mục tiêu |
|---|---|---|
| Nạp 1M ô + tính 100k công thức | **3.20 s** | < 5 s ✔ |
| Sửa 1 ô → tính lại phụ thuộc (≈5 000 ô, gồm chuỗi running total) | **22.8 ms** | < 100 ms ✔ |
| Sửa 1 ô trong bảng VLOOKUP (10 000 VLOOKUP phụ thuộc) | 331 ms | trường hợp xấu |
| Sửa A1 (10 000 VLOOKUP + chuỗi 10 000 ô) | 150 ms | trường hợp xấu |

Tối ưu đã làm: so sánh 15 chữ số có fast-path (không format chuỗi), index range lớn theo bucket,
bỏ propagate khi sửa hàng loạt, cache vùng lớn đã vật chất hoá trong một lượt tính (VLOOKUP lặp lại).

## 5. Còn lại / hạn chế đã biết

- Chưa tính song song các nhánh độc lập (graph đã có SCC; rayon/Web Worker để sau).
- Locale: chuỗi ngày dạng `m/d/yyyy` (en-US) và tên tháng tiếng Anh; TEXT chưa áp dụng `[$-42A]`,
  dấu thập phân theo locale.
- Vòng lặp không bật iterative trả `#CALC!` (bảng tính phổ biến hiển thị 0 kèm cảnh báo).
- Tham chiếu ngoài workbook (`[Book1]Sheet1!A1`) chưa được giải quyết (chỉ ADDRESS sinh chuỗi).
- CELL chỉ hỗ trợ `address/row/col/contents/type/sheet`.
- Chưa có cầu nối tới `bo-xlsx` (đọc công thức/giá trị cache từ file) — làm khi tích hợp BlueSheet.
