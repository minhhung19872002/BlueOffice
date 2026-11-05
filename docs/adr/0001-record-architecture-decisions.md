# ADR-0001: Ghi lại quyết định kiến trúc

- Trạng thái: Chấp nhận · Ngày: 2026-11-05

## Bối cảnh
BlueOffice là dự án dài hạn, nhiều phase, nhiều người (và agent) tham gia. Prompt gốc §0 yêu cầu ghi lại
mọi quyết định kiến trúc quan trọng chưa có trong `BlueOffice-PROMPT.md`.

## Quyết định
Dùng Architecture Decision Record dạng Markdown trong `docs/adr/NNNN-ten-quyet-dinh.md`, gồm: bối cảnh,
quyết định, hệ quả, phương án đã loại. ADR không sửa sau khi chấp nhận; thay đổi bằng ADR mới "thay thế ADR-xxxx".

## Phương án đã loại
- Ghi quyết định trong issue/PR: khó tìm lại, không nằm cạnh code.
- Wiki riêng: lệch phiên bản với code.
