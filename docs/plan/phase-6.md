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
(`packages/hosted` → `openHosted` / `startCollab`, dùng chung cho web nhúng và deep link desktop). Binding TS ở
`packages/collab`: `collabDocHost`, `collabSlideHost`
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
- Server không giữ Y.Doc: sau khi mất kết nối, client đăng lại **toàn bộ** trạng thái replica (`snapshot`) nếu đã sinh
  thay đổi cục bộ kể từ lần kết nối trước (nhật ký room lớn thêm cỡ một tài liệu; compaction thu gọn lại).

## Kiểm chứng end-to-end (server thật + mock BlueDrive)

Tiêu chí §6A ("mở file từ web, mở từ desktop qua deep link, 2 người cùng sửa, lưu tạo version, thu hồi quyền giữa chừng,
sửa offline rồi đồng bộ") được kiểm bằng E2E chạy **server thật**, không giả lập giao thức:

| Thành phần | Cách chạy trong E2E |
|---|---|
| `tests/mock-bluedrive` | `dotnet run` — BOHP, JWKS RS256, `/_admin/*` (tạo file, quyền, token, xem version) |
| `apps/server/BlueOffice.Api` | `dotnet run`, `Development`, SQLite + object store filesystem tạm, hàng đợi in-memory, host `bluedrive` trỏ tới mock (JWKS, webhook, `AllowedOrigins` = origin trang host; `Cors:Origins` thêm `http://tauri.localhost`) |
| Web editor | `apps/web/dist` qua `vite preview` với `BLUEOFFICE_API_PROXY` (proxy `/api`, `/hosting`, `/collab` WebSocket — cùng sơ đồ với `deploy/web/nginx.conf`) |
| Trang host | `tests/e2e/collab/host` (vite dev) nhúng editor bằng `@blueoffice/editor-sdk` `BlueOffice.open`; token đi qua `postMessage` |

Bộ khởi động: `tests/e2e/collab/stack.mjs` (cổng `COLLAB_PORT`, mặc định `E2E_PORT + 20`: editor, host, server, mock).
Kịch bản (hai browser context = hai người dùng của mock):

- `specs/collab-doc.spec.ts`: A (Nguyễn Văn An) gõ tiếng Việt → B (Trần Thị Bình) thấy; B in đậm một cụm từ → A thấy
  trạng thái đậm; caret kèm tên của người kia hiển thị ở cả hai phía; lệnh host `save` → version mới trong mock với
  `editors = [u1, u2]`, nội dung docx có đoạn đã gõ và run đậm; B `setOffline(true)` → banner "Mất kết nối", B sửa,
  A không thấy; B online lại → A nhận thay đổi offline, sửa tiếp của A tới B; mock thu hồi quyền sửa của B +
  `POST /api/hosts/bluedrive/notify` → B chuyển sang **chỉ xem** (banner, gõ phím không đổi tài liệu, lệnh `save` bị
  từ chối) nhưng vẫn thấy A sửa trực tiếp. Đường lưu offline cũ (`POST …/sync?baseVersion=` cũ) → bản
  "(xung đột - Trần Thị Bình - yyyy-MM-dd HH-mm)" cùng thư mục, bản gốc không bị ghi đè.
- `specs/collab-sheet-slide.spec.ts`: BlueSheet — công thức `=2*21` và chữ tiếng Việt qua lại giữa hai người, lưu từ host
  → version có `<f>2*21</f>` và cả hai người sửa; BlueSlide — A chọn và kéo placeholder tiêu đề, B thấy khung chọn của A
  di chuyển và chọn được hình ở vị trí mới (trùng vị trí bên A).
- `tests/e2e-desktop/desktop.test.mjs` ("desktop deep link …"): khởi động cùng stack, người dùng web u1 mở file trong
  trang host; `bluedoc.exe` được mở bằng tham số `bluedoc://open?host=bluedrive&file=…&token=…` với
  `BLUEOFFICE_SERVER` → tạo phiên server, tải file, vào cùng room; chữ gõ trên web hiện ở desktop và ngược lại;
  Ctrl+S trên desktop lưu checkpoint → version mới với `editors = [u1, u2]`.

Chạy:

```bash
pnpm build:wasm && pnpm --filter @blueoffice/web build          # web dist dùng cho preview
pnpm --filter @blueoffice/e2e test:collab                        # chỉ E2E cộng tác (tự khởi động stack)
pnpm test:e2e                                                    # E2E web thường + cộng tác
node tests/e2e/collab/stack.mjs                                  # stack để thử tay (COLLAB_STACK_VERBOSE=1 xem log)
# desktop: build bluedoc.exe rồi
MSEDGEDRIVER=<msedgedriver.exe khớp WebView2> pnpm test:e2e:desktop   # COLLAB_E2E=0 bỏ ca deep link
```

**Desktop**: `apps/desktop/src/main.tsx` nghe `ShellIntegration.onOpenLink` (tham số dòng lệnh, instance thứ hai, deep
link) → `openHosted` của `@blueoffice/hosted` với server lấy từ `BLUEOFFICE_SERVER` (lệnh Tauri `server_url`) hoặc thiết
lập đã lưu (`Platform.shell.serverUrl/setServerUrl`). Replica `bo-collab` chạy bằng WASM trong web view (CSP thêm
`'wasm-unsafe-eval'`); engine là engine native. CSP `connect-src` cho phép `https:`/`wss:` mọi máy chủ và
`http`/`ws` tới `localhost`/`127.0.0.1` (máy chủ HTTP nội bộ khác: thêm origin vào CSP trong `tauri.conf.json` khi
build). Server phải cho origin web view vào CORS: `Cors:Origins` gồm `http://tauri.localhost` (Windows/Linux) và
`tauri://localhost` (macOS).

**Chế độ chỉ xem**: `SessionMode` theo `welcome.mode`, `mode-changed`, `session-revoked`/4403; `guardPlatform` chặn mọi
lệnh sửa của BlueDoc/BlueSheet/BlueSlide (điều hướng, chọn, sao chép, tìm, xuất vẫn chạy), không trả lời
`save-request`, `saveFile` từ chối; `HostedBanner` báo chỉ xem / mất quyền / mất kết nối.

**Lỗi đã sửa khi chạy E2E thật**: người vào đầu tiên không bao giờ seed room (bỏ qua `welcome.seed`); `mode-changed`
không được xử lý (không chuyển chỉ xem); sửa offline không bao giờ tới server (server không có Y.Doc để trả lời sync step
1 bằng phần chênh — giờ client đăng lại snapshot sau khi kết nối lại, và đóng socket ngay khi trình duyệt báo `offline`);
nền tảng nhúng spread instance class nên mất các method `shell` (`recentFiles`…); whitelist origin `postMessage` chỉ
có lúc build (`VITE_EMBED_ORIGINS`) — giờ lấy từ `GET /api/hosting/origins` khi không đặt; Ctrl+S / nút Lưu của editor
đi qua `Platform.storage` nên tải file về thay vì lưu lên host — `hostedPlatform` giờ thay cả `storage`; tauri-driver
nhận deep link dạng `--bluedoc://…` (msedgedriver thêm `--`) nên app bỏ qua; ca E2E desktop cũ để mồ côi msedgedriver
giữ cổng 4445.

**Hạn chế còn lại (quan sát trong E2E)**
- **Lỗi chưa sửa (chập chờn, ~1/3 lần chạy ca deep link desktop)**: chữ người dùng desktop (engine native) gõ hiện cục
  bộ, khung y-sync tới web và **replica hai bên đều chứa đoạn chữ**, nhưng engine web không hiện và engine desktop làm mất
  nó ở thay đổi kế tiếp — tức vị trí chèn rơi vào chỗ ánh xạ coi là "lạc" (cạnh embed khối), nghi do shadow của engine
  native lệch offset với replica trong một tương tranh nào đó. Luôn xảy ra khi desktop gõ ngay tại caret ban đầu (chưa
  bấm vào tài liệu); chập chờn khi đã bấm. Không tái hiện được web↔web (trình duyệt), WASM↔WASM
  (`packages/collab/test/live.test.ts`) hay native↔native (`crates/bo-engine/tests/collab.rs`); seed native và WASM
  giống hệt từng byte. Hướng điều tra tiếp: ghi `collab.change` + `takeRemote` của desktop và so offset với replica.
- Người vào sau thấy tài liệu ở trạng thái "Chưa lưu" và view cuộn tới caret sau khi nội dung chung thay thế bản cục bộ.
- CSP desktop không cho `http://` tới máy chủ khác `localhost` (cần HTTPS hoặc build lại với origin trong CSP).

## Còn lại (ngoài phạm vi lần này)

- **Ký số** (Phase 6): ký PDF PAdES và OOXML XML-DSig; điểm tích hợp dự kiến: một `ISignatureProvider` (USB token qua agent
  cục bộ trên desktop, ký số từ xa của CA Việt Nam qua REST) + job `sign` trong worker; chưa code.
- **Desktop**: màn hình Start / tab BlueDrive (duyệt file, SSO), hỏi chuyển sang cộng tác khi mở file trong thư mục
  đồng bộ BlueDrive; ô nhập server trong giao diện (hiện chỉ biến môi trường + `setServerUrl`).
- `bo-collab` (Yrs) và editor SDK/TS (postMessage, nút Share/Close, chế độ offline) — luồng chính.
- Nhiều instance `api` (room đang ở bộ nhớ một tiến trình): cần định tuyến theo file (consistent hashing) hoặc
  Redis pub/sub cho relay.
- EF Core migrations (hiện `EnsureCreated`); Testcontainers cho PostgreSQL/MinIO/Redis (hiện test dùng SQLite/filesystem/in-memory).
- Chuyển đổi định dạng ngoài docx→pdf (doc→docx, xls→xlsx, ppt→pptx, xlsx/pptx→pdf) phụ thuộc `bo-cli` của Phase 3–5;
  `BoCli:AllowedExtensions` hiện là `.docx/.txt/.md`.
- Server chưa giữ Y.Doc (không merge được bản offline dạng CRDT ở server): khi xung đột luôn tạo bản sao.
