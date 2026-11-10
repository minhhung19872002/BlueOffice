# ADR-0012: BlueScript chạy trong QuickJS (WASM) trong một Web Worker riêng; script lưu theo người dùng

- Trạng thái: Chấp nhận · Ngày: 2026-11-10
- Liên quan: parity CORE-SCRIPT-001 (CORE-SCRIPT-002: ghi thao tác, nút/phím tắt, lưu lịch chạy — mục 8–10);
  `packages/script`; ADR-0008 (cầu lệnh JSON).

## Bối cảnh
BlueScript là ngôn ngữ tự động hóa (JavaScript, API có kiểu TypeScript) cho BlueDoc / BlueSheet / BlueSlide. Script do
người dùng (hoặc người gửi tài liệu) viết nên phải coi là mã không tin cậy: không được chạm DOM, mạng, lưu trữ,
API nền tảng (Tauri IPC), và phải dừng được (vòng lặp vô hạn, ăn bộ nhớ). Chạy thẳng `new Function` trong trang,
hay trong một Web Worker thường, vẫn để lộ `fetch`, `postMessage`, `importScripts`, IndexedDB… của worker.

## Quyết định
1. **Bộ máy**: QuickJS biên dịch sang WebAssembly qua `quickjs-emscripten-core` + biến thể
   `@jitl/quickjs-wasmfile-release-sync` 0.32.0 (MIT, phụ thuộc `@jitl/quickjs-ffi-types` MIT; QuickJS gốc của
   Fabrice Bellard / Charlie Gordon là MIT). Ghi vào `THIRD_PARTY_NOTICES.md`; `pnpm license-check` qua.
   Mỗi lần chạy là một runtime + context QuickJS mới: chỉ có built-in ECMAScript và các đối tượng do prelude tạo.
2. **Cách ly hai lớp**: QuickJS chạy trong **một Web Worker riêng cho mỗi lần chạy** (`script.worker.ts`). Mã người
   dùng nằm trong QuickJS nên không thấy cả global của worker; cầu duy nhất là hàm gốc `__bo_call(method, json)`
   (prelude xóa khỏi global sau khi bọc). Giá trị qua cầu là chuỗi JSON — không đối tượng host nào lọt vào.
   Phía editor, `dispatcher` chỉ nhận các method trong danh sách (`API_METHODS`), kiểm tra đối số rồi dịch sang
   lệnh engine sẵn có: `EngineHost.command` (Doc), `SheetHost.exec` (Sheet; host mà UI dùng, kể cả
   `collabSheetHost` khi cộng tác), `slideCommand` (Slide). Desktop dùng đúng cơ chế này trong WebView (CSP đã có
   `'wasm-unsafe-eval'`, `worker-src 'self'`). Nơi không có Worker (test Node) dùng runner cùng luồng với cùng giới hạn.
3. **Giới hạn**: thời gian (interrupt handler của QuickJS + đồng hồ phía host; worker bị `terminate()` nếu quá hạn
   + 2 s), bộ nhớ heap (`setMemoryLimit`, mặc định 64 MB), stack (512 KB), số lệnh gọi API (200 000), số dòng
   console; nút Dừng hủy ngay (terminate worker). Lệnh gọi API được trả lời tuần tự theo thứ tự script gọi.
4. **Một lần chạy = một bước hoàn tác**: lệnh engine mới `historyLen` / `mergeHistory { since }` cho cả ba engine
   (`bo_edit::Session::merge_history_since`, `SlideEditor::merge_history_since`, `UndoEntry::Group` của BlueSheet).
   Khi đang cộng tác BlueSheet, hoàn tác đi qua replica (mỗi lệnh một bước) — chưa gộp được (ghi ở hạn chế).
5. **Lưu script theo người dùng, không trong tài liệu**: `scriptStore(app)` giữ danh sách script trong local storage
   của nền tảng (web: `localStorage` của origin; desktop: của WebView), khóa `blueoffice.scripts.<app>`. Lý do: một
   tệp mở ở nơi khác không mang theo mã có thể chạy (giống tinh thần không chạy VBA); không phải thêm part tùy biến
   vào ba định dạng. Part tùy biến trong tài liệu (customXml cho docx, part riêng cho xlsx/pptx) để giai đoạn sau nếu
   cần chia sẻ script kèm tệp.
6. **Ngôn ngữ**: JavaScript ES2023 (top-level `await`/`return`), kiểu API ở `packages/script/bluescript.d.ts`; trình
   soạn gợi ý tên từ chính tệp `.d.ts` (không thêm thư viện editor nặng). Chú thích kiểu TypeScript trong script
   chưa được bóc (không đóng gói trình biên dịch TypeScript ~9 MB).
7. **VBA** vẫn không bao giờ chạy (không đổi).
8. **Recorder (CORE-SCRIPT-002)**: mỗi hàm API là đúng một lệnh gọi `method(...args)` JSON, nên bộ ghi chỉ cần quan
   sát các lệnh gọi rồi sinh mã bằng `callToSource`. `ScriptRecorder` bọc đúng các lệnh UI gửi (`recordEngine` cho
   `EngineHost.command` / `slideCommand`, `recordSheetHost` cho `SheetHost`, như `packages/hosted` theo dõi dirty), dịch
   lệnh sửa sang API (gõ liên tiếp → một `insertText`, Enter = `\n`, Backspace bỏ ký tự vừa gõ; định dạng → `format`;
   ô → `range().setValues/format`; hình → `setText/setFill/setLine/move`), còn lại → `command({…})`. Lệnh chọn /
   di chuyển không ghi nguyên văn: bộ ghi giữ vùng chọn của người dùng và của bản chạy lại, chỉ phát `select` (hoặc
   `goTo`/`select` trên slide) khi một thao tác cần. Lệnh do script chạy không bị ghi (`pause`).
9. **Gán script (CORE-SCRIPT-002)**: `SavedScript.shortcut` / `quickAccess` lưu cùng script (theo người dùng); mỗi
   script được gán là lệnh `script.run.<id>` trong `CommandRegistry` (trùng phím tắt bị từ chối, cần Ctrl/Alt hoặc
   F1–F12).
10. **Chạy theo lịch (server)**: `ScriptSchedule` (EF, cả hai provider) + `/api/sessions/{id}/schedules` lưu bản sao
   một script đã lưu của người dùng với biểu thức cron 5 trường (UTC); `ScriptScheduleWorker` chạy lịch đến hạn qua
   `IScriptRunner`. Server chỉ có worker bo-cli (không có JavaScript), nên bộ chạy mặc định
   `UnavailableScriptRunner` ghi `lastStatus = unavailable`; bộ chạy headless thật (QuickJS trong bo-cli hoặc tiến
   trình Node cách ly) để sau. Script nhúng trong tài liệu không bao giờ được lập lịch hay tự chạy.

## Hệ quả
- Gói `@blueoffice/script` (~650 KB WASM, tải lười trong worker khi chạy lần đầu).
- Thêm lệnh vào `EditCommand` / `SheetCommand` / `SlideCommand` (TS) và `Command` / `SheetCommand` / `SlideCommand` (Rust).
- Script không đọc được dữ liệu ngoài tài liệu đang mở; mọi thay đổi đi qua đường lệnh của UI nên undo, review,
  bảo vệ tài liệu và cộng tác áp dụng như thao tác tay.
