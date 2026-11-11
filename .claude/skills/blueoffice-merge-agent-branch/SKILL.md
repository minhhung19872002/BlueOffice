---
name: blueoffice-merge-agent-branch
description: Use this skill when merging a `worktree-agent-*` branch (or any feature branch) into BlueOffice `main` during the Parity loop — resolving the recurring conflicts in `packages/i18n/src/locales/vi-VN.json` / `en-US.json`, keep-both hunks in imports / enums / result structs (`BlueEditor.tsx`, `BlueSlideEditor.tsx`, `types.ts`, `edit.rs`, `slide_edit/mod.rs`), duplicate ADR numbers, EF migrations, then running the full regression (cargo, pnpm, dotnet, Playwright, collab). Trigger words: "gộp nhánh agent", "merge worktree", "regression đầy đủ", "xung đột i18n".
---

# Gộp nhánh agent vào `main` (BlueOffice)

## Khi nào dùng
- Một agent báo xong việc trên nhánh `worktree-agent-<id>` và cần đưa vào `main`.
- Cần chạy regression đầy đủ sau một đợt gộp, hoặc trước khi push.
- Gặp xung đột i18n JSON / import / enum lặp đi lặp lại giữa các nhánh tính năng.

## Quy trình
1. **Không gộp khi regression đang chạy trên `main`** (đổi mã nguồn giữa chừng làm hỏng kết quả). Chờ `summary.txt` có `DONE`.
2. `git merge --no-edit <nhánh>`; xem `git diff --name-only --diff-filter=U`.
3. Giải xung đột:
   - **i18n** (`packages/i18n/src/locales/*.json`): luôn là "hai bên cùng thêm khoá".
     `python .claude/skills/blueoffice-merge-agent-branch/scripts/resolve_i18n.py` — thử các cách nối, chỉ ghi khi JSON hợp lệ
     và **không trùng khoá**. Sau đó `pnpm exec prettier --write` hai tệp.
   - **Giữ cả hai phía** (import, biến thể enum, trường struct, dòng danh sách): xem trước bằng
     `python …/scripts/keep_both.py --show <tệp…>`, rồi chạy không có `--show`. Kiểm tra chỗ nối: hunk hay bị cắt giữa khối
     → phải tự thêm `}` / `)` đóng cho phía "ours" (đã gặp ở `corpus.rs`, `slide_edit/mod.rs`, `cli.rs`).
   - **Xung đột ngữ nghĩa** (hai nhánh sửa cùng một hàm): đọc cả hai phía, không dùng script.
4. Việc phải kiểm sau khi gộp (đều đã từng gây đỏ):
   - `cargo fmt --all` và prettier cho tệp vừa nối.
   - `pnpm install` nếu nhánh thêm devDependency (lỗi `Cannot find type definition file`).
   - **Số ADR trùng**: mỗi nhánh tự lấy số kế tiếp → `git mv` sang số mới, sửa mọi tham chiếu (`CLAUDE.md`, docs, chú thích mã).
   - **EF migrations** khi hai nhánh cùng đổi model: `dotnet ef migrations has-pending-model-changes --context <Sqlite|Postgres>BlueOfficeDbContext`
     (trong `apps/server/BlueOffice.Infrastructure`); test migration cũ phải `DROP TABLE` cả bảng mới của hai nhánh.
   - **Snapshot insta của corpus mới**: `cargo test -p bo-engine --test slides`; nếu thiếu thì `INSTA_UPDATE=always`, và
     `git status` chỉ được có tệp `.snap` **mới**.
   - `git commit --no-edit` để kết thúc merge.
5. Regression đầy đủ (nền, ~1–1,5 giờ): `bash .claude/skills/blueoffice-merge-agent-branch/scripts/regress.sh` từ gốc repo;
   đọc `$TEMP/bo-regress/summary.txt` và log của bước lỗi.
6. E2E lỗi: chạy riêng spec đó (`cd tests/e2e && pnpm exec playwright test <spec> --project=chromium --workers=1 --repeat-each=3`).
   Qua khi chạy riêng nhiều lần = nghi tải máy; lỗi cả khi chạy riêng = lỗi thật, điều tra (xem Pitfalls).
7. Xanh → cập nhật `docs/parity/BACKLOG.md` (`[x]`/`[p]` + phần thiếu), `MATRIX.md`, commit, `git push origin main`.
8. Dọn: xoá worktree bằng Node (`fs.rmSync(path, {recursive:true, force:true})` — `git worktree remove` hỏng vì đường dẫn dài),
   `git worktree unlock`, `git worktree prune`, `git branch -D`; xoá thư mục build của agent trong `D:/bo-targets/`.

## Template / Reference
- `scripts/resolve_i18n.py` — nối hai phía của xung đột i18n, kiểm JSON + khoá trùng.
- `scripts/keep_both.py [--show] <tệp…>` — giữ cả hai phía của mọi hunk trong tệp.
- `scripts/regress.sh` — regression đầy đủ, ghi `summary.txt`.

## Lưu ý / Pitfalls
- **Ổ D đầy rất nhanh**: `df -h /d` trước mỗi đợt; xoá `*.pdb`, `incremental`, thư mục `D:/bo-targets/<tên>` của agent đã xong;
  `target/debug` của checkout chính có thể tới 50+ GB (xoá được khi không có cargo đang chạy).
- Không chạy hai regression cùng lúc; `TaskStop` một lệnh nền không giết tiến trình con — kiểm tra trước khi chạy lại.
- "Chập chờn" lặp lại ≥ 2 lần thường là lỗi thật. Đã gặp: lệnh chỉ-đọc thiếu trong `packages/hosted/src/readonly.ts`
  (phiên bị đánh dấu bẩn sau khi lưu); thao tác nhiều lệnh không bọc `DocController.hold()` (Lưu chen vào giữa);
  helper E2E đọc `boundingBox()` ngay sau khi phần tử được vẽ lại.
- Test hỏng vì kỳ vọng liệt kê cứng (danh sách định dạng "Lưu thành", nhóm ribbon) sau khi thêm tính năng: sửa kỳ vọng và ghi
  lý do trong commit; mọi trường hợp khác sửa mã, không sửa test.
- Hai nhánh cùng thêm export vào `packages/platform/src/index.ts` → chạy `pnpm typecheck` (đã gặp trùng tên).
