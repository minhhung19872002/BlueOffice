# Báo cáo độ trung thực (fidelity)

Sinh tự động bởi `scripts/fidelity-report.mjs` · 2026-11-07 · commit `40c0d24`.

Mỗi file trong `tests/corpus/{doc,sheet,slide}` được mở rồi lưu lại cùng định dạng bằng `bo-cli convert`; file vào và ra đều qua OpenXmlValidator (DocumentFormat.OpenXml, Office 2019). Mức lossless (XML chuẩn hoá giống hệt) được đo bằng test Rust `corpus` của từng crate.

| Nhóm | File | Mở + lưu được | OpenXmlValidator (vào) | OpenXmlValidator (ra) |
|---|---|---|---|---|
| doc (.docx) | 1 | 1 (100.0 %) | 1 | 1 (100.0 %) |
| sheet (.xlsx) | 42 | 42 (100.0 %) | 39 | 39 (92.9 %) |
| slide (.pptx) | 25 | 25 (100.0 %) | 25 | 25 (100.0 %) |

## Đã biết (không tính là lỗi, xem `tests/fidelity/known-issues.txt`)

- sheet\values-1.xlsx: Cell contents have invalid value '2024-01-15T08:30:00' for type 'CellValues { }'. — Ô t="d" (ngày ISO 8601) hợp lệ theo ECMA-376 Part 1 §18.18.11 ST_CellType; OpenXmlValidator chưa chấp nhận giá trị <v> dạng xsd:dateTime. Corpus cố ý giữ để kiểm round-trip.
- sheet\values-2.xlsx: Cell contents have invalid value '2024-02-15T08:30:00' for type 'CellValues { }'. — Ô t="d" (ngày ISO 8601) hợp lệ theo ECMA-376 Part 1 §18.18.11 ST_CellType; OpenXmlValidator chưa chấp nhận giá trị <v> dạng xsd:dateTime. Corpus cố ý giữ để kiểm round-trip.
- sheet\values-3.xlsx: Cell contents have invalid value '2024-03-15T08:30:00' for type 'CellValues { }'. — Ô t="d" (ngày ISO 8601) hợp lệ theo ECMA-376 Part 1 §18.18.11 ST_CellType; OpenXmlValidator chưa chấp nhận giá trị <v> dạng xsd:dateTime. Corpus cố ý giữ để kiểm round-trip.
