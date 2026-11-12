# Báo cáo thị giác desktop BlueOffice

Trạng thái: **đang thực hiện, checklist chưa sạch**. Mục tiêu, phạm vi và các ngoại lệ được
cập nhật tại đây; số đo hiệu năng nằm trong [RESULTS.md](../perf/RESULTS.md).

## Nền tảng đang chuyển đổi

- Đã thêm Fluent UI React v9 và Fluent System Icons, dùng giấy phép MIT; xem
  [THIRD_PARTY_NOTICES.md](../../THIRD_PARTY_NOTICES.md). Logo và icon ứng dụng BlueOffice giữ nguyên.
- Đã tạo theme Fluent riêng theo màu nhấn của từng ứng dụng cho light/dark và theme tương phản
  cao; giữ font/token BlueOffice ở gốc. Thanh truy cập nhanh đã dùng Fluent Button và ba icon
  Fluent. Ribbon, dialog, menu và các điều khiển khác **chưa** được chuyển đầy đủ.
- [Ảnh trước, 125%](screens/before-125/manifest.json): ba app, light/dark, các tab ribbon và
  màn hình Tệp trên WebView2 thật. DPR trong WebView là 1,375 vì thang Windows 125% và zoom
  WebView. Các ảnh dùng tài liệu mới trống; chưa phải bộ ảnh kiểm tra chữ tài liệu mẫu.
- Bộ [ảnh sau, 125%](screens/after-125/manifest.json) đã chụp lại với thư mục khôi phục
  cách ly. Ví dụ: BlueDoc [trước](screens/before-125/doc-light-home.png) /
  [sau](screens/after-125/doc-light-home.png), BlueSheet
  [trước](screens/before-125/sheet-light-home.png) / [sau](screens/after-125/sheet-light-home.png),
  BlueSlide [trước](screens/before-125/slide-light-home.png) /
  [sau](screens/after-125/slide-light-home.png). Ảnh sau ban đầu có banner khôi phục không được sử dụng.

## Rà soát ban đầu

| Kiểm tra | Kết quả hiện tại |
|---|---|
| Nền, font và màu thương hiệu | Đã sửa Fluent không ghi đè font web và nền BlueOffice; đã chụp lại trên bản desktop mới; cần rà đầy đủ các màn hình. |
| Viền và độ sắc ở 100/125/150/200% | Chưa xác nhận; hiện mới chụp 125%. Chưa có màn hình thứ hai để thử kéo qua DPI khác. |
| Trang tài liệu | Dải đen ở đầu trang BlueDoc cũng nhìn thấy trực tiếp trên cửa sổ Windows, không chỉ trong ảnh WebDriver. Pixel đọc trực tiếp từ canvas là trắng; nguyên nhân hiển thị chưa xác định và lỗi chưa được sửa. |
| Khoảng cách và cắt chữ | Một số nhãn nhóm ribbon thu gọn như “Giọn…”; chưa hoàn thành rà tất cả tab và sửa. |
| Dialog, menu, tooltip | Chưa có bộ ảnh/kiểm tra bao phủ. |
| DirectWrite và renderer GPU native | Chưa triển khai; xem [ADR-0017](../adr/0017-desktop-document-rendering.md). |
| Visual regression CI | Đã thêm lệnh chụp/so sánh tab và màn hình Tệp; chỉ so khi có artifact baseline CI cùng viewport/DPR. Lần đầu hoặc thiếu artifact chỉ báo cáo; dialog/menu/tooltip và DPI khác chưa được bao phủ. |

Script [capture-desktop-ui.mjs](../../scripts/capture-desktop-ui.mjs) xác nhận tab được chọn
trước mỗi ảnh và ghi DPR vào manifest. [compare-desktop-ui.mjs](../../scripts/compare-desktop-ui.mjs)
từ chối so sánh khi kích thước hoặc DPR khác. Không dùng ảnh mô phỏng làm bằng chứng cho DPI thật.
