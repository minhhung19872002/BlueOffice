# BlueOffice — Prompt thiết kế & xây dựng giao diện

> Prompt độc lập, chỉ về giao diện (UI/UX) của BlueDoc, BlueSheet, BlueSlide và màn hình khởi động chung. Đặt ở gốc repo, đọc toàn bộ trước khi làm.

---

## 0. Mục tiêu

Người đã quen dùng bộ office phổ biến (Microsoft Office, WPS Office) mở BlueOffice ra là **biết ngay lệnh nằm ở đâu, dùng được ngay không cần học**: cùng kiểu ribbon chia tab, cùng logic nhóm lệnh, cùng phím tắt, cùng cách thao tác.

Đồng thời BlueOffice có **bản sắc thị giác riêng của Bluestar**: icon, logo, màu, font giao diện, hình minh họa, mẫu, văn bản giao diện đều do BlueOffice tự thiết kế. Nhìn vào không ai nhầm là sản phẩm của Microsoft hay Kingsoft.

Tóm gọn: **quen tay như Office, nhìn ra là BlueOffice.**

---

## 1. Ranh giới pháp lý (BẮT BUỘC)

**Được làm (quy ước chức năng, phổ biến trong ngành):**
- Ribbon chia tab, tab ngữ cảnh, ribbon thu gọn/đơn giản.
- Thứ tự tab và cách nhóm lệnh theo logic công việc quen thuộc.
- Phím tắt tiêu chuẩn, phím tắt Alt (KeyTips), menu chuột phải, thanh định dạng nổi.
- Màn hình File toàn trang, thanh truy cập nhanh, thanh trạng thái, ngăn tác vụ bên phải, thước kẻ.
- Phong cách thiết kế phẳng, hiện đại.

**Cấm:**
- Dùng, trích xuất, chụp lại, vẽ đè (trace) hoặc vẽ lại sát icon, logo, hình minh họa, ảnh nền, con trỏ, âm thanh của Microsoft Office, WPS Office hay bất kỳ sản phẩm nào khác.
- Dùng tên/nhãn hiệu của bên khác trong giao diện (trừ câu mô tả tương thích định dạng, ví dụ "Lưu dưới dạng tài liệu Word (.docx)").
- Dùng bảng màu nhận diện đặc trưng của từng ứng dụng bên khác cho app tương ứng của BlueOffice (xem mục 3.2).
- Sao chép văn bản tooltip, mô tả lệnh, nội dung trợ giúp, tên style/theme/mẫu, gallery có sẵn của bên khác. Tên lệnh thông dụng một hai chữ ("In đậm", "Chèn bảng", "Dán") là từ ngữ thông thường, dùng được; câu mô tả phải tự viết.
- Thiết kế tổng thể gây nhầm lẫn là sản phẩm của Microsoft/Kingsoft hoặc có liên kết với họ.

Mọi tài nguyên đồ họa phải do dự án tự tạo hoặc lấy từ nguồn license MIT/Apache/ISC/OFL/CC0, ghi trong `THIRD_PARTY_NOTICES.md`. Nếu không chắc một chi tiết có vượt ranh giới không → chọn phương án khác biệt hơn và ghi lý do trong `docs/adr/`.

---

## 2. Nguyên tắc thiết kế

1. **Quen thuộc trước, sáng tạo sau**: vị trí lệnh và luồng thao tác theo thói quen người dùng office; chỉ đổi khi có lý do rõ ràng về khả dụng.
2. **Tài liệu là trung tâm**: chrome giao diện gọn, màu trung tính; màu nhấn chỉ dùng cho trạng thái chọn, nút chính, nhận diện app.
3. **Nhất quán giữa 3 app**: cùng hệ thống component, cùng vị trí File/thanh truy cập nhanh/tìm lệnh/chia sẻ/tài khoản; chỉ khác tab và màu nhấn.
4. **Tiếng Việt là ngôn ngữ chính**: nhãn ngắn gọn, tự nhiên, dùng thuật ngữ quen thuộc với cán bộ văn phòng Việt Nam; giao diện chịu được nhãn dài hơn tiếng Anh 30–40%.
5. **Bàn phím được ưu tiên ngang chuột**: mọi lệnh đều tới được bằng bàn phím.
6. **Một giao diện cho web và desktop**: khác biệt duy nhất là thanh tiêu đề cửa sổ (desktop) và vài lệnh phụ thuộc nền tảng.

---

## 3. Hệ thống thiết kế (design tokens) — `packages/ui/tokens`

### 3.1. Màu nền tảng (thương hiệu Bluestar)
- Màu thương hiệu chính: xanh Bluestar — tự xác định mã màu từ logo Bluestar nếu có trong repo (`assets/brand/`), nếu chưa có dùng tạm `#1F6FEB` và ghi vào `NEEDS_HUMAN.md` để người dùng xác nhận.
- Thang xám trung tính 12 bậc cho nền, viền, chữ (light và dark).
- Màu trạng thái: thành công, cảnh báo, lỗi, thông tin — mỗi màu 3 bậc.
- Tất cả định nghĩa thành CSS variables + TS constants, có bản light/dark/high-contrast.

### 3.2. Màu nhấn từng app (khác biệt có chủ đích)
Không dùng tổ hợp "xanh dương = văn bản, xanh lá = bảng tính, cam/đỏ = trình chiếu" vốn là nhận diện của bên khác. Dùng:
- **BlueDoc**: chàm (indigo) — gợi ý `#4F46E5`
- **BlueSheet**: xanh ngọc (teal) — gợi ý `#0F9D8A`
- **BlueSlide**: hồng san hô (coral/magenta) — gợi ý `#E0457B`
- **Màn hình chung/BlueDrive**: xanh thương hiệu Bluestar

Màu nhấn dùng cho: icon app, dải nhận diện mảnh trên thanh tiêu đề, tab đang chọn, nút chính. Kiểm tra độ tương phản WCAG 2.2 AA ở cả light/dark.

### 3.3. Typography giao diện
- Font UI: **Be Vietnam Pro** hoặc **Inter** (OFL), hiển thị tiếng Việt chuẩn; fallback font hệ thống.
- Thang cỡ chữ: 11 / 12 / 13 / 14 / 16 / 20 / 24 / 32 px; nhãn ribbon 12px, tiêu đề hộp thoại 16–20px.

### 3.4. Khoảng cách, bo góc, bóng
- Lưới 4px. Bo góc: 4px (nút, ô nhập), 8px (hộp thoại, thẻ, popover).
- Bóng 3 mức (menu, hộp thoại, cửa sổ nổi).
- Mật độ: **Thường** (mặc định desktop), **Gọn** (nhiều lệnh hơn trên màn hình nhỏ), **Cảm ứng** (vùng chạm ≥ 40px, tự bật trên máy tính bảng).

### 3.5. Chuyển động
Ngắn và tiết chế: 120–200ms, easing chuẩn; tôn trọng `prefers-reduced-motion`.

---

## 4. Bộ icon riêng — `packages/ui/icons`

- Tự thiết kế dưới dạng SVG, có thể bắt đầu từ bộ Lucide (ISC) cho các biểu tượng chung rồi bổ sung icon chuyên dụng office tự vẽ.
- Kích thước: 16, 20, 24, 32 px; ribbon dùng 20 (nút nhỏ) và 32 (nút lớn).
- Phong cách thống nhất: nét 1.5px (ở 24px), góc bo nhẹ, hai tông: nét màu chữ + một điểm nhấn màu app. Có bản đơn sắc cho high-contrast.
- Danh mục tối thiểu: định dạng chữ (đậm, nghiêng, gạch chân, gạch ngang, chỉ số trên/dưới, màu chữ, tô sáng, xóa định dạng), đoạn văn (căn trái/giữa/phải/đều, danh sách chấm/số/đa cấp, thụt lề, giãn dòng, viền, tô nền, hiện ký tự ẩn), chèn (bảng, ảnh, hình, biểu tượng, mô hình 3D, SmartArt, biểu đồ, liên kết, chú thích, header/footer, số trang, hộp văn bản, ký tự đặc biệt, phương trình), bảng tính (tổng tự động, hàm, sắp xếp, lọc, định dạng có điều kiện, bảng tổng hợp, cố định ô, gộp ô, định dạng số, tiền tệ, phần trăm, tăng/giảm số thập phân), trình chiếu (slide mới, bố cục, chuyển trang, hiệu ứng, trình chiếu, người thuyết trình), chung (mới, mở, lưu, in, hoàn tác, làm lại, cắt, sao chép, dán, chổi định dạng, tìm, thay thế, chia sẻ, bình luận, theo dõi thay đổi, phóng to/thu nhỏ, chế độ xem).
- Mỗi icon có tên theo command id (`format.bold`, `insert.table`…), lưu trong một sprite/component tree-shakable.
- **Kiểm tra bắt buộc**: không icon nào được tạo bằng cách nhìn và vẽ theo icon của sản phẩm khác. Ghi nguồn gốc từng icon (tự vẽ/Lucide/…) trong `packages/ui/icons/SOURCES.md`.

### Logo & icon ứng dụng
- Logo BlueOffice và 3 icon app (BlueDoc, BlueSheet, BlueSlide) theo hệ thống chung: cùng khung hình, khác màu nhấn và ký hiệu bên trong. Hình dạng phải khác biệt rõ với icon app của Microsoft/Google/WPS (không dùng ô vuông có chữ cái lớn kiểu chữ W/X/P/S chồng lên tờ giấy).
- Làm bản nháp SVG, đặt trong `assets/brand/drafts/`, ghi `NEEDS_HUMAN.md` để người dùng duyệt bản chính thức.

---

## 5. Khung cửa sổ chung

Từ trên xuống:

1. **Thanh tiêu đề** (cao 40px):
   - Trái: icon app, thanh truy cập nhanh (Lưu, Hoàn tác, Làm lại + nút tùy biến).
   - Giữa: tên tài liệu (bấm để đổi tên, xem vị trí lưu, lịch sử phiên bản) + trạng thái lưu ("Đã lưu vào BlueDrive", "Đang lưu…", "Chưa lưu").
   - Ô **Tìm kiếm** (Ctrl+Q hoặc Alt+Q): tìm lệnh, tìm trong tài liệu, tìm trợ giúp.
   - Phải: người đang cùng sửa (avatar), Bình luận, Chia sẻ (nút chính màu nhấn), tài khoản; trên desktop thêm nút cửa sổ hệ điều hành.
2. **Hàng tab ribbon**: tab **Tệp** (màu nhấn, mở màn hình File) + các tab của app + tab ngữ cảnh (cùng màu nhấn, có nhãn nhóm như "Công cụ bảng") + bên phải: nút chuyển chế độ ribbon (đầy đủ/đơn giản/ẩn).
3. **Thân ribbon** (cao ~92px chế độ đầy đủ, ~40px chế độ đơn giản): các nhóm lệnh có nhãn nhóm ở dưới và nút mở hộp thoại nhóm (↘) ở góc.
4. **Vùng làm việc**: tài liệu ở giữa; ngăn điều hướng bên trái (tùy app); ngăn tác vụ bên phải (định dạng, bình luận, style, hiệu ứng…), dock được, đổi độ rộng được, nhiều ngăn xếp tab.
5. **Thanh trạng thái** (cao 24px): trái — thông tin tài liệu (trang/từ, ngôn ngữ, chính tả, ô đang chọn, thống kê vùng chọn); phải — chế độ xem, thanh trượt zoom, % zoom. Chuột phải để chọn mục hiển thị.

Chế độ ribbon:
- **Đầy đủ**: nút lớn + nút nhỏ, nhãn đầy đủ.
- **Đơn giản (một hàng)**: chỉ icon + nhãn ngắn, lệnh ít dùng vào menu "…".
- **Tự ẩn**: chỉ hiện khi bấm tab. Nhớ lựa chọn theo người dùng.
- Khi cửa sổ hẹp: nhóm co dần theo thứ tự ưu tiên khai báo (lớn → nhỏ → chỉ icon → thu vào một nút thả xuống của nhóm).

---

## 6. Cấu trúc tab theo từng app

Tên tab & nhóm dưới đây là tiếng Việt thông dụng; nhóm là gợi ý logic, có thể điều chỉnh nếu có lý do khả dụng (ghi ADR). Mỗi lệnh khai báo trong command registry với: id, nhãn ngắn, nhãn dài, mô tả (tự viết), icon, phím tắt, KeyTip, kích thước mặc định trên ribbon, độ ưu tiên co giãn.

### 6.1. BlueDoc
- **Trang đầu**: Bộ nhớ tạm (Dán ▾, Cắt, Sao chép, Chổi định dạng) · Phông chữ (font, cỡ, tăng/giảm cỡ, đổi kiểu chữ hoa/thường, xóa định dạng, đậm, nghiêng, gạch chân ▾, gạch ngang, chỉ số dưới/trên, hiệu ứng chữ ▾, tô sáng ▾, màu chữ ▾) · Đoạn văn (danh sách chấm/số/đa cấp, giảm/tăng thụt lề, sắp xếp, hiện ký tự ẩn, căn lề ×4, giãn dòng ▾, tô nền ▾, viền ▾) · Kiểu (gallery style + ngăn Kiểu) · Chỉnh sửa (Tìm ▾, Thay thế, Chọn ▾) · Đọc chính tả, BlueAI (nếu bật).
- **Chèn**: Trang (trang bìa, trang trắng, ngắt trang) · Bảng · Hình minh họa (ảnh, hình, biểu tượng, mô hình 3D, SmartArt, biểu đồ, chụp màn hình) · Phương tiện (video trực tuyến) · Liên kết (liên kết, dấu trang, tham chiếu chéo) · Bình luận · Đầu trang & chân trang (đầu trang, chân trang, số trang) · Văn bản (hộp văn bản, khối nội dung, WordArt, chữ cái lớn đầu đoạn, dòng chữ ký, ngày giờ, đối tượng) · Ký hiệu (phương trình, ký tự đặc biệt).
- **Vẽ**: công cụ bút, tẩy, thước, chuyển nét vẽ.
- **Thiết kế**: chủ đề, bộ kiểu tài liệu, màu, phông, giãn đoạn, hiệu ứng, đặt làm mặc định · Nền trang (hình mờ, màu trang, viền trang).
- **Bố cục**: Thiết lập trang (lề, hướng, khổ giấy, cột, ngắt, số dòng, gạch nối từ) · Đoạn văn (thụt lề, khoảng cách) · Sắp xếp (vị trí, ngắt chữ, đưa lên/xuống, ngăn chọn, căn, nhóm, xoay).
- **Tham chiếu**: Mục lục · Chú thích (chân trang, cuối văn bản) · Trích dẫn & tài liệu tham khảo · Chú thích hình/bảng (chèn chú thích, mục lục hình, tham chiếu chéo) · Chỉ mục · Bảng trích dẫn pháp lý.
- **Trộn thư**: Tạo (phong bì, nhãn) · Bắt đầu trộn thư · Viết & chèn trường · Xem trước kết quả · Hoàn tất.
- **Xem lại**: Soát lỗi (chính tả & ngữ pháp, từ đồng nghĩa, đếm từ) · Đọc to · Khả năng tiếp cận · Ngôn ngữ (dịch, ngôn ngữ) · Bình luận · Theo dõi thay đổi · Thay đổi (chấp nhận/từ chối, trước/sau) · So sánh · Bảo vệ (hạn chế chỉnh sửa) · Ký số.
- **Xem**: Chế độ xem (đọc, bố cục in, bố cục web, dàn ý, bản nháp, tập trung, trình đọc tập trung) · Hiển thị (thước, lưới, ngăn điều hướng) · Thu phóng · Cửa sổ (cửa sổ mới, sắp xếp, chia đôi, xem song song) · BlueScript.
- **Trợ giúp**.
- **Tab ngữ cảnh**: Bảng (Thiết kế bảng, Bố cục bảng) · Ảnh (Định dạng ảnh) · Hình/hộp văn bản (Định dạng hình) · Biểu đồ (Thiết kế biểu đồ, Định dạng) · SmartArt · Phương trình · Đầu trang & chân trang.

### 6.2. BlueSheet
- **Trang đầu**: Bộ nhớ tạm · Phông chữ (kèm viền ô, màu nền ô) · Căn lề (căn dọc/ngang, hướng chữ, xuống dòng, gộp & căn giữa ▾, thụt lề) · Số (định dạng số ▾, tiền tệ ₫, phần trăm, dấu phân cách nghìn, tăng/giảm thập phân) · Kiểu (định dạng có điều kiện, định dạng thành bảng, kiểu ô) · Ô (chèn, xóa, định dạng) · Chỉnh sửa (tổng tự động ▾, điền ▾, xóa ▾, sắp xếp & lọc, tìm & chọn) · Phân tích nhanh.
- **Chèn**: Bảng (bảng tổng hợp, bảng tổng hợp đề xuất, bảng) · Hình minh họa · Biểu đồ (biểu đồ đề xuất + từng nhóm loại biểu đồ, biểu đồ tổng hợp, bản đồ) · Biểu đồ thu nhỏ (đường, cột, thắng/thua) · Bộ lọc (slicer, dòng thời gian) · Liên kết · Bình luận · Văn bản (hộp văn bản, đầu/chân trang, WordArt, dòng chữ ký, đối tượng) · Ký hiệu.
- **Vẽ**.
- **Bố cục trang**: Chủ đề · Thiết lập trang (lề, hướng, khổ, vùng in, ngắt trang, nền, in tiêu đề) · Vừa trang · Tùy chọn trang tính (lưới, tiêu đề) · Sắp xếp.
- **Công thức**: Thư viện hàm (chèn hàm, tổng tự động, dùng gần đây, tài chính, logic, văn bản, ngày giờ, tra cứu, toán, thêm hàm ▾) · Tên đã xác định (quản lý tên, xác định tên, dùng trong công thức, tạo từ vùng chọn) · Kiểm tra công thức (truy vết ô tham chiếu/phụ thuộc, bỏ mũi tên, hiện công thức, kiểm tra lỗi, đánh giá công thức, cửa sổ theo dõi) · Tính toán (tùy chọn tính, tính ngay, tính trang tính).
- **Dữ liệu**: Lấy & biến đổi dữ liệu · Truy vấn & kết nối (làm mới tất cả) · Sắp xếp & lọc (sắp xếp, lọc, xóa lọc, áp dụng lại, nâng cao) · Công cụ dữ liệu (văn bản thành cột, điền nhanh, xóa trùng lặp, xác thực dữ liệu, hợp nhất, quan hệ) · Dự báo (phân tích giả định: tìm mục tiêu, kịch bản, bảng dữ liệu; trang dự báo) · Dàn ý (nhóm, bỏ nhóm, tổng phụ) · Phân tích (Solver, công cụ phân tích thống kê).
- **Xem lại**: Soát lỗi · Khả năng tiếp cận · Ngôn ngữ · Bình luận & ghi chú · Bảo vệ (trang tính, sổ làm việc, cho phép sửa vùng) · Hiển thị thay đổi.
- **Xem**: Chế độ xem trang tính · Chế độ xem sổ làm việc (thường, xem ngắt trang, bố cục trang, dạng xem tùy chỉnh) · Hiển thị · Thu phóng · Cửa sổ (cố định ô ▾, chia, ẩn, xem song song) · BlueScript.
- **Trợ giúp**.
- **Tab ngữ cảnh**: Thiết kế bảng · Phân tích & Thiết kế bảng tổng hợp · Biểu đồ · Slicer · Ảnh/Hình.
- **Thanh công thức** dưới ribbon: Hộp tên (có danh sách tên), nút hủy/xác nhận/chèn hàm, ô công thức mở rộng nhiều dòng, gợi ý hàm & tooltip cú pháp.
- **Thanh trang tính** dưới vùng lưới: nút cuộn, các tab trang tính (màu tab, kéo thả, chuột phải), nút thêm trang tính, thanh cuộn ngang.

### 6.3. BlueSlide
- **Trang đầu**: Bộ nhớ tạm · Trang chiếu (slide mới ▾, bố cục, đặt lại, phần) · Phông chữ · Đoạn văn (kèm hướng chữ, căn văn bản, chuyển thành SmartArt) · Vẽ (gallery hình, sắp xếp, kiểu nhanh, tô, viền, hiệu ứng hình) · Chỉnh sửa · Gợi ý thiết kế.
- **Chèn**: Trang chiếu · Bảng · Hình ảnh · Hình minh họa · Thu phóng (tóm tắt, phần, trang chiếu) · Liên kết (liên kết, hành động) · Bình luận · Văn bản (hộp văn bản, đầu/chân trang, WordArt, ngày giờ, số trang chiếu, đối tượng) · Ký hiệu · Phương tiện (video, âm thanh, ghi màn hình).
- **Vẽ**.
- **Thiết kế**: Chủ đề · Biến thể · Tùy chỉnh (kích thước trang chiếu, định dạng nền) · Gợi ý thiết kế.
- **Chuyển tiếp**: Xem trước · Gallery hiệu ứng chuyển tiếp (gồm Biến hình) · Tùy chọn hiệu ứng · Thời gian (âm thanh, thời lượng, áp dụng tất cả, khi bấm chuột/sau thời gian).
- **Hiệu ứng động**: Xem trước · Gallery hiệu ứng (xuất hiện, nhấn mạnh, biến mất, đường chuyển động) · Tùy chọn hiệu ứng · Nâng cao (thêm hiệu ứng, ngăn hiệu ứng, kích hoạt, sao chép hiệu ứng) · Thời gian (bắt đầu, thời lượng, độ trễ, sắp xếp lại).
- **Trình chiếu**: Bắt đầu (từ đầu, từ trang hiện tại, trình chiếu trực tuyến, trình chiếu tùy chỉnh) · Thiết lập (thiết lập trình chiếu, ẩn trang, tập dượt thời gian, ghi lại) · Màn hình (chọn màn hình, dạng xem người thuyết trình) · Phụ đề.
- **Ghi**: ghi trình chiếu, xuất video.
- **Xem lại**: Soát lỗi · Khả năng tiếp cận · Ngôn ngữ · Bình luận · So sánh · Bảo vệ.
- **Xem**: Dạng xem bản trình bày (thường, dàn ý, sắp xếp trang chiếu, trang ghi chú, dạng đọc) · Dạng xem cái (trang chiếu cái, bản phát tay cái, ghi chú cái) · Hiển thị (thước, lưới, đường gióng, ghi chú) · Thu phóng · Màu/Thang xám · Cửa sổ · BlueScript.
- **Trợ giúp**.
- **Tab ngữ cảnh**: Định dạng hình · Định dạng ảnh · Bảng (thiết kế/bố cục) · Biểu đồ · SmartArt · Video/Âm thanh (định dạng, phát lại) · Trang chiếu cái (khi đang sửa master).
- Bố cục vùng làm việc: cột ảnh thu nhỏ slide bên trái (kéo thả, phần), slide ở giữa, ngăn ghi chú dưới (kéo giãn được), ngăn tác vụ phải.

---

## 7. Màn hình File (Tệp) — toàn trang

Cột trái (nền màu nhấn đậm nhạt theo theme): Quay lại · Trang chủ · Mới · Mở · Thông tin · Lưu · Lưu thành · Lịch sử phiên bản · In · Chia sẻ · Xuất · Đóng · (dưới cùng) Tài khoản · Phản hồi · Tùy chọn.

- **Trang chủ/Mới**: dải mẫu nổi bật (tài liệu trống + mẫu tự thiết kế, ưu tiên mẫu hành chính Việt Nam), tìm mẫu, danh sách tài liệu gần đây (tab: Gần đây, Đã ghim, Được chia sẻ với tôi), nguồn: BlueDrive, Máy tính này.
- **Thông tin**: thuộc tính tài liệu, bảo vệ tài liệu, kiểm tra tài liệu (kiểm tra ẩn danh, khả năng tiếp cận, tương thích), quản lý phiên bản.
- **In**: bên trái thiết lập (máy in, số bản, phạm vi, một/hai mặt, hướng, khổ, lề, số trang mỗi tờ, co giãn), bên phải xem trước có điều hướng trang và zoom.
- **Xuất**: PDF/PDF-A, định dạng khác, video (BlueSlide), gói trình chiếu.
- **Tùy chọn**: hộp thoại lớn có danh mục bên trái: Chung, Hiển thị, Soát lỗi, Lưu, Ngôn ngữ, Khả năng tiếp cận, Nâng cao, Tùy biến ribbon, Thanh truy cập nhanh, Bổ trợ, Bảo mật.

---

## 8. Thành phần tương tác

- **Gallery** (style, chủ đề, hình, hiệu ứng): hàng xem trước trong ribbon + nút mở rộng; **xem trước trực tiếp** khi rê chuột (live preview) — áp dụng tạm lên tài liệu, bỏ khi rời chuột.
- **Nút tách (split button)**: phần trái chạy lệnh gần nhất, phần ▾ mở menu (vd màu chữ, gạch chân, dán).
- **Bảng chọn màu**: màu theme (10 cột × 6 sắc độ), màu chuẩn, màu gần đây, màu khác (HEX/RGB/HSL), công cụ hút màu, "Tự động/Không màu".
- **Bộ chọn bảng**: lưới rê chuột chọn số hàng × cột, + Chèn bảng…, Vẽ bảng, Chuyển văn bản thành bảng, Bảng tính nhúng, Bảng nhanh.
- **Bộ chọn font**: danh sách hiển thị tên bằng chính font đó, font chủ đề ở đầu, gần đây, tìm kiếm, cảnh báo font thay thế khi tài liệu dùng font máy không có.
- **Menu chuột phải** theo ngữ cảnh + **thanh định dạng nổi** (mini toolbar) xuất hiện cạnh vùng chọn.
- **Hộp thoại nhóm** (Phông chữ, Đoạn văn, Định dạng ô, Thiết lập trang…): bố cục tab, xem trước, nút Đặt làm mặc định; mở bằng nút ↘ của nhóm.
- **Ngăn tác vụ** Định dạng đối tượng: các mục thu gọn được (Tô, Đường viền, Hiệu ứng, Kích thước & thuộc tính, Văn bản), thay đổi áp dụng ngay.
- **Thông báo**: thanh thông báo dưới ribbon (chế độ chỉ đọc, cần bật nội dung, khôi phục), toast nhỏ góc dưới (đã lưu, lỗi đồng bộ).
- **Tooltip giàu nội dung**: tên lệnh, phím tắt, mô tả 1–2 câu tự viết, liên kết "Tìm hiểu thêm" tới trợ giúp của BlueOffice.

---

## 9. Bàn phím & khả năng tiếp cận

- Phím tắt tiêu chuẩn quen thuộc (Ctrl+N/O/S/P/Z/Y/X/C/V/B/I/U/F/H/K, Ctrl+Shift+V dán văn bản thuần, F2, F4, F5, F7 chính tả, F12 lưu thành, Ctrl+Enter, Shift+F3…), bảng đầy đủ trong `docs/ui/shortcuts.md`, có cấu hình lại được.
- **KeyTips**: nhấn Alt hiện chữ gợi ý trên tab, rồi trên từng lệnh; tự gán KeyTip không trùng trong cùng phạm vi; phím gợi ý dựa trên nhãn tiếng Việt khi giao diện tiếng Việt, hỗ trợ cả bộ KeyTip theo nhãn tiếng Anh như tùy chọn cho người quen tay.
- F6 di chuyển vòng giữa vùng: ribbon → tài liệu → ngăn tác vụ → thanh trạng thái.
- Mọi điều khiển có vai trò & nhãn ARIA, thứ tự focus hợp lý, viền focus rõ; tương thích NVDA, JAWS, VoiceOver, Narrator.
- Chế độ tương phản cao, tôn trọng cỡ chữ hệ thống, không truyền đạt thông tin chỉ bằng màu.
- Đạt WCAG 2.2 AA (kiểm tra tự động bằng axe + kiểm tra tay các luồng chính).

---

## 10. Đa ngôn ngữ

- vi-VN mặc định, en-US đầy đủ; cấu trúc sẵn để thêm ngôn ngữ.
- File thuật ngữ `docs/ui/glossary.md`: bảng thuật ngữ Việt–Anh thống nhất (ví dụ: Ribbon = Dải lệnh, Slide = Trang chiếu, Worksheet = Trang tính, Workbook = Sổ làm việc, PivotTable = Bảng tổng hợp, Track changes = Theo dõi thay đổi…). Mọi chuỗi giao diện phải theo glossary.
- Không cắt chữ: nhãn quá dài phải xuống dòng có kiểm soát hoặc rút gọn theo bản nhãn ngắn đã khai báo; test tự động phát hiện chữ bị tràn.

---

## 11. Màn hình khởi động & điểm vào

- Trang **BlueOffice Home** (web và desktop): nút tạo nhanh 3 loại tài liệu, mẫu nổi bật, tài liệu gần đây/ghim/được chia sẻ, kéo thả file để mở, ô tìm kiếm tài liệu.
- Màn hình chờ khi mở app: logo + tên app + thanh tiến trình mảnh, tối đa ~1 giây trước khi hiện khung giao diện.
- Trang tài liệu trống mặc định: BlueDoc khổ A4, lề và font theo thể thức văn bản Việt Nam (cấu hình được); BlueSheet font và định dạng số vi-VN; BlueSlide 16:9 với theme mặc định của BlueOffice.

---

## 12. Triển khai kỹ thuật

- Code giao diện trong `packages/ui` (component), `packages/ui/tokens`, `packages/ui/icons`, cấu hình ribbon từng app trong `apps/web/src/<app>/ribbon.config.ts` (khai báo dữ liệu, không hard-code JSX từng nút).
- Ribbon render từ **command registry**: mỗi nút liên kết command id, trạng thái enabled/checked/value tính từ selection; menu chuột phải, phím tắt, ô tìm lệnh, KeyTips dùng chung registry.
- Thuật toán co giãn ribbon theo độ rộng, có test cho các mốc 1920, 1440, 1280, 1024, 768 px.
- **Storybook** cho mọi component (light/dark/high-contrast, vi/en, 3 mật độ).
- **Visual regression** cho Storybook và khung cửa sổ 3 app ở các kích thước trên.
- E2E (Playwright) cho luồng: định dạng chữ bằng ribbon, bằng phím tắt, bằng KeyTips, bằng ô tìm lệnh; chèn bảng; mở màn hình File; đổi chế độ ribbon; đổi theme.
- Hiệu năng: mở/đổi tab ribbon < 50ms, hiển thị gallery xem trước < 100ms, không giật khi gõ.
- Chạy giống nhau trên web và desktop (Tauri); trên desktop dùng thanh tiêu đề tùy biến tích hợp nút cửa sổ hệ điều hành.

---

## 13. Quy trình làm việc

1. Tạo `docs/ui/DESIGN.md` tóm tắt hệ thống thiết kế, `docs/ui/glossary.md`, `docs/ui/shortcuts.md`, `docs/ui/BACKLOG.md` (chia nhỏ mọi mục trong prompt này thành hạng mục có ID `UI-…`, độ ưu tiên, tiêu chí hoàn thành).
2. Làm theo thứ tự: tokens → icon cơ bản → khung cửa sổ → command registry & ribbon engine → ribbon BlueDoc → BlueSheet → BlueSlide → màn hình File → hộp thoại & ngăn tác vụ → KeyTips & khả năng tiếp cận → Home & mẫu → hoàn thiện icon.
3. Mỗi hạng mục: làm → test (unit, visual, E2E, axe) → commit & push → cập nhật BACKLOG → làm tiếp hạng mục sau, **không dừng để hỏi**. Quyết định thiết kế chưa rõ: tự chọn phương án hợp lý, ghi `docs/adr/`. Việc cần người duyệt (logo chính thức, mã màu thương hiệu, icon app) ghi `NEEDS_HUMAN.md`, dùng bản nháp và làm tiếp.
4. Kết thúc khi mọi hạng mục `UI-…` xong, toàn bộ test xanh, và đã viết `docs/ui/REPORT.md` kèm ảnh chụp khung giao diện 3 app (light/dark, đầy đủ/đơn giản), danh sách việc cần người dùng duyệt.

---

## 14. Tự kiểm tra trước khi kết thúc

- [ ] Không có tài nguyên đồ họa nào ngoài nguồn ghi trong `SOURCES.md` / `THIRD_PARTY_NOTICES.md`.
- [ ] Không có tên/nhãn hiệu của bên khác trong giao diện ngoài câu mô tả tương thích định dạng.
- [ ] Màu nhấn 3 app đúng mục 3.2, đạt tương phản AA.
- [ ] Mọi chuỗi giao diện qua i18n và đúng glossary.
- [ ] Mọi lệnh tới được bằng bàn phím; axe không báo lỗi nghiêm trọng.
- [ ] Ribbon co giãn đúng ở mọi mốc độ rộng, không chữ tràn.
- [ ] Web và desktop hiển thị như nhau.
