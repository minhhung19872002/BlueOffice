# Add-in của tổ chức (CORE-ADDIN-001)

Add-in là **một trang web nội bộ** (từ origin quản trị viên đã duyệt) chạy trong một ngăn tác vụ của BlueDoc, BlueSheet
hoặc BlueSlide. Nó làm việc với tài liệu **chỉ qua các quyền người dùng đã đồng ý** và qua đúng API của BlueScript.
Thiết kế: [ADR-0013](../adr/0013-web-addins.md) · Bảo mật: [security.md §12A](security.md) ·
Tài liệu cho người viết add-in (tiếng Anh): [API reference](../addins/api-reference.md).

**Mặc định tắt.** Khi `Addins:Enabled = false` (mặc định) mọi route add-in trả 404, các ứng dụng không hiện tab
**Add-in** và không tải trang ngoài nào.

## 1. Dành cho người dùng

1. Mở tab **Add-in** › **Quản lý add-in**. Ngăn "Add-in của tổ chức" liệt kê các add-in quản trị viên đã phát hành
   cho ứng dụng đang mở (BlueDoc / BlueSheet / BlueSlide).
2. **Cài đặt**: hộp thoại cho biết ai cung cấp, trang tải từ đâu và **mọi quyền** add-in xin:

   | Quyền | Nghĩa |
   |---|---|
   | `read-document` | Đọc toàn bộ nội dung tài liệu đang mở |
   | `write-document` | Sửa tài liệu (mỗi lần sửa là một bước Hoàn tác, Ctrl+Z) |
   | `selection` | Đọc và đổi vùng chọn, đọc nội dung đang chọn |
   | `ui:ribbon` | Thêm nút lệnh vào dải lệnh |
   | `storage` | Lưu dữ liệu riêng của add-in trên máy này |
   | `network:<địa chỉ>` | Trang add-in được kết nối tới địa chỉ đó |

   Chọn **Cho phép và cài đặt** để đồng ý tất cả (không cài một phần), hoặc **Hủy**.
3. Sau khi cài, ngăn của add-in mở ra; nút của add-in nằm ở tab manifest chỉ định (vd. **Chèn**) hoặc ở tab
   **Add-in**. Ngăn add-in có thể gắn trái / phải / nổi như các ngăn khác.
4. **Gỡ cài đặt** trong "Quản lý add-in": ngăn đóng, nút biến mất, dữ liệu add-in lưu trên máy bị xoá.

Add-in không bao giờ thấy tài liệu khi bạn chưa cho quyền: ví dụ add-in chỉ có `selection` đọc được đoạn đang chọn
nhưng không đọc được phần còn lại, không sửa được tài liệu.

## 2. Bật tính năng (máy chủ)

`appsettings.json` hoặc biến môi trường `Addins__…`:

| Khoá | Mặc định | Ý nghĩa |
|---|---|---|
| `Addins:Enabled` | `false` | Bật danh mục add-in |
| `Addins:ApprovedOrigins:N` | (rỗng) | Origin được phép làm `allowedOrigins` và `network:` của manifest, vd. `https://addins.noibo.vn`. Rỗng: mọi origin `https` |
| `Addins:AllowLoopbackOrigins` | `false` | Cho phép `http://localhost[:port]` / `http://127.0.0.1[:port]` (chỉ khi phát triển add-in) |
| `Addins:AdminRole` | `blueoffice-admin` | Vai trò được phát hành add-in |
| `Addins:RoleClaims:N` | `roles`, `role`, `groups` | Claim chứa vai trò trong token OIDC (ngoài ra luôn đọc `realm_access.roles` của Keycloak) |
| `Addins:MaxAddins` | `200` | Số add-in tối đa trong danh mục |
| `Addins:MaxManifestChars` | `65536` | Kích thước manifest tối đa |

Bản web tìm danh mục ở cùng origin (`/api/addins`); `VITE_BO_ADDINS_URL` đặt địa chỉ khác, `off` tắt hẳn phía client.
Trình soạn thảo nhúng / mở file từ host và desktop mở bằng deep link dùng phiên editor của máy chủ (`?session=` +
header `X-Session-Token`), cài đặt gắn với người dùng của host.

Desktop: CSP của ứng dụng đã cho phép khung `https:` (và localhost) — không cần cấu hình thêm.

## 3. Phát hành add-in (quản trị viên)

Chưa có trang quản trị trong giao diện web: dùng API với token OIDC mang vai trò `Addins:AdminRole`.

```bash
TOKEN=…   # access token của quản trị viên
# Phát hành / cập nhật (201 khi mới, 200 khi cập nhật); "trusted": true cho phép khung giữ origin riêng.
curl -X PUT https://office.noibo.vn/api/admin/addins/vn.noibo.hopdong \
  -H "Authorization: Bearer $TOKEN" -H 'content-type: application/json' \
  -d '{ "manifest": '"$(cat manifest.json)"', "trusted": false }'
# Ẩn tạm (giữ cài đặt của người dùng)
curl -X PUT …/api/admin/addins/vn.noibo.hopdong -d '{ "manifest": …, "enabled": false }' …
# Danh sách (kèm số người đã cài)
curl -H "Authorization: Bearer $TOKEN" https://office.noibo.vn/api/admin/addins
# Gỡ khỏi danh mục (xoá luôn mọi cài đặt)
curl -X DELETE -H "Authorization: Bearer $TOKEN" https://office.noibo.vn/api/admin/addins/vn.noibo.hopdong
```

Manifest sai trả 400 `invalidManifest` với danh sách `{ path, message }`. Mỗi lần phát hành / gỡ / cài / gỡ cài ghi
một dòng log `BlueOffice.Audit.Addins`.

### Manifest

Lược đồ: [`packages/addin-sdk/manifest.schema.json`](../../packages/addin-sdk/manifest.schema.json). Ví dụ:

```json
{
  "manifestVersion": 1,
  "id": "vn.noibo.hopdong",
  "name": { "vi": "Tra cứu hợp đồng", "en": "Contract lookup" },
  "description": { "vi": "Chèn số và bên ký hợp đồng", "en": "Insert contract numbers and parties" },
  "version": "1.0.0",
  "publisher": "Phòng CNTT",
  "icon": "https://addins.noibo.vn/hopdong/icon.png",
  "startUrl": "https://addins.noibo.vn/hopdong/index.html",
  "allowedOrigins": ["https://addins.noibo.vn"],
  "permissions": ["selection", "write-document", "ui:ribbon", "network:https://api.noibo.vn"],
  "apps": ["doc", "sheet"],
  "ribbon": {
    "tab": "insert",
    "group": { "vi": "Hợp đồng", "en": "Contracts" },
    "commands": [
      { "id": "open", "label": { "vi": "Tra cứu", "en": "Look up" }, "action": "openPane" },
      { "id": "insertNo", "label": { "vi": "Chèn số HĐ", "en": "Insert no." }, "action": "event", "apps": ["doc"] }
    ]
  }
}
```

Quy tắc chính: `id` là tên miền đảo chữ thường; tên, mô tả, nhãn nút có **cả `vi` và `en`**; `startUrl`, `icon` nằm
trên `allowedOrigins`; nút lệnh cần quyền `ui:ribbon`; `ribbon.tab` là id tab (`home`, `insert`, `layout`,
`references`, `review`, `view`, …) — tab không tồn tại thì nút vào tab Add-in.

### Yêu cầu với máy chủ của trang add-in

- Phục vụ qua `https` từ origin trong `allowedOrigins`.
- Chấp nhận CSP mà BlueOffice áp cho khung: trả header `Allow-CSP-From: https://office.noibo.vn` (hoặc `*`), hoặc tự
  gửi CSP chặt hơn. Thiếu header, trình duyệt Chromium/Edge (và desktop WebView2) **chặn** trang.
- Không dùng `<script>` nội tuyến (CSP chỉ cho script từ origin của add-in và `/addin-sdk/v1.js` của BlueOffice).
  Có thể đặt bản sao `v1.js` trên origin của add-in — bắt buộc khi editor ở địa chỉ nội bộ/loopback còn add-in ở
  địa chỉ công khai (trình duyệt chặn theo Private Network Access).
- Gọi API khác chỉ tới origin đã khai bằng `network:` (máy chủ đó phải cho CORS với origin `null`, vì khung không có
  origin riêng — trừ add-in `trusted`).

## 4. Khắc phục sự cố

| Hiện tượng | Nguyên nhân |
|---|---|
| Không có tab Add-in | `Addins:Enabled` chưa bật; client đặt `VITE_BO_ADDINS_URL=off`; máy chủ không truy cập được |
| Add-in không xuất hiện trong danh sách | Không hỗ trợ ứng dụng đang mở (`apps`), bị ẩn (`enabled: false`), manifest không qua kiểm tra phía client |
| Ngăn add-in trắng / bị chặn | Thiếu `Allow-CSP-From`; trang không ở origin khai báo; dùng script nội tuyến |
| Lệnh trả lỗi `-32001` | Thiếu quyền (lỗi kèm danh sách quyền thiếu) |
| Lệnh trả lỗi `-32002` | Gọi quá nhanh (giới hạn 20 lệnh/giây, burst 60) |
