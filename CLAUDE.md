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

# Web (sau khi đổi Rust phải build lại WASM)
pnpm install
pnpm build:wasm                                # crates/bo-wasm → packages/engine-wasm/pkg
pnpm dev                                       # http://localhost:5173/doc
pnpm --filter @blueoffice/web build
pnpm lint && pnpm typecheck && pnpm test       # test parity cần bo-cli (tự build nếu thiếu)
pnpm license-check
pnpm --filter @blueoffice/e2e install-browsers && pnpm test:e2e   # cần web đã build; E2E_PORT=4391 nếu 4173 bận

# Desktop
pnpm desktop:dev                               # Tauri dev (port 1420)
pnpm --filter @blueoffice/desktop tauri build --no-bundle   # BlueDoc → target/release/bluedoc.exe
pnpm --filter @blueoffice/desktop build:bluesheet          # BlueSheet (vn.bluestar.bluesheet)
pnpm --filter @blueoffice/desktop build:blueslide          # BlueSlide (vn.bluestar.blueslide)

# Server
dotnet test BlueOffice.sln
dotnet run --project apps/server/BlueOffice.Api
dotnet format BlueOffice.sln --verify-no-changes
```

## Sơ đồ crate
```
bo-model ◄─ bo-docx ◄─ bo-opc
bo-text ◄─ bo-layout ◄─ bo-render ◄─ bo-raster, bo-pdf
bo-engine (facade) ◄─ bo-wasm (web) · bo-cli (headless) · apps/desktop/src-tauri (desktop)
bo-model ◄─ bo-edit (transaction/undo) ◄─ bo-engine (`command_json`, nav caret/hit-test)
Khung cho phase sau: bo-xlsx, bo-pptx, bo-drawingml, bo-binary, bo-odf, bo-crypto, bo-formula, bo-collab
```
Package TS: `platform` (PAL) → `canvas-host`, `ui`, `i18n` → `editor` → `apps/web`, `apps/desktop`; `storage-providers`, `editor-sdk`, `engine-wasm` (sinh tự động).

## Quy ước quan trọng
- Rust: `#![forbid(unsafe_code)]`, không `unwrap()` trên dữ liệu từ file, clippy pedantic (các lint được cho phép nằm ở `[workspace.lints]`).
- TS: `strict`, không `any`; không hard-code chuỗi UI — thêm key vào **cả** `packages/i18n/src/locales/vi-VN.json` và `en-US.json` (có test so khớp).
- Mọi chức năng là command trong `CommandRegistry` (`packages/editor/src/commands.ts`); lệnh sửa tài liệu đi qua
  `DocController.run` → `EngineHost.command` (JSON `bo_engine::edit::Command`, ADR-0008). Thêm lệnh mới: variant
  trong `Command` (Rust) + type `EditCommand` (`packages/platform/src/types.ts`).
- Vị trí = (id đoạn thân văn bản, offset byte index text); mô hình là nguồn sự thật, UI không giữ văn bản.
- Code UI/editor không gọi API trình duyệt/Tauri trực tiếp cho việc phụ thuộc nền tảng → đi qua `Platform`.
- **Thứ tự font = FaceId**: `assets/fonts/manifest.json`, `crates/bo-engine/src/bundled.rs`, `FONT_FILES` trong desktop phải khớp (có test).
- Đổi layout/render → chạy lại `pnpm build:wasm` rồi test parity; snapshot insta: `INSTA_UPDATE=always cargo test -p bo-layout` rồi review diff.
- Commit: conventional commits, nhỏ, kèm test. Mỗi phase: plan → làm → test xanh → cập nhật PROGRESS + CLAUDE.md → dừng chờ duyệt.

## Phase hiện tại
**Phase 2 — BlueDoc MVP: phần cốt lõi hoàn thành, chờ duyệt** (nhánh `wip/phase2-bluedoc`). Hạn chế còn lại: `docs/plan/phase-2.md`.

## ADR
- [0001](docs/adr/0001-record-architecture-decisions.md) Ghi quyết định kiến trúc
- [0002](docs/adr/0002-engine-facade-and-json-bridge.md) `bo-engine` facade + display list JSON + outline glyph cho canvas
- [0003](docs/adr/0003-deterministic-bundled-fonts.md) Font đóng gói mặc định để hiển thị tất định
- [0004](docs/adr/0004-windows-msvc-toolchain.md) Toolchain MSVC trên Windows
- [0005](docs/adr/0005-lossless-docx-round-trip.md) Round-trip `.docx` lossless (XML gốc + fingerprint)
- [0006](docs/adr/0006-product-naming.md) Đặt tên sản phẩm BlueDoc / BlueSheet / BlueSlide
- [0007](docs/adr/0007-digital-signatures.md) Ký số PAdES / OOXML qua trait `Signer` (USB token, ký từ xa)
- [0008](docs/adr/0008-editing-transactions-and-command-bridge.md) Transaction op khả nghịch + cầu lệnh JSON cho chỉnh sửa
