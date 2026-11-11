# Bàn giao phiên (cập nhật 2026-11-11)

Trạng thái vòng lặp Parity (`BlueOffice-PARITY-PROMPT.md`). Phiên sau đọc file này trước, rồi `BACKLOG.md`.
Số liệu: 79 `[x]`, 89 `[p]`, 2 `[~]`/`[ ]` (CORE-MISC-001 mục gom của rà soát, CORE-MISC-006 phần test chập chờn còn lại).
Mọi hạng mục gốc đã được làm ít nhất ở mức `[p]`; việc còn lại là **vòng hoàn thiện các mục `[p]`** và **vòng rà soát cuối**.

## Đã làm trong phiên 2026-11-10 → 11
- Gộp các nhánh dở của phiên trước (SLIDE-ACTION-001, CORE-CITE-001, CORE-FMT-003, DOC-BB-001, CORE-UI-007).
- Làm hết các mục `[ ]` P2: SHEET-FN-CUBE, CORE-SCRIPT-003/004, CORE-VBA-002, CORE-API-001, CORE-AI-001, CORE-ADDIN-001,
  CORE-ICON-001, CORE-LANG-004, CORE-THES-001, CORE-INK-002, CORE-DESIGN-001, CORE-FMT-004, CORE-UI-008, CORE-MATH-010
  (kèm giữ `w:rPr`/`m:ctrlPr`, track changes), CORE-CHART-006, CORE-PIC-002, SLIDE-ALBUM-001, SLIDE-REMOTE-001,
  CORE-MISC-002/003/004, `docs/user-guide/`.
- Vòng hoàn thiện `[p]` P0 đã xong: SLIDE-ANIM-001/003/004 → `[x]`; CORE-CHART-002/004 bổ sung (VHLC/VOHLC, of-pie, chartex ở Slide).
- Rà soát định kỳ lần 6 (fidelity: mở + lưu 100 %; lỗi validator còn lại đều đã biết).
- Rà soát cuối (prompt §9) đã làm: WCAG 2.2 AA (axe + bàn phím + lớp văn bản cho trình đọc màn hình), bộ 49 tệp độc hại
  (`bo-cli sample-hostile`), fuzz soak 1 giờ mỗi nhóm parser (`BO_FUZZ_SECONDS=3600`; tìm và sửa 1 panic + 1 treo dàn trang),
  CI xanh 11/11 job trên PR #1 (đã gộp vào `main`).

## Đang chạy khi viết file này (kiểm tra `git worktree list`, `git branch --no-merged main`)
| Việc | Nhánh / worktree | Ghi chú |
|---|---|---|
| Biểu đồ trong BlueDoc (CORE-CHART-007/004) + thang giá trục phụ | `worktree-agent-ab119527686d455f1` | mã xong, đang chạy test cuối |
| SmartArt (CORE-SMART-003/004) | worktree agent mới, build `D:/bo-targets/a1` | vòng hoàn thiện P0 |
| Pivot / PivotChart / Slicer / Timeline | worktree agent mới, build `D:/bo-targets/v2` | vòng hoàn thiện P0 |

## Việc tiếp theo (theo thứ tự)
1. Gộp các nhánh trên khi xong (skill `blueoffice-merge-agent-branch`), regression, push, kiểm CI của `main` (skill `ci-run-triage`).
2. Các lô P0 `[p]` còn lại: DOC-FIELD-001/002, DOC-CC-001, SHEET-FN-000 / SHEET-FN-LOOKUP / SHEET-FORMULA-001
   (liên kết workbook ngoài, `@`, trim ref, CSE), rồi 56 mục P1 và 19 mục P2 (brief chung: đọc mục trong BACKLOG, làm hết phần
   kỹ thuật làm được, phần cần con người để lại kèm lý do).
3. Chạy lại fuzz soak 1 giờ cho nhóm docx và nhóm bảng cũ (hai nhóm này dừng sớm vì lỗi đã sửa):
   `BO_FUZZ_SECONDS=3600 cargo test --release -p bo-engine --test fuzz_smoke mutated_docx mutated_tabular -- --nocapture`.
4. Build desktop + E2E desktop local (CI đã chạy cả ba nền tảng); cập nhật `docs/user-guide/` theo tính năng mới.
5. Khi mọi mục `[p]` chỉ còn phần cần con người: đối chiếu từng dòng prompt §4–7 với MATRIX, viết `docs/parity/FINAL_REPORT.md`.

## Cần con người (xem `NEEDS_HUMAN.md`)
Ảnh mốc từ phần mềm tham chiếu; dữ liệu ranh giới 34 tỉnh/thành; model dịch / giọng nói / chữ viết tay / phân đoạn ảnh cục bộ;
từ đồng nghĩa tiếng Việt; bộ mã hoá video cho máy chủ; nghe thử bằng trình đọc màn hình; màu và logo thương hiệu.

## Cách làm (đã đóng gói thành skill trong `.claude/skills/`)
- Mỗi agent một worktree + `CARGO_TARGET_DIR=D:/bo-targets/<tên>` riêng; tối đa 3–4 agent cùng lúc (máy tải nặng làm test chập chờn).
- **Ổ D đầy rất nhanh**: xoá `*.pdb`, `incremental`, thư mục build của agent đã xong, `target/debug` của checkout chính khi không dùng.
- Agent bị dừng vì hết hạn mức: giao agent mới làm tiếp trong đúng worktree cũ (commit WIP trước, merge `main`, hoàn tất).
- Hai nhánh cùng thêm ADR sẽ trùng số (đã có tới ADR-0015); hai nhánh cùng đổi model EF → kiểm `has-pending-model-changes`.
