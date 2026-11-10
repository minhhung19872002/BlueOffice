# Bàn giao phiên (2026-11-10)

Trạng thái khi tạm dừng vòng lặp Parity + UI (`BlueOffice-PARITY-PROMPT.md`, `BlueOffice-UI-PROMPT.md`). Phiên sau đọc file
này trước, rồi tiếp tục theo `BACKLOG.md` (mục `[~]` = đang làm dở).

## Đã xong và đã push lên `main`
- BACKLOG: 57 mục `[x]`. Rà soát định kỳ lần 5 tới hạn khi đạt khoảng 64 `[x]`.
- Đợt gộp cuối: DOC-MERGE-001, SHEET-CELL-001, CORE-PRINT-001, CORE-INK-001 (đều `[p]`, ghi chú phần thiếu trong BACKLOG).
- UI: UI-TOKENS-001..005, UI-ICONS-001, UI-ICONS-002, UI-FRAME-001, UI-FRAME-003 (`docs/ui/BACKLOG.md`).
- Regression đầy đủ xanh. `sheet-pivot-extras.spec.ts` / `sheet-pivot-settings.spec.ts` thỉnh thoảng lỗi khi E2E chạy song song
  lúc máy tải nặng; chạy lại `--workers=1` thì qua.

## Nhánh agent chưa gộp (worktree trong `.claude/worktrees/`)
| Mục | Nhánh | Trạng thái |
|---|---|---|
| CORE-EMBED-001 | `worktree-agent-abe0b5e938e6d808d` (commit `eb7c202`) | **Đã xong, đã commit, chưa gộp.** Gộp → regression → đánh `[p]`: nhúng/liên kết vùng BlueSheet trong Doc/Slide, đọc/giữ OLE, sửa bằng lưới trong hộp thoại, cập nhật liên kết (desktop). Thiếu: mở bằng BlueSheet thật, dán đặc biệt dạng workbook nhúng, xem trước EMF/WMF, chép/dán đối tượng. |
| CORE-FMT-002 | `worktree-agent-a2b47637a5a8898a0` | Dừng giữa chừng, **chưa commit**. Xem `git -C <worktree> status`; làm tiếp hoặc giao lại. |
| CORE-SMART-005 | `worktree-agent-a7a8ff7914a0fc49f` | Dừng giữa chừng, chưa commit. |
| CORE-LANG-003 | `worktree-agent-a9bd5570ae7755c82` | Dừng giữa chừng, chưa commit. |
| CORE-VERSION-001 | `worktree-agent-a440d00ca01166e7c` | Dừng giữa chừng, chưa commit. |

Thư mục build của agent: `D:/bo-targets/<tên>` (xoá khi không dùng; ổ D/C còn khoảng 40/28 GB).

## Cách làm đã dùng
- Brief chung cho agent: mỗi agent làm trong worktree riêng, `CARGO_TARGET_DIR` riêng, `CARGO_PROFILE_DEV_DEBUG=0`, không sửa
  BACKLOG/MATRIX/PROGRESS, tự commit (kèm hai dòng Co-Authored-By/Claude-Session).
- Gộp: `git merge --no-edit worktree-agent-<id>`; xung đột i18n JSON giải bằng cách nối hai khối rồi kiểm tra `json.loads`;
  danh sách import/field/variant thì gộp cả hai bên — cẩn thận hunk bị cắt giữa khối (`if let … {` / JSX): dựng lại xung đột bằng
  `git checkout -m <file>` và sửa tay.
- Regression đầy đủ: lệnh trong CLAUDE.md (clippy → test → build:wasm → lint → typecheck → test → web build → E2E). Spec E2E mới
  do agent viết thường chưa chạy: lỗi thì xem kỹ — đã gặp lỗi do spec (đợi chưa đủ, nhấp sai chỗ, giả định sai ngôn ngữ).

## Việc tiếp theo
1. Gộp CORE-EMBED-001, chạy regression, cập nhật BACKLOG/MATRIX/PROGRESS, push.
2. Xử lý 4 worktree dở (làm tiếp hoặc xoá và giao lại agent mới).
3. Giữ khoảng 4–5 agent cho các mục P1 còn `[ ]` (DOC-TPL-002, CORE-LANG-001/002, CORE-UI-*, SHEET-PQ-001, CORE-SCRIPT-001,
   SLIDE-REC-001, CORE-3DMODEL-001, CORE-COLLAB-001, CORE-COMMENT-001, CORE-PERF-001, SHEET-PERF-001, CORE-MISC-005…).
4. UI: tiếp UI-ICONS-003, UI-CMD-001, UI-RIBBON-*, UI-FILE-*, UI-COMP-*, UI-KEYTIPS-001, UI-A11Y-*.
5. Rà soát định kỳ lần 5; cuối cùng `docs/parity/FINAL_REPORT.md` và `docs/ui/REPORT.md` (kèm ảnh chụp).
