# Sao lưu và khôi phục

Dữ liệu cần sao lưu: **PostgreSQL** (metadata, phiên bản, chia sẻ, audit) và **MinIO** (nội dung file, snapshot cộng tác).
Hai phần phải được sao lưu **cùng thời điểm** (cùng cửa sổ dừng ghi) để metadata khớp nội dung. Redis chỉ chứa job
chuyển đổi tạm thời — không cần sao lưu. File `deploy/.env` (mật khẩu) lưu riêng, có mã hoá.

Ví dụ dưới dùng `C="docker compose -f deploy/docker-compose.yml --env-file deploy/.env"`.

## Sao lưu hằng ngày

```bash
C="docker compose -f deploy/docker-compose.yml --env-file deploy/.env"
TS=$(date +%Y%m%d-%H%M)
mkdir -p /backup/blueoffice/$TS

# 1. Ngừng nhận chỉnh sửa (các phiên cộng tác được lưu snapshot khi api dừng)
$C stop api worker

# 2. PostgreSQL
$C exec -T postgres pg_dump -U blueoffice -d blueoffice -Fc > /backup/blueoffice/$TS/blueoffice.dump
$C exec -T postgres pg_dump -U blueoffice -d keycloak -Fc > /backup/blueoffice/$TS/keycloak.dump   # nếu dùng profile sso

# 3. MinIO (sao chép volume; hoặc dùng `mc mirror` sang MinIO/S3 khác)
docker run --rm -v blueoffice_minio-data:/data:ro -v /backup/blueoffice/$TS:/out alpine \
  tar -czf /out/minio-data.tar.gz -C /data .

# 4. Chạy lại
$C start api worker
sha256sum /backup/blueoffice/$TS/* > /backup/blueoffice/$TS/SHA256SUMS
```

Thời gian dừng thường dưới vài phút. Nếu không thể dừng: dùng `pg_dump` (nhất quán theo snapshot) **trước**, rồi
`mc mirror` MinIO **sau** — nội dung thừa trong MinIO vô hại, nhưng metadata trỏ tới nội dung chưa có thì lỗi.

Khuyến nghị: giữ 7 bản ngày + 4 bản tuần + 12 bản tháng, ít nhất một bản ở vị trí vật lý khác; mã hoá bản sao lưu
(vd. `gpg --symmetric` hoặc `restic`). Thử khôi phục định kỳ mỗi quý lên máy thử nghiệm.

## Khôi phục

```bash
C="docker compose -f deploy/docker-compose.yml --env-file deploy/.env"
B=/backup/blueoffice/<TS>
$C stop api worker
sha256sum -c $B/SHA256SUMS

# PostgreSQL: tạo lại database rồi restore
$C exec -T postgres dropdb -U blueoffice --if-exists blueoffice
$C exec -T postgres createdb -U blueoffice blueoffice
$C exec -T postgres pg_restore -U blueoffice -d blueoffice --no-owner < $B/blueoffice.dump

# MinIO
$C stop minio
docker run --rm -v blueoffice_minio-data:/data -v $B:/in alpine sh -c "rm -rf /data/* /data/.[!.]* ; tar -xzf /in/minio-data.tar.gz -C /data"
$C start minio

$C start api worker
curl -fsS http://127.0.0.1:8080/health/ready
```

## File mở từ BlueDrive

Với file lưu ở BlueDrive (BOHP), nội dung chính nằm ở BlueDrive — BlueOffice chỉ giữ snapshot phiên cộng tác chưa lưu
(`collab/` trong MinIO) và audit log. Sao lưu BlueDrive theo quy trình của BlueDrive.
