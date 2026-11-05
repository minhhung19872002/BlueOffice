# Phase 1 — Nền tảng (Foundation)

Trạng thái: **hoàn thành — chờ duyệt** (kết quả: [PROGRESS.md](../PROGRESS.md)) · Prompt gốc: [`BlueOffice-PROMPT.md`](../../BlueOffice-PROMPT.md) §6 Phase 1, §10.

## 1. Mục tiêu

Mở một file `.docx` đơn giản (đoạn văn nhiều run, căn lề, giãn dòng, thụt lề, tiếng Việt, nhiều trang)
và hiển thị **giống hệt nhau** trên web (WASM trong Web Worker) và desktop (Tauri, core native),
dùng chung `packages/editor`; xuất được PDF và PNG từ cùng một display list.

Ngoài phạm vi Phase 1: chỉnh sửa văn bản, bảng, ảnh, styles kế thừa đầy đủ, numbering, header/footer
(Phase 2); bảng tính / trình chiếu (Phase 3–4); server nghiệp vụ (Phase 6).

## 2. Kiến trúc luồng dữ liệu

```
.docx bytes ─► bo-opc (Package) ─► bo-docx (reader) ─► bo-model (Document)
                                                           │
                         bo-text (FontManager, shaping) ◄──┤
                                                           ▼
                                                 bo-layout (LayoutDocument: pages/lines/runs)
                                                           ▼
                                                 bo-render (DisplayList, đơn vị point)
                                   ┌───────────────────────┼────────────────────────┐
                                   ▼                       ▼                        ▼
                         bo-raster (PNG)          bo-pdf (PDF)        JSON → canvas-host (Canvas 2D)
```

`bo-engine` là facade (ADR-0002) gom toàn bộ pipeline thành một API duy nhất; được dùng bởi
`bo-wasm` (web), `apps/desktop/src-tauri` (desktop) và `bo-cli`. Nhờ đó web và desktop chạy
**cùng một mã Rust** → cùng display list.

## 3. Danh sách crate / package tạo mới

| Thành phần | Nội dung Phase 1 |
|---|---|
| `crates/bo-model` | `Document`, `Section`, `PageSetup`, `Paragraph`, `Run`, `ParaProps`, `RunProps`, đơn vị `Twips`/`HalfPoints`/`Emu` |
| `crates/bo-opc` | Đọc/ghi gói OPC: `[Content_Types].xml`, `_rels/*.rels`, part; giữ nguyên part không hiểu; giới hạn chống zip-bomb |
| `crates/bo-docx` | Reader tối thiểu: `w:p`, `w:r`, `w:t`, `w:tab`, `w:br`, `w:pPr` (jc, ind, spacing, pageBreakBefore), `w:rPr` (rFonts, sz, b, i, u, color), `w:sectPr` (pgSz, pgMar), `docDefaults` |
| `crates/bo-text` | `FontManager` (font đóng gói + font hệ thống trên native), bảng fallback metric-compatible, shaping rustybuzz, cache shaping, metric |
| `crates/bo-layout` | Ngắt dòng UAX#14, căn trái/phải/giữa/đều, giãn dòng auto/exact/atLeast, thụt lề, tab mặc định, ngắt trang, cache theo đoạn |
| `crates/bo-render` | `DisplayList` (rect, line, glyph run, clip) — serde JSON |
| `crates/bo-raster` | DisplayList → PNG (tiny-skia, outline glyph từ ttf-parser) |
| `crates/bo-pdf` | DisplayList → PDF (pdf-writer, font CID Type2 subset bằng `subsetter`, ToUnicode) |
| `crates/bo-engine` | Facade pipeline (ADR-0002) |
| `crates/bo-wasm` | wasm-bindgen bindings của `bo-engine` |
| `crates/bo-cli` | `render`, `convert --to pdf`, `pack`/`unpack`, `dump` (display list JSON), `bench` |
| các crate khác ở §4 | crate rỗng có rustdoc mô tả phạm vi + phase dự kiến (giữ cấu trúc repo) |
| `packages/platform` | Interface `Platform` đủ 10 mục §3.4; `WebPlatform`, `DesktopPlatform` (files, engine, fonts, clipboard; các mục khác bản tối thiểu) |
| `packages/canvas-host` | Vẽ display list lên Canvas 2D, cache `Path2D` glyph, zoom, cuộn ảo hoá theo trang, `<textarea>` ẩn nhận IME |
| `packages/ui` | Ribbon dạng tab, thanh trạng thái, theme sáng/tối (CSS variables), icon Lucide |
| `packages/i18n` | i18next, `vi-VN` mặc định, `en-US` |
| `packages/editor` | `@blueoffice/editor` — React component nhúng được, nhận `Platform` + cấu hình host |
| `packages/storage-providers` | `DocumentStorage` + `LocalStorageProvider` |
| `packages/editor-sdk` | Khung type cho SDK host (cài đặt ở Phase 6A) |
| `apps/web` | Vỏ Vite: khởi tạo `WebPlatform` + `<BlueEditor>` |
| `apps/desktop` | Vỏ Tauri 2: khởi tạo `DesktopPlatform` + `<BlueEditor>`; Tauri command gọi `bo-engine` native |
| `apps/server` | ASP.NET Core 8 Minimal API khung: `/health`, `/api/version`; Domain/Infrastructure rỗng; xUnit |

## 4. API giữa các module

### Rust
```rust
bo_opc::Package::read(bytes, &Limits) -> Result<Package>;  Package::write() -> Result<Vec<u8>>
bo_docx::read_docx(bytes) -> Result<bo_model::Document>
bo_text::FontManager::{add_font, resolve(family, bold, italic), shape(face, size, text)}
bo_layout::layout_document(&Document, &mut FontManager, &mut LayoutCache) -> LayoutDocument
bo_render::build_display_list(&LayoutDocument) -> DisplayList
bo_raster::render_page_png(&PageDisplayList, &FontManager, scale) -> Vec<u8>
bo_pdf::write_pdf(&DisplayList, &FontManager) -> Result<Vec<u8>>
bo_engine::Engine::{new, add_font, open(bytes, name), page_count, page_json(i), glyph_outline(face, gid), export_pdf, render_png}
```

### TS ↔ Engine (`EngineHost`, giống nhau trên web và desktop)
```ts
open(bytes: Uint8Array, name: string): Promise<DocInfo>   // { pageCount, pages: [{width,height}] (pt) }
getPage(index: number): Promise<PageDisplayList>            // JSON của bo-render
getGlyphOutlines(face: number, gids: number[]): Promise<GlyphOutline[]>  // SVG path, font units
exportPdf(): Promise<Uint8Array>
```

## 5. Test sẽ viết

- Rust unit: OPC round-trip (part không hiểu giữ nguyên byte-by-byte), content type lookup, zip-bomb bị chặn,
  reader docx (căn lề, thụt lề, run props, sectPr), font fallback, shaping tiếng Việt (NFC, dấu tổ hợp),
  layout (căn đều, ngắt trang, line spacing), PDF hợp lệ (header, xref, font nhúng), PNG đúng kích thước.
- Property-based (`proptest`): ngắt dòng không làm mất/nhân đôi ký tự; layout không panic với input ngẫu nhiên.
- Snapshot (`insta`): layout của file mẫu `tests/corpus/write/phase1-sample.docx`.
- Parity web/desktop: Vitest nạp `bo-wasm` (WASM) và so display list JSON với `bo-cli dump` (native) → phải giống hệt.
- TS: Vitest cho canvas renderer (mock context), i18n (đủ key 2 ngôn ngữ), LocalStorageProvider, command registry.
- E2E: Playwright mở web, nạp file mẫu, chụp ảnh, kiểm tra số trang trên status bar.
- .NET: xUnit `WebApplicationFactory` cho `/health`.
- Benchmark: `criterion` layout tài liệu 300 trang; `bo-cli bench`.

## 6. Tiêu chí hoàn thành

1. `cargo test --workspace`, `cargo clippy -- -D warnings`, `cargo fmt --check` xanh.
2. `pnpm -r test`, `pnpm -r typecheck`, `pnpm lint` xanh; `pnpm --filter @blueoffice/web build` thành công.
3. `dotnet test` xanh.
4. File mẫu mở được trên web và desktop, display list giống hệt (test parity xanh).
5. `bo-cli convert phase1-sample.docx --to pdf` và `bo-cli render ... --out page-%d.png` chạy được.
6. CI GitHub Actions định nghĩa đủ job build/test/lint/cargo-deny/license-checker.
