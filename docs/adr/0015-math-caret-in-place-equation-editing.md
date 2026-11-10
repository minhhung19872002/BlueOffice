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
- Lần sửa đầu tiên viết lại vùng ở dạng chuẩn của `bo-math` — vì vậy vùng có gì cây không giữ được thì **không** vào
  sửa tại chỗ (phụ lục A).
- Khi bật theo dõi thay đổi, sửa trong phương trình là một revision của cả phương trình (phụ lục B).
- Chỉ hỗ trợ thân văn bản (không trong header/footer, chú thích, hộp văn bản); Lên / Xuống thoát vùng rồi đi theo dòng.

## Phụ lục (2026-11-11, quyết định của chủ dự án)

### A. Không sửa tại chỗ vùng không giữ nguyên được
- `mathEnter`, nhấp và mũi tên vào phương trình chỉ bật con trỏ toán khi **đọc vào cây rồi ghi lại cho đúng
  markup cũ** (`math_edit::lossless`: `from_omml` → `normalize` → `to_omml` → `equation::wrap`, so bằng
  `bo_math::read::same_omml`). Dạng chuẩn để so chỉ bỏ những gì không mang thông tin: thứ tự thuộc tính, khai báo
  `xmlns`, khoảng trắng giữa phần tử, `xml:space` trên `m:t`, phần tử thuộc tính rỗng (`m:…Pr` không nội dung) và cách
  chia chữ thành các `m:r` liền nhau cùng thuộc tính. Mọi thứ khác (`m:box`, `m:groupChr`, `m:sPre`, `m:borderBox`,
  `m:phant`, `w:rPr` / `m:ctrlPr` trong vùng, `m:rPr` ngoài `sty`/`nor`, `m:oMathParaPr`, nhiều `m:oMath` trong một
  `m:oMathPara`, …) làm vùng bị từ chối. Phương trình bị đánh dấu xóa (revision) cũng không sửa được.
- Bị từ chối: engine chọn cả phương trình (như trước khi có sửa tại chỗ) và trả `error: "mathUnsupported"`; UI hiện
  thông báo ở thanh trạng thái (`equation.inPlaceUnsupported`). Gõ chữ ngay sau đó (chưa đổi vùng chọn / tài liệu)
  không thay phương trình (vẫn trả `mathUnsupported`); Delete / Backspace vẫn xóa được. Enter khi đang chọn đúng một
  phương trình, hoặc nhấp đúp vào phương trình (engine chọn nó), mở hộp thoại phương trình (`insert.equation`) như cũ.
  Mũi tên đi qua phương trình như trước.
- Hệ quả: phương trình do BlueDoc tạo (hộp thoại, mẫu, LaTeX) luôn sửa tại chỗ được; phương trình do ứng dụng khác viết
  (`w:rPr` phông toán trong từng run, `m:ctrlPr`, …) sửa tại chỗ được từ phụ lục C — danh sách phần tử bị từ chối ở
  trên được thu hẹp ở đó.
- Kiểm thử: `math_edit::tests::unsupported_markup_is_not_edited_in_place` (mỗi phần tử: mathEnter, nhấp, gõ, mũi tên,
  lưu — XML không đổi), `supported_equations_still_enter`, corpus `tests/corpus/doc/math-06-unsupported.docx`, E2E
  `equation-inline.spec.ts`.

### C. Cây giữ thuộc tính: phương trình do ứng dụng khác viết cũng sửa tại chỗ (2026-11-11)
- Quyết định của chủ dự án: cây phương trình giữ những gì ứng dụng khác viết, để phần A chỉ còn từ chối thứ thật sự
  chưa mô hình hóa. `bo_math::Node` nay có:
  - mỗi run `Node::Run(text, style, RunPr)`: `RunPr.math` = các con của `m:rPr` (lit, nor, scr, sty, brk, aln) nguyên
    văn, `RunPr.text` = phần tử `w:rPr` nguyên văn (phông toán, cỡ, màu, đậm…; con lạ giữ nguyên);
  - mỗi cấu trúc trường `pr: Pr` = các con của phần tử thuộc tính (`m:fPr`, `m:dPr`, `m:naryPr`, `m:radPr`,
    `m:mPr`/`m:mcs`, `m:eqArrPr`, `m:accPr`, `m:barPr`, `m:funcPr`, `m:limLowPr`, `m:sSupPr`…) **kể cả `m:ctrlPr`**,
    nguyên văn theo thứ tự trong tệp;
  - cấu trúc mới `GroupChr`, `Boxed` (`m:box`), `BorderBox`, `Phantom` (`m:phant`), `PreScripts` (`m:sPre`): ô là cơ số
    (sPre: chỉ số dưới, trên, cơ số — đúng thứ tự XML); layout đã vẽ chúng, nay đánh số ô (`slot_ids`) nên đi con trỏ,
    gõ, xóa, nhấp như mọi cấu trúc;
  - `m:oMathParaPr` của vùng hiển thị giữ nguyên (`read::para_props`, `equation::wrap_with`).
- Giá trị mô hình hóa (thanh phân số, ký tự ngoặc, chr / ẩn cận của n-ary, ẩn bậc căn, dấu, vị trí gạch, kiểu run) vẫn
  là trường của nút; `Pr.read` lưu chữ ký của chúng lúc đọc. Khi ghi: giá trị không đổi → các con ghi lại nguyên văn
  (byte như tệp nếu tệp viết theo dạng chuẩn của bộ ghi); đổi (vd. xóa hết cận dưới → `subHide`) → chỉ phần tử của giá
  trị đó được đặt / thêm đúng vị trí theo thứ tự lược đồ / bỏ, phần còn lại giữ (`limLoc`, `grow`, `ctrlPr`…).
  `Pr::default()` (cấu trúc tạo trong BlueDoc) ghi đúng dạng chuẩn cũ — OMML do BlueDoc tạo không đổi byte nào.
- Quy tắc sửa: gõ vào run → vào chính run đó (giữ `w:rPr`, `m:rPr`); run mới ở ô không có run cạnh con trỏ lấy `w:rPr`
  của run gần nhất trong ô, rồi `m:ctrlPr` của cấu trúc bao, rồi của run bất kỳ trong vùng (`edit::ambient`); autobuild:
  run tạo ra lấy định dạng của run đang gõ (kiểu đặt lại theo `Style`, `RunPr::inherit`), cấu trúc tạo ra có
  `m:ctrlPr` chứa `w:rPr` đó (`Pr::with_ctrl`); xóa không đụng thuộc tính láng giềng; hai run liền nhau chỉ gộp khi cùng
  kiểu và cùng thuộc tính.
- Vẫn từ chối (so `same_omml` thấy khác): nhiều `m:oMath` trong một `m:oMathPara`, `w:ins`/`w:del`/bookmark bên trong
  vùng, `m:argPr`, run có nội dung khác `m:t` (`w:br`, `w:t`…), bậc căn / cận ẩn mà có nội dung. Corpus
  `math-06-unsupported.docx` nay chỉ chứa các trường hợp này; `math-07-foreign-style.docx` chứa phương trình kiểu ứng
  dụng khác (phông toán mọi run, `m:ctrlPr`, thuộc tính khác mặc định, groupChr/box/borderBox/phant/sPre,
  `m:oMathParaPr`) — đều sửa tại chỗ.
- Kiểm thử: `bo-math/tests/foreign_style.rs` (round-trip đúng byte, gõ, ô rỗng, autobuild, xóa, đổi giá trị đặt tại
  chỗ, 5 cấu trúc mới: điều hướng + round-trip), `bo-engine` `math_edit::tests::foreign_style_equations_are_edited_in_
  place_and_keep_their_properties`, `autobuild_in_a_foreign_equation_keeps_the_math_font`,
  `corpus_foreign_style_is_edited_in_place_and_unsupported_is_not`, `unsupported_markup_is_not_edited_in_place`; E2E
  `equation-inline.spec.ts` (mở `math-07`, sửa tại chỗ, XML lưu giữ `w:rFonts` trên mọi run).

### B. Theo dõi thay đổi: một revision cho cả phương trình
- Cách ghi (cái `bo-edit` vốn có và round-trip được): revision nằm trên `RunProps` của nguyên tử phương trình, nên
  `w:ins` / `w:del` (ECMA-376 Part 1, §17.13.5.18 / §17.13.5.14) bao **cả** `m:oMath` / `m:oMathPara`
  (`CT_RunTrackChange` cho phép nội dung toán). Không dùng `w:ins`/`w:del` quanh từng `m:r` hay trong `m:ctrlPr`.
  `bo-docx` trước đây bỏ dấu revision của vùng toán khi đọc — nay giữ (`reader.rs`), nên lưu + mở lại giữ revision.
  ```xml
  <w:del w:id="1" w:author="An" w:date="…"><m:oMath>…bản cũ, nguyên byte…</m:oMath></w:del>
  <w:ins w:id="2" w:author="An" w:date="…"><m:oMath>…bản đã sửa…</m:oMath></w:ins>
  ```
- `Session::replace_equation` khi bật theo dõi: lần sửa đầu đánh dấu vùng cũ là xóa (giữ nguyên XML) và chèn bản sửa
  ngay sau nó với dấu chèn; con trỏ toán chuyển sang bản sửa (hàm trả vị trí mới). Các lần sửa sau trên phương trình
  do chính tác giả đó chèn (theo `Tracker::owns`, như gõ trong phần mình đã chèn) thay tại chỗ — một phiên sửa (và cả
  phiên sau trên cùng phương trình đó) chỉ để lại **một** revision; gõ liên tiếp vẫn gộp một bước hoàn tác.
- Hai id khác nhau (id revision phải duy nhất, §17.13.1), ghép theo cấu trúc: vùng toán bị xóa đứng liền trước vùng
  toán được chèn, cùng tác giả, mỗi id chỉ đánh dấu đúng vùng đó (`revisions::equation_pairs`, mọi story). Danh sách
  revision gộp hai nửa thành một mục `RevKind::Equation` (id = id phần xóa; UI: "Sửa phương trình"); chấp nhận / từ
  chối một id giải quyết cả hai nửa (`with_equation_halves`, như hai nửa của một lần di chuyển): từ chối trả đúng XML
  cũ, chấp nhận giữ bản mới. Ngày revision lấy theo đồng hồ host như các sửa khác.
- Kiểm thử: `bo-edit` `track_tests::tracked_equation_replacement_is_one_revision`, `bo-engine`
  `math_edit::tests::tracked_edit_is_one_revision_of_the_whole_equation` (lưu + mở lại, OpenXmlValidator sạch trên
  `tests/fidelity/output/math-tracked-edit.docx`), E2E (ngăn xem lại có một thay đổi; từ chối trả lại phương trình gốc).
