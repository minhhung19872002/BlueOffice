# Vấn đề chất lượng hình ảnh và phạm vi chưa kiểm chứng

| Hạng mục | Trạng thái / bằng chứng |
|---|---|
| Renderer chữ và surface native | Chưa triển khai DirectWrite/GPU; [ADR-0017](../adr/0017-desktop-document-rendering.md) mô tả các ràng buộc cần kiểm chứng. |
| DPI và độ sắc | Mới có ảnh Windows 125%; chưa kiểm tra thực tế 100/150/175/200% và chuyển màn hình. |
| Nội dung tài liệu | Chưa có bộ ảnh tài liệu đầy đủ ở 50/100/200/400%; các ảnh hiện có chủ yếu là tài liệu trống. |
| Đồ họa, hiệu ứng, màu | Chưa kiểm tra đầy đủ ICC, lọc ảnh, gradient, blur, 3D và DrawingML theo checklist. |
| Giao diện | Fluent/theme đã có một phần; ribbon, menu, dialog và icon chưa chuyển đầy đủ. Nhiều dialog/menu và tooltip chưa có ảnh. |
| Hiệu năng | Mốc 10 lượt: shell và độ trễ gõ chưa đạt; xem [REPORT.md](REPORT.md). |
| CI | So ảnh desktop hiện chỉ cho bộ đại diện và có thể bỏ so sánh khi thiếu artifact baseline; chưa bao phủ ma trận yêu cầu. |
| Đối chiếu Office | Chưa có ảnh người dùng cung cấp; xem [NEEDS_HUMAN.md](../../NEEDS_HUMAN.md). |
