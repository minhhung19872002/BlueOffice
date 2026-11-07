# Corpus

File mẫu dùng cho round-trip, visual và E2E test. **Không** đưa file chứa dữ liệu khách hàng thật.
Mọi nội dung đều là giả định.

## doc/ — BlueDoc

Phase 2: 11 nhóm tính năng × 3 file, sinh bằng `cargo run -p bo-cli -- corpus tests/corpus/doc`
(XML WordprocessingML viết tay theo ECMA-376 Part 1 trong `crates/bo-cli/src/corpus.rs`, kèm markup không
được mô hình hoá — rsid, `w14:*`, `mc:AlternateContent`, `w:sdt`, tracked changes — để kiểm tra lossless).
Kiểm thử: `cargo test -p bo-docx --test corpus` (round-trip, ngưỡng ≥ 95 %) và
`cargo test -p bo-engine` (mở, dàn trang, PNG, PDF, lưu lại toàn bộ corpus).

| File | Nguồn gốc | Nội dung |
|---|---|---|
| `doc/phase1-sample.docx` | Tự sinh: `cargo run -p bo-cli -- sample tests/corpus/doc/phase1-sample.docx` (`crates/bo-cli/src/sample.rs`) | Văn bản hành chính giả định theo bố cục NĐ 30/2020: quốc hiệu, tiêu ngữ, căn đều, thụt lề, giãn dòng, đậm/nghiêng/gạch chân/gạch kép/gạch ngang/màu, fallback Arial/Calibri, tab, ngắt dòng, phân trang tự động, ngắt trang, section ngang A4 |
| `doc/text-01-run-props.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | text: Đậm/nghiêng/gạch chân các kiểu, gạch ngang đơn/kép, màu, cỡ chữ, phông |
| `doc/text-02-highlight-sup-caps.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | text: Tô sáng, nền, chỉ số trên/dưới, chữ hoa, chữ hoa nhỏ, giãn chữ, kerning |
| `doc/text-03-vietnamese-rsid.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | text: Tiếng Việt nhiều run, rsid, proofErr, lastRenderedPageBreak, w14:paraId, ký tự đặc biệt |
| `doc/para-01-align-indent.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | paragraph: Căn trái/giữa/phải/đều, thụt lề trái/phải/dòng đầu/treo |
| `doc/para-02-spacing-keep.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | paragraph: Khoảng cách trước/sau, giãn dòng auto/exact/atLeast, keepNext, keepLines, widowControl, pageBreakBefore, contextualSpacing |
| `doc/para-03-tabs-borders.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | paragraph: Tab stop tuỳ biến (trái/giữa/phải/thập phân, dẫn chấm), viền và nền đoạn |
| `doc/styles-01-headings.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | styles: docDefaults, latentStyles, Heading1 kế thừa Normal, next style |
| `doc/styles-02-based-on-chain.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | styles: Chuỗi basedOn Heading2 → Heading1 → Normal, style đoạn tuỳ biến |
| `doc/styles-03-character-style.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | styles: Kiểu ký tự (rStyle) cùng định dạng trực tiếp, kiểu bảng |
| `doc/lists-01-multilevel.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | lists: Danh sách số nhiều cấp (decimal, lowerLetter, lowerRoman, upperRoman) |
| `doc/lists-02-bullets.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | lists: Danh sách gạch đầu dòng các cấp xen đoạn thường |
| `doc/lists-03-restart-override.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | lists: Hai num cùng abstractNum, lvlOverride/startOverride khởi động lại |
| `doc/sections-01-landscape-columns.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | sections: Section dọc → ngang (nextPage) → 2 cột (continuous) |
| `doc/sections-02-header-footer.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | sections: Header/footer default + first (titlePg), số trang PAGE/NUMPAGES |
| `doc/sections-03-even-odd-restart.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | sections: Header chẵn/lẻ (evenAndOddHeaders), section oddPage, đánh số lại pgNumType |
| `doc/tables-01-header-rows-long.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | tables: Bảng dài qua nhiều trang, lặp hàng tiêu đề, nền ô |
| `doc/tables-02-merge.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | tables: Gộp ô ngang (gridSpan) và dọc (vMerge), vAlign, độ rộng pct |
| `doc/tables-03-nested-borders.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | tables: Bảng lồng nhau, viền ô riêng, lề ô, bảng không viền |
| `doc/images-01-inline.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | images: Ảnh PNG inline trong dòng chữ, hai ảnh dùng chung part |
| `doc/images-02-anchored-wrap.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | images: Ảnh nổi wrapTopAndBottom, wrapSquare, behindDoc (wrapNone) |
| `doc/images-03-crop-rotate.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | images: Ảnh cắt (srcRect) và xoay (xfrm rot) |
| `doc/links-01-external.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | links: Hyperlink ngoài (quan hệ External), kiểu Hyperlink |
| `doc/links-02-bookmarks.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | links: Bookmark và hyperlink nội bộ (w:anchor) |
| `doc/links-03-ranges-permissions.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | links: Bookmark lồng qua nhiều đoạn, permStart/permEnd, commentRange không có phần chú thích |
| `doc/notes-01-footnotes.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | notes: Chú thích cuối trang (footnote) với dấu phân cách |
| `doc/notes-02-endnotes.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | notes: Chú thích cuối văn bản (endnote) |
| `doc/notes-03-many-footnotes.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | notes: Nhiều footnote dồn trang, footnote nhiều đoạn |
| `doc/fields-01-simple.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | fields: fldSimple PAGE, NUMPAGES, DATE |
| `doc/fields-02-complex.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | fields: Field phức (fldChar begin/separate/end, instrText) PAGE, DATE, REF |
| `doc/fields-03-toc.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | fields: Mục lục TOC với kết quả cache, sdt docPartGallery |
| `doc/fields-04-toc-stale.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | fields: Mục lục TOC (fldChar nhiều đoạn, hyperlink + PAGEREF lồng) với mục cũ/sai trang, tiêu đề cấp 1–3 (style + outlineLvl) có bookmark _Toc — cần “Cập nhật trường” |
| `doc/fields-05-captions-seq.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | fields: Tiêu đề hình/bảng: SEQ Hình/Bảng (field phức và fldSimple) có số cũ, \* ROMAN, \r đặt lại, \c lặp |
| `doc/fields-06-crossref.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | fields: Tham chiếu chéo REF/PAGEREF tới bookmark _Ref của tiêu đề hình và bookmark người dùng, kết quả cache cũ |
| `doc/unknown-01-sdt-customxml.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | unknown: Content control (w:sdt) cấp khối và cấp run, customXml, smartTag |
| `doc/unknown-02-alternate-content.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | unknown: mc:AlternateContent (Choice wps / Fallback VML), w14 trong rPr, ký hiệu w:sym |
| `doc/unknown-03-tracked-changes.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | unknown: Theo dõi thay đổi w:ins / w:del, rPrChange, pPrChange |
| `doc/comments-01-basic.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | comments: Chú thích một đoạn chữ, chú thích trải qua hai đoạn, chú thích tại một điểm (chỉ commentReference) |
| `doc/comments-02-replies.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | comments: Luồng trả lời (w15:commentEx paraIdParent), luồng đã xử lý (done), chú thích nhiều đoạn |
| `doc/comments-03-ids-tracked.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | comments: commentsIds (w16cid durableId), chú thích trong bảng, chú thích trên vùng có track changes |
| `doc/review-01-insert-delete.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | review: Track changes: w:ins/w:del hai tác giả, chèn rồi xoá (ins ⊃ del), ins trong hyperlink, moveFrom/moveTo kèm range |
| `doc/review-02-format-changes.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | review: Track changes định dạng: rPrChange (đậm, màu, cỡ, phông), pPrChange (căn lề, thụt lề, giãn dòng, style) |
| `doc/review-03-paragraph-marks.docx` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | review: Track changes dấu đoạn: ins/del trong pPr/rPr (kèm định dạng dấu đoạn), field trong vùng chèn, bảng có ô chứa thay đổi |
| `slide/*.pptx` (25 file) | Tự sinh: `cargo run -p bo-cli -- sample-slides tests/corpus/slide` (`crates/bo-cli/src/slide_samples.rs`); test `committed_corpus_is_up_to_date` bảo đảm file khớp generator | BlueSlide: `text-*` (kế thừa placeholder, bullet nhiều cấp, đánh số, autofit fontScale/lnSpcReduction, chia cột), `shapes-*` (≈60 preset, gradient/pattern, nét đứt, mũi tên, connector, custGeom, nhóm lồng xoay/lật, đổ bóng), `images-*` (PNG, crop, alpha, ảnh theo hình, nền ảnh), `tables-*` (gộp ô, lề ô, viền), `charts-*` (cột, thanh chồng, đường, tròn), `anim-*` (fade/push/wipe/cover/random, p14 AlternateContent, appear/fade/fly-in/wipe), `media-*` (video/audio nhúng & liên kết — byte media giả), `notes-*`/`theme-*` (ghi chú, notes master, clrMapOvr, nền gradient/bgRef/pattern, ẩn slide, showMasterSp). Ảnh sinh bằng code, chữ/số giả định |

Phase 3 (`sheet/`), Phase 4 (`slide/`) bổ sung sau.
