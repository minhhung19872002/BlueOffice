# Hệ thống thiết kế giao diện BlueOffice

Tóm tắt từ [`BlueOffice-UI-PROMPT.md`](../../BlueOffice-UI-PROMPT.md). Mục tiêu: **quen tay như bộ office phổ biến, nhìn ra là
BlueOffice**. Danh sách việc: [`BACKLOG.md`](BACKLOG.md) · thuật ngữ: [`glossary.md`](glossary.md) · phím tắt:
[`shortcuts.md`](shortcuts.md).

## Ranh giới (bắt buộc)

- Được: ribbon chia tab, tab ngữ cảnh, thứ tự tab/nhóm theo thói quen, phím tắt tiêu chuẩn, KeyTips, menu chuột phải, màn hình
  Tệp toàn trang, thanh truy cập nhanh, thanh trạng thái, ngăn tác vụ, thiết kế phẳng.
- Cấm: icon/logo/hình/âm thanh của bên khác (kể cả vẽ lại sát); tên/nhãn hiệu bên khác (trừ câu mô tả tương thích định dạng);
  bảng màu nhận diện của app bên khác cho app tương ứng; chép tooltip, mô tả lệnh, tên style/theme/mẫu, gallery.
- Tài nguyên đồ họa: tự tạo hoặc MIT/Apache/ISC/OFL/CC0, ghi `THIRD_PARTY_NOTICES.md` và `packages/ui/icons/SOURCES.md`.

## Màu (`packages/ui/src/tokens`)

| Vai trò | Light | Ghi chú |
|---|---|---|
| Thương hiệu Bluestar (màn hình chung, BlueDrive) | `#1F6FEB` | tạm, chờ xác nhận từ logo (NEEDS_HUMAN) |
| BlueDoc — chàm | `#4F46E5` | |
| BlueSheet — xanh ngọc | `#0F9D8A` | chữ trên nền nhấn dùng bậc đậm `#0B7A6B` để đạt AA |
| BlueSlide — hồng san hô | `#E0457B` | chữ trên nền nhấn dùng bậc đậm `#C2185B` |

Không dùng tổ hợp "xanh dương = văn bản, xanh lá = bảng tính, cam/đỏ = trình chiếu". Màu nhấn chỉ cho: icon app, dải nhận diện
trên thanh tiêu đề, tab đang chọn, nút chính. Thang xám trung tính 12 bậc; màu trạng thái (thành công, cảnh báo, lỗi, thông tin)
3 bậc; ba chế độ light / dark / high-contrast. Biến CSS `--bo-*`, hằng TS trong `tokens.ts`.

## Chữ, khoảng cách, chuyển động

- Font UI: Be Vietnam Pro / Inter (OFL), fallback font hệ thống. Cỡ: 11 / 12 / 13 / 14 / 16 / 20 / 24 / 32 px; nhãn ribbon 12px,
  tiêu đề hộp thoại 16–20px.
- Lưới 4px; bo góc 4px (nút, ô nhập), 8px (hộp thoại, thẻ, popover); bóng 3 mức (menu, hộp thoại, cửa sổ nổi).
- Mật độ: Thường (mặc định), Gọn, Cảm ứng (vùng chạm ≥ 40px).
- Chuyển động 120–200ms, tôn trọng `prefers-reduced-motion`.

## Khung cửa sổ

Thanh tiêu đề 40px (icon app, truy cập nhanh, tên tài liệu + trạng thái lưu, ô Tìm kiếm Ctrl+Q / Alt+Q, người cùng sửa, Bình
luận, Chia sẻ, tài khoản) → hàng tab (Tệp + tab app + tab ngữ cảnh + chuyển chế độ ribbon) → thân ribbon (~92px đầy đủ, ~40px
đơn giản, tự ẩn) → vùng làm việc (ngăn trái tuỳ app, ngăn tác vụ phải dock/resize) → thanh trạng thái 24px.

## Quyết định

Quyết định chưa rõ được chọn phương án khác biệt hơn và ghi `docs/adr/`. Việc cần người duyệt (logo chính thức, mã màu thương
hiệu, icon app) ghi `docs/parity/NEEDS_HUMAN.md`.
