# BlueSlide — đối chiếu hành vi hằng ngày với phần mềm trình chiếu tham chiếu

Phạm vi: các thao tác một nhân viên văn phòng làm hằng ngày trong BlueSlide (không phải tính năng
hiếm). Tham chiếu: Microsoft PowerPoint (bản desktop, định dạng `.pptx`) — hành vi lấy từ tài liệu
công khai (trang "Use keyboard shortcuts to create PowerPoint presentations", "…to deliver
PowerPoint presentations" của Microsoft Support) và quan sát hộp đen; không dịch ngược, không chép
tài nguyên; tên, icon và chuỗi giao diện vẫn là của BlueOffice.

Cách làm: mỗi thao tác được thực hiện trên ứng dụng web thật qua Playwright (bấm đúng nút ribbon,
gõ đúng phím trên dữ liệu gõ vào), so với hành vi tham chiếu, rồi phân loại. Những gì đã sửa hoặc
đã xác nhận đều có assertion trong `tests/e2e/specs/office-behaviour-slide.spec.ts` (ký hiệu **E2E**
ở cột trạng thái); phần chỉ đọc mã / spec có sẵn ghi rõ nguồn.

Trạng thái: **giống** · **đã sửa** · **còn khác** (kèm lý do) · **còn thiếu** (chưa có tính năng).

## Tổng kết

| Nhóm | giống | đã sửa | còn khác | còn thiếu |
| --- | ---: | ---: | ---: | ---: |
| Ngăn slide (thumbnail) | 7 | 8 | 0 | 1 |
| Đối tượng trên slide | 13 | 12 | 3 | 1 |
| Văn bản (Trang đầu) | 11 | 7 | 2 | 3 |
| Tìm / thay thế, bảng tạm | 4 | 2 | 1 | 1 |
| Trình chiếu | 8 | 2 | 0 | 2 |
| Tệp, Xem, Xem lại, thanh trạng thái, Chèn, Thiết kế | 10 | 4 | 1 | 2 |
| **Cộng (105 thao tác)** | **53** | **35** | **7** | **10** |

Ghi chú trung thực về độ chắc chắn của tham chiếu (không có PowerPoint để đo trong môi trường
này): ba chỗ dưới đây là hiểu biết hộp đen chưa đo lại được, đã ghi trong bảng —
(a) bước nudge chính xác (ở đây: 6 pt = 1/12 inch của lưới mặc định, bước nhỏ 1 pt);
(b) cạnh nào đứng yên khi Shift+mũi tên đổi kích thước (ở đây: góc trên-trái đứng yên);
(c) cỡ chữ kế tiếp sau 96 pt (ở đây: bội số 10);
(d) gõ chữ khi đang chọn hình đã có chữ: nối vào cuối (không thay chữ cũ).
Dòng nào có hai trạng thái (đã sửa một phần, phần còn lại còn khác) được đếm theo trạng thái ghi
trước.

## 1. Ngăn slide (thumbnail)

| # | Thao tác | Hành vi tham chiếu | BlueSlide trước đây | Trạng thái |
| --- | --- | --- | --- | --- |
| 1 | Enter trên thumbnail | Thêm slide ngay sau slide đang chọn; sau slide tiêu đề → bố cục "tiêu đề và nội dung", còn lại → cùng bố cục với slide đang chọn; slide mới được chọn | Không làm gì | **đã sửa** (E2E) |
| 2 | Ctrl+M | Như Enter trên thumbnail | Đúng (bố cục do engine chọn đúng quy tắc) | giống (E2E) |
| 3 | Ctrl+D khi đang ở ngăn slide | Nhân bản slide, bản sao nằm ngay sau và được chọn | Không làm gì (Ctrl+D chỉ nhân bản hình) | **đã sửa** (E2E) |
| 4 | Ctrl+Shift+D | Nhân bản slide hiện tại (ở đâu cũng được) | Không có phím tắt | **đã sửa** (E2E) |
| 5 | Delete | Xoá slide; chọn slide kế sau (hoặc slide cuối) | Đúng | giống (E2E) |
| 6 | Backspace | Xoá slide như Delete | Không làm gì | **đã sửa** (E2E) |
| 7 | Ctrl+↑ / Ctrl+↓ | Đưa slide lên / xuống một vị trí | Ctrl+↓ chỉ chuyển sang slide kế; đưa slide dùng Ctrl+Shift+↑/↓ | **đã sửa** (E2E) |
| 8 | Ctrl+Shift+↑ / ↓ | Đưa slide về đầu / cuối bản trình chiếu | Đưa lên / xuống một vị trí | **đã sửa** (E2E) |
| 9 | ↑ ↓ PgUp PgDn Home End | Slide trước / sau / đầu / cuối | Đúng | giống (E2E) |
| 10 | Tiêu điểm bàn phím sau khi thêm / chuyển / xoá slide | Nằm trên slide hiện tại, phím gõ tiếp tác động lên slide đó | Ở lại thumbnail cũ (Enter kế tiếp nhảy về slide cũ) | **đã sửa** (E2E) |
| 11 | Kéo thả thumbnail | Đổi thứ tự slide | Đúng (`slide-editing.spec.ts`) | giống |
| 12 | Menu chuột phải trên thumbnail | Slide mới, nhân bản, xoá, ẩn, cắt / sao chép / dán; chuột phải chọn slide đó | Đúng, thiếu "Ẩn slide" | giống (E2E) |
| 13 | Ẩn slide (Hide Slide) | Bật / tắt; slide vẫn nằm trong tệp, số thứ tự bị gạch, trình chiếu bỏ qua | **Không có lệnh** (tệp có `show="0"` thì được tôn trọng nhưng không đặt được) | **đã sửa** — lệnh engine `hideSlides`, nút Trang đầu › Slide, menu thumbnail (E2E + test Rust) |
| 14 | Ctrl+C / Ctrl+X / Ctrl+V trong ngăn slide | Sao chép / cắt / dán cả slide | Đúng (vitest `slide.test.tsx`) | giống |
| 15 | Hoàn tác sau khi thêm slide | Một bước hoàn tác cho mỗi slide | Đúng | giống (E2E) |
| 16 | Chọn nhiều slide (Shift / Ctrl+bấm, Ctrl+A trong ngăn) | Chọn nhiều để xoá / chuyển / đặt hiệu ứng cùng lúc | Chỉ chọn một slide | **còn thiếu** — ngăn slide chưa có mô hình chọn nhiều (Sắp xếp slide cũng vậy) |

## 2. Đối tượng trên slide

| # | Thao tác | Hành vi tham chiếu | BlueSlide trước đây | Trạng thái |
| --- | --- | --- | --- | --- |
| 17 | Bấm vào hình / chỗ trống | Chọn hình / bỏ chọn | Đúng | giống (E2E) |
| 18 | Gõ chữ khi đang chọn hình / placeholder | Vào chế độ sửa văn bản, chữ gõ nối vào cuối văn bản đang có | Phím bị bỏ qua (phải Enter / bấm đúp trước) | **đã sửa** (E2E) |
| 19 | Enter khi đang chọn hình | Vào văn bản, **chọn toàn bộ** chữ | Vào văn bản, con trỏ ở cuối | **đã sửa** (E2E) |
| 20 | F2 | Chuyển qua lại giữa "chọn toàn bộ chữ" và "chọn hình" | Chỉ vào văn bản (con trỏ ở cuối); F2 lần hai không làm gì | **đã sửa** (E2E) |
| 21 | Esc | Đang sửa chữ → chọn hình → không chọn gì | Đúng | giống (E2E) |
| 22 | Mũi tên | Dịch hình một bước lưới (1/12 inch = 6 pt) | Dịch 1 pt | **đã sửa** (E2E) — xem ghi chú (a) |
| 23 | Ctrl+mũi tên | Dịch bước nhỏ | Giống mũi tên thường (1 pt), không phân biệt | **đã sửa** (E2E) |
| 24 | Shift+mũi tên | Đổi kích thước: → rộng hơn, ← hẹp hơn, ↑ cao hơn, ↓ thấp hơn (Ctrl+Shift: bước nhỏ) | Dịch hình 10 pt | **đã sửa** (E2E) — xem ghi chú (b) |
| 25 | Alt+← / Alt+→ | Xoay 15° ngược / thuận chiều kim đồng hồ | Dịch hình 1 pt | **đã sửa** (E2E; Ctrl+Alt: 1°) |
| 26 | Tab / Shift+Tab | Chọn hình kế tiếp / trước đó | Đúng | giống (E2E) |
| 27 | Ctrl+A | Chọn mọi đối tượng của slide | Đúng | giống (E2E) |
| 28 | Delete / Backspace | Xoá đối tượng đang chọn; Ctrl+Z khôi phục | Đúng | giống (E2E) |
| 29 | Ctrl+D | Nhân bản, bản sao lệch xuống-phải và được chọn | Đúng (lệch 12 pt) | giống (E2E) |
| 30 | Ctrl+D lặp lại sau khi kéo bản sao | Bản sao tiếp theo giữ nguyên khoảng cách vừa kéo | Luôn lệch 12 pt | **còn khác** — chưa ghi nhớ khoảng cách của lần nhân bản trước |
| 31 | Ctrl+G / Ctrl+Shift+G | Nhóm / bỏ nhóm; placeholder không nhóm được | Đúng (thông báo từ chối khi có placeholder) | giống (E2E) |
| 32 | Kéo hình | Dịch hình, hiện đường gióng thông minh; Alt: không bắt dính | Đúng (`slide-editing.spec.ts`) | giống |
| 33 | Shift khi kéo | Chỉ dịch theo một trục (ngang hoặc dọc) | Dịch tự do | **đã sửa** (E2E) |
| 34 | Ctrl+kéo | Để lại hình gốc, thả một bản sao (một bước hoàn tác), bản sao được chọn | Chỉ dịch hình | **đã sửa** (E2E) |
| 35 | Shift+bấm | Thêm / bớt hình khỏi vùng chọn | Đúng | giống |
| 36 | Ctrl+bấm | Thêm / bớt hình khỏi vùng chọn | Thay vùng chọn bằng hình vừa bấm | **đã sửa** (E2E) |
| 37 | Kéo khung chọn ở chỗ trống | Chọn các hình nằm trong khung | Đúng (`slide-editing.spec.ts`) | giống |
| 38 | Shift khi kéo tay nắm góc / tay xoay | Giữ tỉ lệ / xoay theo nấc 15° | Đúng | giống |
| 39 | Ctrl khi kéo tay nắm | Đổi kích thước quanh tâm | Không có | **còn thiếu** |
| 40 | Bấm đúp vào hình có chữ | Vào sửa văn bản tại điểm bấm (và mở tab định dạng hình) | Vào sửa văn bản; tab ngữ cảnh hiện khi chọn hình | giống |
| 41 | Chuột phải trên hình / chỗ trống | Chọn hình rồi mở menu (cắt, sao chép, dán, thứ tự, nhóm…) / menu của slide (dán, chọn tất cả, slide mới) | Đúng | giống (E2E) |
| 42 | Ctrl+] / Ctrl+[ và Ctrl+Shift+] / Ctrl+Shift+[ | Lên / xuống một lớp; lên trên cùng / xuống dưới cùng | Chỉ có Ctrl+] / Ctrl+[ | **đã sửa** (thêm hai phím tắt) |
| 43 | Chèn hình (Chèn › Hình) | Chọn hình rồi **bấm hoặc kéo trên slide** để vẽ; bấm = kích thước mặc định tại điểm bấm | Hình xuất hiện ngay giữa slide với kích thước mặc định | **còn khác** — chưa có chế độ "vẽ bằng kéo"; đã sửa phần liên quan: hình mới nhận bàn phím ngay nên gõ chữ là vào hình (E2E) |
| 44 | Chèn hộp văn bản | Bấm trên slide → hộp có con trỏ, gõ ngay; kéo → hộp theo bề rộng kéo | Hộp xuất hiện giữa slide nhưng tiêu điểm ở lại ribbon: gõ không vào hộp | **đã sửa** phần gõ ngay (E2E); **còn khác** phần bấm / kéo để đặt vị trí (cùng lý do #43) |
| 45 | Chèn bảng | Lưới chọn số hàng × cột | Chèn ngay bảng 3×3 | **còn khác** — chưa có lưới chọn kích thước (sau đó thêm / xoá hàng cột được) |

## 3. Văn bản (tab Trang đầu)

| # | Thao tác | Hành vi tham chiếu | BlueSlide trước đây | Trạng thái |
| --- | --- | --- | --- | --- |
| 46 | Ctrl+B / Ctrl+I / Ctrl+U | Bật / tắt; nút ribbon phản ánh trạng thái | Đúng | giống (E2E) |
| 47 | Ctrl+L / Ctrl+E / Ctrl+R | Căn trái / giữa / phải cả đoạn | Đúng | giống (E2E) |
| 48 | Ctrl+J | Căn đều hai bên | **Không có lệnh, không có nút** (engine đã hỗ trợ) | **đã sửa** (E2E) |
| 49 | Ctrl+Shift+> / Ctrl+Shift+< | Cỡ chữ kế tiếp / trước đó **trong danh sách cỡ** (44 → 48 → 54…) | Cộng / trừ 2 pt (44 → 46) | **đã sửa** (E2E + vitest) — ghi chú (c) |
| 50 | Ctrl+] / Ctrl+[ khi đang sửa chữ | Tăng / giảm cỡ chữ | Không làm gì trong văn bản | **đã sửa** (E2E) |
| 51 | Ô cỡ chữ | Chọn cỡ trong danh sách 8…96 | **Không có ô cỡ chữ** (chỉ có hai nút tăng / giảm) | **đã sửa** (E2E) |
| 52 | Ô phông chữ, giãn dòng, màu chữ, tô sáng, gạch ngang | Áp cho vùng chọn | Đúng (`slide-rich.spec.ts`) | giống |
| 53 | Dấu đầu dòng / đánh số | Bật / tắt cho đoạn | Đúng | giống (E2E: nút phản ánh trạng thái) |
| 54 | Enter / Shift+Enter | Đoạn mới (tiếp tục dấu đầu dòng) / xuống dòng trong đoạn | Đúng | giống (E2E cho Enter; Shift+Enter đã quan sát qua Playwright, có vitest `editing`) |
| 55 | Tab / Shift+Tab ở đầu đoạn | Tăng / giảm cấp danh sách | Đúng | giống (E2E) |
| 56 | Tab giữa đoạn | Chèn ký tự tab | Vẫn đổi cấp danh sách | **còn khác** — văn bản slide của engine chưa có ký tự tab / điểm dừng tab |
| 57 | Backspace ở đầu đoạn có dấu đầu dòng | Lần 1 bỏ dấu đầu dòng (giữ chữ), lần 2 nối với đoạn trước | Nối đoạn ngay | **đã sửa** (E2E) |
| 58 | Ctrl+Enter | Sang placeholder kế tiếp (chọn hết chữ của nó); ở placeholder cuối → slide mới, con trỏ ở tiêu đề | Không làm gì | **đã sửa** (E2E) |
| 59 | Ctrl+A khi sửa chữ | Chọn toàn bộ chữ của hình | Đúng | giống |
| 60 | Bấm đúp / bấm ba trong văn bản | Chọn từ / chọn cả đoạn | Chỉ đặt con trỏ | **đã sửa** (E2E) — tham chiếu chọn kèm dấu cách sau từ, BlueSlide chỉ chọn từ |
| 61 | Kéo chuột, Shift+bấm, Shift+mũi tên, Home / End, Ctrl+←/→ | Chọn / di chuyển trong văn bản | Đúng (`slide-rich.spec.ts`) | giống |
| 62 | ↑ / ↓ trong đoạn nhiều dòng | Theo dòng hiển thị | Theo cách ngắt dòng của ô nhập ẩn, có thể lệch một từ so với dòng vẽ trên canvas | **còn khác** — con trỏ do `<textarea>` ẩn giữ (để IME tiếng Việt hoạt động) |
| 63 | Gõ đè lên vùng chọn; hoàn tác chữ đang gõ | Thay vùng chọn; Ctrl+Z hoàn tác từng đợt gõ | Đúng (một đợt gõ liên tục = một bước) | giống (E2E) |
| 64 | Tự co chữ / tự giãn hộp | Placeholder co chữ khi tràn (`normAutofit`); hộp văn bản tự cao theo chữ (`spAutoFit`) | Đúng | giống (E2E cho hộp văn bản; co chữ: `slide_edit/rich.rs`) |
| 65 | Ctrl+K | Hộp thoại liên kết cho vùng chọn | Đúng (`slide-actions.spec.ts`) | giống |
| 66 | Chổi sao chép định dạng (Ctrl+Shift+C / V) | Sao chép / dán định dạng | Không có | **còn thiếu** |
| 67 | Ctrl+T (hộp thoại phông), Ctrl+= / Ctrl+Shift++ (chỉ số dưới / trên), Shift+F3 (đổi chữ hoa thường), Ctrl+Space (xoá định dạng) | Có | Không có | **còn thiếu** |
| 68 | Tự sửa khi gõ (nháy cong, viết hoa đầu câu, 1. → danh sách số) | Mặc định bật | Không có trong BlueSlide | **còn thiếu** |

## 4. Tìm / thay thế, bảng tạm

| # | Thao tác | Hành vi tham chiếu | BlueSlide trước đây | Trạng thái |
| --- | --- | --- | --- | --- |
| 69 | Ctrl+F — Tìm | Hộp thoại: tìm gì, phân biệt hoa thường, nguyên từ; "Tìm tiếp" đi qua các slide, quay vòng | **Không có** | **đã sửa** (E2E + vitest); **còn khác chi tiết**: kết quả là *hình chứa chuỗi* được chọn trên slide của nó, chưa tô đúng từ trong văn bản; chưa tìm trong ghi chú và ô bảng |
| 70 | Ctrl+H — Thay thế / Thay tất cả | Thay từng chỗ rồi tìm tiếp; thay tất cả báo số chỗ đã thay | **Không có** | **đã sửa** (E2E + vitest; thay tất cả = một bước hoàn tác) |
| 71 | Ctrl+C / Ctrl+X / Ctrl+V với hình | Dán trên cùng slide lệch một chút, slide khác đúng vị trí cũ | Đúng (`paste.spec.ts`, `slide_edit/mod.rs` `DUPLICATE_OFFSET`) | giống |
| 72 | Dán ảnh từ chương trình khác | Ảnh nằm giữa slide | Đúng: giữa slide, tối đa 80 % khổ slide (`cmd_picture`) | giống |
| 73 | Dán chữ khi không sửa văn bản | Tạo hộp văn bản mới | Đúng (`paste.spec.ts`) | giống |
| 74 | Nút tuỳ chọn dán | Giữ định dạng nguồn / theo đích / ảnh / chỉ chữ | Đúng (`pasteOptions.tsx`, `paste.spec.ts`) | giống |
| 75 | Ctrl+Alt+V — Dán đặc biệt | Hộp thoại chọn dạng dán | Không có hộp thoại (dùng nút tuỳ chọn dán) | **còn thiếu** |
| 76 | F4 / Ctrl+Y lặp lại thao tác | Lặp lệnh định dạng / chèn gần nhất | F4 có; Ctrl+Y chỉ là "làm lại" | **còn khác** — Ctrl+Y không lặp lệnh khi không còn gì để làm lại |

## 5. Trình chiếu

| # | Thao tác | Hành vi tham chiếu | BlueSlide trước đây | Trạng thái |
| --- | --- | --- | --- | --- |
| 77 | F5 / Shift+F5 / Alt+F5 | Từ đầu / từ slide hiện tại / dạng xem người thuyết trình | Đúng | giống (E2E) |
| 78 | N, Space, →, ↓, Enter, PgDn | Bước kế tiếp | Đúng | giống (E2E) |
| 79 | P, Backspace, ←, ↑, PgUp | Bước trước | Đúng | giống (E2E) |
| 80 | Số + Enter | Tới slide có số đó | Chữ số bị bỏ qua, Enter sang slide kế | **đã sửa** (E2E) |
| 81 | Home / End | Slide đầu / cuối | Đúng | giống (E2E) |
| 82 | B hoặc "." / W hoặc "," | Màn hình đen / trắng, bấm lại để trở về | Đúng | giống (E2E) |
| 83 | Ctrl+P bút, Ctrl+I bút dạ quang, Ctrl+L laser, Ctrl+E tẩy, Ctrl+A mũi tên, E xoá nét vẽ | Đổi công cụ | Đúng | giống (E2E cho Ctrl+P) |
| 84 | Esc | Trả công cụ về mũi tên, lần nữa thì kết thúc; trở về slide vừa chiếu | Đúng | giống (E2E) |
| 85 | Hết bản trình chiếu | Màn hình đen "hết trình chiếu"; bước kế tiếp (phím hoặc bấm chuột) thoát | Ở lại màn hình cuối cho tới khi bấm Esc | **đã sửa** (E2E) |
| 86 | Slide ẩn | Bị bỏ qua khi chiếu | Đúng | giống (E2E) |
| 87 | H — tới slide ẩn kế tiếp | Có | Không có | **còn thiếu** |
| 88 | S (dừng / chạy tiếp tự động), Ctrl+H / Ctrl+U (ẩn con trỏ) | Có | Không có | **còn thiếu** |

## 6. Tệp, Xem, Xem lại, thanh trạng thái, Chèn, Thiết kế

| # | Thao tác | Hành vi tham chiếu | BlueSlide trước đây | Trạng thái |
| --- | --- | --- | --- | --- |
| 89 | Bản trình chiếu mới (Ctrl+N) | Một slide tiêu đề trống, khổ 16:9 | **Không có lệnh** — chỉ mở được tệp có sẵn | **đã sửa**: nút trên màn hình trống, Trang đầu › Tệp › Mới, Ctrl+N (E2E). Trên web trình duyệt giữ Ctrl+N cho cửa sổ mới — phím tắt chỉ có tác dụng ở bản desktop |
| 90 | Ctrl+O / Ctrl+S | Mở / lưu `.pptx` | Đúng | giống (E2E) |
| 91 | F12 | Lưu thành | Chỉ Ctrl+Shift+S | **đã sửa** (E2E; giữ cả Ctrl+Shift+S) |
| 92 | Ctrl+P | Bảng in | Đúng | giống |
| 93 | Màn hình Tệp | Thông tin, lưu, lưu thành, in, xuất | Đúng (`file-backstage.spec.ts`) | giống |
| 94 | Xem: Thường / Sắp xếp slide / Chế độ đọc (Esc quay lại) / Dàn ý / Trang ghi chú | Đổi dạng xem | Đúng | giống (E2E) |
| 95 | Ghi chú dưới slide; nút Hoàn tác / Làm lại | Gõ ghi chú theo từng slide | Đúng | giống (E2E) |
| 96 | Thanh trạng thái: số slide, thu phóng − / + / thanh trượt, vừa cửa sổ | Đúng | Đúng | giống (E2E) |
| 97 | F7 / Shift+F7 | Soát chính tả / từ đồng nghĩa | Đúng (`spelling-sheet-slide.spec.ts`, `thesaurus.spec.ts`) | giống |
| 98 | Ctrl+Alt+M | Bình luận mới (mở ngăn bình luận) | Không có phím tắt | **đã sửa** (E2E) |
| 99 | Alt+F10 | Ngăn chọn | Không có phím tắt | **đã sửa** |
| 100 | Chuyển trang / Hiệu ứng | Hiệu ứng chuyển áp cho slide hiện tại, "áp dụng cho tất cả"; hiệu ứng động cần đối tượng đang chọn | Đúng | giống (E2E) |
| 101 | Đổi bố cục slide | Trang đầu › Bố cục | Có, nằm ở tab Thiết kế | giống về chức năng (vị trí nút khác) |
| 102 | Shift+F9 lưới, Alt+F9 đường gióng, thước | Bật / tắt | Không có lưới / đường gióng tĩnh / thước (chỉ có đường gióng thông minh khi kéo) | **còn thiếu** |
| 103 | Đầu trang / chân trang, số slide, ngày giờ (Chèn) | Hộp thoại áp cho một / mọi slide | Không có hộp thoại | **còn thiếu** |
| 104 | Ctrl+Shift+Tab (đổi ngăn thumbnail ↔ dàn ý), F6 | Chuyển ngăn | F6 có (`a11y-keyboard.spec.ts`); dàn ý là một dạng xem riêng | **còn khác** — khác cách tổ chức ngăn |
| 105 | Chuột phải lên từ sai chính tả | Menu gợi ý chính tả cho từ đó; bấm chỗ khác của hình thì ra menu của hình | Đúng (menu gợi ý chỉ khi bấm trúng từ có gạch lượn — `spelling.tsx`) | giống |

## Phím tắt đã đối chiếu

| Phím | Tham chiếu | BlueSlide |
| --- | --- | --- |
| Ctrl+N / Ctrl+O / Ctrl+S / F12 / Ctrl+P | Mới / mở / lưu / lưu thành / in | Có (Ctrl+N, F12: mới thêm) |
| Ctrl+Z / Ctrl+Y / F4 | Hoàn tác / làm lại / lặp lại | Có |
| Ctrl+X / Ctrl+C / Ctrl+V | Cắt / sao chép / dán (hình, chữ, slide theo ngữ cảnh) | Có |
| Ctrl+A | Chọn mọi đối tượng / mọi chữ (trong văn bản) | Có; chưa có "mọi slide" trong ngăn slide |
| Ctrl+F / Ctrl+H | Tìm / thay thế | Mới thêm |
| Ctrl+M / Enter (thumbnail) | Slide mới | Có / mới thêm |
| Ctrl+D / Ctrl+Shift+D | Nhân bản đối tượng hoặc slide / nhân bản slide | Có / mới thêm |
| Ctrl+↑ ↓ / Ctrl+Shift+↑ ↓ (thumbnail) | Chuyển slide một vị trí / về đầu, cuối | Đã sửa |
| Delete / Backspace | Xoá đối tượng hoặc slide | Có / Backspace cho slide mới thêm |
| Mũi tên / Ctrl+mũi tên | Nudge / nudge nhỏ | Đã sửa (6 pt / 1 pt) |
| Shift+mũi tên / Ctrl+Shift+mũi tên | Đổi kích thước / bước nhỏ | Mới thêm |
| Alt+← / Alt+→ | Xoay 15° | Mới thêm |
| Tab / Shift+Tab | Hình kế / trước; trong văn bản: cấp danh sách; trong bảng: ô kế / trước | Có |
| Enter / F2 / Esc | Chọn hết chữ của hình / đổi qua lại chữ ↔ hình / lùi một mức | Đã sửa / có |
| Ctrl+Enter | Placeholder kế tiếp / slide mới | Mới thêm |
| Ctrl+G / Ctrl+Shift+G | Nhóm / bỏ nhóm | Có |
| Ctrl+] / Ctrl+[ / Ctrl+Shift+] / Ctrl+Shift+[ | Thứ tự lớp (hình) — cỡ chữ (trong văn bản) | Có / mới thêm |
| Ctrl+B / I / U, Ctrl+Shift+X | Đậm / nghiêng / gạch chân, gạch ngang (riêng BlueSlide) | Có |
| Ctrl+L / E / R / J | Căn trái / giữa / phải / đều | Có / Ctrl+J mới thêm |
| Ctrl+Shift+> / < | Cỡ chữ theo danh sách | Đã sửa |
| Ctrl+K | Liên kết | Có |
| F5 / Shift+F5 / Alt+F5 | Trình chiếu | Có |
| F7 / Shift+F7 | Chính tả / từ đồng nghĩa | Có |
| Ctrl+Alt+M / Alt+F10 | Bình luận mới / ngăn chọn | Mới thêm |
| Trong lúc chiếu: N P ← → ↑ ↓ Space Backspace PgUp PgDn Home End, số+Enter, B W . , Esc, Ctrl+P / I / L / E / A, E, G, Z | Như bảng 5 | Có (số+Enter mới thêm) |
| Ctrl+Shift+C / V, Ctrl+T, Ctrl+=, Shift+F3, Ctrl+Space, Ctrl+Alt+V, Shift+F9, Alt+F9, H, S (lúc chiếu) | Có trong tham chiếu | **Chưa có** |

## Lỗi phát hiện khi kiểm thử (đã sửa kèm)

- **Gõ nhanh rồi bấm phím di chuyển bị mất ký tự / con trỏ nhảy về cuối.** Ô nhập ẩn của hình đồng
  bộ theo kết quả engine trong một effect chạy trễ; khi kết quả của phím _trước_ được xử lý sau khi
  phím _sau_ đã được trả lời, văn bản cũ ghi đè lên ô nhập (mất chữ vừa gõ, con trỏ về cuối). Tái
  hiện 1/8 lần với "gõ `Hai` → `Home` → `Backspace`" cả khi vào sửa chữ bằng bấm đúp (lỗi có từ
  trước đợt này). Nay ô nhập bỏ qua tiếng vọng của các lần gõ cũ (`SlideOverlay.tsx`, `sent`);
  chạy lại 16/16 lần đúng.
- **Phím gõ ngay sau khi bấm chọn hình bị bỏ qua** khi trình xử lý phím còn giữ vùng chọn của lần
  vẽ trước: nay đọc vùng chọn từ kết quả engine mới nhất; các phím gõ trong lúc ô nhập đang mở
  được nối đúng thứ tự vào sau phím đầu tiên.

## Việc còn lại (theo mức dùng hằng ngày)

1. Chế độ vẽ hình / hộp văn bản bằng bấm-kéo trên slide (#43, #44) và lưới chọn kích thước bảng (#45).
2. Chọn nhiều slide trong ngăn slide và trong Sắp xếp slide (#16).
3. Chổi sao chép định dạng, chỉ số trên / dưới, đổi chữ hoa thường, xoá định dạng (#66, #67).
4. Ký tự tab trong văn bản slide (#56) — cần engine hỗ trợ `a:tab` / điểm dừng tab.
5. Tìm kiếm tô đúng từ trong văn bản, tìm cả ghi chú và ô bảng (#69).
6. Lưới, đường gióng tĩnh, thước (#102); đầu trang / chân trang / số slide (#103).
7. Tự sửa khi gõ (#68); H / S / ẩn con trỏ lúc chiếu (#87, #88); Ctrl+kéo tay nắm (#39); nhân bản giữ khoảng cách (#30).
