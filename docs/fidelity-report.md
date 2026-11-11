# Báo cáo độ trung thực (fidelity)

Sinh tự động bởi `scripts/fidelity-report.mjs` · 2026-11-11 · commit `f5f99228`.

Mỗi file trong `tests/corpus/{doc,sheet,slide}` được mở rồi lưu lại cùng định dạng bằng `bo-cli convert`; file vào và ra đều qua OpenXmlValidator (DocumentFormat.OpenXml, Office 2019). Mức lossless (XML chuẩn hoá giống hệt) được đo bằng test Rust `corpus` của từng crate.

| Nhóm | File | Mở + lưu được | OpenXmlValidator (vào) | OpenXmlValidator (ra) |
|---|---|---|---|---|
| doc (.docx) | 94 | 94 (100.0 %) | 93 | 93 (98.9 %) |
| sheet (.xlsx) | 173 | 173 (100.0 %) | 170 | 170 (98.3 %) |
| slide (.pptx) | 80 | 80 (100.0 %) | 79 | 79 (98.8 %) |

## Đã biết (không tính là lỗi, xem `tests/fidelity/known-issues.txt`)

- doc\unknown-01-sdt-customxml.docx: The element has invalid child element 'http://schemas.openxmlformats.org/wordprocessingml/2006/main:smartTag'. List of possible elements expected: <http://schemas.openxmlformats.org/wordprocessingml/2006/main:pPr>. — w:smartTag hợp lệ trong CT_P theo ECMA-376 Part 1 §17.5.1.9 (custom markup, Transitional); DocumentFormat.OpenXml chỉ chấp nhận ở Office 2007 (Word 2010+ bỏ smart tag) nên báo lỗi khi kiểm theo Office 2019. Corpus cố ý giữ để kiểm round-trip lossless.
- sheet\values-1.xlsx: Cell contents have invalid value '2024-01-15T08:30:00' for type 'CellValues { }'. — Ô t="d" (ngày ISO 8601) hợp lệ theo ECMA-376 Part 1 §18.18.11 ST_CellType; OpenXmlValidator chưa chấp nhận giá trị <v> dạng xsd:dateTime. Corpus cố ý giữ để kiểm round-trip.
- sheet\values-2.xlsx: Cell contents have invalid value '2024-02-15T08:30:00' for type 'CellValues { }'. — Ô t="d" (ngày ISO 8601) hợp lệ theo ECMA-376 Part 1 §18.18.11 ST_CellType; OpenXmlValidator chưa chấp nhận giá trị <v> dạng xsd:dateTime. Corpus cố ý giữ để kiểm round-trip.
- sheet\values-3.xlsx: Cell contents have invalid value '2024-03-15T08:30:00' for type 'CellValues { }'. — Ô t="d" (ngày ISO 8601) hợp lệ theo ECMA-376 Part 1 §18.18.11 ST_CellType; OpenXmlValidator chưa chấp nhận giá trị <v> dạng xsd:dateTime. Corpus cố ý giữ để kiểm round-trip.
- slide\slide-equation.pptx: The element 'http://schemas.microsoft.com/office/drawing/2010/main:m' is a leaf element and cannot contain children. — a14:m (Office Math trong đoạn văn của hình, [MS-ODRAWXML]) chứa m:oMathPara / m:oMath (OMML, ECMA-376 Part 1 §22.1) như ứng dụng tham chiếu vẫn ghi; DocumentFormat.OpenXml mô hình a14:m là phần tử lá nên báo lỗi cả ở nhánh mc:Choice. Corpus slide-equation.pptx cố ý giữ để kiểm phát hiện hình phương trình (CORE-UI-008).

## Theo nhóm tính năng

Nhóm = tiền tố tên file corpus (`tables-02-merge.docx` → `tables`, `cf-1.xlsx` → `cf`, `slide-anim-…` → `slide-anim`, `writer-pivot-…` → `pivots`). **Đạt** = mở + lưu được và file ra hợp lệ theo OpenXmlValidator; cột "Đã biết" đếm file ra chỉ vướng lỗi đã chấp nhận trong `known-issues.txt`. Xu hướng: % đạt của tối đa 4 lần chạy trước (từ `tests/fidelity/history.csv`) → lần này, kèm chênh lệch so với lần trước.

| Định dạng | Nhóm tính năng | File | Đạt | % đạt | Đã biết | Xu hướng % |
|---|---|---|---|---|---|---|
| doc | blocks | 3 | 3 | 100.0 % |  | 100 (mới) |
| doc | cite | 3 | 3 | 100.0 % |  | 100 (mới) |
| doc | comments | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| doc | controls | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 (=) |
| doc | fields | 6 | 6 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| doc | images | 6 | 6 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| doc | layout | 3 | 3 | 100.0 % |  | 100 → 100 → 100 (=) |
| doc | links | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| doc | lists | 4 | 4 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| doc | math | 7 | 7 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| doc | merge | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| doc | notes | 7 | 7 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| doc | page | 2 | 2 | 100.0 % |  | 100 → 100 → 100 (=) |
| doc | para | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| doc | review | 13 | 13 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| doc | sample | 1 | 1 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| doc | sections | 4 | 4 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| doc | smartart | 4 | 4 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| doc | styles | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| doc | tables | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| doc | text | 7 | 7 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| doc | unknown | 3 | 2 | 66.7 % | 1 | 67 → 67 → 67 → 67 → 67 (=) |
| sheet | cf | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| sheet | charts | 9 | 9 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| sheet | comments | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| sheet | datamodel | 3 | 3 | 100.0 % |  | 100 (mới) |
| sheet | ext | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| sheet | layout | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| sheet | links | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| sheet | maps | 3 | 3 | 100.0 % |  | 100 (mới) |
| sheet | names | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| sheet | page | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| sheet | panes | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| sheet | pivots | 24 | 24 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| sheet | query | 3 | 3 | 100.0 % |  | 100 (mới) |
| sheet | smartart | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| sheet | sparklines | 3 | 3 | 100.0 % |  | 100 → 100 → 100 (=) |
| sheet | sst | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| sheet | styles | 6 | 6 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| sheet | tables | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| sheet | threaded | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| sheet | timeline | 1 | 1 | 100.0 % |  | 100 → 100 → 100 (=) |
| sheet | validation | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| sheet | values | 6 | 3 | 50.0 % | 3 | 50 → 50 → 50 → 50 → 50 (=) |
| sheet | writer-cell-media | 1 | 1 | 100.0 % |  | 100 → 100 (=) |
| sheet | writer-cf-custom | 3 | 3 | 100.0 % |  | 100 → 100 (=) |
| sheet | writer-cf-databars | 3 | 3 | 100.0 % |  | 100 → 100 (=) |
| sheet | writer-cf-icons | 3 | 3 | 100.0 % |  | 100 → 100 (=) |
| sheet | writer-cf-pivot | 3 | 3 | 100.0 % |  | 100 → 100 (=) |
| sheet | writer-chartex-box | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 (=) |
| sheet | writer-chartex-funnel | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 (=) |
| sheet | writer-chartex-histogram | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 (=) |
| sheet | writer-chartex-map | 3 | 3 | 100.0 % |  | 100 (mới) |
| sheet | writer-chartex-pareto | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 (=) |
| sheet | writer-chartex-sunburst | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 (=) |
| sheet | writer-chartex-treemap | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 (=) |
| sheet | writer-chartex-waterfall | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 (=) |
| sheet | writer-charts-3dbar | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 (=) |
| sheet | writer-charts-3dline | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 (=) |
| sheet | writer-charts-3dpie | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 (=) |
| sheet | writer-charts-bubble | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 (=) |
| sheet | writer-charts-components | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 (=) |
| sheet | writer-charts-ofpie | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 (=) |
| sheet | writer-charts-percent | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 (=) |
| sheet | writer-charts-radar | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 (=) |
| sheet | writer-charts-stock | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 (=) |
| sheet | writer-charts-surface | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 (=) |
| sheet | writer-form-controls | 3 | 3 | 100.0 % |  | 100 → 100 → 100 (=) |
| sheet | writer-slicers | 3 | 3 | 100.0 % |  | 100 → 100 → 100 (=) |
| sheet | writer-sparklines | 3 | 3 | 100.0 % |  | 100 → 100 → 100 (=) |
| slide | slide-action | 3 | 3 | 100.0 % |  | 100 (mới) |
| slide | slide-album | 3 | 3 | 100.0 % |  | 100 (mới) |
| slide | slide-anim | 4 | 4 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| slide | slide-chartex | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 (=) |
| slide | slide-charts | 4 | 4 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| slide | slide-edit | 23 | 23 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| slide | slide-equation | 1 | 0 | 0.0 % | 1 | 0 (mới) |
| slide | slide-images | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| slide | slide-media | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| slide | slide-model3d | 3 | 3 | 100.0 % |  | 100 (mới) |
| slide | slide-notes | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| slide | slide-rec | 3 | 3 | 100.0 % |  | 100 (mới) |
| slide | slide-shapes | 5 | 5 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| slide | slide-smartart | 7 | 7 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| slide | slide-tables | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| slide | slide-text | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| slide | slide-theme | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| slide | slide-zoom | 3 | 3 | 100.0 % |  | 100 (mới) |

### Xu hướng theo định dạng

| Định dạng | % đạt (các lần chạy trước → lần này) |
|---|---|
| doc | 99 → 99 → 99 → 99 → 99 (+0.1) |
| sheet | 96 → 98 → 98 → 98 → 98 (+0.1) |
| slide | 100 → 100 → 100 → 100 → 99 (-1.3) |
