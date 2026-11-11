# BlueSheet — hành vi hằng ngày so với bảng tính quen dùng: mảng **dữ liệu**

Phạm vi: điền (fill handle, Ctrl+D/R, Flash Fill), sắp xếp, lọc tự động, công thức (lỗi, kiểm tra
lỗi, tính lại, tên đã đặt, dấu phân cách vi-VN, 33 hàm thông dụng), công cụ dữ liệu (xoá trùng,
tách cột, xác thực, nhóm, tổng phụ, biểu đồ nhanh, bảng tổng hợp). Hai mảng còn lại (di chuyển /
nhập liệu; định dạng / cấu trúc) do hai tài liệu `office-behaviour-sheet-nav.md` và
`office-behaviour-sheet-format.md` phụ trách. AutoSum (Σ, Alt+=) và tổng của Phân tích nhanh đã
làm trước đó — không làm lại.

Cách làm: hành vi tham chiếu viết từ tài liệu công khai và quan sát hộp đen (không dịch ngược, không
chép tài sản); mỗi hành động được chạy trong app web thật bằng Playwright (gõ phím trên lưới, kéo
tay cầm điền bằng chuột, bấm nút ribbon / hộp thoại) trên dữ liệu gõ vào, so sánh, rồi sửa. Mọi
hành vi đã sửa hoặc xác nhận có khẳng định trong
[`tests/e2e/specs/office-behaviour-sheet-data.spec.ts`](../../tests/e2e/specs/office-behaviour-sheet-data.spec.ts)
(nhóm theo thẻ ribbon; giá trị mong đợi tính tay từ dữ liệu của từng ca).

**Tổng kết:** 58 hành động — **giống: 30**, **đã sửa: 21**, **còn khác: 7** (lý do ở từng dòng),
**thiếu chưa làm: 0** trong phạm vi hằng ngày (các mục "còn khác" là khác biệt nhỏ hoặc cần quyết
định).

## Điền dữ liệu (thẻ Trang đầu › Chỉnh sửa, tay cầm điền)

| Hành động | Bảng tính quen dùng | BlueSheet trước | Trạng thái |
|---|---|---|---|
| Kéo tay cầm với 1, 2 | Tiếp chuỗi 3, 4, 5…; vùng chọn phủ cả vùng điền | Đúng | giống |
| Kéo tay cầm với một số (5) | Chép 5 | Đúng | giống |
| Giữ Ctrl khi thả tay cầm (một số) | Đếm lên 6, 7…; với chuỗi thì ngược lại: chép | Ctrl bị bỏ qua (vẫn chép) | **đã sửa** — `autofill` chế độ `toggle` |
| Thứ Hai / Tháng 1 / Quý 3 / ngày / "Sản phẩm 9" | Danh sách tuần, tháng, quý (vòng lại Quý 1), ngày theo ngày, chữ + số đếm lên | Đúng | giống |
| Kéo công thức | Tham chiếu tương đối dịch theo | Đúng | giống |
| Nút Tuỳ chọn tự điền sau khi kéo | Nút nhỏ ở góc vùng điền: Sao chép ô / Điền chuỗi / Chỉ điền định dạng / Điền không kèm định dạng | Không có | **đã sửa** — nút + menu, đổi kiểu điền thay cho lần điền trước (một bước hoàn tác) |
| Bấm đúp tay cầm | Điền xuống tới hết cột dữ liệu bên cạnh | Đúng | giống |
| Ctrl+D / Ctrl+R trên vùng | Chép hàng đầu xuống / cột đầu sang phải | Đúng | giống |
| Ctrl+D / Ctrl+R trên **một ô** | Chép ô ngay trên / ngay bên trái vào ô đó (công thức dịch theo) | Không làm gì | **đã sửa** (`fillDir`) |
| Hoàn tác một lần điền | Một bước | Đúng | giống |
| Ctrl+E Flash Fill | Điền theo mẫu từ ví dụ | Có (đã có từ trước) | giống |
| "Thứ 2" kéo xuống | (danh sách tuần dạng số không có sẵn) chữ + số: Thứ 3, Thứ 4… Thứ 8 | Thứ 3… Thứ 8 | còn khác nhẹ — không có danh sách "Thứ 2…Chủ nhật"; người dùng có thể thêm danh sách tuỳ chỉnh |
| Điền › Chuỗi… | Hộp thoại: theo hàng/cột (mặc định theo hình vùng chọn), tuyến tính / cấp số nhân / ngày (ngày, ngày làm việc, tháng, năm) / tự điền, bước, giá trị dừng; ô đầu là ngày thì đề xuất kiểu Ngày | **Không có** | **đã sửa** — lệnh `fillSeries`, nút trong nhóm Chỉnh sửa; tháng tính từ ngày đầu (31/1 → 28/2 → 31/3) |

## Sắp xếp (thẻ Dữ liệu)

| Hành động | Bảng tính quen dùng | BlueSheet trước | Trạng thái |
|---|---|---|---|
| A→Z / Z→A với một ô trong danh sách | Sắp cả vùng dữ liệu liền khối, theo cột của ô hiện hành; hàng tiêu đề tự nhận ra và giữ nguyên | Vùng đúng, nhưng **luôn** coi hàng đầu là tiêu đề | **đã sửa** — đoán tiêu đề (`sortRegion`, `bo_sheet::sort::guess_header`): hàng đầu toàn chữ trên cột có số/ngày, hoặc định dạng khác hàng dưới |
| Danh sách toàn số (3, 1, 2) | Không có tiêu đề: sắp cả ba | Giữ "3" ở trên như tiêu đề | **đã sửa** |
| Chọn một cột của danh sách rộng hơn rồi A→Z | Hỏi "Mở rộng vùng chọn" (mặc định) hay "Tiếp tục với vùng chọn hiện tại" | Sắp riêng cột đó, **làm lệch hàng** | **đã sửa** — hộp thoại Cảnh báo sắp xếp |
| Hộp thoại Sắp xếp | Ô "Dữ liệu có tiêu đề" theo đoán; mức sắp hiện tên cột | Luôn bật tiêu đề | **đã sửa** (dùng cùng phép đoán) |
| Sắp xếp từ menu bộ lọc | Sắp danh sách của bộ lọc, dưới hàng tiêu đề | Sắp vùng quanh **hàng 1** của cột (sai khi bảng không bắt đầu ở hàng 1) | **đã sửa** — dùng vùng của bộ lọc |
| Sắp theo màu, theo danh sách tuỳ chỉnh (tháng, thứ tiếng Việt) | Có trong hộp thoại | Có | giống |
| Hoàn tác | Một bước | Đúng | giống |

## Lọc tự động (thẻ Dữ liệu)

| Hành động | Bảng tính quen dùng | BlueSheet trước | Trạng thái |
|---|---|---|---|
| Ctrl+Shift+L | Bật/tắt nút lọc trên hàng tiêu đề của vùng dữ liệu | Đúng | giống |
| Alt+↓ trên ô tiêu đề có nút lọc | Mở menu lọc của cột | Không làm gì | **đã sửa** |
| Ô tìm trong menu lọc + OK | Chỉ giữ các mục khớp tìm kiếm (mục không khớp bị bỏ) | Giữ cả các mục không khớp (vẫn đang tick) → lọc không đổi | **đã sửa** |
| Thanh trạng thái sau khi lọc | "Tìm thấy x trong y bản ghi", rồi "Chế độ lọc" | Chỉ "Chế độ lọc" | **đã sửa** — `setFilter` trả `total`, thông báo ở thanh trạng thái |
| SUBTOTAL / thống kê thanh trạng thái | Bỏ qua hàng bị lọc | Đúng | giống |
| Chép vùng đang lọc | Chỉ chép các hàng còn hiện | Chép cả hàng bị ẩn | **đã sửa** (engine `copy`; công thức chép như từ vị trí liền khối) — kiểm bằng test engine, chưa có E2E vì E2E clipboard chạy riêng |
| Lọc theo số / chữ / ngày / màu, Top 10, xoá lọc cột, Hiện tất cả | Có | Có | giống |
| Menu lọc là danh sách lồng (Bộ lọc số ›, Bộ lọc văn bản ›) | Menu con theo kiểu dữ liệu | Một hộp chọn điều kiện chung | còn khác (bố cục) — đủ chức năng, khác cách trình bày |

## Công thức (thẻ Công thức)

| Hành động | Bảng tính quen dùng | BlueSheet trước | Trạng thái |
|---|---|---|---|
| Gõ `=SUM(A1;B1)`, `=IF(A1>5;"lớn";"nhỏ")` (vi-VN, dấu phân cách danh sách `;`) | Hợp lệ | Ô **trống**, không báo gì | **đã sửa** — nhận cả `;` lẫn `,` (`bo_formula::localized_input`) |
| Số thập phân bằng dấu phẩy trong công thức `=ROUND(2,567;2)`, `=A1*1,5` | 2,57 / 15 | Lỗi / sai | **đã sửa** |
| Hiển thị công thức trên thanh công thức | vi-VN hiện `;` và `,` thập phân | Hiện dạng lưu `,` / `.` | còn khác — quyết định của dự án: lưu và hiện dạng chuẩn ECMA-376 (`,` `.`), nhận cả hai khi gõ (cần chủ dự án quyết nếu muốn hiện theo vùng) |
| Thiếu dấu `)` ở cuối | Đề nghị sửa (thêm `)`) | Ô trống | **đã sửa** — tự thêm `)` (không hỏi) |
| Công thức sai cú pháp (`=A1+*2`) + Enter | Báo lỗi, ở lại chế độ sửa | Ghi vào ô một công thức hỏng, ô trống | **đã sửa** — hộp thông báo, ô giữ nguyên nội dung đang sửa, Esc để bỏ |
| `=A1/B1` với B1 = 0 | `#DIV/0!` | Đúng | giống |
| Tam giác xanh kiểm tra lỗi (nền) + nút cảnh báo | Tam giác ở góc trên trái ô có vấn đề; chọn ô thì có nút ⚠ với menu sửa / bỏ qua / sửa trên thanh công thức / tuỳ chọn | Chỉ có hộp thoại Kiểm tra lỗi | **đã sửa** — `ErrorMarks`: tam giác + nút cảnh báo (chuyển thành số, dùng công thức bên cạnh, xem bước tính, bỏ qua, sửa, quy tắc) |
| Quy tắc "công thức đọc ô trống" | Tắt mặc định | Bật mặc định | **đã sửa** |
| Truy vết ô tiền lệ / phụ thuộc, Tính từng bước | Mũi tên; hộp thoại từng bước | Có | giống |
| Ctrl+` Hiện công thức | Bật/tắt; nút ở nhóm Kiểm tra công thức | Có phím tắt, nút chỉ ở thẻ Xem | **đã sửa** — thêm nút vào nhóm kiểm tra công thức |
| Tính lại khi đổi dữ liệu | Tự động | Đúng | giống |
| Tham chiếu vòng | Cảnh báo | Có hộp thoại | giống |
| `=DATE(…)`, `=TODAY()`, `=NOW()` trong ô General | Ô tự lấy định dạng ngày (ngày giờ với NOW) | Hiện số seri (46068) | **đã sửa** |
| `=A1+7` (A1 là ngày), `=SUM(B1:B2)` (B là %) | Kết quả lấy định dạng của ô được đọc | General | **đã sửa** (+, −, SUM/MIN/MAX/AVERAGE; ô đã có định dạng thì giữ) |
| Quản lý tên (Ctrl+F3), Đặt tên, Tạo từ vùng chọn (Ctrl+Shift+F3) | Có | **Không có** (chỉ dùng được tên có sẵn trong tệp) | **đã sửa** — nhóm "Tên đã đặt", hộp thoại, kiểm tên hợp lệ (không trùng địa chỉ ô, không dấu cách), phạm vi sổ/trang, ghi chú; mỗi thay đổi một bước hoàn tác |
| Gõ tên mới vào Hộp tên | Đặt tên cho vùng chọn | Không làm gì | **đã sửa** |
| Tham chiếu sang trang khác, tham chiếu tương đối khi chép/điền/chèn hàng | Dịch đúng | Đúng | giống |
| Ctrl+Shift+Enter công thức mảng | Mảng động tràn ô | Mảng động có (UNIQUE/FILTER/SORT tràn) | giống |

### 33 hàm thông dụng trên dữ liệu gõ vào (gõ với `;`)

SUM, AVERAGE, COUNT, COUNTA, IF, IFS, SUMIF, SUMIFS, COUNTIF, COUNTIFS, AVERAGEIF, VLOOKUP, HLOOKUP,
XLOOKUP, INDEX/MATCH, ROUND, ROUNDUP, ROUNDDOWN, LEFT, RIGHT, MID, LEN, TRIM, CONCAT, TEXT, DATE,
YEAR, MONTH, DAY, DATEDIF, NETWORKDAYS, IFERROR, TODAY, UNIQUE, FILTER, SORT — kết quả đều khớp giá
trị tính tay (ca "common functions…" trong spec). Trước khi sửa, mọi hàm gõ với `;` cho ô trống;
`=DATE(…)` hiện số seri.

## Công cụ dữ liệu, biểu đồ, bảng tổng hợp

| Hành động | Bảng tính quen dùng | BlueSheet trước | Trạng thái |
|---|---|---|---|
| Xoá trùng lặp | Hộp thoại chọn cột, "dữ liệu có tiêu đề", báo số dòng xoá | Có | giống |
| Tách văn bản thành cột | Trình hướng dẫn phân cách / độ rộng cố định | Có (một hộp thoại) | giống (khác bố cục: một bước thay vì ba) |
| Xác thực dữ liệu kiểu danh sách + mũi tên chọn trong ô | Có, Alt+↓ mở danh sách | Có | giống |
| Nhóm / bỏ nhóm, Tổng phụ | Có | Có (thiếu thanh +/− cạnh lưới — BACKLOG SHEET-DATA-002) | còn khác — thanh dàn ý |
| Alt+Shift+→ / Alt+Shift+← | Nhóm / bỏ nhóm: cả cột được chọn → nhóm cột, cả hàng → nhóm hàng, các ô → hỏi Hàng/Cột (mặc định Hàng) | Không có phím tắt | **đã sửa** — các ô thì nhóm hàng luôn (không hỏi) |
| Alt+F1 biểu đồ mặc định | Biểu đồ cột cụm của vùng dữ liệu, trên trang hiện tại, được chọn | Không có phím tắt | **đã sửa** |
| F11 biểu đồ trên trang biểu đồ riêng | Tạo trang biểu đồ | Không có (chưa có kiểu trang biểu đồ) | còn khác — cần trang biểu đồ (chartsheet) trong mô hình |
| Chèn › Bảng tổng hợp với một ô trong danh sách | Nguồn = cả vùng dữ liệu, mặc định đặt ở trang tính mới | Đúng | giống |

## Phím tắt đã so sánh

| Phím | Bảng tính quen dùng | BlueSheet |
|---|---|---|
| Ctrl+D / Ctrl+R | Điền xuống / sang phải (một ô: chép ô trên / trái) | giống (đã sửa trường hợp một ô) |
| Ctrl+E | Flash Fill | giống |
| Ctrl (khi thả tay cầm điền) | Đổi chép ↔ chuỗi | giống (đã sửa) |
| Ctrl+Shift+L | Bật/tắt lọc | giống |
| Alt+↓ | Mở menu lọc / danh sách chọn | giống (đã sửa cho ô tiêu đề lọc) |
| Ctrl+` | Hiện công thức | giống |
| F9 / Shift+F9 / Ctrl+Alt+F9 | Tính lại sổ / trang / toàn bộ | giống |
| Shift+F3 | Chèn hàm | giống |
| Ctrl+F3 | Quản lý tên | giống (đã thêm) |
| Ctrl+Shift+F3 | Tạo tên từ vùng chọn | giống (đã thêm) |
| Alt+F1 | Biểu đồ mặc định | giống (đã thêm) |
| Alt+Shift+→ / ← | Nhóm / bỏ nhóm | giống (đã thêm; với ô thường không hỏi Hàng/Cột) |
| F11 | Biểu đồ trên trang riêng | **thiếu** — chưa có trang biểu đồ |
| Alt+Shift+V (BlueOffice) | Mở menu nút tuỳ chọn dán / tự điền | dùng chung cho nút Tuỳ chọn tự điền |

## Còn khác (tóm tắt)

1. Thanh công thức hiện dạng lưu (`,` phân cách đối số, `.` thập phân) thay vì `;` và `,` như bảng
   tính chạy với vùng vi-VN — gõ được cả hai. Cần chủ dự án quyết định có hiện theo vùng hay không.
2. Thiếu `)` được tự thêm, không có hộp hỏi "có sửa thành …?".
3. "Thứ 2" kéo xuống đếm tới "Thứ 8" (không có danh sách tuần dạng số).
4. Menu lọc: một hộp chọn điều kiện thay vì menu con Bộ lọc số / văn bản / ngày.
5. Thanh dàn ý +/− cạnh lưới chưa có (đã ghi ở SHEET-DATA-002).
6. F11 (trang biểu đồ) chưa có.
7. Nút Điền trên ribbon là các nút riêng (Xuống, Phải, Chuỗi…) thay vì một nút thả xuống có
   Lên / Trái / Căn đều; Điền lên / sang trái chưa có lệnh riêng (kéo tay cầm lên / sang trái thì có).
