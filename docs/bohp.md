# BlueOffice Host Protocol (BOHP) v1

Giao thức REST giữa **host lưu trữ** (BlueDrive) và **BlueOffice server** (prompt §6A.1). Host cài đặt phần A;
BlueOffice cài đặt phần B–D. Bản cài tối thiểu để test: `tests/mock-bluedrive`. Client phía BlueOffice:
`apps/server/BlueOffice.Infrastructure/Hosts/BohpClient.cs`.

## Xác thực

- Host phát **access token ngắn hạn** (JWT, RS256/ES256) cho người dùng: `iss` = issuer host, `aud` = `blueoffice`,
  `sub` = id người dùng, `name`, `exp` (khuyến nghị ≤ 10 phút, refresh qua editor), tuỳ chọn `fid` = id file (token chỉ dùng
  cho file đó).
- BlueOffice xác minh token bằng **JWKS** của host (`Hosts:N:JwksUrl`, cache 10 phút, tự tải lại khi gặp `kid` mới).
  BlueOffice không lưu mật khẩu người dùng.
- Mọi lời gọi BOHP từ BlueOffice gửi lại đúng token đó: `Authorization: Bearer <token>`.
- Token dịch vụ (vd. `sub=bluedrive-service`) dùng cho `/convert`, `/thumbnail`, `/preview`, `/extract-text`
  và `POST /api/hosts/{host}/notify`.

## A. Endpoint host phải cài (base URL `Hosts:N:BaseUrl`, vd. `https://drive/bohp/v1`)

| Method & path | Mô tả | Thành công | Lỗi |
|---|---|---|---|
| `GET /files/{id}` | Metadata + quyền của người dùng hiện tại | 200 `FileInfo` | 401, 403, 404 |
| `GET /files/{id}/contents` | Nội dung | 200 bytes, header `ETag: "<version>"` hoặc `X-BOHP-Version` | 401, 403, 404 |
| `POST /files/{id}/contents` | Lưu (body = bytes) | 200 `{"version"}` | 409 xung đột, 403, 413 |
| `POST /files/{id}/lock` | Khoá `{"lockId","expiresInSeconds"}` | 200 `{"lockId","expiresAt"}` | 409 nếu khoá bởi lockId khác |
| `POST /files/{id}/refresh-lock` | Gia hạn | 200 như trên | 409 nếu không còn khoá / khác lockId |
| `POST /files/{id}/unlock` | Mở khoá `{"lockId"}` | 200 | 409 nếu khác lockId |
| `GET /files/{id}/versions` | Lịch sử, mới nhất trước | 200 `{"versions":[Version]}` | |
| `POST /files/{id}/versions/{v}/restore` | Khôi phục `v` thành phiên bản mới | 200 `{"version"}` | 404 |
| `POST /files/{id}/rename` | `{"name"}` (kèm phần mở rộng) | 200 `{"name"}` | 409 trùng tên |
| `POST /folders/{folderId}/files?name=` | Tạo file (Save As, bản xung đột); body = bytes | 201 `{"id","name","version"}` | 403, 409 |

`FileInfo`:

```json
{
  "id": "f42", "name": "Báo cáo.docx", "size": 18234, "version": "7",
  "ownerId": "u1", "folderId": "d3", "lastModified": "2026-11-07T07:30:00Z",
  "shareUrl": "https://drive/s/f42",
  "permissions": { "view": true, "comment": true, "edit": true, "share": false, "download": true },
  "user": { "id": "u2", "name": "Trần Thị Bình", "avatarUrl": null },
  "lock": { "lockId": "…", "expiresAt": "…" }
}
```

`Version`: `{"version","size","modifiedAt","modifiedBy":{"id","name"},"note"}`.

Header của `POST /files/{id}/contents`:

| Header | Ý nghĩa |
|---|---|
| `If-Match: "<version>"` | Chỉ ghi nếu version hiện tại bằng giá trị này; khác → **409** |
| `X-BOHP-Lock: <lockId>` | Khoá của phiên; file đang khoá bởi lockId khác → **409** |
| `X-BOHP-Note` | Ghi chú phiên bản (URL-encoded), vd. "Edited in BlueOffice by An, Bình" |
| `X-BOHP-Editors` | Danh sách id người đã sửa (URL-encoded, phân cách bằng dấu phẩy) |

Body 409: `{"error":"version-conflict"|"lock-conflict","currentVersion":"8","lockId":"…"}`.

Version là chuỗi mờ (opaque) do host quản lý. Khoá có hạn (BlueOffice xin `Hosts:N:LockDurationSeconds`, mặc định 1800 s,
gia hạn mỗi nửa thời hạn). **Khoá thuộc về phiên cộng tác** (một lockId cho cả room), không thuộc từng người.

## A2. Duyệt file (tuỳ chọn) — màn hình Start desktop, thư mục đồng bộ

Host cài thêm các endpoint sau nếu muốn desktop có tab "BlueDrive" (duyệt thư mục, gần đây, được chia sẻ) và nhận diện file
trong thư mục đồng bộ. Gọi bằng **token người dùng không gắn `fid`** (token có `fid` → 403). Desktop không gọi thẳng host:
BlueOffice server proxy qua `GET /api/hosts/{host}/me`, `/folders/{id}/children`, `/shared`, `/recent?limit=`,
`/resolve?path=` (token gửi ở `Authorization: Bearer`, server xác minh bằng JWKS của host rồi chuyển nguyên token cho host;
host WOPI → 501). Danh sách host + cấu hình đăng nhập: `GET /api/hosts` (không bí mật).

| Method & path | Mô tả | Thành công | Lỗi |
|---|---|---|---|
| `GET /me` | Người dùng của token | 200 `{"id","name","avatarUrl"}` | 401, 403 |
| `GET /folders/{folderId}/children` | Thư mục con + file người dùng được xem; `root` = thư mục gốc | 200 `Listing` | 401, 403, 404 |
| `GET /shared` | File người khác chia sẻ với người dùng | 200 `{"items":[Item]}` | 401, 403 |
| `GET /me/recent?limit=` | File người dùng mở gần đây (mới nhất trước) | 200 `{"items":[Item]}` | 401, 403 |
| `GET /sync/resolve?path=` | File ở đường dẫn tương đối (`/`) trong thư mục đồng bộ của người dùng | 200 `{"id","name","version"}` | 404 |

`Item`: `{"kind":"folder"|"file","id","name","size?","version?","lastModified?","ownerId?","openedAt?"}`;
`Listing`: `{"folder":{"id","name","parentId"},"items":[Item]}`. Server từ chối `path` rỗng, có `.`/`..` hoặc dài quá 1024 ký tự (400).

**Đăng nhập desktop** (`Hosts:N:SignIn` = `AuthorizeUrl`, `TokenUrl`, `ClientId`, `Scope`): OAuth 2.0 authorization code
(RFC 6749 §4.1) + PKCE S256 (RFC 7636) trong trình duyệt hệ thống, redirect về deep link `bluedoc://auth` (`bluesheet://auth`,
`blueslide://auth`; RFC 8252 §7.1). Desktop tự đổi code lấy token (token endpoint phải cho phép CORS từ `http://tauri.localhost`).
Access token nhận được là token BOHP của người dùng. Không cấu hình → desktop chỉ cho dán token.

**Mock** (`tests/mock-bluedrive`): một cây thư mục chung bắt đầu từ `root`; "thư mục đồng bộ" là ánh xạ đường dẫn
`Thư mục/Con/Tên file` ↔ id theo cây đó; `/me/recent` ghi lại khi người dùng tải nội dung file; `GET /oauth/authorize`
(tự đăng nhập người dùng `login_hint`, không có thì hiện trang chọn) + `POST /oauth/token` (kiểm PKCE S256, mã dùng một lần);
`/_admin/folders` tạo thư mục; `/_admin/webhooks/fail {"count","status"}` làm webhook thất bại N lần (thử retry);
bản ghi webhook có `signatureValid` (chữ ký + timestamp ±5 phút) và `attempt`.

## B. Mở editor

- Iframe: `GET https://office.<domain>/edit?host=bluedrive&file={id}&token={jwt}` → JSON `SessionInfo`
  (shell web đọc cùng dữ liệu). Tương đương `POST /api/sessions` `{"host","fileId","token"}` → 201.
- WOPI: `POST /edit?WOPISrc=<url>` form `access_token=` ([MS-WOPI] §3.1.5.1.1.2); discovery `GET /hosting/discovery`.

`SessionInfo`: `sessionId`, `sessionToken` (bí mật, chỉ trả một lần), `expiresAt`, `host`, `protocol`,
`file{id,name,size,version,ownerId,shareUrl,lastModified}`, `user`, `permissions`, `mode` (`edit|comment|view`),
`collab{url,protocol}`, `endpoints`, `allowedOrigins` (whitelist `postMessage`), `autosaveSeconds`.

API phiên (header `X-Session-Token`): `GET /api/sessions/{id}`, `DELETE`, `GET …/contents`, `POST …/checkpoint[?save=true&overwrite=true]`
(body = file đã serialize bởi editor), `POST …/refresh` `{"token"}` (token mới + đọc lại quyền), `POST …/audit`
`{"action":"print|export|download","detail"}`, `GET …/versions`, `POST …/versions/{v}/restore`, `POST …/rename`,
`POST …/save-as?name=&folderId=`, `POST …/sync?baseVersion=` (sửa offline, xem D).

## C. Cộng tác (`WS /collab/{fileId}?session=&token=`)

- Frame nhị phân theo **y-protocols**: `0` sync (step1/step2/update), `1` awareness, `3` query-awareness. Server không giữ
  Y.Doc: relay update, lưu nhật ký update (snapshot định kỳ vào object store) và phát lại cho người vào sau.
  Người vào đầu tiên có quyền sửa là *seeder* (nạp tài liệu từ file rồi gửi full state).
- Frame text JSON (server → client): `welcome`, `collaborators-changed`, `mode-changed{mode,reason}`, `save-request{reason}`,
  `saved{version,editors,reason}`, `save-conflict`, `save-error`, `session-revoked`, `reload{version}`, `seed`, `pong`.
  Client → server: `{"type":"ping"}`, `{"type":"save"}`.
- Lưu về host: autosave mỗi `autosaveSeconds` (30–60 s) khi có thay đổi, nút Lưu, và khi người cuối rời phiên.
  Server gửi `save-request` cho một editor; editor trả lời bằng `POST …/checkpoint`. Mỗi lần lưu tạo version kèm danh sách người sửa.
- Mã đóng: `4001` reload (sau restore), `4401` phiên hết hạn, `4403` mất quyền, `4009` phòng đang ở instance `api` khác
  (lý do = id instance; client kết nối lại có backoff, xem `docs/deploy/README.md` "Nhiều instance").

## D. Quyền, offline, webhook

- Quyền đọc lại từ host khi mở, khi `refresh`, định kỳ (`Collab:PermissionRecheckInterval`) và khi host gọi
  `POST /api/hosts/{host}/notify` `{"event":"permissions-changed"|"file-changed","fileId","userId?"}` (token dịch vụ của host).
  Mất quyền sửa → `mode-changed` sang `view` ngay; mất quyền xem → `session-revoked` + đóng `4403`.
- Sửa offline: `POST /api/sessions/{id}/sync?baseVersion=<version gốc>` body = file. Nếu host chưa đổi và phiên cộng tác
  không có thay đổi chưa lưu → lưu thành version mới (`{"outcome":"saved"}`); ngược lại **không ghi đè**, tạo
  `"<tên> (xung đột - <người dùng> - yyyy-MM-dd HH-mm).<ext>"` cùng thư mục (`{"outcome":"conflict-copy","conflictCopy":{…}}`).
- Webhook BlueOffice → host (`Hosts:N:WebhookUrl`): `POST` JSON `{"event","host","fileId","at","data"}`, header
  `X-BlueOffice-Event`, `X-BlueOffice-Timestamp` (unix s), `X-BlueOffice-Signature: sha256=hex(HMAC-SHA256(secret, timestamp + "." + body))`.
  Sự kiện: `file.saved`, `file.conflict-copy`, `session.started`, `session.ended`, `audit` (open/edit/export/print/download/restore).
  Thử lại khi lỗi mạng / hết thời gian / 5xx / 429: tổng `Webhooks:MaxAttempts` lần (mặc định 3), chờ `n² × Webhooks:RetryBaseDelay`
  (mặc định 1 s, 4 s); 4xx khác là kết thúc. Host nên trả 2xx nhanh và kiểm tra timestamp (±5 phút).
- Audit gửi qua webhook `audit` (`data`: `action`, `actor`, `actorName`, `at`, `detail`, `ip`): `open` khi tạo phiên, `edit` mỗi
  lần lưu, `restore`, và `print` / `export` / `download` do editor báo (`POST /api/sessions/{id}/audit`; editor nhúng và desktop
  tự báo khi in hoặc xuất định dạng khác — PDF, CSV; `export`/`download` cần quyền `download`).
- Desktop sửa offline: khi server không truy cập được, nút Lưu ghi bản sao vào thư mục cache cục bộ (`<app data>/offline`) kèm
  version gốc; có mạng lại trong cùng phiên thì CRDT gửi lại thay đổi và bản sao bị bỏ; nếu phiên đã kết thúc (đóng app),
  lần mở file tiếp theo gửi bản sao qua `…/sync?baseVersion=` → version mới hoặc bản "(xung đột - …)". Mất mạng ngay lúc
  mở file: desktop mở bản tải gần nhất trong cache, lưu cũng vào hàng chờ đó.
