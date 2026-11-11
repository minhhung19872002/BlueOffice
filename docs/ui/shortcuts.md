# Phím tắt

Phím tắt khai báo cùng command trong registry (`packages/editor/src/commands.ts`, `packages/sheet-editor`,
`packages/slide-editor`). Cột "Trạng thái": **có** = đã gắn trong registry, **dự kiến** = sẽ gắn ở hạng mục UI tương ứng.
Trên macOS, Ctrl ⇒ ⌘. Người dùng cấu hình lại được ở Tệp › Tùy chọn › Tùy biến phím tắt (UI-KEYS-002, CORE-UI-004):
phím người dùng đè lên cột dưới đây, lưu theo app, xuất/nhập JSON.

## Chung (cả ba app)

| Phím | Lệnh | Trạng thái |
|---|---|---|
| Ctrl+N | Tạo mới (`file.new`) | có (BlueDoc) |
| Ctrl+O | Mở (`file.open`) | có (BlueDoc) |
| Ctrl+S | Lưu (`file.save`) | có |
| Ctrl+Shift+S / F12 | Lưu thành (`file.saveAs`) | có; BlueDoc: F12 (như trình soạn thảo tham chiếu) |
| Ctrl+P | In (`file.print`) | có |
| Ctrl+Shift+E | BlueDoc: Theo dõi thay đổi (Xuất PDF không còn phím tắt) | có |
| Ctrl+Z / Ctrl+Y | Hoàn tác / Làm lại | có |
| Ctrl+X / Ctrl+C / Ctrl+V | Cắt / Sao chép / Dán | có |
| Ctrl+Alt+V | Dán đặc biệt | có |
| Ctrl+Shift+V | Dán văn bản thuần | dự kiến |
| Ctrl+A | Chọn tất cả | có |
| Ctrl+F / Ctrl+H | Tìm / Thay thế | có |
| Ctrl+B / Ctrl+I / Ctrl+U | Đậm / Nghiêng / Gạch chân | có |
| Ctrl+K | Chèn liên kết | có (BlueDoc, BlueSlide) |
| Ctrl+Q / Alt+Q | Ô tìm lệnh (gần đây, liên quan); BlueDoc chỉ Alt+Q (Ctrl+Q: xoá định dạng đoạn) | có |
| Alt (nhấn rồi thả) / F10 | Phím gợi ý (KeyTips): chữ trên tab → chữ trên lệnh; Esc lùi một bậc | có |
| Shift+F10 / phím menu | Menu ngữ cảnh tại con trỏ soạn thảo / vùng chọn | có |
| F4 | Lặp lại thao tác cuối (`edit.repeat`; BlueSheet khi sửa công thức: đổi kiểu tham chiếu) | có |
| F6 / Shift+F6 | Chuyển vùng: dải lệnh → tài liệu → ngăn tác vụ → thanh trạng thái | dự kiến (UI-A11Y-002) |
| F7 | Chính tả & ngữ pháp | dự kiến |
| Ctrl+F1 | Thu/mở dải lệnh | có (UI-RIBBON-003; ribbon tự ẩn: Alt hiện lại) |
| Ctrl+Alt+= / Ctrl+Alt+- / Ctrl+Alt+0 | Phóng to / Thu nhỏ / 100% (BlueDoc; Ctrl+lăn chuột cũng thu phóng) | có |
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
| Ctrl+Shift+E | Theo dõi thay đổi | có |
| Ctrl+Alt+M | Bình luận mới | có |
| Ctrl+Alt+F / Ctrl+Alt+D | Chú thích chân trang / cuối văn bản | có |
| Ctrl+Shift+8 | Bật/tắt ký hiệu định dạng ¶ · → (`view.formattingMarks`) | có |
| Alt+Shift+← / Alt+Shift+→ | Nâng / hạ cấp đề mục (`outline.promote` / `outline.demote`) | có |
| Alt+Shift+↑ / Alt+Shift+↓ | Đưa đề mục cùng nội dung con lên / xuống (`outline.moveUp` / `outline.moveDown`) | có |
| Tab / Shift+Tab (dạng xem dàn ý) | Hạ / nâng cấp đoạn đang chọn | có |
| ← → / PageUp PageDown / Space, Home / End (chế độ đọc) | Lật màn hình, về đầu / cuối | có |
| Escape (chế độ đọc, tập trung) | Thoát về dạng xem trước | có |
| Ctrl+E / Ctrl+L / Ctrl+R / Ctrl+J | Căn giữa / trái / phải / đều | có |
| Ctrl+= / Ctrl+Shift+= | Chỉ số dưới / trên | có |
| Ctrl+Shift+> / Ctrl+Shift+< | Cỡ chữ kế tiếp / trước trong danh sách | có |
| Shift+F3 | Đổi chữ hoa/thường | có |
| Ctrl+Space / Ctrl+Q | Xóa định dạng ký tự / đoạn | có |
| Ctrl+D | Hộp thoại phông chữ | có |
| Ctrl+1 / Ctrl+2 / Ctrl+5 / Ctrl+0 | Giãn dòng 1 / 2 / 1,5; ±12 pt trước đoạn | có |
| Ctrl+M / Ctrl+Shift+M, Ctrl+T / Ctrl+Shift+T | Thụt lề / thụt treo | có |
| Ctrl+Alt+3, Ctrl+Shift+L | Đề mục 3, danh sách | có |
| Ctrl+Backspace / Ctrl+Delete, Ctrl+↑ / Ctrl+↓ | Xoá từ; về đoạn trước / sau | có |
| Alt+Shift+D / T / P, Ctrl+Shift+Space | Trường ngày / giờ / trang; dấu cách không ngắt | có |
| Ctrl+nháy | Chọn câu; trên liên kết: mở liên kết | có |

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
| F4 | Khi sửa công thức: tham chiếu tại con trỏ A1 → $A$1 → A$1 → $A1 → A1 (cả hai đầu vùng); ngoài ra lặp lại định dạng / chèn-xoá hàng cột cuối | có |
| Alt+= | Tự tính tổng (`formula.autoSum`): vùng số → tổng ở ô trống ngay dưới / bên phải; một ô trống → đề xuất `=SUM(…)` để sửa rồi Enter | có |
| Shift+F3 | Chèn hàm (`formula.insertFunction`) | có |
| Ctrl+Shift+T | Bật/tắt hàng tổng của bảng (`tableDesign.totalsRow`, khi ô đang chọn nằm trong bảng) | có |
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
