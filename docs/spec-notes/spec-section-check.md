# Đối chiếu số mục spec (CORE-MISC-004, 2026-11-11)

Các số mục dưới đây trước đây do agent suy luận; nay đối chiếu với mục lục công khai của
[MS-OI29500] (Office Implementation Information for ISO/IEC 29500), mỗi mục ghi chú của
Microsoft mang đúng tiêu đề "Part N Section x.y.z, TÊN" của ISO/IEC 29500 (cùng số mục với
ECMA-376 bản 2 trở đi). Bản ECMA-376 đầy đủ không tải được trong phiên này, nên số mục chỉ
xác minh được cho các phần tử/trường mà [MS-OI29500] có ghi chú; các số còn lại (`ASK`
§17.16.5.3, …) vẫn là suy luận theo thứ tự chữ cái và không được dùng thêm.

Nguồn:
- Mục lục [MS-OI29500]: `https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/toc.json`
- 2.1.215 Part 1 Section 17.6.8, lnNumType:
  `https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/c4252e13-48d2-4ac9-9fb5-3b276d02be30`
- 2.1.480 Part 1 Section 17.16.5.29, INDEX:
  `https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/99860875-33d5-46d6-8d52-76e0cdcd5da3`
- 2.1.505 Part 1 Section 17.16.5.62, TA:
  `https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/7fe21147-206c-484f-9a5d-ccb42e2b75d1`

## 1. `w:lnNumType/@w:start` (Part 1 §17.6.8)

- Chuẩn: `start` là giá trị bắt đầu khi đánh số (lại).
- [MS-OI29500] 2.1.215 b: Office coi `start` là **số lượng số bị bỏ qua** mỗi lần bắt đầu /
  bắt đầu lại ("a value of 3 means to start numbering at 4"); a: `start` trong 0..=32767;
  c: `countBy` trong 1..=100.
- Quyết định: giữ cách hiển thị `start + 1` (tương thích tệp do các trình soạn thảo phổ biến
  ghi), áp dụng cả khi bắt đầu lại theo trang/section; đọc kẹp `start` vào 0..=32767 (trước chỉ
  chặn dưới). Test: `bo-layout/tests/page_layout.rs`
  `line_numbers_start_skips_numbers_at_every_restart`, `bo-docx/tests/page_layout.rs`
  `line_numbering_values_are_kept_in_range`.

## 2. Trường (Part 1 §17.16.5.x)

| Trường | Số đúng | Trước đây ghi | Ghi chú |
|---|---|---|---|
| INDEX | §17.16.5.29 | §17.16.5.28 | .28 là INCLUDETEXT |
| TA | §17.16.5.62 | §17.16.5.62 | đúng |
| TOA | §17.16.5.67 | §17.16.5.67 | đúng |
| XE | §17.16.5.72 | §17.16.5.72 | đúng |
| TOC | §17.16.5.68 | §17.16.5.68 | đúng |
| SEQ | §17.16.5.56 | §17.16.5.56 | đúng |
| REF | §17.16.5.51 | §17.16.5.49 | .49 là QUOTE |
| PAGEREF | §17.16.5.45 | §17.16.5.42 | .42 là NUMPAGES |
| PAGE | §17.16.5.44 | §17.16.5.43 | suy ra: NUMPAGES .42 → NUMWORDS → PAGE → PAGEREF .45 |
| NUMPAGES | §17.16.5.42 | §17.16.5.40 / .41 / .38 | .40 là NOTEREF |
| IF | §17.16.5.26 | §17.16.5.24 | .24 là GREETINGLINE |
| BIBLIOGRAPHY | §17.16.5.7 | §17.16.5.5 | |
| CITATION | §17.16.5.8 | §17.16.5.8 | đúng |
| MERGEFIELD, NEXTIF, SKIPIF, LINK | .35, .39, .58, .32 | như vậy | đúng |
| MERGEREC, MERGESEQ, NEXT, SET | .36, .37, .38, .57 | như vậy | không có ghi chú riêng; nằm đúng khoảng trống giữa các số đã xác minh |
| FORMCHECKBOX, FORMDROPDOWN, FORMTEXT | .20, .21, .22 | .19 / .20 / .21 lệch | .20, .21 xác minh; FORMTEXT suy ra (.23 là GOTOBUTTON) |
| `=` (công thức) | §17.16.3 (cú pháp công thức: toán tử §17.16.3.3, tham chiếu ô §17.16.3.5) | §17.16.5.1 | .1 là ADDRESSBLOCK |

Đã sửa các chú thích trong `crates/` và `packages/` (chỉ comment, không đổi hành vi).

## 3. `o:signatureline` (Part 4 §19.2.2.30)

[MS-OI29500] 2.1.1841 "Part 4 Section 19.2.2.30, signatureline (Digital Signature Line)" — đúng
như các chú thích hiện có (VML Office drawing, Part 4). Cùng mục lục xác nhận §19.1.2.11
imagedata (Part 4) dùng trong `bo-docx/src/writer.rs`.
