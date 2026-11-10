# Dịch văn bản (CORE-LANG-001)

Lệnh **Xem lại › Ngôn ngữ › Dịch** của BlueDoc, BlueSheet và BlueSlide dịch qua `TranslationProvider` của nền tảng
(`packages/platform/src/translation.ts`); giao diện không tự gọi dịch vụ nào. Bản web (nhúng / mở file từ host) và bản
desktop mở bằng deep link dịch qua **máy chủ BlueOffice**: `POST /api/translate`, máy chủ chuyển tiếp tới dịch vụ dịch
HTTP mà quản trị viên cấu hình.

**Mặc định tắt.** Khi chưa cấu hình (`Translation:Provider = None`), `/api/translate` trả 404 `unavailable`, ngăn Dịch
chỉ báo "chưa có dịch vụ dịch" và **không có văn bản nào được gửi đi đâu**. Máy chủ không ghi log và không lưu văn bản
được dịch (chỉ ghi mã trạng thái khi dịch vụ lỗi).

## Cấu hình máy chủ (`appsettings.json` hoặc biến môi trường `Translation__…`)

| Khoá | Mặc định | Ý nghĩa |
|---|---|---|
| `Translation:Provider` | `None` | `None` (tắt), `Http` (dịch vụ HTTP bên dưới), `Custom` (một `ITranslationEngine` do bản triển khai tự đăng ký, ví dụ model cục bộ) |
| `Translation:Languages` | `["vi","en"]` | Mã ngôn ngữ đưa ra cho người dùng |
| `Translation:MaxTexts` / `MaxChars` | `128` / `20000` | Giới hạn mỗi yêu cầu (vượt → 413 `tooLarge`; trình soạn thảo tự chia lô nhỏ hơn) |
| `Translation:Timeout` | `00:00:30` | Thời gian chờ dịch vụ |
| `Translation:Http:Url` | — | Địa chỉ dịch vụ (bắt buộc khi `Provider = Http`) |
| `Translation:Http:Headers:<Tên>` | — | Header gửi kèm (xác thực: `Authorization`, khoá API …) |
| `Translation:Http:BodyTemplate` | `{"q":{{texts}},"source":{{from}},"target":{{to}},"format":"text"}` | Thân JSON; `{{texts}}` (mảng JSON), `{{text}}` (chuỗi JSON, khi `PerText`), `{{from}}`, `{{to}}` được thay bằng giá trị đã mã hoá JSON |
| `Translation:Http:PerText` | `false` | Gửi mỗi đoạn văn bản một yêu cầu (API chỉ nhận một chuỗi) |
| `Translation:Http:ResponsePath` | `translatedText` | Đường dẫn tới bản dịch trong câu trả lời: chấm phân cấp, `[*]` duyệt mảng, `[n]` chọn phần tử — ví dụ `translations[*].text`, `data.translations[*].translatedText` |
| `Translation:Http:AutoSource` | `auto` | Giá trị gửi đi khi chưa biết ngôn ngữ nguồn |
| `Translation:Http:LanguageMap:<mã>` | — | Mã ngôn ngữ theo cách dịch vụ viết (`en` → `EN-US`) |

Ví dụ — một máy chủ dịch tự host trong mạng nội bộ nhận `{"q": [...], "source": "vi", "target": "en"}` và trả
`{"translatedText": [...]}`:

```json
"Translation": {
  "Provider": "Http",
  "Http": {
    "Url": "http://translate.internal:5000/translate",
    "Headers": { "Authorization": "Bearer <khoá>" }
  }
}
```

Dịch vụ trả `[{"text": "..."}]` cho từng đoạn trong `translations`, nhận `{"text": [...], "target_lang": "EN-US"}`:

```json
"Translation": {
  "Provider": "Http",
  "Http": {
    "Url": "https://translator.example/v2/translate",
    "Headers": { "Authorization": "<khoá API>" },
    "BodyTemplate": "{\"text\":{{texts}},\"source_lang\":{{from}},\"target_lang\":{{to}}}",
    "ResponsePath": "translations[*].text",
    "AutoSource": "",
    "LanguageMap": { "en": "EN-US", "vi": "VI" }
  }
}
```

Gửi văn bản ra một dịch vụ bên ngoài tổ chức là quyết định của quản trị viên (chính sách dữ liệu); với dữ liệu nhạy cảm
nên dùng dịch vụ tự host.

## API

- `GET /api/translate/languages[?session=<id>]` → `{ "languages": ["vi", "en"] }`; 404 khi tắt.
- `POST /api/translate` thân `{ "texts": [...], "from": "vi" | "en" | "auto", "to": "en", "session": "<id>" }` →
  `{ "texts": [...] }` cùng thứ tự. Người gọi phải đăng nhập (Bearer) hoặc ở trong một phiên soạn thảo (`session` +
  header `X-Session-Token`). Lỗi là problem details có `code`: `unavailable` (404), `invalid` / `unsupportedLanguage` /
  `sameLanguage` (400), `tooLarge` (413), `rateLimited` (429), `upstream` (502/504).

Trình soạn thảo: bản web độc lập gọi máy chủ BlueOffice **cùng origin**, hoặc địa chỉ trong `VITE_BO_TRANSLATE_URL` lúc
build (`off` = không có provider), kèm token đăng nhập; máy chủ chưa cấu hình thì trả 404 và ngăn Dịch báo chưa có dịch
vụ. File mở từ host (web nhúng, desktop deep link) dùng máy chủ của phiên (`X-Session-Token`).

## Model cục bộ (chưa đóng gói)

Chưa có model dịch Việt ↔ Anh nào được xác nhận license cho phép phân phối kèm BlueOffice, nên **không có model nào
được đóng gói**. Hai chỗ chờ sẵn để gắn model sau khi duyệt license:

- Máy chủ: đăng ký `ITranslationEngine` riêng (`apps/server/BlueOffice.Api/Translation/TranslationEndpoints.cs`) và đặt
  `Translation:Provider = Custom`; hoặc chạy model như một dịch vụ HTTP nội bộ và trỏ `Provider = Http` vào đó.
- Thiết bị: `LocalTranslationProvider` bọc một `LocalTranslationModel` (`languages`, `load`, `translate`) — ví dụ model
  WASM/ONNX chạy trong trình duyệt; gán vào `Platform.translation` của bản web/desktop.

## Cách dịch

- **Phần chọn** (BlueDoc, chữ đang sửa trong BlueSlide, ô hiện hành trong BlueSheet): kết quả hiện trong ngăn Dịch, sửa
  được, rồi **Chèn** (sau phần chọn), **Thay thế** hoặc **Sao chép**.
- **Toàn bộ tài liệu** (BlueDoc) và **toàn bộ bản trình chiếu** (BlueSlide) tạo **bản sao** "Tên (EN).docx/.pptx" chưa
  lưu; bản gốc không đổi (lưu bản gốc trước nếu đang có thay đổi chưa lưu). Văn bản được đọc và dịch hết rồi mới mở bản
  sao, nên huỷ giữa chừng không để lại gì.
  - BlueDoc: mỗi đoạn văn được cắt thành các *đoạn chữ* (dừng ở tab, ngắt dòng/trang, ảnh, field, tham chiếu chú thích,
    content control); mỗi đoạn chữ dịch nguyên câu, bản dịch được chia lại cho các run **theo tỉ lệ độ dài cũ** (cắt ở
    ranh giới từ gần nhất) nên run in đậm / nghiêng / màu giữ định dạng trên phần tương ứng (gần đúng — trật tự từ mỗi
    ngôn ngữ khác nhau); kiểu đoạn, đánh số, căn lề giữ nguyên, số đoạn không đổi (`bo_edit::translate`). Chưa dịch
    header/footer, chú thích, comment, text box.
  - BlueSlide: chữ của các hình (không gồm bảng) và ghi chú, dịch theo từng đoạn / dòng, giữ định dạng cấp đoạn.
- **BlueSheet**: các ô chữ trong vùng chọn (bỏ qua số, công thức, ô trống) dịch **sang cột mới** chèn bên phải vùng chọn
  hoặc **tại chỗ** (một bước hoàn tác).
- Ngôn ngữ nguồn **Tự nhận biết**: mỗi đoạn được đoán theo dấu tiếng Việt (`detectLanguage`); đoạn ngắn không đoán được
  theo ngôn ngữ chung của cả phần; đoạn đã ở ngôn ngữ đích giữ nguyên.
