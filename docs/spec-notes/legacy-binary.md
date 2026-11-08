# Định dạng nhị phân cũ (.doc / .xls / .xlsb / .ppt / VBA) — ghi chú đọc spec

Nguồn: Microsoft Open Specifications [MS-CFB], [MS-DOC], [MS-XLS], [MS-XLSB], [MS-PPT], [MS-OVBA], [MS-OFFMACRO2].
Ghi chú chỉ diễn giải và dẫn số mục, không chép nội dung spec. Crate: `crates/bo-binary` (chỉ đọc, kiểm biên; đầu vào hỏng
trả lỗi chứ không panic — `src/bytes.rs`).

## 1. Phạm vi đã cài

- **[MS-CFB] Compound File**: không tự cài — dùng crate `cfb` 0.14 (`crates/bo-binary/Cargo.toml`); chỉ tự kiểm chữ ký
  header §2.2 (`src/lib.rs`, `crates/bo-engine/src/lib.rs`).
- **[MS-DOC]** — `src/doc.rs`: FIB (FibBase §2.5.2, FibRgLw97 §2.5.5, cấu trúc §2.5.1) → table stream → CLX/piece table
  (§2.9.38 Clx, §2.9.177 PlcPcd, §2.9.178 Pcd) → CHPX/PAPX qua bin table và FKP (§2.9.31, §2.9.33, §2.9.166, §2.9.167,
  §2.9.175) → stylesheet STSH §2.9.271 (STD §2.9.260) → bảng font SttbfFfn §2.9.272 (FFN §2.9.88) → section PlcfSed §2.9.255
  / Sepx §2.9.252. Thuộc tính: Sprm §2.2.5.1, CHP §2.6.1 (toggle), SEP §2.6.4; danh sách LSTF/LVL/LVLF/LFO (§2.9.147,
  §2.9.150, §2.9.151, §2.9.131, PlfLst/PlfLfo §2.9.191/.192), MSONFC §2.9.158; màu Ico §2.9.119 / COLORREF §2.9.43.
  Sau khi đọc, chạy chuyển mã TCVN3/VNI → Unicode (`bo_encoding::convert_document`, tắt được — test
  `legacy_conversion_can_be_disabled`).
- **[MS-XLS] BIFF8** — `src/xls.rs`: substream globals §2.1.7.20.3 (BoundSheet8 §2.4.28, SST §2.4.265 + CONTINUE,
  Format §2.4.126, XF §2.4.353, Date1904 §2.4.77, SupBook/ExternSheet/ExternName §2.4.271/.106/.105, Lbl §2.4.150) và
  substream sheet §2.1.7.20.5 (Number, RK, MulRk, Label, LabelSst, RString, BoolErr, Formula §2.4.127, ShrFmla §2.4.260,
  String §2.4.268). Chuỗi XLUnicodeString §2.5.294, ShortXLUnicodeString §2.5.240.
- **Công thức** — `src/xls_formula.rs`, `src/ftab.rs`: Rgce §2.5.198 (BIFF8) và biến thể BIFF12 [MS-XLSB] §2.5.97 dùng chung
  token, khác độ rộng hàng/cột và mã hoá chuỗi (`Dialect`); bảng hàm Ftab [MS-XLS] §2.5.198.17 nối thêm phần mở rộng
  [MS-XLSB] §2.5.97.10. Kết quả là chuỗi công thức kiểu A1.
- **[MS-XLSB]** — `src/xlsb.rs`: gói OPC chứa luồng record BIFF12 (§2.1.4; header record §2.3): workbook (BrtBundleSh
  §2.4.302, BrtWbProp §2.4.304, BrtExternSheet, BrtName), shared strings, styles (BrtFmt, BrtXF), worksheet (BrtRowHdr,
  BrtCell*, BrtFmla*).
- **[MS-PPT]** — `src/ppt.rs`: `Current User` (CurrentUserAtom §2.3.2) → chuỗi UserEditAtom §2.3.3 qua `offsetLastEdit`
  → gộp PersistDirectoryAtom (§2.3.4, entry §2.3.5) → DocumentContainer §2.4.1 (DocumentAtom §2.4.2, khổ slide) →
  SlideListWithTextContainer §2.4.14.3 → SlideContainer §2.5.1 / NotesContainer §2.5.6; chữ lấy từ cặp TextHeaderAtom +
  TextCharsAtom/TextBytesAtom (TextTypeEnum §2.13.33).
- **[MS-OVBA] VBA** — `src/vba.rs`: phát hiện macro (part `vbaProject.bin`; storage `Macros`/`_VBA_PROJECT_CUR`; sheet
  macro XLM; `VBAInfoContainer` của PowerPoint), giải nén CompressedContainer §2.4.1 (chunk header §2.4.1.1.5, CopyToken
  §2.4.1.3.19.1), đọc danh sách module từ luồng `VBA/dir` §2.3.4.2. Test `decompress_spec_example` dùng ví dụ của §3.2.3.
  **Không biên dịch, không thực thi** (doc comment của `vba.rs` và `read_vba_project`).
- **Đường chuyển đổi** (đọc → OOXML/ODF; không ghi lại định dạng nhị phân):
  - `.doc` → `bo_model::Document`: mở trực tiếp trong BlueDoc (`bo-engine/src/lib.rs`), `bo-cli convert` sang
    docx/odt/txt/pdf; khi lưu docx, `vba::carry_over` chép project VBA và đổi content type sang bản macro-enabled (đuôi
    `.docm`) — `crates/bo-cli/src/convert.rs` `document_to_docx`, `bo-cli/src/main.rs`.
  - `.xls`/`.xlsb` → BlueSheet (`bo-engine/src/sheet.rs` `from_legacy`), `bo-cli convert` sang xlsx/csv/ods/pdf.
  - `.ppt` → mô hình chữ ODF (`deck_to_presentation`), chỉ `bo-cli convert --to odp`.

## 2. Điểm mơ hồ & cách đã chọn

- **Grpprl có kích thước biến thiên** (`doc.rs`): sprmTDefTable dùng cb 2 byte; sprmPChgTabs với cb 255 (bố cục phức tạp)
  thì bỏ phần còn lại của grpprl thay vì đoán.
- **Nhãn danh sách** (`doc.rs` `next_label`): bộ đếm theo (lsid, cấp), cấp sâu hơn được đặt lại; LSTF.fSimpleList = một cấp;
  bullet font Symbol vùng private-use (vd U+F0B7) đổi sang ký tự Unicode tương ứng.
- **Font**: dùng sprmCRgFtc0 (font ASCII), không dùng fallback font Bi.
- **SST qua CONTINUE** (`xls.rs`): mỗi lần chuỗi ký tự tràn sang record CONTINUE thì đọc lại cờ `fHighByte` mới.
- **Sheet hỏng / chart nhúng**: substream sheet hỏng cho ra sheet rỗng thay vì làm hỏng cả workbook; substream chart nhúng
  chỉ theo dõi độ lồng BOF/EOF rồi bỏ qua; chart sheet và macro sheet bị bỏ qua (`sheet.rs`, `xlsb.rs`).
- **PPT thiếu persist directory** (`ppt.rs`): nếu không tìm được DocumentContainer qua persist directory thì quét record
  cấp cao nhất; test `fallback_without_current_user` cho thấy vẫn đọc được chữ của slide list. Hộp `OutlineTextRefAtom`
  bị bỏ để không lặp chữ của slide list.
- **Giới hạn chống bom**: số ô tối đa, số shared string, độ lồng substream (`xls.rs`, `xlsb.rs`), số ký tự thân `.doc`,
  độ sâu kế thừa style (`doc.rs`), cỡ giải nén tối đa 64 MiB mỗi luồng VBA (`vba.rs`).

## 3. Quan sát black-box

Code không ghi quan sát black-box. Mọi file mẫu trong test được dựng bằng code (`sample_doc`, `sample_xls`, `sample_xlsb`,
`sample_ppt`); REQUIREMENTS P5.1 ghi rõ chưa có file `.doc` thật trong corpus. Fuzz: `fuzz/fuzz_targets/{doc,xls,xls_formula,
xlsb,ppt,vba}.rs`.

## 4. Chưa hỗ trợ

- File mã hoá: `.doc` có fEncrypted, `.xls` có FilePass §2.4.117 (RC4/XOR), `.ppt` có encryptSessionPersistIdRef đều trả
  `BinaryError::Encrypted` — RC4/CryptoAPI chưa cài (`xls.rs`, `doc.rs`, `ppt.rs`, `crates/bo-crypto/src/lib.rs`).
- `.ppt` chỉ lấy chữ: không shape/ảnh, không thành `bo_model::slide`, BlueSlide không mở, không lưu `.pptx`
  — `docs/REQUIREMENTS.md` 2.10, P5.4.
- Chuyển mã TCVN3/VNI chỉ áp dụng khi mở `.doc` — REQUIREMENTS 2.24.
- Cảnh báo macro chỉ có ở BlueDoc, chưa có ở BlueSheet/BlueSlide — REQUIREMENTS P5.15.
- Server `POST /convert` chưa nối doc→docx, xls→xlsx (chỉ có trong `bo-cli`) — REQUIREMENTS 6A.21.
- Không ghi ngược ra `.doc/.xls/.xlsb/.ppt` (crate chỉ có reader).
