# BlueOffice — hướng dẫn cho Claude Code

Bộ ứng dụng văn phòng (BlueDoc / BlueSheet / BlueSlide) tương thích định dạng OOXML, một core Rust chạy
WASM (web) + native (Tauri desktop, server worker), một UI React dùng chung.
**Prompt gốc (đọc trước mọi việc): [`BlueOffice-PROMPT.md`](BlueOffice-PROMPT.md).**
Kiến trúc: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) · Tiến độ: [`docs/PROGRESS.md`](docs/PROGRESS.md) · Kế hoạch: `docs/plan/phase-N.md`.

## Quy tắc đặt tên (bắt buộc, ADR-0006)
- Họ sản phẩm: **BlueDrive · BlueDoc · BlueSheet · BlueSlide**; giai đoạn sau: **BlueForm**, **BlueNote** (không có "s" ở cuối).
- Tên hiển thị: "BlueDoc – Soạn thảo văn bản", "BlueSheet – Bảng tính", "BlueSlide – Trình chiếu".
- App id: `doc` / `sheet` / `slide`; route web `/doc` `/sheet` `/slide` (route cũ chuyển hướng); desktop `vn.bluestar.bluedoc|bluesheet|blueslide`; deep link `bluedoc://` `bluesheet://` `blueslide://`.
- KHÔNG dùng tên, nhãn hiệu, icon hay màu nhận diện của Microsoft hoặc Google (Word, Excel, PowerPoint, Office, Google Docs/Sheets/Slides/Drive) làm tên sản phẩm, tính năng hay icon. Chỉ nhắc dạng mô tả tương thích: "Tương thích định dạng Microsoft Word (.docx)".
- Icon và bảng màu từng app là thiết kế riêng của Bluestar, không mô phỏng icon Google/Microsoft.

## Clean-room (bắt buộc)
- Không decompile/disassemble/debug-attach Microsoft Office; không chép code, icon, font, template, chuỗi UI, tên thương hiệu.
- Nguồn hợp lệ: ECMA-376 / ISO 29500, Microsoft Open Specifications ([MS-DOCX], [MS-OI29500], …), ODF 1.3, tài liệu công khai, quan sát black-box.
- Dependency chỉ MIT / Apache-2.0 / BSD / ISC / MPL-2.0 / Zlib / OFL; ghi vào `THIRD_PARTY_NOTICES.md`. CI chặn bằng `cargo deny` + `pnpm license-check`.
- Mỗi phần tử định dạng cài đặt phải có comment tham chiếu spec, vd `// ECMA-376 Part 1, §17.3.1.26 pPr`.

## Môi trường (Windows dev)
- Rust stable **MSVC** (`rustup default stable-x86_64-pc-windows-msvc`) + VS 2022 Build Tools (ADR-0004); target `wasm32-unknown-unknown`.
- `wasm-bindgen` CLI **0.2.130** (phải khớp crate). Node ≥ 22, pnpm 9. .NET SDK ≥ 8 (target net8.0).
- PowerShell chưa có `~/.cargo/bin` trong PATH thì thêm: `$env:Path="$env:USERPROFILE\.cargo\bin;$env:Path"`.
- Linux (CI/cloud): `bo-desktop` (Tauri) cần `libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev
  libsoup-3.0-dev libjavascriptcoregtk-4.1-dev` (apt); thiếu thì `cargo test --workspace --exclude bo-desktop`.

## Lệnh
```bash
# Rust
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
cargo deny check licenses bans sources
cargo bench -p bo-layout                      # benchmark layout 300 trang
cargo run -p bo-cli -- sample tests/corpus/doc/phase1-sample.docx
cargo run -p bo-cli -- render tests/corpus/doc/phase1-sample.docx --out out/page-%d.png
cargo run -p bo-cli -- convert tests/corpus/doc/phase1-sample.docx --to pdf --out out/sample.pdf
cargo run -p bo-cli -- dump <file> --page 0    # display list JSON
cargo run -p bo-cli -- corpus tests/corpus/doc # sinh lại corpus Phase 2 (in bảng README)
cargo run -p bo-cli -- roundtrip in.docx --out out.docx   # mở rồi lưu lại
cargo test -p bo-docx --test corpus -- --nocapture       # % round-trip corpus
cargo test -p bo-edit                          # gồm proptest undo/redo
cargo run -p bo-cli -- convert in.xls --to xlsx          # doc/xls/xlsb/ppt/odt/ods/odp → OOXML, csv, pdf
cargo run -p bo-cli -- inspect in.docx                   # định dạng, mã hoá, macro
cargo run -p bo-cli -- sign in.pdf --key k.pem --cert c.pem   # PAdES / OOXML (bo-sign); --p12 f.p12 | --remote URL; --tsa URL (B-T)
cargo run -p bo-cli -- ltv in.pdf --trust root-ca.pem [--crl-fetch --ocsp-fetch] [--tsa URL]   # PAdES B-LT: DSS (chứng thư + CRL/OCSP); --tsa: B-LTA
cargo run -p bo-cli -- verify in.pdf --trust root-ca.pem [--crl ca.crl] [--ocsp r.der | --ocsp-fetch] [--crl-fetch] [--json]  # chữ ký + dấu thời gian + chuỗi tới trust anchor + thu hồi CRL/OCSP (ADR-0007)
cargo run -p bo-cli -- sample-slides tests/corpus/slide  # sinh lại corpus slide
BO_WRITE_CORPUS=1 cargo test -p bo-xlsx --test corpus    # sinh lại corpus sheet
cargo bench -p bo-formula                                # 1 triệu ô / 100k công thức
cargo run -p bo-cli -- sample-big out/big.docx           # .docx 300 trang có ảnh (đo §5); sample-big-sheet → .xlsx 1 triệu ô
node scripts/bench-open-wasm.mjs <docx> <xlsx>           # thời gian mở + bộ nhớ WASM
cargo test -p bo-engine --test fuzz_smoke                # fuzz smoke (corpus đột biến, chạy được trên MSVC)
pnpm fidelity                                            # round-trip corpus + OpenXmlValidator → docs/fidelity-report.md

# Web (sau khi đổi Rust phải build lại WASM)
pnpm install
pnpm build:wasm                                # crates/bo-wasm → packages/engine-wasm/pkg
pnpm dev                                       # http://localhost:5173/doc
pnpm --filter @blueoffice/web build
pnpm lint && pnpm typecheck && pnpm test       # test parity cần bo-cli (tự build nếu thiếu)
pnpm license-check
pnpm --filter @blueoffice/e2e install-browsers && pnpm test:e2e   # cần web đã build + .NET 8; E2E_PORT=4391 nếu 4173 bận
pnpm --filter @blueoffice/e2e test:collab      # chỉ E2E cộng tác (tự dựng api + BlueDrive giả lập + proxy; COLLAB_PORT)
node tests/e2e/collab/stack.mjs                # stack cộng tác để thử tay (COLLAB_STACK_VERBOSE=1)

# Desktop
pnpm desktop:dev                               # Tauri dev (port 1420)
pnpm --filter @blueoffice/desktop tauri build --no-bundle   # BlueDoc → target/release/bluedoc.exe
pnpm --filter @blueoffice/desktop build:bluesheet          # BlueSheet (vn.bluestar.bluesheet)
pnpm --filter @blueoffice/desktop build:blueslide          # BlueSlide (vn.bluestar.blueslide)
cargo install tauri-driver --locked                        # E2E desktop (WebDriver), cần msedgedriver khớp WebView2
MSEDGEDRIVER=<msedgedriver.exe> pnpm test:e2e:desktop      # target/release/bluedoc.exe (BLUEDOC_EXE=…; COLLAB_E2E=0 bỏ ca deep link); ca so ảnh P1.10 cần apps/web/dist
pnpm --filter @blueoffice/desktop tauri build --no-bundle --features updater   # bản có auto-update (docs/deploy/desktop-update.md)
# Desktop env: BLUEOFFICE_SYSTEM_FONTS=0 · BLUEOFFICE_PRINT_DIALOG=0 · BLUEOFFICE_UPDATE_URL · BLUEOFFICE_E2E_SAVE_DIR (docs/deploy/desktop-print.md)
BLUEOFFICE_SERVER=http://localhost:5000 bluedoc.exe "bluedoc://open?host=…&file=…&token=…"   # mở file host + cộng tác

# Server (gồm tests/mock-bluedrive, tools/BlueOffice.Validate = bo-validate)
dotnet test BlueOffice.sln
BO_TEST_POSTGRES="Host=localhost;Username=bo;Password=bo;Database=bo" dotnet test BlueOffice.sln   # + migration trên PostgreSQL thật (DB tạm)
dotnet run --project apps/server/BlueOffice.Api
dotnet format BlueOffice.sln --verify-no-changes
dotnet run --project tools/BlueOffice.Validate -- <file-or-dir>   # OpenXmlValidator
# đổi model server: migration cho cả hai provider (dotnet tool install -g dotnet-ef --version 8.0.11; docs/deploy/upgrade.md)
(cd apps/server/BlueOffice.Infrastructure && dotnet ef migrations add <Ten> --context PostgresBlueOfficeDbContext --output-dir Data/Migrations/Postgres && dotnet ef migrations add <Ten> --context SqliteBlueOfficeDbContext --output-dir Data/Migrations/Sqlite)
docker compose -f deploy/docker-compose.yml up -d                 # on-prem, xem docs/deploy/
```

## Sơ đồ crate
```
bo-opc ◄─ bo-docx · bo-xlsx · bo-pptx (+ bo-drawingml) · bo-crypto · bo-sign      (đọc/ghi gói OOXML)
bo-model (doc · sheet · slide) ◄─ bo-docx · bo-xlsx · bo-pptx · bo-binary (.doc/.xls/.xlsb/.ppt) · bo-odf · bo-encoding
bo-text ◄─ bo-layout ◄─ bo-render ◄─ bo-raster (PNG), bo-pdf
bo-edit (op khả nghịch, undo, track changes, fields, mail merge) · bo-formula ◄─ bo-sheet (lưới, định dạng số, CF/DV, chart)
bo-engine (facade: Engine doc+slide, SheetEngine) ◄─ bo-wasm (web) · bo-cli · apps/desktop/src-tauri
bo-collab (Yrs: y-sync, awareness, undo cục bộ) ◄─ bo-wasm (WasmCollab)
```
Package TS: `platform` (PAL, `EngineHost`, `SheetHost`) → `canvas-host`, `ui`, `i18n` → `editor` (BlueDoc + `CommandRegistry`),
`sheet-editor`, `slide-editor` → `apps/web` (`/doc` `/sheet` `/slide`, nhúng `/edit`), `apps/desktop`;
`collab` (client WebSocket + binding doc/sheet/slide), `hosted` (mở file từ host: phiên server, lưu thành version,
cộng tác, chế độ chỉ xem — dùng chung cho web nhúng và desktop deep link), `editor-sdk` (host ↔ iframe), `storage-providers`, `engine-wasm` (sinh tự động).

## Quy ước quan trọng
- Rust: `#![forbid(unsafe_code)]`, không `unwrap()` trên dữ liệu từ file, clippy pedantic (các lint được cho phép nằm ở `[workspace.lints]`).
- TS: `strict`, không `any`; không hard-code chuỗi UI — thêm key vào **cả** `packages/i18n/src/locales/vi-VN.json` và `en-US.json` (có test so khớp).
- Mọi chức năng là command trong `CommandRegistry` (`packages/editor/src/commands.ts`); lệnh sửa tài liệu đi qua
  `DocController.run` → `EngineHost.command` (JSON `bo_engine::edit::Command`, ADR-0008). Thêm lệnh mới: variant
  trong `Command` (Rust) + type `EditCommand` (`packages/platform/src/types.ts`).
- Vị trí = (id đoạn thân văn bản, offset byte index text); mô hình là nguồn sự thật, UI không giữ văn bản.
- BlueSheet: mọi lệnh sửa qua `SheetEngine::exec_json` / `SheetHost.exec` (`SheetCommand` trong `packages/platform/src/sheet.ts`);
  BlueSlide: `Engine::slide_command_json` / `EngineHost` (`SlideCommand`). Toạ độ engine luôn là point.
- Cập nhật trạng thái cộng tác: sửa ô qua host bọc `collabSheetHost` (`packages/collab`); đừng ghi thẳng engine khi đang cộng tác.
- Code UI/editor không gọi API trình duyệt/Tauri trực tiếp cho việc phụ thuộc nền tảng → đi qua `Platform`.
- **Thứ tự font = FaceId**: `assets/fonts/manifest.json`, `crates/bo-engine/src/bundled.rs`, `FONT_FILES` trong desktop phải khớp (có test).
  Font hệ thống (desktop, `SystemFontIndex`) nạp lười, FaceId luôn **sau** font đóng gói; parity/so ảnh tắt bằng `BLUEOFFICE_SYSTEM_FONTS=0`.
- Đổi layout/render → chạy lại `pnpm build:wasm` rồi test parity; snapshot insta: `INSTA_UPDATE=always cargo test -p bo-layout` rồi review diff.
- Commit: conventional commits, nhỏ, kèm test. Mỗi phase: plan → làm → test xanh → cập nhật PROGRESS + CLAUDE.md
  (chủ dự án đã bỏ bước dừng chờ duyệt giữa các phase).
- Corpus tự sinh phải qua OpenXmlValidator (`pnpm fidelity`); ngoại lệ có lý do ghi ở `tests/fidelity/known-issues.txt`.

## Phase hiện tại
**Phase 1–6A hoàn thành phần chính; Phase 7 (BlueForm, BlueNote) chỉ có kế hoạch.** Ưu tiên của chủ dự án: bộ office
trước, BlueDrive tính sau. Việc tiếp theo và hạn chế: `docs/PROGRESS.md` mục "Hạn chế đã biết" (cộng tác cho bảng/style/
định dạng ô, track changes cấu trúc còn lại, biểu đồ pivot, so sánh với ảnh tham chiếu do người dùng cung cấp).

## ADR
- [0001](docs/adr/0001-record-architecture-decisions.md) Ghi quyết định kiến trúc
- [0002](docs/adr/0002-engine-facade-and-json-bridge.md) `bo-engine` facade + display list JSON + outline glyph cho canvas
- [0003](docs/adr/0003-deterministic-bundled-fonts.md) Font đóng gói mặc định để hiển thị tất định
- [0004](docs/adr/0004-windows-msvc-toolchain.md) Toolchain MSVC trên Windows
- [0005](docs/adr/0005-lossless-docx-round-trip.md) Round-trip `.docx` lossless (XML gốc + fingerprint)
- [0006](docs/adr/0006-product-naming.md) Đặt tên sản phẩm BlueDoc / BlueSheet / BlueSlide
- [0007](docs/adr/0007-digital-signatures.md) Ký số PAdES / OOXML qua trait `Signer` (USB token, ký từ xa)
- [0008](docs/adr/0008-editing-transactions-and-command-bridge.md) Transaction op khả nghịch + cầu lệnh JSON cho chỉnh sửa
- [0009](docs/adr/0009-npgsql-postgresql-license.md) Chấp nhận Npgsql (PostgreSQL License, kiểu BSD) cho server
