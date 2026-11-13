# Vấn đề chất lượng hình ảnh và phạm vi chưa kiểm chứng

| Hạng mục | Trạng thái / bằng chứng |
|---|---|
| Renderer chữ và surface native | Chưa triển khai DirectWrite/GPU; [ADR-0017](../adr/0017-desktop-document-rendering.md) mô tả các ràng buộc cần kiểm chứng. |
| Font khi sửa ô bảng tính | `cellFontFor` chọn font đóng gói cho ô đang sửa; cần đối chiếu font thực của engine khi có font hệ thống để tránh thay đổi hình chữ lúc vào/ra chế độ sửa. |
| Dấu kết hợp tiếng Việt | Ca `a` + dấu chấm dưới với Caladea từng tạo glyph thiếu; đã sửa fallback theo cụm, kiểm thử font và dàn trang qua. Ảnh hồi quy của một tài liệu hiện có gần như không đổi nhưng không chứa ca này. Mẫu DOCX độc lập và ảnh trước/sau còn thiếu; môi trường hiện không có LibreOffice để hoàn tất bước render kiểm tra tài liệu. Benchmark thông thường 10 lượt không báo suy giảm vượt ngưỡng, nhưng chưa đo riêng đoạn dấu tổ hợp. |
| Font giao diện Linux | Token đã đổi sang font UI dự án; kiểm thử token qua, nhưng chưa rà ảnh Linux và dấu tiếng Việt thực tế. |
| DPI và độ sắc | Mới có ảnh Windows 125%; chưa kiểm tra thực tế 100/150/175/200% và chuyển màn hình. |
| Nội dung tài liệu | Đã có một mẫu BlueDoc light/dark ở 50/100/200/400% trên Windows 125%; còn thiếu các mẫu, ứng dụng và DPI khác. |
| Đồ họa, hiệu ứng, màu | Chưa kiểm tra đầy đủ ICC, lọc ảnh, gradient, blur, 3D và DrawingML theo checklist. |
| Giao diện | Fluent/theme đã có một phần; ribbon, menu, dialog và icon chưa chuyển đầy đủ. Nhiều dialog/menu và tooltip chưa có ảnh. |
| Nhãn ribbon | Ảnh mốc BlueDoc 125% cắt nhãn nhóm thành “Giọn...”; ảnh desktop sau hiển thị “Giọng nói”. Cần rà các tab, app, theme và DPI còn lại. |
| So ảnh trước/sau | Một số ảnh tài liệu ở 100/200% vượt ngưỡng 0,1% với bản mới; bản mới có nhiều thay đổi cùng lúc nên chưa quy riêng nguyên nhân hoặc xác nhận toàn bộ khác biệt là mong muốn. |
| Lớp phủ điều khiển nội dung | CI web thất bại hai lần khi lấy bounding box; trace cho thấy khung có mặt trước/sau và mã tạo lại lớp phủ khi lựa chọn đổi, phù hợp với khả năng node bị thay giữa hai bước. Test đã chờ lấy được khung và qua 3 lần trên Windows; chưa xác nhận lại CI Linux, còn cần kiểm tra nhấp nháy cho người dùng. |
| Hiệu năng | Mốc 10 lượt: shell và độ trễ gõ chưa đạt; xem [REPORT.md](REPORT.md). |
| CI | Đã thêm ca tài liệu BlueDoc hai theme, bốn zoom vào workflow, chưa có kết quả CI xác nhận. So ảnh desktop có thể bỏ qua khi thiếu artifact baseline và chưa bao phủ ma trận yêu cầu. |
| Web E2E | CI commit `c3573c7d` thất bại ca khung điều khiển nội dung; hai ca phương trình và từ đồng nghĩa chỉ qua khi chạy lại. Đã sửa cách chờ cho hai ca khung/từ đồng nghĩa và thử cục bộ, chưa xác nhận trên CI. Ca phương trình tái hiện 3/3 cục bộ; trace cho thấy chuỗi `+z_1` xuất hiện sau nội dung đoạn, cần điều tra vì sao thao tác rời vùng phương trình. |
| Đối chiếu Office | Chưa có ảnh người dùng cung cấp; xem [NEEDS_HUMAN.md](../../NEEDS_HUMAN.md). |
