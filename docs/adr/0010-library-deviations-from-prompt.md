# ADR-0010: Các thư viện dùng khác danh sách trong prompt

- Trạng thái: Chấp nhận · Ngày: 2026-11-08
- Liên quan: prompt §4 "Thư viện gợi ý" (Rust core, TypeScript UI); REQUIREMENTS 3.15, 3.16, 3.19, 3.20 (mục hạn chế #43);
  ADR-0002 (cầu nối JSON), ADR-0008 (cầu lệnh JSON cho chỉnh sửa).

## Bối cảnh
Prompt liệt kê thư viện *gợi ý*: `serde-wasm-bindgen`, `chrono`, TanStack Query (gọi API), Radix UI + CSS Modules.
Bốn chỗ trong code hiện tại không theo đúng danh sách đó. ADR này ghi lại lý do để người review không coi là sơ suất,
và nêu điều kiện để xem xét lại.

## Quyết định

### 1. Cầu nối JS ↔ Rust bằng chuỗi JSON thay vì `serde-wasm-bindgen`
`bo-wasm` trả `String` (JSON, `serde_json`) và `Uint8Array` cho dữ liệu nhị phân; TypeScript gọi `JSON.parse`.
- Cùng một payload đi qua **ba** host: WASM (web), Tauri IPC (desktop), `bo-cli`/server. Tauri IPC vốn là JSON;
  `serde-wasm-bindgen` chỉ có ở web → hai đường tuần tự hoá, hai bộ bug (ADR-0002).
- Test parity (`packages/platform/test/parity.test.ts`) so **byte-by-byte** display list WASM với native; chuỗi JSON
  so được trực tiếp, object JS thì không (thứ tự khoá, `Map` vs object, `u64` → `BigInt`).
- Trang A4 thường cho display list 30–60 KB; một lần `JSON.parse` không phải nút thắt so với layout/raster
  (đo thời gian mở file: `scripts/bench-open-wasm.mjs`).

### 2. Serial date tự cài thay vì `chrono`
`crates/bo-formula/src/date.rs` cài thuật toán days-from-civil (lịch Gregorian dự phóng) và hai hệ ngày
ECMA-376 Part 1 §18.17.4 / §18.2.28 `date1904`.
- Phần khó của bảng tính không nằm ở lịch mà ở quy ước serial: ngày giả 1900-02-29 (serial 60), serial 0 = "1900-01-00",
  hệ 1904, giới hạn 9999-12-31. `chrono` không có các quy ước này; vẫn phải tự viết lớp chuyển đổi.
- Hàm ngày giờ của công thức (`TODAY`, `NOW`, `EDATE`, `NETWORKDAYS`, …) chỉ cần y/m/d ↔ serial và phần thập phân giờ;
  một file (`date.rs`, gồm cả phân tích chuỗi ngày giờ) có test theo spec (serial 60, hệ 1904, round-trip từ serial 1 đến
  9999-12-31) rẻ hơn một dependency (thêm vào `cargo deny`, tăng kích thước WASM).
- Thời điểm hiện tại cho `NOW`/`TODAY` lấy từ `now_serial` hoặc do host đặt (`Workbook::set_now`, dùng cho test tất định);
  không cần cơ sở dữ liệu múi giờ IANA.

### 3. Không dùng TanStack Query
Gọi API đi qua `Platform` (`packages/platform/src/web/*`, `oidc.ts`) và `DocumentStorage`
(`packages/storage-providers/src/server.ts`, `packages/hosted`) bằng `fetch` bọc mỏng.
- Ứng dụng là trình soạn thảo một tài liệu, không phải màn hình danh sách dữ liệu: số lời gọi ít (mở, lưu version,
  phiên cộng tác, chia sẻ), không cần cache/refetch/pagination của TanStack Query.
- Lưu và cộng tác có ngữ nghĩa riêng (khoá version, xung đột, ngoại tuyến qua service worker `apps/web/src/sw.ts`);
  cache tự động của thư viện dễ trả dữ liệu cũ hoặc gửi trùng lệnh lưu.
- Desktop (Tauri) và web phải dùng cùng interface `Platform`; hook React của TanStack Query không qua được lớp này.
- Trạng thái UI dùng Zustand như prompt.

### 4. CSS Modules chỉ một phần
Có CSS Modules cho các khối lớn (`Editor.module.css`, `Ruler.module.css`, `SheetEditor.module.css`,
`SlideEditor.module.css`, `ui/Dialog|Popover|Ribbon.module.css`) và token chung `packages/ui/src/theme.css`
(biến CSS, chế độ tối). Một phần component nhỏ dùng `style={{…}}` inline.
- Phần lớn style inline là **giá trị tính lúc chạy** (vị trí overlay theo toạ độ point của engine, kích thước canvas,
  màu ô/nét do tài liệu quy định, con trỏ cộng tác) — không biểu diễn được bằng class tĩnh.
- Phần còn lại là bố cục một-hai thuộc tính trong hộp thoại; tách ra file module riêng không lợi gì về đọc hiểu.
- Quy tắc từ nay: style tĩnh có từ 3 thuộc tính trở lên hoặc dùng lại ở ≥ 2 chỗ → đưa vào CSS Module dùng token
  của `theme.css`; giá trị phụ thuộc dữ liệu giữ inline.

## Hệ quả
- Không thêm dependency nào; `THIRD_PARTY_NOTICES.md`, `cargo deny`, `pnpm license-check` không đổi.
- Thêm lệnh engine mới: trả JSON (ADR-0008), không trả `JsValue`.
- Nếu sau này cần giảm chi phí tuần tự hoá display list, đổi sang định dạng nhị phân (postcard/FlatBuffers) cho
  **cả ba** host, không chuyển riêng web sang `serde-wasm-bindgen`.

## Phương án đã loại
- `serde-wasm-bindgen` chỉ cho web, JSON cho Tauri: hai đường code, phá test parity byte-by-byte.
- `chrono` + lớp quy ước serial: vẫn phải tự viết phần khó, thêm dependency và kích thước WASM mà không bớt test.
- `time` crate: cùng lý do với `chrono`.
- TanStack Query bọc quanh `Platform`: thêm tầng cache phải tắt gần hết (`staleTime: Infinity`, không retry lệnh lưu) để
  giữ ngữ nghĩa version — chi phí lớn hơn lợi ích.
- Chuyển toàn bộ style sang CSS Modules: các giá trị tính lúc chạy vẫn phải inline (hoặc CSS variable đặt inline), không
  giảm số chỗ inline có ý nghĩa.
