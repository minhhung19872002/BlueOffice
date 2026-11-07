# Triển khai BlueOffice on-premise (Docker Compose)

Tài liệu cho quản trị viên hệ thống. Sao lưu: [backup.md](backup.md) · Nâng cấp: [upgrade.md](upgrade.md) ·
Giao thức tích hợp BlueDrive: [../bohp.md](../bohp.md).

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

### Reverse proxy

- Chuyển tiếp `https://office.<domain>/` → `http://127.0.0.1:8080/`.
- **Bật WebSocket** cho đường dẫn `/collab/` (header `Upgrade`/`Connection`), timeout đọc ≥ 120 s
  (server gửi keep-alive 30 s).
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

### Tích hợp BlueDrive

Điền nhóm biến `BLUEDRIVE_*` trong `.env` (URL BOHP, JWKS, issuer, webhook + secret, origin được phép nhúng iframe).
BlueDrive mở editor bằng `https://office.<domain>/edit?host=bluedrive&file={id}&token={jwt}`.
Để trống `BLUEDRIVE_BOHP_URL` nếu chưa tích hợp. Host WOPI khác (Nextcloud, SharePoint on-prem) khai báo thêm
`Hosts__1__Name`, `Hosts__1__Protocol=Wopi`, `Hosts__1__BaseUrl=https://<host>/wopi/files`; trang discovery:
`https://office.<domain>/hosting/discovery`.

## 4. Cấu hình quan trọng (biến môi trường)

| Biến | Ý nghĩa |
|---|---|
| `Auth__Authority`, `Auth__Audience` | Realm OIDC và audience của token người dùng |
| `Storage__MaxUploadBytes` | Giới hạn kích thước upload |
| `Collab__AutosaveInterval` | Chu kỳ autosave về host khi có thay đổi (mặc định 45 s, nằm trong 30–60 s) |
| `Collab__SnapshotInterval` | Chu kỳ ghi snapshot đầy đủ của phiên cộng tác vào MinIO (mặc định 60 s) |
| `Collab__PermissionRecheckInterval` | Chu kỳ hỏi lại quyền từ host (mặc định 60 s) |
| `Collab__TimeZone` | Múi giờ dùng trong tên bản xung đột (mặc định `Asia/Ho_Chi_Minh`) |
| `Jobs__Concurrency`, `WORKER_REPLICAS` | Số job song song mỗi worker / số worker |
| `BoCli__Timeout` | Thời gian tối đa một lần chạy `bo-cli` (mặc định 60 s) |
| `Otel__Endpoint` | OTLP collector (trace + metric), để trống nếu không dùng |

## 5. An toàn thông tin (gợi ý cho hồ sơ cấp độ)

- Không có mật khẩu người dùng trong BlueOffice: xác thực bằng OIDC; token host (BlueDrive) xác minh bằng JWKS, ngắn hạn.
- `Auth:DevSigningKey` bị từ chối khi `ASPNETCORE_ENVIRONMENT=Production`.
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
