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
| Layout | ✅ phân trang, keepNext/keepLines/widow, bảng (ngắt hàng qua trang, lặp hàng tiêu đề, gộp ô, viền, nền), ảnh inline + anchored (top-bottom; square/tight: chữ chảy bên trái/phải/hai bên/bên rộng hơn theo `wrapText`, distT/B/L/R, tight ôm theo cạnh `wrapPolygon` từng dòng), header/footer first/even/default + kế thừa, cột, footnote cuối trang, endnote cuối văn bản, PAGE/NUMPAGES/SECTIONPAGES |
| Render/PNG/PDF | ✅ lệnh `image` (PNG/JPEG, cắt, xoay), viền nét đứt/đôi, tô sáng, lớp sau/trước chữ |
| `bo-edit` ([ADR-0008](../adr/0008-editing-transactions-and-command-bridge.md)) | ✅ transaction + op khả nghịch, undo/redo không giới hạn (proptest), gõ/xoá/Enter, định dạng ký tự/đoạn, style (tự tạo style dựng sẵn), danh sách, chèn bảng/ảnh/ngắt trang/ngắt phần/ký tự đặc biệt, find/replace (hoa thường, cả từ, regex), clipboard nội bộ/HTML/text |
| Layout tăng dần (§5.4) | ✅ `bo_layout::IncrementalLayout`: transaction báo khối đã đổi (`bo_model::change::BlockChanges`), chỉ dàn lại khối đổi, phân trang lại từ checkpoint trước khối đổi và dừng khi trạng thái phân trang khớp layout cũ (dùng lại các trang sau, dời/đánh số lại khi cần); display list dựng khi được yêu cầu; engine trả đúng các trang đổi; proptest ≥ 64 ca: tăng dần == dàn lại từ đầu |
| Editor UI | ✅ caret, selection (chuột, kéo, Shift+mũi tên, Ctrl+A, double/triple click), IME tiếng Việt (pre-edit tại caret), clipboard qua `Platform`, ribbon định dạng, hộp thoại tìm/thay, tạo mới/lưu/lưu thành .docx, xuất PDF, tự lưu + khôi phục qua `Platform.offline` |
| Phông / cỡ / màu | ✅ danh sách phông (phông đóng gói + tên phổ biến hiển thị kèm phông thay thế tương thích số đo: "Times New Roman → Tinos", "Arial → Liberation Sans", "Calibri → Carlito"), ô cỡ chữ (8–72 + gõ giá trị, nửa điểm §17.3.2.38), bảng màu chữ riêng của BlueOffice + mã hex, màu tô sáng theo §17.18.40, menu giãn dòng (§17.3.1.33) và khoảng cách trước/sau đoạn; mọi điều khiển phản ánh định dạng tại vùng chọn (`FormatState`) |
| Format Painter / Clear Formatting | ✅ bấm 1 lần = áp một lần, bấm đúp = giữ đến khi bấm Esc; Ctrl+Shift+C/V sao/dán định dạng; định dạng đoạn được sao khi nguồn là caret hoặc cả đoạn. Xoá định dạng = xoá định dạng ký tự + đoạn trong một bước undo |
| Paste Special | ✅ hộp thoại: văn bản có định dạng (HTML/nội bộ) giữ định dạng nguồn hoặc hợp nhất định dạng đích (giữ đậm/nghiêng/gạch/chỉ số), hoặc văn bản không định dạng; đọc HTML qua `Platform.clipboard.readHtml` |
| Thước kẻ | ✅ thước ngang (lề trái/phải, thụt dòng đầu/treo/trái/phải kéo được, tab stop trái/giữa/phải/thập phân: chọn loại ở góc, bấm để thêm, kéo để di chuyển, kéo ra ngoài để xoá — tab của style bị xoá bằng `w:val="clear"` §17.3.1.37) và thước dọc (lề trên/dưới); cm cho vi-VN, inch cho en-US; mọi thay đổi là lệnh engine (`formatParagraph` indents/tabs, `setPageSetup` §17.6.11) |
| Thiết lập trang | ✅ khổ A4/A5/Letter/Legal/tuỳ chỉnh, hướng giấy, lề có mẫu "Văn bản hành chính (NĐ 30/2020)" (trên/dưới 20 mm, trái 30 mm, phải 15 mm) + thông thường/hẹp/rộng, số cột + khoảng cách, khoảng cách đầu/chân trang; áp dụng cho phần hiện tại hoặc toàn văn bản (một bước undo) |
| Đầu/chân trang, chú thích | ✅ bấm đúp vùng lề trên/dưới để sửa đầu/chân trang của biến thể trang đó (đầu tiên/chẵn/mặc định §17.10.5) trong một phiên chỉnh sửa riêng (hiển thị trực tiếp, đóng = một bước undo), "trang đầu khác" (§17.10.6 titlePg), "trang chẵn lẻ khác" (§17.15.1.35), chèn PAGE/NUMPAGES; "liên kết với phần trước" theo từng phần và từng loại đầu/chân trang (§17.10.5 kế thừa headerReference: sửa đầu trang đang liên kết là sửa của phần nguồn; bỏ liên kết sao chép nội dung kế thừa; nút trên thanh đầu/chân trang, lệnh `hf.linkToPrevious`, `setHeaderLink`); chú thích cuối trang/cuối văn bản (§17.11) sửa **trực tiếp trên trang** (`bo_engine::notes`): dòng chú thích mang `LineSource::Note`, bấm vào vùng chú thích mở phiên sửa chú thích (caret, vùng chọn, định dạng, IME, undo như thân văn bản; đóng = một bước undo), chèn chú thích đưa caret vào chú thích, xoá dấu tham chiếu xoá chú thích, sao chép/dán mang chú thích theo; `footnotePr`/`endnotePr` của section (§17.11.11, §17.11.4: numRestart continuous/eachSect/eachPage, numStart, numFmt, pos pageBottom/beneathText/sectEnd) đọc/ghi/dàn trang, lệnh `setNoteOptions`; ngăn Chú thích chỉ còn là danh sách (bấm để tới chú thích) + tuỳ chọn đánh số/vị trí; ghi lại phần chú thích và `evenAndOddHeaders` của tệp đọc từ gói |
| Bảng | ✅ tab ngữ cảnh "Bảng": chèn hàng trên/dưới, cột trái/phải (giữ độ rộng bảng), xoá hàng/cột/bảng, gộp ô (gridSpan/vMerge), tách ô (bỏ gộp, hoặc chia đôi ô chưa gộp), tô nền ô (bảng màu + hex), viền tất cả/ngoài/trong/không, thuộc tính bảng (rộng tự động/%/cố định, căn lề), lặp hàng tiêu đề; Tab/Shift+Tab đi giữa các ô, Tab ở ô cuối thêm hàng |
| Mẫu văn bản | ✅ "Tệp → Mới từ mẫu": văn bản hành chính theo Nghị định 30/2020 (quốc hiệu, tiêu ngữ, cơ quan, số/ký hiệu, địa danh-ngày, tên loại + trích yếu, nội dung lùi đầu dòng 1 cm, nơi nhận, chức vụ-chữ ký; A4, lề NĐ 30, Times New Roman 13, số trang giữa lề trên từ trang 2), do BlueOffice dựng bằng mã (`bo_engine::templates`) |
| Kiểm thử | `cargo test -p bo-edit feature_tests`, `cargo test -p bo-engine --test doc_features` (lưu .docx rồi mở lại; liên kết đầu trang giữa các phần; chú thích sửa trên trang có định dạng, xoá theo dấu tham chiếu, tuỳ chọn đánh số, sao chép/dán), `cargo test -p bo-layout --test features footnote` (đánh số lại theo phần/trang, beneathText), Vitest `packages/editor/test/controls.test.tsx`, Playwright `tests/e2e/specs/doc-features.spec.ts` (phông + cỡ + màu, Format Painter, bảng + Tab thêm hàng, sửa đầu trang + số trang, gõ và định dạng chú thích trên trang, kéo thụt lề trên thước + tab stop, thiết lập trang + mẫu NĐ 30; lưu/mở lại giữ nguyên); corpus `notes-04-numbering-options.docx`, `sections-04-link-to-previous.docx` |

Ảnh: [mẫu NĐ 30 + thước kẻ](../screenshots/phase2-doc-template.png), [phông/cỡ/màu + Format Painter](../screenshots/phase2-doc-ruler-fonts.png).

## Hạn chế / chuyển phase sau
- Chú thích (bổ sung 2026-11-08): chú thích dài **chuyển tiếp sang trang sau** — dòng tham chiếu chỉ cần dòng đầu của
  chú thích nằm cùng trang (chú thích cuối cùng của trang được phép tách), phần còn lại mở đầu vùng chú thích trang sau
  với đường phân cách chuyển tiếp dài bằng bề rộng chữ (§17.11.1 continuationSeparator), có thể sang nhiều trang và sang
  trang riêng sau trang cuối; thông báo tiếp trang (`w:footnote w:type="continuationNotice"`, §17.18.33) hiện ở cuối phần
  bị tách; chú thích tiếp trang giữ số của trang đầu khi numRestart eachPage. `footnotePr`/`endnotePr` **cấp tài liệu**
  trong settings.xml (§17.11.11, §17.11.4) được đọc, ghi (tài liệu mới) và dùng khi dàn trang (định dạng số, số bắt đầu,
  đánh số lại, vị trí; footnotePr/endnotePr của section ghi đè từng thuộc tính). **numRestart eachPage không còn buộc dàn
  toàn bộ**: layout tăng dần dùng lại số chú thích theo trang của lần dàn trước, kiểm lại sau khi phân trang và chỉ dàn
  toàn bộ khi có chú thích đổi trang (test `per_page_footnote_numbers_stay_incremental`). Corpus `notes-05..07`.
  Còn lại: thông báo tiếp trang chỉ hiển thị (không ghi lại khi viết lại phần chú thích); endnote chưa chia theo dòng
  (đặt như đoạn thân văn bản). Nội dung chú thích/đầu-chân trang đồng bộ khi cộng tác (xem [phase-6.md](phase-6.md)).
  Liên kết đầu/chân trang: phần đầu tiên không có phần trước để liên kết.
- Thước kẻ đo theo lề trang (trong ô bảng chưa hiện ranh giới ô); thước dọc chỉ kéo lề trên/dưới (chưa kéo khoảng cách đầu/chân trang).
- Tách ô chưa có hộp thoại n hàng × m cột. Theo dõi thay đổi cấu trúc: xem [phase-5.md](phase-5.md) §5 (đã đủ hàng, cột,
  ô, gộp/tách, thuộc tính hàng, đầu/chân trang, chú thích, ảnh nổi). Bảng có thêm lệnh **chèn ô (đẩy phải)** / **xoá ô (đẩy
  trái)**. Bảng (từng ô), đầu/chân trang và chú thích đồng bộ trực tiếp khi cộng tác; giới hạn còn lại ở
  [phase-6.md](phase-6.md).
- Màu tô sáng tuỳ chọn được quy về màu `ST_HighlightColor` gần nhất khi lưu (§17.3.2.15 chỉ cho phép tên màu).
- Paste Special đọc HTML từ bộ nhớ tạm cần trình duyệt cho phép `clipboard.read`; nếu không, dùng nội dung vừa sao chép trong BlueDoc.
- Wrap tight theo cạnh đa giác trong dải mỗi dòng (đa giác lõm coi như bao theo chiều ngang); chữ trong bảng không chảy quanh ảnh; đoạn bị chia sang
  trang sau dùng bề rộng tính theo vị trí ở trang trước; text box/shape hiển thị dạng opaque; chưa chia đoạn qua cột cân bằng.
- Layout tăng dần: NUMPAGES/SECTIONPAGES trong thân văn bản và sửa đầu/chân trang **đã tăng dần** (xem mục "Layout tăng
  dần: NUMPAGES và đầu/chân trang" bên dưới). Còn dàn lại toàn bộ: thay đổi số trang khi thân có NUMPAGES, đầu/chân trang
  đổi chiều cao (đẩy thân văn bản), chú thích đổi trang với numRestart eachPage, đổi section, đổi style mặc định hoặc style/danh sách
  dùng trong đầu/chân trang/chú thích/comment/text box (đổi style hay numbering khác chỉ dàn lại từ khối đầu tiên dùng nó); trang dùng lại
  chỉ được dời số trang khi thân văn bản không có field và không có section oddPage/evenPage.
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

## Khả năng mở rộng layout (> 300 trang)
Đo bằng thời gian CPU của tiến trình (máy dev đang tải 100 % bởi các build khác, nên thời gian thực dao động ×2–3):
`bo-cli sample-big --pages N out.docx` rồi `bo-cli dump out.docx --page 0` (mở + layout + display list trang 1).

| Trang | 150 | 300 | 600 | 1 200 |
|---|---|---|---|---|
| CPU (s) | 0,61 | 1,14 | 2,08 | 4,13 |

`sample-big` tăng tuyến tính; con số "600 trang 2,49 s" cũ là nhiễu do tải máy. Phần O(n²) thật nằm ở tài liệu nhiều chú thích:
khoá cache layout đoạn văn băm **toàn bộ** bảng số chú thích cho mỗi đoạn có tham chiếu chú thích (O(đoạn × chú thích)).
Nay chỉ băm số của các chú thích mà đoạn tham chiếu. Đo bằng `cargo run --release -p bo-layout --example scale -- --repeat
tests/corpus/doc/notes-03-many-footnotes.docx` (thân văn bản lặp lại, mỗi bản sao có chú thích riêng):

| Bản sao (trang) | 150 (120) | 300 (240) | 600 (480) | 1 200 (960) |
|---|---|---|---|---|
| Trước | 337 ms (2,8 ms/trang) | 1 143 ms (4,8) | 4 742 ms (9,9) | 13 528 ms (14,1) |
| Sau | 203 ms (1,7 ms/trang) | 397 ms (1,7) | 851 ms (1,8) | 2 167 ms (2,3) |

Cùng đợt: nhãn danh sách tra `num`/`abstractNum` một lần thay vì ~10 lần quét tuyến tính, tập endnote dùng hash set.
`--repeat` cũng đo các corpus danh sách, bảng, đầu/chân trang, field, ảnh wrap: đều tuyến tính.

## Layout tăng dần: NUMPAGES và đầu/chân trang
- **NUMPAGES/SECTIONPAGES trong thân văn bản**: lần phân trang đầu của layout đầy đủ (tổng số trang rỗng, chỉ để đếm trang)
  được giữ như một *probe layout* tăng dần (không đầu/chân trang); layout chính được cập nhật tăng dần khi số trang probe
  không đổi. Gõ phím không làm đổi số trang → `UpdateKind::Incremental { resynced: true }` (test
  `numpages_in_the_body_stays_incremental`). Đổi số trang → dàn lại toàn bộ (mọi trang có NUMPAGES đều đổi). Bộ nhớ: probe
  giữ thêm một bản layout (chỉ khi thân có NUMPAGES).
- **Sửa đầu/chân trang**: transaction ghi `BlockChanges::mark_headers(section)` (op section chỉ khác headers/footers/titlePg,
  xem trước khi gõ trong header, evenAndOddHeaders) thay vì "mọi thứ". Nếu chiều cao đầu/chân trang không đổi, layout chỉ
  trang trí lại các trang từ section đó trở đi, không phân trang lại (`header_edits_redecorate_without_repaginating`,
  `typing_in_a_header_redecorates_without_repaginating` qua engine).
- Proptest `random_edits_match_full_layout` sinh thêm đoạn NUMPAGES và sửa header (ngắn/dài): kết quả tăng dần == layout đầy đủ.
