# Kế hoạch: giao diện Rust tự vẽ trên mọi nền tảng (ADR-0018)

Mỗi mốc kết thúc bằng: test xanh, số đo trong `docs/perf/RESULTS.md`, ảnh so sánh với đối chứng.
Giao diện React là bản phát hành của mỗi nền tảng cho tới khi bản Rust của nền tảng đó đạt W/B/M.

| Mốc | Nội dung | Tiêu chí xong |
|---|---|---|
| **N1** Vùng tài liệu | `bo-ui::DocView` (bố cục trang, cuộn, con trỏ, lệnh sửa gọi thẳng engine) + host Windows `apps/native-win` (`bluedoc-native.exe`): cửa sổ Win32 PMv2, swap chain flip + waitable, trang là bitmap GPU vẽ lại theo dải; mở tệp, cuộn, nhấp đặt con trỏ, gõ, Backspace/Delete/Enter, mũi tên, Ctrl+Z/Y, con trỏ nhấp nháy, đổi DPI/kích thước | phím→present p95 < 16 ms; chữ trang 1:1 pixel |
| **N2** Bộ widget (đang làm: theme từ `tokens.json`, chuỗi i18n chung, 333 icon từ `assets/icons/ui-icons.json`, chữ DirectWrite Segoe UI kiểu GDI classic, ribbon tab + nhóm Lịch sử/Phông chữ/Đoạn văn gọi lệnh engine, thanh trạng thái) | `bo-ui`: cây phần tử giữ trạng thái, bố cục hàng/cột, nút, nút tách, menu thả, ô nhập, combo, tooltip, focus + Tab, theme sáng/tối từ token, `Painter` ghi lệnh cho test headless; backend Direct2D/DirectWrite | widget gallery chạy; ảnh so với ribbon đối chứng |
| **N3** Khung + ribbon BlueDoc (Windows) | thanh tiêu đề (snap layouts, ADR-0016), tab ribbon, các nhóm gọi lệnh engine, thanh trạng thái, thước | tab Trang đầu hoạt động như bản web |
| **N4** Nhập liệu đầy đủ | IME tiếng Việt (TSF), clipboard (HTML/RTF/text), kéo chọn, chuột phải, tìm/thay, lưu/mở/In, Backstage Tệp | gõ Telex/VNI đúng; test Rust |
| **N5** Hộp thoại + trợ năng | hộp thoại Phông chữ, Đoạn văn, Trang…; UI Automation; KeyTips | Accessibility Insights không lỗi nghiêm trọng |
| **W** Trình duyệt | backend Canvas2D/WebGPU trong `bo-wasm`, cây DOM ẩn cho trợ năng, `<textarea>` ẩn cho IME, thay `apps/web` (giữ `/doc` `/sheet` `/slide`, nhúng `/edit`, editor-sdk) | axe WCAG 2.2 AA trên cây DOM ẩn; tính năng như bản React |
| **M/L** macOS, Linux | host AppKit + Core Text; Wayland/X11 + FreeType/HarfBuzz | như N1–N5 |
| **N6** BlueSheet, BlueSlide | lưới ô, trang chiếu trên `bo-ui`, mọi nền tảng | đủ chức năng theo MATRIX.md |
