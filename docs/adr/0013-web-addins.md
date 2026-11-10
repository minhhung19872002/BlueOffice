# ADR-0013: Add-in web trong khung sandbox, nói chuyện qua postMessage JSON-RPC, dùng lại API BlueScript

- Trạng thái: Chấp nhận · Ngày: 2026-11-11
- Liên quan: parity CORE-ADDIN-001; ADR-0012 (BlueScript: API allow-list, một lần chạy = một bước hoàn tác);
  ADR-0008 (cầu lệnh JSON); `packages/addin-sdk`, `packages/addins`, `apps/server/BlueOffice.Api/Addins/`.

## Bối cảnh
Tổ chức muốn mở rộng BlueDoc / BlueSheet / BlueSlide bằng ứng dụng web nội bộ (tra cứu hợp đồng, chèn mẫu văn
bản, đẩy số liệu từ hệ thống khác vào bảng tính…). Mã của add-in là mã không tin cậy so với editor: nó đến từ origin
khác, do bên khác viết, có thể bị chiếm quyền. Nó không được thấy tài liệu khi chưa được cho phép, không được chạm
DOM / token / lưu trữ của editor, và không được tạo một "API thứ hai" song song với BlueScript.

## Quyết định
1. **Trang web trong iframe sandbox** trong một task pane của khung task pane sẵn có (`TaskPaneHost`):
   `sandbox="allow-scripts allow-forms"` — không `allow-same-origin` (origin mờ), không điều hướng cửa sổ trên,
   không popup, không tải xuống, không hộp thoại modal. Chỉ add-in được **quản trị viên** đánh dấu `trusted` trong
   danh mục (không phải do manifest tự khai) mới có thêm `allow-same-origin`. Khung còn có thuộc tính `csp`
   (CSP Embedded Enforcement): `default-src`/`script-src` = các origin của manifest (+ origin editor để nạp
   `/addin-sdk/v1.js`), `connect-src` = quyền `network:<origin>` (không có: `'none'`), `frame-src`/`object-src`
   `'none'`, `form-action 'none'`, `base-uri 'none'`; `allow` tắt camera, micro, vị trí, đọc clipboard, USB… Trang
   add-in phải chấp nhận chính sách (header `Allow-CSP-From` hoặc CSP riêng chặt hơn) — trình duyệt chặn trang không
   chấp nhận. Desktop (Tauri / WebView2) dùng đúng khung này; CSP ứng dụng thêm `frame-src 'self' https:
   http://localhost:* http://127.0.0.1:*`.
2. **Giao thức**: JSON-RPC 2.0 qua `postMessage`, bọc trong phong bì `{ tag: "bo-addin", v: 1, jsonrpc: "2.0", … }`
   (có phiên bản; bắt tay `addin.hello { protocols: [1] }` chọn phiên bản chung). Editor chỉ nhận thông điệp có
   `source` là `contentWindow` của khung đó; mỗi thông điệp được kiểm lược đồ (`parseRequest`: tên method, ≤ 16 tham
   số, JSON thuần ≤ 2 MB — hàm, symbol, vòng tham chiếu bị từ chối), xử lý tuần tự, giới hạn tốc độ (token bucket
   20/s, burst 60). Gửi vào khung dùng `targetOrigin '*'` vì origin mờ không gọi tên được; nội dung trả lời chỉ là
   những gì quyền đã cấp cho phép.
3. **Không có API thứ hai**: method tài liệu là **đúng** danh sách `API_METHODS` của BlueScript (`doc.*`, `sheet.*`,
   `slide.*`; test so khớp hai danh sách) và được thực hiện bởi chính `docScriptHost` / `sheetScriptHost` /
   `slideScriptHost` — tức cùng dispatcher allow-list, cùng kiểm đối số, cùng cầu lệnh engine (hoàn tác, bảo vệ tài
   liệu, theo dõi thay đổi, cộng tác áp dụng như thao tác tay). Mỗi lệnh sửa là **một bước hoàn tác**
   (`historyLen`/`mergeHistory` như một lần chạy script). Thêm ít method riêng của add-in: `addin.hello`,
   `addin.selection` (vùng chọn kèm nội dung), `storage.get|set|remove|keys`.
4. **Quyền** (manifest khai, người dùng đồng ý tất cả khi cài, server lưu danh sách đã đồng ý; quyền hiệu lực = giao
   của đã đồng ý và manifest): `read-document` (đọc toàn bộ), `write-document` (sửa), `selection` (đọc/đổi vùng chọn,
   đọc nội dung đang chọn), `ui:ribbon` (nút lệnh), `storage`, `network:<origin>` (connect-src). Bảng
   `METHOD_PERMISSIONS` (`packages/addin-sdk/src/permissions.ts`) là nguồn duy nhất; method không có trong bảng bị
   từ chối; lệnh thô `*.command` cần cả đọc và sửa. Sự kiện (`selectionChanged`, `documentChanged`, `command`) không
   mang nội dung và chỉ gửi khi có quyền tương ứng.
5. **Manifest JSON riêng** (`manifest.schema.json` + `validateManifest`, server kiểm lại bằng
   `AddinManifestValidator`): id dạng tên miền đảo, tên/mô tả/nhãn nút **vi + en**, phiên bản semver, nhà phát hành,
   biểu tượng, `startUrl` và `icon` phải nằm trên `allowedOrigins` (https; `http://localhost` chỉ khi server cho phép),
   quyền, ứng dụng hỗ trợ, `ribbon { tab?, group?, commands[] }` (cần `ui:ribbon`). Thuộc tính lạ bị từ chối.
6. **Nút lệnh**: mỗi lệnh là `addin.<id>.<lệnh>` trong `CommandRegistry` (tìm lệnh, phím tắt người dùng, thanh truy cập
   nhanh dùng được như lệnh có sẵn); nằm trong tab manifest gọi tên nếu có, không thì tab "Add-in" (tab này còn có
   "Quản lý add-in"). `action: "openPane"` mở ngăn; `"event"` mở ngăn rồi gửi sự kiện `command` (xếp hàng tới khi
   add-in bắt tay xong).
7. **Danh mục của tổ chức** trên server .NET: bảng `Addins` (manifest đã kiểm, `Trusted`, `Enabled`) và
   `AddinInstalls` (theo người dùng: `user:<sub>` khi đăng nhập OIDC, `host:<host>:<user>` khi qua phiên editor),
   migration cho cả PostgreSQL và SQLite. Quản trị viên (vai trò `Addins:AdminRole`, mặc định `blueoffice-admin`,
   kể cả `realm_access.roles` của Keycloak) phát hành qua `PUT /api/admin/addins/{id}`; `Addins:ApprovedOrigins`
   giới hạn origin. **Mặc định tắt**: mọi route 404 tới khi `Addins:Enabled=true`. Chưa có khu quản trị trong web
   nên quản trị qua API + tài liệu ([docs/deploy/addins.md](../deploy/addins.md)).
8. **Lưu trữ của add-in**: theo người dùng, trên máy (local storage của trình duyệt / WebView), **một mục cho mỗi
   add-in** `blueoffice.addin.storage.<id>` do host chọn theo id — add-in không đặt được tên mục nên không đọc được
   dữ liệu của add-in khác; ≤ 1 MB / 500 khoá; gỡ cài đặt (hoặc phát hiện đã gỡ ở nơi khác) xoá mục này. Giống
   script đã lưu (ADR-0012 mục 5): không lên server.
9. **SDK**: `packages/addin-sdk` (TS, không phụ thuộc) — `connect()` → `AddinClient` với promise có kiểu
   (`doc`, `workbook`, `presentation`, `selection`, `storage`, `call`) và `on(event)`; bản IIFE `addin-sdk/v1.js`
   (biến toàn cục `BlueOfficeAddin`) do bản build web sinh ra để trang add-in nạp bằng `<script src>` không cần
   bundler.

## Hệ quả
- Gói mới `@blueoffice/addin-sdk`, `@blueoffice/addins`; ba editor gọi `useAddins` (hook dùng chung).
- Add-in không thấy gì ngoài những gì quyền cho phép; mọi thay đổi đi qua đường lệnh của UI.
- Hạn chế đã biết: trang add-in có thể tự điều hướng *trong* khung (sandbox/CSP không chặn được), trang mới chỉ có
  đúng các quyền đã cấp; Firefox bỏ qua thuộc tính `csp`; lưu trữ add-in không đồng bộ giữa các máy; chưa có trang
  quản trị trong web; chưa hỗ trợ đồng ý từng phần hay nâng quyền khi manifest đổi (quản trị viên cập nhật manifest
  thì quyền đã cấp cũ vẫn là giới hạn — quyền mới không có hiệu lực tới khi người dùng cài lại).
