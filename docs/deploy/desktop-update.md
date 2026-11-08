# Cập nhật tự động bản desktop (BlueDoc / BlueSheet / BlueSlide)

Dùng Tauri updater (plugin `tauri-plugin-updater`, Apache-2.0 OR MIT), bật bằng feature `updater` của `bo-desktop`.

## Chuẩn bị khoá ký
```bash
pnpm --filter @blueoffice/desktop tauri signer generate -w ~/.tauri/bluestar-update.key
```
Giữ khoá bí mật ngoài repo (biến CI `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`).

## Cấu hình có sẵn trong repo
`tauri.conf.json` (và `tauri.bluesheet.conf.json`, `tauri.blueslide.conf.json`) đã có `plugins.updater`:
```json
"updater": {
  "pubkey": "REPLACE_WITH_BLUESTAR_UPDATER_PUBLIC_KEY",
  "endpoints": ["https://updates.bluestar.vn/blueoffice/bluedoc/{{target}}/{{arch}}/{{current_version}}"],
  "windows": { "installMode": "passive" }
}
```
- **`pubkey` là chỗ giữ chỗ**: chừng nào còn giá trị `REPLACE_WITH_BLUESTAR_UPDATER_PUBLIC_KEY`, lệnh "Kiểm tra cập nhật"
  báo "chưa được cấu hình máy chủ cập nhật" và không tải gì (`update.rs` `configured`). Quy trình phát hành thay bằng
  nội dung public key (file `.pub` do `tauri signer generate` tạo) qua file cấu hình bổ sung bên dưới.
- **Điểm cập nhật đổi được** mà không cần build lại: biến môi trường `BLUEOFFICE_UPDATE_URL` (một hoặc nhiều URL, cách
  nhau bằng dấu phẩy, có thể dùng `{{target}}`, `{{arch}}`, `{{current_version}}`) thay cho `endpoints` — dùng cho máy chủ
  cập nhật on-premise. Bản phát hành chỉ chấp nhận `https://`.
- Feature Cargo `updater` bật plugin (CI build desktop luôn bật: `tauri build --features updater`). Bản không bật feature
  trả về trạng thái `disabled`.

## Cấu hình bản phát hành
Tạo file cấu hình bổ sung (không commit khoá) và truyền khi build:
```json
{
  "bundle": { "createUpdaterArtifacts": true },
  "plugins": {
    "updater": {
      "pubkey": "<nội dung public key>",
      "endpoints": ["https://office.<domain>/updates/bluedoc/{{target}}/{{arch}}/{{current_version}}"]
    }
  }
}
```
```bash
pnpm --filter @blueoffice/desktop tauri build --features updater --config src-tauri/tauri.release.conf.json
```
Máy chủ cập nhật on-premise chỉ cần phục vụ file JSON tĩnh theo định dạng của Tauri updater cùng gói cài đặt đã ký
(trả `204 No Content` khi không có bản mới):
```json
{
  "version": "0.2.0",
  "notes": "Sửa lỗi in",
  "pub_date": "2026-11-01T00:00:00Z",
  "platforms": {
    "windows-x86_64": { "signature": "<nội dung .sig>", "url": "https://…/BlueDoc_0.2.0_x64-setup.exe" }
  }
}
```

## Giao diện
- Start → **Cài đặt → Cập nhật**: nút "Kiểm tra cập nhật"; khi có bản mới, "Cài đặt và khởi động lại" (tải, kiểm tra chữ
  ký minisign, cài, khởi động lại).
- Trong BlueDoc: tab **Tệp → Kiểm tra cập nhật** (kết quả ở thanh trạng thái).
- Khi khởi động, app tự kiểm tra tối đa một lần mỗi ngày (`shouldCheckForUpdates`) và báo nếu có bản mới.

## Deep link & một tiến trình
- Mỗi app đăng ký scheme riêng (`bluedoc://`, `bluesheet://`, `blueslide://`) trong `plugins.deep-link` của file cấu hình;
  bộ cài đặt ghi scheme vào hệ điều hành, bản dev tự đăng ký lúc chạy (Windows/Linux).
- Mở file thứ hai/deep link khi app đang chạy: plugin single-instance chuyển tham số sang tiến trình đang chạy
  (sự kiện `open-file` / `open-link`), cửa sổ chính được đưa lên trước.
- Định dạng: `bluedoc://open?host=bluedrive&file={id}&token=...` (`apps/desktop/src-tauri/src/shell.rs`).
