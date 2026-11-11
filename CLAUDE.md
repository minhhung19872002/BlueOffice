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
cargo run -p bo-cli -- convert in.docx --to pdf --pdfa 2b --tagged --bookmarks   # PDF/A-1b/2b/3b (3b đính kèm tệp gốc), PDF có thẻ, dấu trang từ đề mục (cả .pptx/.xlsx)
cargo run -p bo-cli -- dump <file> --page 0    # display list JSON
cargo run -p bo-cli -- corpus tests/corpus/doc # sinh lại corpus Phase 2 (in bảng README)
cargo run -p bo-cli -- blocks assets/blocks/doc  # sinh lại thư viện khối dựng sẵn (.dotx có glossary; có test so khớp)
cargo run -p bo-cli -- roundtrip in.docx --out out.docx   # mở rồi lưu lại
cargo test -p bo-docx --test corpus -- --nocapture       # % round-trip corpus
cargo test -p bo-edit                          # gồm proptest undo/redo
cargo run -p bo-cli -- convert in.xls --to xlsx          # doc/xls/xlsb/ppt/odt/ods/odp + slk/dif/prn/dbf + pdf (chỉ văn bản) → OOXML, csv, pdf; --to slk|dif|prn (một trang tính), --to epub (tài liệu)
cargo run -p bo-cli -- inspect in.docx                   # định dạng, biến thể (dotx/xlsm/ppsx…), mã hoá, macro
cargo run -p bo-cli -- inspect in.xlsm --vba [--json]    # mã nguồn các module VBA, chỉ đọc ([MS-OVBA], không bao giờ chạy)
cargo run -p bo-cli -- sample-macros tests/corpus/macro  # sinh lại corpus có macro (.docm/.xlsm/.pptm, dự án VBA tự sinh)
cargo run -p bo-cli -- convert in.docx --to html         # html (một tệp, ảnh data URI; workbook → bảng HTML) | mht (RFC 2557) | xml-flat (Flat OPC); đầu vào .html/.mht/Flat OPC/Excel XML 2003 nhận theo nội dung
cargo run -p bo-cli -- convert in.docm --to dotx         # mẫu / có macro / trình chiếu (bỏ macro khi kiểu đích không chứa được); --to thmx: tệp chủ đề
cargo run -p bo-cli -- sign in.pdf --key k.pem --cert c.pem   # PAdES / OOXML (bo-sign); --p12 f.p12 | --remote URL; --tsa URL (B-T)
cargo run -p bo-cli -- ltv in.pdf --trust root-ca.pem [--crl-fetch --ocsp-fetch] [--tsa URL]   # PAdES B-LT: DSS (chứng thư + CRL/OCSP); --tsa: B-LTA
cargo run -p bo-cli -- verify in.pdf --trust root-ca.pem [--crl ca.crl] [--ocsp r.der | --ocsp-fetch] [--crl-fetch] [--sunset SHA-1=2014-01-01] [--json]  # chữ ký + dấu thời gian + chuỗi tới trust anchor + thu hồi CRL/OCSP (ADR-0007)
cargo run -p bo-cli -- edit mau.docx --edits edits.json --out out.pdf --to pdf [--strict]   # điền mẫu (content control, bookmark, chuỗi giữ chỗ, thuộc tính; .xlsx: ô / tên) — bo_engine::fill, REST API docs/deploy/rest-api.md
cargo run -p bo-cli -- merge mau.docx data.csv --to zip --each docx --out out.zip     # trộn thư: mỗi bản ghi một tệp (docx|pdf); inspect <tệp> --json
cargo run -p bo-cli -- script run in.docx --script s.js [--out out.docx] [--timeout ms] [--memory mb] [--json]   # BlueScript headless (bo-script, ADR-0013)
node packages/script/scripts/build-headless.mjs          # đóng gói lại API BlueScript cho bo-script sau khi sửa packages/script/src
cargo run -p bo-cli -- sample-slides tests/corpus/slide  # sinh lại corpus slide
cargo run -p bo-cli -- convert deck.pptx --to gif        # trình chiếu → GIF động (bo_engine::slide_edit::video; web: MP4/WebM qua WebCodecs)
BO_WRITE_CORPUS=1 cargo test -p bo-xlsx --test corpus    # sinh lại corpus sheet
cargo bench -p bo-formula                                # 1 triệu ô / 100k công thức
node scripts/bench-bg-wasm.mjs 512                       # xoá nền ảnh (CORE-PIC-002) trong bản WASM release, ngân sách < 1,5 s
cargo run -p bo-cli -- sample-big out/big.docx           # .docx 300 trang có ảnh (đo §5); sample-big-sheet → .xlsx 1 triệu ô
node scripts/bench-open-wasm.mjs <docx> <xlsx>           # thời gian mở + bộ nhớ WASM
cargo run -p bo-cli -- sample-big out/big-1000.docx --pages 1000 --rich   # + bảng, chú thích, trường; sample-big-deck out/deck-300.pptx (ảnh + video) — docs/perf/open-perf.md
cargo test --release -p bo-engine --test open_close_leak # heap sau mở/đóng 100 tài liệu; WASM: node scripts/leak-wasm.mjs 100 [tệp lớn…]
cargo test -p bo-engine --test fuzz_smoke                # fuzz smoke (corpus đột biến, chạy được trên MSVC)
pnpm fidelity                                            # round-trip corpus + OpenXmlValidator → docs/fidelity-report.md
cargo run -p bo-cli -- validate <file-or-dir> [--json] [--openxml auto|require|off] [--choice]   # OPC cấu trúc + OpenXmlValidator (--choice: cả nhánh mc:Choice)
cargo run -p bo-cli -- fidelity-report [--no-validate]   # chạy scripts/fidelity-report.mjs với chính bo-cli này (BO_CLI)

# Web (sau khi đổi Rust phải build lại WASM)
pnpm install
pnpm build:wasm                                # crates/bo-wasm → packages/engine-wasm/pkg
pnpm dev                                       # http://localhost:5173/doc
pnpm --filter @blueoffice/web build
pnpm lint && pnpm typecheck && pnpm test       # test parity cần bo-cli (tự build nếu thiếu)
pnpm license-check
node scripts/gen-icon-library.mjs              # thư viện biểu tượng chèn (assets/icons/library + packages/ui/src/iconLibrary/data.json); sửa hình trong script rồi sinh lại
pnpm --filter @blueoffice/e2e install-browsers && pnpm test:e2e   # cần web đã build + .NET 8; E2E_PORT=4391 nếu 4173 bận
pnpm --filter @blueoffice/e2e test:collab      # chỉ E2E cộng tác (tự dựng api + BlueDrive giả lập + proxy; COLLAB_PORT)
node tests/e2e/collab/stack.mjs                # stack cộng tác để thử tay (COLLAB_STACK_VERBOSE=1)

# Desktop
pnpm desktop:dev                               # Tauri dev (port 1420)
pnpm --filter @blueoffice/desktop tauri build --no-bundle   # BlueDoc → target/release/bluedoc.exe
pnpm --filter @blueoffice/desktop build:bluesheet          # BlueSheet (vn.bluestar.bluesheet)
pnpm --filter @blueoffice/desktop build:blueslide          # BlueSlide (vn.bluestar.blueslide)
cargo install tauri-driver --locked                        # E2E desktop (WebDriver), cần msedgedriver khớp WebView2
pnpm --filter @blueoffice/desktop tauri build --no-bundle --features updater,webdriver --config src-tauri/tauri.webdriver.conf.json   # bản E2E khi chạy với quyền admin (CI): WebView2 bỏ qua biến WEBVIEW2_* ở tiến trình elevated
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
docker compose -f deploy/docker-compose.yml -f deploy/docker-compose.multi.yml up -d   # 2 instance api (lease phòng Redis)
```

## Sơ đồ crate
```
bo-opc ◄─ bo-docx · bo-xlsx · bo-pptx (+ bo-drawingml) · bo-crypto · bo-sign      (đọc/ghi gói OOXML)
bo-model (doc · sheet · slide) ◄─ bo-docx · bo-xlsx (+ Excel XML 2003) · bo-pptx · bo-binary (.doc/.xls/.xlsb/.ppt) · bo-odf · bo-encoding · bo-tabular (.slk/.dif/.prn, đọc .dbf)
bo-html (HTML/MHTML ↔ doc, bảng HTML ↔ sheet; không chạy script, không tải tài nguyên ngoài) · bo-opc::flat (Flat OPC)
bo-text ◄─ bo-layout ◄─ bo-render ◄─ bo-raster (PNG), bo-pdf
bo-edit (op khả nghịch, undo, track changes, fields, mail merge) · bo-formula ◄─ bo-sheet (lưới, định dạng số, CF/DV, chart)
bo-query (Lấy & biến đổi dữ liệu: nguồn CSV/JSON/XML/sổ/thư mục/web + ngôn ngữ bước JSON; docs/spec-notes/sheet-queries.md) ◄─ bo-engine (lệnh `query*`)
bo-segment (xoá nền ảnh: graph cut họ GrabCut, max-flow Boykov–Kolmogorov tự viết, GMM; trait `SegmentationProvider`) ◄─ bo-engine (`picture_bg`: lệnh `pictureBackgroundPreview`/`pictureRemoveBackground`, ô `cellImage*Background`)
bo-engine (facade: Engine doc+slide, SheetEngine) ◄─ bo-wasm (web) · bo-cli · apps/desktop/src-tauri
bo-collab (Yrs: y-sync, awareness, undo cục bộ) ◄─ bo-wasm (WasmCollab)
bo-script (BlueScript headless: QuickJS-ng/rquickjs + bundle API của packages/script, cầu engine đồng bộ) ◄─ bo-cli (`script run`)
```
Package TS: `platform` (PAL, `EngineHost`, `SheetHost`) → `canvas-host`, `ui`, `i18n` → `editor` (BlueDoc + `CommandRegistry`),
`sheet-editor`, `slide-editor` → `apps/web` (`/doc` `/sheet` `/slide`, nhúng `/edit`), `apps/desktop`;
`script` (BlueScript: sandbox QuickJS trong worker, API `doc`/`workbook`/`presentation`, pane `script.editor`; ADR-0012),
`addin-sdk` (manifest + giao thức JSON-RPC postMessage + client cho trang add-in, build `addin-sdk/v1.js`) →
`addins` (khung sandbox, cầu kiểm quyền lên API BlueScript, `useAddins` cho ba editor; ADR-0014),
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

## Giai đoạn Parity (đang chạy)
Đang ở giai đoạn Parity — làm theo vòng lặp trong [`BlueOffice-PARITY-PROMPT.md`](BlueOffice-PARITY-PROMPT.md) mục 1,
không dừng giữa chừng. Nguồn sự thật: `docs/parity/BACKLOG.md` (hạng mục `[~]` = đang làm), `MATRIX.md`,
`functions.md` (`python scripts/parity-functions.py`), `NEEDS_HUMAN.md`, `REFERENCE_GUIDE.md`.

## Phase hiện tại
**Phase 1–6A hoàn thành phần chính; Phase 7 (BlueForm, BlueNote) chỉ có kế hoạch.** Ưu tiên của chủ dự án: bộ office
trước, BlueDrive tính sau. Không còn worktree dở; các nhánh `origin/wip/phase*` là ảnh chụp cũ (2026-11-05) đã làm lại trong
`main`. Việc tiếp theo và hạn chế: `docs/PROGRESS.md` mục "Hạn chế đã
biết" (dựng lưới 3D thật cho slide, so sánh với ảnh tham chiếu do người dùng
cung cấp).

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
- [0010](docs/adr/0010-library-deviations-from-prompt.md) Thư viện khác prompt: cầu nối chuỗi JSON, serial date tự cài, không TanStack Query, CSS Modules một phần
- [0011](docs/adr/0011-own-english-hyphenation-rules.md) Gạch nối tự động: quy tắc tiếng Anh tự viết (không tệp mẫu bên thứ ba), tiếng Việt chỉ ngắt ở dấu cách
- [0012](docs/adr/0012-bluescript-quickjs-sandbox.md) BlueScript: QuickJS (WASM, MIT) trong Web Worker riêng, API qua cầu JSON allow-list, một lần chạy = một bước hoàn tác, script lưu theo người dùng
- [0013](docs/adr/0013-headless-bluescript-runtime.md) BlueScript headless trên server: QuickJS-ng (`rquickjs`, MIT) trong `bo-cli script run`, chạy đúng bundle API của trình duyệt, lịch chạy lưu phiên bản mới (`Scripts:ScheduleEnabled` mặc định tắt)
- [0014](docs/adr/0014-web-addins.md) Add-in web: iframe sandbox + CSP, JSON-RPC qua postMessage, quyền kiểm từng lệnh, dùng lại API BlueScript, danh mục tổ chức trên server (mặc định tắt)
- [0015](docs/adr/0015-math-caret-in-place-equation-editing.md) Sửa phương trình tại chỗ: con trỏ toán (vị trí phương trình + đường dẫn ô + offset) trong engine, sửa trên cây `bo_math::Node`, mỗi thay đổi một op thay đối tượng, hình học ô từ layout
