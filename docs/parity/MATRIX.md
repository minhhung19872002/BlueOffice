# Ma trận tương đương tính năng (Parity MATRIX)

Mức: ✅ đầy đủ · 🟡 một phần · ⬜ chưa có · — không áp dụng. Nguồn đối chiếu: corpus tự sinh + OpenXmlValidator + ảnh
mốc tự sinh (`docs/fidelity-visual.md`); **chưa có ảnh mốc từ phần mềm tham chiếu** (xem `REFERENCE_GUIDE.md`).
Cập nhật sau mỗi hạng mục BACKLOG.

## Tổng hợp (theo hàng tính năng bên dưới; ✅ = 1, 🟡 = 0,5)

| App / nhóm | Hàng | Điểm | % |
|---|---|---|---|
| Chung (CORE) | 32 | 11 | 34 % |
| BlueDoc (DOC) | 18 | 8 | 44 % |
| BlueSheet (SHEET) | 20 | 8 | 40 % |
| BlueSlide (SLIDE) | 12 | 4 | 33 % |
| Hàm bảng tính | 515 hàm | 275 | 53 % (`functions.md`) |

## Chung

| Tính năng | App | Đọc file | Hiển thị | Tạo/sửa | Round-trip | Web | Desktop | Cộng tác | Test | Trạng thái | Ghi chú |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Phương trình OMML | Doc | ✅ | 🟡 | ⬜ | ✅ | 🟡 | 🟡 | ✅ | corpus math-0* | 🟡 | dàn trang xấp xỉ, chưa font toán/MATH table, chưa soạn |
| Phương trình trong Slide/Sheet | Slide/Sheet | ✅ | ⬜ | ⬜ | ✅ | ⬜ | ⬜ | — | — | ⬜ | giữ nguyên |
| Soát chính tả Việt/Anh | all | — | ⬜ | ⬜ | — | ⬜ | ⬜ | — | bo-spell | 🟡 | engine + tiếng Việt (quy tắc) + tiếng Anh (SCOWL) xong; chưa gạch đỏ/UI |
| Ngữ pháp/văn phong | all | — | ⬜ | ⬜ | — | ⬜ | ⬜ | — | — | ⬜ | |
| AutoCorrect | all | — | — | ⬜ | — | ⬜ | ⬜ | — | — | ⬜ | |
| Tìm/thay (regex, định dạng) | all | — | ✅ | ✅ | — | ✅ | ✅ | — | E2E | 🟡 | thiếu ký tự đặc biệt ^p, wildcard |
| SmartArt | all | 🟡 | 🟡 | ⬜ | ✅ | 🟡 | 🟡 | — | corpus smartart-* | 🟡 | chỉ drawing fallback |
| Biểu đồ c: cơ bản (9 loại) | Sheet/Slide | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🟡 | corpus charts-* | 🟡 | thiếu loại khác |
| Biểu đồ c: nâng cao (bubble, radar, stock, surface, 3D…) | all | 🟡 | ⬜ | ⬜ | ✅ | ⬜ | ⬜ | — | — | ⬜ | giữ nguyên XML |
| Biểu đồ cx: (waterfall, treemap…) | all | 🟡 | ⬜ | ⬜ | ✅ | ⬜ | ⬜ | — | — | ⬜ | giữ nguyên |
| Shape preset/custom geometry | all | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 187 preset, snapshots | ✅ | |
| Edit points / merge shapes | all | — | — | ⬜ | — | ⬜ | ⬜ | — | — | ⬜ | |
| Hiệu ứng: shadow, glow | all | ✅ | ✅ | 🟡 | ✅ | ✅ | ✅ | — | snapshots | 🟡 | PDF blur dạng ảnh mặt nạ |
| Hiệu ứng: reflection, soft edge | all | ✅ | ⬜ | ⬜ | ✅ | ⬜ | ⬜ | — | — | ⬜ | |
| 3D (scene3d/sp3d) | all | ✅ | 🟡 | ⬜ | ✅ | 🟡 | 🟡 | — | corpus slide-shapes-3d | 🟡 | xấp xỉ 2D, chưa WebGL |
| WordArt / text warp | all | ✅ | ⬜ | ⬜ | ✅ | ⬜ | ⬜ | — | — | ⬜ | |
| Ảnh: crop, xoay, alpha | all | ✅ | ✅ | 🟡 | ✅ | ✅ | ✅ | ✅ | corpus images-* | 🟡 | thiếu chỉnh màu, xoá nền |
| Ink | all | 🟡 | ⬜ | ⬜ | ✅ | ⬜ | ⬜ | — | — | ⬜ | giữ nguyên |
| Mô hình 3D | all | 🟡 | ⬜ | ⬜ | ✅ | ⬜ | ⬜ | — | — | ⬜ | giữ nguyên |
| Cộng tác thời gian thực | all | — | ✅ | ✅ | — | ✅ | ✅ | ✅ | collab tests + E2E | ✅ | |
| Comment luồng | all | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | tests | 🟡 | thiếu @mention, giao việc |
| Lịch sử phiên bản | all | — | ✅ | ✅ | — | ✅ | ✅ | — | server tests | 🟡 | thiếu so sánh |
| Mật khẩu / bảo vệ | all | ✅ | — | ✅ | ✅ | ✅ | ✅ | — | interop | 🟡 | thiếu đánh dấu bản cuối, nhãn độ mật |
| Ký số | all | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | — | bo-sign | 🟡 | thiếu dòng chữ ký trong văn bản |
| Kiểm tra tài liệu / tiếp cận | all | — | ⬜ | ⬜ | — | ⬜ | ⬜ | — | — | ⬜ | |
| VBA | all | ✅ | — | — | ✅ | — | — | — | test | — (chủ ý) | giữ nguyên, cảnh báo, không chạy |
| BlueScript / recorder | all | — | — | ⬜ | — | ⬜ | ⬜ | — | — | ⬜ | |
| Add-in | all | — | — | ⬜ | — | ⬜ | ⬜ | — | — | ⬜ | |
| Dịch / đọc chính tả / đọc to | all | — | — | ⬜ | — | ⬜ | ⬜ | — | — | ⬜ | |
| Định dạng mẫu/macro (.dotx…) | all | 🟡 | 🟡 | ⬜ | 🟡 | 🟡 | 🟡 | — | — | 🟡 | mở được như tài liệu thường |
| PDF/A, PDF có thẻ | all | — | — | ⬜ | — | ⬜ | ⬜ | — | — | ⬜ | xuất PDF thường ✅ |
| In nâng cao | all | — | 🟡 | — | — | 🟡 | 🟡 | — | E2E print | 🟡 | |

## BlueDoc

| Tính năng | Đọc | Hiển thị | Tạo/sửa | Round-trip | Web | Desktop | Cộng tác | Test | Trạng thái | Ghi chú |
|---|---|---|---|---|---|---|---|---|---|---|
| Định dạng ký tự/đoạn cơ bản | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | corpus text-*, para-* | ✅ | |
| Thuộc tính ký tự nâng cao (emboss, ruby, ligature…) | ✅ | ⬜ | ⬜ | ✅ | ⬜ | ⬜ | — | — | ⬜ | |
| Style, danh sách đa cấp | ✅ | ✅ | 🟡 | ✅ | ✅ | ✅ | ✅ | corpus styles-*, lists-* | 🟡 | thiếu định nghĩa danh sách mới |
| Section, cột, header/footer | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | corpus sections-* | ✅ | |
| Drop cap, đánh số dòng, gạch nối, chữ dọc | ✅ | ⬜ | ⬜ | ✅ | ⬜ | ⬜ | — | — | ⬜ | |
| Viền trang, nền, watermark | ✅ | ⬜ | ⬜ | ✅ | ⬜ | ⬜ | — | — | ⬜ | |
| Bảng (gộp, lồng, lặp tiêu đề) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | corpus tables-* | 🟡 | thiếu bảng nổi, công thức bảng, sort |
| Ảnh & wrap | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | corpus images-* | ✅ | |
| Footnote/endnote, TOC, caption, cross-ref | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | tests bo-edit | ✅ | |
| Chỉ mục, TOA, trích dẫn | ✅ | 🟡 | ⬜ | ✅ | ⬜ | ⬜ | — | — | ⬜ | kết quả cache |
| Field đầy đủ | ✅ | 🟡 | 🟡 | ✅ | 🟡 | 🟡 | 🟡 | tests fields | 🟡 | một phần danh sách |
| Content controls | 🟡 | 🟡 | ⬜ | ✅ | ⬜ | ⬜ | — | — | ⬜ | giữ nguyên |
| Track changes | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | tests + E2E | 🟡 | thiếu move, chế độ hiển thị, khoá |
| So sánh / kết hợp | — | — | ⬜ | — | ⬜ | ⬜ | — | — | ⬜ | |
| Mail merge | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | — | tests | 🟡 | thiếu SQL, email, nhãn |
| Chế độ xem (Read, Outline, Draft…) | — | 🟡 | — | — | 🟡 | 🟡 | — | E2E | 🟡 | chỉ Print Layout |
| Mẫu hành chính NĐ 30 | — | ✅ | ✅ | ✅ | ✅ | ✅ | — | E2E | 🟡 | 1 mẫu; thiếu thư viện + kiểm tra thể thức |
| Phương trình | ✅ | 🟡 | ⬜ | ✅ | 🟡 | 🟡 | ✅ | corpus math-* | 🟡 | xem CORE |

## BlueSheet

| Tính năng | Đọc | Hiển thị | Tạo/sửa | Round-trip | Web | Desktop | Cộng tác | Test | Trạng thái | Ghi chú |
|---|---|---|---|---|---|---|---|---|---|---|
| Hàm (515) | ✅ | ✅ | 🟡 | ✅ | ✅ | ✅ | ✅ | functions.md | 🟡 | 275/515 |
| Mảng động, LET, LAMBDA | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | tests | ✅ | |
| @, trim ref, liên kết ngoài | 🟡 | ⬜ | ⬜ | ✅ | ⬜ | ⬜ | — | — | ⬜ | |
| Định dạng số | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | tests | 🟡 | thiếu [DBNum], lịch âm |
| CF / DV | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | corpus cf-*, dv-* | 🟡 | thiếu một số icon set/data bar âm |
| Sort / filter | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | E2E | 🟡 | thiếu theo màu, advanced filter |
| Pivot | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🟡 | tests + E2E | 🟡 | thiếu drill-down |
| PivotChart | ✅ | ✅ | 🟡 | ✅ | ✅ | ✅ | — | tests | 🟡 | |
| Slicer / Timeline | 🟡 | ⬜ | ⬜ | ✅ | ⬜ | ⬜ | — | — | ⬜ | giữ nguyên |
| Goal Seek / Data Table / Solver | — | — | ⬜ | — | ⬜ | ⬜ | — | — | ⬜ | |
| Power Query / data model | 🟡 | — | ⬜ | ✅ | ⬜ | ⬜ | — | — | ⬜ | giữ nguyên |
| Flash Fill, Text to Columns | — | — | ⬜ | — | ⬜ | ⬜ | — | — | ⬜ | |
| Analysis ToolPak | — | — | ⬜ | — | ⬜ | ⬜ | — | — | ⬜ | |
| Sparklines | 🟡 | ⬜ | ⬜ | ✅ | ⬜ | ⬜ | — | — | ⬜ | giữ nguyên |
| Bảng (ListObject) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | tests | 🟡 | thiếu total row tự động |
| Comment luồng / notes | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | tests | ✅ | |
| Bảo vệ sheet/workbook | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | — | tests | ✅ | |
| Form controls | 🟡 | ⬜ | ⬜ | ✅ | ⬜ | ⬜ | — | — | ⬜ | giữ nguyên |
| In bảng tính | ✅ | ✅ | 🟡 | ✅ | ✅ | ✅ | — | tests | 🟡 | |
| Hiệu năng 1 triệu ô | — | ✅ | ✅ | — | ✅ | ✅ | — | bench | 🟡 | chưa đo 10 triệu ô, pivot 1 triệu dòng |

## BlueSlide

| Tính năng | Đọc | Hiển thị | Tạo/sửa | Round-trip | Web | Desktop | Cộng tác | Test | Trạng thái | Ghi chú |
|---|---|---|---|---|---|---|---|---|---|---|
| Shape, text, bảng, ảnh | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | corpus + E2E | ✅ | |
| Master/layout/theme | ✅ | ✅ | 🟡 | ✅ | ✅ | ✅ | ✅ | tests | 🟡 | chưa sửa master trong UI |
| Chế độ xem | — | 🟡 | — | — | ✅ | ✅ | — | E2E | 🟡 | Normal, Sorter, Reading, Presenter |
| Transition | ✅ | 🟡 | 🟡 | ✅ | ✅ | ✅ | — | corpus anim-* | 🟡 | vài họ cơ bản |
| Animation | ✅ | 🟡 | 🟡 | ✅ | ✅ | ✅ | — | corpus | 🟡 | entrance cơ bản |
| Morph | ✅ | ⬜ | ⬜ | ✅ | ⬜ | ⬜ | — | — | ⬜ | |
| Audio/video | ✅ | 🟡 | 🟡 | ✅ | ✅ | ✅ | — | corpus media-* | 🟡 | phát cơ bản |
| Trình chiếu + Presenter | — | ✅ | — | — | ✅ | ✅ | — | E2E | 🟡 | thiếu bút/laser, custom show |
| Ghi thuyết trình / xuất video | — | — | ⬜ | — | ⬜ | ⬜ | — | — | ⬜ | |
| Xuất PDF/PNG | — | ✅ | — | — | ✅ | ✅ | — | tests | 🟡 | thiếu handout, SVG |
| SmartArt | 🟡 | 🟡 | ⬜ | ✅ | 🟡 | 🟡 | — | corpus | 🟡 | fallback |
| Comment hiện đại | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | tests | ✅ | |
