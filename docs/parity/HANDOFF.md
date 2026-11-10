# Bàn giao phiên (2026-11-10, tối)

Trạng thái khi dừng vòng lặp Parity (`BlueOffice-PARITY-PROMPT.md`). Phiên sau đọc file này trước, rồi tiếp tục theo
`BACKLOG.md`. Số liệu: 66 `[x]`, 75 `[p]`, 28 `[ ]` (P1 đã hết mục `[ ]`; còn lại là P2/P3).

## Đã gộp vào `main` trong phiên này
CORE-EMBED-001, CORE-VERSION-001, CORE-LANG-003, CORE-SMART-005, CORE-FMT-002, DOC-TPL-002, CORE-MISC-005,
CORE-COMMENT-001, CORE-UI-001, CORE-UI-006, CORE-SCRIPT-001, CORE-COLLAB-001, CORE-UI-002, SHEET-PERF-001,
CORE-SCRIPT-002, SHEET-PERF-002, CORE-LANG-001, CORE-UI-003, CORE-UI-004, SHEET-PQ-001, CORE-LANG-002, SLIDE-REC-001,
CORE-3DMODEL-001, CORE-UI-005, SHEET-DM-001, CORE-PERF-001, SLIDE-ZOOM-001, DOC-STYLE-001 (trạng thái `[x]`/`[p]` và phần
thiếu ghi trong BACKLOG). Rà soát định kỳ lần 5 đã làm (fidelity, baseline, MATRIX). Lần 6 tới hạn khoảng 79 `[x]`.

## Regression
- Mốc xanh đầy đủ gần nhất (commit "docs(parity): ribbon customization and open performance done…"): Rust, TS, .NET 208,
  E2E web 132/132, cộng tác 9/9.
- Sau đó gộp SLIDE-ZOOM-001 + DOC-STYLE-001 (đã push): clippy + test Rust + TS xanh; **E2E web 122 qua / 5 lỗi / 8 không
  chạy khi máy tải nặng (5 agent build song song) — CHƯA chạy lại tuần tự**; cộng tác 9/9; build desktop + E2E desktop
  bị dừng giữa chừng. **Việc đầu tiên của phiên sau:** `pnpm build:wasm`, build web, `pnpm exec playwright test
  --workers=1` trong `tests/e2e`, rồi build desktop + E2E desktop.
- E2E desktop: 15/15 lần cuối sau CORE-UI-002. Cần `msedgedriver` khớp WebView2 (157.0.4341.42), tải từ
  `https://msedgedriver.microsoft.com/<phiên bản>/edgedriver_win64.zip`, rồi
  `MSEDGEDRIVER=<đường dẫn> pnpm test:e2e:desktop`. `target/release/bluedoc.exe` hiện là bản 13:54 (cũ hơn `main`).
- Test chập chờn khi tải nặng: xem CORE-MISC-006 trong BACKLOG.

## Nhánh agent chưa gộp (worktree trong `.claude/worktrees/`)
| Mục | Nhánh | Trạng thái |
|---|---|---|
| SLIDE-ACTION-001 | `worktree-agent-aa7f66d3e713bc074` | **Xong, đã commit, chưa gộp** → `[p]`: hyperlink/action hình + chữ, hộp thoại Liên kết/Hành động, nút hành động, trình chiếu + chính sách an toàn (`docs/deploy/security.md` §10.1), link trong PDF. Thiếu: link trong ô bảng, `mailto:` trên desktop, cửa sổ khán giả, mục menu chuột phải. |
| CORE-CITE-001 | `worktree-agent-a8ad51d345fca3ca4` | **Xong, đã commit, chưa gộp** → `[p]`: nguồn `b:Sources`, trường CITATION/BIBLIOGRAPHY, 8 kiểu tự viết (APA, MLA, Chicago, IEEE, Harvard, ISO 690 ×2, luận văn VN), BibTeX/RIS, UI thẻ Tham chiếu. Thiếu: header/footer/chú thích, in nghiêng trong trích dẫn, bản địa hoá đầy đủ, RIS export, cộng tác. |
| CORE-FMT-003 | `worktree-agent-a18b658ce9327ccdb` | Dừng giữa chừng: có commit (Excel XML 2003, mở/lưu qua engine + bo-cli…) **và 14 tệp chưa commit**; đang ở bước lint/test. Xem `git status`, hoàn tất hoặc giao lại. |
| DOC-BB-001 | `worktree-agent-ae3ba4472966c3a06` | Dừng giữa chừng: đã commit phần glossary + op sửa; **30 tệp chưa commit** (đang tích hợp BlueEditor). |
| CORE-UI-007 | `worktree-agent-a4bf5802ac2bca83a` | Dừng giữa chừng: đã commit mô hình thanh trạng thái + tương phản cao; **22 tệp chưa commit** (đang viết E2E). |

Worktree thừa có thể xoá: `agent-a14c92a9cb513d27b`, `agent-a71cbd8088a19759d` (đã gộp), `agent-a24d10d74eac19d89`.
Thư mục build của agent: `D:/bo-targets/<tên>` (`f3`, `bb1`, `u7` thuộc ba nhánh dở).

## Cách làm đã dùng
- Mỗi agent một worktree (`isolation: worktree`), `git merge --no-edit main` trước, `CARGO_TARGET_DIR=D:/bo-targets/<tên>`
  + `CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0`, không sửa BACKLOG/MATRIX/PROGRESS, tự commit. Dặn agent: test parity
  của platform tự build bo-cli — phải đặt `CARGO_TARGET_DIR`; dùng tệp tạm tên riêng (scratchpad dùng chung); commit
  snapshot insta mới của corpus mình thêm.
- **Ổ D hay đầy**: `.pdb` và `incremental` trong `target/` và `D:/bo-targets` là thủ phạm chính
  (`find target -name '*.pdb' -delete`; xoá `target/*/incremental`; xoá thư mục build của agent đã gộp). Kiểm tra `df -h /d`
  trước mỗi đợt agent; 3–5 agent build Rust cùng lúc cần khoảng 40 GB trống.
- Gộp: xung đột i18n JSON luôn là "hai bên cùng thêm khối cuối tệp" → nối cả hai và đóng ngoặc bên "ours" (đếm độ sâu
  ngoặc), kiểm bằng `json.loads` + khoá trùng; xung đột Rust/TSX kiểu hunk bị cắt giữa khối → `git checkout -m <file>` rồi
  thêm phần đóng khối cho bên "ours". Sau khi gộp hai nhánh cùng thêm export vào `packages/platform/src/index.ts`, chạy
  `pnpm typecheck` (đã gặp trùng tên `encodeWav`).
- Regression đầy đủ: clippy → `cargo test --workspace --no-fail-fast` → `cargo deny` → `pnpm build:wasm` → typecheck →
  lint → license-check → `pnpm test` → `dotnet test` → build web → Playwright (`--workers=2`; lỗi khi tải nặng thì chạy lại
  `--workers=1`) → `test:collab` → (định kỳ) build desktop + E2E desktop, `pnpm fidelity`.

## Việc tiếp theo
1. Chạy lại E2E web tuần tự + desktop cho `main` hiện tại (xem mục Regression).
2. Gộp SLIDE-ACTION-001 và CORE-CITE-001 → regression → BACKLOG/MATRIX `[p]`.
3. Xử lý ba worktree dở (CORE-FMT-003, DOC-BB-001, CORE-UI-007).
4. P2 còn `[ ]`: CORE-ADDIN-001, CORE-AI-001, CORE-API-001, CORE-CHART-006 ([!] dữ liệu bản đồ), CORE-DESIGN-001,
   CORE-FMT-004, CORE-ICON-001, CORE-INK-002, CORE-LANG-004, CORE-MATH-010, CORE-MISC-003/004/006, CORE-PIC-002 ([!]
   model), CORE-SCRIPT-003/004, CORE-THES-001, CORE-UI-008, SHEET-FN-CUBE, SLIDE-ALBUM-001, SLIDE-REMOTE-001; P3:
   CORE-MISC-002.
5. Sau đó: rà lại các mục `[p]` (75 mục) xem phần thiếu nào làm được bằng nền tảng mới; rà soát định kỳ lần 6; vòng rà
   soát cuối (prompt mục 9: fuzz 1 giờ mỗi parser, WCAG 2.2 AA, tệp độc hại, tài liệu người dùng `docs/user-guide/`);
   `docs/parity/FINAL_REPORT.md`.
6. NEEDS_HUMAN: ảnh mốc từ phần mềm tham chiếu, model dịch / nhận dạng giọng nói cục bộ, dữ liệu bản đồ, model xoá nền.
