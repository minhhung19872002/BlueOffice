# OPC (Open Packaging Conventions) — ghi chú đọc spec

Nguồn: ECMA-376 Part 2 (OPC); Part 1 chỉ cho các relationship type. Ghi chú diễn giải lại theo cách hiểu của nhóm,
không chép văn bản spec; chỉ ghi số mục. Mã: `crates/bo-opc`.

## 1. Phạm vi đã cài

- **Mô hình package, ánh xạ ZIP** — Part 2 §8, Annex C: `crates/bo-opc/src/package.rs` (`Package`, `Part`).
  - Mọi part, kể cả part không hiểu, được giữ dưới dạng byte thô và ghi lại theo đúng thứ tự gốc
    (`crates/bo-opc/src/lib.rs`, test `roundtrip_preserves_all_parts`, proptest `arbitrary_parts_roundtrip`
    trong `crates/bo-opc/tests/proptest_roundtrip.rs`).
  - `[Content_Types].xml` được lưu như một entry thường (`CONTENT_TYPES = "/[Content_Types].xml"`); khi ghi nó luôn
    là entry đầu tiên (§C.3, phục vụ đọc kiểu streaming). Thiếu stream này thì `Package::read` trả lỗi
    (test `requires_content_types`).
  - Nén khi ghi: Deflate cho mọi part.
- **Content types** — §10.1.2: `crates/bo-opc/src/content_types.rs`.
  - Đọc `Default` (§10.1.2.2.2, khoá theo phần mở rộng, lưu chữ thường) và `Override` (§10.1.2.2.3).
  - Tra cứu: Override thắng Default; so sánh tên không phân biệt hoa thường ASCII (§9.1.1.1).
  - `Package::add_part` chỉ thêm Override khi Default theo phần mở rộng không cho đúng content type.
- **Tên part** — §9.1.1.1: `Package::part` / `put_part` / `remove_part` so khớp không phân biệt hoa thường ASCII.
- **Relationships** — §9.3: `crates/bo-opc/src/rels.rs`.
  - Tên part `.rels` của một nguồn: `/thư-mục/_rels/tên.rels` (§9.3.3.1); nguồn `"/"` là package.
  - Phân giải target tương đối theo thư mục của part nguồn, chuẩn hoá `.`/`..` (§9.3.4, RFC 3986) — `resolve_target`.
  - `TargetMode` (§9.3.2.2): chỉ giá trị `External` được nhận là ngoài; mọi giá trị khác/thiếu → `Internal`.
    Khi ghi chỉ xuất thuộc tính `TargetMode="External"` cho quan hệ ngoài.
  - `Package::related_part` chỉ trả quan hệ **Internal**.
- **Relationship type dùng chung** — Part 1 §15.2.12.1 / §11.3.12: `REL_OFFICE_DOCUMENT`, `REL_STYLES`
  trong `crates/bo-opc/src/lib.rs`.
- **Helper XML** — `crates/bo-opc/src/xml.rs`: `XmlReader` (pull reader trên quick-xml, không trim text, có `pos()`/
  `slice()` để chụp XML gốc phục vụ round-trip), `escape`, `attr` (bỏ qua prefix), `unescape`.

## 2. Điểm mơ hồ & cách đã chọn

- **Phân biệt hoa thường tên part**: spec coi tên part tương đương khi chỉ khác hoa/thường ASCII; cài đặt so khớp
  `eq_ignore_ascii_case` ở mọi chỗ tra cứu, còn khi ghi giữ nguyên chính tả gốc của part (`package.rs`).
- **Kiểm tra tên part khi đọc** chỉ ở mức tối thiểu: thêm `/` đầu, từ chối tên có `\` hoặc đoạn `..`
  (`InvalidPartName`); không kiểm đầy đủ ngữ pháp tên part của §9.1.1.
- **Giá trị `TargetMode` lạ** được coi như Internal (xem trên) thay vì báo lỗi.
- **Relationship thiếu `Id`/`Type`/`Target`** → lỗi đọc part `.rels` (không bỏ qua im lặng) — `rels::parse`.
- **Thực thể XML**: `unescape` chỉ giải 5 thực thể định sẵn và tham chiếu ký tự số; thực thể khác giữ nguyên dạng
  chữ `&…`, không mở rộng.

## 3. An toàn với dữ liệu không tin cậy (prompt §5.9)

- `Limits` mặc định (`package.rs`): tối đa 20 000 entry; 512 MiB mỗi part; 2 GiB tổng; tỉ lệ nén tối đa 200 cho
  part > 1 MiB. Kích thước khai báo trong ZIP không được tin: đọc thật bị chặn ở `max_part_size + 1` byte
  (test `rejects_zip_bomb`, `rejects_garbage`).
- `xml::MAX_DEPTH = 256`: `XmlReader::read` báo lỗi khi lồng sâu hơn (test `depth_limit`).
- Quan hệ External không bao giờ được tải: `crates/bo-docx/src/reader.rs` `Ctx::target` trả `None` cho External;
  hyperlink chỉ lấy chuỗi URL qua `Ctx::external` (ghi nhận trong `docs/REQUIREMENTS.md` mục 5.9).

## 4. Round-trip lossless (ADR-0005)

- Xem `docs/adr/0005-lossless-docx-round-trip.md`. Tóm tắt: reader giữ XML gốc của từng block/style/sectPr/ảnh kèm
  fingerprint FNV-1a; writer xuất nguyên văn khi fingerprint không đổi, chỉ sinh lại block bị sửa; part ngoài tập
  sửa đổi do bo-opc chép nguyên byte.
- Kiểm chứng: `crates/bo-docx/tests/corpus.rs` `corpus_round_trips_losslessly` (so sánh part sau chuẩn hoá XML).

## 5. Quan sát black-box

- Không có quan sát black-box nào được ghi trong code/test của `bo-opc`.

## 6. Chưa hỗ trợ

- Không có xử lý riêng cho core properties, digital signature part (ký số nằm ở `bo-sign`), ZIP chia mảnh
  (piece/interleaving) hay mã hoá phần tên part (percent-encoding/IRI) trong `crates/bo-opc/src` (grep không thấy).
- Kiểm tra ngữ pháp tên part chưa đầy đủ (mục 2).
- Nhánh `rejects ..` / `\` trong tên part chưa có test riêng.
- Fuzz `cargo fuzz` target `opc` có nhưng không chạy trên Windows MSVC / CI (`docs/REQUIREMENTS.md` mục 7.5);
  thay bằng proptest `random_bytes_never_panic`.
