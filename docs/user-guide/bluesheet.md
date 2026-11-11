# BlueSheet – Bảng tính

Hướng dẫn này đi theo từng tab của ribbon BlueSheet. Phần dùng chung cho cả bộ (màn hình Tệp, tìm lệnh, cộng tác,
in, BlueScript…) nằm ở [README.md](README.md). Phím tắt: [phim-tat.md](phim-tat.md#bluesheet).

Tương thích định dạng Microsoft Excel (`.xlsx`). Danh sách định dạng mở và lưu:
[README.md](README.md#22-định-dạng-tệp).

## Mục lục

- [Làm quen với cửa sổ](#làm-quen-với-cửa-sổ)
- [Nhập dữ liệu và công thức](#nhập-dữ-liệu-và-công-thức)
- [Hàm](#hàm)
- [Tab Trang đầu](#tab-trang-đầu)
- [Tab Chèn](#tab-chèn)
- [Tab Bảng tổng hợp](#tab-bảng-tổng-hợp)
- [Tab Công thức](#tab-công-thức)
- [Tab Dữ liệu](#tab-dữ-liệu)
- [Tab Bố cục trang](#tab-bố-cục-trang)
- [Tab Xem lại](#tab-xem-lại)
- [Tab Xem](#tab-xem)
- [Tab ngữ cảnh](#tab-ngữ-cảnh)
- [Giới hạn về kích thước](#giới-hạn-về-kích-thước)
- [Chưa hỗ trợ](#chưa-hỗ-trợ)

## Làm quen với cửa sổ

- **Hộp tên** (bên trái thanh công thức): hiện địa chỉ ô đang chọn.
- **Thanh công thức** (có ký hiệu _fx_): hiện và sửa nội dung thật của ô (công thức, ngày, số).
- **Lưới ô:** bấm để chọn ô, kéo để chọn vùng; bấm tiêu đề hàng hoặc cột để chọn cả hàng, cả cột.
- **Dải tab trang tính** (dưới cùng): bấm để chuyển trang tính; bấm chuột phải để **Thêm trang tính**, **Xoá trang
  tính**, **Đổi tên trang tính**, **Màu tab…**, **Bảo vệ trang tính**.
- **Thanh trạng thái:** khi vùng chọn có từ hai ô có dữ liệu trở lên, hiện **Trung bình**, **Số lượng**, **Tổng**
  (bấm chuột phải để bật thêm **Đếm số**, **Nhỏ nhất**, **Lớn nhất**). Các hàng đang bị bộ lọc ẩn không được tính.

Menu chuột phải trên ô có: cắt, sao chép, dán, **Dán đặc biệt**, **Chèn hàng**, **Chèn cột**, **Xoá hàng**, **Xoá
cột**, **Xoá nội dung**, sắp xếp, **Bộ lọc**, **Định dạng ô**, **Xác thực dữ liệu**, **Ảnh trong ô**.

## Nhập dữ liệu và công thức

1. Chọn ô và gõ. Nhấn `Enter` để ghi và xuống ô dưới, `Tab` để ghi và sang ô phải, `Esc` để huỷ.
2. Nhấn `F2` để sửa ô đang có nội dung; `Backspace` để xoá và gõ lại; `Delete` để xoá nội dung vùng chọn.
3. Công thức bắt đầu bằng dấu `=`, ví dụ `=SUM(B2:B10)`.
4. Khi đang gõ công thức, nhấn `F4` để đổi kiểu tham chiếu của địa chỉ tại con trỏ (`B2` → `$B$2` → `B$2` → `$B2`).
5. Kéo ô vuông nhỏ ở góc dưới phải vùng chọn (**Kéo để điền tự động**) để chép công thức hoặc điền dãy.
   `Ctrl+D` điền xuống, `Ctrl+R` điền sang phải.

Ngày gõ theo kiểu Việt Nam được nhận là ngày: `15/3/2024`, `15 tháng 3 năm 2024`, `tháng 3/2024`, và cả
`15 Mar 2024`.

**Mảng động:** công thức trả về nhiều giá trị tự tràn sang các ô bên cạnh. Có đủ `LET`, `LAMBDA` và các hàm mảng
như `FILTER`, `SORT`, `UNIQUE`, `SEQUENCE`.

> **Hạn chế:** công thức mảng kiểu cũ (nhập cho một vùng cố định) được tính như mảng động. Liên kết tới sổ tính
> khác được giữ nguyên trong tệp nhưng chưa cập nhật, chưa sửa nguồn, chưa ngắt liên kết được.

## Hàm

BlueSheet tính được **508 trên 515 hàm** của danh mục hàm bảng tính phổ biến, gõ bằng tên tiếng Anh quen thuộc.
Danh sách đầy đủ từng hàm: [`../parity/functions.md`](../parity/functions.md).

| Nhóm                     | Số hàm   | Ví dụ                                            |
| ------------------------ | -------- | ------------------------------------------------ |
| Toán và lượng giác       | 80       | `SUM`, `SUMIFS`, `ROUND`, `MOD`, `SUMPRODUCT`    |
| Thống kê                 | 110      | `AVERAGE`, `COUNTIFS`, `MEDIAN`, `STDEV.S`       |
| Tra cứu và tham chiếu    | 40       | `VLOOKUP`, `XLOOKUP`, `INDEX`, `MATCH`, `FILTER` |
| Văn bản                  | 46       | `LEFT`, `TEXT`, `TEXTJOIN`, `TEXTSPLIT`          |
| Logic                    | 19       | `IF`, `IFS`, `AND`, `OR`, `IFERROR`, `LET`       |
| Ngày và giờ              | 25       | `TODAY`, `EDATE`, `NETWORKDAYS`, `DATEDIF`       |
| Tài chính                | 55       | `PMT`, `NPV`, `IRR`, `XIRR`                      |
| Kỹ thuật                 | 54       | `CONVERT`, `DEC2BIN`, `IMSUM`                    |
| Thông tin                | 21       | `ISBLANK`, `ISNUMBER`, `CELL`                    |
| Cơ sở dữ liệu            | 12       | `DSUM`, `DCOUNT`, `DGET`                         |
| Tương thích (tên cũ)     | 42       | `RANK`, `STDEV`, `CONCATENATE`                   |
| Web                      | 3        | `ENCODEURL`, `FILTERXML`, `WEBSERVICE`           |
| Cube                     | 0 trên 7 | xem hạn chế bên dưới                             |
| Hàm riêng của BlueOffice | 1        | `BO.DOCSO`                                       |

### BO.DOCSO – đọc số thành chữ tiếng Việt

`BO.DOCSO(số, [đơn_vị])` trả về số viết bằng chữ tiếng Việt, viết hoa chữ đầu, kèm đơn vị. Đơn vị mặc định là
"đồng"; truyền chuỗi rỗng `""` nếu không cần đơn vị. Phần thập phân được đọc sau chữ "phẩy"; số âm bắt đầu bằng "âm".
Số có giá trị tuyệt đối từ 10^15 trở lên trả về lỗi `#NUM!`.

| Công thức            | Kết quả                                |
| -------------------- | -------------------------------------- |
| `=BO.DOCSO(1500000)` | Một triệu năm trăm nghìn đồng          |
| `=BO.DOCSO(105)`     | Một trăm linh năm đồng                 |
| `=BO.DOCSO(2025)`    | Hai nghìn không trăm hai mươi lăm đồng |
| `=BO.DOCSO(0)`       | Không đồng                             |

Các đối số của hàm ngăn cách bằng dấu phẩy, ví dụ `=IF(A1>0, "Có", "Không")`.

> **Hạn chế về hàm:**
>
> - 7 hàm Cube: tệp có sẵn công thức Cube vẫn giữ công thức và giá trị đã lưu; gõ mới trả về `#NAME?` (không có
>   nguồn dữ liệu OLAP).
> - `RTD`: giữ giá trị đã lưu trong tệp, không lấy dữ liệu mới.
> - `WEBSERVICE`: cần quản trị viên bật nguồn dữ liệu web; nếu chưa có thì trả về lỗi.
> - `GETPIVOTDATA` chưa đọc dữ liệu bảng tổng hợp thật (trả về `#REF!`); `IMAGE` trả về văn bản thay thế khi ảnh
>   chưa tải được.
> - `GROUPBY` chưa có tổng phụ nhiều cấp; `PIVOTBY` chưa nhận nhiều trường giá trị.

## Tab Trang đầu

### Tệp

**Sổ tính mới**, **Mở** (`Ctrl+O`), **Lưu .xlsx** (`Ctrl+S`), **Lưu thành…** (`Ctrl+Shift+S`), **Sửa mẫu**, **Chủ đề
từ tệp…**, **Lưu chủ đề…**, **Lưu .csv**, **Xuất PDF**, **Xuất PDF/A, PDF có thẻ…**, **In** (`Ctrl+P`).

### Chỉnh sửa

- **Dán** (`Ctrl+V`), **Cắt** (`Ctrl+X`), **Sao chép** (`Ctrl+C`), **Hoàn tác** (`Ctrl+Z`), **Làm lại** (`Ctrl+Y`).
- **Dán đặc biệt** (`Ctrl+Alt+V`): **Tất cả**, **Giá trị**, **Công thức**, **Định dạng**; tuỳ chọn **Chuyển vị**,
  **Bỏ qua ô trống**.
- **Sao chép định dạng:** chọn ô mẫu, bấm lệnh, rồi bấm ô hoặc kéo vùng cần áp dụng. `Esc` để thôi.

### Phông chữ và Căn lề

- **Đậm** (`Ctrl+B`), **Nghiêng** (`Ctrl+I`), **Gạch chân** (`Ctrl+U`), **Gạch ngang** (`Ctrl+5`), **Tăng cỡ chữ**,
  **Giảm cỡ chữ**, **Màu chữ**, **Màu nền**, **Viền**.
- **Căn trái**, **Căn giữa**, **Căn phải**, **Căn trên**, **Căn giữa dọc**, **Căn dưới**, **Xuống dòng** (chữ dài
  tự xuống dòng trong ô), **Gộp và căn giữa**, **Bỏ gộp ô**, **Tăng thụt lề**, **Giảm thụt lề**.

### Số

- Nút nhanh: **Tiền tệ (₫)**, **Phần trăm** (`Ctrl+Shift+%`), **Số có phân cách**, **Ngày (dd/MM/yyyy)**.
- **Định dạng số** liệt kê: **Chung**, **Số (1.234,56)**, **Số nguyên (1.234)**, **Tiền Việt (1.234 ₫)**, **Đô la Mỹ
  ($1.234,56)**, **Kế toán (₫)**, **Phần trăm**, **Ngày (dd/MM/yyyy)**, **Ngày dài**, **Giờ (hh:mm:ss)**, **Ngày
  giờ**, **Khoa học**, **Phân số**, **Văn bản**, **Tuỳ chỉnh**.

### Kiểu

- **Kiểu ô:** **Bình thường**, **Tốt**, **Xấu**, **Trung tính**, **Tiêu đề**, **Đầu mục 1**, **Đầu mục 2**,
  **Tổng**, **Ghi chú**.
- **Định dạng ô** (`Ctrl+1`): hộp thoại có các thẻ **Số**, **Căn lề**, **Phông chữ**, **Viền**, **Tô nền**.
- **Xoá định dạng**.

#### Định dạng có điều kiện

1. Chọn vùng ô, bấm **Trang đầu › Kiểu › Định dạng có điều kiện**.
2. Chọn loại quy tắc: **Giá trị ô**, **Ô chứa văn bản**, **Dùng công thức**, **Nhóm đầu / cuối**, **Giá trị trùng
   lặp**, **Thang màu**, **Thanh dữ liệu**, **Bộ biểu tượng**.
3. Đặt điều kiện và định dạng, rồi xác nhận.
4. **Quản lý quy tắc** mở hộp thoại **Quy tắc định dạng có điều kiện** để sửa, đổi thứ tự, bật **Dừng nếu đúng**,
   xoá quy tắc. **Xoá quy tắc điều kiện** gỡ quy tắc khỏi vùng chọn.

### Ô

**Chèn hàng**, **Chèn cột**, **Xoá hàng**, **Xoá cột**, **Tự giãn cột**, **Tự giãn hàng**, **Ẩn hàng**, **Hiện
hàng**, **Ẩn cột**, **Hiện cột**.

### Giọng nói, Sửa, Trang tính

- **Đọc chính tả:** xem [README.md](README.md#91-đọc-chính-tả-nói-thành-chữ).
- **Tự tính tổng** (**Σ**, `Alt+=`) và mũi tên **▾** các phép tính nhanh khác: xem
  [Thư viện hàm – Tự tính tổng](#thư-viện-hàm--tự-tính-tổng).
- **Núm điền** (ô vuông nhỏ ở góc dưới phải vùng chọn): kéo để điền tiếp; **bấm đúp** để điền xuống tới hết khối
  dữ liệu liền kề.
- **Xoá nội dung** (`Delete`), **Điền xuống** (`Ctrl+D`), **Điền sang phải** (`Ctrl+R`), **Tìm** (`Ctrl+F`), **Tìm
  và thay thế** (`Ctrl+H`), **Đi tới** (`Ctrl+G` hoặc `F5`).
- **Thêm trang tính**, **Đổi tên trang tính**, **Xoá trang tính**.

## Tab Chèn

### Biểu đồ

1. Chọn vùng dữ liệu (gồm cả hàng tiêu đề).
2. Bấm **Chèn › Biểu đồ › Chèn biểu đồ** và chọn loại.
3. Chọn biểu đồ để hiện tab **Thiết kế biểu đồ** ([xem bên dưới](#tab-thiết-kế-biểu-đồ)); bấm **Sửa biểu đồ** để
   đổi loại, **Tiêu đề**, **Tiêu đề trục ngang**, **Tiêu đề trục dọc**, vùng dữ liệu. **Xoá biểu đồ** để bỏ.

Các loại biểu đồ tạo được:

- Cơ bản: **Cột**, **Cột chồng**, **Cột chồng 100 %**, **Thanh ngang** (và dạng chồng), **Đường**, **Vùng**,
  **Tròn**, **Vành khuyên**, **Phân tán (XY)**, **Kết hợp (cột + đường)**.
- Nâng cao: **Tròn tách miếng**, **Tròn của tròn**, **Thanh của tròn**, **Bong bóng**, **Radar** (thường, có điểm
  đánh dấu, tô màu), **Chứng khoán (cao–thấp–đóng)**, **Chứng khoán có thanh tăng/giảm**, **Bề mặt**.
- Mở rộng: **Thác nước**, **Bản đồ cây**, **Hình tia nắng**, **Tần suất (histogram)**, **Pareto**, **Hộp và râu**,
  **Phễu**.
- 3D: **Cột 3D**, **Trụ 3D**, **Nón 3D**, **Kim tự tháp 3D**, **Thanh ngang 3D**, **Đường 3D**, **Vùng 3D**, **Tròn
  3D**, **Bề mặt 3D**.

> **Hạn chế:** chưa có biểu đồ chứng khoán kèm khối lượng; chưa có biểu đồ bản đồ; biểu đồ mở rộng (thác nước, bản
> đồ cây…) chưa có nhãn dữ liệu và chưa nhận kiểu / màu nhanh.

- **Biểu đồ đề xuất:** xếp hạng các loại biểu đồ hợp với dữ liệu đã chọn (theo thời gian → đường; vài nhóm →
  cột, một dãy số dương ít nhóm → tròn; hai cột số → phân tán; nhiều nhóm hoặc nhãn dài → thanh ngang; nhiều cột
  nhãn → bản đồ cây / hình nắng), có ảnh xem trước; trỏ vào để xem lý do.
- **Phân tích nhanh:** khi chọn nhiều ô, nút nhỏ ở góc dưới phải vùng chọn mở các thẻ **Định dạng** (thanh dữ
  liệu, thang màu, bộ biểu tượng, trên trung bình, 10 % cao nhất, xóa định dạng), **Biểu đồ** (biểu đồ đề xuất),
  **Tổng** (nhóm **Thêm hàng tổng bên dưới dữ liệu** rồi nhóm **Thêm cột tổng bên phải dữ liệu**: tổng, trung bình,
  đếm, % tổng, lũy kế — biểu tượng tô dải ô sẽ được thêm, trỏ chuột để xem trước công thức và ô đích; một bước
  **Hoàn tác**; các ô bên cạnh phải trống; vùng chọn chỉ có một cột thì nhóm cột bên phải bị tắt, chỉ có một hàng
  thì nhóm hàng bên dưới bị tắt, vì kết quả chỉ lặp lại từng giá trị),
  **Bảng** (bảng, bảng tổng hợp) và **Biểu đồ nhỏ** (đường, cột, thắng/thua).

> **Hạn chế:** thẻ Định dạng chưa xem trước trực tiếp trên ô khi trỏ chuột (chỉ có mô tả).

### SmartArt, Biểu đồ thu nhỏ, Ảnh trong ô, Ô đánh dấu

- **SmartArt:** chèn sơ đồ; cách dùng giống BlueDoc ([bluedoc.md](bluedoc.md#smartart)).
- **Biểu đồ thu nhỏ:** biểu đồ nằm gọn trong một ô. Trong hộp thoại **Chèn biểu đồ thu nhỏ**, chọn vùng dữ liệu và
  ô đặt. Chỉnh bằng tab **Biểu đồ thu nhỏ** ([xem bên dưới](#tab-biểu-đồ-thu-nhỏ)).
- **Ảnh trong ô:** đặt một ảnh vào trong ô đang chọn. **Văn bản thay thế của ảnh** ghi mô tả cho trình đọc màn
  hình.
- **Ô đánh dấu:** biến các ô đang chọn thành ô đánh dấu; nhấn phím cách để đánh dấu / bỏ đánh dấu.
- **Bình luận:** mở ngăn **Bình luận** ([README.md](README.md#52-chú-thích-bình-luận)); gõ rồi nhấn `Ctrl+Enter`
  để gửi.

> **Hạn chế:** BlueSheet chưa có ảnh nổi trên lưới (chỉ ảnh trong ô). Biểu đồ thu nhỏ và ảnh trong ô chưa đồng bộ
> khi cộng tác. Biểu đồ thu nhỏ chưa tự dịch vị trí khi chèn / xoá hàng, cột.

### Bảng

1. Chọn vùng dữ liệu, bấm **Chèn › Bảng › Bảng**.
2. Vùng trở thành bảng có nút lọc, kiểu màu và tên. Khi ô đang chọn nằm trong bảng, tab **Thiết kế bảng** hiện ra
   ([xem bên dưới](#tab-thiết-kế-bảng)).
3. **Slicer** chèn bảng nút bấm để lọc bảng theo một cột.

Nhóm này còn có **Bảng tổng hợp (Pivot)** — xem [Tab Bảng tổng hợp](#tab-bảng-tổng-hợp).

### Điều khiển biểu mẫu

Chèn **Nút bấm**, **Ô đánh dấu**, **Nút chọn một**, **Hộp thả xuống**, **Hộp danh sách**, **Thanh trượt giá trị**,
**Nút tăng giảm**, **Khung nhóm**, **Nhãn chữ**. Chọn điều khiển rồi bấm **Thiết lập điều khiển…** để đặt ô liên kết,
vùng danh sách, giá trị nhỏ nhất / lớn nhất. Giá trị điều khiển và ô liên kết đồng bộ hai chiều. **Xoá điều khiển**
để bỏ.

> **Hạn chế:** điều khiển chưa đổi kích thước được; hộp danh sách chỉ chọn một mục; điều khiển chưa được in và chưa
> có trong PDF. Nút bấm gán macro không chạy macro. Điều khiển ActiveX trong tệp có sẵn chỉ được giữ nguyên.

### Ký hiệu

**Ký tự đặc biệt** mở bảng ký tự Unicode.

## Tab Bảng tổng hợp

### Tạo bảng tổng hợp

1. Chọn một ô trong vùng dữ liệu có hàng tiêu đề.
2. Bấm **Chèn › Bảng › Bảng tổng hợp (Pivot)**.
3. Trong hộp thoại **Tạo bảng tổng hợp**, kiểm tra **Vùng dữ liệu nguồn**, chọn nơi đặt: **Trang tính mới** hoặc
   **Trang tính hiện có**.
4. Ngăn **Trường của bảng tổng hợp** hiện ra. Kéo trường vào bốn vùng **Bộ lọc**, **Cột**, **Hàng**, **Giá trị**.
5. Bấm vào trường trong vùng **Giá trị** để mở **Cài đặt trường giá trị** (kiểu tính: tổng, đếm, trung bình…; cách
   hiển thị giá trị).

### Các nhóm lệnh

| Nhóm               | Lệnh                                                                                                                                              |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Dữ liệu**        | **Làm mới bảng tổng hợp**, **Làm mới tất cả**, **Đổi vùng dữ liệu nguồn**                                                                         |
| **Bố cục báo cáo** | **Dạng thu gọn**, **Dạng phác thảo**, **Dạng bảng**, **Dòng tổng cộng (tổng của mỗi cột)**, **Cột tổng cộng (tổng của mỗi hàng)**                 |
| **Tính toán**      | **Cài đặt trường**, **Trường tính toán**, **Mục tính toán**, **Nhóm mục đã chọn**, **Bỏ nhóm**, **Giá trị trên hàng**, **Tùy chọn bảng tổng hợp** |
| **Thao tác**       | **Biểu đồ tổng hợp**, **Danh sách trường**, **Di chuyển bảng tổng hợp**, **Xoá bảng tổng hợp**                                                    |

Sau khi sửa dữ liệu nguồn, bấm **Làm mới bảng tổng hợp** để cập nhật.

> **Hạn chế:** chưa bấm đúp vào số để xem các dòng chi tiết. **Biểu đồ tổng hợp** chưa có nút lọc ngay trên biểu
> đồ. Slicer của bảng tổng hợp và dòng thời gian (lọc theo **Năm**, **Quý**, **Tháng**, **Ngày**) có sẵn trong tệp
> thì hiển thị và lọc được, nhưng chưa chèn mới được. Bảng tổng hợp chưa đồng bộ đầy đủ khi cộng tác.

## Tab Công thức

### Thư viện hàm – Tự tính tổng

**Tự tính tổng** (nút **Σ**, phím `Alt+=`; cũng có trong nhóm **Sửa** của tab **Trang đầu**) đặt công thức tổng
đúng chỗ mà không phải gõ:

| Đang chọn | Kết quả |
| --- | --- |
| Một **cột** số, ví dụ `C2:C4` | `=SUM(C2:C4)` vào ô trống đầu tiên **bên dưới** (`C5`), ghi ngay |
| Một **hàng** số, ví dụ `B1:D1` | `=SUM(B1:D1)` vào ô trống đầu tiên **bên phải** (`E1`) |
| Một **khối** nhiều hàng, nhiều cột | Mỗi cột một tổng ở hàng ngay dưới khối |
| Khối kèm một hàng trống ở dưới **và** một cột trống bên phải | Điền cả hàng tổng, cột tổng và tổng chung ở góc |
| Cột số kèm ô trống ở cuối, ví dụ `C2:C5` với `C5` trống | Tổng vào chính ô trống đó (`C5`) |
| Các ô trống ngay dưới (hoặc bên phải) dữ liệu | Mỗi ô một tổng của các số phía trên (hoặc bên trái) |
| **Một ô trống** | Đề xuất `=SUM(vùng)` trong ô ở chế độ sửa, vùng được bôi sẵn để chỉnh; `Enter` nhận, `Esc` bỏ. Ưu tiên dãy số liền **phía trên** (dừng ở ô chữ hoặc ô trống, không lấy tiêu đề), không có thì lấy dãy **bên trái**; không tìm thấy số thì là `=SUM()` với con trỏ giữa hai dấu ngoặc |
| Ô đã có công thức tổng | Mở lại công thức đó với vùng được bôi sẵn |

Sau khi ghi, vùng chọn mở rộng để gồm cả ô tổng. Mỗi lần **Tự tính tổng** là một bước **Hoàn tác**. Ô tổng lấy định
dạng số của các ô được cộng. Trong danh sách đang lọc, công thức là `=SUBTOTAL(9,vùng)` để chỉ cộng các hàng đang
hiện. Trong **bảng**, tổng nằm ở **Hàng tổng** của bảng (tự bật nếu chưa có).

Mũi tên **▾** cạnh nút Σ: **Trung bình** (`AVERAGE`), **Đếm số** (`COUNT`), **Lớn nhất** (`MAX`), **Nhỏ nhất**
(`MIN`) — hoạt động y như **Tổng** — và **Hàm khác…** mở hộp **Chèn hàm** (`Shift+F3`): gõ để tìm, `Enter` để bắt
đầu công thức `=TÊN_HÀM()` trong ô đang chọn.

> **Hạn chế:** khi đang sửa đề xuất, chưa dùng phím mũi tên hay kéo chuột để chọn lại vùng (gõ địa chỉ thay vào
> phần đang bôi); hộp **Chèn hàm** chưa có mô tả và ô nhập đối số cho từng hàm; khi cửa sổ hẹp, nhóm **Sửa** của
> tab **Trang đầu** thu lại thành một nút thả xuống, nút Σ nằm trong đó.

### Kiểm tra công thức

- **Truy ô nguồn:** vẽ mũi tên từ các ô mà công thức đang dùng. **Truy ô phụ thuộc:** vẽ mũi tên tới các ô dùng ô
  này. **Xoá mũi tên** để gỡ.
- **Kiểm tra lỗi:** hộp thoại **Kiểm tra lỗi** đi qua từng ô có lỗi hoặc công thức đáng ngờ.
- **Tính từng bước:** hộp thoại **Tính công thức từng bước** cho thấy từng phần của công thức được tính ra sao.
- **Cửa sổ theo dõi:** ghim các ô cần theo dõi giá trị dù đang ở trang tính khác.
- **Hiện công thức** (`` Ctrl+` ``, trong tab **Xem**): hiện công thức thay cho kết quả.

### Tính toán

- **Tuỳ chọn tính toán:** **Tự động**; **Tự động, trừ bảng dữ liệu**; **Chỉ khi yêu cầu (F9)**; **Tính trước khi
  lưu**; **Cho phép tính lặp** (cho tham chiếu vòng) với **Số lần lặp tối đa**; **Tính theo độ chính xác hiển thị**.
- **Tính ngay** (`F9`), **Tính trang tính** (`Shift+F9`), **Tính lại toàn bộ** (`Ctrl+Alt+F9`).

> **Hạn chế:** ở chế độ tính thủ công, chèn hàng vẫn tính lại toàn bộ. Mũi tên truy vết bỏ qua tham chiếu gián tiếp
> (`INDIRECT`, `OFFSET`). Danh sách ở **Cửa sổ theo dõi** chưa lưu vào tệp. **Tính theo độ chính xác hiển thị** chưa
> làm tròn các hằng số đã lưu.

## Tab Dữ liệu

### Lấy & biến đổi

Nhập dữ liệu từ nguồn ngoài thành bảng, có thể làm mới về sau.

1. Chọn nguồn: **Từ CSV/văn bản**, **Từ JSON**, **Từ XML**, **Từ sổ làm việc**, **Từ thư mục**, **Từ web**.
2. Trong **Trình biên tập truy vấn**, thêm các bước biến đổi (chọn cột, đổi kiểu dữ liệu, lọc dòng, tách cột…) và
   xem trước kết quả.
3. Đặt **Tên** truy vấn rồi nạp vào trang tính.
4. **Truy vấn & kết nối** mở ngăn liệt kê các truy vấn để sửa, làm mới, xoá. **Làm mới tất cả** (`Ctrl+Alt+F5`) nạp
   lại mọi truy vấn.

**Từ web** trên bản web cần quản trị viên bật proxy tải trang: [`../deploy/web-data.md`](../deploy/web-data.md).

> **Hạn chế:** truy vấn do phần mềm khác tạo được giữ nguyên trong tệp nhưng không sửa được ở đây. Cột bạn tự thêm
> vào bảng kết quả sẽ mất khi làm mới; đổi tên bảng kết quả làm mất liên kết với truy vấn.

### Mô hình dữ liệu

- **Quản lý mô hình:** chọn các bảng tham gia mô hình (hộp thoại **Bảng trong mô hình dữ liệu**).
- **Quan hệ:** nối hai bảng qua cột khoá.
- **Số đo:** định nghĩa phép tính dùng lại trong bảng tổng hợp (hộp thoại **Số đo và cột tính toán**).
- **Bảng tổng hợp từ mô hình:** tạo bảng tổng hợp lấy trường từ nhiều bảng.

> **Hạn chế:** bảng tổng hợp từ mô hình chưa nhóm mục, chưa có trường tính toán, chưa đổi nguồn, chưa lọc giá trị
> theo số đo, chưa có slicer. Mô hình dữ liệu do phần mềm khác tạo chỉ đọc.

### Sắp xếp & lọc

- **Sắp xếp A → Z**, **Sắp xếp Z → A:** sắp theo cột đang chọn.
- **Sắp xếp tuỳ chỉnh:** nhiều cấp (**Thêm cấp**), **Sắp xếp theo** **Giá trị ô**, **Màu ô**, **Màu chữ**, **Biểu
  tượng định dạng**; **Theo danh sách tuỳ chỉnh**; **Sắp xếp từ trái sang phải**.
- **Bộ lọc** (`Ctrl+Shift+L`): bật nút lọc ở hàng tiêu đề. Bấm nút để **Lọc theo giá trị**, theo **Điều kiện**,
  **Lọc theo màu**, **Lọc theo biểu tượng**, **Nhóm giá trị** (lớn nhất / nhỏ nhất), **Lọc theo ngày / trung bình**
  (**Hôm nay**, **Tuần này**, **Tháng trước**, **Trên trung bình**…).
- **Lọc nâng cao:** nhập **Vùng dữ liệu** và **Vùng điều kiện**; chọn **Lọc ngay trên vùng dữ liệu** hoặc **Chép
  kết quả sang chỗ khác**; tuỳ chọn **Bỏ các bản ghi trùng**.
- **Hiện lại mọi hàng:** bỏ mọi điều kiện lọc.

### Công cụ dữ liệu

| Lệnh                              | Dùng để                                                                                                                      |
| --------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| **Xác thực dữ liệu**              | Giới hạn giá trị nhập: **Danh sách**, **Số nguyên**, **Số thập phân**, **Ngày**, **Độ dài văn bản**, **Công thức tuỳ chỉnh** |
| **Điền nhanh** (`Ctrl+E`)         | Gõ một hai ví dụ ở cột bên cạnh, BlueSheet đoán mẫu và điền các dòng còn lại                                                 |
| **Tìm giá trị đích**              | Tìm giá trị một ô đầu vào để ô công thức đạt một số cho trước                                                                |
| **Kịch bản**                      | Lưu các bộ giá trị đầu vào, chuyển qua lại, **Tạo trang so sánh**                                                            |
| **Bảng dữ liệu**                  | Tính một công thức với nhiều giá trị của một hoặc hai ô đầu vào                                                              |
| **Giải bài toán tối ưu**          | Tìm giá trị các ô biến để ô mục tiêu lớn nhất / nhỏ nhất / đạt một số, có ràng buộc                                          |
| **Tách văn bản thành cột**        | Tách một cột thành nhiều cột theo dấu phân cách                                                                              |
| **Xóa hàng trùng lặp**            | Bỏ các hàng trùng nhau theo những cột bạn chọn                                                                               |
| **Hợp nhất dữ liệu**              | Cộng gộp nhiều vùng dữ liệu vào một vùng                                                                                     |
| **Đánh dấu dữ liệu không hợp lệ** | Khoanh các ô vi phạm quy tắc xác thực; **Xóa đánh dấu dữ liệu không hợp lệ** để gỡ                                           |

Ô có xác thực kiểu **Danh sách** hiện mũi tên chọn; nhấn `Alt+↓` để mở danh sách.

**Giải bài toán tối ưu** có ba **Phương pháp giải**: **Tuyến tính (đơn hình)**, **Phi tuyến trơn**, **Tiến hoá**;
hỗ trợ ràng buộc nguyên, nhị phân; có thể **Ghi báo cáo ra trang tính mới** (**Kết quả**, **Độ nhạy**, **Giới hạn**).

> **Hạn chế:** **Điền nhanh** chưa gợi ý mờ khi đang gõ, chưa điền theo hàng. Vòng khoanh dữ liệu không hợp lệ
> không lưu vào tệp.

### Dàn ý

**Tổng phụ theo nhóm** chèn dòng tổng sau mỗi nhóm giá trị. **Nhóm hàng**, **Bỏ nhóm hàng**, **Nhóm cột**, **Bỏ
nhóm cột**, **Dàn ý từ công thức**, **Xóa dàn ý**, **Thu gọn tất cả nhóm**, **Mở rộng tất cả nhóm**.

> **Hạn chế:** chưa có thanh dấu +/− cạnh lưới để thu gọn từng nhóm; dùng hai lệnh thu gọn / mở rộng tất cả.

### Phân tích

**Phân tích dữ liệu** mở danh sách 19 công cụ thống kê: **Anova** (một yếu tố, hai yếu tố có lặp, hai yếu tố không
lặp), **Tương quan**, **Hiệp phương sai**, **Thống kê mô tả**, **Làm trơn hàm mũ**, **Kiểm định F**, **Phân tích
Fourier**, **Biểu đồ tần suất**, **Trung bình trượt**, **Sinh số ngẫu nhiên**, **Thứ hạng và phân vị**, **Hồi
quy**, **Lấy mẫu**, **Kiểm định t** (ba dạng), **Kiểm định z**. Chọn công cụ, nhập vùng dữ liệu và nơi ghi kết quả.

> **Hạn chế:** các công cụ chưa vẽ biểu đồ đầu ra (tần suất, phần dư, đường khớp).

## Tab Bố cục trang

- **Thiết lập trang:** hộp thoại có các thẻ **Trang** (hướng, khổ giấy, co giãn cho vừa số trang), **Lề**, **Đầu /
  chân trang**, **Trang tính** (**Hàng lặp lại ở đầu trang**, **Cột lặp lại bên trái**, in đường lưới, **In số hàng
  và chữ cột**, thứ tự trang).
- **Đặt vùng in** / **Xoá vùng in:** chỉ in vùng ô đang chọn.
- **Chèn ngắt trang**, **Bỏ ngắt trang**, **Đặt lại mọi ngắt trang**.
- **Xem trước khi in**, **Xuất PDF**.

Hộp thoại In: [README.md](README.md#71-in).

> **Hạn chế:** đầu / chân trang chưa in được ảnh; bình luận chưa in "như hiển thị trên trang tính".

## Tab Xem lại

### Bảo vệ

- **Khoá ô:** đánh dấu ô bị khoá hay không (chỉ có tác dụng khi trang tính được bảo vệ).
- **Bảo vệ trang tính:** nhập **Mật khẩu (không bắt buộc)** và chọn những việc người dùng vẫn được làm: **Chọn ô
  bị khoá**, **Chọn ô không khoá**, **Định dạng ô**, **Chèn hàng**, **Xoá cột**, **Sắp xếp**, **Dùng Lọc tự động**,
  **Dùng bảng tổng hợp**… **Bỏ bảo vệ trang tính** để gỡ.
- **Bảo vệ sổ tính:** khoá cấu trúc (không thêm, xoá, đổi tên, di chuyển trang tính). **Bỏ bảo vệ sổ tính** để gỡ.
- **Lưu kèm mật khẩu:** đặt mật khẩu mở tệp.
- **Bảo vệ tệp:** bản cuối, hạn chế ghi, nhãn mức độ mật ([README.md](README.md#61-bảo-vệ-tệp)), và **Vùng được sửa
  khi trang tính bị khoá**: nhập **Tên vùng**, **Ô** (ví dụ `B2:D9`), **Mật khẩu vùng**, bấm **Thêm vùng**.

### Kiểm tra, Phiên bản, Soát lỗi, Giọng nói, Ngôn ngữ

- **Kiểm tra tài liệu:** [README.md](README.md#111-kiểm-tra-tài-liệu).
- **Hiện thay đổi:** so sánh với phiên bản trước hoặc với một tệp ([README.md](README.md#53-lịch-sử-phiên-bản)).
- **Soát chính tả và ngữ pháp…** (`F7`) soát mọi ô văn bản (bỏ qua công thức, số, ngày); **Kiểm tra chính tả khi
  gõ** gạch dưới từ sai khi bạn đang gõ trong ô.
- **Đọc to**, **Dịch**, **Dịch ô chọn sang cột mới**, **Dịch ô chọn tại chỗ**:
  [README.md](README.md#9-giọng-nói-và-dịch).

## Tab Xem

- **Dạng xem sổ làm việc:**
  - **Thường**;
  - **Xem ngắt trang:** thấy vùng từng trang in; kéo đường ngắt trang để dời hoặc đặt ngắt trang;
  - **Bố cục trang:** thấy trang tính như khi in, kèm vùng đầu trang, chân trang (bấm để sửa);
  - **Dạng xem tùy chỉnh…:** **Lưu dạng xem hiện tại** (tuỳ chọn **Gồm thiết lập in**, **Gồm hàng, cột ẩn và bộ
    lọc**), **Hiện** lại hoặc **Xóa**.
- **Hiển thị › Hiện công thức** (`` Ctrl+` ``).
- **Dạng xem trang tính › Dạng xem tạm…:** giữ bộ lọc / sắp xếp riêng của bạn trên máy này trong phiên làm việc.
- **Cố định:** **Cố định ô** (giữ các hàng phía trên và cột bên trái ô đang chọn), **Cố định hàng đầu**, **Cố định
  cột đầu**, **Bỏ cố định**.
- **Thu phóng:** **Phóng to**, **Thu nhỏ**.
- **Tự động hóa:** **Trình soạn BlueScript**, **Ghi thao tác**
  ([README.md](README.md#10-tự-động-hóa-bằng-bluescript)).

> **Hạn chế:** chưa gõ trực tiếp vào ô ở dạng xem **Bố cục trang**. Danh sách hàng, cột ẩn của dạng xem tùy chỉnh
> chỉ được giữ đến khi đóng sổ tính.

## Tab ngữ cảnh

### Tab Thiết kế biểu đồ

Hiện khi chọn biểu đồ: **Bố cục biểu đồ** (bố cục nhanh), **Kiểu biểu đồ** (12 kiểu và các bộ màu), **Mẫu** —
**Lưu thành mẫu…**, **Xuất mẫu (.crtx)**, áp mẫu đã lưu.

### Tab Thiết kế bảng

Hiện khi ô đang chọn nằm trong bảng.

| Nhóm                   | Lệnh                                                                                                                     |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| **Thuộc tính**         | **Tên bảng**, **Đổi tên**, **Đổi kích thước bảng**                                                                       |
| **Công cụ**            | **Chuyển thành vùng**, **Loại bỏ hàng trùng lặp**, **Chèn Slicer**                                                       |
| **Tùy chọn kiểu bảng** | **Hàng tiêu đề**, **Hàng tổng** (`Ctrl+Shift+T`), **Tô xen kẽ hàng**, **Nhấn cột đầu**, **Nhấn cột cuối**, **Tô xen kẽ cột**, **Nút lọc** |
| **Kiểu bảng**          | **Mẫu màu bảng**, **Hàm hàng tổng**                                                                                      |
| **Tự động**            | **Tự mở rộng bảng** (gõ ngay dưới / bên phải bảng thì bảng tự nới), **Tự điền cột công thức**                            |

> **Hạn chế:** slicer của bảng chưa đổi kích thước / di chuyển khung được, chưa xoá được slicer đã lưu trong tệp,
> chưa in và chưa có trong PDF.

### Tab Biểu đồ thu nhỏ

Hiện khi ô đang chọn có biểu đồ thu nhỏ.

- **Kiểu:** **Đường**, **Cột**, **Thắng/thua**.
- **Hiển thị:** **Điểm cao nhất**, **Điểm thấp nhất**, **Điểm âm**, **Điểm đầu**, **Điểm cuối**, **Điểm đánh dấu**.
- **Kiểu dáng › Màu và trục**.
- **Nhóm:** **Nhóm**, **Bỏ nhóm**, **Xóa**, **Xóa cả nhóm**.

### Tab SmartArt

Hiện khi chọn sơ đồ: **Ngăn văn bản**, **Xoá SmartArt**.

## Giới hạn về kích thước

Trang tính có tối đa 1.048.576 hàng × 16.384 cột. Với dữ liệu khoảng 1 triệu dòng, sắp xếp mất khoảng 4 giây và
dựng bảng tổng hợp mất 6–14 giây. Sổ tính 10 triệu ô mở được nhưng cần khoảng 4 GB bộ nhớ.

## Chưa hỗ trợ

Các tính năng sau **chưa có** trong BlueSheet:

- Hàm Cube tính trực tiếp từ nguồn OLAP; máy chủ dữ liệu thời gian thực cho `RTD`.
- Cập nhật, đổi nguồn, ngắt liên kết tới sổ tính khác.
- Biểu đồ bản đồ; biểu đồ chứng khoán kèm khối lượng.
- Ảnh nổi, hình vẽ, hộp văn bản trên lưới (phương trình trong hình cũng chưa hiển thị).
- Vẽ bằng bút; mô hình 3D (nội dung có sẵn trong tệp chỉ được giữ nguyên).
- Chèn mới slicer cho bảng tổng hợp và dòng thời gian; xem dòng chi tiết của bảng tổng hợp.
- Thanh dàn ý +/− cạnh lưới.
- Ký số và dòng chữ ký trên giao diện.
- Soát ngữ pháp và tự động sửa khi gõ.
- Phân tích nhanh và gợi ý biểu đồ.
- Các mục chung ở [README.md](README.md#12-chưa-hỗ-trợ-chung).
