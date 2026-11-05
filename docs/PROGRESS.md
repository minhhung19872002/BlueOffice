# Tiến độ

## Phase 1 — Nền tảng · ✅ hoàn thành, chờ duyệt (2026-11-05)

Kế hoạch: [`plan/phase-1.md`](plan/phase-1.md).

### Đã làm
- Rust workspace (21 crate), pnpm workspace (9 package + 2 app + e2e), .NET solution (4 project).
- CI GitHub Actions: Rust (fmt, clippy pedantic `-D warnings`, test, build WASM) trên Ubuntu + Windows; `cargo deny`;
  Web (eslint, prettier, typecheck, vitest, license-check, build, Playwright); .NET (format, test); Desktop (Tauri build).
- Core: `bo-opc`, `bo-docx` (reader/writer tối thiểu), `bo-text`, `bo-layout`, `bo-render`, `bo-raster`, `bo-pdf`, `bo-engine`, `bo-wasm`, `bo-cli`.
- Web và desktop dùng chung `packages/editor`; desktop mở file từ đĩa, từ tham số dòng lệnh (file association), xuất PDF qua hộp thoại native.
- Server ASP.NET Core 8: `/health`, `/api/version`, `POST /api/convert/pdf` (gọi `bo-cli`).

### Kết quả kiểm thử (máy dev Windows 11, 2026-11-05)
| Bộ | Kết quả |
|---|---|
| `cargo test --workspace` | 53 test ✅ |
| `cargo clippy --workspace --all-targets -D warnings` (pedantic) | ✅ |
| `cargo deny check licenses bans sources` | ✅ |
| `pnpm test` (Vitest) | 26 test ✅ — gồm parity WASM ↔ native trên 5 trang mẫu |
| `pnpm lint` / `pnpm typecheck` / `pnpm license-check` | ✅ |
| Playwright E2E (web) | 4 test ✅ |
| `dotnet test` | 6 test ✅ |
| `pnpm tauri build --no-bundle` | ✅ `blueoffice.exe` 21 MB (kèm 6,5 MB font) |

### Benchmark
| Phép đo | Kết quả | Mục tiêu (§5) |
|---|---|---|
| Layout nguội 300 trang A4 (Tinos 13pt, căn đều) — criterion | 0,73 s | mở 300 trang < 3 s desktop |
| Sửa 1 đoạn → dàn lại 300 trang (cache theo đoạn) | 20,7 ms | < 16 ms → **chưa đạt**, tối ưu ở Phase 2 |
| `bo-cli bench` file mẫu 5 trang: mở + layout + display list | 8,2 ms | — |
| Mở + xuất PDF file mẫu (101 KB, font subset) | 17,1 ms | — |
| Bundle web: WASM 3,28 MB (gzip 1,12 MB), JS 302 KB (gzip 100 KB) | | |

### Ảnh chụp
- Web: [`screenshots/phase1-web.png`](screenshots/phase1-web.png)
- Desktop: [`screenshots/phase1-desktop.png`](screenshots/phase1-desktop.png)
- `bo-cli render`: [`screenshots/phase1-cli-render-page1.png`](screenshots/phase1-cli-render-page1.png)

### Hạn chế đã biết (chuyển sang Phase 2)
- Chưa chỉnh sửa (các nút định dạng hiển thị nhưng disabled); `ImeInput` mới báo sự kiện.
- Reader `.docx` chưa đọc styles (ngoài docDefaults), numbering, bảng, ảnh, header/footer; chưa round-trip lossless (writer chỉ dùng để sinh corpus).
- Relayout khi sửa vẫn hash lại toàn bộ đoạn (20 ms/300 trang) → cần dirty-tracking theo transaction (`bo-edit`).
- Chưa có Caladea (Cambria); font hệ thống chưa bật trên desktop (ADR-0003).
- In trên desktop: mở PDF bằng trình xem mặc định; hộp thoại in native ở Phase 2.
- E2E desktop (Tauri WebDriver) chưa có; desktop được kiểm bằng parity test + chạy tay (ảnh chụp).
- Font có thể khác hình chữ nhỏ so với font gốc (metric giống) — visual diff với ảnh tham chiếu bắt đầu Phase 2.

### Việc tiếp theo
Phase 2 — BlueWrite MVP (đọc/ghi .docx đầy đủ, chỉnh sửa). **Chờ duyệt trước khi bắt đầu.**
