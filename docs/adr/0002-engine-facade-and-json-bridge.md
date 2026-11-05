# ADR-0002: `bo-engine` facade và display list JSON làm cầu nối JS ↔ Rust

- Trạng thái: Chấp nhận · Ngày: 2026-11-05

## Bối cảnh
Prompt §3.4 yêu cầu `EngineHost` có cùng một API dù chạy WASM (web) hay native (desktop) và các crate core
không biết mình chạy ở đâu. Yêu cầu cứng: web và desktop hiển thị giống hệt.

## Quyết định
1. Thêm crate **`bo-engine`** (không có trong danh sách §4): facade gom pipeline
   `bo-docx → bo-layout → bo-render → bo-raster/bo-pdf`, giữ trạng thái (font, cache layout, tài liệu đang mở).
   Ba host dùng chung: `bo-wasm` (web), `apps/desktop/src-tauri` (desktop), `bo-cli` (server/headless).
2. Dữ liệu có cấu trúc qua ranh giới JS dùng **JSON** (`DocInfo`, `PageList`, `GlyphOutline[]`);
   dữ liệu nhị phân (PDF, PNG, file) dùng `Uint8Array` (WASM) hoặc raw IPC body/`tauri::ipc::Response` (Tauri).
3. Canvas không vẽ chữ bằng `fillText`: core gửi **outline glyph** (SVG path, đơn vị font) và web cache thành
   `Path2D`. Nhờ đó canvas, PNG (`tiny-skia`) và PDF dùng cùng glyph id và vị trí → hiển thị = in = PDF.
4. Thứ tự nạp font quyết định `FaceId`. Danh sách chuẩn ở `assets/fonts/manifest.json`; `bo-engine/src/bundled.rs`
   và `FONT_FILES` của desktop phải khớp (có test).

## Hệ quả
- Test parity (`packages/platform/test/parity.test.ts`) so sánh byte-by-byte display list WASM với native.
- JSON tốn băng thông hơn binary; với trang A4 thông thường ~30–60 KB/trang, chấp nhận được ở Phase 1.
  Có thể chuyển sang định dạng nhị phân (FlatBuffers/postcard) sau mà không đổi interface `EngineHost`.

## Phương án đã loại
- `serde-wasm-bindgen` trả object JS trực tiếp: không dùng được cho Tauri IPC → hai đường code khác nhau.
- Vẽ chữ bằng `fillText` + web font: kết quả phụ thuộc trình duyệt/hinting, lệch với PDF.
- Mỗi host tự ghép pipeline: dễ lệch hành vi giữa web/desktop/server.
