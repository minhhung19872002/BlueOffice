# Phase 6 / 6A — Server, cộng tác, tích hợp BlueDrive (phía .NET)

Trạng thái: **server hoàn thành — chờ duyệt**. Prompt: [`BlueOffice-PROMPT.md`](../../BlueOffice-PROMPT.md) §6 Phase 6, 6A.
Phần Rust `bo-collab` và `@blueoffice/editor-sdk` / TS làm riêng. Giao thức: [`docs/bohp.md`](../bohp.md) ·
Triển khai: [`docs/deploy/`](../deploy/README.md).

## Đã làm

- **Kiến trúc**: `BlueOffice.Domain` (entity, quyền, hợp đồng host `IHostStorage`, job) · `BlueOffice.Infrastructure`
  (EF Core SQLite/PostgreSQL, object store filesystem/MinIO, hàng đợi in-memory/Redis, `BohpClient`, `WopiClient`,
  JWKS validator, webhook dispatcher, `ConversionJobProcessor`) · `BlueOffice.Api` (Minimal API, WebSocket, background services).
  Một image chạy `Role=all|api|worker`.
- **File/thư mục**: CRUD, upload/download (giới hạn kích thước, SHA-256, tên an toàn), phiên bản + khôi phục, `If-Match`/409,
  thùng rác mềm, chia sẻ owner/editor/commenter/viewer (kế thừa theo thư mục), link chia sẻ có hạn, audit log.
- **Xác thực**: OIDC/JWT (Keycloak), token host xác minh bằng JWKS, token dịch vụ của host; dev key bị chặn ở Production.
- **Cộng tác**: một room cho mỗi file (`host/fileId`), relay y-protocols sync + awareness, seeder, nhật ký update lưu
  snapshot/incremental vào object store và phát lại cho phiên sau, compaction, presence (`collaborators-changed`), quyền
  view/comment không được sửa, khoá BOHP thuộc room, autosave 45 s (debounce) + lưu khi người cuối rời, version kèm danh sách
  người sửa, restore → `reload`.
- **Phase 6A**: phiên editor (`/edit`, `/api/sessions`) cho local / BOHP / WOPI, refresh token + đọc lại quyền (định kỳ và
  qua `POST /api/hosts/{host}/notify`) → `mode-changed`/`session-revoked`, audit chuyển về host, webhook ký HMAC
  (`file.saved`, `file.conflict-copy`, `session.started/ended`, `audit`), sửa offline → bản "(xung đột - người dùng - thời gian)",
  WOPI client theo [MS-WOPI] (CheckFileInfo, GetFile, PutFile, Lock/Unlock/RefreshLock, RenameFile, PutRelativeFile, discovery).
- **Dịch vụ phụ trợ**: `POST /convert`, `/thumbnail`, `/preview`, `/extract-text` qua hàng đợi (Redis) + worker `bo-cli`;
  `?wait=true` hoặc poll `/jobs/{id}`.
- **Mock BlueDrive** (`tests/mock-bluedrive`): BOHP v1, JWKS RS256, khoá, phiên bản, webhook receiver, endpoint `/_admin/*`
  để điều khiển test; chạy độc lập `dotnet run --project tests/mock-bluedrive`.
- **Triển khai**: `deploy/Dockerfile`, `deploy/docker-compose.yml` (api, worker, postgres, minio, redis, keycloak tuỳ chọn),
  hướng dẫn cài đặt / sao lưu / nâng cấp tiếng Việt.

## Endpoint

| Nhóm | Endpoint |
|---|---|
| Hệ thống | `GET /health`, `/health/ready`, `/api/version` |
| Người dùng | `GET /api/me` |
| Thư mục | `GET/POST /api/folders`, `GET/PATCH/DELETE /api/folders/{id}`, `GET /api/shared` |
| File | `POST /api/files?name=&folderId=`, `GET/PATCH/DELETE /api/files/{id}`, `GET/PUT /api/files/{id}/content[?version=]`, `GET /api/files/{id}/versions`, `POST /api/files/{id}/versions/{n}/restore` |
| Chia sẻ | `GET/POST /api/files/{id}/shares`, `GET/POST /api/folders/{id}/shares`, `DELETE /api/shares/{id}`, `POST /api/files/{id}/links`, `GET /api/links/{token}[/content]` |
| Audit | `GET/POST /api/files/{id}/audit` |
| Phiên editor | `GET/POST /edit`, `POST /api/sessions`, `GET/DELETE /api/sessions/{id}`, `GET …/contents`, `POST …/checkpoint`, `…/refresh`, `…/audit`, `GET …/versions`, `POST …/versions/{v}/restore`, `…/rename`, `…/save-as`, `…/sync` |
| Host | `POST /api/hosts/{host}/notify`, `GET /hosting/discovery` |
| Cộng tác | `WS /collab/{fileId}?session=&token=` (hoặc `?access_token=` cho file local) |
| Chuyển đổi | `POST /convert`, `/thumbnail`, `/preview`, `/extract-text`, `GET /jobs/{id}`, `/jobs/{id}/result`, `POST /api/convert/pdf` (Phase 1) |

## Kiểm thử

`dotnet test BlueOffice.sln` — xUnit + WebApplicationFactory, SQLite tạm + object store filesystem + hàng đợi in-memory,
`bo-cli` giả (`FakeRunner`). Gồm: ma trận quyền, vòng đời file, link hết hạn, audit, relay Yjs/awareness, snapshot và phát lại,
hạ quyền giữa phiên, các kịch bản BlueDrive (mở file, 2 người cùng phiên + một khoá, lưu tạo version kèm người sửa, autosave,
lưu khi rời phiên, thu hồi quyền, sửa offline → bản xung đột, khoá bởi client khác), WOPI host giả, job chuyển đổi.

## Còn lại (ngoài phạm vi lần này)

- **Ký số** (Phase 6): ký PDF PAdES và OOXML XML-DSig; điểm tích hợp dự kiến: một `ISignatureProvider` (USB token qua agent
  cục bộ trên desktop, ký số từ xa của CA Việt Nam qua REST) + job `sign` trong worker; chưa code.
- **Desktop**: auto-update (Tauri updater), file association, nhiều cửa sổ, recent files, deep link `bluedoc://open?host=…` —
  làm ở phần desktop.
- `bo-collab` (Yrs) và editor SDK/TS (postMessage, nút Share/Close, chế độ offline) — luồng chính.
- Nhiều instance `api` (room đang ở bộ nhớ một tiến trình): cần định tuyến theo file (consistent hashing) hoặc
  Redis pub/sub cho relay.
- EF Core migrations (hiện `EnsureCreated`); Testcontainers cho PostgreSQL/MinIO/Redis (hiện test dùng SQLite/filesystem/in-memory).
- Chuyển đổi định dạng ngoài docx→pdf (doc→docx, xls→xlsx, ppt→pptx, xlsx/pptx→pdf) phụ thuộc `bo-cli` của Phase 3–5;
  `BoCli:AllowedExtensions` hiện là `.docx/.txt/.md`.
- Server chưa giữ Y.Doc (không merge được bản offline dạng CRDT ở server): khi xung đột luôn tạo bản sao.
