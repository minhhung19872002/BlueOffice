# Báo cáo độ trung thực (fidelity)

Sinh tự động bởi `scripts/fidelity-report.mjs` · 2026-11-07 · commit `59c0b31`.

Mỗi file trong `tests/corpus/{doc,sheet,slide}` được mở rồi lưu lại cùng định dạng bằng `bo-cli convert`; file vào và ra đều qua OpenXmlValidator (DocumentFormat.OpenXml, Office 2019). Mức lossless (XML chuẩn hoá giống hệt) được đo bằng test Rust `corpus` của từng crate.

| Nhóm | File | Mở + lưu được | OpenXmlValidator (vào) | OpenXmlValidator (ra) |
|---|---|---|---|---|
| doc (.docx) | 34 | 34 (100.0 %) | 33 | 33 (97.1 %) |
| sheet (.xlsx) | 42 | 42 (100.0 %) | 39 | 39 (92.9 %) |
| slide (.pptx) | 28 | 28 (100.0 %) | 28 | 28 (100.0 %) |

## Đã biết (không tính là lỗi, xem `tests/fidelity/known-issues.txt`)

- doc\unknown-01-sdt-customxml.docx: The element has invalid child element 'http://schemas.openxmlformats.org/wordprocessingml/2006/main:smartTag'. List of possible elements expected: <http://schemas.openxmlformats.org/wordprocessingml/2006/main:pPr>. — w:smartTag hợp lệ trong CT_P theo ECMA-376 Part 1 §17.5.1.9 (custom markup, Transitional); DocumentFormat.OpenXml chỉ chấp nhận ở Office 2007 (Word 2010+ bỏ smart tag) nên báo lỗi khi kiểm theo Office 2019. Corpus cố ý giữ để kiểm round-trip lossless.
- sheet\values-1.xlsx: Cell contents have invalid value '2024-01-15T08:30:00' for type 'CellValues { }'. — Ô t="d" (ngày ISO 8601) hợp lệ theo ECMA-376 Part 1 §18.18.11 ST_CellType; OpenXmlValidator chưa chấp nhận giá trị <v> dạng xsd:dateTime. Corpus cố ý giữ để kiểm round-trip.
- sheet\values-2.xlsx: Cell contents have invalid value '2024-02-15T08:30:00' for type 'CellValues { }'. — Ô t="d" (ngày ISO 8601) hợp lệ theo ECMA-376 Part 1 §18.18.11 ST_CellType; OpenXmlValidator chưa chấp nhận giá trị <v> dạng xsd:dateTime. Corpus cố ý giữ để kiểm round-trip.
- sheet\values-3.xlsx: Cell contents have invalid value '2024-03-15T08:30:00' for type 'CellValues { }'. — Ô t="d" (ngày ISO 8601) hợp lệ theo ECMA-376 Part 1 §18.18.11 ST_CellType; OpenXmlValidator chưa chấp nhận giá trị <v> dạng xsd:dateTime. Corpus cố ý giữ để kiểm round-trip.
