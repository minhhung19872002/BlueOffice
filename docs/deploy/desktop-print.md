# In, font hệ thống và lõi dự phòng của bản desktop

## In (prompt §3.4 `print`, P2.20)
Lệnh **In** (Ctrl+P) mở bảng In dùng chung cho ba ứng dụng (CORE-PRINT-001, `packages/editor/src/print/PrintPanel.tsx`):
chọn trang (tất cả / hiện tại / phần chọn / dãy `1-3, 5, 8-`, BlueDoc thêm `p2s1`), trang chẵn/lẻ, số bản, theo bộ,
in ngược, 1/2/4/6/8/16 trang mỗi tờ, co giãn theo khổ giấy, màu / thang xám / đen trắng; BlueDoc: in sửa đổi và
bình luận, trang thuộc tính; BlueSlide: trang chiếu / trang ghi chú / dàn ý / tài liệu phát tay, trang chiếu ẩn,
đóng khung; BlueSheet: trang tính hiện hành / cả sổ / vùng chọn, đường lưới, tiêu đề hàng cột, hàng/cột lặp lại,
bỏ qua vùng in. Lõi dựng PDF của lệnh in (`bo_engine::print`, `printPdf`) và bản xem trước vẽ đúng tờ thứ i của
PDF đó (`printPreview`). Lệnh **In ngay** (`file.printNow`) in cả tài liệu không qua bảng. Dòng lệnh:
`bo-cli convert in.docx --to pdf --pages "1-3" --per-sheet 4 --paper a4 [--print-options JSON]`.

PDF của lệnh in được gửi tới `print_pdf` (`apps/desktop/src-tauri/src/print.rs`):

| Hệ điều hành | Cách in |
|---|---|
| Windows | Mở cửa sổ in hiển thị PDF (giao thức nội bộ `boprint`, không ghi ra đĩa) và gọi lệnh in của WebView2: hộp thoại in có chọn máy in, trang, số bản, khổ giấy và mục "In bằng hộp thoại hệ thống" (Ctrl+Shift+P). Cửa sổ ở lại làm bản xem trước; đóng cửa sổ thì PDF được giải phóng. |
| macOS | Như Windows; WKWebView mở bảng in của hệ thống (NSPrintPanel). Chưa kiểm thử tự động (không có WebDriver cho WKWebView). |
| Linux | WebKitGTK không hiển thị PDF ổn định: PDF được ghi vào thư mục tạm và mở bằng trình xem mặc định (`xdg-open`), người dùng in từ trình xem đó (hộp thoại in GTK/CUPS). |

Nếu không tạo được cửa sổ in (lỗi web view), Windows/macOS cũng quay về cách của Linux.
Biến môi trường `BLUEOFFICE_PRINT_DIALOG=0` mở cửa sổ in mà không bật hộp thoại (E2E).

## Font hệ thống (prompt §3.4 `fonts`, P1.4, ADR-0003)
- Khi khởi động, một luồng nền liệt kê font đã cài (`%WINDIR%\Fonts`, `%LOCALAPPDATA%\Microsoft\Windows\Fonts`;
  `/System/Library/Fonts`, `/Library/Fonts`, `~/Library/Fonts`; `/usr/share/fonts`, `/usr/local/share/fonts`,
  `~/.local/share/fonts`, `~/.fonts`). Chỉ đọc bảng thư mục và bảng `name` của từng tệp (OpenType); danh sách được
  lưu đệm trong `<app cache>/system-fonts.txt` theo kích thước + thời gian sửa của tệp.
- Font hệ thống **nạp lười**: chỉ khi tài liệu dùng một họ font chưa có trong font đóng gói. Thứ tự tra cứu: font đóng
  gói → font hệ thống cùng tên → font thay thế tương thích số đo (Calibri → Carlito …). Face mới nhận FaceId **sau**
  các font đóng gói, nên thứ tự FaceId của font đóng gói (manifest, `bundled.rs`, `FONT_FILES`) không đổi.
- Hộp chọn font liệt kê cả các họ font hệ thống (lệnh `fontList`).
- Lần mở tài liệu đầu tiên chờ tối đa 3 giây cho việc liệt kê font.
- `BLUEOFFICE_SYSTEM_FONTS=0` tắt font hệ thống (hiển thị giống hệt bản web; dùng trong E2E so ảnh P1.10).

## Lõi dự phòng WASM (prompt §3.4 `engine`)
`FallbackEngineHost` (`packages/platform/src/desktop/index.ts`) dùng lõi native qua IPC; nếu `engine_init` lỗi hoặc
không có font, lõi WASM của bản web chạy trong Web Worker với font đóng gói trong `fonts/` của app, BlueSheet đi theo.
Thanh trạng thái hiển thị "Lõi: WASM".

## Đăng nhập người dùng BlueOffice (OIDC)
`DesktopAuth` dùng cấu hình `GET /api/auth/config` của máy chủ BlueOffice (Start → Cài đặt). Đăng nhập mở trình
duyệt hệ thống (OAuth 2.0 code + PKCE, RFC 8252) và quay lại app qua `bluedoc://auth/oidc`
(`bluesheet://auth/oidc`, `blueslide://auth/oidc`) — cần khai báo các redirect URI này cho client OIDC.
Token lưu theo từng máy chủ trong bộ nhớ của web view.

## Biến môi trường kiểm thử
| Biến | Tác dụng |
|---|---|
| `BLUEOFFICE_E2E_SAVE_DIR` | "Lưu thành" ghi thẳng vào thư mục này (không mở hộp thoại lưu của OS). Chỉ dùng cho E2E. |
| `BLUEOFFICE_PRINT_DIALOG=0` | Cửa sổ in không bật hộp thoại. |
| `BLUEOFFICE_SYSTEM_FONTS=0` | Chỉ dùng font đóng gói. |
| `BLUEOFFICE_UPDATE_URL` | Điểm cập nhật thay cho cấu hình (xem `desktop-update.md`). |
