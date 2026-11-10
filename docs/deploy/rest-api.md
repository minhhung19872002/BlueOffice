# REST API tài liệu cho hệ thống khác (`/api/v1`)

Cho phép phần mềm khác (ERP, HIS, cổng dịch vụ công, hệ thống báo cáo…) **chuyển đổi, trộn thư hàng loạt, sinh báo cáo từ
mẫu và kiểm tra tệp** mà không cần mở trình soạn thảo (CORE-API-001). Mã nguồn: `apps/server/BlueOffice.Api/Rest/`
(`RestApiEndpoints.cs`, `ApiKeyAuth.cs`, `openapi.v1.json`). **Mặc định tắt**: khi `RestApi:Enabled=false` các route
`/api/v1/...` không tồn tại (404).

Mỗi yêu cầu là một job của worker chuyển đổi sẵn có (`bo-cli` chạy trong thư mục tạm, không shell, có timeout — xem
[security.md](security.md) §8), nên API dùng chung hàng đợi, `Jobs__Concurrency` và `BoCli__Timeout` với `/convert`.

| Endpoint | Scope | Việc làm |
|---|---|---|
| `POST /api/v1/documents/convert` | `convert` | Chuyển đổi định dạng; PDF/A, PDF có thẻ, dấu trang |
| `POST /api/v1/documents/merge` | `merge` | Trộn thư: mẫu `.docx` + dữ liệu → một tài liệu, hoặc zip mỗi bản ghi một tệp |
| `POST /api/v1/documents/edit` | `edit` | Điền mẫu `.docx` / `.xlsx` theo danh sách chỉnh sửa JSON, trả tệp hoặc PDF |
| `POST /api/v1/documents/inspect` | `inspect` | Định dạng, biến thể, mã hoá, macro của tệp |
| `GET /api/v1/jobs/{id}`, `GET /api/v1/jobs/{id}/result` | (chủ job) | Trạng thái và kết quả của job chạy lâu |
| `GET /api/v1/openapi.json` | (ẩn danh) | Mô tả OpenAPI 3.0 (nhập vào Swagger UI, Postman, công cụ sinh client) |

## 1. Bật và cấp khoá

```json
// appsettings.json (hoặc biến môi trường RestApi__Enabled=true, RestApi__Keys__0__Name=erp, …)
"RestApi": {
  "Enabled": true,
  "AllowBearer": true,            // false: chỉ nhận khoá API, không nhận token OIDC / host
  "MaxUploadBytes": 52428800,     // tệp lớn nhất (50 MiB); không vượt Storage:MaxUploadBytes
  "MaxRecords": 5000,             // số bản ghi tối đa khi gửi dữ liệu trộn thư dạng JSON ("records")
  "MaxEdits": 1000,               // số chỉnh sửa tối đa của một yêu cầu edit
  "RateLimit": { "PermitLimit": 60, "Window": "00:01:00" },   // mỗi khoá / người dùng; 0 = không giới hạn
  "Keys": [
    { "Name": "erp", "Sha256": "<64 ký tự hex>", "Scopes": ["convert", "merge", "edit"] },
    { "Name": "luu-tru", "Sha256": "<64 ký tự hex>", "Scopes": ["inspect"], "ExpiresAt": "2027-12-31T00:00:00Z" }
  ]
}
```

Quản trị viên tạo khoá và **chỉ lưu SHA-256 của khoá** vào cấu hình; khoá gốc giao cho hệ thống gọi, máy chủ không giữ:

```bash
KEY=$(openssl rand -hex 32)                       # khoá giao cho hệ thống gọi (256 bit ngẫu nhiên)
printf '%s' "$KEY" | sha256sum | cut -d' ' -f1    # giá trị ghi vào RestApi:Keys:N:Sha256
```

```powershell
$key = -join ((1..32) | ForEach-Object { '{0:x2}' -f (Get-Random -Maximum 256) })
$sha = [BitConverter]::ToString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($key))).Replace('-', '').ToLower()
```

- `Scopes` rỗng = mọi scope. `Enabled: false` thu hồi khoá mà không xoá dòng; `ExpiresAt` = hết hạn tự động.
- Cấu hình sai (trùng `Name`, `Sha256` không phải 64 ký tự hex, scope lạ) làm máy chủ **không khởi động** — tránh chạy với
  khoá ghi nhầm dạng rõ.
- Hệ thống gọi gửi khoá ở header `X-Api-Key`. Khi `AllowBearer=true`, token `Authorization: Bearer` của policy `Service`
  (người dùng OIDC, host như BlueDrive) cũng dùng được và có mọi scope.
- Khoá API **chỉ** mở `/api/v1`; không dùng được cho `/api/files`, `/convert`, phiên soạn thảo…

## 2. Đồng bộ và bất đồng bộ

Mặc định máy chủ chờ kết quả và trả thẳng tệp (`200`). Nếu job chưa xong trong `Worker__WaitTimeout` (mặc định 60 s),
hoặc gọi với `?wait=false`, máy chủ trả `202 Accepted` kèm header `Location` và JSON:

```json
{ "id": "0f3c…", "kind": "merge", "status": "queued", "statusUrl": "/api/v1/jobs/0f3c…", "resultUrl": null }
```

Hỏi `statusUrl` đến khi `status` là `succeeded` (lấy tệp ở `resultUrl`) hoặc `failed` (`error` nêu lý do). Chỉ khoá /
người dùng tạo job mới đọc được job (người khác nhận 404). Bản ghi job giữ theo `Jobs__Retention` (mặc định 1 ngày).
Mỗi lần chạy `bo-cli` bị dừng sau `BoCli__Timeout` (job `failed`: "The operation timed out.").

## 3. Ví dụ

```bash
API=https://office.example.vn/api/v1
H="X-Api-Key: $KEY"
```

### Chuyển đổi

```bash
# .docx → PDF/A-2b có thẻ và dấu trang
curl -fsS -H "$H" -F "file=@bao-cao.docx" -F to=pdf -F pdfa=2b -F tagged=true -F bookmarks=true \
     "$API/documents/convert" -o bao-cao.pdf

# .xls → .xlsx ; .doc → .odt
curl -fsS -H "$H" -F "file=@so-lieu.xls" -F to=xlsx "$API/documents/convert" -o so-lieu.xlsx
```

Đầu vào → đầu ra: văn bản (`docx docm doc odt txt md`) → `pdf docx odt txt`; bảng tính (`xlsx xlsm xlsb xls ods`) →
`pdf xlsx ods csv`; `pptx` → `pdf pptx`; `ppt odp` → `odp txt`. `pdfa` = `1b` | `2b` | `3b` (3b đính kèm tệp gốc);
`pdfa`, `tagged`, `bookmarks` chỉ dùng với `to=pdf`. Tệp có macro giữ macro khi chuyển sang OOXML (kết quả là
`.docm`/`.xlsm`, xem `Content-Type`); macro không bao giờ được chạy. Endpoint `/convert` dành cho host (BlueDrive) cũng
nhận ba tham số `pdfa`, `tagged`, `bookmarks` này.

### Trộn thư hàng loạt

Mẫu là tài liệu `.docx` có trường `MERGEFIELD` (soạn bằng BlueDoc, thẻ Thư tín). Dữ liệu: tệp `data` (`.csv` có dòng
tiêu đề, `.json` mảng đối tượng, `.xlsx` trang đầu, `.vcf`) **hoặc** trường `records` (chuỗi JSON, mảng đối tượng).

```bash
# Một tài liệu gộp (mỗi bản ghi một phần), dạng .docx
curl -fsS -H "$H" -F "template=@thu-moi.docx" -F "data=@danh-sach.csv" "$API/documents/merge" -o thu-moi-gop.docx

# Mỗi bản ghi một PDF, đóng zip, đặt tên theo trường dữ liệu
curl -fsS -H "$H" -F "template=@thu-moi.docx" \
     -F 'records=[{"HoTen":"Nguyễn Văn An","DonVi":"Sở Kế hoạch"},{"HoTen":"Trần Thị Bình","DonVi":"Phòng Tài chính"}]' \
     -F to=pdf -F output=zip -F 'pattern=Thư mời «HoTen»' "$API/documents/merge" -o thu-moi.zip
```

`to` = `docx` (mặc định) | `pdf`; `output` = `single` (mặc định) | `zip`; `pattern` là tên tệp trong zip, `«Trường»` được
thay bằng giá trị của bản ghi (ký tự không hợp lệ trong tên tệp thành `_`, trùng tên thêm ` (2)`; bỏ trống: `1`, `2`…).

### Sinh báo cáo từ mẫu (edit)

`edits` là mảng JSON, áp dụng theo thứ tự:

| `op` | Mẫu | Tham số | Ý nghĩa |
|---|---|---|---|
| `setControl` | `.docx` | `tag` và/hoặc `title`, `value` | Điền mọi content control nội dòng có thẻ / tiêu đề đó: văn bản, một mục của danh sách, ngày ISO 8601, `true`/`false` cho hộp kiểm |
| `setBookmark` | `.docx` | `name`, `text` | Thay văn bản của bookmark (bookmark vẫn bao văn bản mới, điền lại được) |
| `replaceText` | `.docx` | `find`, `with`, `matchCase`, `wholeWord` | Thay mọi chỗ xuất hiện của chuỗi giữ chỗ, vd `{{ngay}}`; `\n` trong `with` = xuống đoạn |
| `setProperties` | `.docx` | `props` (`title subject author keywords comments category status company manager`) | Thuộc tính tài liệu |
| `setCell` | `.xlsx` | `ref`, `value` | Ô theo tham chiếu (`B7`, `'Sổ 1'!B7`) hoặc tên đã đặt; chuỗi bắt đầu bằng `=` là công thức; `null` xoá ô |

```bash
curl -fsS -H "$H" -F "file=@hop-dong-mau.docx" -F to=pdf -D headers.txt \
     -F 'edits=[{"op":"setControl","tag":"ho_ten","value":"Nguyễn Văn An"},
                {"op":"setBookmark","name":"SoHopDong","text":"12/2026/HĐ"},
                {"op":"replaceText","find":"{{ngay}}","with":"10/11/2026"},
                {"op":"setProperties","props":{"title":"Hợp đồng 12/2026"}}]' \
     "$API/documents/edit" -o hop-dong.pdf

# Bảng tính: điền ô đã đặt tên rồi xuất PDF (công thức được tính lại)
curl -fsS -H "$H" -F "file=@bao-cao-thang.xlsx" -F to=pdf \
     -F 'edits=[{"op":"setCell","ref":"TyGia","value":25000},{"op":"setCell","ref":"Thang","value":"11/2026"}]' \
     "$API/documents/edit" -o bao-cao-thang.pdf
```

- `strict` (mặc định `true`): một chỉnh sửa **không khớp gì** (sai thẻ, sai tên bookmark, giá trị không có trong danh
  sách thả xuống…) làm yêu cầu thất bại `422` — báo cáo không bị phát hành khi điền thiếu. `strict=false`: vẫn trả tệp;
  header `X-Edits-Applied` / `X-Edits-Unmatched` cho biết số chỉnh sửa có / không có tác dụng.
- Danh sách chỉnh sửa lớn hơn 4 MB: gửi `edits` như một phần tệp (`-F "edits=@edits.json"`).
- `to` mặc định là định dạng của mẫu (`docx` / `xlsx`), hoặc `pdf`.

### Kiểm tra tệp

```bash
curl -fsS -H "$H" -F "file=@tep-nhan-duoc.bin" "$API/documents/inspect"
# {"fileName":"tep-nhan-duoc.bin","format":"docx","variant":"docm","encrypted":false,"opened":true,"macros":true,
#  "kind":"document","paragraphs":42}
```

Định dạng được nhận theo **nội dung**, không theo đuôi tệp. Tệp có mật khẩu: `"encrypted": true, "opened": false`
(API không nhận mật khẩu). Bảng tính trả `sheets`, bản trình chiếu trả `slides`.

### Job chạy lâu

```bash
JOB=$(curl -fsS -H "$H" -F "template=@thu.docx" -F "data=@10000-ban-ghi.csv" -F output=zip "$API/documents/merge?wait=false")
URL=https://office.example.vn$(echo "$JOB" | jq -r .statusUrl)
while :; do
  STATUS=$(curl -fsS -H "$H" "$URL" | jq -r .status)
  [ "$STATUS" = queued ] || [ "$STATUS" = running ] || break
  sleep 2
done
[ "$STATUS" = succeeded ] && curl -fsS -H "$H" "$URL/result" -o thu.zip
```

## 4. Mã lỗi

Lỗi trả theo RFC 7807 (`application/problem+json`, trường `detail`).

| Mã | Khi nào |
|---|---|
| `400` | Thiếu tệp / tham số sai / định dạng không hỗ trợ / JSON `edits`, `records` sai / vượt `MaxEdits`, `MaxRecords` |
| `401` | Không có hoặc sai khoá (khoá bị thu hồi, hết hạn) |
| `403` | Khoá không có scope của endpoint |
| `404` | API đang tắt, hoặc job không tồn tại / không phải của người gọi |
| `409` | `GET …/result` khi job chưa thành công (thân là trạng thái job) |
| `413` | Tệp lớn hơn `RestApi:MaxUploadBytes` |
| `422` | `bo-cli` không xử lý được tệp (tệp hỏng, mẫu không có dữ liệu trộn, chỉnh sửa không khớp khi `strict`…) hoặc quá thời gian |
| `429` | Vượt giới hạn tần suất; header `Retry-After` (giây) |

## 5. Ghi chú an toàn

- **Mặc định tắt**; khi bật, mọi endpoint (trừ `openapi.json`) đều cần khoá API hoặc token. Khoá chỉ lưu dạng băm,
  so sánh thời gian hằng; cấp mỗi hệ thống một khoá riêng với scope tối thiểu và đặt `ExpiresAt` để buộc xoay khoá.
- Chỉ phát hành API qua HTTPS (reverse proxy, README §3); khoá nằm trong header nên không lọt vào log truy cập theo URL.
- Tệp tải lên là **dữ liệu không tin cậy**: xử lý trong tiến trình `bo-cli` riêng, thư mục tạm riêng, tên tệp của client
  không bao giờ thành đường dẫn hay tham số dòng lệnh, macro không chạy, có timeout và giết cả cây tiến trình.
- Giới hạn: kích thước (`MaxUploadBytes`, toàn thân yêu cầu ≤ 2 tệp + 1 MiB), số bản ghi / chỉnh sửa, số job song song
  (`Jobs__Concurrency`), thời gian (`BoCli__Timeout`, `Worker__WaitTimeout`).
- Giới hạn tần suất tính **theo từng khoá / người dùng, trong từng tiến trình `api`** (nhiều instance: mỗi instance một
  cửa sổ) và chỉ áp dụng sau khi xác thực. Yêu cầu sai khoá bị từ chối `401` trước đó — nên đặt thêm giới hạn theo IP ở
  reverse proxy cho `/api/v1/` để chống dò khoá và làm ngập.
- Mỗi yêu cầu được ghi log (`BlueOffice.RestApi`: loại job, mã job, `apikey:<tên>` / `user:<sub>`); nội dung tệp không ghi log.
- Tệp vào / ra của job nằm trong object store dưới `jobs/<id>/` như các job chuyển đổi khác.

## 6. Hạn chế hiện tại

- Khoá quản lý bằng cấu hình (đổi khoá = sửa cấu hình và khởi động lại `api`); chưa có màn hình quản trị.
- `setControl` chỉ điền content control **nội dòng** (trong một đoạn); control dạng khối (bao nhiều đoạn), control ảnh,
  nhóm và phần lặp chưa hỗ trợ. `setBookmark` chỉ với bookmark nằm trong một đoạn của thân văn bản; bookmark rỗng nhận
  văn bản ngay tại vị trí của nó.
- `edit` chưa áp dụng cho `.pptx`; `convert` chưa nhận tệp có mật khẩu.
- `bo-cli` tương ứng: `bo-cli edit <mẫu> --edits edits.json --out <tệp> [--to pdf] [--strict]`,
  `bo-cli merge … --to zip --each docx|pdf`, `bo-cli inspect <tệp> --json`.
