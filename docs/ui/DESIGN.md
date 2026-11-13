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

- Font UI: Be Vietnam Pro / Inter (OFL), fallback font hệ thống. Desktop chọn theo hệ điều hành (`html[data-os]`,
  `FONT_UI_OS` trong `tokens.ts`): Windows dùng font hệ thống `Segoe UI Variable Text` (tiêu đề hộp thoại / màn hình Tệp:
  `--bo-font-display` = `Segoe UI Variable Display`) → `Segoe UI Variable` → `Segoe UI` (Windows 10) → Be Vietnam Pro → `system-ui`
  → `sans-serif`; macOS `-apple-system` (San Francisco) → `BlinkMacSystemFont` → `Helvetica Neue` → Be Vietnam Pro → `sans-serif`;
  Linux dùng Be Vietnam Pro / Inter như web, sau đó fallback font hệ thống. Font chữ tài liệu
  (canvas) không đổi (ADR-0003). Cỡ: 11 / 12 / 13 / 14 / 16 / 20 / 24 / 32 px; nhãn ribbon 12px,
  tiêu đề hộp thoại 16–20px.
- Lưới 4px; bo góc 4px (nút, ô nhập), 8px (hộp thoại, thẻ, popover); bóng 3 mức (menu, hộp thoại, cửa sổ nổi).
- Mật độ: Thường (mặc định), Gọn, Cảm ứng (vùng chạm ≥ 40px).
- Chuyển động 120–200ms, tôn trọng `prefers-reduced-motion`.

## Khung cửa sổ

Thanh tiêu đề 40px (icon app, truy cập nhanh, tên tài liệu + trạng thái lưu, ô Tìm kiếm Ctrl+Q / Alt+Q, người cùng sửa, Bình
luận, Chia sẻ, tài khoản) → hàng tab (Tệp + tab app + tab ngữ cảnh + chuyển chế độ ribbon) → thân ribbon (~92px đầy đủ, ~40px
đơn giản, tự ẩn) → vùng làm việc (ngăn trái tuỳ app, ngăn tác vụ phải dock/resize) → thanh trạng thái 24px.

Desktop Windows ([ADR-0016](../adr/0016-desktop-window-chrome.md)): cửa sổ không có khung hệ thống, thanh tiêu đề là caption —
kéo để di chuyển, nhấp đúp phóng to/khôi phục, chuột phải hoặc Alt+Space mở menu hệ thống, cuối thanh là ba nút cửa sổ 46 × 40px
(Thu nhỏ, Phóng to/Khôi phục, Đóng; Đóng chuyển đỏ `--bo-caption-close-bg` khi rê — quy ước của hệ điều hành). Rê nút Phóng to hiện
snap layouts. Windows 11: nền Mica sau thanh tiêu đề + ribbon (`html[data-backdrop='mica']`, chữ trung tính, màu nhấn ở icon app,
tab Tệp và tab đang chọn); Windows 10, tương phản cao, giảm trong suốt: dải màu nhấn như web. Màu của dải lấy từ token
`--bo-chrome-band-*` / `--bo-ribbon-bg` trong `theme.css`. Web giữ thanh tiêu đề trong trang, không có nút cửa sổ.

## Quyết định

Quyết định chưa rõ được chọn phương án khác biệt hơn và ghi `docs/adr/`. Việc cần người duyệt (logo chính thức, mã màu thương
hiệu, icon app) ghi `docs/parity/NEEDS_HUMAN.md`.
