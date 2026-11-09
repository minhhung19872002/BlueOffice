# Phím tắt

Phím tắt khai báo cùng command trong registry (`packages/editor/src/commands.ts`, `packages/sheet-editor`,
`packages/slide-editor`). Cột "Trạng thái": **có** = đã gắn trong registry, **dự kiến** = sẽ gắn ở hạng mục UI tương ứng.
Trên macOS, Ctrl ⇒ ⌘. Người dùng cấu hình lại được ở Tùy chọn › Tùy biến phím tắt (UI-KEYS-002).

## Chung (cả ba app)

| Phím | Lệnh | Trạng thái |
|---|---|---|
| Ctrl+N | Tạo mới (`file.new`) | có (BlueDoc) |
| Ctrl+O | Mở (`file.open`) | có (BlueDoc) |
| Ctrl+S | Lưu (`file.save`) | có |
| Ctrl+Shift+S / F12 | Lưu thành (`file.saveAs`) | có / F12 dự kiến |
| Ctrl+P | In (`file.print`) | có |
| Ctrl+Shift+E | Xuất PDF (`file.exportPdf`) | có (BlueDoc) |
| Ctrl+Z / Ctrl+Y | Hoàn tác / Làm lại | có |
| Ctrl+X / Ctrl+C / Ctrl+V | Cắt / Sao chép / Dán | có |
| Ctrl+Alt+V | Dán đặc biệt | có |
| Ctrl+Shift+V | Dán văn bản thuần | dự kiến |
| Ctrl+A | Chọn tất cả | có |
| Ctrl+F / Ctrl+H | Tìm / Thay thế | có |
| Ctrl+B / Ctrl+I / Ctrl+U | Đậm / Nghiêng / Gạch chân | có |
| Ctrl+K | Chèn liên kết | dự kiến |
| Ctrl+Q / Alt+Q | Ô tìm lệnh | dự kiến (UI-FRAME-003) |
| Alt | Phím gợi ý (KeyTips) | dự kiến (UI-KEYTIPS-001) |
| F6 / Shift+F6 | Chuyển vùng: dải lệnh → tài liệu → ngăn tác vụ → thanh trạng thái | dự kiến (UI-A11Y-002) |
| F7 | Chính tả & ngữ pháp | dự kiến |
| Ctrl+F1 | Thu/mở dải lệnh | dự kiến (UI-RIBBON-003) |
| Ctrl+Shift+= / Ctrl+- / Ctrl+0 | Phóng to / Thu nhỏ / 100% | có (BlueDoc) |
| Escape | Thoát chế độ hiện tại | có |

## BlueDoc

| Phím | Lệnh | Trạng thái |
|---|---|---|
| Ctrl+] / Ctrl+[ | Tăng / giảm cỡ chữ | có |
| Ctrl+Shift+C / Ctrl+Shift+V | Sao chép / dán định dạng | có |
| Ctrl+Enter | Ngắt trang | có |
| Alt+= | Chèn phương trình | có |
| F9 | Cập nhật trường | có |
| Alt+F9 | Bật/tắt mã trường | có |
| Ctrl+F11 / Ctrl+Shift+F11 | Khóa / mở khóa trường | có |
| Ctrl+Shift+F9 | Bỏ liên kết trường | có |
| Ctrl+Shift+R | Theo dõi thay đổi | có |
| Ctrl+Alt+M | Bình luận mới | có |
| Ctrl+Alt+F / Ctrl+Alt+D | Chú thích chân trang / cuối văn bản | có |
| Ctrl+E / Ctrl+L / Ctrl+R / Ctrl+J | Căn giữa / trái / phải / đều | dự kiến |
| Ctrl+= / Ctrl+Shift+= | Chỉ số dưới / trên | dự kiến (Ctrl+Shift+= hiện là phóng to; chuyển phóng to sang Ctrl+Alt+=) |
| Shift+F3 | Đổi chữ hoa/thường | dự kiến |
| Ctrl+Space | Xóa định dạng ký tự | dự kiến |
| Ctrl+1 / Ctrl+2 / Ctrl+5 | Giãn dòng 1 / 2 / 1,5 | dự kiến |

## BlueSheet

| Phím | Lệnh | Trạng thái |
|---|---|---|
| Ctrl+D / Ctrl+R | Điền xuống / sang phải | có |
| Ctrl+G / F5 | Đi tới | có / F5 dự kiến |
| Ctrl+5 | Gạch ngang | có |
| Ctrl+Shift+% | Định dạng phần trăm | có |
| Ctrl+1 | Định dạng ô | có |
| Ctrl+Shift+L | Bật/tắt lọc | có |
| F2 | Sửa ô | có (lưới) |
| F4 | Đổi kiểu tham chiếu tuyệt đối/tương đối khi sửa công thức | dự kiến |
| Alt+= | Tổng tự động | dự kiến |
| Shift+F3 | Chèn hàm | dự kiến |
| F9 / Shift+F9 | Tính ngay / tính trang tính | dự kiến |
| Ctrl+` | Hiện công thức | dự kiến |
| Ctrl+T | Tạo bảng | dự kiến |

## BlueSlide

| Phím | Lệnh | Trạng thái |
|---|---|---|
| Ctrl+M | Trang chiếu mới | dự kiến |
| Ctrl+D | Nhân bản đối tượng / trang chiếu | dự kiến |
| F5 / Shift+F5 | Trình chiếu từ đầu / từ trang hiện tại | dự kiến |
| Ctrl+G / Ctrl+Shift+G | Nhóm / bỏ nhóm | dự kiến |
| Alt+F5 | Dạng xem người thuyết trình | dự kiến |
