# Bảng mã tiếng Việt cũ (TCVN3/ABC, VNI) — ghi chú đọc spec

Nguồn: bảng mã TCVN 5712:1993 (bảng VN3) và bảng mã VNI-Windows công khai; Windows-1252 theo bảng ánh xạ
WHATWG / Unicode.org `CP1252.TXT` (trích theo `crates/bo-encoding/src/lib.rs`, `cp1252.rs`). Theo ghi chú trong
code, không tham khảo font hay bộ chuyển mã độc quyền nào. Mã: `crates/bo-encoding`.

## 1. Phạm vi đã cài

- **Mô hình vấn đề** (`crates/bo-encoding/src/lib.rs`): văn bản gõ bằng font cũ đến tay reader dưới dạng ký tự đã
  giải mã Windows-1252 (vd TCVN3 `ViÖt Nam`, VNI `Vieät Nam`); crate đổi lại thành Unicode dựng sẵn — vốn đã là NFC
  vì mọi chữ tiếng Việt đều có code point dựng sẵn (test `outputs_are_nfc`).
- **Windows-1252** — `crates/bo-encoding/src/cp1252.rs`: `decode`/`encode`; 5 byte chưa gán (0x81, 0x8D, 0x8F, 0x90,
  0x9D) ánh xạ sang ký tự điều khiển C1 cùng giá trị.
- **TCVN3 / ABC** — `crates/bo-encoding/src/tables.rs` (bảng VN3): một byte cho mỗi chữ; 60 nguyên âm thường có dấu,
  6 chữ cơ sở ă â ê ô ơ ư, đ, và 7 chữ hoa Ă Â Ê Ô Ơ Ư Đ (test `tcvn3_table_is_injective_and_nfc`).
  API: `tcvn3_to_unicode`, `tcvn3_bytes_to_unicode`, `unicode_to_tcvn3`.
- **VNI-Windows** — `tables.rs`: chữ cơ sở + byte dấu theo sau (thanh, mũ, trăng), cộng một số chữ một byte
  (đ, ơ, ư, các dạng i, ỵ). API: `vni_to_unicode`, `vni_bytes_to_unicode`, `unicode_to_vni`; VNI có cả hoa lẫn thường.
- **Nhận diện theo font** — `legacy_from_font`: tên bắt đầu `.Vn` → TCVN3 (đuôi `H` = font chữ hoa), `VNI-`/`VNI ` →
  VNI (test `font_classification`).
- **Chuyển cả tài liệu** — `convert_document`: run có font cũ (của run hoặc mặc định tài liệu) được đổi chữ và đổi
  font sang `REPLACEMENT_FONT` = "Times New Roman"; gọi từ reader `.doc` (`crates/bo-binary/src/doc.rs`, tuỳ chọn
  `convert_legacy_vietnamese`, mặc định bật). Test `document_conversion`.
- **Kiểm chứng**: `tcvn3_known_strings`, `vni_known_strings`, `exhaustive_round_trip` (mọi chữ cái tiếng Việt
  Unicode → mã cũ → Unicode), `ascii_unchanged`, proptest `never_panics`, `bytes_never_panic`.

## 2. Điểm mơ hồ & cách đã chọn

- **Font chữ hoa TCVN3** (`.VnTimeH`, `.VnArialH`…): các font này vẽ chữ hoa tại chính code point của chữ thường,
  nên khi đổi với `upper = true` kết quả được viết hoa (`tcvn3_to_unicode`).
- **Mã hoá ngược sang TCVN3**: nguyên âm hoa có dấu không biểu diễn được (cần font `…H`) nên bị hạ thành chữ
  thường; ký tự ngoài bảng thành `?` (`unicode_to_tcvn3`).
- **Byte ngoài bảng TCVN3**: giữ nghĩa Windows-1252 (ASCII đi thẳng).
- **Ký tự private-use `U+F000..=U+F0FF`**: được quy về byte thấp, vì đó là cách Word lưu một số văn bản font
  symbol/font cũ (`char_to_byte`, ghi trong comment).
- **Heuristic phát hiện** — `detect_legacy_vietnamese`:
  - Trả `None` cho ASCII thuần, cho văn bản đã có ký tự ngoài Windows-1252 (vd Unicode tiếng Việt), hoặc khi bằng
    chứng yếu.
  - TCVN3 "mạnh": byte 0xA1–0xBF (ký hiệu giống dấu câu, không xuất hiện giữa từ trong văn bản phương Tây) và chữ
    hoa Latin-1 kẹp giữa hai chữ thường (`ViÖt`); cần thêm ít nhất một nửa ký tự không-ASCII nằm trong bảng TCVN3.
  - VNI: đếm cặp nguyên âm + byte dấu (`oä`, `aø`), chữ một byte và dấu "mồ côi"; chọn VNI khi số cặp ≥ số tín hiệu
    TCVN3 mạnh và dấu mồ côi không quá nhiều; VNI được xét trước TCVN3.

## 3. Quan sát black-box

- Chỉ có một ghi nhận trong comment: Word lưu một số văn bản font cũ dưới dạng ký tự private-use `U+F0xx`
  (`crates/bo-encoding/src/lib.rs` `char_to_byte`). Không có quan sát nào khác.

## 4. Chưa hỗ trợ

- Chỉ áp dụng khi mở `.doc`; không áp cho `.docx/.xls/.xlsx` dùng font cũ (`docs/REQUIREMENTS.md` 2.24,
  Khoảng trống #34).
- `convert_document` bỏ qua bảng và block opaque (comment trong code); chỉ xét font của run và font mặc định tài
  liệu, không xét font kế thừa từ style.
- `detect_legacy_vietnamese` chưa được gọi ngoài test của crate (grep `crates`): chuyển mã hiện chỉ dựa vào tên font.
- Không hỗ trợ các bảng mã cũ khác (VIQR, VPS, VISCII…) — không có trong `crates/bo-encoding/src`.
