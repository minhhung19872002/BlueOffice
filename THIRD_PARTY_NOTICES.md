# Third-party notices

BlueOffice chỉ dùng thành phần bên thứ ba có license MIT / Apache-2.0 / BSD / ISC / MPL-2.0 / Zlib / OFL
(xem `BlueOffice-PROMPT.md` §1). Chính sách được kiểm tra tự động trong CI:

- Rust: `cargo deny check licenses bans sources` (cấu hình `deny.toml`) — bao gồm toàn bộ dependency bắc cầu.
- npm: `pnpm license-check` (`scripts/license-check.mjs`, dựa trên `pnpm licenses list`).
- .NET: chỉ dùng gói Microsoft (MIT), Serilog (Apache-2.0), xUnit (Apache-2.0).

Bảng dưới liệt kê dependency **trực tiếp**. Danh sách đầy đủ (bắc cầu) sinh bằng
`cargo deny list` và `pnpm licenses list`.

## Font (đóng gói trong `assets/fonts`)

| Font | Phiên bản | License | Mục đích |
|---|---|---|---|
| Tinos (Regular/Bold/Italic/BoldItalic) | google/fonts `ofl/tinos` | Apache-2.0 / OFL-1.1 | Thay thế metric-compatible cho Times New Roman |
| Liberation Sans 2.1.5 | liberationfonts | OFL-1.1 (`OFL-Liberation.txt`) | Thay thế metric-compatible cho Arial |
| Carlito | google/fonts `ofl/carlito` | OFL-1.1 (`OFL-Carlito.txt`) | Thay thế metric-compatible cho Calibri |

## Rust

| Crate | Phiên bản | License | Mục đích |
|---|---|---|---|
| quick-xml | 0.42.0 | MIT | Parse/ghi XML (OOXML) |
| zip | 9.3.0 | MIT | Gói OPC (ZIP) |
| miniz_oxide | 0.8.9 | MIT OR Zlib OR Apache-2.0 | Nén stream PDF |
| ttf-parser | 0.25.1 | MIT OR Apache-2.0 | Đọc font, metric, outline glyph |
| rustybuzz | 0.20.1 | MIT | Text shaping (port HarfBuzz) |
| unicode-linebreak | 0.1.5 | Apache-2.0 | Ngắt dòng UAX #14 |
| unicode-normalization | 0.1.26 | (MIT OR Apache-2.0) AND Unicode-3.0 | Chuẩn hoá NFC tiếng Việt |
| unicode-segmentation | 1.13.3 | MIT OR Apache-2.0 | Grapheme cluster (ngắt từ quá dài) |
| tiny-skia | 0.12.0 | BSD-3-Clause | Rasterize display list → PNG |
| pdf-writer | 0.15.0 | MIT OR Apache-2.0 | Ghi PDF |
| subsetter | 0.2.6 | MIT OR Apache-2.0 | Subset font nhúng PDF |
| serde / serde_json | 1.0.229 / 1.0.154 | MIT OR Apache-2.0 | Tuần tự hoá display list / DocInfo |
| thiserror | 2.0.21 | MIT OR Apache-2.0 | Kiểu lỗi |
| wasm-bindgen | 0.2.130 | MIT OR Apache-2.0 | Binding WASM cho web |
| clap | 4.6.7 | MIT OR Apache-2.0 | CLI `bo-cli` |
| cfb | 0.14.0 | MIT | Compound File Binary ([MS-CFB]) cho .doc/.xls/.ppt, vbaProject.bin, file mã hoá |
| aes / cbc / ecb | 0.8.4 / 0.1.2 / 0.1.2 | MIT OR Apache-2.0 / MIT OR Apache-2.0 / MIT | AES-CBC/ECB cho [MS-OFFCRYPTO] |
| sha1 / sha2 / hmac | 0.10.7 / 0.10.9 / 0.12.1 | MIT OR Apache-2.0 | Băm khoá, HMAC toàn vẹn dữ liệu, hash bảo vệ sheet |
| base64 | 0.22.1 | MIT OR Apache-2.0 | Thuộc tính base64 trong EncryptionInfo / sheetProtection |
| getrandom | 0.3.4 | MIT OR Apache-2.0 | Salt / khoá ngẫu nhiên khi mã hoá |
| tauri / tauri-build | 2.12.5 / 2.7.4 | Apache-2.0 OR MIT | Vỏ desktop |
| tauri-plugin-dialog | 2.8.4 | Apache-2.0 OR MIT | Hộp thoại mở/lưu file native |
| insta (dev) | 1.49.0 | Apache-2.0 | Snapshot test |
| proptest (dev) | 1.12.0 | MIT OR Apache-2.0 | Property-based test |
| criterion (dev) | 0.8.2 | Apache-2.0 OR MIT | Benchmark |

Công cụ fuzz (`fuzz/`, workspace riêng, không đóng gói vào sản phẩm, không chạy trong CI Windows):

| Crate | Phiên bản | License | Mục đích |
|---|---|---|---|
| libfuzzer-sys | 0.4.13 | (MIT OR Apache-2.0) AND NCSA | Harness `cargo fuzz` (NCSA: giấy phép kiểu BSD của LLVM libFuzzer) |
| arbitrary | 1.5.0 | MIT OR Apache-2.0 | Phụ thuộc của libfuzzer-sys |

## TypeScript / npm

| Gói | Phiên bản | License | Mục đích |
|---|---|---|---|
| react / react-dom | 18.3.1 | MIT | UI |
| zustand | 5.0.16 | MIT | State UI |
| i18next / react-i18next | 26.7.1 / 17.1.12 | MIT | Đa ngôn ngữ (vi-VN, en-US) |
| lucide-react | 1.69.0 | ISC | Icon |
| @radix-ui/react-tabs, @radix-ui/react-tooltip | 1.2.1 / 1.4.1 | MIT | Primitive ribbon/tooltip |
| @tauri-apps/api | 2.12.4 | Apache-2.0 OR MIT | IPC desktop |
| @tauri-apps/cli (dev) | 2.12.4 | Apache-2.0 OR MIT | Build desktop |
| vite, @vitejs/plugin-react (dev) | 8.3.6 / 6.2.1 | MIT | Bundler |
| typescript (dev) | 5.9.3 | Apache-2.0 | Ngôn ngữ |
| vitest, happy-dom, @testing-library/react (dev) | 5.1.0 / 21.0.4 / 16.3.3 | MIT | Unit test |
| @playwright/test (dev) | 1.64.1 | Apache-2.0 | E2E |
| eslint, typescript-eslint, prettier (dev) | 10.14.0 / 8.72.1 / 3.9.10 | MIT | Lint / format |

## .NET

| Gói | Phiên bản | License | Mục đích |
|---|---|---|---|
| Serilog.AspNetCore | 8.0.3 | Apache-2.0 | Logging |
| Microsoft.Extensions.* | 8.0.x | MIT | Options, logging |
| Microsoft.AspNetCore.Mvc.Testing (dev) | 8.0.11 | MIT | Integration test |
| xunit, xunit.runner.visualstudio (dev) | 2.9.2 / 2.8.2 | Apache-2.0 | Unit test |
| Microsoft.NET.Test.Sdk (dev) | 17.11.1 | MIT | Test host |

## Icon

Icon giao diện từ **Lucide** (ISC). Logo/favicon BlueOffice tự thiết kế.
