# Phase 5 — Định dạng cũ & tính năng nâng cao (phần định dạng)

Trạng thái: **phần định dạng hoàn thành — chờ duyệt** (nhánh `wip/phase5-legacy-crypto-odf`).
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
- **Tính năng tài liệu (chưa làm, thuộc Phase 5 nhưng phụ thuộc mô hình/editor Phase 2–4)**:
  track changes (ins/del/rPrChange/pPrChange: hiển thị, accept/reject, ghi nhận), comments có luồng
  trả lời, TOC tự động/caption/cross-reference, mail merge (CSV/XLSX), PivotTable (đọc cache, tạo pivot
  cơ bản), SmartArt (hiển thị drawing fallback). Cần `bo-model` có revision/comment/field/pivot và
  `bo-edit` có transaction trước.
- UI cảnh báo macro (dùng `has_macros`) và hộp thoại mật khẩu (dùng `open_package`/`save_package`)
  chưa nối vào editor.
