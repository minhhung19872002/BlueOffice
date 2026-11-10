# BlueSheet — Mô hình dữ liệu (data model)

Hạng mục SHEET-DM-001. Mã: `crates/bo-sheet/src/datamodel/` (định nghĩa, lưu trữ, ngôn ngữ biểu thức `expr.rs`,
đánh giá `eval.rs`, bảng tổng hợp trên mô hình `pivot.rs`), `crates/bo-engine/src/sheet/model_cmd.rs` (lệnh `model*`
của cầu JSON), `packages/sheet-editor/src/dataModel.tsx` (UI, tab Dữ liệu ▸ "Mô hình dữ liệu").

Nguồn: ECMA-376 Part 1 §15.2.4 / §22.5 (custom XML), §18.10 (pivot cache, pivot table), §18.5 (bảng). Mô hình dữ liệu
nhị phân của phần mềm khác ([MS-XLDM], phần `xl/model/item.data`) **không** được đọc hay ghi: giữ nguyên byte và chỉ
liệt kê.

## Mô hình

* **Bảng** — bảng trang tính (SHEET-TABLE-001) theo tên; bảng do truy vấn Get & Transform nạp vào (SHEET-PQ-001) là
  bảng trang tính nên dùng được như nhau (hộp thoại ghi tên truy vấn). Dòng tiêu đề và dòng tổng bị bỏ; giá trị là giá
  trị đã tính của ô.
* **Quan hệ** — một-nhiều trên một cột khoá: `fromTable[fromColumn]` (phía nhiều) → `toTable[toColumn]` (phía một,
  giá trị không trùng; ô trống bỏ qua). `modelDetect` dò lực lượng từ tính duy nhất của hai cột (`manyToOne`,
  `oneToMany` — đổi hai phía khi lưu, `oneToOne`, `manyToMany` — từ chối). Mỗi cặp bảng có tối đa một quan hệ
  **đang dùng** (`active`); đặt một quan hệ đang dùng sẽ tắt các quan hệ khác giữa hai bảng đó.
* **Cột tính toán** — biểu thức tính cho từng dòng của bảng (ngữ cảnh dòng), theo thứ tự khai báo (cột sau dùng được
  cột trước). Chỉ có trong mô hình, không ghi vào ô.
* **Số đo** — biểu thức tính trong ngữ cảnh lọc của từng ô báo cáo; có bảng "nhà" (nơi danh sách trường hiển thị) và
  định dạng số tuỳ chọn.

## Biểu thức

Ngôn ngữ riêng của BlueOffice (không phải ngôn ngữ của phần mềm thương mại nào; chỉ mượn ý tưởng chung của các ngôn
ngữ phân tích). Văn phạm (dấu `=` đầu tuỳ chọn; `,` hoặc `;` ngăn đối số):

```
expr    := or
or      := and ("||" and)*
and     := cmp ("&&" cmp)*
cmp     := concat (("=" | "<>" | "<" | "<=" | ">" | ">=") concat)*
concat  := add ("&" add)*
add     := mul (("+" | "-") mul)*
mul     := unary (("*" | "/") unary)*
unary   := ("-" | "+") unary | primary
primary := số | "chuỗi" | TRUE | FALSE | "(" expr ")" | HÀM "(" đối số ")"
         | Bảng[Cột] | 'Tên bảng có dấu cách'[Cột] | [Số đo] | Bảng
```

`]]` trong ngoặc vuông là `]`, `''` trong tên bảng là `'`, `""` trong chuỗi là `"`. Lỗi cú pháp trả về vị trí ký tự
(`modelCheck`: `{ ok, error: { message, start, end } }`); lỗi tên (bảng, cột, hàm, số đo, số đối số, số đo tự tham
chiếu trực tiếp hay gián tiếp) trả về `start = end = 0`.

Giá trị: trống (blank), số, chuỗi, logic, lỗi. Phép tính: trống + trống = trống, trống + x = x, trống × x = trống,
x / 0 = `#DIV/0!`; so sánh chuỗi không phân biệt hoa thường; trống = 0 = "".

| Hàm | Ý nghĩa |
|---|---|
| `SUM`, `AVERAGE`, `MIN`, `MAX`, `COUNT` (`Bảng[Cột]`) | trên các số của cột ở các dòng thấy được; không có số → trống |
| `COUNTA`, `DISTINCTCOUNT` (`Bảng[Cột]`) | ô không trống / số giá trị khác nhau (trống tính là một giá trị) |
| `COUNTROWS(bảng)` | số dòng thấy được của một biểu thức bảng (`Bảng`, `FILTER(…)`, `ALL(Bảng)`) |
| `SUMX`, `AVERAGEX`, `MINX`, `MAXX` (`bảng`, `biểu thức`) | lặp các dòng (ngữ cảnh dòng) |
| `DIVIDE(a, b[, thay thế])` | `b` trống hoặc 0 → `thay thế` (mặc định trống) |
| `CALCULATE(biểu thức, bộ lọc…)` | đổi ngữ cảnh lọc rồi tính (xem dưới) |
| `FILTER(bảng, điều kiện)` | các dòng của bảng thoả điều kiện (ngữ cảnh dòng) |
| `ALL(Bảng[Cột])`, `ALL(Bảng)` | trong `CALCULATE`: bỏ lọc cột / bỏ lọc bảng và các bảng nó tới được; `ALL(Bảng)` cũng là biểu thức bảng |
| `USERELATIONSHIP(Bảng1[Cột], Bảng2[Cột])` | trong `CALCULATE`: dùng quan hệ (kể cả đang tắt) thay cho quan hệ khác giữa hai bảng |
| `RELATED(Bảng[Cột])` | từ dòng hiện tại theo các quan hệ nhiều→một; không khớp → trống |
| `IF`, `AND`, `OR`, `NOT`, `ISBLANK`, `BLANK()`, `ABS`, `ROUND` | như tên |

**Ngữ cảnh lọc**: bộ lọc trên cột (tập giá trị giữ hoặc bỏ) và trên dòng của bảng. Một dòng của bảng T thấy được khi
qua bộ lọc của chính T và, theo từng quan hệ nhiều→một đang dùng (bắc cầu), qua bộ lọc của bảng phía một. Lọc chỉ đi
từ phía một sang phía nhiều (lọc bảng danh mục lọc bảng giao dịch, không ngược lại). Dòng không khớp phía một liên
kết với "dòng trống" của bảng đó: qua được bộ lọc giá trị có chứa giá trị trống (mục "(trống)" của báo cáo).

**Ngữ cảnh dòng**: dòng hiện tại của cột tính toán, `FILTER`, `SUMX`…: `Bảng[Cột]` đọc dòng đó, `[Cột]` đọc cột của
bảng hiện tại (cột tính toán), `RELATED` theo quan hệ. Hàm tổng hợp luôn dùng ngữ cảnh lọc; tham chiếu `[Số đo]` hoặc
`CALCULATE` trong ngữ cảnh dòng biến các dòng hiện tại thành bộ lọc (chuyển ngữ cảnh).

**`CALCULATE`**: trước hết áp `ALL` / `USERELATIONSHIP`, rồi các bộ lọc (tính trong ngữ cảnh ngoài): `FILTER(bảng, …)`
thay mọi bộ lọc của bảng đó bằng tập dòng; một điều kiện chỉ dùng một cột (`Bảng[Cột] = "x"`,
`Bảng[Giá] > 10 && Bảng[Giá] < 100`) thay bộ lọc của cột đó bằng các giá trị thoả; điều kiện trên nhiều cột của một
bảng thành bộ lọc dòng; điều kiện trên nhiều bảng → `#VALUE!`. Số đo tự tham chiếu (vòng) → `#REF!`.

## Bảng tổng hợp trên mô hình

`modelPivotCreate` tạo bảng tổng hợp có pivot cache riêng; các trường là mọi cột của mọi bảng (`Bảng[Cột]`, cột tính
toán gồm cả) rồi mọi số đo (`[Số đo]`); danh sách trường gom theo bảng (số đo nằm dưới bảng nhà). Mỗi lần dựng báo cáo,
cache được dựng lại từ mô hình:

1. **Bảng gốc**: bảng tới được (qua quan hệ đang dùng) nhiều bảng mà báo cáo dùng nhất (cột trên hàng/cột/bộ lọc/giá
   trị, mục ẩn, bảng mà số đo đọc); hoà thì bảng được dùng, rồi bảng nhiều dòng hơn.
2. **Bản ghi** = các dòng của bảng gốc nối với dòng liên quan của từng bảng tới được (bảng không tới được để trống).
   Trường hàng/cột/bộ lọc và giá trị thường (Sum of `Bảng[Cột]`…) là của engine pivot sẵn có (tổng phụ, bố cục,
   sắp xếp, kiểu — giữ hiệu năng SHEET-PERF-002 cho bảng tổng hợp một bảng: đường cũ không đổi).
3. **Số đo**: trường giá trị trên số đo lấy giá trị số đo tính trong ngữ cảnh lọc của từng ô (mục hàng + mục cột +
   bộ lọc báo cáo + mục ẩn của mọi trường), kể cả tổng phụ và tổng chung.

**Slicer**: slicer của bảng (SHEET-SLICER) trên một bảng của mô hình lọc cả các bảng tổng hợp trên mô hình (giá trị có
văn bản hiển thị được chọn), và mục ẩn của một trường bất kỳ (không chỉ trường trên trục) lọc báo cáo.

Không có trên mô hình: nhóm mục, trường/mục tính toán, đổi nguồn (báo lỗi); lọc giá trị/top-N trên số đo; mục danh
mục không có dòng giao dịch không hiện (báo cáo theo bản ghi của bảng gốc).

## Lưu trong tệp

1. **Định nghĩa** — phần custom XML `/customXml/itemN.xml` (quan hệ `customXml` từ workbook, `itemPropsN.xml` với
   `ds:itemID="{6E1B3C2A-9D84-4F5B-A7C1-3B0E5D9F2A43}"`, schemaRef = namespace dưới):

   ```xml
   <dataModel xmlns="http://schemas.bluestar.vn/blueoffice/2026/datamodel" version="1">
     <table name="Sản_phẩm"/>
     <relationship fromTable="Bán_hàng" fromColumn="Mã SP" toTable="Sản_phẩm" toColumn="Mã" active="1"/>
     <column table="Bán_hàng" name="Nhóm">RELATED(Sản_phẩm[Nhóm])</column>
     <measure name="Tổng SL" table="Bán_hàng" format="#,##0">SUM(Bán_hàng[SL])</measure>
     <pivot cacheId="3"/>
   </dataModel>
   ```

   `pivot/@cacheId` đánh dấu pivot cache dựng từ mô hình. Mô hình trống thì phần bị xoá. Hoàn tác/làm lại đi theo phần
   này như mọi phần khác.
2. **Pivot cache thường** (§18.10.1.67–68) với bản ghi đã nối, `worksheetSource name` = bảng gốc, **không**
   `refreshOnLoad` — phần mềm khác hiện giá trị của báo cáo (ô của báo cáo là ô thường). Trường số đo có bản ghi trống.
3. **Mô hình dữ liệu của phần mềm khác** — `xl/model/item.data`, mục custom XML của mô hình, kết nối
   `ThisWorkbookDataModel` (`xl/connections.xml`): giữ nguyên byte; `modelInfo.foreign` liệt kê (chỉ đọc).

## Lệnh

`modelInfo`, `modelAddTable`, `modelRemoveTable` (bỏ cả quan hệ, cột tính toán, số đo của bảng), `modelDetect`,
`modelSetRelationship` (`index` để sửa), `modelDeleteRelationship`, `modelCheck`, `modelSetMeasure` (`original` để
sửa/đổi tên), `modelDeleteMeasure`, `modelSetColumn`, `modelDeleteColumn`, `modelEvaluate` (giá trị không lọc),
`modelPivotCreate`. Lệnh sửa: một bước hoàn tác, dựng lại các bảng tổng hợp trên mô hình, trả về `modelInfo`.
Kiểu TS: `packages/platform/src/sheetModel.ts`.

Kiểm thử: `cargo test -p bo-sheet --lib datamodel` (cú pháp, đánh giá bảng-điều-khiển với kết quả tính độc lập bằng
phép nối vòng lặp lồng nhau, lan truyền quan hệ, báo cáo nhiều bảng so với phép nối ngây thơ, khứ hồi định nghĩa),
`cargo test -p bo-engine --test sheet_datamodel` (`BO_WRITE_CORPUS=1` sinh `tests/corpus/sheet/datamodel-{1,2,3}.xlsx`),
`packages/sheet-editor/test/dataModel.test.tsx`, E2E `tests/e2e/specs/sheet-datamodel.spec.ts`.
