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

Bản 0.6 tạo schema bằng `EnsureCreated` khi khởi động lần đầu. Khi một bản phát hành thay đổi schema, gói phát hành
kèm script SQL `upgrade-<from>-<to>.sql` — chạy **sau khi dừng** `api`, **trước khi** khởi động image mới:

```bash
$C exec -T postgres psql -U blueoffice -d blueoffice -v ON_ERROR_STOP=1 < upgrade-<from>-<to>.sql
```

(Chuyển sang EF Core migrations tự động là việc còn lại của Phase 6, xem `docs/plan/phase-6.md`.)

## Quay lui (rollback)

Nếu bản mới lỗi: dừng `api worker`, đặt lại `BLUEOFFICE_IMAGE` cũ; nếu đã chạy script schema thì khôi phục PostgreSQL
(và MinIO) từ bản sao lưu ở bước 2, rồi `up -d`.

## Thành phần phụ thuộc

- PostgreSQL: nâng cấp minor (16.x) chỉ cần đổi tag image; nâng major (16 → 17) phải `pg_dump`/`pg_restore` hoặc `pg_upgrade`.
- MinIO, Redis, Keycloak: theo hướng dẫn của từng dự án; kiểm tra lại `/health/ready` sau khi nâng.
