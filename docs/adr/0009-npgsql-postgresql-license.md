# ADR-0009 — Chấp nhận Npgsql (PostgreSQL License) cho server

- Trạng thái: chấp nhận (2026-11-08); chủ dự án có thể bác bỏ — khi đó xem "Phương án thay thế".
- Bối cảnh: prompt §1 cho phép dependency license MIT / Apache-2.0 / BSD / MPL-2.0 / Zlib / OFL. Server dùng
  PostgreSQL (prompt §3.1) qua EF Core; driver .NET duy nhất được duy trì là **Npgsql**
  (`Npgsql`, `Npgsql.EntityFrameworkCore.PostgreSQL`), phát hành theo **PostgreSQL License**.

## Phân tích
- PostgreSQL License được OSI phê duyệt, cùng họ với giấy phép BSD/MIT: cho phép dùng, sửa, phân phối (kể cả thương mại,
  mã đóng) miễn giữ thông báo bản quyền và đoạn miễn trừ trách nhiệm; không copyleft, không điều khoản bằng sáng chế
  hạn chế, không yêu cầu công bố mã nguồn. Về nghĩa vụ, nó **nhẹ hơn hoặc bằng** BSD-2-Clause/MIT.
- Mục đích của quy tắc §1 là loại GPL/AGPL/LGPL-static (copyleft); PostgreSQL License không thuộc nhóm đó.

## Quyết định
Xếp PostgreSQL License vào nhóm "BSD-style permissive" được phép. Ghi trong `THIRD_PARTY_NOTICES.md` (đã có) và giữ
nguyên văn thông báo bản quyền trong bản phân phối server (gói NuGet đã kèm `LICENSE`). Chỉ dùng ở server (.NET);
không ảnh hưởng `cargo deny` / `pnpm license-check` (Rust/npm).

## Phương án thay thế (nếu chủ dự án không chấp nhận)
- Dùng SQL Server/SQLite qua provider EF Core của Microsoft (MIT) — `BlueOffice.Infrastructure` đã tách `DbContext`, chỉ
  cần đổi `UseNpgsql` sang `UseSqlServer`/`UseSqlite` trong `DependencyInjection.cs` và cấu hình chuỗi kết nối.
- Loại bỏ PostgreSQL khỏi `deploy/docker-compose.yml` tương ứng.
