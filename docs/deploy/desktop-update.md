# Cập nhật tự động bản desktop (BlueDoc / BlueSheet / BlueSlide)

Dùng Tauri updater (plugin `tauri-plugin-updater`, Apache-2.0 OR MIT), bật bằng feature `updater` của `bo-desktop`.

## Chuẩn bị khoá ký
```bash
pnpm --filter @blueoffice/desktop tauri signer generate -w ~/.tauri/bluestar-update.key
```
Giữ khoá bí mật ngoài repo (biến CI `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`).

## Cấu hình bản phát hành
Tạo file cấu hình bổ sung (không commit khoá) và truyền khi build:
```json
{
  "bundle": { "createUpdaterArtifacts": true },
  "plugins": {
    "updater": {
      "pubkey": "<nội dung public key>",
      "endpoints": ["https://office.<domain>/updates/{{target}}/{{arch}}/{{current_version}}?app=bluedoc"]
    }
  }
}
```
```bash
pnpm --filter @blueoffice/desktop tauri build --features updater --config src-tauri/tauri.release.conf.json
```
Máy chủ cập nhật on-premise chỉ cần phục vụ file JSON tĩnh theo định dạng của Tauri updater cùng gói cài đặt đã ký.

## Deep link & một tiến trình
- Mỗi app đăng ký scheme riêng (`bluedoc://`, `bluesheet://`, `blueslide://`) trong `plugins.deep-link` của file cấu hình;
  bộ cài đặt ghi scheme vào hệ điều hành, bản dev tự đăng ký lúc chạy (Windows/Linux).
- Mở file thứ hai/deep link khi app đang chạy: plugin single-instance chuyển tham số sang tiến trình đang chạy
  (sự kiện `open-file` / `open-link`), cửa sổ chính được đưa lên trước.
- Định dạng: `bluedoc://open?host=bluedrive&file={id}&token=...` (`apps/desktop/src-tauri/src/shell.rs`).
