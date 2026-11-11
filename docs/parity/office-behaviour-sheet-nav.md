# BlueSheet — so sánh thao tác di chuyển, chọn, nhập và sửa với bảng tính tham chiếu

Phạm vi: khu vực **nav** của đợt kiểm tra hành vi BlueSheet (2026-11-11) — di chuyển, chọn
vùng, gõ và sửa ô, gõ công thức, AutoComplete, hoàn tác, sao chép / cắt / dán. Hai khu vực
khác (định dạng, dữ liệu) có tài liệu riêng. Hành vi tham chiếu được viết từ tài liệu công khai
và quan sát hộp đen của ứng dụng bảng tính phổ biến (Microsoft Excel); không dịch ngược, không
chép chuỗi giao diện.

Kiểm chứng trên giao diện thật (Playwright, bấm phím / chuột trên lưới):
`tests/e2e/specs/office-behaviour-sheet-nav.spec.ts` và
`tests/e2e/specs/office-behaviour-sheet-nav-paste.spec.ts` (dự án `clipboard`). Kiểm thử đơn vị:
`packages/sheet-editor/test/formulaEdit.test.ts`, `crates/bo-engine/src/sheet/cmd.rs`
(`ctrl_arrow_lands_on_the_edge_of_the_data`, `ctrl_enter_fills_every_selected_cell_in_one_step`,
`column_entries_are_the_text_of_the_list_around_the_cell`).

Trạng thái: **giống** = đã giống trước đợt này; **đã sửa** = khác / thiếu, đã sửa trong đợt này;
**còn khác** = vẫn khác, có lý do.

## Di chuyển bằng Enter / Tab

| Thao tác | Hành vi tham chiếu | BlueSheet trước | Trạng thái |
| --- | --- | --- | --- |
| Enter / Shift+Enter | Ô dưới / ô trên; vùng chọn giữ nguyên khi chọn nhiều ô | Đúng | giống |
| Tab / Shift+Tab | Ô phải / ô trái | Đúng | giống |
| Tab…Tab…Enter | Enter về hàng sau, **cột bắt đầu loạt Tab** (nhập theo hàng của danh sách) | Enter xuống thẳng cột hiện tại | đã sửa |
| Mũi tên cắt ngang loạt Tab | Enter sau đó đi thẳng xuống | — | đã sửa (cùng lúc) |
| Enter / Tab trong vùng chọn nhiều ô | Đi vòng trong vùng (xuống từng cột, rồi cột kế), vùng không bị bỏ chọn | Đúng | giống |

## Gõ và sửa ô

| Thao tác | Hành vi tham chiếu | BlueSheet trước | Trạng thái |
| --- | --- | --- | --- |
| Gõ khi đang chọn ô | Thay nội dung ô, chế độ **Nhập** | Đúng | giống |
| Esc | Huỷ nội dung đang gõ, ô giữ giá trị cũ | Đúng | giống |
| Delete | Xoá nội dung, giữ định dạng | Đúng | giống |
| Backspace | Xoá nội dung và vào chế độ Nhập | Đúng | giống |
| Alt+Enter | Xuống dòng trong ô | Không làm gì (ô nhập là một dòng `input`) | đã sửa (ô nhập và thanh công thức là `textarea`, ô cao theo số dòng) |
| Mũi tên ở chế độ Nhập | Ghi ô rồi di chuyển | Đúng | giống |
| F2 | Vào chế độ **Sửa** (mũi tên di chuyển con trỏ); F2 lần nữa: về chế độ Nhập | Vào Sửa được, F2 lần hai không làm gì | đã sửa |
| Ctrl+Enter | Ghi nội dung vào **mọi ô đang chọn** (công thức dịch tham chiếu theo từng ô), vùng chọn giữ nguyên, một bước hoàn tác | Chỉ ghi ô hiện hành và xuống dòng | đã sửa (lệnh engine `fillInputs`) |
| Ctrl+Shift+Enter | Công thức mảng kiểu cũ | Ghi như Enter | còn khác — engine dùng mảng động (công thức tràn kết quả) nên Enter đã cho kết quả mảng; chưa có công thức mảng `{=…}` kiểu cũ |
| Thanh công thức | Sửa giống trong ô (Enter, Tab, Esc, Alt+Enter, F4, danh sách hàm, chọn tham chiếu bằng chuột) | Chỉ Enter / Esc / F4; Enter luôn xuống, Tab rời thanh | đã sửa (dùng chung bộ xử lý phím với ô) |
| Mở rộng thanh công thức (Ctrl+Shift+U, nút ▾) | Hiện nhiều dòng | Không có | đã sửa |

## Nhảy và chọn vùng

| Thao tác | Hành vi tham chiếu | BlueSheet trước | Trạng thái |
| --- | --- | --- | --- |
| Ctrl+mũi tên | Tới **mép vùng dữ liệu**: trong khối dữ liệu → ô cuối khối; ô trống hoặc mép khối → ô có dữ liệu kế tiếp; không còn → mép trang tính | Nhảy cố định 50 ô (sai) | đã sửa (lệnh engine `dataEdge`) |
| Ctrl+Shift+mũi tên | Mở rộng tới cùng điểm đó, tính từ đầu di động của vùng | Mở rộng 50 ô | đã sửa |
| Home | Về cột A cùng hàng | Không làm gì | đã sửa |
| Ctrl+Home | Về A1 (ô đầu vùng cuộn nếu đã cố định ngăn) | Không làm gì | đã sửa |
| Ctrl+End | Ô cuối dùng tới (hàng cuối × cột cuối có dữ liệu) | Không làm gì | đã sửa |
| Ctrl+Shift+Home / End | Mở rộng tới A1 / ô cuối | Không làm gì | đã sửa |
| PgDn / PgUp | Xuống / lên một màn hình | Đúng | giống |
| Alt+PgDn / Alt+PgUp | Sang phải / trái một màn hình | Đi xuống một màn hình như PgDn | đã sửa |
| Ctrl+Space / Shift+Space | Chọn cả cột / cả hàng (ô hiện hành giữ nguyên) | Không làm gì (Shift+Space gõ dấu cách) | đã sửa — vùng chọn giống khi bấm tiêu đề cột / hàng của BlueSheet |
| Ctrl+A | Lần 1: vùng dữ liệu quanh ô; lần 2: cả trang; ô trống đứng riêng: cả trang ngay | Luôn chọn A1 tới ô cuối dùng tới | đã sửa |
| Shift+click, Ctrl+click, kéo chuột | Mở rộng / thêm vùng / chọn vùng | Đúng | giống |
| F5 / Ctrl+G | Hộp thoại Đi tới | Đúng | giống |
| Hộp tên: gõ địa chỉ hoặc tên đã đặt | Chọn vùng đó | Đúng; Enter trong Hộp tên lại rơi một dấu xuống dòng vào ô nhập sau khi đổi sang `textarea` | giống (đã chặn dấu xuống dòng lạc) |
| Chọn cả cột / hàng / trang | Cả 1 048 576 hàng / 16 384 cột; Hộp tên hiện ô hiện hành (`C1`) hoặc `C:C` khi đang kéo | Vùng hàng đã dùng (ít nhất 1000 hàng, 26 cột); Hộp tên hiện cả vùng (`C1:C1000`) | còn khác — giữ quy ước sẵn có của BlueSheet (tiêu đề cột/hàng, lệnh định dạng chạy trên vùng hữu hạn); đổi cách Hộp tên hiển thị sẽ ảnh hưởng các đặc tả E2E khác |
| Ctrl+mũi tên qua hàng / cột ẩn | Bỏ qua hàng / cột ẩn | Chưa bỏ qua | còn khác — chưa làm |

## Gõ công thức

| Thao tác | Hành vi tham chiếu | BlueSheet trước | Trạng thái |
| --- | --- | --- | --- |
| Mũi tên sau `=`, toán tử, `(`, dấu phân cách (chế độ Chỉ) | Chọn ô làm tham chiếu; mũi tên tiếp theo dời nó, Shift mở rộng; gõ tiếp thì kết thúc chọn | Ghi ô và di chuyển (công thức dở bị ghi) | đã sửa |
| Bấm chuột / kéo chuột vào ô khi đang gõ công thức | Chèn tham chiếu ô / vùng | Ghi công thức dở rồi chọn ô | đã sửa |
| Bấm thẻ trang tính khác khi đang gõ | Chèn `Trang2!`, chọn ô trên trang đó; Enter ghi vào ô gốc và quay về trang gốc | Đổi trang và mất công thức đang gõ | đã sửa (tên trang có dấu cách / ký tự đặc biệt được bọc `'…'`) |
| Tô màu tham chiếu | Mỗi tham chiếu một màu; khung màu cùng màu quanh vùng trên lưới | Không có | đã sửa (khung màu trên lưới; chữ tham chiếu trong ô cùng màu) |
| F4 | Đổi `A1` → `$A$1` → `A$1` → `$A1` | Đúng | giống |
| Gợi ý tên hàm | Danh sách hàm khớp phần đã gõ; mũi tên chọn, Tab nhận (`TÊN(`) | Không có | đã sửa |
| Gợi ý đối số | Ô nhỏ dưới ô nhập: cú pháp hàm, đối số hiện tại đậm | Không có | đã sửa — có cho khoảng 55 hàm thông dụng (tên đối số là cách gọi riêng của BlueOffice) |
| Dấu phân cách đối số theo vùng | `;` với thiết lập vùng Việt Nam | Engine dùng `,` | thuộc khu vực **data** |

## AutoComplete và chọn từ danh sách

| Thao tác | Hành vi tham chiếu | BlueSheet trước | Trạng thái |
| --- | --- | --- | --- |
| Gõ đầu một mục chữ đã có trong cột | Đề xuất phần còn lại (bôi đen) khi chỉ một mục khớp; Enter nhận; xoá thì không đề xuất lại | Không có | đã sửa (lệnh engine `columnEntries`: các ô liền khối trên / dưới của cột) |
| Alt+↓ trên ô không có xác thực | Danh sách các mục chữ của cột, sắp xếp; mũi tên + Enter chọn | Chỉ mở với ô có xác thực kiểu Danh sách | đã sửa (cả danh sách xác thực cũng chọn được bằng bàn phím) |

## Hoàn tác

| Thao tác | Hành vi tham chiếu | BlueSheet trước | Trạng thái |
| --- | --- | --- | --- |
| Một ô đã ghi | Một bước | Đúng | giống |
| Ctrl+Enter vào nhiều ô | Một bước | — (mới) | đã sửa |
| Dán | Một bước | Đúng | giống |
| Ctrl+D / điền | Một bước | Đúng | giống |

## Sao chép / cắt / dán

| Thao tác | Hành vi tham chiếu | BlueSheet trước | Trạng thái |
| --- | --- | --- | --- |
| Đường viền nhấp nháy sau Ctrl+C / Ctrl+X | Còn tới khi Esc, gõ một mục, hoặc dán vùng cắt | Không có | đã sửa |
| Enter khi có vùng đang được đánh dấu sao chép | Dán một lần rồi bỏ đánh dấu | Chỉ di chuyển xuống | đã sửa |
| Ctrl+V | Dán, vùng sao chép vẫn được đánh dấu để dán tiếp | Dán được | đã sửa (đánh dấu) |
| Dán vào vùng chọn là bội số kích thước đã sao chép | Lặp lại cho đủ vùng | Đúng | giống |
| Cắt rồi dán | Di chuyển ô; công thức trỏ tới ô đó đi theo | Đúng | giống |
| Nút tuỳ chọn dán, Ctrl+Alt+V | Có | Đúng | giống |
| Chuột phải trong vùng chọn | Giữ vùng chọn | Đúng | giống |

## Lỗi đã sửa thêm

- Enter trong Hộp tên (và các ô nhập khác chuyển tiêu điểm về lưới khi nhấn Enter) không còn
  để lại dấu xuống dòng trong ô nhập.
- Danh sách hàm không còn hiện muộn (sau khi đã gõ tiếp) và chặn Esc huỷ ô.

## Phím tắt đã so sánh

`Enter`, `Shift+Enter`, `Tab`, `Shift+Tab`, `Esc`, `Delete`, `Backspace`, `F2`, `Alt+Enter`,
`Ctrl+Enter`, `Ctrl+Shift+Enter`, `←↑→↓`, `Ctrl+←↑→↓`, `Ctrl+Shift+←↑→↓`, `Shift+←↑→↓`, `Home`,
`Ctrl+Home`, `Ctrl+End`, `Ctrl+Shift+Home`, `Ctrl+Shift+End`, `PgUp`, `PgDn`, `Alt+PgUp`,
`Alt+PgDn`, `Ctrl+Space`, `Shift+Space`, `Ctrl+A`, `F5`, `Ctrl+G`, `F4`, `Tab` (danh sách hàm),
`Alt+↓`, `Ctrl+Shift+U`, `Ctrl+Z`, `Ctrl+Y`, `Ctrl+C`, `Ctrl+X`, `Ctrl+V`, `Ctrl+Alt+V`,
`Ctrl+D`.

## Còn khác

- Hộp tên hiện cả vùng chọn (`B2:C3`) thay vì ô hiện hành; chọn cả cột / hàng / trang là vùng
  hữu hạn (xem trên).
- Ctrl+mũi tên chưa bỏ qua hàng / cột ẩn; Ctrl+A trong bảng (Table) chưa có ba bậc (dữ liệu →
  dữ liệu + tiêu đề → trang).
- Chưa có công thức mảng kiểu cũ bằng Ctrl+Shift+Enter (engine dùng mảng động).
- Gợi ý đối số chỉ có cho các hàm thông dụng trong `FUNCTION_ARGS`
  (`packages/sheet-editor/src/formulaEdit.ts`); hàm khác chỉ có danh sách tên.
- Khi chọn tham chiếu bằng mũi tên, khung vùng đang chọn vẽ liền (bảng tính tham chiếu vẽ nét
  đứt nhấp nháy).
- Mũi tên đầu tiên sau khi bấm thẻ trang khác đi từ A1 của trang đó (bảng tính tham chiếu đi từ
  ô hiện hành đã nhớ của trang đó — BlueSheet chưa nhớ ô hiện hành theo từng trang).
