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
11. **Chuyển macro VBA → bản nháp BlueScript (CORE-SCRIPT-003)**: `packages/script/src/vbaConvert/` (TypeScript
   thuần, viết từ đặc tả công khai [MS-VBAL], không thêm thư viện) gồm bộ tách từ (`lexer.ts`: nối dòng ` _`, chú
   thích `'`/`Rem`, chuỗi `""`, số `&H`/`&O`, ngày `#…#`), bộ phân tích (`parser.ts`) và bộ sinh mã (`emit.ts`).
   `convertVba(source, { app })` trả `{ code, warnings, procedures }`. VBA chỉ được **đọc dưới dạng văn bản**, không
   bao giờ chạy; bản nháp cũng không tự chạy — lệnh `script.convertVba` ("Chuyển macro VBA sang BlueScript…", nhóm
   Tự động hóa của tab Xem ở cả ba app, và nút trong ngăn BlueScript) mở ô dán mã VBA, rồi đưa bản nháp vào trình
   soạn như một **script mới chưa lưu** kèm danh sách cảnh báo (dòng, cấu trúc, lý do); người dùng tự xem lại rồi mới
   Lưu / Chạy.
   - *Ngôn ngữ được chuyển*: `Sub`/`Function` (→ `async function`, lời gọi thêm `await`; macro không tham số đầu tiên
     được gọi ở cuối bản nháp), `Dim`/`Const`/`ReDim`, biến chưa khai báo (đưa lên đầu thủ tục vì phạm vi biến VBA là
     cả thủ tục), `If`/`ElseIf`/`Else` (cả dạng một dòng), `For`/`Next` (`Step`), `For Each`, `Do`/`Loop`
     (`While`/`Until` trước và sau), `While`/`Wend`, `Select Case` (danh sách, `To`, `Is`), `With`, gán, `Set`,
     `Call`, `Exit Sub/Function/For/Do`, toán tử (`&`, `Mod`, `\`, `^`, `<>`, `Not`/`And`/`Or`/`Xor`, `Is Nothing`),
     hàm chuỗi/số thông dụng (`Len`, `Left`, `Right`, `Mid`, `InStr`, `Replace`, `UCase`, `CLng`, `Round`, `IIf`,
     `Split`, `UBound`…; vài hàm cần hàm phụ `vba…` chèn ở đầu bản nháp), `MsgBox`/`Debug.Print` → `console.log`.
   - *Mô hình đối tượng được ánh xạ* (chỉ những gì API BlueScript có): BlueSheet — `Range("A1")`, `Cells(r, c)`
     (chỉ số từ 1 → từ 0), `ActiveSheet`, `Worksheets("x")`/`Sheets(i)`, `.Value`/`.Formula`, `.Font.*`,
     `.Interior.Color`, `.NumberFormat`, căn lề, `.ClearContents`, `.Merge`, `Worksheets.Add`/`.Count`, `.Name`;
     BlueDoc — `Selection.TypeText`/`TypeParagraph`/`Font.*`/`WholeStory`/`InsertBreak`,
     `ActiveDocument.Range.Text`, `Paragraphs(i).Range.Text`/`.Count`/`.Alignment`/`.Style`,
     `Find.Execute … Replace:=wdReplaceAll`; BlueSlide — `ActivePresentation.Slides(i)`/`.Count`, `Shapes(i | "tên")`,
     `TextFrame.TextRange.Text`/`Font.*`, `Fill.ForeColor.RGB`, `Shapes.AddTextbox`/`AddShape`, `.Delete`.
   - *Không chuyển được* (`On Error`, `GoTo`/nhãn, `Type`/`Enum`/`Property`, tệp/`Declare`/COM, `New`, `Like`,
     `InputBox`, `Selection` của bảng tính, `.Select`, `Application.*`, thành viên không có trong API…): **không bao
     giờ bị bỏ lặng lẽ** — dòng gốc thành `// TODO(VBA): <dòng gốc>` và có một cảnh báo; khối có tiêu đề không chuyển
     được vẫn giữ cấu trúc (điều kiện thay bằng `undefined /* TODO(VBA) */`) để phần thân vẫn được chuyển. Riêng dòng
     `Attribute VB_…` (siêu dữ liệu của tệp module xuất ra, không phải mã) được lược đi. Bản nháp
     luôn là JavaScript hợp lệ (test phân tích cú pháp mọi kết quả, kể cả đầu vào hỏng và chuỗi token ngẫu nhiên).
   - *Hạn chế*: ngữ nghĩa chỉ gần đúng (so sánh dùng `==`, ô trống là `null`, `ByRef` thành truyền giá trị, `And`/`Or`
     coi là logic, không có mảng nhiều chiều); mã VBA phải được **dán vào** — BlueOffice mới chỉ đọc danh sách module
     trong `vbaProject.bin` (`bo_binary::vba`), chưa trích mã nguồn module để hiện/chuyển trực tiếp từ tệp đang mở.

## Hệ quả
- Gói `@blueoffice/script` (~650 KB WASM, tải lười trong worker khi chạy lần đầu).
- Thêm lệnh vào `EditCommand` / `SheetCommand` / `SlideCommand` (TS) và `Command` / `SheetCommand` / `SlideCommand` (Rust).
- Script không đọc được dữ liệu ngoài tài liệu đang mở; mọi thay đổi đi qua đường lệnh của UI nên undo, review,
  bảo vệ tài liệu và cộng tác áp dụng như thao tác tay.
