# Spec notes

Ghi chú đọc spec theo từng vùng định dạng (ECMA-376 / ISO 29500, Microsoft Open Specifications, ODF 1.3…), phục vụ
clean-room (BlueOffice-PROMPT §1, yêu cầu 4.6). Quy tắc: **không chép văn bản spec** — chỉ diễn giải và dẫn số mục;
mỗi ghi chú có 4 phần: *Phạm vi đã cài* (mục spec → crate/module), *Điểm mơ hồ & cách đã chọn*, *Quan sát black-box*
(chỉ những gì code/test ghi lại), *Chưa hỗ trợ*. Khi cài thêm phần tử, cập nhật ghi chú tương ứng cùng commit.

| Ghi chú | Vùng | Crate chính |
|---|---|---|
| [opc.md](opc.md) | Đóng gói OPC — ECMA-376 Part 2 (content types, relationships, ZIP, giới hạn) | `bo-opc` |
| [docx-wordprocessingml.md](docx-wordprocessingml.md) | WordprocessingML — ECMA-376 Part 1 §17 (pPr/rPr, style, numbering, bảng, section, field, track changes, ghi chú) | `bo-docx`, `bo-model`, `bo-edit` |
| [sml-spreadsheetml.md](sml-spreadsheetml.md) | SpreadsheetML — §18 (ô, shared strings, styles/numFmt, CF/DV, pivot, chart, CSV) | `bo-xlsx`, `bo-sheet` |
| [formula.md](formula.md) | Công thức — §18.17 (ngữ pháp, giá trị lỗi, hệ ngày 1900 gồm 29/02/1900, so sánh 15 chữ số) | `bo-formula` |
| [pml-presentationml.md](pml-presentationml.md) | PresentationML — §19 (master/layout/placeholder kế thừa, theme, hiệu ứng) | `bo-pptx` |
| [drawingml.md](drawingml.md) | DrawingML — §20/§21 (preset geometry, fill/line, màu, chart) | `bo-drawingml` |
| [legacy-binary.md](legacy-binary.md) | Nhị phân cũ — [MS-CFB], [MS-DOC], [MS-XLS], [MS-XLSB], [MS-PPT], [MS-OVBA] | `bo-binary` |
| [crypto-sign.md](crypto-sign.md) | Mã hoá [MS-OFFCRYPTO], chữ ký PAdES / OOXML (XML-DSig) | `bo-crypto`, `bo-sign` |
| [odf.md](odf.md) | ODF 1.3 (.odt/.ods/.odp, manifest) | `bo-odf` |
| [encoding-tcvn3-vni.md](encoding-tcvn3-vni.md) | Bảng mã tiếng Việt cũ TCVN3 (ABC), VNI, cp1252 | `bo-encoding` |

Liên quan: [ADR-0005](../adr/0005-lossless-docx-round-trip.md) (round-trip lossless),
[ADR-0007](../adr/0007-digital-signatures.md) (ký số), `tests/fidelity/known-issues.txt` (ngoại lệ OpenXmlValidator),
[../deploy/security.md](../deploy/security.md) (xử lý tệp không tin cậy).
