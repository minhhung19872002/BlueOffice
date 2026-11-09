# Báo cáo độ trung thực (fidelity)

Sinh tự động bởi `scripts/fidelity-report.mjs` · 2026-11-09 · commit `a65de51`.

Mỗi file trong `tests/corpus/{doc,sheet,slide}` được mở rồi lưu lại cùng định dạng bằng `bo-cli convert`; file vào và ra đều qua OpenXmlValidator (DocumentFormat.OpenXml, Office 2019). Mức lossless (XML chuẩn hoá giống hệt) được đo bằng test Rust `corpus` của từng crate.

| Nhóm | File | Mở + lưu được | OpenXmlValidator (vào) | OpenXmlValidator (ra) |
|---|---|---|---|---|
| doc (.docx) | 81 | 81 (100.0 %) | 80 | 80 (98.8 %) |
| sheet (.xlsx) | 148 | 148 (100.0 %) | 145 | 145 (98.0 %) |
| slide (.pptx) | 64 | 64 (100.0 %) | 64 | 64 (100.0 %) |

## Đã biết (không tính là lỗi, xem `tests/fidelity/known-issues.txt`)

- doc\unknown-01-sdt-customxml.docx: The element has invalid child element 'http://schemas.openxmlformats.org/wordprocessingml/2006/main:smartTag'. List of possible elements expected: <http://schemas.openxmlformats.org/wordprocessingml/2006/main:pPr>. — w:smartTag hợp lệ trong CT_P theo ECMA-376 Part 1 §17.5.1.9 (custom markup, Transitional); DocumentFormat.OpenXml chỉ chấp nhận ở Office 2007 (Word 2010+ bỏ smart tag) nên báo lỗi khi kiểm theo Office 2019. Corpus cố ý giữ để kiểm round-trip lossless.
- sheet\values-1.xlsx: Cell contents have invalid value '2024-01-15T08:30:00' for type 'CellValues { }'. — Ô t="d" (ngày ISO 8601) hợp lệ theo ECMA-376 Part 1 §18.18.11 ST_CellType; OpenXmlValidator chưa chấp nhận giá trị <v> dạng xsd:dateTime. Corpus cố ý giữ để kiểm round-trip.
- sheet\values-2.xlsx: Cell contents have invalid value '2024-02-15T08:30:00' for type 'CellValues { }'. — Ô t="d" (ngày ISO 8601) hợp lệ theo ECMA-376 Part 1 §18.18.11 ST_CellType; OpenXmlValidator chưa chấp nhận giá trị <v> dạng xsd:dateTime. Corpus cố ý giữ để kiểm round-trip.
- sheet\values-3.xlsx: Cell contents have invalid value '2024-03-15T08:30:00' for type 'CellValues { }'. — Ô t="d" (ngày ISO 8601) hợp lệ theo ECMA-376 Part 1 §18.18.11 ST_CellType; OpenXmlValidator chưa chấp nhận giá trị <v> dạng xsd:dateTime. Corpus cố ý giữ để kiểm round-trip.

## Theo nhóm tính năng

Nhóm = tiền tố tên file corpus (`tables-02-merge.docx` → `tables`, `cf-1.xlsx` → `cf`, `slide-anim-…` → `slide-anim`, `writer-pivot-…` → `pivots`). **Đạt** = mở + lưu được và file ra hợp lệ theo OpenXmlValidator; cột "Đã biết" đếm file ra chỉ vướng lỗi đã chấp nhận trong `known-issues.txt`. Xu hướng: % đạt của tối đa 4 lần chạy trước (từ `tests/fidelity/history.csv`) → lần này, kèm chênh lệch so với lần trước.

| Định dạng | Nhóm tính năng | File | Đạt | % đạt | Đã biết | Xu hướng % |
|---|---|---|---|---|---|---|
| doc | comments | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| doc | controls | 2 | 2 | 100.0 % |  | 100 → 100 (=) |
| doc | fields | 6 | 6 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| doc | images | 6 | 6 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| doc | layout | 3 | 3 | 100.0 % |  | 100 (mới) |
| doc | links | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| doc | lists | 4 | 4 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| doc | math | 5 | 5 | 100.0 % |  | 100 → 100 → 100 (=) |
| doc | merge | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| doc | notes | 7 | 7 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| doc | page | 2 | 2 | 100.0 % |  | 100 (mới) |
| doc | para | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| doc | review | 13 | 13 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| doc | sample | 1 | 1 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| doc | sections | 4 | 4 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| doc | smartart | 4 | 4 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| doc | styles | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| doc | tables | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| doc | text | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| doc | unknown | 3 | 2 | 66.7 % | 1 | 67 → 67 → 67 → 67 → 67 (=) |
| sheet | cf | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| sheet | charts | 9 | 9 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| sheet | comments | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| sheet | ext | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| sheet | layout | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| sheet | links | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| sheet | names | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| sheet | page | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| sheet | panes | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| sheet | pivots | 24 | 24 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| sheet | smartart | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| sheet | sparklines | 3 | 3 | 100.0 % |  | 100 (mới) |
| sheet | sst | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| sheet | styles | 6 | 6 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| sheet | tables | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| sheet | threaded | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| sheet | timeline | 1 | 1 | 100.0 % |  | 100 (mới) |
| sheet | validation | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| sheet | values | 6 | 3 | 50.0 % | 3 | 50 → 50 → 50 → 50 → 50 (=) |
| sheet | writer-chartex-box | 3 | 3 | 100.0 % |  | 100 → 100 (=) |
| sheet | writer-chartex-funnel | 3 | 3 | 100.0 % |  | 100 → 100 (=) |
| sheet | writer-chartex-histogram | 3 | 3 | 100.0 % |  | 100 → 100 (=) |
| sheet | writer-chartex-pareto | 3 | 3 | 100.0 % |  | 100 → 100 (=) |
| sheet | writer-chartex-sunburst | 3 | 3 | 100.0 % |  | 100 → 100 (=) |
| sheet | writer-chartex-treemap | 3 | 3 | 100.0 % |  | 100 → 100 (=) |
| sheet | writer-chartex-waterfall | 3 | 3 | 100.0 % |  | 100 → 100 (=) |
| sheet | writer-charts-3dbar | 3 | 3 | 100.0 % |  | 100 → 100 (=) |
| sheet | writer-charts-3dline | 3 | 3 | 100.0 % |  | 100 → 100 (=) |
| sheet | writer-charts-3dpie | 3 | 3 | 100.0 % |  | 100 → 100 (=) |
| sheet | writer-charts-bubble | 3 | 3 | 100.0 % |  | 100 → 100 (=) |
| sheet | writer-charts-components | 3 | 3 | 100.0 % |  | 100 → 100 (=) |
| sheet | writer-charts-ofpie | 3 | 3 | 100.0 % |  | 100 → 100 (=) |
| sheet | writer-charts-percent | 3 | 3 | 100.0 % |  | 100 → 100 (=) |
| sheet | writer-charts-radar | 3 | 3 | 100.0 % |  | 100 → 100 (=) |
| sheet | writer-charts-stock | 3 | 3 | 100.0 % |  | 100 → 100 (=) |
| sheet | writer-charts-surface | 3 | 3 | 100.0 % |  | 100 → 100 (=) |
| sheet | writer-form-controls | 3 | 3 | 100.0 % |  | 100 (mới) |
| sheet | writer-slicers | 3 | 3 | 100.0 % |  | 100 (mới) |
| sheet | writer-sparklines | 3 | 3 | 100.0 % |  | 100 (mới) |
| slide | slide-anim | 4 | 4 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| slide | slide-chartex | 3 | 3 | 100.0 % |  | 100 → 100 (=) |
| slide | slide-charts | 4 | 4 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| slide | slide-edit | 23 | 23 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| slide | slide-images | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| slide | slide-media | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| slide | slide-notes | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| slide | slide-shapes | 5 | 5 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| slide | slide-smartart | 7 | 7 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| slide | slide-tables | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| slide | slide-text | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |
| slide | slide-theme | 3 | 3 | 100.0 % |  | 100 → 100 → 100 → 100 → 100 (=) |

### Xu hướng theo định dạng

| Định dạng | % đạt (các lần chạy trước → lần này) |
|---|---|
| doc | 99 → 99 → 99 → 99 → 99 (+0.1) |
| sheet | 96 → 96 → 96 → 98 → 98 (+0.2) |
| slide | 100 → 100 → 100 → 100 → 100 (=) |
