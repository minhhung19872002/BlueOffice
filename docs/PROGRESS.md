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
| `pnpm tauri build --no-bundle` | ✅ `bluedoc.exe` 21 MB (kèm 6,5 MB font) |

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

## Đổi tên sản phẩm · ✅ (2026-11-05)
BlueWrite → **BlueDoc**; route `/doc` `/sheet` `/slide` (route cũ chuyển hướng); desktop `vn.bluestar.bluedoc|bluesheet|blueslide`,
binary `bluedoc.exe`; corpus `tests/corpus/doc`; quy tắc đặt tên ở `CLAUDE.md` và ADR-0006; Phase 7 (BlueForm, BlueNote) chỉ có kế hoạch.
Toàn bộ test xanh sau khi đổi tên (Rust 53, TS 28, E2E web 5, .NET 6, smoke test desktop BlueDoc/BlueSheet).

## Phase 2–6A · ⏸ đang làm dở, tạm dừng theo yêu cầu (2026-11-05)

`main` chỉ chứa phần đã kiểm thử (Phase 1 + đổi tên). Phần dở dang nằm ở các nhánh riêng trên GitHub,
**chưa được kiểm thử/hợp nhất** — cần review, chạy test và merge từng nhánh:

| Nhánh | Phạm vi | Tình trạng khi dừng |
|---|---|---|
| `wip/phase2-bluedoc` | Phase 2: model mở rộng (styles, numbering, bảng, ảnh, header/footer, field, footnote, XML gốc), reader/writer `.docx` lossless, `bo_model::index`, phân giải style, layout đoạn văn mới | Model + `bo-docx` biên dịch và **test xanh** (round-trip lossless, sửa 1 đoạn giữ nguyên các đoạn khác). `bo-layout` đang viết lại: `paragraph.rs`/`style.rs`/`types.rs` mới xong, **chưa có** `lib.rs` (phân trang, bảng, header/footer, cột, footnote) → crate chưa biên dịch. Chưa làm: render ảnh/bảng, `bo-edit` (chỉnh sửa, undo/redo), UI caret/selection, clipboard, find/replace, lưu .docx từ editor |
| `wip/phase3-formula` | `bo-formula`: lexer/parser, giá trị, hàm | Dừng khi đang viết workbook (dependency graph, spill); chưa xác minh build/test |
| `wip/phase3-xlsx-sheet` | `bo-xlsx` + `bo-sheet` + `bo_model::sheet` | Dừng khi đang viết grid geometry; chưa xác minh |
| `wip/phase4-pptx-drawingml` | `bo-pptx` + `bo-drawingml` + `bo_model::slide` | Đã biên dịch được; đang làm writer giữ thứ tự relationship, chưa có corpus |
| `wip/phase5-legacy-crypto-odf` | `bo-binary`, `bo-crypto`, `bo-odf`, `bo-encoding` (TCVN3/VNI) | Dừng khi đang làm bảng hàm `.xls` (Ptg → text); chưa xác minh |
| `wip/phase6-server-bohp` | Server .NET Phase 6/6A, BOHP, mock BlueDrive | Dừng khi đang viết test conversion job; chưa xác minh |

### Khi tiếp tục
1. Phase 2: viết `bo-layout/src/lib.rs` (flow/pagination theo `ParaLayout`), bảng, header/footer → cập nhật `bo-render` (Image/PageItem),
   `bo-engine`; rồi `bo-edit` + editor UI. Kế hoạch: `docs/plan/phase-2.md`.
2. Mỗi nhánh `wip/*` của phase 3–6: checkout, `cargo test`/`dotnet test`, sửa đến khi xanh, cập nhật tên sản phẩm mới (BlueDoc…) nếu còn sót, rồi merge vào `main`.
3. Phase 6A: xác nhận API thật của BlueDrive (prompt §6A yêu cầu hỏi trước) — nhánh server hiện cài BOHP đúng như mô tả trong prompt.
