# Ma trận truy vết yêu cầu (Requirements Traceability Matrix)

Đối chiếu từng yêu cầu của [`BlueOffice-PROMPT.md`](../BlueOffice-PROMPT.md) mục 0–10 với code và test trên `main`.
Kiểm tra ngày 2026-11-08 (commit gốc `e94a3d6` + các commit đo hiệu năng/fuzz của lần kiểm này).
Không dựa vào `docs/PROGRESS.md`: mọi bằng chứng là file/test đã mở ra xem và test đã chạy lại.

**Ký hiệu**: ✅ đạt · 🟡 một phần (ghi rõ thiếu gì) · ❌ chưa có · 📝 chỉ kế hoạch (đúng yêu cầu).

**Tổng**: 295 yêu cầu — ✅ 198 · 🟡 84 · ❌ 10 · 📝 3. Danh sách thiếu: mục [Khoảng trống](#khoảng-trống).

**Các bộ test đã chạy lại trong lần kiểm này** (Windows 11, máy dev, có agent khác build song song):

| Bộ | Kết quả |
|---|---|
| `cargo test --workspace` | 658 ✅, 0 lỗi, 2 ignored (`typing_in_300_page_document_under_16_ms` – chạy riêng bằng `--release --ignored` ✅; `dump_regenerated_corpus` – thủ công) + 3 test fuzz mới ✅ |
| `pnpm test` (Vitest) | 109 ✅ (gồm `packages/platform/test/parity.test.ts`) |
| Playwright web (`playwright.config.ts`) | 24/24 ✅ (23 cũ + `perf.spec.ts` mới); `sheet.spec.ts` lỗi 1 lần khi chạy cùng tải nặng, chạy lại 3/3 ✅ → flaky |
| `dotnet test BlueOffice.sln` | 108 ✅ |
| `pnpm lint` · `pnpm typecheck` · `cargo fmt --check` · `cargo clippy -D warnings` (bo-cli, bo-engine) | ✅ |
| `cargo check --manifest-path fuzz/Cargo.toml --bins` | ✅ (14 target biên dịch được trên MSVC) |
| **Không chạy lại**: Playwright cộng tác (`playwright.collab.config.ts`, cần server + BlueDrive giả lập), E2E desktop Tauri (cần `bluedoc.exe` release + msedgedriver), `cargo deny`, `pnpm fidelity` | các dòng dựa vào chúng ghi "(không chạy lại)" |

Tên test Rust được ghi ngắn (`tên_hàm`); đường dẫn là file chứa test. Tên test TS/E2E ghi trong ngoặc đơn `'…'`.

---

## §0. Vai trò & quy trình

| # | Yêu cầu | TT | Bằng chứng |
|---|---|---|---|
| 0.1 | Mỗi phase build được, chạy được, có test tự động | ✅ | Các bộ test ở bảng trên; `.github/workflows/ci.yml` |
| 0.2 | Kế hoạch `docs/plan/phase-N.md` (mục tiêu, file, API, test, tiêu chí) | ✅ | `docs/plan/phase-1.md`, `phase-2.md`, `phase-3-formula.md`, `phase-3-xlsx.md`, `phase-4.md`, `phase-5.md`, `phase-6.md`, `phase-7.md` (không có file riêng cho 6A; 6A nằm ở `phase-6.md` + `docs/bohp.md`) |
| 0.3 | Commit nhỏ, conventional commits | ✅ | `git log` (vd `feat(sign): …`, `test(e2e-desktop): …`) |
| 0.4 | Chạy toàn bộ test Rust + TS + .NET đến khi xanh | ✅ | Bảng trên |
| 0.5 | Cập nhật `PROGRESS.md`, `CLAUDE.md` | ✅ | `docs/PROGRESS.md`, `CLAUDE.md` |
| 0.6 | Dừng chờ duyệt giữa các phase | ✅ | Chủ dự án đã bỏ bước này (ghi trong `CLAUDE.md`, `docs/PROGRESS.md`) |
| 0.7 | Quyết định kiến trúc → ADR | ✅ | `docs/adr/0001…0009` |

## §1. Clean-room

| # | Yêu cầu | TT | Bằng chứng |
|---|---|---|---|
| 1.1 | Không decompile/disassemble/debug-attach Office | ✅ | Quy tắc trong `CLAUDE.md`; không có công cụ/tài liệu RE nào trong repo (không kiểm chứng được bằng test) |
| 1.2 | Không chép code, icon, font, template, chuỗi UI, tên thương hiệu | ✅ | `packages/i18n/src/locales/vi-VN.json` không chứa Word/Excel/PowerPoint/Office/Google; font đóng gói OFL/Apache (`assets/fonts/manifest.json`) |
| 1.3 | Chỉ dùng nguồn hợp lệ, comment tham chiếu spec | ✅ | Comment `// ECMA-376 Part 1, §…` khắp `crates/bo-docx`, `bo-xlsx`, `bo-pptx`, `bo-drawingml`; `[MS-XLS]`, `[MS-OFFCRYPTO]` trong `bo-binary`, `bo-crypto` |
| 1.4 | License chỉ MIT/Apache/BSD/MPL/Zlib/OFL, ghi `THIRD_PARTY_NOTICES.md` | ✅ | `deny.toml`, `THIRD_PARTY_NOTICES.md`, `scripts/license-check.mjs`; Npgsql (PostgreSQL License) qua ADR-0009 |
| 1.5 | CI `cargo deny` + `license-checker` | ✅ | `ci.yml` job `cargo-deny` (`check licenses bans sources`), job `web` bước `pnpm license-check` (không chạy lại `cargo deny`) |
| 1.6 | Ribbon được, nhưng icon/màu/logo riêng; icon Lucide/Fluent | ✅ | `lucide-react` trong `packages/editor|sheet-editor|slide-editor/package.json`; `packages/ui/src/theme.ts` |

## §2. Mục tiêu sản phẩm

| # | Yêu cầu | TT | Bằng chứng |
|---|---|---|---|
| 2.1 | BlueDoc: `.docx` đọc/ghi | ✅ | `crates/bo-docx`; `crates/bo-docx/tests/corpus.rs` `corpus_round_trips_losslessly` |
| 2.2 | BlueDoc: `.doc` đọc | ✅ | `crates/bo-binary/src/doc.rs` `reads_built_doc`; mở qua `Engine::open` |
| 2.3 | BlueDoc: `.odt` | 🟡 | Đọc: `crates/bo-engine/tests/security.rs` `opens_odt_as_a_document`; ghi chỉ qua `bo-cli convert --to odt` (`crates/bo-odf/src/odt_write.rs` `round_trip`), editor lưu thành `.docx` |
| 2.4 | BlueDoc: `.rtf` | ❌ | Không có reader/writer RTF |
| 2.5 | BlueDoc: `.txt`, `.md` | 🟡 | `Engine::open` (`crates/bo-engine/src/lib.rs`) mở `.txt`/`.md` như văn bản thuần (`plain_text_and_garbage` trong `tests/engine.rs`); không phân tích cú pháp Markdown, không lưu ra `.txt`/`.md` |
| 2.6 | BlueDoc: xuất PDF | ✅ | `Engine::export_pdf`; e2e `'exports a PDF'`; `parity.test.ts` `'exports a PDF from WASM'` |
| 2.7 | BlueSheet: `.xlsx` r/w, `.xls`/`.xlsb` đọc, `.csv`, xuất PDF | ✅ | `crates/bo-xlsx/tests/corpus.rs` `corpus_roundtrip`; `bo-binary/src/xls.rs` `reads_sample_workbook`, `xlsb.rs` `reads_sample`; `SheetEngine::save("csv")`; `crates/bo-engine/src/sheet/cmd.rs` `page_setup_drives_pdf_pages` |
| 2.8 | BlueSheet: `.ods` | 🟡 | Đọc trong `SheetEngine::open`; ghi chỉ qua `bo-cli convert --to ods` (`crates/bo-cli/src/convert.rs` `workbook_maps_to_ods_and_csv`), chỉ ghi giá trị |
| 2.9 | BlueSlide: `.pptx` r/w, xuất PDF/PNG | ✅ | `crates/bo-pptx/tests/roundtrip.rs` `corpus_round_trip_is_lossless`; `crates/bo-engine/tests/slides.rs` `slide_exports_pdf_png_and_saves` |
| 2.10 | BlueSlide: `.ppt` đọc | 🟡 | `crates/bo-binary/src/ppt.rs` `reads_sample_deck` chỉ lấy chữ (`PptDeck`); không thành `bo_model::slide`, editor không mở được, không lưu `.pptx` |
| 2.11 | BlueSlide: `.odp` | 🟡 | `crates/bo-odf/src/odp.rs` `write_read_round_trip` (chỉ khung chữ); BlueSlide không mở `.odp` |
| 2.12 | Quy tắc đặt tên (họ sản phẩm, tên hiển thị, app id, route, route cũ chuyển hướng) | ✅ | ADR-0006; `apps/desktop/src-tauri/tauri*.conf.json` (`vn.bluestar.bluedoc|bluesheet|blueslide`); e2e `'legacy routes redirect to the new app routes'` |
| 2.13 | Không dùng tên/icon/màu Microsoft/Google | ✅ | Xem 1.2 |
| 2.14 | Lossless round-trip, mở lại không lỗi | ✅ | Round-trip: `corpus_round_trips_losslessly`, `corpus_roundtrip`, `corpus_round_trip_is_lossless`; OpenXmlValidator `tools/BlueOffice.Validate` + `tests/fidelity/history.csv` (2026-11-08: docx 54/55, xlsx 57/60, pptx 35/35 hợp lệ; ngoại lệ ở `known-issues.txt`) — không chạy lại validator |
| 2.15 | Triển khai Web (cộng tác, on-prem) | ✅ | `apps/web`, `deploy/docker-compose.yml`; collab e2e `'two users edit one .docx: …'` (không chạy lại) |
| 2.16 | Desktop Win/macOS/Linux, offline, mở file trực tiếp, file association | 🟡 | CI build desktop chỉ trên `windows-latest` (job `desktop`); job `rust` có deps Linux nhưng không đóng gói macOS/Linux. File association: `shell.rs` `classifies_args`, e2e desktop `'opens the file passed on the command line (file association)'` (không chạy lại) |
| 2.17 | Server: lưu trữ, phân quyền, cộng tác, chuyển đổi hàng loạt, PDF headless | 🟡 | Xem §6 Phase 6; chuyển đổi chỉ ra PDF (`ConversionJobProcessor.cs`) |
| 2.18 | Nhúng BlueDrive sẵn sàng từ Phase 1 | ✅ | `DocumentStorage`/`AuthSession` trong `packages/platform/src/types.ts`; §6A |
| 2.19 | Web và desktop là MỘT sản phẩm (cùng React + cùng core) | ✅ | `apps/web/src/main.tsx`, `apps/desktop/src/main.tsx` chỉ là vỏ của `packages/editor`; `parity.test.ts` |
| 2.20 | Tiếng Việt: NFC, dấu tổ hợp, IME (composition) | ✅ | `packages/canvas-host/test/ime.test.ts` `'commits composed Vietnamese text once, NFC-normalized'`; `crates/bo-text/tests/fonts.rs` `shapes_vietnamese_precomposed_and_decomposed_alike`; e2e `'types Vietnamese, bolds it, saves .docx and reopens it'` |
| 2.21 | Mẫu văn bản hành chính NĐ 30/2020 | ✅ | e2e `'page setup dialog and the administrative template'`; `crates/bo-cli/src/sample.rs` |
| 2.22 | Font fallback metric-compatible (TNR→Tinos, Arial→Liberation Sans, Calibri→Carlito, Cambria→Caladea) | 🟡 | `crates/bo-text/tests/fonts.rs` `metric_compatible_fallback`, `tinos_matches_times_metrics`; **Caladea không có trong `assets/fonts/manifest.json`** (Cambria chưa có font thay thế) |
| 2.23 | Định dạng số/ngày vi-VN (`.` nghìn, `,` thập phân, dd/MM/yyyy, ₫) | ✅ | `crates/bo-sheet/src/numfmt/tests.rs` `digits_and_separators`, `locale_and_currency_tags`, `dates_and_times` |
| 2.24 | TCVN3/VNI → Unicode khi mở file cũ | 🟡 | `crates/bo-encoding/src/lib.rs` `tcvn3_known_strings`, `vni_known_strings`, `document_conversion`; chỉ áp dụng khi mở `.doc` (không áp cho `.docx/.xls/.xlsx` dùng font cũ) |

## §3. Stack công nghệ

### §3.1–3.3 Thư viện

| # | Yêu cầu | TT | Bằng chứng |
|---|---|---|---|
| 3.1 | UI React 18 + TS + Vite; Canvas cho vùng tài liệu; textarea ẩn cho IME | ✅ | `react@18.3.1`; `packages/canvas-host/src/DocumentView.ts`, `ime.ts` |
| 3.2 | Bridge wasm-bindgen (web) / Tauri command (desktop) | ✅ | `crates/bo-wasm`; `apps/desktop/src-tauri/src/*_cmds.rs`; `packages/platform/test/desktop.test.ts` `'routes engine calls to Tauri commands with raw binary bodies'` |
| 3.3 | Core Rust một codebase → WASM + native | ✅ | `parity.test.ts` `'produces identical display lists for every page of the sample'` |
| 3.4 | Tauri 2 desktop; ASP.NET Core 8 + PostgreSQL + MinIO + Redis; worker Rust headless | ✅ | `apps/desktop/src-tauri/Cargo.toml`; `apps/server/*`; `deploy/docker-compose.yml` |
| 3.5 | Yrs cho cộng tác, tương thích giao thức Yjs | ✅ | `crates/bo-collab/tests/sync.rs` `concurrent_edits_converge`, `initial_sync_transfers_existing_state`; server `CollabTests.EncodesSyncAndAwarenessLikeYProtocols` |
| 3.6 | Rust: quick-xml, zip, cfb | ✅ | `crates/bo-opc|bo-xlsx|bo-binary/Cargo.toml` |
| 3.7 | Rust: rustybuzz, ttf-parser/skrifa | ✅ | `crates/bo-text/Cargo.toml` (skrifa không cần — "/") |
| 3.8 | Rust: unicode-linebreak/bidi/segmentation/normalization | 🟡 | Có linebreak, segmentation, normalization; **thiếu `unicode-bidi`** (chưa hỗ trợ văn bản hai chiều) |
| 3.9 | Rust: `hyphenation` | ❌ | Không phải dependency; không ngắt từ tiếng Anh |
| 3.10 | Rust: tiny-skia, pdf-writer, subsetter | ✅ | `bo-raster`, `bo-pdf`; `crates/bo-pdf/src/lib.rs` `subset_tags_are_uppercase` |
| 3.11 | Rust: `image` (PNG/JPEG/GIF/BMP/TIFF) | 🟡 | Dùng `png` + `zune-jpeg` thay `image`; GIF/BMP/TIFF không giải mã |
| 3.12 | EMF/WMF tự viết bộ chuyển ([MS-EMF]/[MS-WMF]) | ❌ | Chỉ ánh xạ MIME (`crates/bo-docx/src/writer.rs`); không vẽ EMF/WMF |
| 3.13 | Rust: aes, sha2, cbc | ✅ | `crates/bo-crypto/Cargo.toml`; `key_derivation_known_answers` |
| 3.14 | Rust: yrs, wasm-bindgen, serde | ✅ | `bo-collab`, `bo-wasm` |
| 3.15 | Rust: serde-wasm-bindgen | 🟡 | Không dùng; cầu nối bằng chuỗi JSON (`serde_json`, ADR-0002/0008) — phương án khác có lý do |
| 3.16 | Rust: chrono (serial date tự cài) | 🟡 | Không dùng chrono; ngày tự cài ở `crates/bo-formula/src/date.rs` (`date_and_serials`, `date_system_1904`) — đạt mục đích, khác thư viện |
| 3.17 | Test: insta, proptest, criterion | ✅ | `crates/bo-layout/tests/layout.rs` `snapshot_sample_layout`; proptest ở 12 crate; `crates/bo-layout/benches/layout.rs`, `bo-formula/benches/recalc.rs`, `bo-engine/benches/*` |
| 3.18 | TS: Zustand | ✅ | `packages/editor/package.json` |
| 3.19 | TS: TanStack Query | ❌ | Không có trong `package.json` nào |
| 3.20 | TS: Radix UI + CSS Modules + Lucide | 🟡 | Radix chỉ trong `packages/ui`; CSS Modules dùng một phần (7 file `*.module.css`, còn nhiều style inline) |
| 3.21 | TS: i18next (vi-VN mặc định, en-US), Vitest, Playwright | ✅ | `packages/i18n/test/i18n.test.ts` `'vi-VN and en-US define exactly the same keys'`; Vitest 109; Playwright 24 |
| 3.22 | .NET: Minimal API + WebSocket cho Yjs | ✅ | `apps/server/BlueOffice.Api/Program.cs`; `CollabTests.RelaysSyncUpdatesAndAwarenessBetweenClients` |
| 3.23 | .NET: EF Core + Npgsql, Minio SDK, StackExchange.Redis | ✅ | `BlueOffice.Infrastructure.csproj` (test chạy SQLite/filesystem/in-memory) |
| 3.24 | .NET: DocumentFormat.OpenXml (validate) | ✅ | `tools/BlueOffice.Validate` (job `fidelity`) |
| 3.25 | .NET: Serilog, OpenTelemetry, OIDC/JWT | ✅ | `BlueOffice.Api.csproj`; `Auth/AuthSetup.cs` |
| 3.26 | .NET: xUnit, Testcontainers | 🟡 | xUnit 108 test; **không có Testcontainers** — chưa có test tích hợp với PostgreSQL/MinIO/Redis thật |

### §3.4 Platform Abstraction Layer

| # | Yêu cầu | TT | Bằng chứng |
|---|---|---|---|
| 3.27 | Interface `Platform` đủ 10 thành viên | ✅ | `packages/platform/src/types.ts` (kind, files, storage, clipboard, fonts, print, engine, auth, window, offline, shell) |
| 3.28 | `files`: web upload/download + File System Access API; desktop ổ đĩa thật | 🟡 | Web `WebFileAccess` chỉ `<input>` + download (không File System Access API); desktop `DesktopFileAccess` ✅ (`desktop.test.ts` `'returns null when the file dialog is cancelled'`) |
| 3.29 | `storage`: Local / BlueOfficeServer / BlueDrive / Wopi provider | 🟡 | Chỉ `LocalStorageProvider` (`packages/storage-providers/test/local.test.ts`); chế độ nhúng dùng storage riêng trong `packages/hosted/src/session.ts`; chưa có 3 provider kia trong `storage-providers` |
| 3.30 | `clipboard`: web HTML + text; desktop native nhiều định dạng | 🟡 | `BrowserClipboard` chỉ text cho cả hai nền tảng (HTML nội bộ đi qua engine: `clipboard_internal_html_text`) |
| 3.31 | `fonts`: web font đóng gói + font server; desktop + font hệ thống | 🟡 | Chỉ font đóng gói (`BundledFontSource`, `NativeFontSource`); không có font server, không nạp font hệ thống (ADR-0003) |
| 3.32 | `print`: web PDF qua trình duyệt; desktop hộp thoại in OS | 🟡 | Web `WebPrint` ✅; desktop `print_pdf` (`apps/desktop/src-tauri/src/main.rs`) chỉ mở PDF bằng trình xem mặc định |
| 3.33 | `engine`: web WASM trong Web Worker; desktop native, fallback WASM | 🟡 | `packages/platform/src/web/engine.worker.ts`, `sheet.worker.ts` ✅; desktop `NativeEngineHost` ✅ nhưng **không có fallback WASM** |
| 3.34 | `auth`: OIDC redirect (web) / PKCE + deep link (desktop) | ❌ | `AnonymousSession` (stub) ở cả hai; chế độ nhúng dùng token BOHP |
| 3.35 | `window`: tab trình duyệt / nhiều cửa sổ native | ✅ | `DesktopWindow.openNew`; `main.rs` `window_labels_and_urls` |
| 3.36 | `offline`: IndexedDB + Service Worker (PWA); desktop thư mục cache | 🟡 | `IndexedDbCache` cho cả hai; **không có Service Worker/manifest PWA**; desktop không dùng thư mục cache |
| 3.37 | `shell`: file association, recent, deep link; web no-op | ✅ | `apps/desktop/src-tauri/src/shell.rs` `parses_bluedrive_link`, `rejects_bad_links`; recent dùng localStorage |
| 3.38 | `WebPlatform`/`DesktopPlatform`; phần còn lại dùng chung 100% | ✅ | `packages/platform/src/web/index.ts`, `desktop/index.ts` |
| 3.39 | `EngineHost` cùng API WASM/native | ✅ | `packages/platform/src/types.ts` `EngineHost`; `parity.test.ts` |
| 3.40 | PWA sửa offline, đồng bộ khi có mạng | 🟡 | Không có PWA; offline chỉ khi phiên cộng tác đang mở rồi kết nối lại (collab e2e bước "offline", không chạy lại) |
| 3.41 | Mỗi tính năng có E2E trên cả hai target trong CI | 🟡 | CI job `web` (Playwright) + job `desktop` (`pnpm test:e2e:desktop`); desktop chỉ 5 test (`tests/e2e-desktop/desktop.test.mjs`) so với 24 web + 4 collab |

### §3.5 Editor nhúng được

| # | Yêu cầu | TT | Bằng chứng |
|---|---|---|---|
| 3.42 | `@blueoffice/editor` độc lập + bản standalone chạy iframe | 🟡 | `packages/editor` (chỉ export source, không có library build); iframe là route `/edit` của `apps/web` |
| 3.43 | Nguồn file / người dùng / quyền / nơi lưu qua `DocumentStorage` + `AuthSession` | ✅ | `packages/hosted/src/session.ts`; `packages/hosted/test/session.test.ts` |
| 3.44 | Branding, menu File, Share, Close do host cấu hình | 🟡 | `HostConfig` (brandName, showShare, showClose, onShare, onClose) trong `BlueEditor`, nhưng route `/edit` không truyền vào; menu File không cấu hình được |

## §4. Cấu trúc repo

| # | Yêu cầu | TT | Bằng chứng |
|---|---|---|---|
| 4.1 | File gốc: `CLAUDE.md`, `THIRD_PARTY_NOTICES.md`, `Cargo.toml`, `package.json`, `pnpm-workspace.yaml`, `BlueOffice.sln` | ✅ | Thư mục gốc |
| 4.2 | 19 crate theo danh sách (bo-model … bo-cli) | ✅ | `crates/` có đủ, thêm `bo-sheet`, `bo-sign`, `bo-encoding` |
| 4.3 | `apps/web`, `apps/desktop`, `apps/server/{Api,Domain,Infrastructure,Tests}` | ✅ | `apps/` |
| 4.4 | `packages/{platform,editor,editor-sdk,storage-providers,ui,canvas-host,i18n}` | ✅ | `packages/` (thêm `collab`, `hosted`, `sheet-editor`, `slide-editor`, `engine-wasm`) |
| 4.5 | `tests/{corpus,fidelity,e2e}` | ✅ | `tests/` (thêm `e2e-desktop`, `mock-bluedrive`) |
| 4.6 | `docs/{ARCHITECTURE.md,PROGRESS.md,adr,plan,spec-notes}` | 🟡 | Có đủ thư mục; `docs/spec-notes/` chỉ có `README.md` (chưa có ghi chú đọc spec theo phần tử) |
| 4.7 | `bo-cli`: convert, render, validate, fidelity-report | 🟡 | `convert`, `render`, `inspect`, `roundtrip`, `sign/verify`, `bench`; validate là `tools/BlueOffice.Validate` (.NET), fidelity-report là `scripts/fidelity-report.mjs` — không phải lệnh của `bo-cli`; `bo-cli` không có test riêng |

## §5. Nguyên tắc thiết kế cốt lõi

| # | Yêu cầu | TT | Bằng chứng |
|---|---|---|---|
| 5.1 | Lossless: giữ phần tử/thuộc tính chưa hiểu tại chỗ (kể cả `mc:AlternateContent`, `w14:*`, `x14:*`, extLst); xoá cha thì xoá opaque con | ✅ | ADR-0005; `crates/bo-docx/src/writer.rs` `unchanged_document_round_trips_byte_identical_parts`; `crates/bo-docx/tests/corpus.rs` `editing_one_paragraph_keeps_other_parts`; `crates/bo-xlsx/tests/roundtrip.rs` `unknown_fragments_survive_in_place`; `crates/bo-drawingml/src/props.rs` `sp_pr_roundtrip_keeps_unknown_in_place` |
| 5.2 | Mô hình bất biến + Transaction/Op → undo/redo không giới hạn, 1-1 với CRDT | ✅ | ADR-0008; `crates/bo-edit/src/tests.rs` `ops_are_checked_and_invertible`, `undo_all_restores_original`; `crates/bo-engine/src/collab.rs` `run_attributes_round_trip` |
| 5.3 | Đơn vị spec (twips, EMU, half-points); pixel chỉ ở render, có zoom + DPR | ✅ | `bo_model::{Twips, HalfPoints}`; `crates/bo-engine` toạ độ point; e2e `'zoom changes the page size and the status bar'` |
| 5.4 | Layout tăng dần + cache shaping | 🟡 | `crates/bo-layout/src/incremental.rs`; `typing_in_300_page_document_under_16_ms` (`resynced: true`); vẫn dàn lại toàn bộ khi có NUMPAGES trong thân bài, đổi style/numbering/section, sửa header/footer (`docs/plan/phase-2.md`) |
| 5.5 | Display list chung cho Canvas/PNG/PDF | ✅ | `crates/bo-render/src/lib.rs` `json_shape`; `bo-raster` `renders_rect_and_size`; `bo-pdf` `vector_commands_and_images`; `packages/canvas-host/test/renderer.test.ts` |
| 5.6 | Ảo hoá: chỉ vẽ vùng thấy + đệm; bảng tính sparse | ✅ | `DocumentView.ts` (vẽ trang trong viewport, `schedule` khi cuộn); `crates/bo-sheet/src/viewport.rs` `visible_range_without_freeze`; `perf.spec.ts` kiểm trang > 5 được vẽ sau khi cuộn |
| 5.7 | Đa luồng: web Web Worker; desktop thread pool (rayon) | 🟡 | Web Worker ✅ (`engine.worker.ts`); công thức tính song song bằng `std::thread::scope` (`crates/bo-formula/src/workbook.rs` `eval_parallel`, `large_levels_with_dynamic_references`); **không có rayon/thread pool cho layout/render trên desktop** |
| 5.8 | Command registry (id, ribbon/phím tắt/menu/palette, enabled/checked từ selection) | ✅ | `packages/editor/src/commands.ts`; `packages/ui/test/ribbon.test.tsx` `'renders tabs and dispatches enabled commands only'`; `packages/editor/test/keys.test.ts` |
| 5.9 | An toàn: chống zip bomb, giới hạn độ sâu XML, không chạy macro, không tải tài nguyên ngoài | ✅ | `crates/bo-opc/src/package.rs` `rejects_zip_bomb`; `bo_opc::xml::MAX_DEPTH` (256); `crates/bo-xlsx/tests/roundtrip.rs` `rejects_zip_bomb_and_deep_xml`; `security.rs` `macros_are_reported_and_kept`; reader bỏ qua target `External` (`crates/bo-docx/src/reader.rs` `target`); fuzz `crates/bo-engine/tests/fuzz_smoke.rs` (mới, lồng sâu 5 000 tầng trên stack 2 MiB) |

### Mục tiêu hiệu năng (đo trong lần kiểm này — số liệu chi tiết ở mục "Số đo")

| # | Mục tiêu | TT | Kết quả · bằng chứng |
|---|---|---|---|
| 5.10 | Mở `.docx` 300 trang có ảnh < 3 s (desktop) | ✅ | **0,42–0,44 s** mở + layout + display list trang 1 (release native, file 14,5 MB, 300 ảnh 640×480). `bo-cli sample-big` + `bo-cli bench` (mới) |
| 5.11 | Mở `.docx` 300 trang có ảnh < 5 s (web) | ✅ | **0,94 s** từ chọn file tới trang 1 vẽ xong trong Chromium (`tests/e2e/specs/perf.spec.ts`, mới); WASM thuần trong Node 0,74–1,27 s (`scripts/bench-open-wasm.mjs`, mới) |
| 5.12 | Gõ phím → hiển thị < 16 ms ở 300 trang | ✅ | Native 1,2–1,8 ms (trường hợp xấu nhất 7,7 ms) — `crates/bo-engine/tests/typing_perf.rs` `typing_in_300_page_document_under_16_ms` (`--release --ignored`); WASM 2,8–3,8 ms — `scripts/bench-typing-wasm.mjs` |
| 5.13 | Mở `.xlsx` 1 triệu ô / 100 000 công thức < 5 s | 🟡 | Native **1,75–1,99 s** (lúc máy rảnh hơn; 3,1–4,1 s khi tải nặng) ✅; **WASM 6,1–7,7 s ❌** (`bo-cli sample-big-sheet` + `bench`, `bench-open-wasm.mjs`, mới). Criterion `crates/bo-formula/benches/recalc.rs` đo trong bộ nhớ (không đọc file) |
| 5.14 | Sửa 1 ô → tính lại phụ thuộc < 100 ms | ✅ | Ô trong bảng VLOOKUP (10 000 phụ thuộc): native 45–84 ms (lên tới 219 ms khi máy tải nặng), WASM 64–67 ms; ô thường 9–21 ms. `crates/bo-engine/benches/sheet_edit.rs`, `bo-formula/benches/recalc.rs` |
| 5.15 | Cuộn 60 fps | ✅ | **60,0 fps**, p95 16,8 ms, 0 khung > 33 ms khi cuộn ~2 400 px/s qua 300 trang (`perf.spec.ts`, Chromium headless) |
| 5.16 | Bộ nhớ WASM < 1 GB cho các file trên | ✅ | `.docx` 300 trang có ảnh: **80 MB**; `.xlsx` 1 triệu ô: **399 MB** (`WebAssembly.Memory` sau khi mở, `bench-open-wasm.mjs`) |
| 5.17 | Đo bằng benchmark **trong CI** | 🟡 | Chỉ `perf.spec.ts` chạy trong job `web` (giới hạn cứng khi `BO_PERF_STRICT=1`); criterion bench, `typing_perf` (ignored) và `bench-open-wasm.mjs` chưa chạy trong CI |

## §6. Lộ trình theo phase

### Phase 1 — Nền tảng

| # | Yêu cầu | TT | Bằng chứng |
|---|---|---|---|
| P1.1 | Rust/pnpm workspace, .NET solution | ✅ | `Cargo.toml`, `pnpm-workspace.yaml`, `BlueOffice.sln` |
| P1.2 | CI: build, test, clippy, rustfmt, eslint, prettier, dotnet format, cargo deny | ✅ | `ci.yml` jobs `rust` (fmt, clippy `-D warnings`, test, clippy wasm32, build wasm; Ubuntu + Windows), `cargo-deny`, `web` (`pnpm lint` = eslint + prettier, typecheck, test, license-check, build, E2E), `dotnet` (`dotnet format --verify-no-changes`, test), `fidelity`, `desktop` |
| P1.3 | `bo-opc` đọc/ghi, giữ part không hiểu | ✅ | `crates/bo-opc/src/package.rs` `roundtrip_preserves_all_parts`; `tests/proptest_roundtrip.rs` `arbitrary_parts_roundtrip` |
| P1.4 | `bo-text`: font manager (font hệ thống + font kèm app), fallback, rustybuzz, metric | 🟡 | `crates/bo-text/tests/fonts.rs` `metric_compatible_fallback`, `itemize_falls_back_for_missing_glyphs`, `outlines`; **không nạp font hệ thống** |
| P1.5 | `bo-layout` v1: nhiều run, căn 4 kiểu, giãn dòng, thụt lề, ngắt dòng Unicode, ngắt trang | ✅ | `crates/bo-layout/tests/layout.rs` `alignment_positions`, `justify_fills_all_but_last_line`, `indents_and_hanging`, `line_spacing_rules`, `pagination_and_breaks`, `overlong_word_is_broken` |
| P1.6 | `bo-render` + `bo-raster` + `bo-pdf` | ✅ | `json_shape_vector_commands`, `clip_limits_painting`, `gradient_and_image`, `empty_document_is_valid_pdf` |
| P1.7 | `bo-wasm` + `canvas-host`: hiển thị, zoom, cuộn | ✅ | e2e `'opens the sample .docx and renders it with the WASM engine'`, `'zoom changes the page size and the status bar'`; cuộn: `perf.spec.ts` (mới) |
| P1.8 | Shell UI: ribbon tab, thanh trạng thái, sáng/tối, đa ngôn ngữ | ✅ | `packages/ui/src/Ribbon.tsx`, `StatusBar.tsx`; e2e `'switches to English and dark theme'`; `editor.test.tsx` `'switches language and theme through commands'` |
| P1.9 | `packages/platform` đủ interface, WebPlatform/DesktopPlatform (files, engine, fonts, clipboard) | ✅ | Xem 3.27–3.39 |
| P1.10 | Cùng file mẫu trên web và desktop hiển thị giống nhau (test visual 2 target) | 🟡 | `parity.test.ts` so display list WASM ↔ native (`bo-cli dump`, cùng `bo-engine` với Tauri) **từng byte** trên mọi trang; vì renderer canvas dùng chung nên tương đương hiển thị. Chưa có so sánh **ảnh** giữa web và cửa sổ Tauri thật |
| P1.11 | `packages/editor` tách khỏi `apps/web`; web/desktop chỉ là vỏ | ✅ | `packages/editor/src/BlueEditor.tsx`; `apps/web/src/main.tsx`, `apps/desktop/src/main.tsx` |
| P1.12 | `DocumentStorage` + `LocalStorageProvider` | ✅ | `local.test.ts` `'opens through FileAccess with full local permissions'`, `'saves and save-as with MIME types'` |
| P1.13 | `bo-cli render input.docx --out page-%d.png`, `convert --to pdf` | ✅ | `crates/bo-cli/src/main.rs` `Cmd::Render`/`Convert`; dùng trong CI job `fidelity` (`scripts/visual-diff.mjs`) và server `ConvertsSampleWithRealCli` (.NET ✅); không có test Rust riêng cho `bo-cli` |

### Phase 2 — BlueDoc MVP: đọc/ghi `.docx`

| # | Yêu cầu | TT | Bằng chứng |
|---|---|---|---|
| P2.1 | styles.xml: docDefaults, basedOn, style đoạn/ký tự/bảng/numbering, giữ latentStyles | ✅ | `crates/bo-docx/src/reader.rs` (docDefaults, latentStyles giữ nguyên); corpus `styles-01..03`; `bo-edit` `paragraph_formatting_lists_and_styles` |
| P2.2 | Run: font ascii/hAnsi/eastAsia/cs | 🟡 | ascii và hAnsi gộp một trường (`reader.rs`) → hAnsi khác ascii mất khi model dựng lại; eastAsia/cs đọc/ghi nhưng layout không dùng; theme font (`asciiTheme`) chưa phân giải |
| P2.3 | Run: size, bold, italic, strike, màu, highlight, shading, sup/sub, caps, small caps, spacing | ✅ | `bo-layout/tests/features.rs` `paragraph_borders_shading_and_highlight`; `bo-edit` `character_formatting_toggles`; corpus `text-01`, `text-02` |
| P2.4 | Run: underline các kiểu | 🟡 | `Underline` có single/double/thick/dotted/dash/wave/words; thiếu dotDash, dotDotDash, dashLong, wavyDouble, các biến thể heavy |
| P2.5 | Run: kerning | 🟡 | `w:kern` đọc/ghi nhưng layout không áp dụng |
| P2.6 | Đoạn: căn lề, thụt lề, spacing, line auto/exact/atLeast, keepNext, keepLines, widowControl, pageBreakBefore, tab, border, shading | ✅ | `layout.rs` `line_spacing_rules`; `features.rs` `keep_next_moves_heading`, `widow_control_never_leaves_single_lines`; `bo-edit/src/feature_tests.rs` `indents_line_spacing_and_tabs`; corpus `para-01..03` |
| P2.7 | numbering: nhiều cấp, abstractNum, lvlOverride, định dạng số, nối tiếp/khởi động lại | ✅ | `reader.rs` (abstractNum/lvlOverride/startOverride); corpus `lists-01..03-restart-override` (round-trip); không có test layout riêng cho upperRoman/lvlOverride |
| P2.8 | Section: khổ, hướng, lề, cột, header/footer first/even/default, các loại section break | ✅ | `features.rs` `two_columns`, `sections_page_setup_and_continuous`, `page_number_restart_and_even_headers`, `headers_footers_and_page_fields`; corpus `sections-01..04` |
| P2.9 | Bảng: grid, gộp ngang/dọc, độ rộng auto/fixed/pct, border, shading, lặp tiêu đề, lồng nhau, ngắt hàng qua trang | 🟡 | `features.rs` `table_rows_and_repeated_header`, `tall_row_breaks_across_pages`, `merged_cells_and_shading`; corpus `tables-01..03`; `w:hMerge` (gộp ngang kiểu cũ) chỉ giữ nguyên, không hiểu |
| P2.10 | Ảnh inline/anchored (wrap square/tight/top-bottom/behind/in-front), crop, xoay | ✅ | `features.rs` `inline_and_anchored_pictures`, `square_and_tight_wrapping_flow_beside_pictures`; `renderer.test.ts` `'draws pictures (crop, rotation), placeholders and dashed lines'`; corpus `images-01..06` (wrap tight dùng khung bao đa giác) |
| P2.11 | Hyperlink, bookmark, footnote/endnote, field PAGE/NUMPAGES/DATE/TOC (kết quả cache) | ✅ | `features.rs` `numpages_in_body_uses_second_pass`, `footnotes_at_page_bottom`, `footnote_numbering_restarts_per_section_and_page`; corpus `links-01..03`, `notes-01..04`, `fields-01..06` |
| P2.12 | Text box, shape đơn giản | ❌ | Không có mô hình/hiển thị `wps:wsp`/`w:txbxContent` (chỉ giữ opaque); corpus không có file loại này |

### Phase 2 — Chỉnh sửa

| # | Yêu cầu | TT | Bằng chứng |
|---|---|---|---|
| P2.13 | Con trỏ, chọn vùng (chuột, Shift+mũi tên, Ctrl+A, double/triple click), gõ, xoá, IME | ✅ | `crates/bo-engine/src/edit.rs` (click count 1/2/3); `bo-edit` `typing_vietnamese_and_undo_by_word`, `delete_by_grapheme_and_merge`, `navigation_helpers`; `keys.test.ts` `'maps caret and editing keys'`; e2e `'Enter, Backspace and arrows edit paragraphs'`; chưa có e2e double/triple click |
| P2.14 | Copy/paste nội bộ, HTML, plain text, Paste Special | ✅ | `bo-edit` `clipboard_internal_html_text`; `html.rs` `imports_inline_formatting_and_paragraphs`, `tolerates_garbage`; `feature_tests.rs` `paste_special_modes` |
| P2.15 | Định dạng ký tự & đoạn, style gallery, Format Painter, Clear Formatting | 🟡 | `feature_tests.rs` `format_painter_and_clear_formatting`; e2e `'font picker, size, colour, format painter; kept after save and reopen'`; style gallery chỉ 4 style cố định (`BlueEditor.tsx`), không liệt kê style của tài liệu |
| P2.16 | Find/Replace: hoa thường, cả từ, regex, theo định dạng | 🟡 | `crates/bo-edit/src/find.rs` (`match_case`, `whole_word`, `regex`); `bo-edit` `find_and_replace`; **chưa tìm theo định dạng** |
| P2.17 | Chèn: bảng, ảnh, ngắt trang, ngắt section, ký tự đặc biệt, header/footer, số trang | ✅ | `bo-edit` `insert_table_image_breaks`, `header_footer_options_and_notes`; e2e `'table: insert, Tab adds a row, ribbon row insert; kept after reopen'`, `'header: double-click to edit, page number, kept after reopen'` |
| P2.18 | Thước ngang/dọc, kéo thả tab stop | ✅ | `packages/editor/src/Ruler.tsx`; `controls.test.tsx` `'shows margins, indents and tab stops; the tab kind cycles'`; e2e `'ruler: drag the left indent and add a tab stop; kept after reopen'` |
| P2.19 | Undo/redo, autosave, khôi phục khi crash | ✅ | `bo-edit` `undo_all_restores_original`; `editor.test.tsx` `'opens the find panel and offers crash recovery'` (autosave chỉ kiểm gián tiếp) |
| P2.20 | Xuất PDF; in (desktop hộp thoại OS; web in PDF) | 🟡 | PDF ✅ (e2e `'exports a PDF'`); web in PDF qua iframe ✅; desktop chưa mở hộp thoại in của OS |
| P2.21 | Tiêu chí: 95% corpus `tests/corpus/doc` round-trip không mất dữ liệu | ✅ | `crates/bo-docx/tests/corpus.rs` `corpus_round_trips_losslessly` (assert ≥ 95 %; corpus 55 file tự sinh) |
| P2.22 | Tiêu chí: visual diff trung bình < 2 % pixel | 🟡 | `scripts/visual-diff.mjs`, `tests/fidelity/thresholds.json` (`default: 0.02`), `docs/fidelity-visual.md`: so với baseline tự sinh 0 trang vượt ngưỡng; **`tests/fidelity/reference/` (ảnh từ phần mềm gốc) chưa có** nên chưa đo được độ giống |

### Phase 3 — BlueSheet MVP

| # | Yêu cầu | TT | Bằng chứng |
|---|---|---|---|
| P3.1 | workbook, worksheets, sharedStrings (rich text) | ✅ | `crates/bo-xlsx/tests/roundtrip.rs` `reads_every_modelled_feature`, `model_roundtrip_is_lossless` |
| P3.2 | styles: numFmts, fonts, fills, borders, cellXfs, cellStyles, dxfs | ✅ | `reads_every_modelled_feature`; corpus `styles-1..3.xlsx` |
| P3.3 | merge, độ rộng cột/chiều cao hàng, ẩn hàng/cột, outline/group, freeze/split | ✅ | `crates/bo-sheet/src/rowcol.rs` `column_widths_split_and_merge_records`, `rows_hide_and_freeze`, `merge_and_unmerge`; corpus `panes-*`, `layout-*` |
| P3.4 | defined names (2 phạm vi), print area, print titles, page setup | ✅ | `reads_every_modelled_feature`; corpus `names-*`, `page-*` |
| P3.5 | hyperlink, comments, data validation, CF (cellIs, expression, colorScale, dataBar, iconSet) | ✅ | `crates/bo-sheet/src/cf.rs` `cell_is_and_expression_rules`, `scales_bars_and_icons`; `validation.rs` `lists_inline_and_from_range` |
| P3.6 | Table (ListObject) + structured references | ✅ | `crates/bo-formula/tests/engine.rs` `structured_references`, `tables_follow_structural_changes` |
| P3.7 | Chart: chartSpace → model → render | ✅ | `crates/bo-xlsx/src/chart.rs` `parses_combo_chart`; `bo-sheet/src/chart.rs` `all_types_draw_and_live_values_follow_cells` |
| P3.8 | Định dạng số: 4 section, `[>100]`, `[Red]`, ngày giờ, %, phân số, khoa học, `@`, `[$-42A]`, ₫ | ✅ | `crates/bo-sheet/src/numfmt/tests.rs` `sections_conditions_and_colors`, `percent_scientific_fraction`, `dates_and_times`, `text_section`, `locale_and_currency_tags`, `malformed_codes_do_not_panic`. Lưu ý: hàm `TEXT()` dùng bộ định dạng riêng (`bo-formula/src/numfmt.rs`) bỏ qua `[cond]`/`[color]` |
| P3.9 | Parser A1 & R1C1, tham chiếu tương đối/tuyệt đối/hỗn hợp, range, union, intersection, sheet khác, 3D, names, structured refs, array constants | ✅ | `engine.rs` `reference_forms`, `sheet_names_with_spaces`, `r1c1_parse_and_format`, `defined_names_scopes`, `implicit_intersection`; `proptest_roundtrip.rs` `parse_format_parse_round_trip`, `r1c1_format_parse_round_trip` |
| P3.10 | Kiểu giá trị + 9 lỗi (#DIV/0! … #CALC!) | ✅ | `engine.rs` `error_values` |
| P3.11 | Ép kiểu ngầm định | ✅ | `engine.rs` `operators_precedence_and_coercion`, `comparisons` |
| P3.12 | Dependency graph, tính lại tăng dần, song song nhánh độc lập, phát hiện vòng + iterative | ✅ | `engine.rs` `incremental_recalculation_touches_only_dependents`, `evaluation_order_is_topological`, `circular_references`, `iterative_calculation`; `proptest_recalc.rs` `incremental_matches_full_recalc`; song song `workbook.rs` `eval_parallel` (tắt trên wasm32) |
| P3.13 | Hàm volatile NOW/TODAY/RAND/OFFSET/INDIRECT | ✅ | `engine.rs` `volatile_functions_recalculate` |
| P3.14 | Dynamic arrays & spill, LET, LAMBDA | ✅ | `engine.rs` `spill_and_spill_errors`, `overlapping_spills_block`; `fn_logical.rs` `let_lambda`, `named_lambda` |
| P3.15 | Serial date 1900 (29/02/1900 giả) và 1904 | ✅ | `fn_datetime.rs` `date_and_serials`, `date_system_1904` |
| P3.16 | So sánh 15 chữ số có nghĩa | ✅ | `engine.rs` `comparisons` (`=0.1+0.2=0.3` → TRUE) |
| P3.17 | Math & Trig (danh sách prompt) | ✅ | `crates/bo-formula/src/functions/math.rs`; `tests/fn_math.rs` — đủ mọi tên được liệt kê |
| P3.18 | Logical | ✅ | `functions/logical.rs`; `tests/fn_logical.rs` |
| P3.19 | Lookup & Reference | ✅ | `functions/lookup.rs`; `tests/fn_lookup.rs` |
| P3.20 | Text | ✅ | `functions/text.rs`; `tests/fn_text.rs` |
| P3.21 | Date & Time | ✅ | `functions/datetime.rs`; `tests/fn_datetime.rs` |
| P3.22 | Statistical | ✅ | `functions/stats.rs`; `tests/fn_stats.rs` |
| P3.23 | Financial | ✅ | `functions/financial.rs`; `tests/fn_financial.rs` |
| P3.24 | Information | ✅ | `functions/info.rs`; `tests/fn_info.rs` (tổng 276 hàm đăng ký) |
| P3.25 | Mỗi hàm một file test (bảng công thức/input/kỳ vọng, lỗi + biên), kỳ vọng do người kiểm thử cung cấp, ghi nguồn | 🟡 | Một file **mỗi nhóm** (`tests/fn_*.rs`, harness `tests/common/mod.rs`), cả 276 hàm có ít nhất một dòng; nguồn ghi ở mức file/test, không theo dòng; kỳ vọng do tác giả tự tính lại (vd `fn_financial.rs`), chưa có bộ kỳ vọng do người kiểm thử cung cấp |
| P3.26 | Grid ảo hoá canvas, thanh công thức, Name Box | ✅ | `crates/bo-sheet/src/viewport.rs` `frozen_panes_and_hit_test`; `packages/sheet-editor/test/grid.test.ts` `'parses Name Box input'` |
| P3.27 | Tab sheet: thêm/xoá/đổi tên/kéo thả/màu tab | 🟡 | Thêm/xoá/đổi tên ✅ (`SheetEngine::add_sheet/rename_sheet/delete_sheet`); **chưa kéo thả đổi thứ tự, chưa đặt màu tab** (đội khác đang làm phần còn lại của BlueSheet) |
| P3.28 | Chọn nhiều vùng bằng Ctrl | ❌ | `packages/sheet-editor/src/grid.ts` `Selection` chỉ một vùng |
| P3.29 | Autofill (số, ngày, thứ, tháng, danh sách tự định nghĩa) | ✅ | `crates/bo-sheet/src/fill.rs` `numbers`, `dates`, `lists_and_text`, `autofill_down_right_and_up` |
| P3.30 | Chèn/xoá hàng cột cập nhật tham chiếu | ✅ | `crates/bo-sheet/src/ops.rs` `insert_rows_moves_everything_below`; `bo-sheet/tests/formula_engine.rs` `insert_delete_adjust_references` |
| P3.31 | Copy/paste điều chỉnh tham chiếu; Paste Special values/formats/formulas/transpose | ✅ | `crates/bo-engine/src/sheet/cmd.rs` `find_replace_goto_and_paste_special`, `cut_paste_moves_references_and_paste_fills_selection` |
| P3.32 | Sort nhiều cấp, AutoFilter, Find/Replace, Go To | ✅ | `crates/bo-sheet/src/sort.rs` `multi_level_with_header_and_blanks`; `filter.rs` `apply_sets_row_flags_and_lists_values`; e2e `'BlueSheet: numbers + SUM, bold + fill, sort, save .xlsx and reopen'` (flaky khi tải nặng) |
| P3.33 | Format Cells, cell styles, CF, data validation dropdown | ✅ | `packages/sheet-editor/src/FormatCellsDialog.tsx`; `bo-sheet/src/style.rs` `named_styles_painter_and_clear`; `cmd.rs` `conditional_formats_and_validation`; `editor.test.tsx` `'formats cells from the ribbon, shortcuts and the Format Cells dialog'` |
| P3.34 | Biểu đồ cột/thanh/đường/vùng/tròn/doughnut/scatter/combo; sửa tiêu đề/trục/chú giải | ✅ | `crates/bo-sheet/src/chart.rs` (8 loại); `cmd.rs` `charts_insert_edit_save` (chart neo tuyệt đối chưa kéo được) |
| P3.35 | Xuất PDF theo page setup, in tiêu đề lặp | ✅ | `crates/bo-engine/src/sheet/print.rs` `titles_and_spans`; `cmd.rs` `page_setup_drives_pdf_pages` |
| P3.36 | Tiêu chí: 100 % test hàm đợt 1 pass | ✅ | `cargo test -p bo-formula` (`fn_math` … `fn_info`) xanh |
| P3.37 | Tiêu chí: round-trip 95 % corpus | ✅ | `crates/bo-xlsx/tests/corpus.rs` `corpus_roundtrip` (assert ≥ 95 %, 60 file tự sinh) |
| P3.38 | Tiêu chí: benchmark mục 5 | 🟡 | Native đạt; mở 1 triệu ô trên WASM 6,1–7,7 s (> 5 s) — xem 5.13 |

### Phase 4 — BlueSlide MVP

| # | Yêu cầu | TT | Bằng chứng |
|---|---|---|---|
| P4.1 | `.pptx` r/w: presentation, master, layout, slide, notes | ✅ | `crates/bo-pptx/tests/roundtrip.rs` `corpus_round_trip_is_lossless`, `corpus_models_features`, `edited_deck_round_trips` |
| P4.2 | Kế thừa placeholder master → layout → slide | ✅ | `crates/bo-pptx/src/inherit.rs`; `corpus_models_features` |
| P4.3 | Theme: color/font/format scheme | ✅ | `crates/bo-drawingml/src/theme.rs` `parse_and_roundtrip`; `color.rs` `scheme_through_clr_map`, `lum_mod_off` |
| P4.4 | Text: autofit (normAutofit, shrink), bullet nhiều cấp, cột trong text box | ✅ | `crates/bo-engine/tests/slide_rich.rs` `shrink_on_overflow_records_font_scale`, `paragraph_formats_levels_and_spacing`; snapshot `slides__slide-text-columns-*` |
| P4.5 | Toàn bộ preset geometry (Annex presetShapeDefinitions) | ✅ | `crates/bo-drawingml/src/presets.dsl` 187 mục; `tests/presets.rs` `full_annex_preset_set`, `every_preset_evaluates_at_odd_sizes` |
| P4.6 | Custom geometry | ✅ | `props.rs` `cust_geom_roundtrip`; `geom.rs` `formulas` |
| P4.7 | Fill solid/gradient/pattern/picture, line, shadow | ✅ | `crates/bo-pptx/src/render.rs`; snapshot `slides__slide-images-fill-1`, `slides__slide-shapes-*`; `slide_rich.rs` `blurred_shadows_and_picture_transparency_render` (chưa có 3D) |
| P4.8 | Group, connector | ✅ | `slide_rich.rs` `connectors_attach_to_sites_and_follow_shapes`, `connector_inside_a_group_follows_its_shapes`, `ungroup_keeps_flips` |
| P4.9 | Ảnh | ✅ | `slide_editing.rs` `insert_picture_adds_media_part`; e2e `'pastes a slide copied from another deck, with its pictures; kept after reopen'` |
| P4.10 | Video/audio: giữ nguyên + phát nếu codec hỗ trợ | 🟡 | Giữ nguyên (`corpus_models_features`: `/ppt/media/media1.mp4`); **không phát** (không có `<video>/<audio>` trong `packages/slide-editor`) |
| P4.11 | Bảng | ✅ | `slide_rich.rs` `table_cells_rows_columns_merge_fill_and_style`; e2e `'bolds part of a shape text, copies/pastes a shape, edits a table cell, saves and reopens'` |
| P4.12 | Chart (dùng chung `bo-drawingml`) | 🟡 | Đọc + vẽ: `bo-drawingml/src/chart.rs` `bar_chart_cache`, snapshot `slides__slide-charts-*`; chưa chèn/sửa chart trong BlueSlide |
| P4.13 | Normal (thumbnail + slide + notes) | ✅ | `packages/slide-editor/test/slide.test.tsx` `'opens a deck, shows thumbnails and notes, navigates and saves'` |
| P4.14 | Slide Sorter | ✅ | `slide.test.tsx` `'switches to the slide sorter and back'` |
| P4.15 | Reading View | 🟡 | Có code (`BlueSlideEditor.tsx` `view === 'reading'`), chưa có test |
| P4.16 | Trình chiếu toàn màn hình | ✅ | `slide.test.tsx` `'maps keys and steps through the deck'` |
| P4.17 | Presenter View (màn hình 2: slide kế, ghi chú, đồng hồ) | ✅ | `slide.test.tsx` `'runs the presenter view with next slide, notes and timer, Esc exits'`, `'opens a second window and keeps it on the presenter slide'` |
| P4.18 | Transition & animation (fade, push, wipe, appear, fly-in), timeline | 🟡 | Model/IO: `crates/bo-pptx/src/anim.rs` `transition_alternate_content`, `generated_timing_parses_back`; **khi trình chiếu không chạy hiệu ứng** (chỉ fade CSS chung), chưa có UI đặt transition/animation |
| P4.19 | Smart guides, căn chỉnh/phân bố, nhóm/bỏ nhóm, thứ tự lớp | ✅ | `crates/bo-engine/src/slide_edit/guides.rs` `snaps_to_slide_centre`; `slide_editing.rs` `arrange_group_align_distribute`, `random_edits_then_undo_all_restore_the_deck`; e2e `'inserts a shape, shows smart guides, adds a slide and edits notes'` |
| P4.20 | Xuất PDF, PNG từng slide | 🟡 | `slides.rs` `slide_exports_pdf_png_and_saves`; PNG có ở engine/WASM/CLI nhưng UI BlueSlide chưa có lệnh xuất PNG |

### Phase 5 — Định dạng cũ & tính năng nâng cao

| # | Yêu cầu | TT | Bằng chứng |
|---|---|---|---|
| P5.1 | `.doc` (FIB, piece table, CHP/PAP/SEP, stylesheet, list) → model → lưu OOXML | ✅ | `crates/bo-binary/src/doc.rs` `reads_built_doc`, `sprm_sizes`, `number_formats`, `mutated_doc_never_panics` (chỉ file `.doc` tự dựng, không có file thật trong corpus) |
| P5.2 | `.xls` (BIFF8, SST, formula tokens) → lưu `.xlsx` | ✅ | `xls.rs` `reads_sample_workbook`; `xls_formula.rs` `functions_and_strings`, `three_d_and_names`, `shared_relative`, `array_constants` |
| P5.3 | `.xlsb` | ✅ | `xlsb.rs` `reads_sample`; `xls_formula.rs` `biff12_tokens` |
| P5.4 | `.ppt` → bo-model → lưu OOXML | 🟡 | Chỉ chữ (`ppt.rs` `reads_sample_deck`); không có shape/ảnh, không lưu `.pptx` |
| P5.5 | bo-crypto: mở/lưu Agile + Standard | 🟡 | `crates/bo-crypto/src/lib.rs` `agile_round_trip`, `standard_round_trip`; `tests/interop.rs` `decrypts_a_package_encrypted_by_msoffcrypto_tool`; `security.rs` `password_protected_open_and_save`; **lưu có mật khẩu chỉ `.docx`**, BlueSheet không mở/lưu file có mật khẩu |
| P5.6 | Bảo vệ sheet/workbook | 🟡 | Chỉ hàm băm (`bo-crypto/src/protection.rs` `legacy_known_answers`, `modern_round_trip`); `sheetProtection` giữ nguyên dạng opaque; không áp khoá ô, không UI |
| P5.7 | Track changes ins/del/rPrChange/pPrChange: hiển thị, accept/reject, ghi mới | 🟡 | `crates/bo-edit/src/revisions.rs`; `bo-edit` `track_changes_records_insertions_and_deletions`, `track_changes_paragraph_marks_and_formatting`, `tracked_edits_reject_restores_text`; e2e `'track changes: record edits, reject all, accept all'`. Chưa ghi: chèn/xoá cột-ô, gộp/tách ô, header/footer, nội dung chú thích, ảnh nổi đổi vị trí (**đội khác đang làm**) |
| P5.8 | Comments có luồng trả lời | ✅ | `bo-edit` `comments_threads_resolve_delete_and_undo`; `editing.rs` `comment_threads_save_and_reopen`; e2e `'comments: add, reply, resolve and delete'` (chỉ BlueDoc) |
| P5.9 | TOC cập nhật, caption, cross-reference | ✅ | `crates/bo-engine/tests/editing.rs` `toc_captions_and_cross_references`, `update_fields_on_corpus_files` |
| P5.10 | Mail merge (CSV/XLSX) | ✅ | `editing.rs` `mail_merge_csv_and_xlsx`, `mail_merge_corpus_templates`; `bo-cli merge` |
| P5.11 | PivotTable: đọc cache + hiển thị, tạo cơ bản | ✅ | `crates/bo-sheet/src/pivot/tests.rs` `rows_and_values`, `refresh_follows_source_changes`; `cmd.rs` `pivot_create_update_save_reopen_refresh`; e2e `'BlueSheet: create a pivot table, change the aggregation, save and reopen'` (biểu đồ pivot: **đội khác đang làm**) |
| P5.12 | SmartArt: drawing fallback | 🟡 | BlueDoc ✅ (`security.rs` `smartart_shows_its_drawing_fallback`); BlueSlide/BlueSheet chưa vẽ fallback |
| P5.13 | ODF đọc/ghi | 🟡 | `crates/bo-odf` `round_trip` (odt/ods/odp), `tests/fuzz.rs`; editor không lưu ODF (chỉ `bo-cli convert`), BlueSlide không mở `.odp` |
| P5.14 | TCVN3/VNI → Unicode | ✅ | `bo-encoding` `exhaustive_round_trip`, `tcvn3_table_is_injective_and_nfc`, `detection` (xem 2.24) |
| P5.15 | VBA: giữ nguyên khi lưu, cảnh báo, không thực thi | 🟡 | `bo-binary/src/vba.rs` `detects_and_preserves_vba`; `security.rs` `macros_are_reported_and_kept`; cảnh báo `macro-warning` chỉ trong BlueDoc (chưa có ở BlueSheet/BlueSlide) |

### Phase 6 — Cộng tác, server & triển khai

| # | Yêu cầu | TT | Bằng chứng |
|---|---|---|---|
| P6.1 | `bo-collab`: Yrs cho văn bản, ô bảng tính, đối tượng slide | 🟡 | `crates/bo-collab/tests/sync.rs` `concurrent_edits_converge`, `undo_only_reverts_local_changes`, `shapes_and_presence`; `bo-engine` `concurrent_typing_in_one_paragraph_converges`, `formatting_and_typing_converge`; `packages/collab/test/live.test.ts`, `sheet.test.ts` `'concurrent edits of different and same cells converge'`. Chưa: style/numbering/bảng/định dạng ô/chèn-xoá hàng cột (**đội khác đang làm**) |
| P6.2 | API file/thư mục, upload/download | ✅ | `apps/server/BlueOffice.Api/Files/FileEndpoints.cs`; `FileApiTests.UploadDownloadVersionsAndRestore`, `CannotMoveFolderIntoItself`, `OversizedUploadIsRejectedAndLeavesNoFile` |
| P6.3 | Version history, khôi phục | ✅ | `FileApiTests.UploadDownloadVersionsAndRestore` |
| P6.4 | WebSocket relay Yjs | ✅ | `Collab/CollabRoom.cs`; `CollabTests.RelaysSyncUpdatesAndAwarenessBetweenClients` |
| P6.5 | Snapshot định kỳ vào PostgreSQL/MinIO | 🟡 | `CollabRoom` lưu 60 s vào `IObjectStore` (MinIO/filesystem); `UnsavedLogIsPersistedAndReplayedToTheNextSession` chạy filesystem/SQLite; không ghi snapshot vào PostgreSQL, không test với MinIO thật |
| P6.6 | Presence: con trỏ/vùng chọn người khác, danh sách người xem | ✅ | `live.test.ts` `'undoes only the own edits and shows collaborators carets'`; `sheet.test.ts` `'shares presence'`; collab e2e (không chạy lại) |
| P6.7 | Chia sẻ owner/editor/commenter/viewer, link có hạn | ✅ | `FileApiTests.FolderShareIsInherited`, `LinkWorksAnonymouslyUntilItExpires`, `ExpiringUserShareStopsGrantingAccess` |
| P6.8 | SSO OIDC | 🟡 | Server JwtBearer + Keycloak (`Auth/AuthSetup.cs`, `deploy/docker-compose.yml`); client chưa đăng nhập OIDC (xem 3.34); test dùng khoá HMAC dev |
| P6.9 | Audit log | ✅ | `Audit/AuditService.cs`; `DownloadsPrintsAndEditsAreAudited` |
| P6.10 | Worker chuyển đổi (convert, PDF, thumbnail) qua hàng đợi Redis gọi `bo-cli` | 🟡 | `Jobs/ConversionJobProcessor.cs`, `RedisJobQueue`; `ConvertWaitReturnsPdf`, `ConvertsSampleWithRealCli`, `AsyncJobCanBePolledAndOnlyByItsOwner` (hàng đợi in-memory). `/convert` chỉ hỗ trợ đích PDF; `RedisJobQueue` chưa có test |
| P6.11 | Ký PDF (PAdES) | ✅ | `crates/bo-sign/src/pdf.rs`; `pdf_sign_and_verify`, `pdf_second_signature_keeps_first_valid`, `pdf_tampering_is_detected` (B-B, chưa có B-T/RFC 3161) |
| P6.12 | Ký OOXML (XML-DSig) | ✅ | `crates/bo-sign/src/ooxml.rs`; `ooxml_sign_and_verify`, `ooxml_tampering_is_detected` |
| P6.13 | Điểm tích hợp USB token / ký số từ xa CA Việt Nam | 🟡 | Trait `Signer` (`crates/bo-sign/src/lib.rs`, ADR-0007) + `SoftwareSigner`; chưa có PKCS#11, client CA từ xa, UI ký |
| P6.14 | Docker Compose + hướng dẫn cài đặt, sao lưu, nâng cấp | ✅ | `deploy/docker-compose.yml`, `deploy/Dockerfile`; `docs/deploy/README.md`, `backup.md`, `upgrade.md` |
| P6.15 | Tài liệu đáp ứng an toàn thông tin cấp độ | 🟡 | Chỉ mục "An toàn thông tin (gợi ý cho hồ sơ cấp độ)" trong `docs/deploy/README.md` |
| P6.16 | Desktop auto-update (Tauri updater) | 🟡 | Feature Cargo `updater` (`apps/desktop/src-tauri/Cargo.toml`); không có cấu hình `plugins.updater`, UI kiểm tra cập nhật, test; CI build không bật feature; `docs/deploy/desktop-update.md` |
| P6.17 | Desktop file association | ✅ | `tauri*.conf.json`; `shell.rs` `classifies_args`; e2e desktop (không chạy lại) |
| P6.18 | Desktop nhiều cửa sổ | 🟡 | `main.rs` `open_window`, `window_labels_and_urls`; chưa có E2E mở cửa sổ thứ hai |
| P6.19 | Desktop recent files | 🟡 | `LocalRecentFiles` (localStorage) dùng trong `BlueEditor.tsx`; không dùng danh sách recent của OS; chưa có test |

### Phase 6A — Tích hợp BlueDrive

| # | Yêu cầu | TT | Bằng chứng |
|---|---|---|---|
| 6A.1 | BOHP `GET /files/{id}` metadata (tên, size, version, owner, quyền, URL chia sẻ, người dùng) | ✅ | `tests/mock-bluedrive/MockBlueDriveApp.cs`; `BohpClient.GetFileAsync`; `OpensFileFromBlueDriveThroughTheEditUrl` |
| 6A.2 | `GET /files/{id}/contents` | ✅ | `BohpClient.GetContentsAsync`; như trên |
| 6A.3 | `POST /files/{id}/contents` + If-Match, 409 khi xung đột | ✅ | `BohpClient.PutContentsAsync`; `OfflineEditOnStaleVersionCreatesConflictCopy`, `LockHeldByAnotherClientMakesEveryoneReadOnly` |
| 6A.4 | lock / unlock / refresh-lock | ✅ | `TwoUsersShareOneSessionLockAndSaveCreatesVersionWithEditors` |
| 6A.5 | versions + restore | ✅ | `SessionVersionsRestoreAndRenameGoThroughBohp` |
| 6A.6 | rename, `POST /folders/{id}/files` (Save As / file mới) | 🟡 | `BohpClient.RenameAsync/CreateFileAsync`, `/api/sessions/{id}/save-as`; rename có test, **save-as chưa có test** |
| 6A.7 | Access token JWT ngắn hạn, xác minh bằng JWKS, không lưu mật khẩu | ✅ | `Infrastructure/Hosts/HostRegistry.cs`; `OpensFileFromBlueDriveThroughTheEditUrl` (token giả/khác file → 403); `CollabTests.DirectJwtConnectionAndBadTokens` |
| 6A.8 | `WopiProvider` theo [MS-WOPI] | 🟡 | `Infrastructure/Hosts/WopiClient.cs`; `WopiTests.OpensEditsSavesUnderLockAndUnlocks`, `ConflictCopyUsesPutRelativeFile`, `RejectsForeignWopiSrcAndBadToken`, `DiscoveryListsTheThreeApps`; chưa kiểm proof key (X-WOPI-Proof), không có version |
| 6A.9 | Iframe `/edit?file=&host=&token=` | ✅ | `SessionEndpoints.cs` (`GET /edit`); `apps/web/src/main.tsx` |
| 6A.10 | SDK `BlueOffice.open(container, {fileId, accessToken, host, mode, lang, theme, ui})` | 🟡 | `packages/editor-sdk/src/host.ts`; chỉ `lang` được áp dụng; `theme`, `ui.showShare/showClose/brandName`, `mode: 'comment'` chưa có tác dụng |
| 6A.11 | Sự kiện saved/dirty/closed/share-requested/error/collaborators-changed | 🟡 | Phát `ready`, `saved`, `collaborators-changed`, `error`; **chưa phát `dirty`, `closed`, `share-requested`** |
| 6A.12 | Lệnh save/print/export-pdf/close | 🟡 | `packages/hosted/src/session.ts` `commandHandler`: `save` có e2e (collab, không chạy lại); `close` không làm gì; `print`, `export-pdf` chưa có test |
| 6A.13 | postMessage kiểm tra origin (whitelist), không lộ token | ✅ | `packages/editor-sdk/test/sdk.test.ts` `'builds iframe URL without the access token'`, `'ignores messages from non-whitelisted origins'`, `'editor never binds when host origin is not whitelisted'`; `session.test.ts` `'reads the embedding origins from the server (never "*")'`; .NET `ListsTheOriginsAllowedToEmbedTheEditor` |
| 6A.14 | Nút Share/Close/Back gửi sự kiện cho BlueDrive | ❌ | `HostConfig.onShare/onClose` chưa nối với SDK; không có nút Back |
| 6A.15 | Deep link `bluedoc://open?host=bluedrive&file=&token=` (+ bluesheet/blueslide), tải về cache, lưu qua BOHP | ✅ | `shell.rs` `parse_deep_link`, `parses_bluedrive_link`, `rejects_bad_links`, `token_is_not_serialized_when_absent`; e2e desktop `'opens the file from the server session and edits it live with the web user'` (không chạy lại) |
| 6A.16 | Thư mục đồng bộ BlueDrive → hỏi chuyển sang cộng tác online | ❌ | Chưa có (**đội khác đang làm**) |
| 6A.17 | Màn hình Start desktop: tab "BlueDrive" (duyệt, gần đây, chia sẻ với tôi, SSO) | ❌ | Chưa có UI; BlueDrive giả lập chưa có API duyệt/gần đây/chia sẻ (**đội khác đang làm**) |
| 6A.18 | Sửa offline, đồng bộ khi có mạng; xung đột → bản "(xung đột - tên - thời gian)" | 🟡 | Server: `SessionService.ConflictCopyName`, `/api/sessions/{id}/sync`; `OfflineEditOnStaleVersionCreatesConflictCopy`, `OfflineSyncDoesNotOverwriteUnsavedCollaborativeChanges`, `UsesLocalTimeAndKeepsExtension`; **client (web/desktop) chưa gọi `/sync`**, chỉ có offline khi đang trong phiên rồi kết nối lại |
| 6A.19 | Nhiều người cùng file → một phiên Yjs; khoá thuộc phiên | ✅ | `RoomManager.cs`; `TwoUsersShareOneSessionLockAndSaveCreatesVersionWithEditors` |
| 6A.20 | Autosave 30–60 s + khi người cuối rời; version kèm danh sách người sửa | ✅ | `CollabRoom.AutosaveInterval` = 45 s; `AutosaveAsksForACheckpointAndSavesAfterTheInterval`, `UnsavedCheckpointIsSavedWhenTheLastParticipantLeaves`, `CheckpointSaveCreatesVersionWithEditors` |
| 6A.21 | `POST /convert` (docx↔pdf, doc→docx, xls→xlsx, ppt→pptx…) | 🟡 | `Jobs/JobEndpoints.cs`; chỉ `to=pdf` (`ConvertWaitReturnsPdf`); doc→docx, xls→xlsx có trong `bo-cli convert` nhưng server chưa nối |
| 6A.22 | `POST /thumbnail` | ✅ | `AsyncJobCanBePolledAndOnlyByItsOwner`, `BlueDriveServiceTokenCanUseConversionServices` |
| 6A.23 | `POST /preview` (PDF/ảnh từng trang) | 🟡 | `PreviewProducesPdf` — chỉ PDF, chưa có ảnh từng trang |
| 6A.24 | `POST /extract-text` + metadata | ✅ | `ExtractTextReadsEveryPage`, `TextExtractionIgnoresMalformedCommands` (metadata: tên file + số ký tự) |
| 6A.25 | Webhook BlueOffice → BlueDrive (đã lưu, phiên bắt đầu/kết thúc) | ✅ | `Infrastructure/Hosts/HostWebhooks.cs` (ký HMAC); `TwoUsersShareOneSessionLockAndSaveCreatesVersionWithEditors` (file.saved, session.started, session.ended) — đội khác đang mở rộng |
| 6A.26 | Quyền lấy mỗi lần mở + khi refresh; bị thu hồi → chỉ xem ngay | ✅ | `RevokingPermissionMidSessionSwitchesToViewThenCloses`, `DowngradedShareSwitchesConnectionToViewMode`; `packages/hosted/test/readonly.test.ts` `'turns edits into state reads once the mode becomes view'` |
| 6A.27 | Audit (mở, sửa, xuất/in/tải) gửi về BlueDrive | 🟡 | Webhook `audit` cho open/edit (`OpensFileFromBlueDriveThroughTheEditUrl`); client chưa báo print/export/download qua `/api/sessions/{id}/audit` (**đội khác đang làm**) |
| 6A.28 | Tiêu chí: BlueDrive giả lập `tests/mock-bluedrive` (BOHP tối thiểu) | ✅ | `tests/mock-bluedrive/MockBlueDriveApp.cs`; `tests/e2e/collab/stack.mjs` |
| 6A.29 | Tiêu chí E2E: mở từ web | ✅ | Collab e2e `'two users edit one .docx: text, bold, carets, save, revoke, offline'` (không chạy lại) + `HostIntegrationTests` .NET ✅ |
| 6A.30 | Tiêu chí E2E: mở từ desktop qua deep link | ✅ | `tests/e2e-desktop/desktop.test.mjs` (job `desktop`, không chạy lại) |
| 6A.31 | Tiêu chí E2E: 2 người cùng sửa, lưu tạo version, thu hồi quyền giữa chừng | ✅ | Collab e2e như 6A.29; .NET `TwoUsersShareOneSessionLockAndSaveCreatesVersionWithEditors`, `RevokingPermissionMidSessionSwitchesToViewThenCloses` |
| 6A.32 | Tiêu chí E2E: sửa offline rồi đồng bộ | 🟡 | Chỉ kết nối lại trong phiên và gọi API `/sync` trực tiếp (`'stale offline save goes to a conflict copy instead of overwriting'`); chưa có luồng client sửa offline rồi đồng bộ |
| 6A.33 | Hỏi chủ dự án về API BlueDrive trước khi code | 🟡 | BOHP cài đúng như prompt (`docs/bohp.md`); chờ API thật (`docs/PROGRESS.md`) |

### Phase 7 — BlueForm & BlueNote

| # | Yêu cầu | TT | Bằng chứng |
|---|---|---|---|
| P7.1 | BlueForm: biểu mẫu → kết quả vào workbook BlueSheet | 📝 | `docs/plan/phase-7.md` |
| P7.2 | BlueNote: dùng lại engine BlueDoc + bo-collab | 📝 | `docs/plan/phase-7.md` |
| P7.3 | Kế hoạch chi tiết, chưa code | 📝 | `docs/plan/phase-7.md`; không có code BlueForm/BlueNote |

## §7. Kiểm thử độ trung thực

| # | Yêu cầu | TT | Bằng chứng |
|---|---|---|---|
| 7.1 | `tests/corpus/{doc,sheet,slide}`: mỗi tính năng ≥ 3 file, README ghi nguồn, không có dữ liệu thật | 🟡 | `tests/corpus/README.md` (tự sinh bằng `bo-cli corpus`, `sample-slides`, `BO_WRITE_CORPUS=1`); doc 55, sheet 60, slide 35 file; **nhóm `slide-notes` chỉ 1 file, `slide-theme` 2 file**; không có file `.doc/.xls/.ppt` thật |
| 7.2 | Round-trip: XML chuẩn hoá, phần không sửa phải giống hệt | ✅ | `bo_xlsx::canonicalize_xml`; `corpus_round_trips_losslessly`, `corpus_roundtrip`, `corpus_round_trip_is_lossless`, `editing_one_paragraph_keeps_other_parts` |
| 7.3 | Mọi file xuất ra qua OpenXmlValidator trong CI | 🟡 | Job `fidelity` (`scripts/fidelity-report.mjs` + `tools/BlueOffice.Validate`); lần gần nhất docx 54/55, xlsx 57/60 hợp lệ — ngoại lệ ghi ở `tests/fidelity/known-issues.txt` (không chạy lại) |
| 7.4 | Visual regression `bo-cli render` so với `tests/fidelity/reference/`, ngưỡng theo file | 🟡 | `scripts/visual-diff.mjs`, `tests/fidelity/thresholds.json`, baseline tự sinh `tests/fidelity/baseline/`; **chưa có ảnh tham chiếu** từ phần mềm gốc (cần người dùng cung cấp) |
| 7.5 | Fuzzing `cargo fuzz` cho OPC, XML, CFB, BIFF, formula — không panic, không treo | 🟡 | `fuzz/fuzz_targets/` 14 target (thêm `docx`, `xlsx`, `pptx`, `formula` trong lần kiểm này; OPC `opc`, CFB qua `doc`/`xls`/`ppt`/`vba`/`crypto`, BIFF `xls`/`xlsb`/`xls_formula`); biên dịch được (`cargo check`) nhưng **libFuzzer không chạy trên Windows MSVC và không có job CI**. Thay thế trong CI: `crates/bo-engine/tests/fuzz_smoke.rs` (mới) `mutated_docx_corpus_never_panics_or_hangs`, `mutated_xlsx_…`, `mutated_pptx_…` (2 700 đầu vào đột biến XML + ZIP, giới hạn 20 s/lần gọi) và các proptest `*_never_panic*` (`bo-binary` doc/xls/xlsb/ppt/vba/xls_formula, `bo-crypto`, `bo-odf/tests/fuzz.rs`, `bo-encoding`, `bo-opc`, `bo-xlsx`, `bo-formula` `arbitrary_text_never_panics`) |
| 7.6 | Property-based cho engine công thức và undo/redo | ✅ | `crates/bo-formula/tests/proptest_recalc.rs` `incremental_matches_full_recalc`; `proptest_roundtrip.rs`; `bo-edit` `undo_all_restores_original`; `slide_editing.rs` `random_edits_then_undo_all_restore_the_deck`; `bo-collab` proptest hội tụ |
| 7.7 | E2E: tạo văn bản hành chính, bảng tính có công thức, tạo slide, cộng tác 2 người | ✅ | `'page setup dialog and the administrative template'`, `'BlueSheet: numbers + SUM, …'`, `'inserts a shape, shows smart guides, adds a slide and edits notes'`, collab `'two users edit one .docx: …'` (collab không chạy lại) |
| 7.8 | Báo cáo fidelity tự động mỗi lần CI (`docs/fidelity-report.md`): % theo nhóm, xu hướng | 🟡 | Job `fidelity` sinh `docs/fidelity-report.md` + `tests/fidelity/history.csv` (artifact); % theo nhóm định dạng (doc/sheet/slide), chưa theo **nhóm tính năng**; không commit lại vào repo tự động |

## §8. Quy ước code

| # | Yêu cầu | TT | Bằng chứng |
|---|---|---|---|
| 8.1 | Rust edition 2021, `#![forbid(unsafe_code)]` mọi crate | ✅ | `Cargo.toml` (`edition = "2021"`, `[workspace.lints.rust] unsafe_code = "forbid"`); mọi `src/lib.rs`/`main.rs` có `#![forbid(unsafe_code)]` (kể cả `apps/desktop/src-tauri`) |
| 8.2 | Không `unwrap()` trên dữ liệu từ file | 🟡 | Quy ước trong `CLAUDE.md`; không có lint `clippy::unwrap_used` để cưỡng chế (~540 `.unwrap()` trong `src/`, phần lớn trong module test — chưa rà từng chỗ); fuzz smoke không phát hiện panic |
| 8.3 | Clippy pedantic mức warn | ✅ | `[workspace.lints.clippy] pedantic = warn`; CI `-D warnings` |
| 8.4 | TS `strict`, không `any` | ✅ | `tsconfig.base.json` `strict: true`; `eslint.config.js` `@typescript-eslint/no-explicit-any: error` |
| 8.5 | C# nullable, TreatWarningsAsErrors | ✅ | `apps/server/Directory.Build.props`, `tools/Directory.Build.props`, `tests/mock-bluedrive/MockBlueDrive.csproj` |
| 8.6 | Comment tham chiếu mục spec cho mỗi phần tử định dạng | ✅ | Xem 1.3 (không có công cụ kiểm tự động độ phủ) |
| 8.7 | Không hard-code chuỗi UI — i18n vi-VN + en-US | ✅ | `i18n.test.ts` `'vi-VN and en-US define exactly the same keys'` |
| 8.8 | Mỗi commit chức năng kèm test | ✅ | Lịch sử commit; tính năng có test tương ứng ở các bảng trên (`bo-cli` thiếu test riêng) |
| 8.9 | Rustdoc cho public API mỗi crate | 🟡 | Có `//!`/`///` ở các crate chính; không bật `missing_docs`, CI không chạy `cargo doc` |

## §9. CLAUDE.md

| # | Yêu cầu | TT | Bằng chứng |
|---|---|---|---|
| 9.1 | Tóm tắt dự án + link prompt | ✅ | `CLAUDE.md` đầu file |
| 9.2 | Clean-room nhắc lại ngắn | ✅ | `CLAUDE.md` mục "Clean-room" |
| 9.3 | Lệnh build/test/run Rust, web, desktop, server | ✅ | `CLAUDE.md` mục "Lệnh" (chưa có các lệnh mới `sample-big`, `sample-big-sheet`, `bench-open-wasm.mjs`, `fuzz_smoke` — lead cập nhật) |
| 9.4 | Sơ đồ crate và phụ thuộc | ✅ | `CLAUDE.md` mục "Sơ đồ crate" |
| 9.5 | Phase hiện tại & việc tiếp theo | ✅ | `CLAUDE.md` mục "Phase hiện tại" |
| 9.6 | Danh sách ADR | ✅ | `CLAUDE.md` mục "ADR" (0001–0009) |

## §10. Việc cần làm ngay

| # | Yêu cầu | TT | Bằng chứng |
|---|---|---|---|
| 10.1 | Cấu trúc repo + `CLAUDE.md`, `ARCHITECTURE.md`, `PROGRESS.md`, `THIRD_PARTY_NOTICES.md` | ✅ | Xem §4 |
| 10.2 | `docs/plan/phase-1.md` chi tiết | ✅ | `docs/plan/phase-1.md` |
| 10.3 | Phase 1 đến khi CI xanh; mở/hiển thị `.docx` mẫu trên web lẫn desktop, xuất PDF | ✅ | e2e `'opens the sample .docx and renders it with the WASM engine'`, `'exports a PDF'`; `parity.test.ts`; `docs/screenshots/phase1-*.png` |
| 10.4 | Báo cáo (ảnh chụp, benchmark) | ✅ | `docs/PROGRESS.md`, `docs/screenshots/` |

---

## Số đo hiệu năng (lần kiểm này)

Máy dev Windows 11, 16 luồng logic; trong lúc đo có các agent khác build song song (tải CPU 23–32 %), nên ghi khoảng
giá trị qua nhiều lần chạy. File đo do `bo-cli` sinh (không đưa vào corpus vì lớn).

| Phép đo | Lệnh | Kết quả |
|---|---|---|
| `.docx` 300 trang A4, 300 ảnh PNG 640×480 (40 part khác nhau), 14,5 MB — mở + layout + display list trang 1, native release | `bo-cli sample-big out.docx` · `bo-cli bench out.docx --iterations 7` | 418–440 ms (1,5 s khi tải nặng); display list cả 300 trang 474 ms; xuất PDF 1,8–1,9 s (29 MB) |
| Cùng file, web thật (Chromium headless, worker WASM, canvas, giải mã ảnh) — chọn file → trang 1 vẽ xong | `npx playwright test specs/perf.spec.ts` | 942 ms |
| Cùng file, WASM trong Node — mở + display list trang 1 / bộ nhớ WASM | `node scripts/bench-open-wasm.mjs out.docx out.xlsx` | 740–1 266 ms / 80 MB |
| Cuộn 300 trang ~2 400 px/s trong 4 s (Chromium headless) | `perf.spec.ts` | 60,0 fps, p50 16,7 ms, p95 16,8 ms, max 16,8 ms, 0 khung > 33 ms |
| Gõ phím ở 300 trang (native release) | `cargo test --release -p bo-engine --test typing_perf -- --ignored` | 1,24–1,84 ms (xấu nhất 7,65 ms) |
| Gõ phím ở 308 trang (WASM) | `node scripts/bench-typing-wasm.mjs` | 2,79–3,80 ms |
| `.xlsx` 1 000 000 ô / 100 000 công thức, 5,6 MB — mở + tính toàn bộ, native release | `bo-cli sample-big-sheet out.xlsx` · `bo-cli bench out.xlsx` | 1,75–1,99 s (3,1–4,1 s khi tải nặng) |
| Cùng file — sửa ô trong bảng VLOOKUP (10 000 phụ thuộc) / sửa ô thường, native | như trên | 45–84 ms (212–219 ms khi tải nặng) / 9–21 ms |
| Cùng file, WASM — mở + tính / sửa ô VLOOKUP / bộ nhớ WASM | `bench-open-wasm.mjs` | **6,1–7,7 s** / 64–67 ms / 399 MB |
| Khả năng mở rộng layout (native release) | `bo-cli sample-big --pages N` | 150 trang 250 ms · 300 trang 440 ms · 600 trang 2,49 s (tăng siêu tuyến tính khi > 300 trang) |
| Fuzz smoke (debug, stack 2 MiB) | `cargo test -p bo-engine --test fuzz_smoke` | doc 990, sheet 1 080, slide 630 đầu vào đột biến; 0 panic, 0 treo; 4–8 s |

---

## Khoảng trống

Sắp theo mức quan trọng (ảnh hưởng tới mục tiêu tương thích/độ tin cậy trước, tiện ích sau). Các mục có ghi
"(đội khác đang làm)" là trạng thái tại thời điểm kiểm tra.

### Cao — ảnh hưởng tương thích định dạng / tiêu chí hoàn thành
1. **Text box & shape trong `.docx` (P2.12, ❌)**: không mô hình, không hiển thị `wps:wsp`/`w:txbxContent` (chỉ giữ nguyên khi lưu); corpus không có file loại này.
2. **Mở `.xlsx` 1 triệu ô trên web > 5 s (5.13, P3.38)**: WASM 6,1–7,7 s (native 1,75–1,99 s); WASM không tính song song (tắt `eval_parallel` trên wasm32).
3. **Visual diff so với phần mềm gốc chưa đo (P2.22, 7.4)**: thiếu `tests/fidelity/reference/` (ảnh do người dùng cung cấp); mới so với baseline tự sinh.
4. **`.ppt` chỉ đọc chữ (P5.4, 2.10)**: không shape/ảnh, không thành `bo_model::slide`, editor không mở, không lưu `.pptx`.
5. **Track changes chưa đủ (P5.7)**: chèn/xoá cột-ô, gộp/tách ô, header/footer, nội dung chú thích, ảnh nổi đổi vị trí; chú thích dài chưa tràn trang (đội khác đang làm).
6. **Mật khẩu & bảo vệ (P5.5, P5.6)**: lưu có mật khẩu chỉ `.docx`; BlueSheet không mở/lưu file có mật khẩu; bảo vệ sheet/workbook chỉ có hàm băm, không khoá ô, không UI.
7. **Run fonts (P2.2)**: hAnsi gộp với ascii (mất khi khác nhau), eastAsia/cs không dùng khi layout, theme font chưa phân giải.
8. **Thiếu thư viện/khả năng nền (3.8, 3.9, 3.11, 3.12)**: không có `unicode-bidi` (văn bản hai chiều), `hyphenation`, giải mã GIF/BMP/TIFF, bộ chuyển EMF/WMF.
9. **Caladea (2.22)**: Cambria chưa có font thay thế metric-compatible trong `assets/fonts`.
10. **Định dạng BlueDoc (2.4, 2.5, 2.3)**: không có `.rtf`; `.md` mở như văn bản thuần, không lưu `.txt/.md`; editor không lưu ODF (chỉ CLI).
11. **ODF (P5.13, 2.8, 2.11)**: BlueSlide không mở `.odp`; `.odp` chỉ khung chữ; `.ods` ghi chỉ giá trị.
12. **Layout tăng siêu tuyến tính trên 300 trang**: 600 trang 2,49 s so với 300 trang 0,44 s (×5,6 khi ×2) — cần tìm phần O(n²); layout vẫn dàn lại toàn bộ khi có NUMPAGES trong thân bài, đổi style/numbering/section, sửa header/footer (5.4).

### Trung bình — tính năng nền tảng & tích hợp
13. **Auth OIDC phía client (3.34, P6.8)**: `AnonymousSession` ở cả web và desktop; chưa có redirect/PKCE + deep link.
14. **Cộng tác thời gian thực chưa phủ style/numbering/bảng/định dạng ô/chèn-xoá hàng cột (P6.1)** (đội khác đang làm).
15. **Sửa offline rồi đồng bộ phía client (6A.18, 6A.32, 3.40)**: client không gọi `/api/sessions/{id}/sync`; không có PWA/Service Worker; desktop không có luồng lưu cục bộ rồi đồng bộ.
16. **Desktop Start screen tab BlueDrive & phát hiện thư mục đồng bộ (6A.16, 6A.17, ❌)** (đội khác đang làm).
17. **Editor SDK chưa đủ (6A.10–6A.12, 6A.14, 3.44)**: `theme`/`ui`/`mode: 'comment'` không áp dụng; chưa phát `dirty`/`closed`/`share-requested`; `close` không làm gì; nút Share/Close/Back chưa gửi sự kiện cho host; `print`/`export-pdf` chưa có test.
18. **Audit print/export/download từ editor (6A.27)** chưa gửi (đội khác đang làm).
19. **Server chuyển đổi (P6.10, 6A.21, 6A.23)**: `/convert` chỉ ra PDF (chưa doc→docx, xls→xlsx, ppt→pptx dù `bo-cli` đã làm được); `/preview` chưa có ảnh từng trang; `RedisJobQueue` chưa có test.
20. **Snapshot cộng tác chưa vào PostgreSQL; không có test tích hợp PostgreSQL/MinIO/Redis thật (P6.5, 3.26)** — thiếu Testcontainers.
21. **WOPI (6A.8)**: chưa kiểm proof key `X-WOPI-Proof`, không có version.
22. **Ký số (P6.13, P6.11)**: chưa có PKCS#11/USB token, client CA từ xa, UI ký, dấu thời gian RFC 3161 (PAdES B-T).
23. **Desktop (P6.16, P6.18, P6.19, 2.16, 3.32, 3.33)**: auto-update chưa cấu hình/không có UI/test; nhiều cửa sổ & recent files chưa có E2E (recent dùng localStorage); chưa có hộp thoại in OS; không có fallback WASM; CI chỉ đóng gói Windows.
24. **E2E desktop mỏng (3.41)**: 5 test desktop so với 24 web + 4 cộng tác; yêu cầu "mỗi tính năng có E2E trên cả hai target".
25. **PAL web/desktop (3.28–3.31, 3.36)**: web không dùng File System Access API; clipboard chỉ text; không có font server/font hệ thống; desktop offline dùng IndexedDB thay vì thư mục cache.
26. **Storage providers (3.29)**: chưa có `BlueOfficeServerProvider`, `BlueDriveProvider`, `WopiProvider` trong `packages/storage-providers`.
27. **Đa luồng desktop (5.7)**: không có rayon/thread pool cho layout/render.
28. **Save As qua BOHP chưa có test (6A.6)**.

### BlueSheet / BlueSlide / BlueDoc UI
29. **BlueSheet chọn nhiều vùng bằng Ctrl (P3.28, ❌)**; **tab sheet chưa kéo thả/màu tab (P3.27)** (đội khác đang làm phần BlueSheet còn lại).
30. **BlueSlide transition/animation khi trình chiếu (P4.18)**: không chạy hiệu ứng, không có UI đặt hiệu ứng.
31. **BlueSlide chart (P4.12)**: chưa chèn/sửa; **video/audio không phát (P4.10)**; **xuất PNG chưa có trong UI (P4.20)**; Reading View chưa có test (P4.15).
32. **SmartArt fallback cho `.pptx`/`.xlsx` (P5.12)**; **cảnh báo macro ở BlueSheet/BlueSlide (P5.15)**; comment có luồng chỉ BlueDoc (P5.8).
33. **BlueDoc UI**: style gallery chỉ 4 style cố định (P2.15); Find/Replace chưa tìm theo định dạng (P2.16); underline thiếu kiểu (P2.4); kerning không áp dụng (P2.5); `w:hMerge` kiểu cũ (P2.9); desktop chưa có hộp thoại in OS (P2.20).
34. **TCVN3/VNI chỉ khi mở `.doc` (2.24)**.

### Quy trình, kiểm thử, tài liệu
35. **Fuzzing (7.5)**: libFuzzer không chạy trên Windows MSVC và không có job CI (đã thêm `fuzz_smoke` + 4 target mới; nên thêm job Linux nightly `cargo fuzz run <target> -- -max_total_time=60`).
36. **Benchmark trong CI (5.17)**: criterion, `typing_perf` và `bench-open-wasm.mjs` chưa chạy trong CI; `perf.spec.ts` chỉ áp giới hạn cứng khi `BO_PERF_STRICT=1`.
37. **Test hàm công thức (P3.25)**: một file mỗi nhóm thay vì mỗi hàm; nguồn không ghi theo dòng; kỳ vọng do tác giả tự tính, chưa có bộ do người kiểm thử cung cấp; `TEXT()` dùng bộ định dạng số rút gọn.
38. **Corpus (7.1)**: `slide-notes` 1 file, `slide-theme` 2 file (< 3); không có `.doc/.xls/.ppt` thật.
39. **Báo cáo fidelity (7.8)**: % theo định dạng, chưa theo nhóm tính năng; chỉ là artifact CI. OpenXmlValidator còn lỗi đã biết (docx 1, xlsx 3 — `known-issues.txt`).
40. **Flaky E2E**: `sheet.spec.ts` `'BlueSheet: numbers + SUM, …'` hỏng khi máy tải nặng (A4 hiện `10` thay vì `=SUM(A1:A3)` ngay sau khi mở lại).
41. **Quy ước (8.2, 8.9)**: không có lint `unwrap_used`, không `missing_docs`/`cargo doc` trong CI; `docs/spec-notes/` trống (4.6); `bo-cli` không có test riêng và không có lệnh validate/fidelity-report (4.7, P1.13).
42. **Tài liệu an toàn thông tin cấp độ (P6.15)** mới ở mức gợi ý.
43. **Thư viện khác prompt (3.15, 3.16, 3.19, 3.20)**: không dùng serde-wasm-bindgen, chrono (đã thay bằng cài đặt riêng có lý do), TanStack Query, CSS Modules chỉ một phần.
44. **Parity web ↔ desktop (P1.10)**: so display list (đủ cho renderer dùng chung) nhưng chưa có so sánh ảnh với cửa sổ Tauri thật.
