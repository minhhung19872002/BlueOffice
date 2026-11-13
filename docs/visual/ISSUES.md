# Vấn đề chất lượng hình ảnh và phạm vi chưa kiểm chứng

| Hạng mục | Trạng thái / bằng chứng |
|---|---|
| Renderer chữ và surface native | Chưa triển khai DirectWrite/GPU; [ADR-0017](../adr/0017-desktop-document-rendering.md) mô tả các ràng buộc cần kiểm chứng. |
| Font khi sửa ô bảng tính | `cellFontFor` chọn font đóng gói cho ô đang sửa; cần đối chiếu font thực của engine khi có font hệ thống để tránh thay đổi hình chữ lúc vào/ra chế độ sửa. |
| Font giao diện Linux | Token desktop Linux hiện ưu tiên font hệ thống; cần chuyển sang font UI dự án theo yêu cầu mới và kiểm tra dấu tiếng Việt. |
| DPI và độ sắc | Mới có ảnh Windows 125%; chưa kiểm tra thực tế 100/150/175/200% và chuyển màn hình. |
| Nội dung tài liệu | Đã có một mẫu BlueDoc light/dark ở 50/100/200/400% trên Windows 125%; còn thiếu các mẫu, ứng dụng và DPI khác. |
| Đồ họa, hiệu ứng, màu | Chưa kiểm tra đầy đủ ICC, lọc ảnh, gradient, blur, 3D và DrawingML theo checklist. |
| Giao diện | Fluent/theme đã có một phần; ribbon, menu, dialog và icon chưa chuyển đầy đủ. Nhiều dialog/menu và tooltip chưa có ảnh. |
| Nhãn ribbon | Ảnh mốc BlueDoc 125% cắt nhãn nhóm thành “Giọn...”; ảnh desktop sau hiển thị “Giọng nói”. Cần rà các tab, app, theme và DPI còn lại. |
| So ảnh trước/sau | Một số ảnh tài liệu ở 100/200% vượt ngưỡng 0,1% với bản mới; bản mới có nhiều thay đổi cùng lúc nên chưa quy riêng nguyên nhân hoặc xác nhận toàn bộ khác biệt là mong muốn. |
| Hiệu năng | Mốc 10 lượt: shell và độ trễ gõ chưa đạt; xem [REPORT.md](REPORT.md). |
| CI | Đã thêm ca tài liệu BlueDoc hai theme, bốn zoom vào workflow, chưa có kết quả CI xác nhận. So ảnh desktop có thể bỏ qua khi thiếu artifact baseline và chưa bao phủ ma trận yêu cầu. |
| Đối chiếu Office | Chưa có ảnh người dùng cung cấp; xem [NEEDS_HUMAN.md](../../NEEDS_HUMAN.md). |
