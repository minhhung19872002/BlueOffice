# ADR-0018: Giao diện Rust tự vẽ trên mọi nền tảng (thay React)

- Trạng thái: Chấp nhận, đang triển khai (N1) · Ngày: 2026-11-14
- Liên quan: ADR-0002 (cầu engine), ADR-0016 (khung cửa sổ), ADR-0017 (vẽ tài liệu desktop),
  `docs/perf/RESULTS.md`, `docs/plan/native-ui.md`.

## Bối cảnh

Chủ dự án so BlueDoc desktop với trình soạn thảo đối chứng trên cùng máy: gõ vẫn chậm, chữ trang
và ribbon chưa rõ bằng. Số đo (RESULTS.md): phần Rust mỗi phím ~6 ms (lệnh 0,2 ms, vẽ GPU ~5 ms)
nhưng phím→khung hình trung vị ~35 ms vì một phím đi qua WebView → JS → 3 IPC nối tiếp → canvas →
compositor Chromium. Chữ giao diện do Chromium vẽ; chỉnh CSS đã cải thiện nhưng không bằng ribbon
tự vẽ bằng DirectWrite. Đổi ngôn ngữ (C++) không giải quyết: Rust gọi đúng các API hệ điều hành
với chi phí như C++. Chủ dự án chọn viết lại giao diện native (hướng B + C) và áp dụng cho **mọi
nền tảng**, kể cả bản trình duyệt (2026-11-14).

## Quyết định

Giao diện của BlueDoc / BlueSheet / BlueSlide được viết lại bằng Rust, tự vẽ, thay React:

- **`bo-ui`** (không phụ thuộc nền tảng): cây widget giữ trạng thái, bố cục, focus/phím tắt/KeyTips,
  theme (token xuất từ `packages/ui/src/tokens/tokens.ts`), chuỗi i18n (`packages/i18n` JSON), cây
  trợ năng, vùng tài liệu (trang, cuộn, con trỏ, vùng chọn). Lệnh sửa gọi thẳng `bo_engine` trong
  cùng tiến trình (kiểu `Command`, không JSON/IPC). Vẽ qua trait `Painter`; trang tài liệu là ảnh
  của backend, chỉ vẽ lại dải thay đổi (`bo_gpu_win::changed_region` → chuyển vào crate chung).
- **Backend vẽ / host cửa sổ** theo nền tảng:
  - Windows: Win32 PMv2, swap chain DXGI flip + waitable (độ trễ 1 khung), Direct2D + DirectWrite
    (Segoe UI, ClearType), UI Automation, IMM32/TSF (`bo-gpu-win`, `apps/native-win`).
  - Trình duyệt: WASM vẽ lên `<canvas>` (Canvas2D, sau WebGPU), cây DOM ẩn cho trình đọc màn hình,
    `<textarea>` ẩn nhận IME; chữ giao diện dùng bộ vẽ chữ của trình duyệt.
  - macOS: AppKit + Core Graphics/Core Text; Linux: Wayland/X11 + GPU, FreeType/HarfBuzz.
- Thứ tự: Windows (N1–N3) → trình duyệt (thay `apps/web`) → macOS/Linux; trong lúc chuyển, giao
  diện React vẫn chạy và là bản phát hành của từng nền tảng cho tới khi bản Rust của nền tảng đó
  đạt tiêu chí trong `docs/plan/native-ui.md`.

**Không đổi:** engine, định dạng tệp, server, BlueScript, giao thức add-in/editor-sdk (iframe).

## Hệ quả

- Toàn bộ giao diện viết lại (ribbon, hàng trăm hộp thoại, ba app, bốn nền tảng): theo mốc đo được.
- `unsafe` có ở các host nền tảng (Win32, AppKit), giới hạn trong module `win32` / `macos`.
- Test Playwright của giao diện React dần thay bằng test Rust của `bo-ui` (chạy headless với
  `Painter` ghi lệnh) + kiểm thử trợ năng từng nền tảng (UIA, cây DOM ẩn, NSAccessibility).
- Bản web mất khả năng chọn/tìm chữ giao diện như trang HTML thường; trợ năng phải tự cung cấp.
