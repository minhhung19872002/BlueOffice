# ADR-0015: Sửa phương trình tại chỗ — con trỏ toán học (vị trí đoạn + đường dẫn ô + offset)

- Trạng thái: Chấp nhận · Ngày: 2026-11-11
- Liên quan: parity CORE-MATH-010; `crates/bo-math/src/edit.rs`, `crates/bo-engine/src/math_edit.rs`,
  `bo_layout::equation_geometry`; ADR-0008 (transaction op khả nghịch + cầu lệnh JSON).

## Bối cảnh
Trong mô hình, một phương trình (vùng Office Math, ECMA-376 Part 1, §22.1.2.77 `m:oMath` / §22.1.2.78
`m:oMathPara`) là **một đối tượng nội dòng**: một `RunContent::Markup` giữ XML gốc, chiếm một ký tự giữ chỗ U+FFFC
trong index text. Vị trí con trỏ là (id đoạn, offset byte) — không đi được vào bên trong. Trước đây phương trình chỉ
sửa qua hộp thoại dạng tuyến tính (UnicodeMath / LaTeX, `bo-math`). Cần: con trỏ vào được vùng toán, đi qua cấu trúc
(tử / mẫu, cơ số / chỉ số, căn, n-ary, ngoặc, ma trận, hàm, dấu), gõ / xóa trong ô, tự dựng (autobuild) `x^2␣`,
`a/b␣`, `\sqrt␣`, `\alpha`… mà không phá hoàn tác, cộng tác hay OMML khi lưu.

## Quyết định
1. **Con trỏ toán học** (`MathCaret` trong `Engine`, không trong `bo_edit::Selection`): `at` = vị trí bắt đầu của
   phương trình trong thân văn bản (đoạn, offset), `pos` = `bo_math::edit::MathPos { path, offset }` và cờ
   `selected`. `path` là dãy `Step { item, slot }` từ vùng xuống một *ô* (slot): `item` = chỉ số cấu trúc trong ô
   cha, `slot` = chỉ số ô của cấu trúc theo thứ tự đọc (phân số: tử, mẫu; chỉ số: cơ số, dưới, trên; căn: bậc, biểu
   thức; n-ary: cận dưới, cận trên, biểu thức; ngoặc: các phần; ma trận: ô theo hàng; hàm: tên, đối số; giới hạn:
   cơ số, giới hạn; dấu / gạch: cơ số). `offset` đếm ký tự của các run (`m:r`) cộng 1 cho mỗi cấu trúc.
   Trong khi con trỏ toán đang bật, lựa chọn chính đứng ngay sau phương trình (các lệnh phương trình cũ vẫn thấy nó).
2. **Mô hình sửa là cây `bo_math::Node`** (đã có, dùng chung với hộp thoại): đọc bằng `from_omml`, sửa trên cây,
   ghi bằng `to_omml` — không viết bộ phân tích thứ hai. Autobuild dùng đúng `unicode_math::parse` (kể cả bảng
   `\từ_khóa`) trên phần chữ vừa gõ trước con trỏ; `^`/`_` gõ ngay sau một cấu trúc thì gắn chỉ số cho cấu trúc đó.
3. **Mỗi thay đổi = một op khả nghịch**: `Session::replace_equation(at, xml, group)` thay XML của đối tượng (giữ định
   dạng run) bằng `Tx::replace_para` — một transaction bình thường (ADR-0008) nên undo / redo, cộng tác (Yrs) và
   thông báo thay đổi khối hoạt động như sửa văn bản. Gõ từng ký tự liên tiếp trong cùng phương trình gộp thành
   **một bước hoàn tác** (`group`, như gõ chữ); di chuyển, autobuild, xóa cấu trúc là bước riêng (hoàn tác autobuild
   trả về chữ tuyến tính đã gõ). Sau mỗi lần ghi, cây được chuẩn hóa (`edit::normalize`) và con trỏ được kẹp
   (`edit::clamp`) theo cây đọc lại; sau undo / redo cũng kẹp như vậy.
4. **Lệnh engine**: `mathEnter`, `mathMove { motion }`, `mathInsert`, `mathDelete { back }`, `mathAutobuild`.
   Khi con trỏ toán bật, engine tự định tuyến các lệnh soạn thảo thường (`insertText`, `deleteBackward/Forward`,
   `move` trái/phải/Home/End, `tab`, `click`) vào phương trình — UI không cần biết trạng thái để gửi phím; lệnh khác
   (Enter, dán, định dạng…) thoát vùng trước. Mũi tên đi vào phương trình từ hai bên; nhấp vào phương trình đặt con
   trỏ vào ô dưới điểm nhấp (ô nhỏ nhất chứa điểm, điểm dừng gần nhất). Backspace / Delete ở mép ô hay cạnh một cấu
   trúc: lần 1 chọn cấu trúc (vẽ vùng chọn), lần 2 xóa nó.
5. **Hình học từ layout**: `bo_layout` ghi lại hình học của mỗi ô khi dàn trang toán (`MathSlotBox`: điểm dừng con
   trỏ, hộp từng nút, trên / dưới), đánh số ô theo tiền thứ tự trên XML khớp với `edit::all_slots` trên cây (ô ẩn
   `degHide`/`subHide`/`supHide` và ô rỗng của chúng không phải ô). `equation_geometry` dàn lại vùng ở dạng ghi từ cây
   với đúng kiểu run như đoạn văn; engine cộng gốc của đối tượng (điểm dừng con trỏ đầu đối tượng, đường cơ sở dòng).
   Ô rỗng giữ bề rộng một ô giữ chỗ (0,6 em) trong layout; khung chấm của nó và nền vùng toán chỉ vẽ ở lớp phủ màn
   hình (không in). Kết quả lệnh trả `caret` / `rects` của con trỏ toán và `math` (đường dẫn, offset, hộp vùng, ô rỗng).

## Hệ quả
- Không đổi định dạng vị trí của mô hình (Pos vẫn là đoạn + offset): mọi tính năng khác không bị ảnh hưởng.
- Lần sửa đầu tiên viết lại vùng ở dạng chuẩn của `bo-math`: phần tử không có trong cây (`m:box`, `m:groupChr`,
  `m:sPre`, `m:borderBox`, `m:phant`, thuộc tính run ngoài `sty`/`nor`) không giữ được; với vùng như vậy hình học con
  trỏ tính theo dạng chuẩn nên có thể lệch nhẹ so với hình vẽ cho tới lần sửa đầu.
- Khi bật theo dõi thay đổi, sửa trong phương trình không ghi thành revision (thay thẳng đối tượng).
- Chỉ hỗ trợ thân văn bản (không trong header/footer, chú thích, hộp văn bản); Lên / Xuống thoát vùng rồi đi theo dòng.
