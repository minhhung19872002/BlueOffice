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

## Cộng tác thời gian thực (BlueDoc, BlueSlide)

Cả ba ứng dụng dùng chung một replica `bo-collab` (Yrs, offset UTF-16 như Yjs) trong chế độ nhúng
(`apps/web/src/main.tsx` → `startCollab`). Binding TS ở `packages/collab`: `collabDocHost`, `collabSlideHost`
(bọc `EngineHost`, cùng khuôn với `collabSheetHost`). Lõi ánh xạ ở `bo-engine`: `collab.rs` (BlueDoc) và
`slide_edit/collab.rs` (BlueSlide); WASM `WasmEngine.collab*` / `WasmCollab.applyLocal|takeRemote|fullState|seed`,
Tauri `engine_collab_*`.

**Đồng bộ những gì**
- BlueDoc — `body` Y.Text: mỗi block cấp section là một dòng kết thúc bằng `\n`. Định dạng run là thuộc tính:
  `bold`, `italic`, `underline`, `strike`, `size` (half-point), `color`, `font`, `highlight` + `r` (JSON phần `RunProps`
  còn lại, gồm cả dấu track changes). Thuộc tính đoạn nằm trên `\n`: `style`, `align`, `list` + `p` (phần `ParaProps`
  còn lại); `\n` cuối mỗi section mang `sect` (JSON sectPr: khổ giấy, lề, cột, header/footer). Tab, ngắt dòng/trang,
  ảnh, field, bookmark… là embed một vị trí `{"c": JSON}`; bảng và block lạ là embed `{"b": JSON}` + `\n`.
- Sau mỗi lệnh cục bộ engine so mô hình với *shadow* của văn bản chung (so block, rồi tiền tố/hậu tố theo ký tự) và
  trả `collab.change` (delta Quill) trong `EditResult`; delta từ xa áp vào shadow rồi dựng lại các block đổi, áp như
  một transaction **ngoài lịch sử cục bộ** (`bo_edit::Session::apply_external`); con trỏ của mình được dời theo delta.
- Undo/redo khi cộng tác đi qua `UndoManager` của replica (chỉ hoàn tác thay đổi của chính mình).
- BlueSlide — `slides` Y.Map (khoá `sldId`, giá trị `{"pos","slide"}`: thuộc tính slide + vị trí) và `shapes` Y.Map
  (khoá `"<sldId>/<cNvPr id>"`, giá trị `{"z","node"}`), last-writer-wins theo đối tượng; slide thêm trong phiên được
  cấp `sldId` theo client id. Ảnh/part mới (doc media, pptx part) đi qua `blobs` (base64).
- Người vào đầu tiên nhận `seed` và đăng toàn bộ thân văn bản/deck; người vào sau thay nội dung bằng trạng thái chung
  sau lần đồng bộ đầu (`fullState`). Khung nhận được khi engine đang bận được giữ lại (`LiveQueue`) để engine và
  replica luôn thấy thay đổi theo cùng thứ tự.
- Presence: BlueDoc gửi vùng chọn (offset UTF-16) → caret màu kèm tên (`DocumentView.setPeers`, lệnh
  `view.collaborators` bật/tắt); BlueSlide gửi đối tượng đang chọn → khung nét đứt màu trên slide.

**Hạn chế**
- Bảng là một đối tượng: hai người sửa cùng bảng thì người ghi sau thắng cả bảng; văn bản lạc cạnh embed bảng (do sửa
  đồng thời) bị bỏ như nhau trên mọi replica.
- Không đồng bộ trực tiếp: định nghĩa style (chỉ tự thêm style dựng sẵn khi được tham chiếu), numbering ngoài kiểu
  bullet/number, comment, footnote/endnote, header/footer ngoài thuộc tính section; slide master/layout/theme, phần
  trình bày cấp presentation (notes master). Những phần này đến người khác với version đã lưu.
- Danh sách tạo trong phiên được chia sẻ theo kiểu (`bullet:<lvl>`/`number:<lvl>`): các danh sách mới cùng kiểu dùng
  chung một định nghĩa ở replica nhận.
- Vị trí caret của người khác tính theo offset tuyệt đối (chưa dùng relative position của Yjs) nên có thể lệch tạm thời
  cho tới khi họ gửi presence mới; caret trong ô bảng hiển thị ở đầu bảng.
- Sửa trước lần đồng bộ đầu tiên của người vào sau không được chia sẻ (bị nội dung chung thay thế).
- Mỗi lệnh cục bộ so toàn bộ danh sách block với shadow (O(số block), không cấp phát) — đủ nhanh cho tài liệu
  thường; tài liệu rất dài cần đánh dấu block bẩn từ op.
- Hai người cùng thêm slide cùng lúc nhận `sldId` khác nhau, nhưng tên part `/ppt/slides/slideN.xml` có thể trùng
  (được đặt lại khi lưu).
- Desktop (Tauri) đã có lệnh engine nhưng chưa có client WebSocket (chỉ chế độ nhúng web dùng `startCollab`).
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
