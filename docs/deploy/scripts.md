# BlueScript chạy theo lịch trên máy chủ

Người dùng lập lịch cho một script đã lưu từ ngăn BlueScript (CORE-SCRIPT-002, `POST /api/sessions/{id}/schedules`,
biểu thức cron 5 trường, giờ UTC). Từ CORE-SCRIPT-004 máy chủ **chạy thật** các lịch đến hạn: mở tệp từ host bằng
quyền của chủ lịch, chạy script trong runtime headless của `bo-cli`, rồi lưu kết quả thành **phiên bản mới** của tệp
trên host. **Mặc định tắt** — quản trị viên phải bật (ADR-0013).

## Bật

```json
// appsettings.json (hoặc biến môi trường Scripts__ScheduleEnabled=true …)
"Scripts": {
  "SchedulerEnabled": true,        // có chạy bộ lập lịch trong tiến trình này (nhiều instance api: chỉ bật ở một)
  "ScheduleEnabled": true,         // công tắc quản trị: có thực thi script hay không (mặc định false)
  "PollInterval": "00:01:00",      // chu kỳ tìm lịch đến hạn
  "Timeout": "00:00:30",           // thời gian tối đa một lần chạy (giữ nhỏ hơn BoCli:Timeout)
  "MemoryMb": 64,                  // heap QuickJS của script
  "MaxCalls": 200000,              // số lệnh gọi API tối đa
  "MaxConsecutiveFailures": 5,     // lỗi liên tiếp → tự tắt lịch (0 = không bao giờ)
  "MaxRunsKept": 50,               // số lần chạy giữ trong lịch sử mỗi lịch
  "MaxSchedulesPerFile": 20,
  "MaxCodeChars": 200000
},
"BoCli": { "ExecutablePath": "bo-cli", "Timeout": "00:01:00" },
"DataProtection": { "KeysPath": "/data/dp-keys" }   // nên đặt: xem "Token của host"
```

Khi `ScheduleEnabled = false`, lịch đến hạn vẫn được ghi nhận (trạng thái `unavailable`, lý do "switched off"), không
tính là lỗi và không lưu gì. Bộ chạy cần `bo-cli` có lệnh `script run` (bản build cùng phiên bản server).

## Một lần chạy

1. Đọc thông tin tệp bằng token của chủ lịch; chủ lịch phải còn quyền **sửa**. Chỉ tệp `.docx/.docm/.xlsx/.xlsm/.pptx/.pptm`
   (lập lịch trên định dạng khác bị từ chối 400).
2. Nếu tệp đang mở cộng tác (có phòng soạn thảo), lần chạy bị **hoãn** (`skipped`, không tính lỗi) để không ghi đè.
3. Tải nội dung vào một thư mục tạm riêng (tên tệp cố định), chạy
   `bo-cli script run input.<ext> --script script.js --out output.<ext> --json --timeout … --memory … --max-calls …`.
4. Script lỗi → `failed` (thông báo kèm dòng/cột). Script thành công nhưng không đổi gì → `succeeded`, không tạo phiên
   bản. Có thay đổi → ghi lên host với `If-Match` phiên bản vừa đọc (tệp bị sửa trong lúc chạy → `failed`, không ghi
   đè), ghi chú phiên bản `BlueScript: <tên script> (<yyyy-MM-dd HH:mm> UTC)`, người sửa = chủ lịch, gửi webhook
   `file.saved` (`reason = "script"`).
5. Ghi lịch sử: thời điểm, thời lượng, trạng thái, lỗi (≤ 2000 ký tự), console (≤ 8000 ký tự), phiên bản đã lưu.
   Sau `MaxConsecutiveFailures` lần `failed` liên tiếp lịch bị tắt, có `disabledReason`; chủ lịch bật lại bằng
   `PATCH /api/sessions/{id}/schedules/{scheduleId}` `{ "enabled": true }` (đếm lỗi về 0).

API cho chủ lịch (chỉ thấy lịch của mình): `GET /api/sessions/{id}/schedules` (có `lastStatus`, `lastError`,
`consecutiveFailures`, `disabledReason`), `GET …/schedules/{scheduleId}/runs` (lịch sử, mới nhất trước), `PATCH`, `DELETE`.

## Giới hạn của runtime (bo-cli script run)

- QuickJS-ng nhúng trong `bo-cli` (crate `rquickjs`, MIT), mỗi lần chạy một runtime mới. Script chỉ thấy built-in
  ECMAScript và API `doc` / `workbook` / `sheet` / `presentation` / `console` — **giống hệt trình duyệt** vì máy chủ chạy
  đúng mã API của `packages/script` (đóng gói sẵn thành `crates/bo-script/js/bluescript-headless.js`).
- Không có hệ thống tệp, mạng, `require`/`import`, `fetch`, bộ hẹn giờ (`setTimeout`…): script chỉ đọc/sửa tài liệu
  đang mở. Lệnh engine không tự mở tệp hay mạng.
- Giới hạn cứng: thời gian (ngắt bằng interrupt handler của QuickJS; tiến trình còn bị `BoCli:Timeout` chặn ngoài),
  heap (`--memory`, MiB), stack 512 KB, số lệnh gọi API (`--max-calls`), 2000 dòng console và tổng đầu ra (console +
  giá trị trả về, `--max-output` KiB, mặc định 1024).
- Báo cáo JSON: `ok`, `value`, `error { kind: syntax|error|timeout|memory|limit, message, stack, line, column }`, `logs`,
  `logsTruncated`, `calls`, `ms`, `modified`, `output`. Thử tay:
  `bo-cli script run bao-cao.docx --script tong-hop.js --out bao-cao-moi.docx --json`.

## Token của host

Lịch chạy khi người dùng không mở tệp, nên máy chủ giữ **token host** của phiên tạo lịch (và làm mới mỗi khi chủ lịch
mở danh sách lịch hoặc bật lại lịch), mã hóa bằng ASP.NET Core Data Protection (`ProtectedToken`). Tệp cục bộ
(BlueOffice tự lưu) không cần token. Lưu ý:

- Token BOHP/WOPI có hạn dùng do host quyết định. Hết hạn → `failed` "host refused the stored access token", sau
  `MaxConsecutiveFailures` lần lịch tự tắt; mở tệp rồi mở danh sách lịch (hoặc bật lại lịch) để làm mới token.
- Đặt `DataProtection:KeysPath` trên ổ bền (dùng chung khi có nhiều instance api). Mất khóa → token cũ không giải mã
  được, xử lý như token hết hạn.
- Script lập lịch là bản sao script **người dùng gửi lên**, không bao giờ lấy từ tài liệu; VBA không bao giờ chạy.
