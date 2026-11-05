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

| Crate | Vai trò | Phase 1 |
|---|---|---|
| `bo-model` | Mô hình tài liệu, đơn vị spec (twips, half-points, EMU) | ✅ văn bản: section/paragraph/run |
| `bo-opc` | OPC: content types, relationships, part; giữ part không hiểu; chống zip bomb, giới hạn độ sâu XML | ✅ |
| `bo-docx` | WordprocessingML ↔ model | ✅ reader tối thiểu + writer tối thiểu |
| `bo-text` | FontManager, fallback metric-compatible, shaping rustybuzz, cache, outline | ✅ |
| `bo-layout` | Dàn trang → pages/lines/fragments | ✅ bản đầu, cache theo đoạn |
| `bo-render` | Layout → DisplayList | ✅ |
| `bo-raster` | DisplayList → PNG (tiny-skia) | ✅ |
| `bo-pdf` | DisplayList → PDF (CIDFontType2 subset + ToUnicode) | ✅ |
| `bo-engine` | Facade pipeline (ADR-0002) | ✅ |
| `bo-wasm` | wasm-bindgen | ✅ |
| `bo-cli` | render / convert / dump / pack / unpack / sample / bench | ✅ |
| `bo-xlsx`, `bo-pptx`, `bo-drawingml`, `bo-binary`, `bo-odf`, `bo-crypto`, `bo-formula`, `bo-collab`, `bo-edit` | Phase 2–6 | khung rỗng |

## 3. Package TypeScript

| Package | Vai trò |
|---|---|
| `@blueoffice/platform` | Interface `Platform` (files, storage, clipboard, fonts, print, engine, auth, window, offline, shell); `createWebPlatform`, `createDesktopPlatform` |
| `@blueoffice/engine-wasm` | Output `wasm-bindgen` (sinh bởi `pnpm build:wasm`, không commit) |
| `@blueoffice/canvas-host` | `renderPage` (display list → Canvas 2D), `GlyphStore` (Path2D), `DocumentView` (ảo hoá trang, zoom, Ctrl+wheel), `ImeInput` (textarea ẩn, composition events) |
| `@blueoffice/ui` | Ribbon (Radix Tabs/Tooltip), StatusBar, design tokens sáng/tối |
| `@blueoffice/i18n` | i18next, `vi-VN` mặc định, `en-US`; định dạng số theo locale |
| `@blueoffice/editor` | `<BlueEditor>` nhúng được; `CommandRegistry` (id, shortcut, enabled/checked) |
| `@blueoffice/storage-providers` | `LocalStorageProvider` (BlueOfficeServer/BlueDrive/WOPI ở Phase 6/6A) |
| `@blueoffice/editor-sdk` | Hợp đồng type cho host (BlueDrive); cài đặt ở Phase 6A |

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
