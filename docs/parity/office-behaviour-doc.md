# BlueDoc — đối chiếu hành vi hằng ngày với trình soạn thảo văn bản tham chiếu

Phạm vi: các thao tác một nhân viên văn phòng làm hằng ngày trong BlueDoc (không phải tính năng
hiếm) và **cảm giác khi dùng** (con trỏ, vùng chọn, độ trễ gõ phím, thước, thanh trạng thái, menu,
dải lệnh). Tham chiếu: Microsoft Word (bản desktop, định dạng `.docx`) — hành vi lấy từ tài liệu
công khai (trang "Keyboard shortcuts in Word" của Microsoft Support, các trang trợ giúp
AutoFormat/AutoCorrect) và quan sát hộp đen; không dịch ngược, không chép tài nguyên; tên, icon,
màu và chuỗi giao diện vẫn là của BlueOffice.

Cách làm: mỗi thao tác được thực hiện trên ứng dụng web thật qua Playwright (bấm đúng nút ribbon,
gõ đúng phím trên dữ liệu gõ vào), so với hành vi tham chiếu, rồi phân loại. Những gì đã sửa hoặc
đã xác nhận có assertion trong `tests/e2e/specs/office-behaviour-doc.spec.ts` (ký hiệu **E2E**),
phần cảm giác / độ trễ trong `tests/e2e/specs/office-feel-doc.spec.ts` (**E2E-feel**); phần có
test đơn vị ghi **Rust** (`bo-edit` / `bo-engine`) hoặc **vitest**; phần chỉ đọc mã / spec có sẵn
ghi rõ nguồn.

Trạng thái: **giống** · **đã sửa** · **còn khác** (kèm lý do) · **còn thiếu** (chưa có tính năng).

## Tổng kết

| Nhóm | giống | đã sửa | còn khác | còn thiếu |
| --- | ---: | ---: | ---: | ---: |
| Gõ chữ, di chuyển, chọn (27) | 15 | 10 | 1 | 1 |
| Trang đầu (38) | 17 | 16 | 2 | 3 |
| Chèn, Xem lại, Tệp (20) | 10 | 6 | 1 | 3 |
| Xem, thanh trạng thái, menu, cảm giác (24) | 15 | 6 | 3 | 0 |
| **Cộng (109 thao tác)** | **57** | **38** | **7** | **7** |

Ghi chú trung thực về độ chắc chắn của tham chiếu (không có Word để đo trong môi trường này):
(a) độ mịn của hoàn tác khi gõ (Word gom một cụm gõ liên tục; BlueDoc gom theo từ — kiểm tra chỉ
khẳng định "không cả đoạn, không từng chữ"); (b) thứ tự Shift+F3 khi văn bản đang lẫn hoa/thường
(ở đây: → CHỮ HOA); (c) danh sách cỡ chữ của BlueDoc có thêm 13 pt (cỡ phổ biến của văn bản hành
chính Việt Nam) nên Ctrl+Shift+> từ 12 lên 13 thay vì 14 — chủ ý, ghi ở dòng 31.

## 1. Gõ chữ, di chuyển, chọn

| # | Thao tác | Hành vi tham chiếu | BlueDoc trước đây | Trạng thái |
| --- | --- | --- | --- | --- |
| 1 | Gõ khi đang chọn | Văn bản gõ thay vùng chọn | Đúng | giống (E2E) |
| 2 | Backspace / Delete | Xoá ký tự (cụm grapheme) trước / sau; nối đoạn ở mép | Đúng | giống (Rust có sẵn) |
| 3 | Ctrl+Backspace | Xoá từ trước con trỏ (tới đầu từ), một bước hoàn tác | Xoá **một ký tự** | **đã sửa** — lệnh `deleteWordBackward` (E2E + Rust) |
| 4 | Ctrl+Delete | Xoá tới đầu từ kế tiếp (từ + dấu cách sau) | Xoá một ký tự | **đã sửa** — `deleteWordForward` (E2E + Rust) |
| 5 | Ctrl+← / Ctrl+→ | Về đầu từ trước / đầu từ sau | Đúng | giống (E2E) |
| 6 | Ctrl+↑ / Ctrl+↓ | Về đầu đoạn này (rồi đoạn trước) / đầu đoạn sau | Lên / xuống **một dòng** | **đã sửa** — chuyển động `paraUp` / `paraDown` (E2E + Rust + vitest) |
| 7 | Home / End | Đầu / cuối dòng | Đúng | giống (E2E) |
| 8 | Ctrl+Home / Ctrl+End | Đầu / cuối tài liệu | Đúng | giống (E2E) |
| 9 | Shift + các phím trên | Mở rộng vùng chọn theo cùng bước | Đúng, trừ Ctrl+Shift+↑/↓ (theo dòng) | **đã sửa** Ctrl+Shift+↑/↓; còn lại giống (E2E) |
| 10 | PageUp / PageDown | Lên / xuống một **màn hình** | Lên / xuống một **trang** (cùng cột) | **còn khác** — bước theo trang in; với trang A4 ở 100 % gần bằng một màn hình, chưa đổi |
| 11 | Nháy đúp | Chọn từ **kèm dấu cách sau** | Chọn từ, không kèm dấu cách | **đã sửa** — `word_select_at` (E2E + Rust) |
| 12 | Nháy ba lần | Chọn cả đoạn | Đúng | giống (E2E) |
| 13 | Ctrl+nháy | Chọn cả **câu** (kèm dấu cách sau) | Đặt con trỏ | **đã sửa** — lệnh `selectSentence` (E2E + Rust + vitest) |
| 14 | Kéo chuột qua nhiều từ | Khi rời từ đầu: chọn **trọn từ** (từ đầu từ thứ nhất tới hết từ dưới chuột) | Chọn theo ký tự | **đã sửa** — `nav::drag_selection` (E2E + Rust) |
| 15 | Nháy đúp / ba lần rồi kéo | Mở rộng theo từ / theo đoạn | Theo ký tự | **đã sửa** (Rust) |
| 16 | Shift+nháy | Mở rộng vùng chọn tới điểm nháy | Đúng | giống (mã `extend`) |
| 17 | Ctrl+A | Chọn toàn bộ | Đúng | giống (E2E dùng trong test giãn dòng) |
| 18 | Ctrl+Z khi gõ | Hoàn tác theo cụm gõ, không từng chữ | Theo từ | giống (gần; E2E) — xem ghi chú (a) |
| 19 | Ctrl+Y | Làm lại | Đúng | giống (E2E) |
| 20 | F4 | Lặp lại thao tác cuối (định dạng) ở chỗ mới | Đúng | giống (E2E) |
| 21 | Insert | Chế độ gõ đè **tắt** mặc định, phím Insert bật / tắt | Đúng | giống (E2E) |
| 22 | Shift+Enter | Ngắt dòng trong đoạn | Đúng | giống (vitest `keys`) |
| 23 | Enter sau chữ đậm | Đoạn mới gõ tiếp vẫn **đậm** (định dạng của dấu đoạn đi theo) | Đoạn mới mất định dạng | **đã sửa** — `split_paragraph` mang định dạng vào đoạn trống (E2E + Rust) |
| 24 | Ctrl+nháy vào liên kết | Mở liên kết | Không có (không có liên kết tự chèn) | **đã sửa** — mở qua `Platform.shell.openExternal` (vitest) |
| 25 | Kéo-thả văn bản đã chọn | Di chuyển (Ctrl: sao chép) | Nhấn vào vùng chọn chỉ đặt lại con trỏ | **còn thiếu** — chưa có kéo-thả nội bộ (Ctrl+X / Ctrl+V vẫn dùng được) |
| 26 | Gõ tiếng Việt (Telex/VNI qua IME) | Văn bản dựng tại con trỏ, không mất dấu | Đúng | giống (spec `ime` có sẵn) |
| 27 | Viết hoa chữ đầu câu / đầu mục | Khi xong từ (dấu cách, Enter) | Đúng | giống (E2E) |

## 2. Trang đầu (Home)

| # | Thao tác | Hành vi tham chiếu | BlueDoc trước đây | Trạng thái |
| --- | --- | --- | --- | --- |
| 28 | Thứ tự nhóm | Hoàn tác · Bộ nhớ tạm · Phông chữ · Đoạn văn · Kiểu · Chỉnh sửa · (Giọng nói) | Nhóm Giọng nói nằm **trước** Chỉnh sửa | **đã sửa** (E2E) |
| 29 | Ctrl+B / I / U | Bật / tắt trên vùng chọn; ở con trỏ: áp cho chữ gõ tiếp | Đúng | giống (E2E) |
| 30 | Ctrl+] / Ctrl+[ | Tăng / giảm 1 pt | Đúng | giống (E2E) |
| 31 | Ctrl+Shift+> / < và nút Tăng / Giảm cỡ chữ | Theo **danh sách cỡ** (… 12 14 16 18 20 …, trên 72: +10) | Không có phím; nút ribbon chỉ ±1 pt | **đã sửa** — `format.growStep` / `shrinkStep`, nút ribbon và thanh mini dùng bước danh sách (E2E); danh sách có thêm 13 pt (ghi chú c) |
| 32 | Ctrl+Shift+= | Chỉ số trên | Là **phóng to** | **đã sửa** — phóng to chuyển sang Ctrl+Alt+= (E2E) |
| 33 | Ctrl+= | Chỉ số dưới | Đúng | giống (E2E) |
| 34 | Ctrl+Space | Xoá định dạng ký tự | Không có | **đã sửa** (E2E) |
| 35 | Ctrl+D | Hộp thoại Phông chữ | Không có phím | **đã sửa** — mở hộp thoại hiệu ứng ký tự (E2E) |
| 36 | Shift+F3 | Đổi chữ: HOA → thường → Hoa Đầu Mỗi Từ | Không có | **đã sửa** — lệnh `changeCase` (giữ định dạng, chọn lại văn bản; E2E + Rust); nút "Đổi chữ hoa / thường" trên ribbon |
| 37 | Menu Đổi chữ (5 kiểu) | Câu / thường / HOA / Mỗi Từ / đảo | Không có | **còn khác** — có 5 lệnh (`format.case.*`, tìm qua ô Tìm lệnh) nhưng chưa có danh sách thả xuống trên ribbon |
| 38 | Ctrl+Shift+A / K | CHỮ HOA (thuộc tính) / chữ hoa nhỏ | Không có | **còn thiếu** — mô hình định dạng ký tự chưa nối `caps` / `smallCaps` vào lệnh |
| 39 | Ctrl+Shift+D | Gạch chân đôi | Không có | **còn thiếu** (hộp thoại phông chưa có kiểu gạch chân) |
| 40 | Sao định dạng | Nháy: một lần; nháy đúp: giữ tới Esc | Đúng | giống (`doc-features.spec.ts`) |
| 41 | Ctrl+Shift+C / V | Sao / dán định dạng | Đúng | giống (mã) |
| 42 | Ctrl+E / L / R / J | Căn giữa / trái / phải / đều cả đoạn | Đúng | giống (E2E) |
| 43 | Ctrl+1 / 2 / 5 | Giãn dòng đơn / đôi / 1,5 | Không có | **đã sửa** (E2E đo khoảng cách dòng) |
| 44 | Ctrl+0 | Thêm / bỏ 12 pt trước đoạn | Ctrl+0 là **100 %** | **đã sửa** — 100 % chuyển sang Ctrl+Alt+0 (E2E) |
| 45 | Ctrl+M / Ctrl+Shift+M | Thụt lề trái thêm / bớt một nấc | Không có phím | **đã sửa** (E2E đo vị trí con trỏ) |
| 46 | Ctrl+T / Ctrl+Shift+T | Thụt treo thêm / bớt (dòng đầu đứng yên) | Không có | **đã sửa** (E2E) |
| 47 | Ctrl+Q | Xoá định dạng đoạn (giữ kiểu) | Mở **ô tìm lệnh** | **đã sửa** — `ParaFormat::Reset`; ô tìm lệnh của BlueDoc chỉ còn Alt+Q (E2E + Rust + vitest) |
| 48 | Gõ "1. " | Tự thành danh sách số | Đúng | giống (E2E) |
| 49 | Gõ "* " / "- " | Tự thành gạch đầu dòng | Đúng | giống (E2E) |
| 50 | Enter trong danh sách | Thêm mục; Enter ở mục trống kết thúc danh sách | Đúng | giống (E2E) |
| 51 | Tab / Shift+Tab ở đầu mục | Hạ / nâng **cấp** mục | Chèn **ký tự tab** | **đã sửa** — `Session::tab_key` (E2E + Rust) |
| 52 | Backspace ở đầu mục | Bỏ số, giữ chữ | Đúng | giống (E2E) |
| 53 | Tab ở đầu đoạn có chữ | Thụt **dòng đầu** 0,5″ (lần nữa: cả đoạn); Backspace bỏ thụt | Chèn ký tự tab | **đã sửa** (E2E + Rust) |
| 54 | Tab giữa dòng | Ký tự tab | Đúng | giống (E2E) |
| 55 | Ctrl+Shift+L | Danh sách gạch đầu dòng | Không có | **đã sửa** (E2E) |
| 56 | Kiểu khi không chọn gì | Áp cho cả đoạn chứa con trỏ | Đúng | giống (E2E) |
| 57 | Ctrl+Alt+1 / 2 / 3 | Đề mục 1 / 2 / 3 | Có 1 và 2, **thiếu 3** | **đã sửa** (E2E) |
| 58 | Ctrl+Shift+N | Kiểu Bình thường | Đúng | giống (E2E) |
| 59 | Bộ sưu tập kiểu | Hàng ô xem trước kiểu nhanh, ô của kiểu tại con trỏ được nhấn, ▾ mở mọi kiểu | 4–5 nút nhỏ biểu tượng | **đã sửa** — `StylesGallery`: kiểu nhanh (qFormat, theo uiPriority) vẽ bằng định dạng của chính nó, nút ▾ mở ngăn Kiểu (E2E) |
| 60 | Ctrl+C / Ctrl+X / Ctrl+V | Sao chép / cắt / dán | Đúng | giống (E2E) |
| 61 | Dán mặc định | Giữ định dạng nguồn; nút Tuỳ chọn dán xuất hiện cạnh chỗ dán | Đúng | giống (E2E; `paste.spec.ts`) |
| 62 | Ctrl+Alt+V | Dán đặc biệt | Đúng | giống (mã) |
| 63 | Ctrl+F | Ngăn Điều hướng với ô tìm | Thanh **Tìm** phía trên | **còn khác** — thanh Tìm có đủ tìm / tô sáng tất cả / tuỳ chọn; ô tìm của ngăn Điều hướng mở từ Xem › Điều hướng. Nhiều spec (cả cộng tác) dùng thanh này, chưa đổi |
| 64 | Ctrl+H | Thay thế | Đúng | giống (E2E) |
| 65 | Ctrl+G / F5 | Đi tới (trang, đề mục…) | Không có | **còn thiếu** |

## 3. Chèn, Xem lại, Tệp

| # | Thao tác | Hành vi tham chiếu | BlueDoc trước đây | Trạng thái |
| --- | --- | --- | --- | --- |
| 66 | Ctrl+Enter | Ngắt trang | Đúng | giống (E2E) |
| 67 | Tab ở ô cuối của bảng | Thêm hàng | Đúng | giống (E2E) |
| 68 | Chèn › Bảng | Lưới chọn số hàng × cột | Chèn ngay bảng 3 × 3 | **còn khác** — chưa có lưới chọn; sửa số hàng / cột ở thẻ Bảng |
| 69 | Ctrl+K | Hộp thoại Chèn liên kết (văn bản hiển thị + địa chỉ); vùng chọn thành liên kết | **Không có lệnh liên kết** trong BlueDoc | **đã sửa** — lệnh `setLink` (§17.16.22), nhóm Chèn › Liên kết, địa chỉ trần được thêm `https://` / `mailto:` (E2E + Rust) |
| 70 | Chuột phải trên liên kết | Sửa / Mở / Gỡ liên kết | Không có | **đã sửa** (E2E) |
| 71 | Alt+Shift+D / T / P | Trường ngày / giờ / số trang | Không có phím | **đã sửa** (E2E kiểm ngày dd/MM/yyyy) |
| 72 | Ctrl+Shift+Space | Dấu cách không ngắt | Không có | **đã sửa** (E2E) |
| 73 | Ctrl+Shift+Enter | Ngắt cột | Không có | **còn thiếu** |
| 74 | Chèn ảnh, ký tự đặc biệt, biểu tượng, phương trình (Alt+=) | Như tham chiếu | Đúng | giống (spec có sẵn) |
| 75 | Nháy đúp lề trên / dưới | Mở đầu / chân trang | Đúng | giống (mã `fonts.rs` count ≥ 2) |
| 76 | Chú thích chân trang / cuối văn bản | Ctrl+Alt+F / Ctrl+Alt+D | Đúng | giống (mã) |
| 77 | F7 | Kiểm tra chính tả & ngữ pháp | Đúng | giống (E2E) |
| 78 | Shift+F7 | Từ đồng nghĩa | Đúng | giống (mã) |
| 79 | Ctrl+Shift+E | Bật / tắt Theo dõi thay đổi | Là **Xuất PDF**; theo dõi ở Ctrl+Shift+R | **đã sửa** — Xuất PDF không còn phím tắt (tham chiếu không có), thanh trạng thái đổi theo (E2E) |
| 80 | Ctrl+Alt+M | Bình luận mới | Đúng | giống (mã) |
| 81 | Ctrl+N / Ctrl+O / Ctrl+S / Ctrl+P | Mới / Mở / Lưu / In | Đúng | giống (`doc-features.spec.ts`) |
| 82 | F12 | Lưu thành | Ctrl+Shift+S | **đã sửa** |
| 83 | Ctrl+W | Đóng tài liệu | Không có (web không đóng được thẻ) | **còn thiếu** (desktop: đóng cửa sổ) |
| 84 | Ctrl+F2 | Xem trước khi in | Không có phím (Ctrl+P mở bản xem trước in) | **còn thiếu** phím riêng |
| 85 | Lưu → mở lại giữ định dạng | — | Đúng | giống (`doc-features.spec.ts`) |

## 4. Xem, thanh trạng thái, menu, cảm giác

| # | Thao tác | Hành vi tham chiếu | BlueDoc trước đây | Trạng thái |
| --- | --- | --- | --- | --- |
| 86 | Con trỏ | Mảnh **1 px**, nhấp nháy 530 ms bật / 530 ms tắt, cao bằng dòng chữ | 1,5 px, nhịp đúng | **đã sửa** độ rộng (E2E-feel) |
| 87 | Con trỏ khi nhấn chuột | Đặt ngay khi **nhấn** (mouse-down), không chờ nhả | Đúng | giống (E2E-feel đo độ trễ) |
| 88 | Màu vùng chọn | Xám trung tính trong suốt, chữ đen vẫn đen | Xanh `#3378DC` 32 % | **đã sửa** — token `--bo-chrome-selection` sáng / tối = đen 20 % (giấy trắng thành #CCC; tương phản chữ 13:1); tương phản cao giữ xanh có viền (E2E-feel + vitest token) |
| 89 | Trang | Trang trắng trên nền xám, bóng mờ, khe giữa trang | Đúng (nền `--bo-canvas-bg`, bóng 1–3 px, khe 16 px) | giống (ảnh chụp) |
| 90 | Thước ngang / dọc | Lề tô xám, dấu thụt lề kéo được, bấm thêm điểm dừng tab | Đúng | giống (`Ruler.tsx`, ảnh chụp) |
| 91 | Thước hiện mặc định | Tham chiếu **ẩn** thước khi mới cài | Hiện | **còn khác** (chủ ý: dự án muốn thước hiện; tắt ở Xem › Thước) |
| 92 | Thanh trạng thái | Trang x / y · số từ (x / y từ khi chọn) · ngôn ngữ · … · nút chế độ xem · thanh trượt thu phóng · % ở **mép phải** | Đúng (thêm "Lõi: WASM" của BlueOffice) | giống (E2E) |
| 93 | Ctrl+lăn chuột | Thu phóng | Đúng | giống (E2E) |
| 94 | Phím thu phóng | Tham chiếu không có phím riêng | Ctrl+Shift+= / Ctrl+- / Ctrl+0 (đụng chỉ số trên và 12 pt trước đoạn) | **đã sửa** — chuyển sang Ctrl+Alt+= / Ctrl+Alt+- / Ctrl+Alt+0 |
| 95 | Thanh mini | Hiện khi chọn bằng **chuột**, không khi chọn bằng bàn phím | Đúng | giống (E2E) |
| 96 | Menu chuột phải trên chữ | Cắt / Chép / Dán (tuỳ chọn dán) · Phông chữ… · danh sách · Từ đồng nghĩa · Dịch · Liên kết · Bình luận mới | Có thêm Xoá định dạng, Chèn chú thích ảnh, Ngôn ngữ; không có Liên kết; từ đồng nghĩa ở cuối | **đã sửa** thứ tự và nội dung (E2E) |
| 97 | Chuột phải ngoài vùng chọn | Đặt con trỏ tại đó trước khi mở menu | Đúng | giống (mã) |
| 98 | Shift+F10 / phím menu | Menu tại con trỏ | Đúng | giống (`command-ui.spec.ts`) |
| 99 | Ctrl+F1 | Thu / mở dải lệnh | Đúng | giống |
| 100 | Gõ phím → chữ hiện | Không thấy trễ (một vài khung hình) | ~63 ms trung vị (gồm 1–2 khung của phép đo) | **còn khác** — giảm việc của luồng chính nhưng trung vị chưa đổi rõ; xem mục Độ trễ |
| 101 | Nhấn chuột → con trỏ | Tức thì | ~32 ms trung vị (≈ 2 khung, gồm phép đo) | **còn khác** (nhỏ) — xem mục Độ trễ |
| 102 | Cuộn | Mượt | 16,7 ms / khung (p95 16,7–33 ms) | giống (E2E-feel) |
| 103 | Thu phóng bằng thanh trượt | Kéo / bấm − + | Đúng | giống (`statusBar.test.tsx`) |
| 104 | Chế độ đọc / In / Web | Ba nút ở thanh trạng thái | Đúng | giống (`doc-views.spec.ts`) |
| 105 | Ribbon thu gọn khi hẹp | Nhóm thu dần, phần thừa vào "…" | Đúng, nhưng **đo lại mọi nhóm sau mỗi lần vẽ** (ép bố trí sau mỗi phím) | **đã sửa** — chỉ đo khi nhóm / nhãn / bề rộng / mức thu đổi |
| 106 | Phím trong ô nhập của ngăn (vd. trình sửa script) | Phím của ô nhập | Ctrl+Space, Ctrl+B… rơi vào tài liệu | **đã sửa** — ô `textarea` / `contenteditable` của ngăn giữ phím của mình |
| 107 | Ctrl+Shift+8 | Hiện / ẩn ký hiệu định dạng | Đúng | giống |
| 108 | Alt / F10 (KeyTips) | Chữ gợi ý trên tab / lệnh | Đúng | giống (spec có sẵn) |
| 109 | Đa cửa sổ, chia đôi | Như tham chiếu | Đúng | giống (spec có sẵn) |

## Độ trễ (đo trên ứng dụng web, Playwright, `requestAnimationFrame`)

Tài liệu ~3 trang chữ tiếng Việt (`tests/e2e/specs/office-feel-doc.spec.ts`); tính từ **dấu
thời gian của sự kiện** `keydown` / `mousedown` tới khung hình *kế sau* khung đã vẽ lại trang (gõ)
hoặc đã dời con trỏ (nhấn chuột) — tức là số đo gồm thêm 1–2 khung (16–33 ms) của chính phép đo.
"Trước" là bản dựng của commit gốc `df5567d7`, "sau" là nhánh này, đo liên tiếp trên cùng máy lúc
máy rảnh, ba lượt mỗi bên (trung vị của từng lượt):

| Đo | Trước | Sau |
| --- | ---: | ---: |
| Phím chữ → glyph hiện | 63,2 / 63,5 / 63,5 ms | 55,3 / 63,6 / 63,6 ms |
| Enter → hiện | 48,8 / 55,3 / 63,5 ms | 45,2 / 45,8 / 57,0 ms |
| Backspace → hiện | 46,5 / 60,2 / 61,7 ms | 47,4 / 52,8 / 52,8 ms |
| Nhấn chuột → con trỏ | 31,7 / 31,9 / 33,5 ms | 33,3 / 33,4 / 34,1 ms |
| Khung hình khi cuộn (trung vị / p95) | 16,7 / 16,8 ms | 16,7 / 16,7 ms |

Kết luận trung thực: **độ trễ đầu-cuối không đổi đáng kể** (chênh lệch nằm trong dao động; Enter /
Backspace hơi tốt hơn). Các số lớn hơn đo được lúc đầu đợt (80–88 ms) là do máy đang bận và phép đo
cũ tính cả thời gian Playwright gửi phím — đã bỏ.

Phân tích (hồ sơ CPU của luồng chính, bản không rút gọn): bản thân lõi WASM chỉ tốn ~0,3 ms một
lệnh gõ và ~0,1 ms một lần nháy (đo bằng Node trên đúng bản WASM); thời gian nằm ở luồng chính và
ở các vòng hỏi-đáp với worker: phím → lệnh → kết quả → vẽ lại React toàn bộ `BlueEditor` → hỏi
display list của trang → vẽ canvas ở khung kế tiếp. Đã bớt việc thừa của luồng chính (không đổi
số đo đầu-cuối nhưng giảm CPU mỗi phím): ribbon không còn đo lại mọi nhóm sau mỗi lần vẽ (ép bố
trí), tra chuỗi i18n không nội suy được nhớ đệm, bản phản chiếu cho trình đọc màn hình chạy sau
khung hình kế tiếp, trang bị đổi được hỏi lõi ngay khi kết quả lệnh về, không vẽ lại lớp vùng chọn
khi không có ô tô sáng tìm kiếm để xoá. **Còn khác:** ~45–65 ms (trừ phần đo còn ~30–45 ms, 2–3
khung) so với ≤ 1–2 khung của tham chiếu. Muốn giảm thật cần: gửi display list của trang bị đổi kèm
kết quả lệnh (bớt một vòng worker) và tách ribbon / thanh trạng thái khỏi phần nhận kết quả gõ
(không vẽ lại cả `BlueEditor` mỗi phím) — thay đổi kiến trúc lớn hơn phạm vi đợt này.
## Phím tắt đã so

| Phím | Tham chiếu | BlueDoc trước | Bây giờ |
| --- | --- | --- | --- |
| Ctrl+B / I / U | Đậm / nghiêng / gạch chân | ✓ | ✓ |
| Ctrl+] / Ctrl+[ | ±1 pt | ✓ | ✓ |
| Ctrl+Shift+> / < | Cỡ chữ theo danh sách | — | ✓ |
| Ctrl+= / Ctrl+Shift+= | Chỉ số dưới / trên | ✓ / phóng to | ✓ / ✓ |
| Ctrl+Space | Xoá định dạng ký tự | — | ✓ |
| Ctrl+D | Hộp thoại phông | — | ✓ |
| Shift+F3 | Đổi chữ hoa / thường | — | ✓ |
| Ctrl+Shift+A / K / D | HOA / hoa nhỏ / gạch chân đôi | — | — (còn thiếu) |
| Ctrl+Shift+C / V | Sao / dán định dạng | ✓ | ✓ |
| Ctrl+E / L / R / J | Căn | ✓ | ✓ |
| Ctrl+1 / 2 / 5 | Giãn dòng | — | ✓ |
| Ctrl+0 | ±12 pt trước đoạn | 100 % | ✓ (100 % → Ctrl+Alt+0) |
| Ctrl+M / Ctrl+Shift+M | Thụt lề | — | ✓ |
| Ctrl+T / Ctrl+Shift+T | Thụt treo | — | ✓ |
| Ctrl+Q | Xoá định dạng đoạn | Tìm lệnh | ✓ (Tìm lệnh: Alt+Q) |
| Ctrl+Shift+L | Danh sách | — | ✓ |
| Ctrl+Shift+N | Bình thường | ✓ | ✓ |
| Ctrl+Alt+1 / 2 / 3 | Đề mục 1 / 2 / 3 | 1, 2 | ✓ |
| Ctrl+Enter / Shift+Enter | Ngắt trang / ngắt dòng | ✓ | ✓ |
| Ctrl+Shift+Enter | Ngắt cột | — | — (còn thiếu) |
| Ctrl+Shift+Space | Dấu cách không ngắt | — | ✓ |
| Ctrl+K | Liên kết | — | ✓ |
| Alt+Shift+D / T / P | Trường ngày / giờ / trang | — | ✓ |
| Ctrl+Backspace / Ctrl+Delete | Xoá từ | xoá ký tự | ✓ |
| Ctrl+← → | Theo từ | ✓ | ✓ |
| Ctrl+↑ ↓ | Theo đoạn | theo dòng | ✓ |
| Home / End / Ctrl+Home / Ctrl+End | — | ✓ | ✓ |
| PageUp / PageDown | Một màn hình | một trang | còn khác |
| Ctrl+A / Ctrl+Z / Ctrl+Y / F4 | — | ✓ | ✓ |
| Ctrl+C / X / V / Ctrl+Alt+V | — | ✓ | ✓ |
| Ctrl+F | Ngăn điều hướng | thanh Tìm | còn khác |
| Ctrl+H | Thay thế | ✓ | ✓ |
| Ctrl+G / F5 | Đi tới | — | — (còn thiếu) |
| Insert | Gõ đè (tắt mặc định) | ✓ | ✓ |
| F7 / Shift+F7 | Chính tả / từ đồng nghĩa | ✓ | ✓ |
| Ctrl+Shift+E | Theo dõi thay đổi | Xuất PDF | ✓ |
| Ctrl+Alt+M | Bình luận | ✓ | ✓ |
| Ctrl+Alt+F / Ctrl+Alt+D | Chú thích chân trang / cuối | ✓ | ✓ |
| F9 / Alt+F9 / Ctrl+F11 / Ctrl+Shift+F9 | Trường | ✓ | ✓ |
| Ctrl+N / O / S / P | Tệp | ✓ | ✓ |
| F12 | Lưu thành | Ctrl+Shift+S | ✓ |
| Ctrl+W / Ctrl+F2 | Đóng / xem trước in | — | — (còn thiếu) |
| Ctrl+Shift+8 | Ký hiệu định dạng | ✓ | ✓ |
| Ctrl+F1 / Alt / F10 / Shift+F10 | Dải lệnh / KeyTips / menu | ✓ | ✓ |
| Ctrl+click | Chọn câu / mở liên kết | — | ✓ |

## Còn khác / còn thiếu — vì sao chưa làm

- **Kéo-thả văn bản** (25): cần chế độ kéo riêng trong `DocumentView` (bóng con trỏ thả, Ctrl để
  chép) và lệnh di chuyển một bước hoàn tác — một hạng mục riêng.
- **PageUp/PageDown theo màn hình** (10): lõi chỉ biết trang in; cần chiều cao khung nhìn từ giao
  diện gửi kèm lệnh.
- **Ctrl+F mở ngăn Điều hướng** (63): thanh Tìm hiện được nhiều spec (cả cộng tác) dùng làm công
  cụ chọn văn bản; đổi phím cần đổi các spec đó cùng lúc.
- **Ctrl+Shift+A / K / D, menu Đổi chữ trên ribbon** (37–39): thiếu thuộc tính `caps` /
  `smallCaps` / gạch chân đôi trong bản vá định dạng ký tự.
- **Lưới chọn bảng, Ctrl+G, Ctrl+Shift+Enter, Ctrl+W / Ctrl+F2** (65, 68, 73, 83, 84): chưa có.
- **Độ trễ gõ phím** (100–101): xem mục Độ trễ.
