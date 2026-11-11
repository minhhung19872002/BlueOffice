# BlueSheet — đối chiếu thao tác định dạng / cấu trúc với Microsoft Excel

Phạm vi: **vùng "format"** của đợt rà soát BlueSheet (tab Trang đầu, Bố cục trang, Xem; hàng /
cột; trang tính). Điều hướng, nhập liệu và chỉnh sửa ô thuộc vùng "nav"; điền, sắp xếp, lọc,
công thức và công cụ dữ liệu thuộc vùng "data". Tự tính tổng (Σ, Alt+=) và Phân tích nhanh đã
làm trước đó nên không nhắc lại.

Hành vi tham chiếu là của Excel (bản 365, máy cài vi-VN: dấu thập phân `,`, nhóm nghìn `.`,
ngày `dd/mm/yyyy`, tiền `₫`), viết lại từ tài liệu công khai và quan sát hộp đen — không dịch
ngược, không chép chuỗi giao diện. Mọi dòng "giống" / "đã sửa" có kiểm tra trong
`tests/e2e/specs/office-behaviour-sheet-format.spec.ts` (bấm nút thật / phím thật trên lưới) hoặc
kiểm thử đơn vị nêu ở cột cuối.

Kiểm thử:
- E2E: `cd tests/e2e && E2E_PORT=4430 pnpm exec playwright test office-behaviour-sheet-format --workers=1`
- Rust: `cargo test -p bo-sheet --lib numfmt::input`, `cargo test -p bo-engine --lib sheet::cmd::tests::typed`,
  `…::insert_and_delete_cells_shift_their_band`, `…::column_width_in_characters_and_row_height_in_points`
- Vitest: `packages/sheet-editor/test/homeFormat.test.ts`

## Bảng đối chiếu

### Trang đầu › Số — nhập liệu tự định dạng

| Thao tác | Excel | BlueSheet trước | Trạng thái |
|---|---|---|---|
| Gõ `10%` | 0,1, định dạng `0%`, hiện `10%` | 0,1 dạng Chung, hiện `0,1` | đã sửa |
| Gõ `12,5%` | 0,125, định dạng `0.00%` | 0,125 dạng Chung | đã sửa |
| Gõ `1.234` / `1.234,5` | số 1234 với `#,##0` / `#,##0.00` | số đúng nhưng dạng Chung (mất dấu nhóm) | đã sửa |
| Gõ `15.000 ₫`, `15000₫`, `₫12,5` | số với định dạng tiền tệ của vùng | **văn bản** | đã sửa (`#,##0 [$₫-42A]`, có thập phân thì `.00`) |
| Gõ `1e3` | 1000, định dạng `0.00E+00` | 1000 dạng Chung | đã sửa |
| Gõ `0 1/2`, `1 3/16` | 0,5 `# ?/?`; 1,1875 `# ??/??` | **văn bản** | đã sửa |
| Gõ `1/2` | ngày 1/2 **năm hiện tại** (vi-VN đọc ngày trước), `d-mmm` | văn bản `1/2` (dự án cố ý bỏ vì phụ thuộc ngày hôm nay) | đã sửa — năm lấy từ đồng hồ của máy chủ giao diện (`setNow`); không có đồng hồ thì vẫn là văn bản |
| Gõ `15 tháng 3`, `Mar 15` (en-US) | ngày năm hiện tại | văn bản | đã sửa |
| Gõ `(5)` | −5 | **văn bản** | đã sửa |
| Gõ `'0123` | văn bản `0123` | giống | giống |
| Gõ `1,5` | 1,5 dạng Chung | giống | giống |
| Ô đã có định dạng rồi mới gõ (`0.000` rồi gõ `10%`) | giữ định dạng của ô | giống (với ngày); nay cả các dạng mới | giống |
| Ô có dạng `@` (Văn bản) | giữ nguyên chữ gõ | giống | giống |
| Gõ `$12` ở vi-VN | (tuỳ cài đặt) | văn bản | còn khác — chỉ nhận ký hiệu tiền của vùng đang dùng |
| Gõ `1.5` ở vi-VN | phụ thuộc cài đặt, thường không phải số | thành 15 (quy tắc cũ bỏ dấu nhóm) | còn khác — giữ quy tắc cũ, không gán định dạng; cần quyết định riêng |
| Cột tự nới rộng khi gõ ngày / số dài vào cột rộng mặc định | có | không (hiện `####`) | còn khác — chưa làm |

### Trang đầu › Số — nút và phím tắt

| Thao tác | Excel | BlueSheet trước | Trạng thái |
|---|---|---|---|
| Hộp định dạng số (Chung, Số, Tiền tệ, Kế toán, Ngày ngắn/dài, Giờ, Phần trăm, Phân số, Khoa học, Văn bản, Thêm…) hiện loại của ô hiện hành | có | **không có hộp**, chỉ có menu | đã sửa (`sheet-number-format`) |
| Nút tiền (ký hiệu tiền) | định dạng **Kế toán** | định dạng Tiền tệ `#,##0 ₫` | đã sửa (Kế toán ₫) |
| Nút dấu phẩy | kiểu Comma (kế toán không ký hiệu, `-` cho số 0) | `#,##0.00` | đã sửa |
| Nút % / Ctrl+Shift+% | `0%` | giống | giống |
| Tăng / Giảm số thập phân | thêm / bớt một chữ số thập phân; ô Chung bắt đầu từ số chữ số đang hiện | **không có** | đã sửa (mọi phần của mã, kể cả `0%`, `E+00`, kế toán) |
| Ctrl+Shift+~ ! @ # $ ^ | Chung; `#,##0.00`; giờ AM/PM; ngày `d-mmm-yy`; tiền 2 số lẻ; khoa học | **không có** | đã sửa |
| Ctrl+1 Định dạng ô | 6 thẻ: Số, Căn lề, Phông, Viền, Tô nền, Bảo vệ | 5 thẻ (thiếu Bảo vệ) | đã sửa (thêm Bảo vệ: Khoá ô, Ẩn công thức) |
| Thẻ Số: số chữ số thập phân, mẫu xem trước | có | chỉ chọn loại / sửa mã | còn khác — dùng nút Tăng/Giảm số thập phân hoặc sửa mã |

### Trang đầu › Phông chữ

| Thao tác | Excel | BlueSheet trước | Trạng thái |
|---|---|---|---|
| Hộp tên phông, hộp cỡ chữ (gõ rồi Enter, chọn trong danh sách; Enter trả tiêu điểm về lưới) | có | **không có trên ribbon** (chỉ trong Ctrl+1) | đã sửa |
| Tăng / giảm cỡ chữ | theo dãy 8, 9, 10, 11, 12, 14, 16 … 72 | +1 / −1 điểm | đã sửa (trên 72 bước 10, dưới 8 bước 1) |
| Ctrl+B / I / U / 5 bật–tắt, trạng thái nút | theo ô hiện hành; vùng hỗn hợp: ô hiện hành đậm → bỏ đậm cả vùng | giống | giống |
| Ctrl+2 / 3 / 4 (đậm / nghiêng / gạch chân) | có | không | đã sửa |
| Màu nền / màu chữ dạng nút tách (phần chính tô màu dùng gần nhất; mặc định vàng / đỏ) | có | chỉ mở bảng màu | đã sửa |
| Ctrl+Shift+F / Ctrl+Shift+P mở Định dạng ô ở thẻ Phông | có | không | đã sửa |
| Bố cục nhóm Phông: hàng trên tên phông, cỡ, tăng/giảm cỡ; hàng dưới B I U, viền▾, nền▾, màu chữ▾ | có | 10 nút nhỏ, không hộp | đã sửa (ribbon nhận bố cục hai hàng `row`); nút Gạch ngang bỏ khỏi ribbon như Excel — vẫn có Ctrl+5 và Ctrl+1 |

### Trang đầu › Viền và Căn lề

| Thao tác | Excel | BlueSheet trước | Trạng thái |
|---|---|---|---|
| Viền dưới / trên / trái / phải trên một vùng | chỉ cạnh ngoài tương ứng của vùng | giống | giống (E2E kiểm B2:C3) |
| Danh sách viền: thêm Viền ngoài đậm, Viền dưới kép, Viền dưới đậm, Viền trên và dưới, "Thêm viền…" (mở Ctrl+1 thẻ Viền) | có | thiếu | đã sửa |
| Nút viền tách: phần chính lặp lại viền chọn gần nhất (mặc định viền dưới) | có | không | đã sửa |
| Ctrl+Shift+& / Ctrl+Shift+_ (viền ngoài / bỏ viền) | có | không | đã sửa |
| Gộp và căn giữa nhiều ô có dữ liệu | hỏi "chỉ giữ giá trị trên-trái"; Huỷ thì không đổi | gộp luôn, mất dữ liệu không báo | đã sửa |
| Bấm lại Gộp và căn giữa trên ô đã gộp | bỏ gộp | giống | giống |
| Gộp và căn giữa dạng nút tách: menu Gộp và căn giữa / Gộp theo hàng / Gộp ô (không căn giữa) / Bỏ gộp | có | có lệnh gộp theo hàng nhưng không có nút; thiếu Gộp ô | đã sửa |
| Tăng thụt lề trên ô Chung / căn giữa | ô thành căn trái + thụt lề | chỉ đặt thụt lề (không có tác dụng với ô Chung) | đã sửa |
| Hướng chữ (xoay lên/xuống 45°, chữ dọc, xoay 90°) | menu trên nút | **không có nút** (chỉ Ctrl+1) | đã sửa (chọn lại lựa chọn đang dùng → về ngang) |
| Căn trái / giữa / phải bật–tắt | có | giống | giống |
| Xuống dòng (Wrap) bật–tắt | có | giống | giống |

### Trang đầu › Ô, Sửa

| Thao tác | Excel | BlueSheet trước | Trạng thái |
|---|---|---|---|
| Ctrl++ trên ô / vùng | hộp thoại: Đẩy ô sang phải / xuống dưới / Toàn bộ hàng / cột (mặc định xuống) | **không làm gì** | đã sửa (lệnh động cơ mới `shiftCells`, tham chiếu trong dải ô đi theo, một bước hoàn tác) |
| Ctrl+- trên ô / vùng | hộp thoại xoá (mặc định kéo lên) | không làm gì | đã sửa (tham chiếu tới ô bị xoá → `#REF!`) |
| Ctrl++ / Ctrl+- khi chọn nguyên hàng / cột | chèn / xoá ngay | không làm gì | đã sửa |
| Menu chuột phải ô: "Chèn…" / "Xoá…" | mở hộp thoại trên | 4 mục chèn/xoá hàng/cột | đã sửa |
| Nhóm Ô: Chèn▾ (ô…, hàng, cột, trang tính), Xoá▾, Định dạng▾ (chiều cao hàng, độ rộng cột, tự giãn, ẩn/hiện, đổi tên / chuyển trang tính, khoá ô, Định dạng ô) | có | 10 nút rời | đã sửa (lệnh trong menu vẫn tìm được bằng Alt+Q) |
| Độ rộng cột… (ký tự, mặc định 8,43) / Chiều cao hàng… (pt, 15) | có, cả trong menu tiêu đề cột/hàng | **không có** | đã sửa (lệnh `sizeOf`, `resize` với `chars`) |
| Tự giãn cột bằng nhấp đúp biên tiêu đề (cả vùng chọn nhiều cột) | có | có | giống (không thêm E2E mới) |
| Ctrl+9 / Ctrl+0 ẩn hàng / cột; Ctrl+Shift+( hiện hàng | có | không có phím | đã sửa (Ctrl+Shift+) hiện cột: có phím nhưng Windows thường giữ tổ hợp này) |
| Kéo biên hàng/cột đã ẩn để hiện lại | có | chưa kiểm | còn khác — chưa kiểm/chưa làm |
| Xoá › Xoá tất cả / Xoá định dạng / Xoá nội dung | menu, "Xoá tất cả" một bước hoàn tác | chỉ hai nút riêng | đã sửa (thêm menu và Xoá tất cả); Xoá ghi chú / liên kết: còn khác |
| Chổi định dạng: nhấp một lần dán một lần; nhấp đúp giữ đến Esc | có | chỉ một lần | đã sửa |
| Delete chỉ xoá nội dung | có | giống | giống |

### Trang tính

| Thao tác | Excel | BlueSheet trước | Trạng thái |
|---|---|---|---|
| Nút + thêm trang tính **ngay sau** trang đang mở | có | thêm ở cuối | đã sửa (một bước hoàn tác) |
| Shift+F11 chèn trang tính **trước** trang đang mở | có | không | đã sửa |
| Ctrl+PgUp / Ctrl+PgDn sang trang trước / sau | có | không (PgUp/PgDn cuộn) | đã sửa |
| Nhấp đúp thẻ để đổi tên (cả tên được chọn sẵn) | có | **lỗi**: tiêu điểm trả về lưới ở khung hình sau cú nhấp đóng ngay ô đổi tên (thấy rõ khi máy chậm / tự động hoá) | đã sửa |
| Bấm + rồi gõ / Ctrl+PgUp ngay | bàn phím ở lưới | tiêu điểm kẹt ở nút + | đã sửa |
| Trang tính trong ribbon Trang đầu | không có nhóm riêng (nằm trong Ô › Chèn/Xoá/Định dạng) | nhóm "Trang tính" 5 nút | đã sửa (chuyển vào menu Ô, menu chuột phải thẻ, Alt+Q) |
| Kéo thẻ để đổi thứ tự | có | giống | giống (không thêm E2E) |
| Ctrl+kéo thẻ để sao chép trang tính; "Di chuyển hoặc sao chép…" | có | không | còn khác — động cơ chưa có lệnh sao chép trang (bảng, tên, bảng tổng hợp phải đổi tên kèm) |
| Màu thẻ (chuột phải) | có | có | giống |
| Ẩn / hiện trang tính | có | không có lệnh | còn khác — chưa làm |

### Xem / Bố cục trang

| Thao tác | Excel | BlueSheet trước | Trạng thái |
|---|---|---|---|
| Cố định ngăn tại ô hiện hành (hàng trên, cột trái) | có | có | giống |
| Khi đã cố định, nút thành "Bỏ cố định" | có | nút luôn cố định lại | đã sửa |
| Cố định hàng đầu / cột đầu | có | có | giống |
| Ctrl+lăn chuột phóng to/thu nhỏ lưới | có | trình duyệt phóng cả trang | đã sửa (bước 10 %, 50–200 %) |
| Ẩn/hiện đường lưới, tiêu đề; xem ngắt trang; xem trước khi in | có | có (Xem › Hiển thị, thanh trạng thái, Tệp › In) | giống ở mức cơ bản, không thêm E2E mới |
| Tạo bảng Ctrl+T / Ctrl+L: lấy vùng dữ liệu quanh ô, hỏi vùng và "hàng đầu là tên cột" (tự đoán) | có | tạo bảng ngay trên đúng vùng chọn, không hỏi, không có phím | đã sửa (bỏ chọn "hàng đầu là tên" → chèn hàng tên Cột1, Cột2… phía trên, một bước hoàn tác) |
| Định dạng có điều kiện (quy tắc nhanh), kiểu ô, thư viện Định dạng thành bảng, Tìm & Thay (Ctrl+F/H, tuỳ chọn) | có | có hộp thoại riêng | chưa rà sâu trong đợt này — xem mục "Còn khác" |
| Tab Trang đầu: không có nhóm Tệp | (Tệp nằm ở màn hình Tệp) | nhóm Tệp 11 nút | còn khác một phần — rút còn Mới / Mở / Lưu / Lưu thành / Xuất PDF / In (các lệnh khác ở màn hình Tệp); giữ vì nhiều kiểm thử và người dùng web quen |

## Phím tắt đã so

| Phím | Excel | BlueSheet |
|---|---|---|
| Ctrl+B / I / U / 5 | đậm / nghiêng / gạch chân / gạch ngang | giống |
| Ctrl+2 / 3 / 4 | đậm / nghiêng / gạch chân | đã thêm |
| Ctrl+1 | Định dạng ô | giống (nay 6 thẻ) |
| Ctrl+Shift+F, Ctrl+Shift+P | Định dạng ô › Phông | đã thêm |
| Ctrl+Shift+~ ! @ # $ % ^ | các định dạng số | đã thêm (trước chỉ có %) |
| Ctrl+Shift+& / _ | viền ngoài / bỏ viền | đã thêm |
| Ctrl++ / Ctrl+- | chèn / xoá ô (hỏi cách đẩy) | đã thêm |
| Ctrl+9 / Ctrl+0, Ctrl+Shift+( / ) | ẩn / hiện hàng, cột | đã thêm |
| Shift+F11 | chèn trang tính | đã thêm |
| Ctrl+T / Ctrl+L | tạo bảng | đã thêm |
| Ctrl+PgUp / Ctrl+PgDn | trang tính trước / sau | đã thêm |
| Ctrl+lăn chuột | phóng to / thu nhỏ | đã thêm |
| Ctrl+F / Ctrl+H | Tìm / Thay | giống (có sẵn) |
| F4 | lặp lại lệnh định dạng cuối | giống; các lệnh mới (số thập phân, viền, màu, định dạng số) cũng lặp được |

## Còn khác (lý do)

- **Sao chép trang tính** (Ctrl+kéo thẻ, "Di chuyển hoặc sao chép…") và **ẩn/hiện trang tính**:
  động cơ chưa có lệnh; sao chép cần đổi tên bảng, tên vùng cục bộ, nguồn bảng tổng hợp — để
  làm riêng.
- **Cột tự nới** khi gõ ngày/số không vừa cột rộng mặc định: chưa làm (hiện `####` như cũ).
- **Chèn/Xoá ô có đẩy**: định dạng có điều kiện, xác thực dữ liệu, bảng nằm trong dải ô bị đẩy
  chưa đi theo (ô, công thức, ô gộp, liên kết, chú thích thì đi theo).
- **Thẻ Số của Ctrl+1** chưa có ô "số chữ số thập phân" và mẫu xem trước.
- **Xoá ghi chú / Xoá liên kết** trong menu Xoá chưa có.
- **Ký hiệu tiền khác vùng** (`$12` khi dùng vi-VN) và `1.5` ở vi-VN giữ quy tắc cũ.
- Định dạng có điều kiện nhanh, thư viện kiểu ô, thư viện kiểu của Định dạng thành bảng, Tìm &
  Thay (Tìm tất cả, trong sổ/trang): có sẵn nhưng **chưa rà chi tiết** từng tuỳ chọn so với
  Excel trong đợt này.
- Ctrl+Shift+) (hiện cột): đã gắn, nhưng Windows thường giữ tổ hợp này cho bàn phím nên có máy
  không nhận (Excel cũng vậy).
- Ribbon Trang đầu ở cửa sổ 1366 px: nhóm Sửa vẫn gập thành nút thả xuống (như trước đợt này);
  ở 1920 px mọi nhóm hiện đủ.
