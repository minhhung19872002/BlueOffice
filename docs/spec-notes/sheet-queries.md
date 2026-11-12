# BlueSheet — Lấy & biến đổi dữ liệu (Get & Transform data)

Hạng mục SHEET-PQ-001. Mã: `crates/bo-query` (nguồn, ngôn ngữ bước, đánh giá, lưu trữ),
`crates/bo-engine/src/sheet/query_cmd.rs` (lệnh `query*` của cầu JSON), `packages/sheet-editor/src/queries.tsx` (UI).

## Truy vấn

Một truy vấn = **nguồn** + **danh sách bước** theo thứ tự, viết bằng ngôn ngữ bước riêng của BlueOffice (JSON,
không phải M). Kiểu Rust: `bo_query::Query`; kiểu TS: `QueryDef` (`packages/platform/src/sheetQuery.ts`).

```json
{ "name": "Doanh số",
  "source": { "kind": "csv", "file": "sales.csv", "delimiter": ";", "encoding": "utf-8" },
  "steps": [ { "op": "promoteHeaders" },
             { "op": "changeType", "columns": [ { "column": "Số lượng", "type": "integer" } ] },
             { "op": "filterRows", "column": "Khu vực", "condition": "notEquals", "value": "Trung" } ],
  "load": { "table": "Doanh_số" } }
```

Nguồn (`kind`): `csv` (mã hoá: BOM → UTF-8/UTF-16; UTF-8 hợp lệ; UTF-16 không BOM; còn lại Windows-1252 rồi dò
TCVN3/VNI; dấu phân cách `,` `;` tab `|` dò trên 20 dòng đầu; tách trường bằng bộ đọc RFC 4180 của `bo-xlsx`),
`json` (mảng bản ghi theo `path`), `xml` (phần tử lặp `element`), `workbook` (trang tính hoặc bảng của tệp
.xlsx/.xlsm/.xls/.xlsb/.ods/.csv — đọc bằng các bộ đọc sẵn có), `folder` (mọi tệp một loại, nối lại, thêm cột
`Source.Name`), `web` (bảng HTML thứ `table` của trang), `query` (kết quả truy vấn khác).

Bước (`op`): `promoteHeaders`, `demoteHeaders`, `changeType`, `filterRows`, `removeColumns`, `keepColumns`,
`renameColumns`, `reorderColumns`, `splitColumn`, `replaceValues`, `groupBy` (sum/average/min/max/count/countDistinct/
median), `sort`, `merge` (leftOuter/rightOuter/fullOuter/inner/leftAnti/rightAnti), `append`, `unpivot`, `addColumn`
(công thức BlueSheet: `[@Cột]` là giá trị của dòng, `[Cột]` cả cột, bảng ảo tên `Query`), `removeDuplicates`,
`keepTop`, `removeTop`, `removeBlankRows`.

Engine không tự đọc tệp hay mạng: host đọc qua `Platform` (`files`, `dataSources.pickFolder/readFolder/fetchWeb`) rồi
gửi byte bằng `querySource`. Web: desktop gọi thẳng; bản web chỉ qua proxy máy chủ khi quản trị bật
(`docs/deploy/web-data.md`).

## Lưu trong tệp

1. **Định nghĩa** — phần custom XML (ECMA-376 Part 1 §15.2.4/§22.5) `/customXml/itemN.xml`, quan hệ `customXml` từ
   workbook, kèm `itemPropsN.xml` (`ds:itemID="{6E1B3C2A-9D84-4F5B-A7C1-3B0E5D9F2A42}"`, schemaRef = namespace dưới):

   ```xml
   <queries xmlns="http://schemas.bluestar.vn/blueoffice/2026/queries" version="1">
     <query name="Doanh số">{JSON của truy vấn, escape XML}</query>
   </queries>
   ```

   Phần mềm khác giữ phần này nguyên vẹn và không dùng tới. Hoàn tác/làm lại đi theo phần này như mọi phần khác.
2. **Kết nối chuẩn**, khi nguồn có dạng chuẩn (CSV → `connection type="6"` + `textPr` §18.13.12; web →
   `type="4"` + `webPr`/`tables` §18.13.15), ghi lúc lưu `.xlsx`: `xl/connections.xml` (§18.13), một phần
   `queryTable` (§18.12.3, `queryTableRefresh/queryTableFields` trỏ `tableColumnId`) có quan hệ từ phần bảng, bảng có
   `tableType="queryTable"`, mỗi cột `queryTableFieldId`. Bảng tải lên vẫn là bảng thường ở phần mềm khác.
   JSON, XML, workbook, thư mục, truy vấn: ECMA-376 không có dạng kết nối tương ứng, nên ghi kết nối OLE DB
   (`type="5"`, §18.13.4 `dbPr connection="Provider=BlueOffice.Query;Location=<tên truy vấn>;Source=<kiểu>;Data
   Source=<tệp/thư mục/truy vấn>" command="<tên>" commandType="3"`) cùng query table như trên: phần mềm khác giữ
   kết nối, bảng và liên kết nhưng không làm mới được (không có trình cung cấp `BlueOffice.Query`); định nghĩa thật
   vẫn ở phần custom XML. Cột người dùng thêm vào bảng: `queryTableField dataBound="0"` (§18.12.4), `queryTable
   fillFormulas="1"` (công thức được điền xuống khi làm mới).
   Phần tử BlueOffice mang `extLst/ext uri="{6E1B3C2A-9D84-4F5B-A7C1-3B0E5D9F2A41}"` chứa
   `<bq:query xmlns:bq="…/queries" name="…"/>`; mỗi lần lưu chỉ các phần tử có dấu này được viết lại, kết nối và
   query table của phần mềm khác giữ nguyên byte.
3. **[MS-QDEFF] DataMashup** (`customXml` gốc `DataMashup`, thường UTF-16) của phần mềm khác: giữ nguyên byte, chỉ đọc;
   `queryMashup` giải base64 → `Version`, `PackagePartsLength`, gói OPC → `Formulas/Section1.m` để hiện mã M dạng văn bản.

## Tải và làm mới

Close & Load: bảng (SHEET-TABLE-001) trên trang mới hoặc tại ô chọn; tên bảng lấy từ tên truy vấn. Làm mới: chạy lại
và viết lại bảng tại chỗ (giữ ô góc, kiểu bảng, cài đặt cột theo tên, hàng tổng), một bước hoàn tác cho cả "làm mới
tất cả". Nguồn thiếu byte → `errors[].missing` để UI hỏi lại tệp.

Cột người dùng thêm vào bảng đã tải (như phần mềm bảng tính phổ biến): `load.columns` ghi các cột kết quả của lần tải
trước; cột khác của bảng là của người dùng. Khi làm mới, cột của truy vấn theo thứ tự mới, mỗi cột người dùng đứng sau
cột truy vấn nó đứng sau trước đó (không có thì đứng đầu); cột tính (`calculatedColumnFormula`) điền công thức xuống
mọi hàng mới, cột thường giữ giá trị theo số thứ tự hàng. Định nghĩa cũ không có `load.columns`: cột tính không có
trong kết quả được coi là của người dùng.

Đổi tên bảng (lệnh `tableUpdate`): truy vấn nạp vào bảng đó đổi `load.table` theo (cùng bước hoàn tác).

Hạn chế: không có trình soạn mã M; không chạy truy vấn M; phần mềm khác không làm mới được truy vấn JSON/XML/sổ
tính/thư mục (kết nối OLE DB trỏ trình cung cấp riêng).
