# Fidelity

- `reference/<doc|sheet|slide>/<corpus-file>.page-<n>.png` — ảnh tham chiếu **do người dùng cung cấp** (chụp/xuất từ
  phần mềm có bản quyền của họ, không đưa vào repo nếu không được phép). `scripts/visual-diff.mjs` render cùng trang
  bằng `bo-cli render` ở đúng độ phân giải của ảnh và so pixel (pixelmatch); ngưỡng theo file ở `thresholds.json`
  (`"doc/<ảnh>.png": 0.03`, mặc định 2 % — tiêu chí Phase 2).
- `baseline/` — ảnh do BlueOffice tự render (trang 1 mỗi file corpus, scale 0,5) để phát hiện hồi quy hiển thị (ngưỡng
  0,1 %). Sau khi thay đổi render có chủ đích và đã xem lại: `pnpm fidelity:update-baseline`.
- `known-issues.txt` — lỗi OpenXmlValidator được chấp nhận (kèm lý do).
- `history.csv` — kết quả round-trip/validator theo thời gian.
- `output/` — kết quả của CI (bản round-trip, ảnh diff); không commit.

Báo cáo: `docs/fidelity-report.md` (round-trip + validator), `docs/fidelity-visual.md` (visual diff). Chạy: `pnpm fidelity`.
