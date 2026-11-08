# Hướng dẫn tạo ảnh mốc từ phần mềm tham chiếu

BlueOffice so sánh hiển thị với phần mềm office thương mại bằng **ảnh do người dùng tự xuất** (clean-room: đội phát
triển không tự tạo ảnh từ phần mềm có bản quyền).

## Cần làm
1. Mở từng file trong `tests/corpus/doc/*.docx`, `tests/corpus/sheet/*.xlsx`, `tests/corpus/slide/*.pptx` bằng phần mềm
   tham chiếu (bản có bản quyền của bạn).
2. Xuất PDF (Lưu thành → PDF, chất lượng tiêu chuẩn, không tối ưu cho web).
3. Chuyển từng trang PDF thành PNG 96 dpi (ví dụ `pdftoppm -r 96 -png file.pdf page`) — hoặc xuất ảnh trực tiếp nếu
   phần mềm cho phép (slide: Xuất → PNG).
4. Đặt ảnh vào `tests/fidelity/reference/<app>/<tên file>/page-N.png` với `<app>` = `doc` | `sheet` | `slide`,
   `<tên file>` là tên file corpus (kể cả phần mở rộng), `N` đánh số từ 1.
   Ví dụ: `tests/fidelity/reference/doc/phase1-sample.docx/page-1.png`.
5. Commit thư mục `tests/fidelity/reference/` (hoặc gửi cho nhóm phát triển).

## Sau khi có ảnh
`pnpm fidelity` tự so sánh (ngưỡng mỗi file trong `tests/fidelity/thresholds.json` nếu có) và ghi kết quả vào
`docs/fidelity-visual.md`; mỗi file lệch vượt ngưỡng sinh một hạng mục P0 trong `docs/parity/BACKLOG.md`.

## Lưu ý
- Không đưa tài liệu chứa dữ liệu thật của khách hàng vào corpus.
- Dùng font mặc định của máy (không cài thêm font) để kết quả lặp lại được; ghi phiên bản phần mềm vào
  `tests/fidelity/reference/README.md`.
