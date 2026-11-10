# Kiến trúc BlueOffice

Tài liệu gốc: [`BlueOffice-PROMPT.md`](../BlueOffice-PROMPT.md). Quyết định chi tiết: [`docs/adr/`](adr/).

## 1. Tổng quan

```
┌──────────────────────────── apps/web (Vite) ─────┐   ┌──────── apps/desktop (Tauri 2) ─────────┐
│ createWebPlatform()  +  <BlueEditor/>            │   │ createDesktopPlatform() + <BlueEditor/> │
└──────────────┬───────────────────────────────────┘   └──────────────┬──────────────────────────┘
               │            packages/editor (dùng chung 100%)          │
               │  ui · i18n · canvas-host · command registry · store   │
               ▼                                                       ▼
   EngineHost (WebEngineHost)                              EngineHost (NativeEngineHost)
   Web Worker ─► bo-wasm ─┐                                Tauri commands ─► bo-engine (native)
                          ▼                                                       │
                    ┌──────────────────────────── bo-engine ───────────────────────┘
                    │ open → bo-docx → bo-model → bo-layout → bo-render (DisplayList)
                    │                         ▲ bo-text (fonts, shaping)      │
                    │                         │                     ┌─────────┼─────────┐
                    │                         │                  bo-raster  bo-pdf   JSON → canvas
                    └─────────────────────────┴──────── bo-cli (headless, server worker)
```

- **Một core Rust** chạy ở 3 nơi: WASM (trình duyệt), native trong Tauri, native trong `bo-cli` (server).
- **Một UI React**: `packages/editor` nhận một `Platform` (§3.4 prompt) và không biết mình chạy ở đâu.
- **Display list** là hợp đồng giữa core và mọi backend vẽ (canvas, PNG, PDF) → hiển thị = in = xuất PDF.

## 2. Crate Rust và phụ thuộc

```
bo-model ◄── bo-docx ◄──┐
   ▲         bo-opc ◄───┤
   │                    │
bo-text ◄── bo-layout ◄─┼── bo-render ◄── bo-raster
   ▲                    │       ▲    ◄── bo-pdf
   └────────────────────┴── bo-engine ◄── bo-wasm
                               ▲      ◄── bo-cli
                               └───── apps/desktop/src-tauri (bo-desktop)
```

| Crate | Vai trò |
|---|---|
| `bo-model` | Mô hình tài liệu (`doc`), bảng tính (`sheet`), trình chiếu (`slide`); đơn vị spec (twips, half-points, EMU) |
| `bo-opc` | OPC: content types, relationships, part; giữ part không hiểu; chống zip bomb, giới hạn độ sâu XML |
| `bo-docx` · `bo-xlsx` · `bo-pptx` | WordprocessingML / SpreadsheetML / PresentationML ↔ model, round-trip lossless (ADR-0005) |
| `bo-drawingml` | DrawingML: màu, theme, 187 preset shape, fill/line/text, chart |
| `bo-html` | HTML / MHTML (RFC 2557) ↔ tài liệu, bảng HTML ↔ bảng tính; bộ phân tích HTML khoan dung, bỏ script, không tải tài nguyên ngoài (CORE-FMT-003). Flat OPC ở `bo_opc::flat`, Excel XML 2003 ở `bo_xlsx::xml2003` |
| `bo-binary` · `bo-odf` · `bo-encoding` · `bo-crypto` | `.doc/.xls/.xlsb/.ppt` (đọc), ODF 1.3, TCVN3/VNI → Unicode, [MS-OFFCRYPTO] |
| `bo-text` | FontManager, fallback metric-compatible, shaping rustybuzz, cache, outline |
| `bo-layout` | Dàn trang: phân trang, bảng, ảnh, header/footer, cột, footnote, field |
| `bo-edit` | Transaction op khả nghịch, undo/redo, track changes, comment, field/TOC, mail merge (ADR-0008) |
| `bo-formula` · `bo-sheet` | Engine công thức (276 hàm, dependency graph, spill); lưới, định dạng số, CF/DV, sort/filter/fill, chart |
| `bo-render` · `bo-raster` · `bo-pdf` | DisplayList (rect, line, glyph, image, path, transform, clip) → canvas / PNG / PDF |
| `bo-engine` | Facade (ADR-0002): `Engine` (doc + slide, chỉnh sửa slide), `SheetEngine` |
| `bo-collab` | Yrs (CRDT tương thích Yjs): y-sync, awareness, undo cục bộ |
| `bo-sign` | Ký số PAdES / OOXML qua trait `Signer` (ADR-0007) |
| `bo-wasm` · `bo-cli` | Bindings web (`WasmEngine`, `WasmSheet`, `WasmCollab`); CLI render/convert/inspect/sign/fidelity |

## 3. Package TypeScript

| Package | Vai trò |
|---|---|
| `@blueoffice/platform` | Interface `Platform` (files, storage, clipboard, fonts, print, engine, auth, window, offline, shell); `createWebPlatform`, `createDesktopPlatform` |
| `@blueoffice/engine-wasm` | Output `wasm-bindgen` (sinh bởi `pnpm build:wasm`, không commit) |
| `@blueoffice/canvas-host` | `renderPage` (display list → Canvas 2D), `GlyphStore` (Path2D), `DocumentView` (ảo hoá trang, zoom, Ctrl+wheel), `ImeInput` (textarea ẩn, composition events) |
| `@blueoffice/ui` | Ribbon (Radix Tabs/Tooltip), StatusBar, design tokens sáng/tối |
| `@blueoffice/i18n` | i18next, `vi-VN` mặc định, `en-US`; định dạng số theo locale |
| `@blueoffice/editor` | `<BlueEditor>` (BlueDoc) nhúng được; `CommandRegistry` (id, shortcut, enabled/checked) |
| `@blueoffice/sheet-editor` · `@blueoffice/slide-editor` | `<BlueSheetEditor>` (lưới canvas, thanh công thức) · `<BlueSlideEditor>` (chỉnh sửa, trình chiếu, presenter) |
| `@blueoffice/collab` | `CollabClient` (WebSocket y-sync, kết nối lại, mã đóng) + binding BlueSheet ↔ `bo-collab` |
| `@blueoffice/storage-providers` | `LocalStorageProvider`; phiên server/BlueDrive/WOPI đi qua chế độ nhúng `apps/web/src/embed.ts` |
| `@blueoffice/editor-sdk` | Host ↔ iframe qua `postMessage` (whitelist origin, token không nằm trong URL) |

## 4. Luồng mở tài liệu

1. Editor gọi `platform.storage.open()` → `FileAccess.openFile()` (web: `<input type=file>`; desktop: hộp thoại native + đọc đĩa trong Rust).
2. `platform.engine.open(bytes, name)`:
   - web: `postMessage` (transfer) sang Web Worker → `WasmEngine.open`;
   - desktop: `invoke('engine_open', bytes)` với raw IPC body → `Engine::open`.
3. `DocInfo` (số trang, kích thước) → `DocumentView.setDocument` tạo khung trang đúng kích thước.
4. Khi trang vào vùng nhìn thấy: `engine.getPage(i)` (JSON display list) → `GlyphStore.ensure` lấy outline glyph còn thiếu
   → `renderPage` lên `<canvas>` với tỉ lệ `zoom × devicePixelRatio × 96/72`.

## 5. Đơn vị & toạ độ

- Model: đơn vị spec (twips, half-points, EMU).
- Layout/DisplayList: point (1/72 inch), gốc trên-trái, y hướng xuống.
- Canvas: `px = pt × 96/72 × zoom × devicePixelRatio`. PDF: y lật (`H − y`). PNG: `scale` px/pt.

## 6. An toàn khi mở file

- `bo_opc::Limits`: số entry, kích thước part, tổng kích thước giải nén, tỉ lệ nén (đọc thật bị chặn bằng `take`).
- Tên part chứa `..`/`\` bị từ chối. XML: giới hạn độ sâu 256 (`bo_opc::xml::XmlReader`).
- Không thực thi macro, không tải tài nguyên ngoài. Toàn bộ crate `#![forbid(unsafe_code)]`.
- Server: `BoCliConverter` chạy `bo-cli` trong thư mục tạm riêng, không dùng tên file của client làm đường dẫn, có timeout & giới hạn kích thước.

## 7. Kiểm thử

| Loại | Ở đâu |
|---|---|
| Unit/integration Rust | `cargo test --workspace` (OPC, reader, font, layout, PDF, PNG, engine) |
| Property-based | `bo-opc/tests/proptest_roundtrip.rs`, `bo-layout/tests/layout.rs` (`no_text_lost`) |
| Snapshot | `bo-layout/tests/snapshots/` (insta) |
| Parity web ↔ desktop | `packages/platform/test/parity.test.ts`: display list WASM == native, từng trang |
| Unit TS | Vitest trong từng package |
| E2E web | `tests/e2e` (Playwright) |
| .NET | `apps/server/BlueOffice.Tests` (xUnit, WebApplicationFactory, chạy `bo-cli` thật nếu có) |
| Benchmark | `cargo bench -p bo-layout`, `bo-cli bench` |
