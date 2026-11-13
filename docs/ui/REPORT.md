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
| Trang tài liệu | Đã sửa dải đen ở đầu trang: con trỏ có `position: static` và rộng bằng trang do CSS lớp phủ tạo động không được áp dụng trong desktop. Đưa toàn bộ CSS lớp phủ vào stylesheet đóng gói; [ảnh sau sửa](screens/fix-caret-125/doc-light-home.png) trên release Windows 125% không còn dải đen. Đã thêm kiểm thử kích thước/vị trí con trỏ vào desktop E2E. |
| Khoảng cách và cắt chữ | Một số nhãn nhóm ribbon thu gọn như “Giọn…”; chưa hoàn thành rà tất cả tab và sửa. |
| Dialog, menu, tooltip | Đã chụp 10 ảnh đại diện light/dark ở 125%: [manifest](screens/dialogs-125/manifest.json), ví dụ [BlueDoc](screens/dialogs-125/doc-light-dialog-font-effects.png), [BlueSheet](screens/dialogs-125/sheet-light-dialog-chart.png), [BlueSlide](screens/dialogs-125/slide-light-menu-table.png). Bộ mới đã chụp các lệnh đại diện bị thu gọn; tooltip và các dialog còn lại chưa được bao phủ. |
| DirectWrite và renderer GPU native | Chưa triển khai; xem [ADR-0017](../adr/0017-desktop-document-rendering.md). |
| Visual regression CI | Đã thêm lệnh chụp/so sánh tab, màn hình Tệp và một số dialog/menu; chỉ so khi có artifact baseline CI cùng viewport/DPR. Lần đầu hoặc thiếu artifact chỉ báo cáo; nhiều dialog/menu, tooltip và DPI khác chưa được bao phủ. |

Script [capture-desktop-ui.mjs](../../scripts/capture-desktop-ui.mjs) xác nhận tab được chọn
trước mỗi ảnh và ghi DPR vào manifest. [compare-desktop-ui.mjs](../../scripts/compare-desktop-ui.mjs)
từ chối so sánh khi kích thước hoặc DPR khác. Không dùng ảnh mô phỏng làm bằng chứng cho DPI thật.

## Kiểm tra tiếp theo

- Màn hình Tệp BlueSheet: nền hover được làm tối để chữ trắng đạt tương phản; bài kiểm tra
  accessibility BlueSheet đã qua trên bản web build lại. Test chuyển theme chờ trạng thái
  sáng ổn định trước khi đổi lần tiếp theo.
- Ảnh sửa con trỏ được chụp với executable SHA-256
  `4629ac6dd15d4ae8dbe7ea93348b04522ec5860de9baeb0bf3f31aec1331ba5c`.
  Bộ ảnh này mới có BlueDoc light 125%; không thay thế bộ ảnh đủ màn hình và DPI.

Script chụp có thêm các dialog/menu đại diện và báo cảnh báo khi không mở được điều khiển.
Ở bộ ban đầu, Giãn dòng và Thay thế của BlueDoc cùng Định dạng ô của BlueSheet bị thu gọn;
bộ mới đã mở được chúng qua nhóm ribbon và phím tắt. Hộp thoại hiệu ứng/giãn cách ký tự
không đại diện toàn bộ hộp thoại phông chữ.

## Ảnh bản release đo 10 lượt

[Bộ 80 ảnh](screens/after-immediate-125/manifest.json) được chụp ở Windows 125% trên
cùng SHA-256 `75e9508742bbb0f8fdaab4de7a2f8b7bc0d9e5dca759697d118820b222acda56`
như phép đo hiệu năng: ba app, light/dark, các tab và một số dialog/menu. Trước mỗi màn
hình, script đặt lại lựa chọn trong profile WebDriver tạm để trạng thái ngăn của app trước
không lọt sang ảnh sau. Ảnh BlueSlide dark đã được chụp lại sau khi phát hiện ngăn hiệu ứng
còn mở từ phiên trước. Chưa thể kết luận checklist sạch: vẫn thiếu nhiều dialog/menu, tooltip, tài liệu chữ mẫu,
các DPI khác và so sánh màn hình thứ hai.
