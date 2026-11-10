# WordprocessingML (.docx) — ghi chú đọc spec

Nguồn: ECMA-376 Part 1 §17 (WordprocessingML), §20.4 (DrawingML WordprocessingML Drawing), Part 3 §10.2
(Markup Compatibility), [MS-DOCX], [MS-ODRAWXML]. Chỉ ghi số mục, nội dung là diễn giải của nhóm.
Mã: `crates/bo-docx/src/reader.rs`, `crates/bo-docx/src/writer.rs`, mô hình `crates/bo-model/src/write.rs`,
chỉnh sửa `crates/bo-edit/src/*`.

## 1. Phạm vi đã cài

- **Part và quan hệ** (`reader.rs` `read_docx`): main document qua officeDocument; styles, numbering, settings,
  footnotes, endnotes, comments, commentsExtended, commentsIds qua relationship của main part.
- **Styles** — §17.7.5.1 docDefaults, §17.7.4.17 style (`name`, `basedOn`, `next`, `link`, `qFormat`, pPr/rPr),
  §17.7.4.5 latentStyles giữ nguyên XML (`read_styles`, `read_style`).
- **Numbering** — §17.9.1 abstractNum, §17.9.15 num (+ `lvlOverride/startOverride`), §17.9.6 lvl
  (`start`, `numFmt`, `lvlText`, `lvlJc`, `lvlRestart`, `suff`, pPr/rPr) — `read_numbering`, `read_level`.
- **Settings** — §17.15.1.25 defaultTabStop, §17.15.1.35 evenAndOddHeaders (`read_settings`).
- **Đoạn** — pPr: §17.3.1.12 ind, §17.3.1.13 jc, §17.3.1.19 numPr, §17.3.1.24 pBdr, §17.3.1.31 shd, §17.3.1.33 spacing,
  §17.3.1.38 tabs, §17.3.1.29 rPr của dấu đoạn; thứ tự CT_PPrBase khi ghi theo §17.3.1.26 (`writer.rs`).
- **Run** — rPr: §17.3.2.26 rFonts, §17.3.2.38 sz, §17.3.2.40 u, §17.3.2.6 color, §17.3.2.42 vertAlign và các
  toggle khác (§17.3.2.1, .5, .9, .15, .16, .19, .28, .32, .33, .35, .37, .41); nội dung §17.3.3.31 t, §17.3.3.1 br,
  §17.3.3.7 delText, §17.3.3.9 drawing.
- **Bảng** — §17.4.38 tbl, §17.4.60 tblPr, §17.4.39 tblBorders, §17.4.79 tr, §17.4.66 tc / tcBorders, §17.4.70 tcPr;
  độ rộng §17.18.90 (`table_width`).
- **Section** — §17.6.17 sectPr (trong dấu đoạn kết thúc section), §17.6.13 pgSz, §17.6.11 pgMar, §17.6.12 pgNumType,
  §17.6.4 cols, §17.6.22 type; header/footer §17.10.5 headerReference / §17.10.3 footerReference, part §17.10.4 hdr /
  §17.10.2 ftr; chỉnh sửa trong `crates/bo-edit/src/section.rs`.
- **Chú thích** — §17.11.15 footnotes, §17.11.8 endnotes, §17.11.11 footnotePr / §17.11.4 endnotePr (numFmt §17.11.17,
  numStart §17.11.20, numRestart §17.11.19, pos §17.11.21/.22), separator §17.11.23 / continuationSeparator §17.11.1.
- **Comment** — §17.13.4.2 comment, §17.13.4.6 comments part, range §17.13.4.4/§17.13.4.3, §17.13.4.5
  commentReference; luồng trả lời và cờ "done" theo [MS-DOCX] §2.5.2 `w15:commentEx`, id bền `w16cid:commentsIds`
  (`read_comments_ext`, `read_comments_ids`; `crates/bo-edit/src/comments.rs`).
- **Track changes** — §17.13.5: ins §17.13.5.18, del §17.13.5.14, moveTo §17.13.5.25, moveFrom §17.13.5.22,
  rPrChange §17.13.5.31, pPrChange §17.13.5.29, ins/del dấu đoạn §17.13.5.16/.15, hàng §17.13.5.19/.12,
  tcPrChange §17.13.5.36, tblPrChange §17.13.5.34, sectPrChange §17.13.5.32 (`crates/bo-edit/src/revisions.rs`).
- **Field** — §17.16.18 fldChar, §17.16.23 instrText, §17.16.19 fldSimple, §17.16.22 hyperlink; cập nhật TOC
  §17.16.5.68, SEQ §17.16.5.56, REF §17.16.5.51, PAGEREF §17.16.5.45, MERGEFIELD §17.16.5.35, switch định dạng số
  §17.16.4.3 (`crates/bo-edit/src/fields.rs`, `crates/bo-edit/src/merge.rs`); PAGE/NUMPAGES/SECTIONPAGES tính ở
  layout (`crates/bo-layout/src/paragraph.rs`).
- **Mục lục hình/bảng, chỉ mục, bảng trích dẫn** — TOC `\c`/`\a` (§17.16.5.68) lấy đoạn có SEQ; INDEX §17.16.5.29
  từ XE §17.16.5.72 (`"chính:phụ"`, `\b \i \r \t \f`; INDEX `\e \l \g \k \h \p \r \f \b \z`, `\c` = section liên tục
  có cột); TOA §17.16.5.67 từ TA §17.16.5.62 (`\l \s \c`; TOA `\c \h \p \e \l \g \b`; tên nhóm do BlueDoc tự đặt).
  Dấu dẫn của số trang giữ ở tab phải của đoạn đầu trường. Sắp xếp tiếng Việt theo `collate.rs`. Chuyển chú thích
  chân trang ↔ cuối văn bản: §17.11.14 / §17.11.7 (`crates/bo-edit/src/refs.rs`).
- **Ảnh** — §20.4.2.8 inline / §20.4.2.3 anchor, §20.4.2.7 extent, wrap §20.4.2.17 wrapSquare, §20.4.2.20 wrapTight +
  §20.4.2.16 wrapPolygon, §20.4.3.7 ST_WrapText; §20.1.8.13 blip, §20.1.8.55 srcRect, §20.1.7.6 xfrm (`read_drawing`).
- **Lossless** — ADR-0005: XML gốc + fingerprint mỗi block; con không hiểu của pPr/rPr/tblPr/tcPr/trPr ghi lại theo
  thứ tự schema; block lạ (`w:sdt`, `w:customXml`) là `Block::Opaque`.

## 2. Điểm mơ hồ & cách đã chọn

- **ST_OnOff** (§17.17.4): thiếu `val` = bật; chỉ `0`/`false`/`off` là tắt (`reader.rs` `on_off`).
- **Độ rộng phần trăm** (§17.18.90): chấp nhận cả số nguyên phần năm mươi phần trăm lẫn chuỗi `NN%` (`table_width`).
- **Giá trị mặc định khi vắng**: defaultTabStop = 720 twip; `cols` có `num` = 1, `space` = 720 (`writer.rs`).
- **Ô bảng phải kết thúc bằng đoạn** (§17.4.66): writer luôn chèn đoạn cuối ô nếu thiếu.
- **Nội dung của pPrChange/rPrChange**: pPrChange chứa CT_PPrBase (không rPr/sectPr/pPrChange lồng),
  rPrChange chứa CT_RPrOriginal (không rPrChange lồng) — writer lọc theo đó.
- **Range comment** nằm ngoài run (§17.13.4.4/.3) → mô hình là `RunContent::Markup`, không phải nội dung run.
- **Field phức hợp**: chỉ gộp thành `RunContent::Field` khi chuỗi begin…end nằm trọn trong một đoạn, không lồng;
  SEQ/REF/PAGEREF/MERGEFIELD chỉ gộp khi phạm vi không chứa gì khác ngoài cấu trúc field và văn bản kết quả
  (để không mất dữ liệu) — `collapse_simple_fields`. Còn lại giữ `Opaque`.
- **mc:AlternateContent** (Part 3 §10.2.1): dùng ảnh trong `mc:Fallback` nếu có, ngược lại giữ nguyên văn.
- **SmartArt**: hiển thị drawing dựng sẵn ([MS-ODRAWXML] §2.3.2) tìm qua `dsp:dataModelExt/@relId` của data model.
- **Theme font** (`asciiTheme`): giữ trong `unknown` để ghi lại, layout dùng font dự phòng; `rFonts` đọc `ascii`
  rồi mới tới `hAnsi`.
- **Header/footer liên kết** (§17.10.5): section không có tham chiếu riêng dùng của section gần nhất trước nó; sửa
  header liên kết là sửa header nguồn (`section.rs` `effective_hf`, `hf_source`).
- **paraId comment mới**: 8 chữ số hex nhỏ hơn 0x80000000 ([MS-DOCX] ST_LongHexNumber) — `comments.rs`.
- Một số trích dẫn [MS-DOCX] trong code chưa có số mục chính xác (`§2.6.x`, `§2.2.x` ở `reader.rs`) — cần bổ sung.

## 3. Quan sát black-box

- Đa giác wrap tight được hiểu trong hình vuông 21 600 đơn vị phủ extent "mà Word dùng" (`crates/bo-model/src/write.rs`
  trường `polygon`).
- Đánh số chữ cái sau Z lặp ký tự (AA, BB, …) (`crates/bo-model/src/write.rs` `letters`).
- Validator: `w:smartTag` hợp lệ theo §17.5.1.9 nhưng OpenXmlValidator báo lỗi khi kiểm theo bản 2019 — ghi ở
  `tests/fidelity/known-issues.txt`.

## 4. Chưa hỗ trợ

- Text box & shape (`wps:wsp`, `w:txbxContent`): không mô hình/hiển thị, chỉ giữ khi lưu (`docs/REQUIREMENTS.md`
  Khoảng trống #1, P2.12).
- Track changes chưa ghi: chèn/xoá cột-ô, gộp/tách ô, header/footer, nội dung chú thích, ảnh nổi đổi vị trí
  (`docs/PROGRESS.md` Hạn chế #3, P5.7). Chú thích dài chưa tràn trang.
- Run font: hAnsi gộp với ascii, eastAsia/cs chưa dùng khi layout, theme font chưa phân giải (Khoảng trống #7).
- Field lồng nhau / field trải nhiều đoạn chưa được tính (giữ opaque); switch số chỉ ARABIC/ROMAN/roman/ALPHABETIC/
  alphabetic (`fields.rs` `format_seq`).
- Style gallery chỉ 4 style cố định (P2.15); EMF/WMF chỉ ánh xạ MIME, không vẽ (3.12); không có `.rtf` (2.4).
- Layout dàn lại toàn bộ khi có NUMPAGES trong thân bài, đổi style/numbering/section (Hạn chế #2).
- Cộng tác: bảng là một khối; style/numbering/comment/nội dung footnote chưa đồng bộ (Hạn chế #1).
