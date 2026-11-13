# BlueOffice — Đưa giao diện & đồ họa lên ngang chất lượng Microsoft Office

> Prompt độc lập. Áp dụng cho bản desktop (Windows trước, sau đó macOS/Linux) và bản web.

## 0. Mục tiêu & ranh giới

Người dùng đặt BlueOffice cạnh Microsoft Office sẽ thấy chất lượng hình ảnh ngang nhau: chữ sắc nét, hình vẽ mịn, ảnh nét, giao diện chuyên nghiệp và thao tác mượt. Không sao chép icon, logo, hình minh họa, ảnh màn hình, mẫu hoặc gallery Microsoft Office; không dùng bảng màu nhận diện Word/Excel/PowerPoint hay tạo tổng thể gây nhầm lẫn. Giữ tên, logo, icon app và màu BlueOffice: BlueDoc chàm, BlueSheet xanh ngọc, BlueSlide hồng san hô. Được dùng Fluent 2 công khai, Fluent UI React và Fluent UI System Icons (MIT), cùng bố cục ribbon theo BlueOffice-UI-PROMPT.md.

Ảnh Microsoft Office do người dùng cung cấp trong `docs/visual/reference/` chỉ dùng so chất lượng hiển thị và ngắt dòng/trang cùng tài liệu, không làm mẫu giao diện. Nếu thiếu ảnh, ghi vào `NEEDS_HUMAN.md` và tiếp tục các mục khác.

## 1. Chữ — ưu tiên số 1

- Desktop Windows: DirectWrite (`windows`) với hinting và khử răng cưa native; macOS: Core Text; Linux: FreeType + HarfBuzz với hinting hệ thống. Web giữ Canvas/WebGL, render đúng devicePixelRatio. Thực hiện theo ADR-0017.
- Render đúng 100/125/150/175/200% và khi kéo cửa sổ giữa hai màn hình khác tỷ lệ.
- Font UI: Segoe UI Variable trên Windows 11, San Francisco trên macOS, font UI dự án trên Linux/web. Không đóng gói font hệ thống.
- Font tài liệu: dùng font có sẵn trên máy (Calibri/Cambria/Times New Roman nếu có); thiếu thì fallback metric-compatible, cảnh báo nhẹ.
- Kerning, ligature, dấu tiếng Việt, baseline và chữ đậm/nghiêng tổng hợp phải đúng và đẹp.

## 2. Đồ họa trong tài liệu

- Surface GPU native theo ADR-0017; CPU fallback chất lượng cao (tiny-skia hoặc tương đương MIT/Apache).
- Khử răng cưa đường cong/chéo; đường mảnh không mất hoặc nhòe, đường 1px căn pixel; nét đứt, mũi tên, nối góc đúng DrawingML.
- Ảnh thu nhỏ bằng mipmap và lọc Lanczos/bicubic; zoom dùng ảnh gốc; đọc ICC, hiển thị sRGB đúng, PNG trong suốt không quầng.
- Gradient có dithering; bóng/glow/viền mềm/phản chiếu dùng Gaussian blur đúng thông số, alpha và blend đúng; 3D dựng GPU với ánh sáng mềm.
- Biểu đồ mịn, nhãn sắc, lưới mảnh, màu BlueOffice; trang giấy trắng bóng mềm, viền bảng và lưới bảng tính đều, vùng chọn không nhấp nháy.

## 3. Giao diện (khung, ribbon, hộp thoại)

- Chuyển component sang Fluent UI React v9 và `@fluentui/react-icons` (MIT); theme BlueOffice riêng theo app, light/dark/high-contrast; cập nhật THIRD_PARTY_NOTICES.md.
- Fluent 2: lưới 4px, thang chữ, góc 4/8px, elevation, motion 100–200 ms.
- Windows 11: Mica cho thanh tiêu đề và dải tab, ribbon phẳng sáng, thanh tiêu đề liền khối, nút cửa sổ có snap layouts; tôn trọng sáng/tối và tùy chọn màu nhấn hệ thống.
- Viền và đường ngăn 1px căn pixel vật lý ở mọi DPI; icon chỉ 16/20/24/32. Nút, nhãn, khoảng cách, màu và trạng thái tương tác nhất quán.
- Giữ icon app/logo; icon chuyên dụng ngoài Fluent tự thiết kế và ghi nguồn trong `packages/ui/icons/SOURCES.md`.

## 4. Mượt mà

- Gõ phím → chữ hiện < 16 ms (p95 < 20 ms): local echo rồi lõi dàn trang thay thế không nhấp nháy, đúng bộ gõ tiếng Việt; một IPC mỗi phím, chỉ gửi phần thay đổi.
- Cuộn, zoom, kéo chọn ≥ 60 FPS kể cả tài liệu nhiều ảnh, hình, biểu đồ.
- Khung app < 500 ms, editor sẵn sàng < 1,5 s; menu/gallery/ngăn tác vụ mượt, tôn trọng reduced-motion.

## 5. Vòng kiểm tra thị giác bắt buộc

1. Bộ mẫu `tests/corpus/visual/`: văn bản hành chính/nhiều kiểu chữ, bảng phức tạp, hình cong/đường mảnh, ảnh lớn/PNG trong suốt, gradient/bóng đổ, biểu đồ, slide 3D, bảng tính nhiều định dạng.
2. Chụp khung ba app, từng tab, dialog, menu, tooltip; vùng tài liệu zoom 50/100/200/400%, DPI 100/125/150/200%, light/dark. Lưu `docs/visual/screens/`.
3. Rà từng ảnh: chữ nhòe/nhạt, viền mờ/dày, răng cưa, sọc gradient, mất đường mảnh, nhiễu/quầng ảnh, màu ngoài token, lệch lưới, icon sai cỡ, khoảng cách không đều, cắt chữ. Ghi `docs/visual/ISSUES.md`, sửa và chụp lại đến khi sạch.
4. Khi có ảnh tham chiếu: so cùng tài liệu/zoom/DPI về cạnh chữ, ngắt dòng/trang, độ mịn; sửa chất lượng hiển thị, không sao chép giao diện.
5. Visual regression toàn bộ ảnh trong CI; benchmark không giảm.

## 6. Thứ tự làm

1. Đo/chụp baseline. 2. DPI và renderer chữ native. 3. GPU và đồ họa tài liệu. 4. Fluent/theme/Mica. 5. Pixel và tính nhất quán. 6. Độ mượt. 7. Rà thị giác đến khi sạch.

Mỗi bước: làm → chụp/đo → ghi trước/sau vào `docs/visual/REPORT.md` → commit & push → bước tiếp. Không hỏi giữa chừng; quyết định chưa rõ ghi ADR; việc cần người dùng ghi `NEEDS_HUMAN.md` và tiếp tục.

## 7. Điều kiện kết thúc

- Checklist mục 5 sạch trên mọi ảnh/DPI/light/dark.
- Mọi chỉ số mục 4 đạt trên cùng bản release, ≥ 10 lượt đo.
- Unit, E2E web/desktop/cộng tác, visual regression và CI xanh.
- `docs/visual/REPORT.md` có ảnh trước/sau từng màn hình và số liệu; `NEEDS_HUMAN.md` liệt kê việc cần người dùng.
