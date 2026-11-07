# Phase 2 — BlueDoc MVP

Trạng thái: **hoàn thành phần cốt lõi, chờ duyệt** (2026-11-07) · Prompt gốc §6 Phase 2 · Kết quả: [PROGRESS.md](../PROGRESS.md).

## Chiến lược lossless round-trip ([ADR-0005](../adr/0005-lossless-docx-round-trip.md))
- Reader ghi lại **XML gốc** của từng block (`w:p`, `w:tbl`, `w:sdt`, …) và của `w:sectPr`, cùng một *fingerprint*
  (hash của phần mô hình hoá được). Writer xuất lại XML gốc nguyên văn nếu block không đổi; chỉ block bị sửa mới sinh lại.
- Khi sinh lại một đoạn: phần tử con không hiểu của `w:pPr`/`w:rPr` (vd `w14:*`, `w:rPrChange`) được giữ nguyên và ghi lại
  vào đúng node cha; thuộc tính lạ của `w:p` (`w14:paraId`…) được giữ.
- Block không hiểu (`w:sdt`, `w:customXml`, `mc:AlternateContent` cấp body) → `Block::Opaque` (giữ nguyên, không hiển thị nội dung
  trừ khi có fallback văn bản).
- Package: mọi part không thuộc tập sửa đổi được sao chép nguyên byte (bo-opc). Ảnh mới thêm → part `/word/media/*` + rel + content type.

## Phạm vi
| Nhóm | Nội dung |
|---|---|
| Model | styles (paragraph/character/table/numbering, basedOn), numbering (abstractNum, num, lvlOverride), section (khổ, hướng, lề, cột, header/footer first/even/default, titlePg, loại ngắt), bảng (grid, gridSpan, vMerge, độ rộng, border, shading, tblHeader, lồng nhau), ảnh inline/anchor, hyperlink, bookmark (opaque), footnote/endnote, field (PAGE, NUMPAGES, DATE, TOC — kết quả cache), thuộc tính run/đoạn đầy đủ theo §6 |
| Layout | style resolution, nhãn danh sách, tab stop tùy biến, keepNext/keepLines/widow, bảng (ngắt hàng qua trang, lặp hàng tiêu đề), ảnh inline + anchored (top-bottom/behind/in-front; square/tight ≈ top-bottom ở bản này), header/footer, cột, footnote cuối trang |
| Render/PDF/PNG | ảnh (Image command), border/shading bảng & đoạn, highlight, sup/sub, caps |
| Chỉnh sửa (`bo-edit`) | vị trí/selection, transaction + Op có nghịch đảo → undo/redo không giới hạn; gõ, xoá, Enter, định dạng ký tự/đoạn, style, chèn bảng/ảnh/ngắt trang/ngắt section/ký tự đặc biệt, find/replace (regex, hoa thường, cả từ) |
| Editor UI | caret, selection chuột/Shift+mũi tên/Ctrl+A/double/triple click, IME tiếng Việt, clipboard (nội bộ + HTML + text), format painter, clear formatting, style gallery, thước kẻ, autosave + khôi phục, lưu .docx, xuất PDF, in |

## Tiêu chí hoàn thành
95% corpus `tests/corpus/doc` round-trip không mất dữ liệu (XML chuẩn hoá giống hệt cho part không sửa); property test undo/redo;
E2E gõ tiếng Việt + lưu + mở lại.

## Đã làm
| Nhóm | Trạng thái |
|---|---|
| Model + `.docx` lossless | ✅ 34/34 file corpus lossless và ổn định mô hình (`cargo test -p bo-docx --test corpus`) |
| Layout | ✅ phân trang, keepNext/keepLines/widow, bảng (ngắt hàng qua trang, lặp hàng tiêu đề, gộp ô, viền, nền), ảnh inline + anchored (top-bottom; square/tight ≈ top-bottom), header/footer first/even/default + kế thừa, cột, footnote cuối trang, endnote cuối văn bản, PAGE/NUMPAGES/SECTIONPAGES |
| Render/PNG/PDF | ✅ lệnh `image` (PNG/JPEG, cắt, xoay), viền nét đứt/đôi, tô sáng, lớp sau/trước chữ |
| `bo-edit` ([ADR-0008](../adr/0008-editing-transactions-and-command-bridge.md)) | ✅ transaction + op khả nghịch, undo/redo không giới hạn (proptest), gõ/xoá/Enter, định dạng ký tự/đoạn, style (tự tạo style dựng sẵn), danh sách, chèn bảng/ảnh/ngắt trang/ngắt phần/ký tự đặc biệt, find/replace (hoa thường, cả từ, regex), clipboard nội bộ/HTML/text |
| Editor UI | ✅ caret, selection (chuột, kéo, Shift+mũi tên, Ctrl+A, double/triple click), IME tiếng Việt (pre-edit tại caret), clipboard qua `Platform`, ribbon định dạng, hộp thoại tìm/thay, tạo mới/lưu/lưu thành .docx, xuất PDF, tự lưu + khôi phục qua `Platform.offline` |

## Hạn chế / chuyển phase sau
- Chưa có thước kẻ, kéo tab stop, Format Painter, Paste Special, hộp thoại thiết lập trang (lề/hướng/khổ), chọn phông/cỡ bằng danh sách
  (hiện có tăng/giảm cỡ chữ), bảng màu đầy đủ (màu chữ đỏ/tô vàng mặc định).
- Chưa sửa nội dung header/footer/footnote; chỉnh sửa trong bảng: gõ/xoá trong ô được, chưa thêm/xoá hàng-cột.
- Wrap square/tight xấp xỉ top-and-bottom; text box/shape hiển thị dạng opaque; chưa chia đoạn qua cột cân bằng.
- Mỗi lệnh dàn lại cả tài liệu (cache theo đoạn) — đủ nhanh cho tài liệu vài chục trang; dirty-tracking theo transaction để đạt
  < 16 ms/300 trang còn phải làm. Visual diff với ảnh tham chiếu (< 2 % pixel) chưa chạy vì chưa có ảnh tham chiếu do người dùng cung cấp.
- E2E desktop (Tauri WebDriver) chưa có.