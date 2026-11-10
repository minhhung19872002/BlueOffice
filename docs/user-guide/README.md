# Hướng dẫn sử dụng BlueOffice

BlueOffice gồm ba ứng dụng văn phòng dùng chung một giao diện:

| Ứng dụng  | Dùng để                     | Hướng dẫn riêng              |
| --------- | --------------------------- | ---------------------------- |
| BlueDoc   | Soạn thảo văn bản           | [bluedoc.md](bluedoc.md)     |
| BlueSheet | Bảng tính                   | [bluesheet.md](bluesheet.md) |
| BlueSlide | Trình chiếu                 | [blueslide.md](blueslide.md) |
| (cả ba)   | Phím tắt theo từng ứng dụng | [phim-tat.md](phim-tat.md)   |

Tài liệu này mô tả phần **dùng chung** cho cả ba ứng dụng. Tên lệnh, tên tab và tên hộp thoại được ghi đúng như
giao diện tiếng Việt hiển thị. Ký hiệu **Tab › Nhóm › Lệnh** chỉ đường đi trên ribbon, ví dụ
**Xem lại › Soát lỗi › Soát chính tả và ngữ pháp…**.

Mỗi tính năng còn giới hạn có ghi chú **Hạn chế**. Những gì chưa có được liệt kê ở mục "Chưa hỗ trợ" cuối mỗi tệp.

## Mục lục

1. [Chạy ứng dụng](#1-chạy-ứng-dụng)
2. [Mở, lưu và định dạng tệp](#2-mở-lưu-và-định-dạng-tệp)
3. [Giao diện chung](#3-giao-diện-chung)
4. [Sao chép, dán, tìm kiếm](#4-sao-chép-dán-tìm-kiếm)
5. [Cộng tác, chú thích, phiên bản](#5-cộng-tác-chú-thích-phiên-bản)
6. [Bảo vệ tệp và chữ ký số](#6-bảo-vệ-tệp-và-chữ-ký-số)
7. [In và xuất PDF](#7-in-và-xuất-pdf)
8. [Chính tả, ngữ pháp, tự động sửa](#8-chính-tả-ngữ-pháp-tự-động-sửa)
9. [Giọng nói và dịch](#9-giọng-nói-và-dịch)
10. [Tự động hóa bằng BlueScript](#10-tự-động-hóa-bằng-bluescript)
11. [Kiểm tra tài liệu và trợ năng](#11-kiểm-tra-tài-liệu-và-trợ-năng)
12. [Chưa hỗ trợ (chung)](#12-chưa-hỗ-trợ-chung)

## 1. Chạy ứng dụng

- **Trên web:** mở địa chỉ máy chủ BlueOffice của đơn vị, đường dẫn `/doc` (BlueDoc), `/sheet` (BlueSheet),
  `/slide` (BlueSlide).
- **Trên máy tính (bản desktop):** mở BlueDoc, BlueSheet hoặc BlueSlide như một chương trình thông thường.
  Màn hình **Bắt đầu** có ba thẻ:
  - **Trang chủ:** **Tài liệu trống** / **Bảng tính trống** / **Bản trình chiếu trống**, **Mở tệp trên máy…**,
    danh sách **Tệp gần đây**.
  - **BlueDrive:** đăng nhập (**Đăng nhập bằng tài khoản chung (SSO)** hoặc dán mã truy cập), rồi duyệt
    **Tệp của tôi**, **Gần đây**, **Được chia sẻ với tôi**.
  - **Cài đặt:** nhập địa chỉ **Máy chủ BlueOffice** (ví dụ `https://office.congty.vn`) và khai báo
    **Thư mục đồng bộ BlueDrive**. Tệp mở từ thư mục đồng bộ có thể chuyển sang **Cộng tác trực tuyến**.

Thẻ BlueDrive chỉ dùng được khi quản trị viên đã kết nối máy chủ BlueOffice với kho lưu trữ BlueDrive.

## 2. Mở, lưu và định dạng tệp

### 2.1. Màn hình Tệp

Bấm **Tệp** ở đầu ribbon để mở màn hình Tệp. Bấm **Quay lại** để trở về tài liệu. Các mục bên trái:

| Mục           | Nội dung                                                                                         |
| ------------- | ------------------------------------------------------------------------------------------------ |
| **Mới**       | Tài liệu trống; BlueDoc có thêm **Mới từ mẫu**                                                   |
| **Mở**        | **Duyệt** để chọn tệp trên máy; **Đã ghim và gần đây** (bấm **Ghim** / **Bỏ ghim**)              |
| **Thông tin** | **Thuộc tính** (Tiêu đề, Chủ đề, Tác giả, Từ khóa…) và **Bảo vệ, kiểm tra và phiên bản**         |
| **Lưu**       | Lưu tệp hiện tại                                                                                 |
| **Lưu thành** | **Kiểu tài liệu** (xem bảng định dạng bên dưới) và **Định dạng khác**                            |
| **In**        | **In… (xem trước và thiết lập)**                                                                 |
| **Chia sẻ**   | Mời người khác xem hoặc cùng sửa (khi tệp mở từ kho lưu trữ)                                     |
| **Xuất**      | PDF, và tuỳ ứng dụng: CSV, ảnh PNG, ảnh động / video, tệp chủ đề                                 |
| **Đóng**      | Đóng tài liệu                                                                                    |
| **Tùy chọn**  | Ngôn ngữ giao diện, giao diện sáng / tối, mật độ hiển thị, soát lỗi, tuỳ biến ribbon và phím tắt |

> **Hạn chế:** mục **Mở** chưa liệt kê tệp trên BlueDrive hoặc tệp được chia sẻ; danh sách tệp gần đây chỉ mở lại
> được trên bản desktop. **Chia sẻ** và **Đóng** chỉ hoạt động khi tệp được mở từ kho lưu trữ (BlueDrive hoặc máy
> chủ). **In** mở bảng In dạng hộp thoại.

### 2.2. Định dạng tệp

| Ứng dụng  | Mở được                                                       | Lưu thành                                       | Xuất                                                        |
| --------- | ------------------------------------------------------------- | ----------------------------------------------- | ----------------------------------------------------------- |
| BlueDoc   | `.docx` `.docm` `.dotx` `.dotm` `.doc` `.odt` `.txt` `.md`    | `.docx` `.docm` `.dotx` `.dotm`                 | PDF, PDF/A, PDF có ký số                                    |
| BlueSheet | `.xlsx` `.xlsm` `.xltx` `.xltm` `.xlsb` `.xls` `.ods` `.csv`  | `.xlsx` `.xlsm` `.xltx` `.xltm`, `.csv`         | PDF, PDF/A                                                  |
| BlueSlide | `.pptx` `.pptm` `.potx` `.potm` `.ppsx` `.ppsm` `.ppt` `.odp` | `.pptx` `.pptm` `.potx` `.potm` `.ppsx` `.ppsm` | PDF, PDF/A, ảnh PNG / JPEG / SVG, video, ảnh động, gói .zip |

- Tương thích định dạng Microsoft Word (`.docx`), Microsoft Excel (`.xlsx`) và Microsoft PowerPoint (`.pptx`).
- Tệp kiểu cũ (`.doc`, `.xls`, `.xlsb`, `.ppt`) và tệp OpenDocument (`.odt`, `.ods`, `.odp`) được chuyển đổi khi mở.
  Khi lưu, bạn chọn một kiểu trong cột "Lưu thành".
- **Mẫu:** mở tệp mẫu (`.dotx`, `.xltx`, `.potx`…) sẽ tạo một tài liệu mới từ mẫu. Muốn sửa chính tệp mẫu, dùng
  lệnh **Sửa mẫu**.
- **Chủ đề:** lệnh **Chủ đề từ tệp…** áp dụng màu và phông từ một tệp chủ đề (`.thmx`) hoặc từ một tài liệu khác;
  **Lưu chủ đề…** ghi chủ đề đang dùng ra tệp `.thmx`.

> **Hạn chế:** tệp `.thmx` có bố cục và trang cái thì chỉ phần chủ đề (màu, phông) được dùng. Tài liệu mới tạo từ mẫu
> chưa ghi lại tên mẫu gốc.

### 2.3. Tệp có macro

BlueOffice **không bao giờ chạy macro VBA**. Khi mở tệp có macro, ứng dụng hiện thông báo "Tài liệu chứa macro — macro
sẽ được giữ nguyên nhưng không chạy". Macro được giữ nguyên trong tệp khi bạn lưu lại ở kiểu có macro (`.docm`,
`.xlsm`, `.pptm`…). Nếu bạn chọn **Lưu thành** một kiểu không chứa được macro (ví dụ `.docx`), ứng dụng báo trước
rằng macro sẽ bị bỏ.

**Xem mã macro (chỉ đọc).** Bấm **Xem macro** trên thanh thông báo, hoặc tab **Xem › Tự động hóa › Xem macro
(VBA)…**, để mở hộp thoại liệt kê các mô-đun của dự án VBA và mã của từng mô-đun. Mã chỉ để đọc và sao chép: không
sửa được, không chạy được. Chọn một mô-đun rồi bấm **Chuyển thành bản nháp BlueScript** để đưa mã sang trình soạn
BlueScript dưới dạng bản nháp — bạn xem lại, sửa các chỗ đánh dấu `TODO(VBA)` rồi mới tự lưu hoặc chạy; bản nháp không
bao giờ tự chạy. Dự án dùng bảng mã mà BlueOffice chưa giải mã (ngoài 1252, 1258, UTF-8) vẫn hiện được nhưng chữ có
dấu có thể sai; mô-đun hỏng được ghi là "không đọc được". Không có trình sửa mã macro. Trong BlueSheet, nút bấm được gán macro vẫn hiện tên macro ở
hộp thoại thiết lập điều khiển, nhưng bấm nút không chạy macro. Để tự động hóa, dùng
[BlueScript](#10-tự-động-hóa-bằng-bluescript).

> **Hạn chế:** trang macro kiểu cũ (XLM) trong sổ tính chưa bị bỏ khi lưu thành kiểu không có macro.

### 2.4. Tệp có mật khẩu

- Mở tệp có mật khẩu: hộp thoại **Cần mật khẩu** hiện ra, nhập mật khẩu rồi bấm **Mở**.
- Đặt mật khẩu mở tệp: chọn **Lưu kèm mật khẩu**, nhập mật khẩu hai lần, bấm **Đặt**, sau đó lưu tệp. Để trống
  mật khẩu nghĩa là lưu không có mật khẩu.

### 2.5. Tự lưu và khôi phục

Ứng dụng tự lưu bản nháp trong lúc bạn làm việc (thanh trạng thái hiện "Đã tự lưu lúc …"). Nếu ứng dụng bị đóng đột
ngột, lần mở sau sẽ hỏi "Có bản tự lưu chưa được lưu của …": chọn **Khôi phục** hoặc **Bỏ qua**.

### 2.6. Văn bản dùng phông TCVN3 / VNI

Chữ gõ bằng bảng mã cũ TCVN3 hoặc VNI có thể chuyển sang Unicode bằng lệnh **Chuyển chữ TCVN3/VNI sang Unicode**
(gõ tên lệnh vào ô **Tìm lệnh**). Sau khi chuyển, thông báo có nút **Hoàn tác chuyển đổi**.

## 3. Giao diện chung

### 3.1. Thanh tiêu đề và thanh truy cập nhanh

Trên cùng là tên tệp, trạng thái lưu (**Đã lưu**, **Chưa lưu**, **Đang lưu…**) và **Thanh truy cập nhanh** chứa các
lệnh bạn hay dùng. Bấm chuột phải vào một lệnh trên ribbon và chọn **Thêm vào thanh truy cập nhanh** để đưa lệnh
lên thanh; chọn **Xóa khỏi thanh truy cập nhanh** để gỡ.

### 3.2. Ribbon

- Lệnh được xếp theo **tab** (Trang đầu, Chèn, Xem lại, Xem…) và **nhóm**.
- **Tab ngữ cảnh** chỉ hiện khi bạn chọn đối tượng tương ứng, ví dụ **Bảng** khi con trỏ nằm trong bảng,
  **Định dạng ảnh** khi chọn ảnh.
- Khi cửa sổ hẹp, nhóm ở bên phải tự thu thành nút; các lệnh không đủ chỗ nằm trong **Thêm lệnh**.
- Một số nhóm có nút mở ngăn ở góc nhóm, ví dụ nhóm **Bộ nhớ tạm** mở **Lịch sử bộ nhớ tạm**.
- Chế độ hiển thị ribbon: **Ribbon đơn giản** (một hàng), **Thu gọn ribbon** / **Mở rộng ribbon** (phím `Ctrl+F1`).

> **Hạn chế:** tab ngữ cảnh cho phương trình, nét mực và đối tượng nhúng chưa tự hiện theo vùng chọn. Chưa có chế độ
> ribbon tự ẩn hoàn toàn.

### 3.3. Tìm lệnh

1. Bấm ô **Tìm lệnh (Alt+Q)** trên thanh tiêu đề hoặc nhấn `Alt+Q`.
2. Gõ vài chữ trong tên lệnh (có thể gõ không dấu).
3. Chọn lệnh trong danh sách. Danh sách có phần **Dùng gần đây** và **Lệnh liên quan**.

### 3.4. Dùng bàn phím thay chuột

- Nhấn rồi thả `Alt` (hoặc `F10`) để hiện chữ gợi ý phím trên ribbon; gõ chữ của tab, rồi chữ của lệnh.
- `Shift+F10` mở menu chuột phải tại vị trí đang chọn.
- `F6` chuyển qua lại giữa tài liệu và các ngăn tác vụ.
- Xem bảng đầy đủ ở [phim-tat.md](phim-tat.md).

### 3.5. Menu chuột phải và thanh định dạng nổi

Bấm chuột phải để mở menu theo ngữ cảnh (văn bản, ô, ảnh, tab trang tính…). Khi bạn chọn văn bản, **Thanh định
dạng nổi** hiện gần vùng chọn với các lệnh định dạng thường dùng.

> **Hạn chế:** BlueSheet và BlueSlide dùng menu chính tả riêng; chưa có menu con; thanh định dạng nổi của BlueSlide
> chưa có ô phông chữ và cỡ chữ.

### 3.6. Ngăn tác vụ

Các ngăn như **Kiểu**, **Điều hướng**, **Chú thích**, **Lịch sử phiên bản**, **BlueScript**, **Chọn đối tượng** mở
ở cạnh cửa sổ. Trên tiêu đề ngăn có các nút:

- **Gắn bên trái** / **Gắn bên phải**;
- **Tách thành cửa sổ nổi** (kéo để di chuyển, nhấp đúp tiêu đề để gắn lại);
- **Đóng ngăn**; kéo mép ngăn để **Đổi độ rộng ngăn**.

Nhiều ngăn gắn cùng một cạnh sẽ xếp thành các thẻ. Ứng dụng nhớ vị trí và độ rộng của ngăn.

> **Hạn chế:** chưa kéo thả để gắn ngăn; cửa sổ nổi chỉ di chuyển bằng chuột; ứng dụng chưa nhớ ngăn nào đang mở;
> chưa có ngăn định dạng hình; ngăn bảng tổng hợp, tìm kiếm, ký hiệu chưa chuyển sang cơ chế này.

### 3.7. Giao diện sáng / tối, mật độ, ngôn ngữ

Trong BlueDoc, vào **Xem › Giao diện** (hoặc **Tệp › Tùy chọn**):

- **Sáng / Tối / Tương phản cao:** mỗi lần bấm chuyển sang chế độ kế tiếp.
- **Mật độ: Thường / Gọn / Cảm ứng:** khoảng cách giữa các nút; ứng dụng nhớ lựa chọn theo người dùng.
- **English** / **Tiếng Việt:** đổi ngôn ngữ giao diện.
- **Thước kẻ**, **Người cùng sửa:** bật / tắt thước và danh sách người đang cùng sửa.

> **Hạn chế:** ribbon của BlueSheet và BlueSlide chưa có các nút này.

### 3.8. Tuỳ biến ribbon, thanh truy cập nhanh và phím tắt

Vào **Tệp › Tùy chọn**:

- **Tùy biến ribbon…**: thêm **Tab mới**, **Nhóm mới**, đưa lệnh vào nhóm (**Thêm »** / **« Xóa**), **Đổi tên…**,
  **Chuyển lên** / **Chuyển xuống**, ẩn / hiện tab; **Đặt lại tab** hoặc **Đặt lại tất cả**.
- **Tùy biến thanh truy cập nhanh…**: chọn lệnh, sắp thứ tự, **Hiện thanh truy cập nhanh bên dưới ribbon**.
- **Tùy biến phím tắt…**: tìm lệnh, bấm **Đổi**, nhấn tổ hợp phím mới, bấm **Gán**. Phím tắt phải có `Ctrl` hoặc
  `Alt`, hoặc là một phím `F1`–`F12`. Nếu tổ hợp đang gán cho lệnh khác, ứng dụng báo trước khi gán lại.
  **Mặc định** trả một lệnh về phím gốc; **Khôi phục tất cả** trả lại toàn bộ.
- **Xuất…** / **Nhập…**: ghi cấu hình ra tệp để dùng trên máy khác. Tệp cấu hình của ứng dụng nào chỉ nhập được
  vào ứng dụng đó.

## 4. Sao chép, dán, tìm kiếm

### 4.1. Lịch sử bộ nhớ tạm

Bấm nút ở góc nhóm **Bộ nhớ tạm** (lệnh **Hiện lịch sử bộ nhớ tạm**). Ngăn liệt kê những gì bạn đã sao chép hoặc
cắt trong BlueDoc, BlueSheet, BlueSlide. Bấm một mục để dán vào vị trí hiện tại; **Xóa hết** để dọn danh sách.

### 4.2. Tùy chọn dán

Sau khi dán, nút **Tùy chọn dán** hiện cạnh nội dung vừa dán (phím `Alt+Shift+V` trong BlueDoc):

| Ứng dụng  | Lựa chọn                                                                                                                                          |
| --------- | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| BlueDoc   | **Giữ nguyên định dạng gốc**, **Theo định dạng xung quanh (giữ đậm, nghiêng, danh sách)**, **Chỉ văn bản thuần**, **Dạng ảnh**, **Dạng liên kết** |
| BlueSheet | **Tất cả**, **Chỉ giá trị**, **Chỉ công thức**, **Chỉ định dạng**, **Đổi hàng thành cột**, **Tất cả, kèm độ rộng cột**, **Tất cả trừ đường viền** |
| BlueSlide | **Dùng chủ đề của bản trình chiếu này**, **Giữ nguyên định dạng gốc**, **Dạng ảnh**, **Chỉ văn bản thuần**                                        |

Trong BlueDoc, **Thiết lập dán mặc định…** cho chọn cách dán mặc định theo nguồn (**Trong cùng tài liệu**, **Từ tài
liệu, ứng dụng khác**, **Văn bản thuần**).

> **Hạn chế:** BlueSheet chưa có nút tuỳ chọn sau khi cắt; mục cũ trong lịch sử của BlueSheet dán ở dạng giá trị;
> BlueSlide dán "giữ nguyên định dạng gốc" chưa lấy màu từ trang cái nguồn. Nếu dán ngay tức thì sau khi sao chép,
> đôi khi nội dung cũ được dán — hãy dán lại.

### 4.3. Tìm và thay thế

Nhấn `Ctrl+F` (**Tìm**) hoặc `Ctrl+H` (**Thay thế** / **Tìm và thay thế**). Các tuỳ chọn: **Phân biệt hoa thường**,
**Cả từ**, **Biểu thức chính quy**, **Dùng ký tự đại diện**, **Tô sáng tất cả**. Trong BlueDoc, nút **Đặc biệt**
chèn mã như **Dấu đoạn (^p)**, **Tab (^t)**, **Ngắt dòng (^l)**, **Ngắt trang (^m)**, **Chữ số bất kỳ (^#)**,
**Chữ cái bất kỳ (^$)**, **Ký tự bất kỳ (^?)**.

### 4.4. Ký tự đặc biệt

**Chèn › Ký hiệu › Ký tự đặc biệt** mở bảng ký tự Unicode có ô tìm kiếm; bấm một ký tự để chèn.

## 5. Cộng tác, chú thích, phiên bản

### 5.1. Cùng sửa một tệp

Cộng tác thời gian thực có khi tệp được **mở từ kho lưu trữ** (BlueDrive hoặc hệ thống nhúng BlueOffice), trên web
lẫn bản desktop.

- Bạn thấy con trỏ và vùng chọn của người khác kèm tên: "… đang sửa đoạn này" (BlueDoc), "… đang chọn B2:D5"
  (BlueSheet), "… đang ở trang chiếu này" (BlueSlide).
- Mất mạng: thông báo "Mất kết nối — thay đổi sẽ được đồng bộ khi có mạng". Thay đổi được gửi lại khi có mạng. Nếu
  tệp đã bị người khác sửa trong lúc bạn ngoại tuyến, phần của bạn được lưu thành một bản sao riêng.
- Quyền: tệp có thể mở ở chế độ **Chỉ xem** hoặc **Chỉ xem (được bình luận)** tuỳ quyền người chia sẻ cấp.

> **Hạn chế:** khi hai người cùng đổi một thiết lập chung (ví dụ cùng một thuộc tính của tài liệu), thay đổi của
> người ghi sau được giữ. Chi tiết phạm vi đồng bộ: [`../collab-coverage.md`](../collab-coverage.md).

### 5.2. Chú thích (bình luận)

BlueDoc gọi là **Chú thích**, BlueSheet và BlueSlide gọi là **Bình luận**.

1. Chọn văn bản, ô hoặc đối tượng.
2. BlueDoc: **Xem lại › Chú thích › Chú thích mới** (`Ctrl+Alt+M`). BlueSheet: **Chèn › Biểu đồ › Bình luận**.
   BlueSlide: **Chèn › Văn bản › Bình luận**.
3. Gõ nội dung rồi bấm **Thêm**. Người khác bấm **Trả lời** để nối luồng.
4. Xong việc, bấm **Đã xử lý**; cần mở lại thì bấm **Mở lại**. **Xoá** để bỏ chú thích.

Gõ `@` rồi tên người để **nhắc tên**; chọn **Giao cho** để giao việc cho người được nhắc. Người được nhắc nhận
thông báo qua kho lưu trữ hoặc thư điện tử nếu quản trị viên đã bật (xem [`../comment-mentions.md`](../comment-mentions.md)).

### 5.3. Lịch sử phiên bản

| Ứng dụng  | Lệnh                                        |
| --------- | ------------------------------------------- |
| BlueDoc   | **Xem lại › So sánh › Lịch sử phiên bản…**  |
| BlueSheet | **Xem lại › Phiên bản › Hiện thay đổi**     |
| BlueSlide | **Xem lại › Phiên bản › So sánh phiên bản** |

1. Mở ngăn bằng lệnh trên. Danh sách **Các phiên bản** hiện số phiên bản, người sửa, thời gian.
2. Bấm **So sánh** để xem **Thay đổi kể từ phiên bản …**. Trong BlueDoc, khác biệt hiện ngay trong văn bản dưới dạng
   theo dõi thay đổi; BlueSheet và BlueSlide liệt kê từng thay đổi (đổi giá trị, đổi công thức, thêm trang chiếu…).
3. Bấm **Khôi phục** để đặt một phiên bản cũ làm phiên bản hiện tại.
4. **So sánh với tệp…** so tài liệu đang mở với một tệp trên máy.

> **Hạn chế:** lịch sử phiên bản chỉ có với tệp mở từ kho lưu trữ; ngoài ra chỉ so sánh được với tệp. Thay đổi được
> gán cho tác giả và thời điểm của phiên bản đem so, chưa tách theo từng phiên bản trung gian; chưa có lịch sử sửa
> của từng ô.

## 6. Bảo vệ tệp và chữ ký số

### 6.1. Bảo vệ tệp

Lệnh **Bảo vệ tệp** (màn hình **Tệp › Thông tin**; BlueSheet: **Xem lại › Bảo vệ**; BlueSlide: **Trang đầu › Tệp**)
mở hộp thoại có các phần:

- **Bản cuối:** **Đánh dấu là bản cuối** để người nhận biết tệp đã hoàn tất; tệp mở ở chế độ chỉ xem, người mở
  bấm **Vẫn sửa** nếu cần. **Bỏ dấu bản cuối** để gỡ.
- **Hạn chế ghi:** **Đề nghị mở ở chế độ chỉ xem**, hoặc đặt **Mật khẩu sửa** (không có mật khẩu thì chỉ xem được).
- **Nhãn mức độ mật:** **Công khai**, **Nội bộ**, **Mật**; có thể **Thêm dòng ghi nhãn lên đầu trang**.
- **Giới hạn chỉnh sửa** (BlueDoc): xem [bluedoc.md](bluedoc.md#bảo-vệ-văn-bản).
- **Vùng được sửa khi trang tính bị khoá** (BlueSheet): xem [bluesheet.md](bluesheet.md#bảo-vệ).

> **Hạn chế:** mật khẩu bảo vệ kiểu cũ của một số tệp không kiểm tra được (báo "Không kiểm tra được kiểu mật khẩu
> này"). Chưa phân quyền theo nhóm người dùng. Dòng ghi nhãn mức độ mật chưa có ở BlueSheet và BlueSlide.

### 6.2. Chữ ký số

Các lệnh ký số có trong **BlueDoc** (màn hình **Tệp › Thông tin**):

- **Ký số:** ký tài liệu `.docx`. Chọn **Phương thức ký**: **File chứng thư** (`.p12` / `.pfx` / `.pem` và mật
  khẩu), **USB token** (chỉ bản desktop; nhập đường dẫn module PKCS#11, bấm **Đọc token**, chọn chứng thư, nhập
  **Mã PIN token**) hoặc **Ký số từ xa** (nhập **Địa chỉ dịch vụ ký số**). Có thể nhập **Máy chủ dấu thời gian**,
  **Lý do**, **Nơi ký**. Bấm **Ký**.
- **Xuất PDF có ký số:** xuất PDF và ký luôn bản PDF.
- **Kiểm tra chữ ký:** hiện trạng thái từng chữ ký: **Hợp lệ và được tin cậy**; **Hợp lệ, nhưng chứng thư chưa
  được tin cậy**; **Hợp lệ, nhưng tài liệu đã bị sửa sau khi ký**; hoặc không hợp lệ. Bấm **Chi tiết** để xem
  chứng thư, dấu thời gian và lý do.
- **Dòng chữ ký:** xem [bluedoc.md](bluedoc.md#dòng-chữ-ký).

Khi ký từ xa, chỉ giá trị băm của tài liệu được gửi tới dịch vụ ký. Địa chỉ dịch vụ ký, máy chủ dấu thời gian và
danh sách chứng thư gốc tin cậy do quản trị viên cấu hình: [`../deploy/remote-signing.md`](../deploy/remote-signing.md).

> **Hạn chế:** BlueSheet và BlueSlide chưa có lệnh ký số trên giao diện. Ký tạo ra một bản riêng có đuôi "-signed".

## 7. In và xuất PDF

### 7.1. In

Nhấn `Ctrl+P` hoặc vào **Tệp › In**. Hộp thoại **In** có phần xem trước và các thiết lập:

- **Số bản**, **In theo từng bộ**, **In ngược thứ tự**.
- **Trang:** **Tất cả**, **Trang hiện tại**, **Phần đang chọn**, **Trang tuỳ chọn** (ví dụ `1-3, 5, 8-`);
  **Trang chẵn/lẻ**.
- **Số trang mỗi tờ** và **Thứ tự** xếp; **Co giãn theo giấy** (A3, A4, A5, B5, Letter, Legal).
- **Màu:** **Màu**, **Thang xám**, **Đen trắng**; **Viền mảnh quanh mỗi trang**.
- BlueDoc: **In sửa đổi và bình luận**, **Thêm trang thuộc tính tài liệu**.
- BlueSheet: **Phạm vi** (**Trang tính hiện hành**, **Toàn bộ sổ làm việc**, **Vùng ô đang chọn**), **Đường lưới**,
  **Tiêu đề hàng và cột**, **Hàng lặp lại**, **Cột lặp lại**, **Bỏ qua vùng in**.
- BlueSlide: **Bố cục in** (**Trang chiếu**, **Trang ghi chú**, **Dàn ý**, **Tài liệu phát tay**), **Số trang
  chiếu mỗi trang**, **In cả trang chiếu ẩn**, **Đóng khung trang chiếu**.

Trong BlueDoc, màn hình **Tệp › In** có thêm **In ngay** để in luôn mà không qua hộp thoại. Cấu hình in của bản
desktop: [`../deploy/desktop-print.md`](../deploy/desktop-print.md).

> **Hạn chế:** BlueDoc chưa in riêng vùng đang chọn; BlueSheet chưa in nhiều trang tính được chọn cùng lúc; hướng
> giấy luôn tự động; bong bóng sửa đổi vẽ đơn giản; dàn ý in dạng văn bản thuần; phần xem trước không tự cập nhật
> nếu bạn sửa tài liệu khi hộp thoại In đang mở.

### 7.2. Xuất PDF

- **Xuất PDF:** lưu một bản PDF với thiết lập mặc định (BlueDoc: `Ctrl+Shift+E`).
- **Xuất PDF/A, PDF có thẻ…** mở hộp thoại **Xuất PDF**:
  - **Chuẩn lưu trữ (PDF/A):** **PDF thường**, **PDF/A-1b**, **PDF/A-2b**, **PDF/A-3b** (đính kèm tệp gốc);
  - **PDF có thẻ (cấu trúc cho trình đọc màn hình)**;
  - **Tạo dấu trang từ đề mục** (BlueDoc), **từ tiêu đề trang chiếu** (BlueSlide), **từ tên trang tính** (BlueSheet).

> **Hạn chế:** PDF/A-1b không cho phép trong suốt nên độ mờ được trộn lên nền trắng và bóng mờ bị bỏ. Tệp PDF/A chưa
> được kiểm bằng công cụ kiểm định độc lập. Tài liệu phát tay và trang ghi chú chưa gắn thẻ.

## 8. Chính tả, ngữ pháp, tự động sửa

### 8.1. Soát chính tả

Từ điển có sẵn: **tiếng Việt** và **tiếng Anh**. Văn bản thuộc ngôn ngữ khác không được soát.

- **Kiểm tra chính tả khi gõ** (**Xem lại › Soát lỗi**): từ sai được gạch sóng đỏ. Bấm chuột phải vào từ để chọn
  gợi ý, bỏ qua, bỏ qua tất cả hoặc thêm vào từ điển của bạn.
- **Soát chính tả và ngữ pháp…** (`F7`): đi qua lần lượt từng lỗi trong cả tài liệu, sổ tính hoặc bản trình chiếu.
- BlueDoc: **Ngôn ngữ soát lỗi…** đặt ngôn ngữ cho vùng chọn hoặc đánh dấu không soát.

### 8.2. Ngữ pháp và văn phong (BlueDoc)

**Soát ngữ pháp và văn phong khi gõ** gạch sóng xanh các lỗi: thừa dấu cách, dấu cách trước dấu câu, thiếu dấu cách
sau dấu câu, không viết hoa đầu câu, lặp từ, ngoặc hoặc nháy không đóng, kiểu đặt dấu thanh không thống nhất
(hòa / hoà). Bấm chuột phải để xem quy tắc, sửa hoặc bỏ qua.

> **Hạn chế:** chỉ là các quy tắc trình bày; chưa phân tích cú pháp câu. Chỉ có trong BlueDoc.

### 8.3. Tự động sửa khi gõ (BlueDoc)

**Xem lại › Soát lỗi › Tuỳ chọn tự động sửa…** mở hộp thoại **Tự động sửa khi gõ**:

- **Bật tự động sửa khi gõ**;
- **Thay thế theo danh sách bên dưới** (cột **Khi gõ** → **Đổi thành**; **Thêm** / **Xoá**);
- **Viết hoa chữ cái đầu câu**, **Viết hoa chữ cái đầu ô bảng**, **Sửa từ bị viết hoa hai chữ đầu**;
- **Đổi dấu nháy thẳng thành nháy cong**, **Đổi gạch nối thành gạch ngang**, **Đổi phân số thành ký tự**;
- **Tự tạo danh sách** khi đầu đoạn gõ "- ", "\* ", "1. ", "a) "; **Tự tạo liên kết** cho địa chỉ web và thư điện tử;
- danh sách ngoại lệ; **Khôi phục mặc định**.

Nhấn `Ctrl+Z` ngay sau khi ứng dụng tự sửa để lấy lại chữ bạn đã gõ.

> **Hạn chế:** tự động sửa chỉ chạy trong thân văn bản BlueDoc; chưa chạy trong đầu trang, chân trang, chú thích,
> hộp văn bản, cũng như trong BlueSheet và BlueSlide. Thiết lập chưa lưu theo tài khoản.

## 9. Giọng nói và dịch

Ba tính năng dưới đây phụ thuộc trình duyệt hoặc dịch vụ do **quản trị viên bật trên máy chủ**. Nếu chưa bật, ứng
dụng báo "chưa dùng được ở đây".

### 9.1. Đọc chính tả (nói thành chữ)

1. Đặt con trỏ vào chỗ cần nhập (BlueSheet: chọn ô; BlueSlide: đang sửa một khối chữ).
2. Bấm **Trang đầu › Giọng nói › Đọc chính tả** (`Ctrl+Alt+Shift+D`) và cho phép dùng micrô.
3. Nói. Thanh điều khiển hiện trạng thái **Đang nghe…**, cho chọn **Ngôn ngữ**, **Tạm dừng**, **Tiếp tục nghe**,
   **Dừng đọc chính tả** (`Esc`).

Nói tên dấu câu để chèn dấu: "chấm", "phẩy", "chấm hỏi", "chấm than", "hai chấm", "chấm phẩy", "ba chấm", "xuống
dòng", "đoạn mới", "mở ngoặc" / "đóng ngoặc", "mở ngoặc kép" / "đóng ngoặc kép", "xoá từ vừa rồi". Mỗi câu đã chốt là
một bước hoàn tác.

Quản trị: [`../deploy/dictation.md`](../deploy/dictation.md).

> **Hạn chế:** chưa có bộ nhận dạng chạy ngay trên máy; "xoá từ vừa rồi" chỉ xoá trong đoạn hiện tại; trong
> BlueSheet cả ô là một bước hoàn tác.

### 9.2. Đọc to

Bấm **Xem lại › Giọng nói › Đọc to** (`Ctrl+Alt+Shift+R`). Từ đang đọc được tô sáng. Thanh điều khiển có **Tạm
dừng**, **Tiếp tục đọc**, **Dừng đọc**, chuyển tới đoạn / ô / khối chữ trước hoặc tiếp theo, **Tốc độ** và **Giọng
đọc**. BlueSlide có thêm **Đọc cả ghi chú**.

> **Hạn chế:** dùng giọng đọc cài sẵn của máy hoặc trình duyệt. Nếu máy chưa cài giọng tiếng Việt, ứng dụng chỉ báo
> và không đọc được tiếng Việt.

### 9.3. Dịch

1. Chọn văn bản, bấm **Xem lại › Ngôn ngữ › Dịch** để mở ngăn **Dịch**.
2. Chọn **Từ** (hoặc **Tự nhận biết**) và **Sang** (**Tiếng Việt**, **Tiếng Anh**), bấm **Dịch**.
3. Sửa bản dịch nếu cần rồi bấm **Chèn**, **Thay thế** hoặc **Sao chép**.

Dịch cả tệp: **Dịch tài liệu** (BlueDoc) và **Dịch bản trình chiếu** (BlueSlide) mở một bản sao đã dịch, tệp gốc
không đổi. BlueSheet có **Dịch ô chọn sang cột mới** và **Dịch ô chọn tại chỗ**.

Văn bản được gửi tới dịch vụ dịch do quản trị viên cấu hình: [`../deploy/translation.md`](../deploy/translation.md).

> **Hạn chế:** chưa có bộ dịch chạy ngay trên máy. BlueDoc chưa dịch đầu trang, chân trang, chú thích, bình luận,
> hộp văn bản; BlueSlide chưa dịch bảng. Dịch cả tài liệu chưa hỏi xác nhận khi tệp đang có thay đổi chưa lưu.

### 9.4. BlueAI

Bấm **Xem lại › BlueAI** để mở ngăn **BlueAI**. Ngăn nêu rõ nội dung sẽ được gửi tới máy chủ nào và giới hạn ký tự
mỗi yêu cầu; không có gì được gửi cho tới khi bạn bấm **Tạo**. Kết quả luôn hiện ở dạng **xem trước**: bấm nút áp dụng
để dùng, **Sao chép** hoặc **Bỏ**.

- **BlueDoc:** **Viết lại phần chọn** (giọng văn **Trang trọng (văn phong hành chính)**, **Ngắn gọn**, **Thân
  thiện**), **Tóm tắt** phần chọn hoặc cả tài liệu, **Soạn nháp theo yêu cầu**; rồi **Thay thế phần chọn** hoặc
  **Chèn** (một bước hoàn tác, được ghi nhận khi đang theo dõi thay đổi).
- **BlueSlide:** **Trang chiếu từ dàn ý (không cần AI)** — gõ dàn ý, mỗi dòng không thụt lề là tiêu đề một trang, dòng
  thụt lề hoặc bắt đầu bằng `-` là ý của trang đó; hoặc **Dàn ý trang chiếu từ chủ đề**; rồi **Tạo trang chiếu**.
- **BlueSheet:** **Gợi ý công thức** từ mô tả (công thức được kiểm tra cú pháp, chỉ chèn khi bạn bấm **Chèn vào ô hiện
  hành**) và **Phân tích dữ liệu** vùng chọn.

BlueAI mặc định tắt; quản trị viên bật và chọn dịch vụ: [`../deploy/ai.md`](../deploy/ai.md). Khi tắt, chỉ **Trang
chiếu từ dàn ý** dùng được.

> **Hạn chế:** nội dung do AI tạo có thể sai, hãy đọc lại trước khi dùng. BlueOffice không kèm mô hình nào. Dàn ý chỉ
> tạo trang tiêu đề + nội dung (chưa chọn bố cục, chưa có hình ảnh).

## 10. Tự động hóa bằng BlueScript

BlueScript là công cụ viết kịch bản bằng JavaScript chạy trong vùng cách ly, thay cho macro.

1. Vào **Xem › Tự động hóa › Trình soạn BlueScript** để mở ngăn **BlueScript**.
2. Gõ mã vào ô **Mã script**. `Ctrl+Space` hiện **Gợi ý**. Đối tượng gốc là `doc` (BlueDoc), `workbook`
   (BlueSheet), `presentation` (BlueSlide).
3. Bấm **Chạy** (`Ctrl+Enter`). Kết quả và lỗi hiện ở ô **Kết quả**. Bấm **Dừng** để ngắt script đang chạy.
4. Đặt **Tên script** rồi bấm **Lưu**; script nằm trong danh sách **Script đã lưu**.

Mỗi lần chạy script là **một bước hoàn tác**: nhấn `Ctrl+Z` để huỷ toàn bộ thay đổi của lần chạy đó. Script bị dừng
khi quá thời gian, quá bộ nhớ hoặc gọi quá nhiều lệnh.

**Ghi thao tác:** bấm **Xem › Tự động hóa › Ghi thao tác**, thực hiện các thao tác sửa tài liệu, rồi bấm **Dừng
ghi**. Script ghi được mở trong trình soạn — hãy lưu lại.

**Gán phím tắt hoặc nút:** với script đã lưu, đặt **Phím tắt** hoặc bật **Nút trên thanh truy cập nhanh**.

> **Hạn chế:** script lưu theo người dùng, không lưu trong tài liệu. Chỉ viết bằng JavaScript. Khi cộng tác trong
> BlueSheet, hoàn tác vẫn theo từng lệnh. **Lịch chạy trên máy chủ** đặt được nhưng máy chủ chưa có bộ chạy script
> (trạng thái "máy chủ chưa có bộ chạy script"). Chưa chuyển được macro VBA sang BlueScript.

## 11. Kiểm tra tài liệu và trợ năng

### 11.1. Kiểm tra tài liệu

Lệnh **Kiểm tra tài liệu** (BlueDoc: **Xem lại › Soát lỗi**; BlueSheet và BlueSlide: **Xem lại › Kiểm tra tài
liệu**) có ba thẻ:

- **Rà soát nội dung:** tìm chú thích, thay đổi được theo dõi, thuộc tính và thông tin cá nhân, dữ liệu XML tùy
  chỉnh, macro, văn bản ẩn, hàng / cột / trang tính ẩn, trang chiếu ẩn, ghi chú của người trình bày… và cho gỡ
  từng loại trước khi gửi tệp ra ngoài.
- **Trợ năng:** báo thiếu văn bản thay thế, bảng chưa có hàng tiêu đề, ô đã hợp nhất, bỏ qua cấp đề mục, chữ khó
  đọc, trang chiếu không có tiêu đề, trang tính còn tên mặc định…
- **Tương thích:** liệt kê nội dung có thể hiển thị khác khi mở bằng phần mềm khác.

> **Hạn chế:** kết quả hiện trong hộp thoại, chưa phải ngăn bên giữ mở; BlueSheet chưa có nút sửa nhanh; chưa gỡ được
> tài liệu nhúng; chưa tìm văn bản ẩn trong chú thích cuối trang.

### 11.2. Hỗ trợ tiếp cận

- Dùng trọn bằng bàn phím: gợi ý phím (`Alt`), `F6`, `Shift+F10`, **Tìm lệnh** (`Alt+Q`).
- Giao diện **Tương phản cao** và mật độ **Cảm ứng** (mục 3.7).
- **Đọc to** (mục 9.2) và **Đọc chính tả** (mục 9.1).
- Văn bản thay thế cho ảnh, mô hình 3D; **PDF có thẻ** cho trình đọc màn hình (mục 7.2).

## 12. Chưa hỗ trợ (chung)

Các tính năng sau **chưa có** trong bản này:

- Chạy macro VBA (chủ ý không hỗ trợ), trình xem mã macro, chuyển macro VBA thành BlueScript.
- Add-in (tiện ích mở rộng) và kho add-in nội bộ.
- BlueAI (viết lại, tóm tắt, soạn nháp, gợi ý công thức).
- API REST tạo / chuyển đổi tài liệu trên máy chủ; chạy BlueScript theo lịch trên máy chủ.
- Từ điển đồng nghĩa; trình đọc tập trung.
- Xoá nền ảnh; thư viện biểu tượng và ảnh mẫu.
- Biểu đồ bản đồ (tệp có sẵn biểu đồ bản đồ được giữ nguyên khi lưu).
- Chuyển nét mực thành hình, văn bản hoặc công thức toán.
- Soạn phương trình trực tiếp trên công thức (hiện sửa qua hộp thoại, chỉ trong BlueDoc).
- Lưu và mở HTML / MHT; xuất `.epub`, `.xps`; mở PDF để sửa.
- Bộ dịch và bộ nhận dạng giọng nói chạy ngay trên máy (hiện cần dịch vụ do quản trị viên cấu hình).
- Tuỳ biến thanh trạng thái, chia cửa sổ, cuộn đồng bộ hai tài liệu.
