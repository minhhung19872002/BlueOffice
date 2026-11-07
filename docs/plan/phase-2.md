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
| Layout | ✅ phân trang, keepNext/keepLines/widow, bảng (ngắt hàng qua trang, lặp hàng tiêu đề, gộp ô, viền, nền), ảnh inline + anchored (top-bottom; square/tight: chữ chảy bên trái/phải/hai bên/bên rộng hơn theo `wrapText`, distT/B/L/R, tight theo hộp bao `wrapPolygon`), header/footer first/even/default + kế thừa, cột, footnote cuối trang, endnote cuối văn bản, PAGE/NUMPAGES/SECTIONPAGES |
| Render/PNG/PDF | ✅ lệnh `image` (PNG/JPEG, cắt, xoay), viền nét đứt/đôi, tô sáng, lớp sau/trước chữ |
| `bo-edit` ([ADR-0008](../adr/0008-editing-transactions-and-command-bridge.md)) | ✅ transaction + op khả nghịch, undo/redo không giới hạn (proptest), gõ/xoá/Enter, định dạng ký tự/đoạn, style (tự tạo style dựng sẵn), danh sách, chèn bảng/ảnh/ngắt trang/ngắt phần/ký tự đặc biệt, find/replace (hoa thường, cả từ, regex), clipboard nội bộ/HTML/text |
| Layout tăng dần (§5.4) | ✅ `bo_layout::IncrementalLayout`: transaction báo khối đã đổi (`bo_model::change::BlockChanges`), chỉ dàn lại khối đổi, phân trang lại từ checkpoint trước khối đổi và dừng khi trạng thái phân trang khớp layout cũ (dùng lại các trang sau, dời/đánh số lại khi cần); display list dựng khi được yêu cầu; engine trả đúng các trang đổi; proptest ≥ 64 ca: tăng dần == dàn lại từ đầu |
| Editor UI | ✅ caret, selection (chuột, kéo, Shift+mũi tên, Ctrl+A, double/triple click), IME tiếng Việt (pre-edit tại caret), clipboard qua `Platform`, ribbon định dạng, hộp thoại tìm/thay, tạo mới/lưu/lưu thành .docx, xuất PDF, tự lưu + khôi phục qua `Platform.offline` |

## Hạn chế / chuyển phase sau
- Chưa có thước kẻ, kéo tab stop, Format Painter, Paste Special, hộp thoại thiết lập trang (lề/hướng/khổ), chọn phông/cỡ bằng danh sách
  (hiện có tăng/giảm cỡ chữ), bảng màu đầy đủ (màu chữ đỏ/tô vàng mặc định).
- Chưa sửa nội dung header/footer/footnote; chỉnh sửa trong bảng: gõ/xoá trong ô được, chưa thêm/xoá hàng-cột.
- Wrap tight dùng hộp bao của đa giác (chưa theo từng cạnh đa giác); chữ trong bảng không chảy quanh ảnh; đoạn bị chia sang
  trang sau dùng bề rộng tính theo vị trí ở trang trước; text box/shape hiển thị dạng opaque; chưa chia đoạn qua cột cân bằng.
- Layout tăng dần: tài liệu có NUMPAGES trong thân văn bản, thay đổi đánh số chú thích, style/numbering/section dàn lại toàn bộ;
  trang dùng lại chỉ được dời số trang khi thân văn bản không có field và không có section oddPage/evenPage.
- Visual diff với ảnh tham chiếu (< 2 % pixel) chưa chạy vì chưa có ảnh tham chiếu do người dùng cung cấp.
- E2E desktop (Tauri WebDriver) chưa có.

## Benchmark gõ phím (§5: < 16 ms ở tài liệu 300 trang)
Tài liệu 308 trang (4 300 đoạn căn đều, Times New Roman 13), sửa đoạn giữa rồi lấy display list của trang chứa caret.
Native: `cargo bench -p bo-engine --bench doc_typing` (hoặc `cargo test --release -p bo-engine --test typing_perf -- --ignored --nocapture`);
WASM (node): `pnpm build:wasm && node scripts/bench-typing-wasm.mjs`. Trung vị, máy dev Windows:

| Thao tác | Native trước | Native sau | WASM trước | WASM sau |
|---|---|---|---|---|
| Gõ 1 ký tự | 45,8 ms | ~1,1 ms | 60,3 ms | 2,5 ms |
| Enter | 66,9 ms | ~1,3 ms | 64,6 ms | 2,4 ms |
| Backspace | 78,7 ms | ~1,1 ms | 74,6 ms | 2,4 ms |
| Bật/tắt đậm | 54,2 ms | ~1,9 ms | 77,6 ms | 4,1 ms |
| Gõ 1 ký tự, không khớp lại (phân trang lại ~150 trang) | — | ~8 ms | — | — |
