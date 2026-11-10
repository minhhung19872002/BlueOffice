# Third-party notices

BlueOffice chỉ dùng thành phần bên thứ ba có license MIT / Apache-2.0 / BSD / ISC / MPL-2.0 / Zlib / OFL
(xem `BlueOffice-PROMPT.md` §1). Chính sách được kiểm tra tự động trong CI:

- Rust: `cargo deny check licenses bans sources` (cấu hình `deny.toml`) — bao gồm toàn bộ dependency bắc cầu.
- npm: `pnpm license-check` (`scripts/license-check.mjs`, dựa trên `pnpm licenses list`).
- .NET: gói Microsoft (MIT), Serilog / OpenTelemetry / Minio / xUnit (Apache-2.0), StackExchange.Redis (MIT),
  Npgsql (PostgreSQL License — permissive kiểu BSD/MIT); danh sách bắc cầu: `dotnet list package --include-transitive`.

Bảng dưới liệt kê dependency **trực tiếp**. Danh sách đầy đủ (bắc cầu) sinh bằng
`cargo deny list` và `pnpm licenses list`.

## Font (đóng gói trong `assets/fonts`)

| Font | Phiên bản | License | Mục đích |
|---|---|---|---|
| Tinos (Regular/Bold/Italic/BoldItalic) | google/fonts `ofl/tinos` | Apache-2.0 / OFL-1.1 | Thay thế metric-compatible cho Times New Roman |
| Liberation Sans 2.1.5 | liberationfonts | OFL-1.1 (`OFL-Liberation.txt`) | Thay thế metric-compatible cho Arial |
| Carlito | google/fonts `ofl/carlito` | OFL-1.1 (`OFL-Carlito.txt`) | Thay thế metric-compatible cho Calibri |
| Caladea (Regular/Bold/Italic/BoldItalic) | google/fonts `ofl/caladea` (Copyright 2012 The Caladea Project Authors) | OFL-1.1 (`OFL-Caladea.txt`) | Thay thế metric-compatible cho Cambria |
| STIX Two Math 2.13 | stipub/stixfonts `fonts/static_ttf` (Copyright 2001-2021 The STIX Fonts Project Authors; STIX Fonts™ là nhãn hiệu của IEEE; tên font dành riêng "TM Math" của MicroPress) | OFL-1.1 (`OFL-STIXTwo.txt`) | Font toán có bảng OpenType MATH cho phương trình (thay cho Cambria Math), dùng nguyên bản, không sửa |
| Be Vietnam Pro (Regular/Medium/SemiBold/Bold) | google/fonts `ofl/bevietnampro` (Copyright 2021 The Be Vietnam Pro Project Authors) | OFL-1.1 (`packages/ui/fonts/OFL-BeVietnamPro.txt`) | Font giao diện (không dùng cho nội dung tài liệu) |

## Rust

| Crate | Phiên bản | License | Mục đích |
|---|---|---|---|
| quick-xml | 0.42.0 | MIT | Parse/ghi XML (OOXML) |
| zip | 9.3.0 | MIT | Gói OPC (ZIP) |
| miniz_oxide | 0.8.9 | MIT OR Zlib OR Apache-2.0 | Nén stream PDF; giải nén storage dự án VBA của .ppt (`bo-binary`) |
| ttf-parser | 0.25.1 | MIT OR Apache-2.0 | Đọc font, metric, outline glyph |
| rustybuzz | 0.20.1 | MIT | Text shaping (port HarfBuzz) |
| unicode-linebreak | 0.1.5 | Apache-2.0 | Ngắt dòng UAX #14 |
| unicode-bidi | 0.3.18 | MIT OR Apache-2.0 | Thuật toán hai chiều UAX #9 (sắp xếp lại chữ RTL) |
| unicode-normalization | 0.1.26 | (MIT OR Apache-2.0) AND Unicode-3.0 | Chuẩn hoá NFC tiếng Việt |
| unicode-segmentation | 1.13.3 | MIT OR Apache-2.0 | Grapheme cluster (ngắt từ quá dài) |
| tiny-skia | 0.12.0 | BSD-3-Clause | Rasterize display list → PNG |
| pdf-writer | 0.15.0 | MIT OR Apache-2.0 | Ghi PDF |
| png | 0.18.1 | MIT OR Apache-2.0 | Giải mã ảnh PNG để nhúng PDF |
| zune-jpeg / zune-core | 0.4.21 / 0.4.12 | MIT OR Apache-2.0 OR Zlib | Giải mã ảnh JPEG khi raster hoá |
| libm | 0.2.16 | MIT | Hàm toán học thuần Rust (sin/cos/tan/pow) cho `bo-model3d` và khối 3D của `bo-pptx`: ảnh và mô hình sinh ra giống hệt nhau trên mọi nền tảng (thư viện C của hệ điều hành có thể khác bit cuối) |
| subsetter | 0.2.6 | MIT OR Apache-2.0 | Subset font nhúng PDF |
| regex | 1.13.1 | MIT OR Apache-2.0 | Tìm/thay thế bằng biểu thức chính quy (`bo-edit`) |
| serde / serde_json | 1.0.229 / 1.0.154 | MIT OR Apache-2.0 | Tuần tự hoá display list / DocInfo |
| thiserror | 2.0.21 | MIT OR Apache-2.0 | Kiểu lỗi |
| wasm-bindgen | 0.2.130 | MIT OR Apache-2.0 | Binding WASM cho web |
| clap | 4.6.7 | MIT OR Apache-2.0 | CLI `bo-cli` |
| rquickjs / rquickjs-core / rquickjs-sys | 0.14.0 | MIT | BlueScript chạy headless trong `bo-cli` (`bo-script`, CORE-SCRIPT-004); `rquickjs-sys` biên dịch mã C QuickJS-ng (MIT, Fabrice Bellard, Charlie Gordon, các tác giả quickjs-ng) |
| allocator-api2 / relative-path | 0.2.21 / 2.0.3 | MIT OR Apache-2.0 | Phụ thuộc của `rquickjs-core` |
| cfb | 0.14.0 | MIT | Compound File Binary ([MS-CFB]) cho .doc/.xls/.ppt, vbaProject.bin, file mã hoá |
| aes / cbc / ecb | 0.8.4 / 0.1.2 / 0.1.2 | MIT OR Apache-2.0 / MIT OR Apache-2.0 / MIT | AES-CBC/ECB cho [MS-OFFCRYPTO] |
| sha1 / sha2 / hmac | 0.10.7 / 0.10.9 / 0.12.1 | MIT OR Apache-2.0 | Băm khoá, HMAC toàn vẹn dữ liệu, hash bảo vệ sheet; CertID của OCSP (bo-sign) |
| base64 | 0.22.1 | MIT OR Apache-2.0 | Thuộc tính base64 trong EncryptionInfo / sheetProtection |
| getrandom | 0.4.3 | MIT OR Apache-2.0 | Salt / khoá ngẫu nhiên khi mã hoá (WASM: `wasm_js`, crypto.getRandomValues) |
| tauri / tauri-build | 2.12.5 / 2.7.4 | Apache-2.0 OR MIT | Vỏ desktop |
| tauri-plugin-dialog | 2.8.4 | Apache-2.0 OR MIT | Hộp thoại mở/lưu file native |
| tauri-plugin-deep-link | 2.6.3 | Apache-2.0 OR MIT | Deep link `bluedoc://` · `bluesheet://` · `blueslide://` |
| tauri-plugin-single-instance | 2.5.5 | Apache-2.0 OR MIT | Chuyển file/deep link sang tiến trình đang chạy |
| tauri-plugin-updater (feature `updater`) | 2.14.1 | Apache-2.0 OR MIT | Tự cập nhật bản desktop |
| rsa, sha2, x509-cert, cms, der, spki, const-oid (bo-sign) | 0.9.10, 0.10.9, 0.2.5, 0.2.3, 0.7.10, 0.7.3, 0.9.6 | Apache-2.0 OR MIT | Ký số PAdES / OOXML (RustCrypto) |
| p256, p384, ecdsa, elliptic-curve, primeorder, sec1, crypto-bigint, ff, group, base16ct, rfc6979 (bo-sign) | 0.13.2, 0.13.1, 0.16.9, 0.13.8, 0.13.6, 0.7.3, 0.5.5, 0.13.1, 0.13.0, 0.2.0, 0.4.0 | Apache-2.0 OR MIT | Ký và kiểm tra chữ ký ECDSA P-256 / P-384 (chứng thư, CRL, OCSP, CMS, XML-DSig) |
| base64 (bo-sign) | 0.22 | Apache-2.0 OR MIT | Mã hoá chữ ký XML-DSig |
| ureq (bo-sign, feature `http`; kéo theo rustls, ring, webpki-roots) | 3.5.1 | MIT OR Apache-2.0 (ring: Apache-2.0 AND ISC; webpki-roots: CDLA-Permissive-2.0) | HTTP(S) tới máy chủ dấu thời gian RFC 3161 và dịch vụ ký số từ xa |
| cryptoki (bo-sign, feature `pkcs11`, chỉ desktop) | 0.12.1 | Apache-2.0 | USB token qua PKCS#11 (FFI nằm trong crate, crate của ta vẫn `forbid(unsafe_code)`) |
| p12-keystore (bo-sign, feature `pkcs12`) | 0.4.1 | MIT OR Apache-2.0 | Đọc file chứng thư `.p12` / `.pfx` (PKCS#12) |
| serde, serde_json (bo-sign) | 1.0.229, 1.0.154 | MIT OR Apache-2.0 | Giao thức JSON ký số từ xa, báo cáo xác minh |
| lopdf | 0.38.0 | MIT | Mở PDF thành tài liệu BlueDoc (chỉ văn bản, `bo-engine::pdf_import`); kiểm tra PDF đã ký (dev, bo-sign) |
| rand (dev, bo-sign) | 0.8.8 | Apache-2.0 OR MIT | Sinh khoá test |
| yrs (bo-collab) | 0.25.0 | MIT | CRDT tương thích Yjs, giao thức y-sync/awareness |
| insta (dev) | 1.49.0 | Apache-2.0 | Snapshot test |
| proptest (dev) | 1.12.0 | MIT OR Apache-2.0 | Property-based test |
| criterion (dev) | 0.8.2 | Apache-2.0 OR MIT | Benchmark |
| stats_alloc (dev) | 0.1.10 | MIT | Đếm byte heap trong test rò rỉ bộ nhớ mở/đóng 100 tài liệu (CORE-PERF-001) |

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
| quickjs-emscripten-core, @jitl/quickjs-wasmfile-release-sync, @jitl/quickjs-ffi-types (QuickJS © Fabrice Bellard, Charlie Gordon) | 0.32.0 | MIT | Sandbox BlueScript (QuickJS WASM trong Web Worker, ADR-0012) |
| @radix-ui/react-tabs, @radix-ui/react-tooltip, @radix-ui/react-dialog | 1.2.1 / 1.4.1 / 1.3.1 | MIT | Primitive ribbon/tooltip/dialog |
| @tauri-apps/api | 2.12.4 | Apache-2.0 OR MIT | IPC desktop |
| @tauri-apps/cli (dev) | 2.12.4 | Apache-2.0 OR MIT | Build desktop |
| pixelmatch, pngjs (dev, scripts/visual-diff.mjs) | 7.2.0, 7.0.0 | ISC, MIT | So sánh ảnh visual regression (§7) |
| vite, @vitejs/plugin-react (dev) | 8.3.6 / 6.2.1 | MIT | Bundler |
| typescript (dev) | 5.9.3 | Apache-2.0 | Ngôn ngữ |
| vitest, happy-dom, @testing-library/react (dev) | 5.1.0 / 21.0.4 / 16.3.3 | MIT | Unit test |
| @playwright/test (dev) | 1.64.1 | Apache-2.0 | E2E |
| eslint, typescript-eslint, prettier (dev) | 10.14.0 / 8.72.1 / 3.9.10 | MIT | Lint / format |

## .NET

| Gói | Phiên bản | License | Mục đích |
|---|---|---|---|
| Serilog.AspNetCore | 8.0.3 | Apache-2.0 | Logging |
| Microsoft.Extensions.* | 8.0.x | MIT | Options, logging, HttpClient, hosting |
| Microsoft.AspNetCore.Authentication.JwtBearer | 8.0.11 | MIT | OIDC / JWT |
| Microsoft.IdentityModel.JsonWebTokens | 7.1.2 | MIT | JWT host (JWKS), mock BlueDrive |
| Microsoft.EntityFrameworkCore (+ .Sqlite) | 8.0.11 | MIT | ORM, SQLite (dev/test) |
| Microsoft.EntityFrameworkCore.Design | 8.0.11 | MIT | `dotnet ef migrations` (chỉ lúc phát triển, PrivateAssets) |
| Microsoft.Extensions.Diagnostics.HealthChecks.EntityFrameworkCore | 8.0.11 | MIT | Health check |
| Npgsql.EntityFrameworkCore.PostgreSQL (Npgsql) | 8.0.11 (8.0.6) | PostgreSQL License | PostgreSQL |
| Minio (CommunityToolkit.HighPerformance, System.Reactive) | 6.0.4 | Apache-2.0 (MIT) | Object store |
| StackExchange.Redis (Pipelines.Sockets.Unofficial) | 2.8.16 | MIT | Hàng đợi job |
| OpenTelemetry.Extensions.Hosting, .Instrumentation.AspNetCore/Http, .Exporter.OpenTelemetryProtocol | 1.20.1 | Apache-2.0 | Trace / metric |
| SQLitePCLRaw.* (bắc cầu) | 2.1.6 | Apache-2.0 (SQLite: public domain) | SQLite native |
| Microsoft.AspNetCore.Mvc.Testing (dev) | 8.0.11 | MIT | Integration test |
| xunit, xunit.runner.visualstudio (dev) | 2.9.2 / 2.8.2 | Apache-2.0 | Unit test |
| DocumentFormat.OpenXml (tools/BlueOffice.Validate) | 3.1.1 | MIT | Validate OOXML xuất ra (prompt §7) |
| Microsoft.NET.Test.Sdk (dev) | 17.11.1 | MIT | Test host |
| Testcontainers.Redis (Testcontainers, Docker.DotNet.Enhanced, BouncyCastle.Cryptography, SSH.NET, SharpZipLib) (dev) | 4.1.0 | MIT | Test tích hợp Redis thật (bỏ qua khi không có Docker) |

## Icon

Icon giao diện từ **Lucide** (ISC). Logo/favicon BlueOffice tự thiết kế.

Thư viện biểu tượng chèn vào tài liệu (`assets/icons/library/`, 210 biểu tượng, lệnh "Biểu tượng" của
BlueDoc / BlueSheet / BlueSlide) là **tác phẩm gốc của dự án BlueOffice (Bluestar)**: từng hình được vẽ
bằng tay trên lưới 24 × 24 trong `scripts/gen-icon-library.mjs`, không sao chép, không dò nét từ bất kỳ
bộ icon bên thứ ba hay phần mềm thương mại nào (kể cả Lucide). Không có ràng buộc giấy phép bên thứ ba;
xem `assets/icons/library/README.md`.

## Dữ liệu đặc tả

- `crates/bo-drawingml/src/presets.dsl`: 187 hình preset chuyển đổi cơ học từ phụ lục
  `presetShapeDefinitions.xml` của **ECMA-376 Part 1** (ECMA International, tài liệu công khai —
  nguồn hợp lệ theo quy tắc clean-room). Chỉ giữ công thức guide, text rect và path.
- `crates/bo-drawingml/src/textwarps.dsl`: 40 kiểu uốn chữ preset (WordArt, §20.1.9.19 `prstTxWarp`)
  chuyển đổi cơ học từ phụ lục `presetTextWarpDefinitions.xml` của **ECMA-376 Part 1** (cùng gói
  `OfficeOpenXML-DrawingMLGeometries.zip`, ECMA International, tài liệu công khai — nguồn hợp lệ theo quy tắc
  clean-room). Chỉ giữ avLst, công thức guide và path (bỏ ahLst). Cách chữ bám theo path do BlueOffice tự chọn
  (`crates/bo-drawingml/src/textwarp.rs`).
- `packages/ui/src/symbols/data.ts`: tên ký tự và khối Unicode (25 khối, ~3.460 ký tự) sinh bằng
  `scripts/gen-symbols.mjs` từ `UnicodeData.txt` + `Blocks.txt` của **Unicode Character Database 16.0.0**
  — © 1991-2024 Unicode, Inc., **Unicode License v3** (permissive; https://www.unicode.org/license.txt).
  Dùng cho bảng chọn ký hiệu (CORE-SYMBOL-001). Từ khoá tiếng Việt và nhãn ký tự đặc biệt do BlueOffice tự viết.
- `crates/bo-encoding/src/cp_tables.rs`: nửa trên (0x80–0xFF) của 18 code page một byte (437, 737, 850, 852,
  857, 860, 861, 863, 865, 866, 874, 1250, 1251, 1253–1257) để đọc bảng dBASE (CORE-FMT-004). Ánh xạ byte →
  ký tự là của các tệp `MAPPINGS/VENDORS/MICSFT/{PC,WINDOWS}/CPnnn.TXT` — © Unicode, Inc., **Unicode License v3**;
  bảng được xuất một lần bằng các codec `cpNNN` của CPython (vốn sinh từ chính các tệp ánh xạ đó), không chép mã nguồn.

## Từ điển chính tả

- `assets/dictionaries/en_US/` — **SCOWL / en_US Hunspell Dictionary 2020.12.07** (Kevin Atkinson,
  http://wordlist.sourceforge.net). Giấy phép cho phép sử dụng, sửa đổi, phân phối kèm thông báo bản quyền
  (SCOWL, VarCon), một phần public domain (MWords, 12Dicts…), WordNet 1.6 (Princeton, cho phép kèm thông báo),
  UKACD (J Ross Beresford — phải kèm nguyên văn thông báo) và BSD (affix file từ Ispell). Toàn văn các thông báo nằm
  trong `assets/dictionaries/en_US/README_en_US.txt`, phân phối kèm từ điển. Dùng cho soát chính tả tiếng Anh (`bo-spell`).
- Tiếng Việt: không dùng từ điển ngoài — `bo-spell::vi` kiểm tra theo quy tắc chính tả (tự cài).

## Từ điển đồng nghĩa

- `assets/thesaurus/en/en.json.gz` — dữ liệu rút gọn sinh bằng `scripts/build-thesaurus.mjs` từ
  **Open English WordNet 2025** (https://github.com/globalwordnet/english-wordnet, bản phát hành `2025-edition`),
  giấy phép **CC BY 4.0** (https://creativecommons.org/licenses/by/4.0/; ghi công: "Thesaurus data derived from
  Open English WordNet 2025, licensed under CC BY 4.0"; thay đổi: chỉ giữ mục từ một từ viết thường, từ đồng
  nghĩa, trái nghĩa, định nghĩa rút ngắn). Open English WordNet dựa trên Princeton WordNet — "WordNet 3.0
  Copyright 2006 by Princeton University. All rights reserved." (giấy phép WordNet, cho phép kèm thông báo);
  nguồn, phiên bản, cách sinh và toàn văn thông báo Princeton nằm trong `assets/thesaurus/en/README.md`, phân
  phối kèm dữ liệu. Dùng cho ngăn / menu "Từ đồng nghĩa" (CORE-THES-001).
- Tiếng Việt: chưa có dữ liệu (không có nguồn mở với giấy phép tương thích đã kiểm chứng).

## Ranh giới bản đồ

- `assets/maps/world/countries.bin` — ranh giới quốc gia rút gọn sinh bằng `scripts/build-world-map.mjs` từ
  **Natural Earth 1:50m Admin 0 – Countries, phiên bản 5.1.1** (https://www.naturalearthdata.com/), **public
  domain** ("All versions of Natural Earth raster + vector map data found on this website are in the public
  domain."; không cần xin phép, không bắt buộc ghi công — vẫn ghi: "Made with Natural Earth."). Nguồn, mã
  SHA-256 của tệp tải về và cách sinh: `assets/maps/README.md`. Dùng cho biểu đồ bản đồ (CORE-CHART-006).
- Ranh giới tỉnh/thành Việt Nam: chưa có dữ liệu (cần bộ dữ liệu có giấy phép được phê duyệt,
  `assets/maps/vn-provinces/README.md`).
