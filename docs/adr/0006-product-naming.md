# ADR-0006: Đặt tên sản phẩm BlueDoc / BlueSheet / BlueSlide

- Trạng thái: Chấp nhận · Ngày: 2026-11-05

## Quyết định
- Họ sản phẩm: BlueDrive · BlueDoc · BlueSheet · BlueSlide (sau: BlueForm, BlueNote) — không có "s" ở cuối.
- Route web `/doc`, `/sheet`, `/slide`; route cũ `/write`, `/docs`, `/sheets`, `/slides` chuyển hướng (`apps/web/src/routes.ts`).
- Desktop: một vỏ Tauri dùng chung, ba cấu hình sản phẩm — `tauri.conf.json` (BlueDoc, `vn.bluestar.bluedoc`, `.docx`),
  `tauri.bluesheet.conf.json` (`vn.bluestar.bluesheet`, `.xlsx`), `tauri.blueslide.conf.json` (`vn.bluestar.blueslide`, `.pptx`);
  cửa sổ mở `index.html?app=doc|sheet|slide`. Binary: `bluedoc`.
- Deep link theo app: `bluedoc://`, `bluesheet://`, `blueslide://` (đăng ký khi cài plugin deep-link ở Phase 6A).
- Không dùng tên/nhãn hiệu/icon/màu của Microsoft hoặc Google cho sản phẩm hay tính năng; chỉ mô tả tương thích
  ("Tương thích định dạng Microsoft Word (.docx)").

## Phương án đã loại
- BlueWrite / BlueDocs / BlueSheets / BlueSlides: số nhiều dễ nhầm với sản phẩm của bên thứ ba.
- Ba vỏ desktop riêng biệt: trùng lặp code, trái yêu cầu "một sản phẩm".
