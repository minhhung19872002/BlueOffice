# Triển khai BlueOffice on-premise (Docker Compose)

Tài liệu cho quản trị viên hệ thống. Sao lưu: [backup.md](backup.md) · Nâng cấp: [upgrade.md](upgrade.md) ·
An toàn thông tin (hồ sơ cấp độ): [security.md](security.md) ·
Giao thức tích hợp BlueDrive: [../bohp.md](../bohp.md) · Ký số (ký từ xa, dấu thời gian, USB token, Root CA):
[remote-signing.md](remote-signing.md) · REST API cho hệ thống khác (mặc định tắt): [rest-api.md](rest-api.md) ·
BlueScript chạy theo lịch trên máy chủ (mặc định tắt): [scripts.md](scripts.md) ·
Dịch văn bản (mặc định tắt): [translation.md](translation.md) ·
Đọc chính tả (mặc định tắt): [dictation.md](dictation.md) ·
Nhận dạng chữ viết tay — nét vẽ thành chữ / phương trình (mặc định tắt): [handwriting.md](handwriting.md) ·
BlueAI — trợ lý dùng mô hình ngôn ngữ do quản trị viên cấu hình (mặc định tắt): [ai.md](ai.md).
Add-in web của tổ chức (danh mục do quản trị viên phát hành, mặc định tắt): [addins.md](addins.md).

## 1. Thành phần

| Service | Vai trò | Dữ liệu |
|---|---|---|
| `api` | ASP.NET Core 8: REST API, WebSocket cộng tác (`/collab`), phiên editor (`/edit`), BOHP/WOPI client, webhook | không trạng thái (trừ phiên cộng tác đang mở, xem §6) |
| `worker` | Hàng đợi chuyển đổi: gọi `bo-cli` (convert/PDF, thumbnail, preview, trích văn bản) | file tạm trong container |
| `postgres` | Metadata file/thư mục, phiên bản, chia sẻ, audit log, phiên cộng tác | volume `postgres-data` |
| `minio` | Nội dung file, snapshot/nhật ký cập nhật Yjs, input/output của job | volume `minio-data` |
| `redis` | Hàng đợi job + trạng thái job (giữ 1 ngày) | volume `redis-data` (AOF) |
| `keycloak` (tuỳ chọn, profile `sso`) | Nhà cung cấp OIDC nếu đơn vị chưa có SSO | database `keycloak` trong postgres |

Cùng một image `blueoffice/server` chạy cả `api` và `worker`, phân biệt bằng biến `Role` (`api` / `worker`;
mặc định `all` = cả hai trong một tiến trình, chỉ dùng cho môi trường dev).

## 2. Yêu cầu

- Linux x86-64 (Ubuntu 22.04/24.04, RHEL 9…) hoặc Windows Server có Docker; Docker Engine ≥ 24, Docker Compose v2.
- Tối thiểu 4 vCPU, 8 GB RAM, ổ đĩa SSD cho volume (dung lượng ≈ 2–3 × tổng dung lượng file × số phiên bản giữ lại).
- Reverse proxy TLS phía trước (nginx, HAProxy, F5…) — compose chỉ mở cổng `api` trên `127.0.0.1`.
- Một realm OIDC (Keycloak hoặc tương thích) với client `blueoffice-api`; token người dùng phải có claim `sub`, `name`, `aud=blueoffice-api`.

## 3. Cài đặt

```bash
git clone <repo> blueoffice && cd blueoffice
cp deploy/.env.example deploy/.env
# Sửa deploy/.env: thay MỌI giá trị CHANGE_ME bằng mật khẩu ngẫu nhiên (vd: openssl rand -base64 32)
chmod 600 deploy/.env

# Build image (hoặc docker load image do Bluestar cung cấp, rồi đặt BLUEOFFICE_IMAGE)
docker compose -f deploy/docker-compose.yml --env-file deploy/.env build

docker compose -f deploy/docker-compose.yml --env-file deploy/.env up -d
# Kèm Keycloak:
docker compose -f deploy/docker-compose.yml --env-file deploy/.env --profile sso up -d
```

Kiểm tra:

```bash
curl -fsS http://127.0.0.1:8080/health          # tiến trình sống
curl -fsS http://127.0.0.1:8080/health/ready    # đã kết nối được PostgreSQL, MinIO, Redis
curl -fsS http://127.0.0.1:8080/api/version
```

Lần chạy đầu `api` tự tạo schema (`Database__AutoCreate=true`); bucket MinIO `blueoffice` được tạo tự động.

### Web shell (`web`)

Container `web` (nginx) phục vụ giao diện BlueDoc/BlueSheet/BlueSlide (`apps/web/dist`) và chuyển `/api`, `/collab`,
`/hosting`, `/health`, `/convert`, `/thumbnail`, `/preview`, `/extract-text`, `/jobs` sang `api`
(`deploy/web/nginx.conf`). Với `/edit`: trình duyệt (iframe của BlueDrive) nhận trang editor, editor nhận token qua
`postMessage` (`@blueoffice/editor-sdk`) rồi gọi `POST /api/sessions`; client yêu cầu JSON và form WOPI `POST /edit`
đi thẳng tới `api`. Build trước khi dựng image:

```bash
pnpm install && pnpm build:wasm
VITE_EMBED_ORIGINS=https://drive.<domain> pnpm --filter @blueoffice/web build   # origin được phép nhúng
docker compose -f deploy/docker-compose.yml build web
```

### Reverse proxy

- Chuyển tiếp `https://office.<domain>/` → `http://127.0.0.1:8081/` (container `web`; `api` ở 8080 chỉ cần cho mạng nội bộ).
- **Bật WebSocket** cho đường dẫn `/collab/` và `/api/present` (header `Upgrade`/`Connection`), timeout đọc
  ≥ 120 s (server gửi keep-alive 30 s). `/api/present` là điều khiển trình chiếu từ điện thoại và trình chiếu trực
  tuyến (xem mục "Trình chiếu từ xa" bên dưới).
- `client_max_body_size` ≥ `MAX_UPLOAD_BYTES` (mặc định 200 MB).
- Ví dụ nginx:

```nginx
location /collab/ {
    proxy_pass http://127.0.0.1:8080;
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
    proxy_read_timeout 3600s;
}
location / {
    proxy_pass http://127.0.0.1:8080;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
    client_max_body_size 210m;
}
```

### Trình chiếu từ xa (điện thoại, trực tuyến, phụ đề)

BlueSlide › Trình chiếu › nhóm "Trình chiếu từ xa" (SLIDE-REMOTE-001):

- **Điều khiển từ điện thoại**: hộp thoại hiện mã QR + liên kết `/remote/<mã>`; điện thoại quét mã (mã dùng một lần,
  hết hạn sau 5 phút) rồi chuyển trang, bật màn hình đen, kết thúc trình chiếu. Điện thoại phải **mở được địa chỉ của
  máy chủ**: người trình bày cần mở BlueOffice bằng tên miền / địa chỉ LAN (không phải `localhost`, hộp thoại sẽ cảnh
  báo), điện thoại ở cùng mạng (Wi-Fi nội bộ) hoặc qua VPN. Không cần Internet, không dùng dịch vụ bên thứ ba.
- **Trình chiếu trực tuyến**: người xem mở liên kết `/watch/<token>`; bản trình bày được tải lên máy chủ (giới hạn
  `Present__MaxDeckBytes`, cần quyền tải xuống tệp) và người xem theo dõi trang + bước hiệu ứng đang chiếu.
- **Phụ đề trực tiếp**: dùng dịch vụ đọc chính tả đã cấu hình ([dictation.md](dictation.md)) — hoặc bộ nhận dạng của
  trình duyệt — và dịch qua dịch vụ dịch nếu có ([translation.md](translation.md)). Chưa cấu hình thì nút phụ đề báo lý
  do và không bật được.
- Cấu hình (`appsettings.json` mục `Present`, hoặc biến môi trường): `Present__Enabled` (mặc định `true`),
  `Present__PairingTtl` (`00:05:00`), `Present__MaxLifetime` (`12:00:00`), `Present__IdleTimeout` (`00:30:00`),
  `Present__MaxDeckBytes`, `Present__MaxViewers` (200), `Present__MaxPerOwner` (5), `Present__CommandLimit` /
  `Present__CommandWindow`, `Present__RequestLimit` / `Present__RequestWindow`. An toàn thông tin: [security.md](security.md) §10.2.
- Web shell build riêng với API ở địa chỉ khác: `VITE_BO_PRESENT_URL` (mặc định cùng origin, `off` = tắt trong giao diện).
- Buổi trình chiếu nằm trong bộ nhớ của instance `api`; với nhiều instance, mọi `/api/present` phải tới cùng một instance
  (`deploy/web/nginx-multi.conf` gửi về `api-1`).

### Nhiều instance `api`
Phiên soạn thảo và phòng cộng tác nằm trong bộ nhớ của một instance, nên mọi yêu cầu của **một file** phải tới cùng
instance. Bật bằng file ghép `deploy/docker-compose.multi.yml` (hai instance `api-1`, `api-2`):

```bash
docker compose -f deploy/docker-compose.yml -f deploy/docker-compose.multi.yml --env-file deploy/.env up -d
```

- `Collab__Ownership=Redis`: mỗi phòng giữ một *lease* trong Redis (`SET blueoffice:room:<phòng> <instance> NX PX`,
  gia hạn mỗi nhịp bảo trì, nhả khi phòng đóng; chỉ instance đang giữ mới gia hạn/nhả được). Instance không giữ
  lease đóng WebSocket với mã **4009** (lý do = id instance đang giữ); instance mất lease (mất kết nối Redis quá
  `Collab__LeaseTime`, mặc định 30 s) ngắt khách của phòng bằng 4009 để không có hai phòng cho cùng một file.
- `Collab__InstanceId` (mặc định: tên máy) đứng đầu id phiên: `{instance}~{guid}`. Yêu cầu `/api/sessions/{id}` tới nhầm
  instance nhận **421**.
- `deploy/web/nginx-multi.conf`: `/collab/{fileId}` và yêu cầu có `?file=` (client gửi kèm khi tạo phiên, `/edit?file=`)
  băm nhất quán theo id file; `/api/sessions/{instance}~…` chuyển thẳng tới instance đó (tên máy = id instance); phần
  còn lại cân bằng tải. Thêm instance: thêm dịch vụ `api-N` và dòng `server api-N:8080;` trong `upstream bo_api`.
- Khi thêm/bớt instance, một phần file đổi instance: phòng đang mở vẫn ở instance cũ tới khi đóng (khách mới nhận 4009 và
  kết nối lại có backoff tới khi phòng cũ đóng).

### Tích hợp BlueDrive

Điền nhóm biến `BLUEDRIVE_*` trong `.env` (URL BOHP, JWKS, issuer, webhook + secret, origin được phép nhúng iframe).
BlueDrive mở editor bằng `https://office.<domain>/edit?host=bluedrive&file={id}&token={jwt}`.
Để trống `BLUEDRIVE_BOHP_URL` nếu chưa tích hợp. Host WOPI khác (Nextcloud, SharePoint on-prem) khai báo thêm
`Hosts__1__Name`, `Hosts__1__Protocol=Wopi`, `Hosts__1__BaseUrl=https://<host>/wopi/files`; trang discovery:
`https://office.<domain>/hosting/discovery`.

**Desktop (màn hình Start, tab BlueDrive)**: desktop chỉ gọi BlueOffice server (`/api/hosts`, `/api/hosts/bluedrive/…`,
xem [`docs/bohp.md`](../bohp.md) §A2), không gọi thẳng BlueDrive. Đăng nhập SSO: điền `BLUEDRIVE_SIGNIN_AUTHORIZE_URL`,
`BLUEDRIVE_SIGNIN_TOKEN_URL`, `BLUEDRIVE_SIGNIN_CLIENT_ID` (public client, PKCE S256, redirect URI `bluedoc://auth`,
`bluesheet://auth`, `blueslide://auth`; token endpoint cho phép CORS từ `http://tauri.localhost`). Access token nhận được
phải là token BOHP của BlueDrive (xác minh bằng `BLUEDRIVE_JWKS_URL`). Để trống thì desktop chỉ cho dán token.
Thêm `http://tauri.localhost` (Windows/Linux) và `tauri://localhost` (macOS) vào `Cors__Origins__N`.

## 4. Cấu hình quan trọng (biến môi trường)

| Biến | Ý nghĩa |
|---|---|
| `Auth__Authority`, `Auth__Audience` | Realm OIDC và audience của token người dùng |
| `Storage__MaxUploadBytes` | Giới hạn kích thước upload |
| `Collab__AutosaveInterval` | Chu kỳ autosave về host khi có thay đổi (mặc định 45 s, nằm trong 30–60 s) |
| `Collab__SnapshotInterval` | Chu kỳ ghi snapshot đầy đủ của phiên cộng tác (mặc định 60 s) |
| `Collab__SnapshotStore` | Nơi giữ log cộng tác (snapshot + cập nhật chưa lưu): `Database` (mặc định, bảng `CollabLog` trong PostgreSQL) hoặc `ObjectStore` (MinIO) |
| `Wopi__ProofKey`, `Wopi__OldProofKey` | Khoá RSA (PEM hoặc đường dẫn file PEM) ký `X-WOPI-Proof` / `X-WOPI-ProofOld`; khoá công khai ở `/hosting/discovery` (`proof-key`). Bắt buộc ở Production khi có host WOPI; khi xoay khoá, đặt khoá cũ vào `OldProofKey` |
| `Collab__PermissionRecheckInterval` | Chu kỳ hỏi lại quyền từ host (mặc định 60 s) |
| `Collab__TimeZone` | Múi giờ dùng trong tên bản xung đột (mặc định `Asia/Ho_Chi_Minh`) |
| `Jobs__Concurrency`, `WORKER_REPLICAS` | Số job song song mỗi worker / số worker |
| `BoCli__Timeout` | Thời gian tối đa một lần chạy `bo-cli` (mặc định 60 s) |
| `BoCli__AllowedExtensions__N` | Định dạng đầu vào của `/convert` `/thumbnail` `/preview` `/extract-text` (mặc định docx, docm, doc, odt, txt, md, xlsx, xlsm, xlsb, xls, ods, pptx, ppt, odp) |
| `Scripts__ScheduleEnabled`, `Scripts__Timeout`, `Scripts__MemoryMb`, `Scripts__MaxCalls`, `Scripts__MaxConsecutiveFailures`, `DataProtection__KeysPath` | Chạy BlueScript theo lịch bằng `bo-cli script run` (mặc định tắt), giới hạn mỗi lần chạy, tự tắt lịch sau N lỗi liên tiếp; thư mục khóa Data Protection giữ token host của lịch ([scripts.md](scripts.md)) |
| `RestApi__Enabled`, `RestApi__Keys__N__*`, `RestApi__MaxUploadBytes`, `RestApi__RateLimit__*` | REST API tài liệu `/api/v1` cho hệ thống khác: bật, khoá API (chỉ lưu SHA-256) + scope, giới hạn kích thước và tần suất ([rest-api.md](rest-api.md)) |
| `Webhooks__MaxAttempts`, `Webhooks__RetryBaseDelay` | Số lần gửi webhook (mặc định 3, thử lại khi lỗi mạng/5xx/429) và độ trễ gốc (1 s → 1 s, 4 s) |
| `Hosts__N__SignIn__*`, `Hosts__N__DisplayName` | Đăng nhập SSO của màn hình Start desktop và tên hiển thị host |
| `Otel__Endpoint` | OTLP collector (trace + metric), để trống nếu không dùng |

## 5. An toàn thông tin (gợi ý cho hồ sơ cấp độ)

- Không có mật khẩu người dùng trong BlueOffice: xác thực bằng OIDC; token host (BlueDrive) xác minh bằng JWKS, ngắn hạn.
- `Auth:DevSigningKey` bị từ chối khi `ASPNETCORE_ENVIRONMENT=Production`.
- Mọi request tới host WOPI được ký bằng proof key ([MS-WOPI]); host kiểm tra với khoá công khai trong discovery.
- Chỉ `api` được publish cổng (bind `127.0.0.1`), các service dữ liệu nằm trong mạng nội bộ của compose.
- Header bảo mật: `X-Content-Type-Options`, `Referrer-Policy: no-referrer`, `Content-Security-Policy: frame-ancestors`
  chỉ gồm origin host đã cấu hình. CORS chỉ cho phép origin cấu hình.
- Webhook gửi host được ký HMAC-SHA256 (`X-BlueOffice-Signature`), host phải kiểm tra chữ ký + timestamp.
- `WOPISrc` chỉ được chấp nhận khi thuộc host đã cấu hình (chống SSRF); tên file client không bao giờ dùng làm đường dẫn.
- Audit log (mở, sửa, tải về, in, xuất, chia sẻ, khôi phục) lưu trong PostgreSQL (bảng `AuditEntries`) và chuyển về host
  qua webhook `audit`. Log ứng dụng ra stdout (Serilog) → gom bằng hệ thống log của đơn vị.
- Container chạy user không phải root; `bo-cli` chạy với danh sách tham số (không qua shell), có timeout và kill cả cây tiến trình.

## 6. Vận hành

- **Một instance `api`**: phiên cộng tác (room Yjs) được giữ trong bộ nhớ của tiến trình `api`; mọi người sửa cùng file
  phải vào cùng instance. Chưa hỗ trợ nhiều instance `api` sau load balancer (xem `docs/plan/phase-6.md`).
  `worker` có thể scale ngang.
- Dừng `api` (`docker compose stop api`) → các phiên đang mở được lưu snapshot vào MinIO và khôi phục ở phiên kế tiếp
  nếu nội dung trên host chưa đổi.
- Log: `docker compose -f deploy/docker-compose.yml logs -f api worker`.
- Metric OpenTelemetry: `blueoffice.collab.updates`, `blueoffice.collab.connections`, `blueoffice.jobs.submitted`,
  `blueoffice.jobs.completed` + metric HTTP chuẩn.
