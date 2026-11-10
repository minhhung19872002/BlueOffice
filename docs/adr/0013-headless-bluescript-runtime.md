# ADR-0013: BlueScript headless — QuickJS-ng (rquickjs) trong bo-cli, chạy đúng mã API của trình duyệt

- Trạng thái: Chấp nhận · Ngày: 2026-11-11
- Liên quan: parity CORE-SCRIPT-004; ADR-0012 (§10 lịch chạy); `crates/bo-script`, `bo-cli script run`,
  `apps/server/BlueOffice.Api/Scripts/`; hướng dẫn triển khai `docs/deploy/scripts.md`.

## Bối cảnh
ADR-0012 §10 để lại việc chạy lịch trên máy chủ: server chỉ có worker `bo-cli` (Rust), không có JavaScript. Cần một
runtime cách ly (không tệp, không mạng, giới hạn thời gian/bộ nhớ) và **cùng hành vi** với trình duyệt — script viết và
thử trong editor phải cho cùng kết quả khi chạy theo lịch.

## Quyết định
1. **Bộ máy**: crate `rquickjs` 0.14 (MIT; `rquickjs-core`, `rquickjs-sys` MIT, biên dịch mã C QuickJS-ng MIT bằng `cc`,
   có binding sinh sẵn cho `x86_64-pc-windows-msvc` và `x86_64-unknown-linux-gnu` nên không cần bindgen/libclang).
   Chỉ bật feature `std` (không `loader`, không `futures`, không allocator tùy biến — `set_memory_limit` cần allocator
   mặc định). Phụ thuộc mới khác: `allocator-api2`, `relative-path` (MIT OR Apache-2.0). `cargo deny` qua. Crate của ta
   (`bo-script`) giữ `#![forbid(unsafe_code)]`; `unsafe` nằm trong `rquickjs`. Phương án dự phòng (tiến trình Node với
   `--permission`) không cần dùng.
2. **Cùng mã API với trình duyệt**: thay vì viết lại các host API bằng Rust, `packages/script/scripts/build-headless.mjs`
   (Vite/Rolldown có sẵn) đóng gói `hosts/doc.ts`, `hosts/sheet.ts`, `hosts/slide.ts`, `host.ts`, `prelude.ts`, `wrap.ts`
   (qua `headless.ts` / `headless-entry.ts`) thành một IIFE JavaScript thuần `crates/bo-script/js/bluescript-headless.js`
   (`include_str!`, kiểm tra trong `test/headless.test.ts` — bản đóng gói cũ thì test đỏ). Runtime Rust chỉ cấp một
   **cầu engine đồng bộ** `engine(op, argsJson)` → `Engine::command_json` (BlueDoc), `Engine::slide_command_json`
   (BlueSlide), `SheetEngine::{info, cell, exec_json, add_sheet, rename_sheet}` (BlueSheet) — đúng các lệnh mà `EngineHost`
   / `SheetHost` của editor gửi (ADR-0008). Kiểm tra đối số, danh sách method được phép, lỗi trả về script, offset
   UTF-16 ↔ byte… vì thế giống hệt. Test parity (`headless.test.ts`) chạy cùng script ở runner trình duyệt (QuickJS WASM +
   engine WASM) và `bo-cli script run` (QuickJS native + engine native) rồi so `ok`, giá trị, console, số lệnh gọi.
3. **Cách ly**: mỗi lần chạy một `Runtime` + `Context::full` mới: chỉ built-in ECMAScript; không module loader nên
   `import()` bị từ chối; không `require`, `fetch`, `std`/`os`, timer. Bundle công bố một global `__bo_headless`, runtime
   đọc rồi xóa trước khi chạy prelude; prelude lại xóa `__bo_call`/`__bo_log`/`__bo_app` như trong trình duyệt. Cầu engine
   chỉ nằm trong closure của bundle. Mã người dùng chạy cùng context với mã host (khác trình duyệt, nơi host ở ngoài
   QuickJS): script có thể sửa built-in để lừa phần kiểm tra đối số, nhưng điều đó không mở thêm quyền — cầu engine chỉ
   nhận các lệnh của tài liệu đang mở, và `command({…})` vốn đã cho script gửi lệnh engine thô; engine không đọc tệp hay
   mạng. Không closure Rust nào giữ giá trị JS (bộ gom chu trình của QuickJS không nhìn xuyên closure native).
   Không thêm `setTimeout` (trình duyệt cũng không có) để giữ hành vi giống nhau.
4. **Giới hạn**: thời gian (interrupt handler so với hạn chót; cả khi script chờ lệnh API liên tục), heap
   (`JS_SetMemoryLimit` = dung lượng sau khi nạp API + `--memory`), stack 512 KB, số lệnh gọi API (vượt → `limit`),
   2000 dòng console (dòng cuối `…`, như trình duyệt) và tổng đầu ra console + giá trị trả về (`--max-output`). Promise
   không bao giờ xong → lỗi ngay (hàng đợi job cạn), không treo. Lỗi có `line`/`column` trong mã người dùng (đã trừ
   dòng bọc).
5. **Máy chủ**: `BoCliScriptRunner` (`IScriptRunner` mặc định) đọc tệp bằng token host của chủ lịch (lưu mã hóa bằng
   Data Protection, làm mới khi chủ lịch mở danh sách lịch), chạy `bo-cli script run` qua `IBoCliRunner` trong thư mục
   tạm, lưu phiên bản mới (`If-Match`, ghi chú "BlueScript: tên (thời điểm UTC)"); `ScriptScheduleWorker` ghi lịch sử
   `ScriptRun`, tắt lịch sau `Scripts:MaxConsecutiveFailures` lỗi liên tiếp; công tắc quản trị `Scripts:ScheduleEnabled`
   mặc định tắt. Tệp đang mở cộng tác → hoãn (`skipped`).

## Hệ quả
- `bo-cli` chứa thêm bộ máy QuickJS (mã C liên kết tĩnh) và cần trình biên dịch C (MSVC / gcc) lúc build — toolchain
  hiện có (VS Build Tools, ADR-0004; gcc trên Linux) đã đủ.
- Đổi `packages/script/src/{host,hosts/*,prelude,wrap,headless*}.ts` phải chạy lại
  `node packages/script/scripts/build-headless.mjs` (test nhắc).
- Token host có hạn: lịch chạy lâu không mở tệp sẽ lỗi rồi tự tắt khi token hết hạn; cần host hỗ trợ token dài hạn /
  ủy quyền ngoại tuyến để khắc phục hẳn (ghi ở hạn chế).
