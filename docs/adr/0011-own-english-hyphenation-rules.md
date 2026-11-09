# ADR-0011: Gạch nối tự động bằng quy tắc tiếng Anh tự viết, không dùng tệp mẫu (pattern) bên thứ ba

- Trạng thái: Chấp nhận · Ngày: 2026-11-09
- Liên quan: parity DOC-LAYOUT-001; ECMA-376 Part 1 §17.15.1.10 autoHyphenation, §17.15.1.52 hyphenationZone,
  §17.15.1.21 consecutiveHyphenLimit, §17.15.1.40 doNotHyphenateCaps, §17.3.1.34 suppressAutoHyphens;
  `crates/bo-layout/src/hyphen.rs`.

## Bối cảnh
Gạch nối tự động cần biết chỗ được ngắt trong một từ. Cách phổ biến là thuật toán Liang với tệp mẫu TeX
(dự án hyph-utf8). Chính sách giấy phép của BlueOffice chỉ nhận MIT / Apache-2.0 / BSD / ISC / MPL-2.0 / Zlib / OFL
(CC0 / CC-BY cho dữ liệu). Các tệp mẫu tiếng Anh thông dụng không chắc nằm trong danh sách đó:
- `hyph-en-us` (mẫu gốc của Knuth/Liang) mang điều khoản riêng kiểu "chép tự do nhưng không được sửa mà giữ tên tệp" —
  không phải một giấy phép trong danh sách;
- `hyph-en-gb` theo LPPL — không nằm trong danh sách.
Không có mạng trong môi trường phát triển để kiểm chứng tiêu đề giấy phép của đúng tệp sẽ dùng, nên không đưa tệp nào vào.

## Quyết định
- Viết bộ quy tắc gạch nối tiếng Anh riêng, nhỏ (không dữ liệu ngoài, không ghi gì vào `THIRD_PARTY_NOTICES.md`):
  1. chỉ xét từ toàn chữ cái ASCII, dài ≥ 6; giữ ≥ 2 chữ trước và ≥ 3 chữ sau dấu gạch;
  2. tách hậu tố thường gặp (`-tion`, `-sion`, `-ment`, `-ness`, `-less`, `-ful`, `-able`, `-ible`, `-ship`, `-ing`;
     phụ âm đôi trước `-ing` tách giữa: run-ning) và vài tiền tố dài (`inter-`, `over-`, `under-`, `super-`, …);
  3. trong gốc từ: tìm nhóm nguyên âm (`y` sau phụ âm là nguyên âm, `u` sau `q` là phụ âm, `e` câm cuối không tính);
     một phụ âm giữa hai nhóm đi với âm tiết sau (V-CV; `x` ở lại âm tiết trước), hai phụ âm tách giữa trừ cặp đọc như
     một âm (`ch`, `sh`, `th`, `ph`, `wh`, `gh`; `ck` ở lại âm tiết trước), cụm dài hơn giữ lại phụ âm đầu hợp lệ
     (`st`, `dr`, `str`, …) cho âm tiết sau.
- Áp dụng khi dòng tràn: chọn điểm ngắt cuối cùng mà phần đầu + dấu gạch còn vừa dòng; không ngắt khi khoảng trống
  cuối dòng nhỏ hơn hyphenationZone (mặc định 0,25 in), khi đã đủ consecutiveHyphenLimit dòng gạch liên tiếp, với từ
  viết hoa toàn bộ khi doNotHyphenateCaps, hoặc đoạn có suppressAutoHyphens.
- Tiếng Việt viết theo âm tiết (có dấu) nên không bao giờ ngắt trong âm tiết — các từ có ký tự ngoài ASCII bị bỏ qua,
  văn bản tiếng Việt chỉ xuống dòng ở dấu cách.

## Hệ quả
- Không phụ thuộc giấy phép bên thứ ba; mã nhỏ, có test theo cách chia âm tiết của từ điển (in-for-ma-tion,
  es-tab-lish-ment, chil-dren, …).
- Kém chính xác hơn từ điển/mẫu đầy đủ: một số từ ngắt khác từ điển (vd. com-pu-ter thay vì com-put-er), các ngôn ngữ
  khác tiếng Anh không được gạch nối.
- Xem xét lại khi có một tệp mẫu tiếng Anh với giấy phép đã kiểm chứng nằm trong danh sách cho phép: thay `points()` bằng
  thuật toán Liang trên tệp đó, giữ nguyên phần áp dụng trong `paragraph.rs`.
