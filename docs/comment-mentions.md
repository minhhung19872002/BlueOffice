# @nhắc tên và giao việc trong bình luận (CORE-COMMENT-001)

Gõ `@` trong ô bình luận (BlueDoc, BlueSheet, BlueSlide) mở bộ chọn người; nguồn là thư mục người dùng của host
qua server (`GET /api/sessions/{id}/people`, BOHP §A3 trong [`bohp.md`](bohp.md)) cộng với người đang cộng tác.
Ngoài phiên host (mở file cục bộ, desktop không có server) không có bộ chọn: `@` chỉ là chữ.

Khi đăng bình luận có người được nhắc mới, editor gọi `POST /api/sessions/{id}/mentions`; server gửi thông báo cho host
(`POST /files/{id}/notifications`) và, nếu đã cấu hình SMTP, một e-mail cho từng người có địa chỉ.

Ô "Giao cho @người" (chỉ hiện khi bình luận có người được nhắc) gán việc cho người đó: bình luận hiện "Đã giao cho X";
**giải quyết** chuỗi bình luận = hoàn thành việc, mở lại = chưa xong.

## Lưu trong tệp

| Định dạng | Nhắc tên | Người | Việc được giao |
|---|---|---|---|
| `.docx` | run "@Tên" trong `w:hyperlink` tới `mailto:<email>` (ECMA-376 Part 1, §17.16.22; quan hệ của phần `comments.xml`) — cách phổ biến | phần people [MS-DOCX] (`w15:people/w15:person w15:author`, `w15:presenceInfo providerId="None" userId=<email>`), chỉ **thêm** người còn thiếu, giữ nguyên người đã có | phần riêng BlueOffice (dưới) |
| `.xlsx` | [MS-XLSX] `threadedComment/mentions/mention` (`mentionpersonId`, `mentionId`, `startIndex`, `length`) | phần person [MS-XLSX] (`userId` = e-mail, `providerId="None"`) | `threadedComment/extLst/x:ext` BlueOffice |
| `.pptx` | [MS-PPTX] modern comments không định nghĩa phần tử nhắc tên → giữ "@Tên" trong văn bản + `a:ext` BlueOffice | (trong `a:ext`) | `p188:extLst/a:ext` BlueOffice |

Khi ghi lại, bình luận đọc từ tệp giữ nguyên: `.docx` giữ nguyên phần không đổi (ADR-0005); `.xlsx` đọc/ghi đủ `mentions`
và giữ `mentionId`; `.pptx` giữ nguyên `p188:txBody` (định dạng, cách nhắc tên của phần mềm khác) và các `ext` lạ khi văn bản
không đổi, cùng khai báo namespace của phần gốc.

### Phần mở rộng BlueOffice

Namespace `http://schemas.bluestar.vn/blueoffice/2026/comments` (tiền tố `bo`), `uri` của `ext` =
`{B10E0FF1-CE00-4A55-9C0D-7A5C0000C0DE}`. Phần mềm khác bỏ qua (`ext` không biết uri; quan hệ không biết loại — OPC).

- `.docx`: phần `/word/blueoffice/commentTasks.xml`, quan hệ từ `document.xml`
  `http://schemas.bluestar.vn/blueoffice/2026/relationships/commentTasks`, content type
  `application/vnd.bluestar.blueoffice.commentTasks+xml`:
  `<bo:commentTasks><bo:task paraId="<w14:paraId của bình luận>" assigneeId assigneeName assigneeEmail?/></bo:commentTasks>`.
  Hết việc → bỏ phần và quan hệ.
- `.xlsx`: `<extLst><x:ext uri="{B10E…}"><bo:task assigneeId assigneeName assigneeEmail?/></x:ext></extLst>` trong `threadedComment`.
- `.pptx`: `<p188:extLst><a:ext uri="{B10E…}"><bo:mention personId personName personEmail? start length/>…<bo:task assigneeId assigneeName assigneeEmail?/></a:ext></p188:extLst>`
  trong `p188:cm` / `p188:reply`.

`start`/`length` đếm theo đơn vị UTF-16 (như [MS-XLSX] `startIndex` và chuỗi JavaScript). Tệp sinh ra qua
OpenXmlValidator (`crates/bo-engine/tests/mentions.rs` ghi vào `%TEMP%/bo-mentions`).

## Cấu hình e-mail (server)

Mục `Mail` (dùng chung cho mọi e-mail server gửi):

```json
"Mail": { "SmtpHost": "smtp.example.vn", "SmtpPort": 587, "EnableSsl": true,
          "User": "blueoffice", "Password": "…", "From": "blueoffice@example.vn", "FromName": "BlueOffice" }
```

Biến môi trường: `Mail__SmtpHost`, `Mail__From`, … `SmtpHost` hoặc `From` trống → không gửi e-mail nào.
Người nhận và địa chỉ luôn lấy từ host (không tin địa chỉ editor gửi lên); tối đa 20 người mỗi bình luận.
