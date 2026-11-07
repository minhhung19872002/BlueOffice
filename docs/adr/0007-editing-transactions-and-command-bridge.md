# ADR-0007: Chỉnh sửa qua `Transaction` của `Op` khả nghịch và cầu lệnh JSON

- Trạng thái: Chấp nhận · Ngày: 2026-11-07

## Bối cảnh
Prompt §5.2: mọi chỉnh sửa là một `Transaction` gồm `Op` nguyên tử, undo/redo không giới hạn, ánh xạ 1-1 sang CRDT (Phase 6).
Editor UI chạy trên web (WASM trong Web Worker) và desktop (Tauri native) với cùng một mã React (ADR-0002).

## Quyết định
1. **Mô hình là nguồn sự thật duy nhất, nằm trong core.** `bo-edit::Session` giữ `Document`, selection và lịch sử;
   UI không giữ bản sao văn bản.
2. **Vị trí** = (id đoạn thân văn bản theo thứ tự tài liệu, offset byte trong *index text* của đoạn — `bo_model::index`).
   Layout gắn cùng id vào mỗi dòng (`LineSource::Body`) cùng các điểm caret, nên hit-test/caret/selection được tính trong core.
3. **Op** ở mức khối: `Blocks { path, index, remove, insert }`, `Sections`, `Media`, `Numbering`, `Styles`.
   Mỗi op mang nội dung nó xoá ⇒ nghịch đảo chính xác; `apply` kiểm tra nội dung bị xoá khớp (phát hiện lệnh cũ).
   Gõ phím liên tiếp được gộp theo từ thành một bước undo. Property test: chuỗi thao tác ngẫu nhiên rồi undo hết == ban đầu.
4. **Cầu nối**: `Engine::command_json(Command) → EditResult` (selection, hình học caret/selection, trang bẩn, trạng thái
   định dạng cho ribbon, cờ `modified`). Web gọi qua Worker, desktop qua lệnh Tauri `engine_command`.
   Lệnh được tuần tự hoá phía UI (`DocController`), kéo chuột được gộp.
5. Clipboard: định dạng nội bộ JSON khối (`application/x-blueoffice-blocks+json`, lossless), HTML (xuất/nhập), text;
   UI dùng `Platform.clipboard` và sự kiện `paste` của textarea IME.

## Hệ quả
- Op mức khối thô hơn CRDT ký tự; Phase 6 sẽ thêm op mức ký tự (`InsertText`/`DeleteText`) có cùng giao diện nghịch đảo.
- Mỗi lệnh dàn lại toàn bộ tài liệu (cache theo đoạn giữ chi phí thấp); chỉ trang có display list đổi mới vẽ lại.
- Undo lưu bản sao đoạn trước/sau — bộ nhớ tỉ lệ với số đoạn bị sửa, chấp nhận được.

## Phương án đã loại
- Giữ văn bản ở JS (contenteditable/DOM): không đảm bảo hiển thị = in = PDF và trùng lặp logic giữa web và desktop.
