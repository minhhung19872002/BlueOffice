# ADR-0005: Chiến lược round-trip lossless cho `.docx`

- Trạng thái: Chấp nhận · Ngày: 2026-11-07

## Bối cảnh
Prompt §5.1 yêu cầu phần tử/thuộc tính XML chưa hiểu phải giữ nguyên và ghi lại đúng chỗ; tiêu chí Phase 2 là
≥ 95 % corpus round-trip không mất dữ liệu. Mô hình hoá toàn bộ WordprocessingML (ECMA-376 Part 1 §17) là không khả thi
trong một phase, và việc *sinh lại* XML từ mô hình luôn có rủi ro làm rơi markup lạ (`w14:*`, `mc:AlternateContent`, rsid…).

## Quyết định
1. Reader lưu **XML gốc** (`Raw { xml, fingerprint }`) của từng block (`w:p`, `w:tbl`), của `w:sectPr`, của style
   và của ảnh (`w:drawing`), kèm fingerprint (FNV-1a) của phần đã mô hình hoá tại thời điểm đọc.
2. Writer xuất XML gốc **nguyên văn** khi fingerprint hiện tại trùng; chỉ block bị sửa mới được sinh lại.
   Khi sinh lại, con không hiểu của `pPr`/`rPr`/`tblPr`/`tcPr`/`trPr` (`unknown: Vec<Raw>`) được ghi lại theo đúng thứ
   tự schema; markup cấp đoạn (bookmark, comment range, permission, `proofErr`) là `RunContent::Markup` và được giữ.
3. Block không hiểu (`w:sdt`, `w:customXml`, …) là `Block::Opaque`. Part không thuộc tập sửa đổi được bo-opc chép
   nguyên byte; ảnh mới thêm → part `/word/media/*` + relationship + content type.
4. Tài liệu mới (không có package nguồn) được sinh đầy đủ: styles, numbering, settings, header/footer, footnotes/endnotes.

## Kiểm chứng
`crates/bo-docx/tests/corpus.rs` trên `tests/corpus/doc` (34 file): (a) mở → lưu: mọi part giống hệt sau chuẩn hoá XML;
(b) bỏ hết XML gốc → sinh lại toàn bộ → đọc lại: mô hình không đổi; (c) sửa 1 đoạn: mọi part khác giữ nguyên.

## Hệ quả
- Bộ nhớ tăng (giữ thêm XML nguồn của mỗi block) — chấp nhận được; có thể chuyển sang lưu offset vào bytes nguồn sau.
- Sửa một đoạn chứa markup lạ ở *bên trong run* (vd `mc:AlternateContent`) vẫn giữ markup đó dưới dạng `RunContent::Opaque`.
- Footnote/endnote, header/footer của tài liệu có nguồn chưa sửa được trong Phase 2 nên part của chúng luôn được giữ nguyên.

## Phương án đã loại
- DOM XML đầy đủ (giữ cây XML làm mô hình): mỗi thao tác chỉnh sửa phải đồng bộ hai biểu diễn, phức tạp và chậm.
- Chỉ mô hình hoá rồi sinh lại toàn bộ: không đạt yêu cầu lossless với extension không biết trước.
