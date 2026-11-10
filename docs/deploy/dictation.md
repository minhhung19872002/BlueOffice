# Đọc chính tả (CORE-LANG-002)

Nút **Trang đầu › Giọng nói › Đọc chính tả** (phím tắt `Ctrl+Alt+Shift+D`) của BlueDoc, BlueSheet và BlueSlide nghe
micrô và gõ chữ vào chỗ con trỏ, qua `DictationProvider` của nền tảng (`packages/platform/src/dictation.ts`); giao diện
không tự mở micrô hay gọi dịch vụ nào. Có ba nguồn nhận dạng:

| Nguồn | Khi nào dùng | Âm thanh đi đâu |
|---|---|---|
| **Máy chủ BlueOffice** (`ServerDictationProvider`) | Quản trị viên đã cấu hình `Dictation:Provider` (mặc định ưu tiên nguồn này) | Micrô → cắt thành từng câu ở chỗ ngừng nói → WAV 16 kHz mono → `POST /api/dictation` → dịch vụ nhận dạng giọng nói mà quản trị viên cấu hình (thường tự host trong mạng nội bộ) |
| **Bộ nhận dạng của trình duyệt / webview** (`WebDictationProvider`, Web Speech API) | Máy chủ chưa cấu hình và trình duyệt có `SpeechRecognition` | Do trình duyệt quyết định — **một số trình duyệt gửi âm thanh tới dịch vụ đám mây của hãng trình duyệt**. Muốn chặn: build web với `VITE_BO_DICTATION=server` hoặc `off` |
| **Model trên thiết bị** (`LocalDictationProvider`) | Chưa có (xem "Model cục bộ") | Không rời máy |

Bản desktop dùng bộ nhận dạng của webview nếu có (WebView2 có thể không có hoặc báo lỗi khi bắt đầu — thanh đọc chính tả
báo "chưa dùng được ở đây"). File mở từ host (web nhúng, desktop deep link) dùng máy chủ của phiên (`X-Session-Token`)
khi máy chủ đó đã cấu hình.

**Phía máy chủ mặc định tắt.** Khi chưa cấu hình (`Dictation:Provider = None`), `/api/dictation` trả 404
`unavailable` và **không có âm thanh nào được gửi đi đâu**. Máy chủ không ghi log và không lưu âm thanh hay chữ nhận
dạng được (chỉ ghi mã trạng thái khi dịch vụ lỗi).

## Cấu hình máy chủ (`appsettings.json` hoặc biến môi trường `Dictation__…`)

| Khoá | Mặc định | Ý nghĩa |
|---|---|---|
| `Dictation:Provider` | `None` | `None` (tắt), `Http` (dịch vụ HTTP bên dưới), `Custom` (một `IDictationEngine` do bản triển khai tự đăng ký, ví dụ model cục bộ) |
| `Dictation:Languages` | `["vi","en"]` | Mã ngôn ngữ đưa ra cho người dùng (subtag chính) |
| `Dictation:MaxAudioBytes` | `4194304` | Âm thanh tối đa của một câu (WAV 16 kHz 16-bit ≈ 32 KB/giây; vượt → 413 `tooLarge`; trình soạn thảo cắt câu dài quá 15 giây) |
| `Dictation:Timeout` | `00:01:00` | Thời gian chờ dịch vụ |
| `Dictation:Http:Url` | — | Địa chỉ dịch vụ (bắt buộc khi `Provider = Http`); `{{lang}}` được thay bằng mã ngôn ngữ |
| `Dictation:Http:Headers:<Tên>` | — | Header gửi kèm (xác thực: `Authorization`, khoá API …) |
| `Dictation:Http:Mode` | `Raw` | `Raw`: thân yêu cầu là tệp WAV (`Content-Type: audio/wav`); `Multipart`: `multipart/form-data` |
| `Dictation:Http:FileField` | `file` | Tên trường chứa tệp âm thanh (`Multipart`) |
| `Dictation:Http:FormFields:<tên>` | — | Trường form khác (`Multipart`); `{{lang}}` trong giá trị được thay bằng mã ngôn ngữ |
| `Dictation:Http:ResponsePath` | `text` | Đường dẫn tới chữ trong câu trả lời JSON (chấm phân cấp, `[*]`, `[n]` như phần dịch) — ví dụ `results[0].alternatives[0].transcript`; nhiều giá trị được nối bằng dấu cách |
| `Dictation:Http:ConfidencePath` | — | Đường dẫn tới độ tin cậy (0–1), nếu dịch vụ trả |
| `Dictation:Http:LanguageMap:<mã>` | — | Mã ngôn ngữ theo cách dịch vụ viết (`vi` → `vi-VN`); không có thì gửi subtag chính (`vi`, `en`) |

Ví dụ — một máy chủ nhận dạng tự host nhận form `file` + `language` và trả `{"text": "..."}`:

```json
"Dictation": {
  "Provider": "Http",
  "Http": {
    "Url": "http://stt.internal:8080/inference",
    "Mode": "Multipart",
    "FileField": "file",
    "FormFields": { "language": "{{lang}}", "response_format": "json" }
  }
}
```

Dịch vụ nhận thân WAV thô, ngôn ngữ trên URL, trả `{"results":[{"alternatives":[{"transcript":"…","confidence":0.9}]}]}`:

```json
"Dictation": {
  "Provider": "Http",
  "Http": {
    "Url": "https://stt.example/v1/recognize?language={{lang}}",
    "Headers": { "Authorization": "Bearer <khoá>" },
    "ResponsePath": "results[0].alternatives[0].transcript",
    "ConfidencePath": "results[0].alternatives[0].confidence",
    "LanguageMap": { "vi": "vi-VN", "en": "en-US" }
  }
}
```

Gửi giọng nói ra một dịch vụ bên ngoài tổ chức là quyết định của quản trị viên (chính sách dữ liệu); với dữ liệu nhạy cảm
nên dùng dịch vụ tự host.

## API

- `GET /api/dictation/languages[?session=<id>]` → `{ "languages": ["vi", "en"] }`; 404 khi tắt.
- `POST /api/dictation?lang=vi-VN[&session=<id>]`, thân là âm thanh một câu (`Content-Type: audio/wav`) →
  `{ "text": "...", "confidence": 0.9 }` (`confidence` khi biết). Người gọi phải đăng nhập (Bearer) hoặc ở trong một
  phiên soạn thảo (`session` + header `X-Session-Token`). Lỗi là problem details có `code`: `unavailable` (404),
  `invalid` / `unsupportedLanguage` (400), 415 (không phải âm thanh), `tooLarge` (413), `rateLimited` (429),
  `upstream` (502/504).

Bản web độc lập: `VITE_BO_DICTATION` lúc build = `auto` (mặc định: máy chủ nếu đã cấu hình, không thì trình duyệt) |
`browser` | `server` | `off`; `VITE_BO_DICTATION_URL` = địa chỉ máy chủ (mặc định cùng origin).

## Model cục bộ (chưa đóng gói)

Chưa có model nhận dạng giọng nói tiếng Việt nào được xác nhận license cho phép phân phối kèm BlueOffice (cần license
MIT/Apache-2.0/BSD cho cả mã và trọng số), nên **không có model nào được đóng gói**. Hai chỗ chờ sẵn:

- Máy chủ: đăng ký `IDictationEngine` riêng (`apps/server/BlueOffice.Api/Dictation/DictationEndpoints.cs`) và đặt
  `Dictation:Provider = Custom`; hoặc chạy model như một dịch vụ HTTP nội bộ và trỏ `Provider = Http` vào đó.
- Thiết bị: `LocalDictationProvider` bọc một `LocalDictationModel` (`languages`, `load`, `transcribe` PCM 16 kHz) —
  ví dụ model WASM/ONNX trong trình duyệt; gán vào `Platform.dictation`.

## Lệnh bằng giọng nói

Nói tên dấu thay vì dấu; khớp không phân biệt hoa thường và cả hai cách bỏ dấu thanh (`xoá` / `xóa`). Chỉ lệnh của ngôn
ngữ đang chọn có hiệu lực.

| Tiếng Việt | English | Kết quả |
|---|---|---|
| chấm, dấu chấm | period, full stop | `.` |
| phẩy, dấu phẩy | comma | `,` |
| chấm hỏi, dấu chấm hỏi, dấu hỏi | question mark | `?` |
| chấm than, dấu chấm than | exclamation mark / point | `!` |
| hai chấm, dấu hai chấm | colon | `:` |
| chấm phẩy, dấu chấm phẩy | semicolon | `;` |
| ba chấm, dấu ba chấm | ellipsis, dot dot dot | `…` |
| xuống dòng, dòng mới | new line, next line | ngắt dòng trong đoạn |
| đoạn mới, sang đoạn mới | new paragraph, next paragraph | đoạn mới |
| mở ngoặc (đơn) / đóng ngoặc (đơn) | open / close parenthesis (paren, bracket) | `(` `)` |
| mở ngoặc kép / đóng ngoặc kép | open quote, begin quote / close quote, end quote, unquote | `“` `”` |
| xoá từ vừa rồi, xoá từ vừa nói, xoá từ cuối | delete last word, delete previous word | xoá từ (hoặc dấu) ngay trước con trỏ |

Khoảng trắng: các dấu `. , ; : ! ? …` và ngoặc đóng dính vào chữ trước, ngoặc mở dính vào chữ sau, các từ cách nhau một
dấu cách (tiếng Việt và tiếng Anh như nhau). Chữ đầu câu (đầu văn bản, sau ngắt dòng, sau `. ! ? …`) được viết hoa.
Lệnh khớp từ trái sang phải, cụm dài trước: "hai chấm hỏi" là `:` + "hỏi".

## Cách nhập

- Chữ **tạm** (kết quả chưa chốt) chỉ hiện gạch chân chấm tại con trỏ (BlueDoc) và trong thanh đọc chính tả, chưa vào
  tài liệu. Mỗi **câu đã chốt** được ghi một lần — **một bước hoàn tác**.
- BlueDoc: tại con trỏ (thay phần chọn). BlueSlide: vào khối chữ / ô bảng đang sửa (không có thì báo đặt con trỏ).
  BlueSheet: vào ô đang sửa tại con trỏ; chưa sửa ô nào thì mở sửa ô hiện hành (Enter để chốt như khi gõ).
- Thanh đọc chính tả: trạng thái (đang nghe / tạm dừng / lỗi), chọn ngôn ngữ (nhớ theo trình duyệt), tạm dừng / tiếp
  tục, dừng (Esc).
