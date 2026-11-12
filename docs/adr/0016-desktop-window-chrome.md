# ADR-0016: Khung cửa sổ desktop — thanh tiêu đề tự vẽ, snap layouts, Mica, font hệ thống

- Trạng thái: Chấp nhận · Ngày: 2026-11-12
- Liên quan: `BlueOffice-UI-PROMPT.md` §5 + §12 ("trên desktop dùng thanh tiêu đề tùy biến tích hợp nút cửa sổ hệ điều
  hành"); `crates/bo-winchrome`, `apps/desktop/src-tauri/src/chrome_cmds.rs`, `packages/platform/src/desktop/chrome.ts`,
  `packages/ui/src/TitleBar.tsx`, `packages/ui/src/theme.css`; ADR-0003 (font tài liệu), ADR-0006 (tên sản phẩm).

## Bối cảnh
Bản desktop (Tauri 2) có khung cửa sổ của hệ điều hành *và* thanh tiêu đề 40px của app ngay bên dưới: hai thanh tiêu đề
chồng nhau, tốn 30px, không giống ứng dụng Windows 11 hiện đại. Chủ dự án yêu cầu: thanh tiêu đề của app chính là caption
của cửa sổ (kéo, nhấp đúp, nút cửa sổ, menu hệ thống), snap layouts khi rê nút phóng to, nền Mica sau thanh tiêu đề + ribbon,
font giao diện của hệ điều hành. Ràng buộc: `#![forbid(unsafe_code)]`, chỉ thư viện MIT/Apache/BSD…, WCAG 2.2 AA, giao diện
đi qua `Platform`, web giữ nguyên.

## Quyết định
1. **Không khung hệ thống trên Windows.** Cả bốn cấu hình Tauri (`tauri.conf.json`, `tauri.bluesheet.conf.json`,
   `tauri.blueslide.conf.json`, `tauri.webdriver.conf.json`) và cửa sổ tài liệu mở thêm (`open_window`) đặt
   `decorations: false`, `transparent: true` (để Mica nhìn xuyên qua web view). Viền kéo giãn, bóng và góc bo Windows 11 do
   Tauri lo (`undecorated_resizing`: cửa sổ con mảnh trên mép trên + `shadow` mặc định). macOS và Linux bật lại khung native
   lúc khởi động (`set_decorations(true)`): chưa có máy kiểm thử, nút đèn giao thông / nút GTK của hệ thống tốt hơn bản tự vẽ.
2. **Thanh tiêu đề là caption** (`TitleBar` nhận `window`, chỉ có trên desktop qua `platform.window.chrome`): nhấn chuột
   trái vào phần không tương tác → `startDragging`, nhấp đúp → phóng to/khôi phục, chuột phải hoặc Alt+Space → menu hệ thống
   thật của cửa sổ (`GetSystemMenu` + `TrackPopupMenu`, mục bật/tắt theo trạng thái, lệnh gửi `WM_SYSCOMMAND`). Ba nút
   Thu nhỏ / Phóng to–Khôi phục / Đóng 46 × 40px (cao bằng thanh, sát mép phải để mép màn hình của cửa sổ phóng to vẫn bấm
   được), là `<button>` thật trong thứ tự Tab, có `aria-label` + tooltip; Đóng đỏ `#C42B1C` khi rê (quy ước của hệ điều hành,
   không phải màu thương hiệu). Biểu tượng nút là hình hình học tự vẽ (`window.*` trong `icons/custom.tsx`), không dùng font
   biểu tượng của hệ điều hành. Màn hình Bắt đầu của desktop cũng có caption (`StartScreen` `caption`).
3. **Snap layouts bằng cửa sổ con phủ nút phóng to** (`crates/bo-winchrome`). Windows 11 chỉ hiện bảng snap khi cửa sổ dưới
   con trỏ trả `HTMAXBUTTON` cho `WM_NCHITTEST`; vùng client là web view (cửa sổ con của WebView2) nên không thể trả lời từ
   trang. Web báo hình chữ nhật của nút (CSS px + `devicePixelRatio`, mỗi khi bố cục đổi) → `chrome_set_max_button` →
   `to_physical` (làm tròn ra ngoài, mép trên < 1px dính về 0, cắt theo vùng client; có test) → một cửa sổ con
   `WS_EX_LAYERED | WS_EX_NOREDIRECTIONBITMAP` (không vẽ gì, vẫn được hit-test) nằm trên web view đúng chỗ nút, trả
   `HTMAXBUTTON`, không lấy focus (`MA_NOACTIVATE`), bấm thì `SC_MAXIMIZE` / `SC_RESTORE`, rê / nhấn thì gửi sự kiện
   `bo-chrome:max-button` để nút web vẽ trạng thái hover / pressed. Bàn phím vẫn đi vào nút web. Đây là cùng kỹ thuật Tauri
   dùng cho viền kéo giãn của cửa sổ không khung (và Windows Terminal cho thanh kéo), không subclass HWND của Tauri.
   **Ngoại lệ `unsafe` duy nhất**: `bo-winchrome` đặt `unsafe_code = "deny"` thay cho `forbid` của workspace, `unsafe` chỉ nằm
   trong `src/win32.rs` (gọi Win32 có tài liệu, mỗi khối có chú thích SAFETY); phần tính toán thuần (`to_physical`,
   `has_mica`) không `unsafe`, chạy test trên mọi nền tảng. Binding: `windows-sys` 0.61 + `windows-version` 0.1 (MIT OR
   Apache-2.0, microsoft/windows-rs, đã có trong cây phụ thuộc của Tauri). Đã cân nhắc: (a) giả lập Win+Z khi rê nút
   (`tauri-plugin-decorum`) — mở bảng snap bằng phím ảo, không phải bảng snap của nút, không có trên Windows 10, nên bỏ;
   (b) `app-region` / vùng non-client của WebView2 — chỉ có ở chế độ composition hosting, wry dùng chế độ HWND.
4. **Mica trên Windows 11, màu đặc ở nơi khác.** `chrome_info` báo build (`windows-version`); từ 22000 trở lên
   `chrome_set_backdrop` gọi `WebviewWindow::set_effects(MicaLight | MicaDark)` (Tauri → `window-vibrancy`, kèm
   `set_theme` để màu Mica theo theme của app, không theo hệ thống). Thanh tiêu đề xin backdrop theo `data-theme` của gốc
   editor (theo dõi bằng `MutationObserver`); tương phản cao (`data-theme='hc'`, `forced-colors: active`) và
   `prefers-reduced-transparency: reduce` xin `none`. Khi Mica bật, `<html data-backdrop="mica">`: trang trong suốt, nhưng
   `.bo-root` chỉ trong suốt trong dải chrome (`--bo-chrome-h` = đáy thanh tiêu đề + ribbon, đo bằng `ResizeObserver`) —
   vùng tài liệu luôn đặc. Dải chrome: chữ trung tính `--bo-text`, nền phủ 40% `--bo-bg`, ribbon phủ 72% `--bo-surface`
   (Win11 "lớp" trên Mica); màu nhấn ở icon app, tab Tệp (nền nhấn đậm, chữ trắng) và tab đang chọn. Tương phản được kiểm
   tra trên Mica trung tính và Mica nhuộm màu hình nền (`MICA_BASE`, `tokens.test.ts`): chữ thường / chữ phụ ≥ 4.5:1, chữ
   màu nhấn chỉ đặt trên lớp ribbon. Không có Mica (Windows 10, HC, giảm trong suốt): dải màu nhấn như web, như trước.
   Màu của dải là token `--bo-chrome-band-*`, `--bo-chrome-search-*`, `--bo-ribbon-bg` trong `theme.css` (khai báo ở mỗi
   phạm vi `[data-app]` để tham chiếu màu nhấn đúng app), thay cho các giá trị `rgb(255 255 255 / x%)` rải trong CSS.
5. **Font giao diện của hệ điều hành trên desktop.** Desktop gắn `<html data-os="windows|macos|linux">` (từ Rust
   `std::env::consts::OS`, qua `Platform`, không dò user agent); `tokens.css` sinh `:root[data-os=…]` đổi `--bo-font` và
   `--bo-font-display`: Windows `Segoe UI Variable Text` / `Segoe UI Variable Display` (Windows 10: `Segoe UI`), macOS
   `-apple-system`, Linux `system-ui`; mỗi chuỗi kết thúc bằng Be Vietnam Pro (đóng gói) rồi `sans-serif`. Không đóng gói font
   hệ thống. Web giữ Be Vietnam Pro. Font chữ tài liệu (canvas, ADR-0003) không đổi.

## Hệ quả
- Một thanh tiêu đề duy nhất trên Windows; snap layouts, menu hệ thống, Alt+Space, kéo/nhấp đúp như cửa sổ có khung.
- Thêm crate `bo-winchrome` với `unsafe` có giới hạn; khi Tauri/wry hỗ trợ vùng non-client của WebView2 thì có thể bỏ.
- `apps/desktop` phụ thuộc thêm `@blueoffice/ui`, `@blueoffice/i18n` (caption của màn hình Bắt đầu).
- Ảnh chụp kiểm tra: `docs/ui/screenshots/desktop-win11-*.png`. Chưa kiểm thử trên macOS/Linux (khung native) và
  Windows 10 thật (đường màu đặc chỉ qua test đơn vị + giả lập).
