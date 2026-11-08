# ODF (OpenDocument 1.3) — ghi chú đọc spec

Nguồn: OASIS ODF 1.3 Part 2 (Packages), Part 3 (Schema); Part 1 Appendix C cho mimetype. Chỉ ghi số mục, nội dung
diễn giải. Mã: `crates/bo-odf`.

## 1. Phạm vi đã cài

- **Package** — Part 2: `crates/bo-odf/src/package.rs` (`OdfPackage`).
  - Ghi: entry `mimetype` đầu tiên, lưu không nén, không extra field (Part 2 §3.3; test `mimetype_first_and_stored`
    kiểm trực tiếp header ZIP); các entry khác Deflate; `META-INF/manifest.xml` (Part 2 §4) liệt kê `/` và mọi file,
    `manifest:version="1.3"`.
  - `meta.xml` chỉ có `meta:generator` (Part 3 §3.1.3) — `meta_xml`.
  - Mimetype: `MIME_ODT/ODS/ODP` trong `crates/bo-odf/src/lib.rs` (Part 1 Appendix C); `detect_mimetype`.
- **Cây XML** — `crates/bo-odf/src/dom.rs`: cây đã phân giải namespace (quick-xml `NsReader`), namespace ở
  `lib.rs` `ns` (Part 3 §1.5 / Part 2 §4).
- **.odt đọc** — `crates/bo-odf/src/odt_read.rs` → `bo_model::Document`:
  - Style §16.2 `style:style` + chuỗi `style:parent-style-name`; font-face §16.23; master page → page layout.
  - `style:paragraph-properties` §17.6: §20.216 text-align, §20.204 line-height / §20.317 line-height-at-least,
    lề, thụt, khoảng cách, §20.186 break-before, §20.183/§20.191 break-after.
  - `style:text-properties` §17.10: §20.190 font-weight, §20.188 font-style, §20.187 font-size, §20.282 font-name /
    §20.185 font-family, §20.389/§20.390 gạch chân, §20.375 gạch ngang, §20.180 color.
  - `style:page-layout-properties` §17.2; độ dài §18.3.18 (`crates/bo-odf/src/units.rs`: cm, mm, in, pt, pc, px).
  - Nội dung: §5.1.3 text:p, §5.1.2 text:h, §6.1.3 text:s, §6.1.4 text:tab, §6.1.5 text:line-break, §6.1.7 text:span;
    list/section/index/table: đi xuống lấy đoạn.
- **.odt ghi** — `crates/bo-odf/src/odt_write.rs`: content.xml (§3.1.3.2), styles.xml (§3.1.3.3), automatic style
  cho đoạn/span, font-face-decls.
- **.ods** — `crates/bo-odf/src/ods.rs`: bảng §9.1, kiểu giá trị §19.385 `office:value-type` (float, percentage,
  currency, date, time, boolean, string); `number-rows-repeated` / `number-columns-repeated`; `covered-table-cell`;
  `table:formula` giữ nguyên chuỗi (vd `of:=SUM(...)`).
- **.odp** — `crates/bo-odf/src/odp.rs`: §10.1 draw:page, §10.3.15 draw:g, §10.4.2 draw:frame → §10.4.3 draw:text-box,
  §19.392 presentation:class, §16.33 presentation:notes; ghi page layout §16.5, master page §16.9.

## 2. Điểm mơ hồ & cách đã chọn

- **Khoảng trắng** (Part 3 §6.1.2): đọc gộp chuỗi khoảng trắng thành một dấu cách, bỏ khoảng trắng cuối;
  `text:s` không bị gộp và không gộp cái theo sau. Writer mã hoá khoảng trắng (`text:s`, `text:tab`) sao cho reader
  áp quy tắc gộp thu lại đúng văn bản (`odt_write.rs`, `ods.rs`).
- **text-align**: `start`/`left` → trái, `end`/`right` → phải (không xét hướng viết).
- **line-height**: `normal` → đơn (240); phần trăm → bội số dòng; độ dài tuyệt đối → Exact.
- **font-size**: chỉ nhận độ dài tuyệt đối (bỏ phần trăm). `px` tính theo 96 dpi.
- **Gạch chân khi ghi**: chỉ phân biệt none / double; mọi kiểu khác ghi thành một nét liền.
- **Mimetype**: chấp nhận cả biến thể có hậu tố `-…` (vd mẫu) ngoài giá trị đúng; thiếu entry `mimetype` không lỗi
  (`expect_mimetype`).
- **Giới hạn**: ODF không đặt trần hàng/cột; chọn 2^20 hàng, 2^14 cột, tối đa 2 000 000 ô sau khi mở rộng repeat
  (`ods.rs`). Chuỗi kế thừa style tối đa 32 (`odt_read.rs` `MAX_STYLE_DEPTH`).
- **An toàn** (prompt §5.9): `package.rs` tối đa 20 000 entry, 256 MiB/entry, 1 GiB tổng, tỉ lệ nén 200 cho entry
  > 1 MiB; `dom.rs` độ sâu 256, tối đa 20 000 000 node; proptest `crates/bo-odf/tests/fuzz.rs`.
- **.odp ghi**: một master page, khổ 16:9 28 cm × 15,75 cm, các frame xếp dọc; round-trip lần hai là điểm cố định
  (`odp.rs` test).

## 3. Quan sát black-box

- Không có quan sát black-box nào được ghi trong code/test của `bo-odf`.

## 4. Chưa hỗ trợ

- `.odt`: ghi chú, annotation, bookmark, dấu thay đổi không vào thân văn bản; frame/text box trong đoạn bị bỏ qua;
  field (text:date, text:page-number…) chỉ lấy nội dung hiển thị (`odt_read.rs`). Writer bỏ object không phải chữ
  (ảnh, bảng…) trừ ngắt (`odt_write.rs`).
- Editor không lưu ODF (chỉ `bo-cli convert --to odt|ods|odp`) — `docs/REQUIREMENTS.md` 2.3, P5.13.
- `.ods`: BlueSheet mở được nhưng `from_ods` (`crates/bo-engine/src/sheet.rs`) chỉ lấy giá trị — công thức bị bỏ,
  ngày/giờ thành chữ; ghi `.ods` chỉ giá trị (2.8).
- `.odp`: chỉ khung chữ và ghi chú; BlueSlide không mở `.odp` (2.11, Khoảng trống #11).
- Không có mã hoá ODF (manifest encryption), chữ ký ODF; reader không kiểm `..` trong tên entry như `bo-opc`
  (grep `crates/bo-odf/src`).
