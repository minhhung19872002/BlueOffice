# Phase 5 — Định dạng cũ & tính năng nâng cao (phần định dạng)

Trạng thái: **phần định dạng hoàn thành — chờ duyệt** (nhánh `wip/phase5-legacy-crypto-odf`);
**tính năng tài liệu BlueDoc (§5) hoàn thành — chờ duyệt**.
Prompt gốc: [`BlueOffice-PROMPT.md`](../../BlueOffice-PROMPT.md) §6 Phase 5, §5 (giới hạn an toàn), §7 (fuzz).

Nguồn kỹ thuật duy nhất: [MS-DOC], [MS-XLS], [MS-XLSB], [MS-PPT], [MS-CFB], [MS-OFFCRYPTO],
[MS-OVBA], ECMA-376, OASIS ODF 1.3. Mọi file thử nghiệm được **tự sinh trong test** (CFB qua crate
`cfb`), không dùng file của bên thứ ba hay khách hàng.

## 1. Đã cài đặt

| Crate | Nội dung | API chính |
|---|---|---|
| `bo-binary` | `.doc`: FIB, piece table (CLX, nén cp1252 + UTF-16), CHPX/PAPX qua FKP, stylesheet (kế thừa `istdBase`), bảng font, SEPX (khổ giấy, lề), list (LSTF/LVL/LFO → nhãn số/bullet), field (ẩn mã, giữ kết quả), chuyển TCVN3/VNI theo font → `bo_model::Document` | `read_doc`, `read_doc_with(DocOptions)` |
| | `.xls` BIFF8: BoundSheet8, SST (kể cả CONTINUE tách chuỗi), Format/XF → mã định dạng số, Date1904, SupBook/ExternSheet/ExternName, Lbl; ô Number/RK/MulRk/LabelSst/Label/BoolErr/Formula(+String); ShrFmla/Array qua PtgExp; bỏ qua substream chart; FilePass → `Encrypted` | `read_xls` → `Workbook` |
| | Công thức BIFF8/BIFF12 → text A1: toàn bộ Ftab [MS-XLS] §2.5.198.17 (0x000–0x17B) + phần mở rộng [MS-XLSB] (0x17C–0x1E4, `IFERROR`, `SUMIFS`…), PtgFunc/FuncVar/UDF, 3-D ref, name, NameX (`_xlfn.*`), RefN/AreaN, mảng hằng (BIFF8) | `xls_formula::formula_to_text_with(Dialect, …)` |
| | `.xlsb`: BrtBundleSh, BrtWbProp, BrtExternSheet, BrtName, SST, styles (BrtFmt + cellXfs), ô BrtCell*/BrtFmla*, BrtShrFmla/BrtArrFmla | `read_xlsb`, `read_xlsb_package` |
| | `.ppt`: CurrentUser → chuỗi UserEditAtom → PersistDirectory, DocumentAtom (khổ slide → EMU), SlideListWithText (tiêu đề/thân), drawing của slide (text box tự do), notes, fallback khi thiếu Current User; mã hoá → `Encrypted` | `read_ppt` → `PptDeck` |
| | VBA [MS-OVBA]: giải nén §2.4.1, đọc `VBA/dir` (danh sách module), phát hiện macro (OOXML `vbaProject.bin`, `Macros`/`_VBA_PROJECT_CUR`, XLM macro sheet, `VBAInfoContainer`), chuyển dự án VBA sang package mới khi lưu (`carry_over`, đổi content type sang macroEnabled). **Không bao giờ thực thi.** | `has_macros`, `read_vba_project`, `vba::carry_over` |
| | Nhận diện CFB | `detect` → `LegacyKind` |
| `bo-crypto` | Standard (AES-ECB, SHA-1) giải/mã hoá; Agile (AES-CBC theo segment 4096, SHA-1/256/384/512, HMAC toàn vẹn) giải/mã hoá — mặc định AES-256/SHA-512/100 000 vòng; hash bảo vệ sheet/workbook (legacy 16-bit + ISO SHA-512); `open_package`/`save_package` | `decrypt`, `encrypt`, `open_package`, `save_package`, `protection::*` |
| `bo-encoding` | TCVN3 (ABC) và VNI-Windows ↔ Unicode NFC, nhận diện theo font (`.Vn*`, `VNI-*`) và heuristic nội dung, chuyển cả `Document` | `convert`, `detect_legacy_vietnamese`, `convert_document` |
| `bo-odf` | ODF 1.3: `.odt` đọc/ghi (đoạn, heading, span, style tự động, trang), `.ods` đọc/ghi (kiểu giá trị, lặp hàng/cột có giới hạn, công thức `of:` nguyên văn), `.odp` đọc/ghi (frame theo `presentation:class`, notes) | `read_odt`/`write_odt`, `read_ods`/`write_ods`, `read_odp`/`write_odp` |
| `bo-cli` | `convert x.doc|.odt|.docx --to docx|odt|txt|pdf`, `x.xls|.xlsb|.ods --to csv|ods`, `x.ppt|.odp --to odp|txt`, `--password` (đầu vào mã hoá), `--protect` (mã hoá .docx đầu ra); `inspect` (định dạng, mã hoá, macro) | — |

Kiểu trung gian `bo_binary::Workbook`/`PptDeck` và `bo_odf::SpreadsheetDoc`/`Presentation` được giữ
đơn giản; ánh xạ sang mô hình BlueSheet/BlueSlide của Phase 3–4 nằm trong các hàm nhỏ
(`crates/bo-cli/src/convert.rs`: `workbook_to_ods`, `deck_to_presentation`, …) để thay khi merge.

## 2. An toàn (§5)

- `#![forbid(unsafe_code)]`, không `unwrap` trên dữ liệu file; mọi đọc qua `bytes::*_at` có kiểm biên.
- Giới hạn: stream CFB ≤ 512 MiB, ≤ 20 triệu ô/workbook, ≤ 10 triệu chuỗi SST, độ sâu style ≤ 16,
  substream lồng ≤ 8, container PPT ≤ 48 tầng, ≤ 4096 UserEditAtom (phát hiện vòng lặp),
  stack công thức ≤ 4096, text công thức ≤ 1 MiB, VBA giải nén ≤ 64 MiB, ODF lặp hàng/cột có trần.
- Fuzz: proptest (byte ngẫu nhiên + đột biến file mẫu) cho mọi parser; `fuzz/` có 10 target
  `cargo fuzz` (workspace riêng; `cargo check --manifest-path fuzz/Cargo.toml` biên dịch được trên
  Windows; chạy fuzz thật cần nightly trên Linux/macOS).

## 3. Kiểm thử

- `.doc`/`.xls`/`.xlsb`/`.ppt`/`vbaProject.bin` được dựng byte-by-byte trong test theo spec.
- Crypto: round-trip Standard/Agile, sai mật khẩu, HMAC bị sửa, vector đã biết của hash legacy
  (`password` → `83AF`), và known-answer cho dẫn xuất khoá Standard/Agile và hash ISO tính bằng một
  cài đặt độc lập (Python `hashlib`) theo công thức trong [MS-OFFCRYPTO] §2.3.4.7, §2.3.4.11, §2.4.2.4.
- VBA: ví dụ giải nén của [MS-OVBA] §3.2; dự án VBA giữ nguyên byte qua round-trip và qua `carry_over`.
- ODF: round-trip odt/ods/odp, fuzz fragment XML.

## 4. Chưa làm / hạn chế

- **Định dạng**: `.doc` chưa đọc bảng như bảng (ô thành đoạn), ảnh/OfficeArt, header/footer,
  footnote, textbox; nhãn list được "vật chất hoá" thành text vì `bo-model` chưa có numbering.
  `.xls` BIFF5/95 và RC4/XOR (FilePass) chưa hỗ trợ; merge, độ rộng cột, comment, chart chưa đọc.
  BIFF12: mảng hằng (`PtgArray`) và structured reference (`PtgList`) → công thức bỏ trống (giữ giá
  trị). `.ppt`: chỉ text + cấu trúc, chưa có hình/định dạng; RC4 CryptoAPI chưa hỗ trợ.
  Công thức A1 chưa dịch sang OpenFormula khi xuất `.ods`.
- **Xuất `.xlsx`/`.pptx`**: writer `bo-xlsx`/`bo-pptx` không có trên nhánh này → `bo-cli` xuất CSV/ODS
  và ODP; nối vào writer Phase 3/4 khi merge.
- **Tính năng tài liệu**: xem §5 (BlueDoc). PivotTable (đọc cache, tạo pivot cơ bản) thuộc BlueSheet —
  chưa làm.

## 5. Tính năng tài liệu (BlueDoc) — hoàn thành, chờ duyệt

| Tính năng | Mô hình / định dạng | Core (`bo-edit` / `bo-engine`) | Editor |
|---|---|---|---|
| Track changes (ECMA-376 Part 1 §17.13.5) | `RunProps.ins/del` (`w:ins`, `w:del` + `w:delText`/`delInstrText`, `w:moveTo`/`moveFrom`, ins ⊃ del lồng), `RunProps.change` (`rPrChange`), `ParaProps.mark_ins/mark_del` (`pPr/rPr/ins|del`), `ParaProps.change` (`pPrChange`); đọc/ghi lossless, field trong vùng theo dõi ghi dạng field phức | `revisions`: liệt kê, chấp nhận/từ chối theo id / vùng chọn / tất cả (1 bước undo, nối đoạn khi dấu đoạn mất); chế độ theo dõi ghi nhận gõ, xoá (xoá phần mình chèn thì xoá thật), Enter/Backspace qua dấu đoạn, định dạng ký tự/đoạn, tìm-thay, dán; tác giả = `Platform.auth.user` (mặc định "BlueOffice"), ngày từ đồng hồ host (`now` trong lệnh JSON). Layout: hiển thị *Mọi đánh dấu* (chèn gạch chân, xoá gạch ngang, màu theo tác giả), *Không đánh dấu* (bản cuối), *Bản gốc* (định dạng cũ của `rPrChange`) | Tab **Xem lại**: bật/tắt theo dõi (Ctrl+Shift+R), 3 chế độ hiển thị, chấp nhận/từ chối, tất cả, thay đổi trước/tiếp, ngăn xem lại liệt kê thay đổi |
| Chú thích có trả lời (§17.13.4, [MS-DOCX] §2.5.2) | `Document.comments` (`comments.xml`), neo `CommentStart/End/Ref`; `commentsExtended` (`paraIdParent`, `done`), `commentsIds` (`durableId`); part giữ nguyên khi không đổi | thêm (vùng chọn hoặc từ tại con trỏ), trả lời (neo cạnh chú thích gốc), đánh dấu đã xử lý / mở lại, xoá cả luồng và neo, chọn neo, chú thích trước/tiếp | Ngăn **Chú thích**: luồng + trả lời, đã xử lý, xoá; nút ribbon |
| Mục lục / tiêu đề hình / tham chiếu chéo (§17.16) | TOC giữ dạng field phức nhiều đoạn; SEQ/REF/PAGEREF/MERGEFIELD phức "sạch" gộp thành `RunContent::Field` khi đọc | `fields::update`: dựng lại TOC từ outline level (`\o`, `\h`, thêm bookmark `_Toc`, PAGEREF), đánh số SEQ (`\*` ARABIC/ROMAN/alphabetic, `\r`, `\c`, `\h`), REF/PAGEREF theo bookmark; dàn trang lặp tới khi số trang ổn định (≤ 3 lượt), 1 bước undo; chèn TOC, chèn tiêu đề hình/bảng, tham chiếu chéo (tự thêm bookmark `_Ref` cho tiêu đề hình) | Tab **Tham chiếu**: mục lục, cập nhật mục lục, tiêu đề hình/bảng, tham chiếu chéo, **Cập nhật trường (F9)**; `bo-cli fields` |
| Trộn thư (§17.16.5.35) | MERGEFIELD (tên có dấu cách, `\b`, `\f`, `\* Upper/Lower/Caps/FirstCap`) | nguồn CSV (RFC 4180, tự nhận `,` `;` tab) hoặc XLSX (sheet hiện đầu tiên, định dạng số của `bo-sheet`); xem trước bản ghi N (layout hiển thị giá trị), trộn thành 1 .docx (mỗi bản ghi một lượt section, header/footer được điền) hoặc PDF | Tab **Gửi thư**: chọn nguồn dữ liệu, chèn trường, xem trước/bản ghi trước-tiếp, trộn thành tài liệu / PDF; `bo-cli merge` |
| Macro, mật khẩu, định dạng cũ | `DocInfo.macros` (`bo-binary::has_macros`), `vbaProject.bin` giữ nguyên khi lưu .docm; `.doc` (`bo-binary::read_doc`) và `.odt` (`bo-odf::read_odt`) mở thành mô hình BlueDoc, lưu .docx | `Engine::open_with_password` ([MS-OFFCRYPTO] Standard/Agile qua `bo-crypto`; lỗi `password required` / `wrong password`), lưu kèm mật khẩu (Agile AES-256/SHA-512, `setSavePassword`); file mã hoá mở ra được lưu lại với cùng mật khẩu. `bo-crypto` dùng `getrandom` 0.4 (`wasm_js` trên web) | Thanh cảnh báo "Tài liệu chứa macro — macro sẽ được giữ nguyên nhưng không chạy", hộp thoại mật khẩu (thử lại khi sai), **Lưu kèm mật khẩu** (Tệp và Xem lại), trạng thái "Có mật khẩu" |
| SmartArt (hiển thị) | `mc:AlternateContent` có `mc:Fallback` là ảnh → hiện ảnh (XML gốc giữ nguyên); SmartArt `dgm:relIds` → part `dsp:drawing` ([MS-ODRAWXML]) theo `dsp:dataModelExt/@relId` | engine vẽ các hình của drawing part bằng renderer BlueSlide (`bo-pptx`) thành PNG trong bộ nhớ; part gốc giữ nguyên khi lưu | — |

Corpus mới (`tests/corpus/doc`, OpenXmlValidator hợp lệ, round-trip 100 %): `review-01..03`, `comments-01..03`,
`fields-04..06`, `merge-01..03` (+ `merge-data.csv`/`.xlsx`), `smartart-01..03`. Kiểm thử: `bo-edit` (đơn vị + proptest
theo dõi thay đổi: từ chối tất cả trả về văn bản gốc, chấp nhận tất cả = bản cuối, undo chính xác), `bo-engine`
(`tests/editing.rs`, `tests/security.rs`: cầu JSON, lưu/mở lại, corpus fields/merge/SmartArt, mật khẩu, macro, .odt),
Playwright `tests/e2e/specs/review.spec.ts` (chấp nhận/từ chối, luồng chú thích).

### Hạn chế còn lại
- TOC nằm trong `w:sdt` cấp khối (cách nhiều trình soạn thảo lưu) chưa cập nhật được vì nội dung sdt cấp khối còn là
  `Block::Opaque` (Phase 2); TOC ngoài sdt và mọi field khác cập nhật bình thường.
- Theo dõi thay đổi (bổ sung 2026-11-08): ghi nhận chèn/xoá hàng bảng (`trPr/ins|del`, xoá hàng mình chèn thì xoá thật, bảng mới
  = các hàng được chèn), thuộc tính ô (`tcPrChange`: tô nền, viền) và bảng (`tblPrChange`: rộng, căn, viền), ảnh (chèn/xoá như run,
  bỏ part ảnh khi từ chối), thiết lập trang/titlePg (`sectPrChange`); hiển thị: hàng tô màu tác giả (hàng xoá gạch ngang), vạch
  thay đổi bên trái bảng, ảnh có khung màu (ảnh xoá gạch chéo), ẩn trong chế độ bản cuối/bản gốc; ngăn xem lại liệt kê, chấp
  nhận/từ chối; proptest `track_tests`: sửa cấu trúc ngẫu nhiên có theo dõi rồi từ chối tất cả == ban đầu; corpus
  `review-04-tables-pictures-page-setup.docx`; E2E chèn hàng có theo dõi → lưu/mở lại → từ chối.
- Theo dõi thay đổi còn thiếu: ảnh nổi (anchor) đổi vị trí, chèn/xoá cột và ô (`cellIns`/`cellDel`/`cellMerge`, `trPrChange`
  được giữ nguyên khi đọc/ghi nhưng không hiển thị/chấp nhận); chế độ *Bản gốc* chưa dùng `pPrChange`/`tcPrChange` cho hình
  thức; vạch thay đổi ở lề chỉ có cho bảng. Trạng thái bật theo dõi không lưu vào `settings.xml` (`w:trackRevisions`).
- Chú thích: chưa tô vùng được chú thích trên trang, chưa sửa nội dung chú thích đã có, chú thích trong header/footer
  không hiển thị; `commentsExtensible`/`people.xml` giữ nguyên (không cập nhật).
- Trộn thư: chỉ MERGEFIELD (chưa IF/NEXT/ASK/FILLIN, chưa lọc/sắp xếp bản ghi); "Trộn thành PDF" = trộn rồi xuất PDF.
- Macro của file `.doc` không được chuyển sang .docm khi lưu (chỉ cảnh báo); `.xls`/`.ppt` không mở trong BlueDoc.
- SmartArt: chỉ hiển thị (không sửa); drawing fallback vẽ theo màu/hình khai báo trực tiếp, `schemeClr` lấy theo
  theme mặc định của renderer.
