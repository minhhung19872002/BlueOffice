# Nhận dạng chữ viết tay: nét vẽ thành chữ / phương trình (CORE-INK-002)

Thẻ **Vẽ › Chuyển đổi** của BlueSlide có bốn lệnh:

| Lệnh | Cần gì | Ghi chú |
|---|---|---|
| **Chuyển nét vẽ thành hình** | Không cần dịch vụ nào | Bộ nhận dạng hình viết sẵn trong lõi (không dùng mô hình học máy): đường thẳng, mũi tên, hình chữ nhật / vuông, tam giác, elip / tròn, hình thoi, ngũ giác, lục giác, ngôi sao. Giữ màu và độ dày nét; một bước hoàn tác; nét không nhận ra được giữ nguyên là nét mực |
| **Tự chuyển thành hình khi vẽ** | Không cần dịch vụ nào | Bật / tắt; mỗi nét bút nhận ra được thành hình ngay khi nhấc bút. Mũi tên: vẽ đường thẳng rồi vẽ hai nét đầu mũi tên |
| **Chuyển nét vẽ thành chữ** | Một bộ nhận dạng chữ viết tay (dưới đây) | Thay nét mực bằng một hộp chữ ở đúng chỗ, cùng màu |
| **Chuyển nét vẽ thành phương trình** | Dịch vụ nhận dạng công thức trên máy chủ | Kết quả (LaTeX hoặc UnicodeMath) đi qua bộ nhập phương trình có sẵn và thành phương trình Office Math (`a14:m`) |

Các lệnh làm việc trên nét vẽ **đã chọn bằng công cụ "Chọn nét vẽ (vòng chọn)"**; chưa chọn nét nào thì làm trên mọi nét
vẽ của trang chiếu.

BlueOffice **không đóng gói mô hình nhận dạng chữ viết tay nào**: chưa có mô hình cục bộ được xác nhận license cho phép
phân phối kèm (cần license MIT / Apache-2.0 / BSD cho cả mã và trọng số). Vì vậy chữ / phương trình đi qua
`HandwritingProvider` của nền tảng (`packages/platform/src/handwriting.ts`):

| Nguồn | Khi nào dùng | Nét vẽ đi đâu |
|---|---|---|
| **Máy chủ BlueOffice** (`ServerHandwritingProvider`) | Quản trị viên đã cấu hình `Handwriting:Provider` (ưu tiên cho chế độ đã cấu hình) | Toạ độ nét (điểm, thời gian) dạng JSON → `POST /api/handwriting` → dịch vụ nhận dạng mà quản trị viên cấu hình |
| **Bộ nhận dạng của trình duyệt / webview** (`BrowserHandwritingProvider`, W3C Handwriting Recognition API, `navigator.createHandwritingRecognizer`) | Trình duyệt có API này; chỉ chữ (API không nhận dạng công thức) | Do trình duyệt quyết định (thường nhận dạng trên thiết bị). Muốn chặn: build web với `VITE_BO_HANDWRITING=server` hoặc `off` |

Không có nguồn nào cho một chế độ thì nút tương ứng bị mờ và chú thích giải thích lý do. Bản desktop dùng API của
webview nếu có; file mở từ host (web nhúng, desktop deep link) dùng máy chủ của phiên (`X-Session-Token`) khi máy chủ đó
đã cấu hình, phần còn lại dùng bộ nhận dạng của webview.

**Phía máy chủ mặc định tắt.** Khi chưa cấu hình (`Handwriting:Provider = None`), `/api/handwriting` trả 404
`unavailable` và **không có nét vẽ nào được gửi đi đâu**. Máy chủ không ghi log và không lưu nét vẽ hay chữ nhận dạng
được (chỉ ghi mã trạng thái khi dịch vụ lỗi).

## Cấu hình máy chủ (`appsettings.json` hoặc biến môi trường `Handwriting__…`)

| Khoá | Mặc định | Ý nghĩa |
|---|---|---|
| `Handwriting:Provider` | `None` | `None` (tắt), `Http` (dịch vụ HTTP bên dưới), `Custom` (một `IHandwritingEngine` do bản triển khai tự đăng ký, ví dụ mô hình cục bộ) |
| `Handwriting:Modes` | `["text"]` | Dịch vụ nhận dạng được gì: `text` (chữ) và / hoặc `math` (công thức) |
| `Handwriting:Languages` | `["vi","en"]` | Mã ngôn ngữ đưa ra cho người dùng (subtag chính) |
| `Handwriting:MathFormat` | `latex` | Kết quả công thức viết bằng `latex` hay `unicodeMath` |
| `Handwriting:MaxStrokes` | `2000` | Số nét tối đa một lần (vượt → 413 `tooLarge`) |
| `Handwriting:MaxPoints` | `200000` | Tổng số điểm tối đa một lần |
| `Handwriting:MaxBodyBytes` | `4194304` | Kích thước thân yêu cầu tối đa |
| `Handwriting:Timeout` | `00:00:30` | Thời gian chờ dịch vụ |
| `Handwriting:Http:Url` | — | Địa chỉ dịch vụ (bắt buộc khi `Provider = Http`); `{{mode}}` và `{{lang}}` được thay bằng chế độ và mã ngôn ngữ |
| `Handwriting:Http:Headers:<Tên>` | — | Header gửi kèm (xác thực: `Authorization`, khoá API …) |
| `Handwriting:Http:ResponsePath` | `text` | Đường dẫn tới kết quả tốt nhất trong câu trả lời JSON (chấm phân cấp, `[*]`, `[n]` như phần dịch), ví dụ `results[0].label` |
| `Handwriting:Http:AlternativesPath` | — | Đường dẫn tới các cách đọc khác (mảng chuỗi hoặc nhiều giá trị), nếu dịch vụ trả |
| `Handwriting:Http:LanguageMap:<mã>` | — | Mã ngôn ngữ theo cách dịch vụ viết (`vi` → `vi-VN`); không có thì gửi subtag chính |

Máy chủ gửi tới dịch vụ một yêu cầu `POST` thân JSON:

```json
{ "mode": "text", "language": "vi", "strokes": [ { "points": [[x, y, t], [x, y, t]] } ] }
```

(`x`, `y`: toạ độ trên trang chiếu, đơn vị point, trục y hướng xuống; `t`: mili giây, tăng dần qua các nét). Dịch vụ
có định dạng khác thì đặt một lớp chuyển đổi nhỏ ở giữa, hoặc đăng ký `IHandwritingEngine` riêng
(`apps/server/BlueOffice.Api/Handwriting/HandwritingEndpoints.cs`) với `Provider = Custom`.

Ví dụ — một dịch vụ tự host nhận dạng cả chữ và công thức, trả `{"results":[{"label":"…"}, …]}`:

```json
"Handwriting": {
  "Provider": "Http",
  "Modes": ["text", "math"],
  "MathFormat": "latex",
  "Http": {
    "Url": "http://hwr.internal:8080/recognize/{{mode}}?lang={{lang}}",
    "Headers": { "Authorization": "Bearer <khoá>" },
    "ResponsePath": "results[0].label",
    "AlternativesPath": "results[*].label",
    "LanguageMap": { "vi": "vi-VN", "en": "en-US" }
  }
}
```

Gửi nét chữ viết tay ra một dịch vụ bên ngoài tổ chức là quyết định của quản trị viên (chính sách dữ liệu); với dữ liệu
nhạy cảm nên dùng dịch vụ tự host.

## API

- `GET /api/handwriting/capabilities[?session=<id>]` → `{ "modes": ["text","math"], "languages": ["vi","en"],
  "mathFormat": "latex" }`; 404 khi tắt.
- `POST /api/handwriting?mode=text|math&lang=vi-VN[&session=<id>]`, thân `application/json`
  `{ "strokes": [ { "points": [[x, y, t?], …] } ] }` → `{ "text": "...", "alternatives": [...] }` (công thức thêm
  `"format": "latex" | "unicodeMath"`). Người gọi phải đăng nhập (Bearer) hoặc ở trong một phiên soạn thảo (`session` +
  header `X-Session-Token`). Lỗi là problem details có `code`: `unavailable` (404), `invalid` / `unsupportedLanguage` /
  `unsupportedMode` (400), 415 (không phải JSON), `tooLarge` (413), `rateLimited` (429), `upstream` (502/504).

Bản web độc lập: `VITE_BO_HANDWRITING` lúc build = `auto` (mặc định: máy chủ cho chế độ đã cấu hình ở đó, trình duyệt
cho phần còn lại) | `browser` | `server` | `off`; `VITE_BO_HANDWRITING_URL` = địa chỉ máy chủ (mặc định cùng origin).

## Mô hình cục bộ (chưa đóng gói)

Hai chỗ chờ sẵn khi có một mô hình được xác nhận license:

- Máy chủ: đăng ký `IHandwritingEngine` và đặt `Handwriting:Provider = Custom`; hoặc chạy mô hình như một dịch vụ HTTP
  nội bộ và trỏ `Provider = Http` vào đó.
- Thiết bị: một `HandwritingProvider` có `kind = 'local'` (`capabilities`, `recognize`) gán vào `Platform.handwriting`
  (ví dụ mô hình WASM / ONNX chạy trong trình duyệt).

## Bộ nhận dạng hình (không cần cấu hình)

Mỗi nét được lấy mẫu lại cách đều và làm mịn, rồi xét kín / hở: nét hở thẳng thành đường thẳng (gần 0°/45°/90° thì bắt
thẳng); nét kín được so giữa elip khớp bình phương tối thiểu và đa giác (Douglas–Peucker, gộp góc gần thẳng, chỉnh góc
bằng giao điểm các cạnh khớp đường thẳng) — loại nào sát nét hơn thì được chọn, rồi phân loại theo số góc, góc vuông,
độ dài cạnh. Mũi tên là một đường thẳng cộng một hoặc hai nét ngắn ở gần một đầu, chĩa ngược về thân. Ngưỡng là lựa chọn
riêng của BlueSlide; nét không vượt ngưỡng nào được giữ là nét mực. Hình vẽ bằng nhiều nét rời (ví dụ hình chữ nhật vẽ
bốn nét) chưa được ghép.
