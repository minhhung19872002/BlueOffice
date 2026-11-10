# An toàn thông tin — BlueOffice on-premise

Tài liệu cho quản trị viên và người lập hồ sơ đề xuất cấp độ an toàn hệ thống thông tin. Mục tiêu: liệt kê **các
biện pháp kỹ thuật mà phần mềm thực sự cung cấp**, chỉ ra **vị trí trong mã nguồn/cấu hình** để kiểm chứng, và tách rõ
**phần đơn vị vận hành phải tự làm**.

> **Không phải chứng nhận.** BlueOffice chưa được đánh giá, chứng nhận hay thẩm định theo bất kỳ khung nào. Đơn vị vận
> hành thường ánh xạ các biện pháp dưới đây vào Nghị định 85/2016/NĐ-CP (phân loại theo cấp độ) và TCVN 11930:2017
> (yêu cầu cơ bản theo cấp độ); việc ánh xạ, đánh giá và phê duyệt hồ sơ là trách nhiệm của đơn vị. Tài liệu này mô tả
> trạng thái mã nguồn tại thời điểm viết; khi nâng cấp cần đối chiếu lại.

Liên quan: [README.md](README.md) (cài đặt) · [backup.md](backup.md) · [upgrade.md](upgrade.md) ·
[desktop-update.md](desktop-update.md) · [../bohp.md](../bohp.md) · [ADR-0007](../adr/0007-digital-signatures.md).

Ký hiệu: **[Đã có]** = cài trong code/cấu hình mặc định · **[Cấu hình]** = có sẵn nhưng phải bật/điền ·
**[Vận hành]** = trách nhiệm đơn vị · **[Chưa có]** = chưa cài đặt.

---

## 1. Phạm vi & giả định

- Phạm vi: stack Docker Compose `deploy/docker-compose.yml` (`api`, `worker`, `web`, `postgres`, `minio`, `redis`,
  `keycloak` tuỳ chọn), lõi Rust (`crates/`) chạy trong `bo-cli` của worker, trong WASM của trình duyệt và trong ứng
  dụng desktop Tauri (`apps/desktop/src-tauri`).
- Giả định:
  - Có **reverse proxy TLS** do đơn vị quản lý đứng trước `web`/`api` (compose chỉ bind `127.0.0.1`).
  - Có **nhà cung cấp OIDC** (Keycloak của đơn vị hoặc profile `sso`) quản lý tài khoản, mật khẩu, MFA, khoá tài khoản.
  - Máy chủ Docker, hệ điều hành, mạng, tường lửa, chống mã độc, giám sát tập trung do đơn vị quản lý.
  - Khi tích hợp BlueDrive/WOPI host, **host là nơi quyết định quyền** trên file của host; BlueOffice chỉ thực thi
    quyền host trả về.
- Ngoài phạm vi: BlueDrive (sản phẩm riêng), bảo mật của host WOPI bên thứ ba, máy trạm người dùng.

## 2. Xác thực & phân quyền

### 2.1 Xác thực người dùng (OIDC/JWT) — [Đã có] + [Cấu hình]
- BlueOffice **không lưu mật khẩu người dùng**. API nhận JWT Bearer do OIDC phát hành:
  `apps/server/BlueOffice.Api/Auth/AuthSetup.cs` → `AddBlueOfficeAuth` (JwtBearer, `Authority`, `ValidAudience`,
  `ClockSkew = 1 phút`, `RequireHttpsMetadata`).
- Cấu hình: `Auth__Authority` (`OIDC_AUTHORITY`), `Auth__Audience` (`OIDC_AUDIENCE`, mặc định `blueoffice-api`),
  `Auth__RequireHttpsMetadata` (`OIDC_REQUIRE_HTTPS`, mặc định `true`).
- Khoá ký thử nghiệm `Auth:DevSigningKey` bị **từ chối khi chạy Production** (ném `InvalidOperationException` trong
  `AddBlueOfficeAuth`); image đặt `ASPNETCORE_ENVIRONMENT=Production` (`deploy/Dockerfile`).
- Người dùng được tạo khi gọi lần đầu theo claim `sub` (`Auth/UserService.cs` → `CurrentAsync`).
- WebSocket: trình duyệt không đặt được header nên `?access_token=` **chỉ được nhận trên `/collab`**
  (`OnMessageReceived` trong `AuthSetup.cs`).
- [Vận hành]: chính sách mật khẩu, MFA, khoá tài khoản khi đăng nhập sai, thời hạn token, thu hồi phiên, nhật ký đăng
  nhập — cấu hình trên Keycloak/IdP. BlueOffice **không** tự ghi sự kiện đăng nhập.

### 2.2 Token của host (BlueDrive/BOHP) — [Đã có]
- `HostJwtHandler` (`AuthSetup.cs`) và `HostTokenValidator` (`apps/server/BlueOffice.Infrastructure/Hosts/HostRegistry.cs`):
  xác minh chữ ký bằng JWKS của host (cache 10 phút, tải lại khi gặp `kid` lạ, tối thiểu 30 s/lần), kiểm tra `iss`
  (khi cấu hình `Hosts__N__Issuer`), `aud`, **bắt buộc `exp`**, lệch giờ 1 phút; token có claim `fid` chỉ dùng được cho
  đúng file đó (`SessionService.CreateHostSessionAsync`).
- Token bị từ chối được ghi log mức Information **không kèm token** (`HostTokenValidator.ValidateAsync`).
- Desktop đăng nhập host bằng OAuth 2.0 code + **PKCE S256** và kiểm tra `state` (`packages/hosted/src/signin.ts`).

### 2.3 Phiên editor & WOPI — [Đã có]
- `POST /api/sessions` tạo phiên với id công khai + **token bí mật 256 bit** (`RandomNumberGenerator.GetBytes(32)`,
  `Sessions/SessionService.cs` → `StartAsync`); so sánh token **thời gian hằng** (`CryptographicOperations.FixedTimeEquals`,
  `Sessions/EditorSession.cs` → `SessionStore.Get`). Hết hạn trượt `Collab:SessionLifetime` (mặc định 12 giờ,
  `Collab/CollabRoom.cs` → `CollabOptions`); dọn phiên hết hạn bằng `PurgeExpired`.
- Lệnh ghi (`checkpoint`, `save-as`, `sync`, `rename`, `restore`) đi qua `RequireEdit` (`Sessions/SessionEndpoints.cs`).
- **Chống SSRF với WOPISrc**: chỉ nhận URL có tiền tố `BaseUrl` của một host WOPI đã cấu hình và id file không chứa `/`
  (`HostRegistry.FindWopi`); nếu không → 403.
- Quyền được hỏi lại host theo chu kỳ `Collab__PermissionRecheckInterval` (mặc định 60 s, `Collab/RoomManager.cs`);
  người chỉ có quyền xem gửi cập nhật Yjs bị **bỏ qua** (`Collab/CollabRoom.cs`, chú thích "Read-only participants may
  not change the document"); tin nhắn WebSocket lớn hơn `Collab:MaxMessageBytes` (16 MiB) bị từ chối (`RoomManager.cs`).
- [Chưa có]: **WOPI proof key** (`X-WOPI-Proof`) — BlueOffice không ký request gửi tới host WOPI; host không xác minh
  được request đến từ BlueOffice ngoài access token.

### 2.4 Phân quyền file do BlueOffice quản lý (host `local`) — [Đã có]
- Vai trò có thứ tự `Viewer < Commenter < Editor < Owner` (`apps/server/BlueOffice.Domain/Entities.cs` → `ShareRole`);
  ma trận thao tác → vai trò tối thiểu trong `apps/server/BlueOffice.Domain/Access.cs` → `AccessEvaluator.RequiredRole`
  (xoá, quản lý chia sẻ, đọc audit = Owner). Chia sẻ thư mục được kế thừa; chia sẻ hết hạn bị bỏ qua (`IsActive`).
- Mọi `/api/...` của file yêu cầu policy `User` (`Files/FileEndpoints.cs`). Endpoint chuyển đổi (`/convert`, `/jobs`,
  `/api/convert/pdf`) yêu cầu policy `Service` (người dùng OIDC hoặc host) — `Jobs/JobEndpoints.cs`, `Program.cs`.
- REST API `/api/v1` (mặc định tắt) yêu cầu policy `RestApi`: khoá API ở header `X-Api-Key` (cấu hình chỉ giữ SHA-256,
  có scope `convert`/`merge`/`edit`/`inspect`, thu hồi và hết hạn) hoặc token của policy `Service`; khoá API không mở
  endpoint nào khác — `Rest/ApiKeyAuth.cs`, [rest-api.md](rest-api.md).
- **Link chia sẻ**: token ngẫu nhiên 256 bit, base64url (`FileService.CreateLinkAsync`), có hạn tuỳ chọn (hết hạn → 410),
  không cho cấp vai trò Owner; truy cập link là **ẩn danh** (`/api/links/{token}`), mỗi lần tải ghi audit `link:<id>`.
- Tên file/thư mục được kiểm tra (`FileService.ValidateName`); MIME suy ra từ phần mở rộng, không tin client
  (`ContentTypeOf`); tên file client không dùng làm đường dẫn lưu (khoá đối tượng `files/<id>/v<n>`).
- Hạn chế: token link lưu **dạng rõ** trong cột `Shares.LinkToken` (chưa băm); chưa có giới hạn tần suất (rate limit)
  cho endpoint ẩn danh.

### 2.5 Trình duyệt — [Đã có]
- Header cho mọi phản hồi của `api` (`Program.cs`): `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`,
  `Content-Security-Policy: frame-ancestors 'self' <origin host đã cấu hình>`.
- CORS chỉ cho origin trong `Cors__Origins__N` và `Hosts__N__AllowedOrigins__N` (`Program.cs`, `AddCors`).
- Nhúng iframe: editor chỉ nhận `postMessage` từ origin được phép và khoá vào origin đầu tiên
  (`packages/editor-sdk/src/embedded.ts`); danh sách origin lấy từ `VITE_EMBED_ORIGINS` lúc build hoặc
  `GET /api/hosting/origins` (bỏ `*`).
- [Chưa có]: container `web` (`deploy/web/nginx.conf`) **không thêm header bảo mật** cho trang tĩnh (`index.html`,
  JS, WASM) — không có CSP, `frame-ancestors`, `X-Content-Type-Options`, HSTS cho trang editor. **Khuyến nghị** thêm ở
  reverse proxy (xem §7.2).

## 3. Nhật ký kiểm toán (audit)

- [Đã có] Bảng `AuditEntries` trong PostgreSQL (`BlueOffice.Domain/Entities.cs` → `AuditEntry`: thời gian UTC, actor,
  tên, hành động, host, loại/id tài nguyên, chi tiết, IP). Ghi bởi `apps/server/BlueOffice.Api/Audit/AuditService.cs`
  → `RecordAsync`; sự kiện của file host được chuyển về host qua webhook `audit`.
- Hành động (`AuditAction`): `Open`, `Edit`, `Export`, `Print`, `Download`, `Upload`, `Share`, `Restore`, `Rename`,
  `Delete`. Nguồn ghi:
  - phía server: mở phiên (`SessionService.StartAsync`, kèm `mode=`), lưu phiên bản / autosave cộng tác
    (`FileService`, `CollabRoom`), tải về, upload, chia sẻ/huỷ chia sẻ/tạo link, khôi phục, đổi tên, xoá;
  - **do client báo**: `Print`, `Export` (và `Download` phía client) qua `POST /api/files/{id}/audit` hoặc
    `POST /api/sessions/{id}/audit` — client sửa đổi có thể không báo. Server từ chối báo `Export`/`Download` khi phiên
    không có quyền tải.
- Đọc audit: `GET /api/files/{id}/audit`, chỉ Owner (`Operation.ReadAudit`).
- Webhook gửi host ký **HMAC-SHA256** trên `timestamp + "." + body` (`X-BlueOffice-Signature`, `X-BlueOffice-Timestamp`;
  `BlueOffice.Infrastructure/Hosts/HostWebhooks.cs`), thử lại `Webhooks__MaxAttempts` (mặc định 3).
- Hạn chế / [Chưa có]:
  - **IP trong audit là IP của proxy**: chưa bật `ForwardedHeaders` nên `RemoteIpAddress` là container `web`/reverse
    proxy, không phải máy người dùng. Đối chiếu bằng log của reverse proxy cho tới khi bổ sung.
  - Không có chống sửa đổi (hash chain/WORM), không có chính sách lưu giữ/xoay vòng, không có API tra cứu audit toàn hệ
    thống cho quản trị (chỉ theo từng file) — [Vận hành]: sao lưu bảng, phân quyền DB, đẩy về SIEM nếu cần.
  - Sự kiện đăng nhập/đăng xuất nằm ở IdP, không ở BlueOffice.

## 4. Mã hoá

### 4.1 Đường truyền
- [Vận hành] **TLS kết thúc tại reverse proxy** của đơn vị; compose không cấu hình chứng thư. `api` lắng nghe HTTP
  `:8080` (`ASPNETCORE_URLS=http://+:8080`), `web` HTTP `:8081`, cả hai bind `127.0.0.1` (`API_BIND`, `WEB_BIND`).
- [Chưa có] HSTS/redirect HTTPS trong ứng dụng (`UseHsts`/`UseHttpsRedirection` không dùng) — đặt ở reverse proxy.
- Trong mạng compose các kết nối **không mã hoá**: `api`/`worker` → PostgreSQL (chuỗi kết nối không có `SSL Mode`),
  Redis (không TLS), MinIO (`Storage__UseSsl` mặc định `false`, `Infrastructure/DependencyInjection.cs` → `WithSSL`).
  Chấp nhận được khi cả stack chạy trên một máy; nếu tách máy phải bật TLS: `Storage__UseSsl=true` + chứng thư MinIO,
  `SSL Mode=Require` trong `ConnectionStrings__Default`, `ssl=true` trong `ConnectionStrings__Redis` [Cấu hình].
- Keycloak (profile `sso`) chạy `--http-enabled=true --proxy-headers=xforwarded`, bind `${API_BIND}:8180` — phải đặt sau
  reverse proxy TLS.
- Gọi host (JWKS, BOHP, WOPI, webhook): dùng URL đơn vị cấu hình; `OIDC_REQUIRE_HTTPS=true` bắt buộc HTTPS cho metadata
  OIDC. Timeout HttpClient: JWKS/webhook 10 s, BOHP/WOPI 60 s (`DependencyInjection.cs`).

### 4.2 Lưu trữ (at-rest)
- [Chưa có ở tầng ứng dụng] Nội dung file trong MinIO và metadata trong PostgreSQL **không được BlueOffice mã hoá**.
  Mỗi phiên bản có SHA-256 (`FileVersion.Sha256`, `HashingStream`) để kiểm tra toàn vẹn, không phải mã hoá.
- [Vận hành] Lựa chọn: mã hoá ổ đĩa/volume (LUKS, BitLocker), MinIO SSE (SSE-S3 với KMS/KES — cấu hình phía MinIO,
  BlueOffice không gửi header SSE), mã hoá bản sao lưu (`backup.md`: `gpg --symmetric`/`restic`).
- **Tài liệu có mật khẩu** ([MS-OFFCRYPTO]) được giữ nguyên mã hoá khi lưu lại: `crates/bo-crypto` (Agile AES-256/SHA-512,
  100 000 vòng khi ghi; đọc Standard + Agile, kiểm HMAC toàn vẹn). Giới hạn `MAX_SPIN_COUNT` = 10 triệu
  (`crates/bo-crypto/src/info.rs`) chống DoS bằng số vòng băm khổng lồ; `MAX_STREAM_SIZE` 1 GiB (`src/lib.rs`).
  Test: `crates/bo-engine/tests/security.rs` → `password_protected_open_and_save`. RC4/CryptoAPI chưa hỗ trợ.

## 5. Sao lưu & khôi phục

- [Vận hành] Quy trình chi tiết: [backup.md](backup.md) — `pg_dump` PostgreSQL + sao chép volume MinIO cùng thời điểm,
  `SHA256SUMS`, khuyến nghị 7 ngày/4 tuần/12 tháng, bản ở vị trí vật lý khác, mã hoá bản sao lưu, diễn tập khôi phục
  hằng quý. `deploy/.env` (mật khẩu) lưu riêng, có mã hoá.
- [Đã có] Phiên bản file (`FileVersion`, khôi phục `POST /api/files/{id}/versions/{n}/restore`); dừng `api` ghi snapshot
  phiên cộng tác vào MinIO để khôi phục sau (README §6).
- Redis chỉ chứa hàng đợi job (giữ 1 ngày) — không cần sao lưu.

## 6. Ghi log & giám sát

- [Đã có] Serilog ra **stdout** (`Program.cs` → `UseSerilog ... WriteTo.Console()`), mức theo `Serilog:MinimumLevel`
  (`appsettings.json`: Information; `Microsoft.AspNetCore`, `Microsoft.EntityFrameworkCore`: Warning). Log request
  (`UseSerilogRequestLogging`) ghi phương thức, **đường dẫn không gồm query string**, mã trạng thái, thời gian.
- [Đã có] OpenTelemetry trace + metric (`Common/HealthAndTelemetry.cs`, `Otel__Endpoint`); health `/health`,
  `/health/ready` (ẩn danh, chỉ trạng thái).
- [Vận hành] Gom log container (journald/Loki/ELK/SIEM), giữ log theo yêu cầu cấp độ, đồng bộ thời gian NTP
  (image đặt `TZ=Asia/Ho_Chi_Minh`; DB lưu UTC).
- Lưu ý: token host có thể nằm trong **query string** (`/edit?...&token=`, `/collab?...&token=`/`access_token=`).
  Serilog không ghi query, nhưng **access log nginx** (container `web` và reverse proxy) mặc định ghi `$request` đầy đủ
  → **khuyến nghị** dùng `log_format` bỏ query hoặc che tham số `token`/`access_token`.

## 7. Hardening Docker Compose

### 7.1 Những gì `deploy/docker-compose.yml` + Dockerfile đang làm
- Chỉ publish cổng trên `127.0.0.1`: `api` `${API_BIND:-127.0.0.1}:8080`, `web` `${WEB_BIND:-127.0.0.1}:8081`, console
  MinIO `127.0.0.1:9001`, Keycloak `${API_BIND}:8180`. PostgreSQL, Redis, MinIO S3 API, `worker` không publish cổng.
- Bí mật bắt buộc: compose dừng nếu thiếu `POSTGRES_PASSWORD`, `REDIS_PASSWORD`, `MINIO_ROOT_USER`,
  `MINIO_ROOT_PASSWORD`, `OIDC_AUTHORITY` (cú pháp `${VAR:?}`); `.env.example` dùng `CHANGE_ME`, README yêu cầu
  `chmod 600 deploy/.env`. Redis chạy `--requirepass`.
- Image `blueoffice/server` chạy **user không phải root** (`useradd --system blueoffice`, `USER blueoffice` trong
  `deploy/Dockerfile`), có `HEALTHCHECK`; build nhiều tầng, runtime `mcr.microsoft.com/dotnet/aspnet:8.0`.
- Image phụ thuộc gắn phiên bản: `postgres:16-alpine`, `redis:7-alpine`, `minio/minio:RELEASE.2025-04-22T22-12-26Z`,
  `quay.io/keycloak/keycloak:26.0`, `nginx:1.27-alpine`.
- `bo-cli` được gọi với `ArgumentList` (không qua shell), `UseShellExecute=false`, thư mục tạm riêng mỗi job
  (`Directory.CreateTempSubdirectory`) và xoá sau đó, timeout `BoCli__Timeout` (60 s) rồi
  `Kill(entireProcessTree: true)`; chỉ nhận phần mở rộng trong `BoCli__AllowedExtensions`
  (`Infrastructure/BoCliConverter.cs`, `Infrastructure/Jobs/ConversionJobProcessor.cs`).
- Giới hạn upload: `Storage__MaxUploadBytes` (`MAX_UPLOAD_BYTES`, mặc định 200 MiB) áp vào Kestrel
  `MaxRequestBodySize`, multipart và `HashingStream` (`Program.cs`, `Files/FileService.cs`); nginx `client_max_body_size 210m`.

### 7.2 Khuyến nghị — **chưa làm** trong compose
| Hạng mục | Hiện trạng | Khuyến nghị |
|---|---|---|
| Mạng | Mọi service chung mạng mặc định của project | Tách mạng `frontend` (web, api) và `backend` (`internal: true`: postgres, redis, minio) |
| Filesystem | Ghi được | `read_only: true` + `tmpfs: /tmp` cho `api`/`worker`/`web` |
| Quyền kernel | Mặc định Docker | `cap_drop: [ALL]`, `security_opt: [no-new-privileges:true]` |
| Tài nguyên | Không giới hạn | `mem_limit`/`cpus`/`pids_limit`, đặc biệt `worker` (bo-cli xử lý file không tin cậy) |
| Bí mật | Biến môi trường (thấy trong `docker inspect`) | Docker secrets / vault; xoay vòng định kỳ |
| `web` | nginx master chạy root, không header bảo mật | `nginxinc/nginx-unprivileged`, thêm CSP/HSTS/`X-Content-Type-Options`/`frame-ancestors` |
| MinIO | Dùng tài khoản root cho ứng dụng | Tạo user + policy chỉ trên bucket `blueoffice` |
| PostgreSQL | User `blueoffice` sở hữu cả DB `keycloak` | User riêng cho Keycloak; chỉ quyền cần thiết |
| Image | Gắn tag, chưa gắn digest | Gắn `@sha256:`, quét lỗ hổng (Trivy/Grype) trước khi triển khai |
| Keycloak | Mật khẩu admin mặc định rỗng nếu không điền | Bắt buộc `KEYCLOAK_ADMIN_PASSWORD`, tắt console admin ra ngoài |
| Rate limit | Chỉ có cho REST API `/api/v1` (theo khoá / người dùng, sau xác thực) | Giới hạn ở reverse proxy cho `/api/links`, `/api/sessions`, `/edit`, `/convert`, `/api/v1/` |

## 8. Xử lý tệp không tin cậy

Mọi định dạng đọc từ người dùng đều qua lõi Rust: `#![forbid(unsafe_code)]` ở các crate, không `unwrap()` trên dữ liệu
từ file (CLAUDE.md, clippy). Cùng một lõi chạy trong worker (`bo-cli`), WASM và desktop.

- **Zip-bomb (OOXML)** — `crates/bo-opc/src/package.rs` → `Limits` (mặc định): ≤ 20 000 entry, ≤ 512 MiB/part,
  ≤ 2 GiB tổng giải nén, tỉ lệ nén ≤ 200 cho part > 1 MiB; **không tin kích thước khai báo** (đọc qua `take(max+1)`);
  từ chối tên part chứa `\` hoặc `..`. Test `rejects_zip_bomb`. `bo-docx`, `bo-xlsx`, `bo-pptx` đọc với
  `Limits::default()`.
- **ODF** — `crates/bo-odf/src/package.rs`: ≤ 20 000 entry, ≤ 256 MiB/entry, ≤ 1 GiB tổng, tỉ lệ ≤ 200;
  `crates/bo-odf/src/dom.rs`: `MAX_DEPTH` 256, `MAX_NODES` 20 triệu. Reader ODF chưa từ chối tên entry chứa `..`
  như `bo-opc` (gói chỉ được đọc trong bộ nhớ, không giải nén ra đĩa).
- **Độ sâu XML** — `crates/bo-opc/src/xml.rs`: `MAX_DEPTH = 256`, lỗi "maximum XML nesting depth exceeded" (test
  `depth_limit`).
- **Entity/DTD (XXE, "billion laughs")** — XML đọc bằng `quick-xml`, không xử lý DTD; `bo_opc::xml::unescape` chỉ giải
  5 entity định sẵn và tham chiếu số, entity khác giữ nguyên dạng chữ → không mở rộng entity, không tải tài nguyên ngoài.
- **Định dạng nhị phân cũ** — `crates/bo-binary`: `MAX_STREAM` 512 MiB/stream CFB; `.xls`/`.xlsb` `MAX_CELLS` 20 triệu,
  `MAX_SST` 10 triệu, `MAX_SUBSTREAM_DEPTH` 8; `.doc` `MAX_CHARS` 64 Mi, `MAX_STYLE_DEPTH` 16, `MAX_LISTS` 4096;
  `.ppt` `MAX_DEPTH` 48, `MAX_SLIDES` 10 000; công thức BIFF `MAX_STACK` 4096; VBA `MAX_DECOMPRESSED` 64 MiB.
- **Công thức** — `crates/bo-formula/src/parser.rs`: `MAX_FORMULA_CHARS` 8192, `MAX_NESTING` 100, `MAX_TREE_DEPTH` 1024;
  `eval.rs` `MAX_DEPTH` 200. Pivot/định dạng: `MAX_SOURCE_CELLS`, `MAX_REPORT_CELLS`, `MAX_FORMAT_CELLS` (`crates/bo-sheet`).
- **Ảnh/raster** — `crates/bo-raster/src/lib.rs`: `MAX_DIMENSION` 16 384 px.
- **Fuzzing** — 14 target `cargo-fuzz` trong `fuzz/fuzz_targets/` (opc, docx, xlsx, pptx, doc, xls, xlsb, xls_formula,
  ppt, vba, crypto, odf, encoding, formula), CI chạy 60 s/target (`.github/workflows/ci.yml`, job `fuzz`). Test khói
  không cần nightly: `crates/bo-engine/tests/fuzz_smoke.rs` (đột biến XML + ZIP trên corpus, không panic, ngân sách
  20 s/lần gọi); proptest `*_never_panic*` cạnh parser trong các crate.
- Server: định dạng đầu vào chuyển đổi giới hạn bởi `BoCli__AllowedExtensions`; mỗi lần `bo-cli` có timeout và bị kill
  cả cây tiến trình (§7.1).
- [Chưa có] giới hạn bộ nhớ cho tiến trình `bo-cli` ở tầng ứng dụng — dùng giới hạn container (§7.2).

## 9. Macro không thực thi

- [Đã có] BlueOffice **không có trình thông dịch VBA/XLM**. `crates/bo-binary/src/vba.rs`: chỉ phát hiện
  (`has_macros`: part `vbaProject.bin`, storage `Macros`/`_VBA_PROJECT_CUR`, macro sheet XLM, `VBAInfoContainer` của PPT),
  đọc danh sách module (`read_vba_project`) và sao chép nguyên trạng khi chuyển đổi (`carry_over`).
- Mở rồi lưu `.docm` giữ nguyên byte `vbaProject.bin`, `DocInfo.macros = true` → BlueDoc hiện thanh cảnh báo
  "Tài liệu chứa macro — macro sẽ được giữ nguyên nhưng không chạy" (`packages/i18n/src/locales/vi-VN.json` →
  `macroWarning`). Test: `crates/bo-engine/tests/security.rs` → `macros_are_reported_and_kept`.
- `.xlsm`/`.pptm`: phần VBA được giữ như part không xử lý (opaque) khi lưu; **chưa có thanh cảnh báo** trong
  BlueSheet/BlueSlide và chưa có test riêng như `.docm`.

## 10. Chặn liên kết & tài nguyên ngoài

- [Đã có] Các crate lõi **không có HTTP client** (không phụ thuộc `reqwest`/`ureq`/`hyper` trong `crates/*/Cargo.toml`)
  và không mở socket → mở/hiển thị/chuyển đổi tài liệu không bao giờ tải tài nguyên từ mạng.
- Quan hệ `TargetMode="External"` ([ECMA-376 Part 2] OPC) được đọc như dữ liệu và ghi lại nguyên trạng:
  - BlueDoc: `crates/bo-docx/src/reader.rs` → `target()` trả `None` cho quan hệ ngoài nên **ảnh liên kết ngoài không được
    tải**; URL hyperlink chỉ lưu làm thuộc tính run.
  - BlueSlide: `crates/bo-pptx/src/render.rs` → `image_part` bỏ quan hệ `external`.
  - BlueSheet: `crates/bo-xlsx/src/read.rs` → hyperlink ngoài giữ dạng dữ liệu, `opaque_rel` giữ quan hệ ngoài khi ghi.
  - Tham chiếu sổ tính ngoài (`[Book1]Sheet!A1`) không mở tệp ngoài.
- Mở URL ra ngoài chỉ dùng cho đăng nhập host trên desktop: `open_url` chỉ chấp nhận `http(s)://`, không khoảng trắng/ký
  tự điều khiển, ≤ 8192 ký tự (`apps/desktop/src-tauri/src/shell.rs` → `external_url`), gọi trình duyệt không qua shell.
- Khác với mô tả "chặn khi chưa được người dùng cho phép" của prompt §5.9: hiện **không có cơ chế cho phép** — tài nguyên
  ngoài luôn bị bỏ qua. Thao tác mở hyperlink duy nhất là trong trình chiếu BlueSlide (§10.1).

### 10.1 Liên kết & hành động trong trình chiếu BlueSlide (SLIDE-ACTION-001) — [Đã có]

Hình và đoạn chữ của slide có thể mang hành động khi bấm / khi rê chuột (ECMA-376 Part 1 §21.1.2.3.5 `a:hlinkClick`,
§21.1.2.3.6 `a:hlinkHover`, §21.1.2.3.7 `a:hlinkMouseOver`; thuộc tính `action` dạng `ppaction://…`). Tệp do người khác
gửi tới là dữ liệu không tin cậy, nên trình chiếu theo đúng một chính sách, cài ở **hai lớp độc lập**:
`bo_engine::slide_edit::action::safety` (engine gắn nhãn `internal` / `external` / `blocked` cho từng vùng bấm) và
`packages/slide-editor/src/actions.ts` → `actionPolicy` (giao diện tự phân loại lại từ chính hành động; nhãn của engine
chỉ có thể làm chặt hơn, không bao giờ nới lỏng).

| Hành động | Trong trình chiếu |
|---|---|
| Nhảy trong bài: trang sau/trước/đầu/cuối, trang vừa xem, một trang cụ thể (`hlinksldjump`), trình chiếu tuỳ chỉnh (`customshow`), kết thúc | Chạy ngay, không hỏi |
| Âm thanh nhúng (`a:snd`, chỉ WAV nằm trong gói), dừng âm thanh (`endSnd`), làm nổi hình (`highlightClick`) | Chạy ngay |
| Địa chỉ `http://`, `https://`, `mailto:` | **Hỏi lại** (hiện nguyên địa chỉ), chỉ mở khi người dùng bấm "Mở"; không bao giờ mở chỉ vì rê chuột qua |
| `ppaction://program` (chạy chương trình) | **Không bao giờ chạy** — hiện thông báo đã chặn |
| `ppaction://macro` | **Không bao giờ chạy** (BlueOffice không có trình thông dịch macro, §9) |
| `ppaction://hlinkfile`, `ppaction://hlinkpres` (tệp / bài trình chiếu khác), mọi đích `file:` hay đường dẫn cục bộ/UNC | **Không bao giờ mở** |
| Scheme khác (`javascript:`, `data:`, `vbscript:`, `ms-*:`, `ftp:`, deep link `bluedoc://`…) | **Không bao giờ mở** |
| `action` không nhận ra | **Không bao giờ chạy**; giữ nguyên từng byte khi lưu |

- Địa chỉ được coi là hợp lệ chỉ khi viết "thẳng": không khoảng trắng, không ký tự điều khiển ở bất kỳ đâu (chặn kiểu
  `java\tscript:`), `http(s)` phải có `//host`. Kiểm ba lần: engine (`allowed_url`), giao diện trước khi hỏi và ngay
  trước khi mở (`isAllowedUrl`), và nền tảng: web `WebShell.openExternal` (`isExternalUrl`, mở tab mới với
  `rel="noopener noreferrer"` nên trang đích không với tới được cửa sổ BlueSlide); desktop `open_url` → `external_url`
  (chỉ `http(s)`, gọi trình duyệt hệ thống không qua shell — `mailto:` trên desktop vì thế bị từ chối và báo "không mở
  được").
- Hành động bị chặn **không bị xoá**: mở rồi lưu giữ nguyên phần tử và quan hệ (test
  `saving_keeps_every_action_as_it_was`). Trình soạn thảo từ chối *tạo* hành động chạy chương trình / macro / không rõ
  (`unsafeAction`); hộp thoại Chèn › Hành động hiện "được giữ nguyên nhưng không bao giờ chạy" và chỉ cho thay bằng hành
  động khác.
- Âm thanh của hành động chỉ nhận tệp RIFF/WAVE ≤ 8 MB do người dùng chọn hoặc âm dựng sẵn do BlueSlide tự tổng hợp
  (`builtinWav`); không tải âm thanh từ mạng.
- Xuất PDF chỉ giữ chú thích liên kết (ISO 32000-1 §12.5.6.5) cho địa chỉ `http(s)`/`mailto` và cho các bước nhảy tới
  trang; hành động bị chặn không xuất hiện trong PDF.
- Test: `crates/bo-engine/src/slide_edit/action.rs` (`only_web_and_mail_urls_may_be_opened`,
  `programs_macros_files_and_unknown_actions_are_blocked`), `crates/bo-engine/tests/slide_action.rs`
  (`unsafe_and_unknown_actions_are_blocked_and_cannot_be_created`), `packages/slide-editor/test/actions.test.ts`,
  `actionDialogs.test.tsx`, `packages/platform/test/web-pal.test.ts`, E2E `tests/e2e/specs/slide-actions.spec.ts`;
  corpus `tests/corpus/slide/slide-action-unsafe.pptx`.
- Hạn chế: mở địa chỉ trong chế độ toàn màn hình khiến trình duyệt rời toàn màn hình (phiên chiếu kết thúc); liên kết
  trong ô bảng chưa bấm được khi chiếu.

## 11. Chữ ký số

- [Đã có — qua `bo-cli`] `crates/bo-sign` (ADR-0007): PAdES baseline B-B cho PDF (`pdf.rs`, cập nhật tăng dần
  ISO 32000-1 §12.8), chữ ký gói OOXML theo ECMA-376 Part 2 §12 / XML-DSig (`ooxml.rs`), CMS (`cms_sig.rs`);
  RSA PKCS#1 v1.5 + SHA-256 (crate `rsa`, `sha2`, `cms`, `x509-cert`). Khoá không cần nằm trong tiến trình: trait
  `Signer` nhận digest SHA-256 — điểm tích hợp USB token (PKCS#11 qua tiến trình phụ) và ký từ xa của CA Việt Nam.
  Lệnh: `bo-cli sign <file> --key k.pem --cert c.pem`, `bo-cli verify <file>`.
- Xác minh (`verify_pdf`, `verify_ooxml`) kiểm digest + chữ ký RSA; **không kiểm chuỗi tin cậy tới Root CA quốc gia,
  không kiểm thu hồi (CRL/OCSP)**, chưa có dấu thời gian RFC 3161 (PAdES B-T) — ghi trong ADR-0007 và PROGRESS.
- [Chưa có] ký số trong giao diện web/desktop và trong API server.

## 12. Ứng dụng desktop (Tauri)

- CSP (`apps/desktop/src-tauri/tauri.conf.json` → `app.security.csp`): `default-src 'self'`, `script-src 'self'
  'wasm-unsafe-eval'` (không `unsafe-inline` cho script), `img-src 'self' data: blob:`, `connect-src` cho IPC,
  `https:`/`wss:` (server BlueOffice) và localhost; `style-src` có `'unsafe-inline'`.
- Capability tối thiểu (`capabilities/default.json`): `core:default`, đặt tiêu đề/focus cửa sổ, `dialog:default`,
  `deep-link:default` — không bật plugin `fs`/`shell`/`http` của Tauri.
- Lệnh native tự viết trong `src/main.rs` (`#![forbid(unsafe_code)]`): khoá cache offline thành tên file hex, không bao giờ
  là đường dẫn (`shell::cache_file_name`), ghi cache kiểu ghi-tạm-rồi-đổi-tên; deep link `blue*://open?host=&file=&token=`
  được phân tích chặt (`shell::parse_deep_link`, `is_safe_id`); token deep link không được serialize khi rỗng và chú thích
  "never logged"; một tiến trình (`tauri-plugin-single-instance`).
- Cập nhật tự động (feature `updater`): gói phải ký bằng khoá Tauri, client kiểm bằng `pubkey` ([desktop-update.md](desktop-update.md)).
- Hạn chế: lệnh `read_file(path)` đọc **bất kỳ đường dẫn** mà webview yêu cầu (dùng cho mở file/deep link) — an toàn
  dựa vào CSP chỉ cho script đóng gói; token host sau đăng nhập không lưu vào kho khoá hệ điều hành; gói cài đặt chưa
  ký Authenticode trong repo [Vận hành: ký khi phát hành].

## 13. Chuỗi cung ứng phần mềm

- [Đã có] `cargo deny check licenses bans sources` (`deny.toml`: chỉ registry crates.io, cấm git/registry lạ, danh sách
  giấy phép cho phép) và `pnpm license-check`; `THIRD_PARTY_NOTICES.md`.
- [Chưa có] Job CI tên "advisories" nhưng lệnh **không chạy `advisories`** (`.github/workflows/ci.yml`, job
  `cargo-deny`); chưa có `dotnet list package --vulnerable`, `pnpm audit`, SBOM, quét image. **Khuyến nghị** bổ sung.

## 14. Bảng ánh xạ kiểm soát → vị trí

| Kiểm soát | Trạng thái | Vị trí kiểm chứng |
|---|---|---|
| Xác thực OIDC/JWT, không lưu mật khẩu | Đã có | `apps/server/BlueOffice.Api/Auth/AuthSetup.cs` (`AddBlueOfficeAuth`); `Auth__*` |
| Cấm khoá dev ở Production | Đã có | `AuthSetup.cs`; `deploy/Dockerfile` (`ASPNETCORE_ENVIRONMENT=Production`) |
| Xác minh token host qua JWKS, bắt buộc `exp`, `aud` | Đã có | `Infrastructure/Hosts/HostRegistry.cs` (`HostTokenValidator`) |
| Phân quyền theo vai trò, kế thừa thư mục, hết hạn chia sẻ | Đã có | `BlueOffice.Domain/Access.cs` (`AccessEvaluator`), `Entities.cs` (`ShareRole`) |
| Link chia sẻ ngẫu nhiên 256 bit, có hạn | Đã có (token lưu rõ) | `Api/Files/FileService.cs` (`CreateLinkAsync`, `ResolveLinkAsync`) |
| Phiên editor: token bí mật, so sánh thời gian hằng, hết hạn | Đã có | `Api/Sessions/EditorSession.cs`, `SessionService.cs`; `Collab:SessionLifetime` |
| Chống SSRF WOPISrc | Đã có | `HostRegistry.FindWopi` |
| Hỏi lại quyền định kỳ, chặn cập nhật của người chỉ xem | Đã có | `Api/Collab/RoomManager.cs`, `CollabRoom.cs`; `Collab__PermissionRecheckInterval` |
| WOPI proof key | Chưa có | — |
| Header bảo mật, CORS theo danh sách | Đã có (chỉ `api`) | `Api/Program.cs`; `Cors__Origins__N` |
| Header bảo mật cho trang tĩnh `web` | Chưa có | `deploy/web/nginx.conf` |
| Kiểm tra origin `postMessage` | Đã có | `packages/editor-sdk/src/embedded.ts` |
| Audit thao tác file | Đã có (IP là proxy; Print/Export do client báo) | `Api/Audit/AuditService.cs`, bảng `AuditEntries` |
| Webhook ký HMAC-SHA256 | Đã có | `Infrastructure/Hosts/HostWebhooks.cs` |
| TLS | Vận hành (reverse proxy) | README §3 "Reverse proxy" |
| TLS nội bộ MinIO/PostgreSQL/Redis | Cấu hình (mặc định tắt) | `Storage__UseSsl`, `ConnectionStrings__*` |
| Mã hoá at-rest | Vận hành | MinIO SSE / mã hoá đĩa; `backup.md` |
| Toàn vẹn phiên bản (SHA-256) | Đã có | `FileService.cs` (`HashingStream`), `FileVersion.Sha256` |
| Tài liệu có mật khẩu (MS-OFFCRYPTO) | Đã có | `crates/bo-crypto`; test `crates/bo-engine/tests/security.rs` |
| Giới hạn upload | Đã có | `Storage__MaxUploadBytes`; `Program.cs`; `HashingStream` |
| Chạy `bo-cli` an toàn (không shell, timeout, kill cây, thư mục tạm) | Đã có | `Infrastructure/BoCliConverter.cs`, `Jobs/ConversionJobProcessor.cs`; `BoCli__*` |
| Container không chạy root (server) | Đã có | `deploy/Dockerfile` (`USER blueoffice`) |
| Chỉ bind cổng localhost, dịch vụ dữ liệu không publish | Đã có | `deploy/docker-compose.yml` |
| Bắt buộc mật khẩu dịch vụ | Đã có | `${VAR:?}` trong compose; `deploy/.env.example` |
| Tách mạng, read-only FS, cap_drop, giới hạn tài nguyên, secrets | Chưa có | §7.2 |
| Chống zip-bomb | Đã có | `crates/bo-opc/src/package.rs` (`Limits`), `crates/bo-odf/src/package.rs` |
| Giới hạn độ sâu XML, không mở rộng entity | Đã có | `crates/bo-opc/src/xml.rs` (`MAX_DEPTH`, `unescape`), `crates/bo-odf/src/dom.rs` |
| Giới hạn parser nhị phân/công thức | Đã có | `crates/bo-binary/src/*.rs`, `crates/bo-formula/src/parser.rs` |
| Fuzzing | Đã có | `fuzz/fuzz_targets/`, CI job `fuzz`, `crates/bo-engine/tests/fuzz_smoke.rs` |
| Macro không thực thi, giữ nguyên, cảnh báo (BlueDoc) | Đã có | `crates/bo-binary/src/vba.rs`; test `macros_are_reported_and_kept` |
| Không tải tài nguyên ngoài | Đã có | không có HTTP client trong `crates/`; `bo-docx/src/reader.rs`, `bo-pptx/src/render.rs` |
| Hành động trong trình chiếu: chương trình/macro/tệp không bao giờ chạy, URL ngoài phải hỏi | Đã có | §10.1; `crates/bo-engine/src/slide_edit/action.rs`, `packages/slide-editor/src/actions.ts` |
| Chữ ký số PAdES/OOXML | Đã có (CLI; chưa B-T, chưa kiểm chuỗi CA) | `crates/bo-sign`; ADR-0007 |
| Desktop CSP + capability tối thiểu | Đã có | `apps/desktop/src-tauri/tauri.conf.json`, `capabilities/default.json` |
| Ghi log tập trung (stdout) + OTel | Đã có / Vận hành gom log | `Program.cs` (Serilog), `Common/HealthAndTelemetry.cs` |
| Kiểm soát giấy phép/nguồn dependency | Đã có | `deny.toml`, `pnpm license-check` |
| Quét lỗ hổng dependency/image | Chưa có | — |

## 15. Hạn chế & khoảng trống đã biết

1. IP trong audit là IP proxy (chưa `UseForwardedHeaders` với danh sách proxy tin cậy).
2. Không có rate limit/chống dò cho endpoint ẩn danh (`/api/links/{token}`, `/api/sessions`, `/edit`).
3. Token link chia sẻ lưu dạng rõ trong DB; phiên editor chỉ nằm trong bộ nhớ (mất khi khởi động lại `api`).
4. Print/Export được audit theo báo cáo của client.
5. Audit không chống sửa đổi, không có chính sách lưu giữ, không có màn hình/API tra cứu cho quản trị toàn hệ thống.
6. Không mã hoá at-rest ở tầng ứng dụng; kết nối nội bộ compose không TLS (mặc định).
7. Container `web` không có header bảo mật/CSP; nginx master chạy root.
8. Compose chưa tách mạng, chưa `read_only`, `cap_drop`, `no-new-privileges`, giới hạn tài nguyên, Docker secrets;
   MinIO dùng tài khoản root cho ứng dụng.
9. Chưa có WOPI proof key.
10. Chữ ký số: chưa RFC 3161 (B-T), chưa kiểm chuỗi CA/thu hồi; chưa có trong UI/API.
11. Cảnh báo macro chỉ có ở BlueDoc; RC4/CryptoAPI chưa hỗ trợ.
12. Desktop: `read_file` không giới hạn đường dẫn; token không lưu trong kho khoá hệ điều hành.
13. CI chưa quét advisories (`cargo deny` không chạy `advisories`), chưa SBOM/quét image.
14. Một instance `api` (phòng cộng tác trong bộ nhớ) — ảnh hưởng tính sẵn sàng; chưa HA.
15. Token host có thể xuất hiện trong query string → cần che trong access log của nginx/reverse proxy.
