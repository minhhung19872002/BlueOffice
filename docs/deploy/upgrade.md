# Nâng cấp BlueOffice

## Quy trình

1. Đọc ghi chú phát hành (release notes) của phiên bản mới: thay đổi cấu hình, thay đổi schema, yêu cầu dừng dịch vụ.
2. **Sao lưu** đầy đủ theo [backup.md](backup.md).
3. Lấy image mới:
   ```bash
   # image do Bluestar cung cấp
   docker load -i blueoffice-server-<version>.tar
   # hoặc build từ mã nguồn đã checkout tag mới
   git fetch --tags && git checkout v<version>
   docker compose -f deploy/docker-compose.yml --env-file deploy/.env build
   ```
4. Đặt `BLUEOFFICE_IMAGE=blueoffice/server:<version>` trong `deploy/.env`; so sánh `deploy/.env.example` mới với `.env`
   để bổ sung biến mới.
5. Thông báo người dùng lưu tài liệu; áp dụng:
   ```bash
   C="docker compose -f deploy/docker-compose.yml --env-file deploy/.env"
   $C stop api worker          # các phiên cộng tác ghi snapshot rồi đóng
   $C up -d                    # khởi động image mới
   curl -fsS http://127.0.0.1:8080/health/ready
   curl -fsS http://127.0.0.1:8080/api/version
   ```
6. Kiểm tra nhanh: đăng nhập, mở một file, sửa, lưu, xem lịch sử phiên bản, xuất PDF.

## Schema cơ sở dữ liệu

Từ bản 0.7 schema do **EF Core migrations** quản lý (`apps/server/BlueOffice.Infrastructure/Data/Migrations/Postgres`
và `.../Sqlite`, bảng lịch sử `__EFMigrationsHistory`). Khi khởi động, api chạy các migration còn thiếu nếu
`Database__AutoCreate` bật (mặc định). Cơ sở dữ liệu tạo bằng `EnsureCreated` ở bản ≤ 0.6 được **nhận** tự động: api
tạo bảng còn thiếu (vd `CollabLog`), ghi migration `Initial` là đã áp dụng rồi chạy các migration sau.

Tắt `Database__AutoCreate` thì DBA áp schema bằng script SQL sinh từ migrations (idempotent):

```bash
dotnet ef migrations script --idempotent --context PostgresBlueOfficeDbContext \
  --project apps/server/BlueOffice.Infrastructure -o upgrade.sql
$C exec -T postgres psql -U blueoffice -d blueoffice -v ON_ERROR_STOP=1 < upgrade.sql
```

Nhà phát triển đổi model: thêm migration cho **cả hai** provider (test `MigrationsAreUpToDateWithTheModel` báo khi quên):

```bash
cd apps/server/BlueOffice.Infrastructure
dotnet ef migrations add <Ten> --context PostgresBlueOfficeDbContext --output-dir Data/Migrations/Postgres
dotnet ef migrations add <Ten> --context SqliteBlueOfficeDbContext --output-dir Data/Migrations/Sqlite
``` Từ bản có `CollabLog`, log cộng tác mặc định nằm trong cơ sở dữ liệu
(`Collab__SnapshotStore=Database`): log chưa lưu của bản cũ (MinIO) không được đọc lại, nên lưu hết tài liệu trước khi
nâng cấp, hoặc giữ `Collab__SnapshotStore=ObjectStore`.


## Quay lui (rollback)

Nếu bản mới lỗi: dừng `api worker`, đặt lại `BLUEOFFICE_IMAGE` cũ; nếu đã chạy script schema thì khôi phục PostgreSQL
(và MinIO) từ bản sao lưu ở bước 2, rồi `up -d`.

## Thành phần phụ thuộc

- PostgreSQL: nâng cấp minor (16.x) chỉ cần đổi tag image; nâng major (16 → 17) phải `pg_dump`/`pg_restore` hoặc `pg_upgrade`.
- MinIO, Redis, Keycloak: theo hướng dẫn của từng dự án; kiểm tra lại `/health/ready` sau khi nâng.
