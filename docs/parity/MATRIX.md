# Ma trận tương đương tính năng (Parity MATRIX)

Mức: ✅ đầy đủ · 🟡 một phần · ⬜ chưa có · — không áp dụng. Nguồn đối chiếu: corpus tự sinh + OpenXmlValidator + ảnh
mốc tự sinh (`docs/fidelity-visual.md`); **chưa có ảnh mốc từ phần mềm tham chiếu** (xem `REFERENCE_GUIDE.md`).
Cập nhật sau mỗi hạng mục BACKLOG.

## Tổng hợp (theo hàng tính năng bên dưới; ✅ = 1, 🟡 = 0,5)

Tính lại khi rà soát định kỳ 2026-11-09 (lần 3, sau 34 mục) từ cột Trạng thái. Fidelity (`pnpm fidelity`, 2026-11-09): .docx 74 file,
98,6 % hợp lệ sau khi lưu (1 lỗi đã biết); .xlsx 135 file, 97,8 % (3 đã biết); .pptx 64 file, 100 %; ảnh mốc tự sinh 273 trang, 0 vượt
ngưỡng sau khi làm mới 9 trang thay đổi có chủ đích (mục lục / content control khối nay hiển thị, bộ vẽ biểu đồ chung, lưới 3D).

| App / nhóm | Hàng | Điểm | % |
|---|---|---|---|
| Chung (CORE) | 32 | 11 | 34 % |
| BlueDoc (DOC) | 18 | 8,5 | 47 % |
| BlueSheet (SHEET) | 20 | 7,5 | 38 % |
| BlueSlide (SLIDE) | 12 | 7 | 58 % |
| Hàm bảng tính | 515 hàm | 508 | 98,6 % (`functions.md`) |

## Chung

| Tính năng | App | Đọc file | Hiển thị | Tạo/sửa | Round-trip | Web | Desktop | Cộng tác | Test | Trạng thái | Ghi chú |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Phương trình OMML | Doc | ✅ | ✅ | ✅ | ✅ | ✅ | 🟡 | ✅ | corpus math-0*, math::tests, equation.spec | 🟡 | bảng MATH (STIX Two Math), oMathPara theo jc, nhập UnicodeMath/LaTeX, sửa, tuyến tính ↔ chuyên nghiệp; chưa soạn trực tiếp trên công thức, thư viện mẫu |
| Phương trình trong Slide/Sheet | Slide/Sheet | ✅ | ⬜ | ⬜ | ✅ | ⬜ | ⬜ | — | — | ⬜ | giữ nguyên |
| Soát chính tả Việt/Anh | all | — | 🟡 | ✅ | — | ✅ | 🟡 | — | bo-spell, spelling.spec | 🟡 | BlueDoc gạch đỏ + gợi ý + bỏ qua/thêm từ; chưa w:lang, hộp thoại toàn tài liệu, Sheet/Slide |
| Ngữ pháp/văn phong | all | — | 🟡 | ✅ | — | ✅ | 🟡 | — | bo-spell grammar, spelling.spec | 🟡 | BlueDoc gạch xanh: dấu cách, dấu câu, viết hoa đầu câu, lặp từ, ngoặc/nháy, kiểu dấu thanh; chưa phân tích cú pháp |
| AutoCorrect | all | — | — | ✅ | — | ✅ | ✅ | — | — | 🟡 | khi gõ trong thân văn bản BlueDoc (CORE-AUTOCORR-001); thiếu header/footer/text box |
| Tìm/thay (regex, định dạng) | all | — | ✅ | ✅ | — | ✅ | ✅ | — | E2E | ✅ | ký tự đặc biệt ^p ^t…, wildcard, tô sáng tất cả (CORE-FIND-001) |
| SmartArt | all | ✅ | ✅ | 🟡 | ✅ | ✅ | ✅ | — | corpus smartart-*, diagram_layout, smartart.spec | 🟡 | hiển thị: fallback hoặc tự dựng (7 nhóm); tạo/sửa/đổi màu/chuyển hình: BlueSlide; Doc/Sheet chưa tạo |
| Biểu đồ c: cơ bản (9 loại) | all | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🟡 | corpus charts-* | ✅ | một bộ vẽ chung cho Sheet/Slide/Doc; kiểu/màu/bố cục nhanh/mẫu biểu đồ (CORE-CHART-007, Sheet + Slide) |
| Biểu đồ c: nâng cao (bubble, radar, stock, surface, 3D…) | all | ✅ | 🟡 | ✅ | ✅ | ✅ | ✅ | — | corpus writer-charts-*, slide_show | 🟡 | 100 %, bubble, radar, stock HLC/OHLC, of-pie, tách miếng, surface (nhìn trên), 3D (cột/thanh/trụ/nón/kim tự tháp, đường, vùng, tròn, bề mặt; khối thật); thiếu VHLC/VOHLC |
| Biểu đồ cx: (waterfall, treemap…) | all | ✅ | ✅ | 🟡 | ✅ | ✅ | ✅ | — | corpus writer-chartex-*, slide-chartex-* | 🟡 | 7 loại đọc/vẽ ở mọi app; tạo trong BlueSheet; chưa regionMap |
| Shape preset/custom geometry | all | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 187 preset, snapshots | ✅ | |
| Edit points / merge shapes | all | — | — | ⬜ | — | ⬜ | ⬜ | — | — | ⬜ | |
| Hiệu ứng: shadow, glow | all | ✅ | ✅ | 🟡 | ✅ | ✅ | ✅ | — | snapshots | 🟡 | PDF blur dạng ảnh mặt nạ |
| Hiệu ứng: reflection, soft edge | all | ✅ | ⬜ | ⬜ | ✅ | ⬜ | ⬜ | — | — | ⬜ | |
| 3D (scene3d/sp3d) | all | ✅ | ✅ | ⬜ | ✅ | ✅ | ✅ | — | corpus slide-shapes-3d, mesh3d::tests | 🟡 | khối thật: đùn, bevel trên/dưới, camera/phối cảnh, ánh sáng + material (phần mềm, ADR-0010 §5); chưa có UI chỉnh 3D |
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
| Style, danh sách đa cấp | ✅ | ✅ | 🟡 | ✅ | ✅ | ✅ | ✅ | corpus styles-*, lists-* | 🟡 | định nghĩa danh sách mới, liên kết heading, đặt giá trị (DOC-LIST-001); thiếu restart cấp con, legal-style |
| Section, cột, header/footer | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | corpus sections-* | ✅ | |
| Drop cap, đánh số dòng, gạch nối, chữ dọc | ✅ | ⬜ | ⬜ | ✅ | ⬜ | ⬜ | — | — | ⬜ | |
| Viền trang, nền, watermark | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | — | corpus page-* | 🟡 | viền thường + nghệ thuật tự vẽ, màu trang, hình mờ chữ/ảnh, chủ đề (DOC-PAGE-001); thiếu in không màu trang, w:frame |
| Bảng (gộp, lồng, lặp tiêu đề) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | corpus tables-* | 🟡 | + sắp xếp, công thức, AutoFit, văn bản↔bảng, chữ dọc, bảng nổi (DOC-TABLE-001); thiếu vẽ/tẩy bảng |
| Ảnh & wrap | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | corpus images-* | ✅ | |
| Footnote/endnote, TOC, caption, cross-ref | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | tests bo-edit | ✅ | |
| Chỉ mục, TOA, trích dẫn | ✅ | 🟡 | ⬜ | ✅ | ⬜ | ⬜ | — | — | ⬜ | kết quả cache |
| Field đầy đủ | ✅ | 🟡 | 🟡 | ✅ | 🟡 | 🟡 | 🟡 | tests fields | 🟡 | một phần danh sách |
| Content controls | ✅ | 🟡 | 🟡 | ✅ | ✅ | ✅ | — | corpus controls-01, tests bo-docx/bo-edit/bo-engine, content-controls.spec | 🟡 | chèn + điền text/rich/hộp kiểm/danh sách/kết hợp/ngày; thiếu tạo ảnh/phần lặp, hộp thoại thuộc tính, khung viền |
| Track changes | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | tests + E2E | 🟡 | chế độ hiển thị + bong bóng + ngăn danh sách (DOC-REVIEW-001); thiếu move, khoá |
| So sánh / kết hợp | — | — | ✅ | — | ✅ | ✅ | — | compare::tests, compare_with_the_original_document, combine_two_reviewers_copies, review.spec | ✅ | so sánh với bản gốc → track changes (đoạn, từ, định dạng, bảng); kết hợp bản sửa của hai người (trộn 3 chiều theo đoạn/từ, theo tác giả) |
| Mail merge | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | — | tests | 🟡 | thiếu SQL, email, nhãn |
| Chế độ xem (Read, Outline, Draft…) | — | 🟡 | — | — | 🟡 | 🟡 | — | E2E | 🟡 | chỉ Print Layout |
| Mẫu hành chính NĐ 30 | — | ✅ | ✅ | ✅ | ✅ | ✅ | — | E2E | 🟡 | 1 mẫu; thiếu thư viện + kiểm tra thể thức |
| Phương trình | ✅ | 🟡 | ⬜ | ✅ | 🟡 | 🟡 | ✅ | corpus math-* | 🟡 | xem CORE |

## BlueSheet

| Tính năng | Đọc | Hiển thị | Tạo/sửa | Round-trip | Web | Desktop | Cộng tác | Test | Trạng thái | Ghi chú |
|---|---|---|---|---|---|---|---|---|---|---|
| Hàm (515) | ✅ | ✅ | 🟡 | ✅ | ✅ | ✅ | ✅ | functions.md | 🟡 | 508/515 (thiếu Cube 7; WEBSERVICE cần provider) |
| Mảng động, LET, LAMBDA | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | tests | ✅ | |
| @, trim ref, liên kết ngoài | 🟡 | ⬜ | ⬜ | ✅ | ⬜ | ⬜ | — | — | ⬜ | |
| Định dạng số | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | tests | 🟡 | thiếu [DBNum], lịch âm |
| CF / DV | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | corpus cf-*, dv-* | 🟡 | thiếu một số icon set/data bar âm |
| Sort / filter | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | E2E | 🟡 | thiếu theo màu, advanced filter |
| Pivot | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🟡 | tests + E2E | 🟡 | thiếu drill-down |
| PivotChart | ✅ | ✅ | 🟡 | ✅ | ✅ | ✅ | — | tests | 🟡 | |
| Slicer / Timeline | ✅ | 🟡 | 🟡 | ✅ | ✅ | ✅ | — | slicers.rs, timelines.rs, writer-slicers-*, timeline-1, sheet-slicer/timeline.spec | 🟡 | slicer bảng tạo/lọc/lưu; slicer pivot và timeline đọc/lọc/lưu lựa chọn (chưa tạo mới) |
| Goal Seek / Data Table / Solver | — | — | ✅ | — | ✅ | ✅ | — | — | 🟡 | + Scenario Manager & báo cáo (SHEET-WHATIF-003); Solver phi tuyến còn thiếu |
| Power Query / data model | 🟡 | — | ⬜ | ✅ | ⬜ | ⬜ | — | — | ⬜ | giữ nguyên |
| Flash Fill, Text to Columns | — | — | ✅ | — | ✅ | ✅ | — | — | 🟡 | Flash Fill học từ ví dụ (SHEET-FLASH-001); thiếu gợi ý khi gõ |
| Analysis ToolPak | — | — | ✅ | — | ✅ | ✅ | — | — | 🟡 | 19 công cụ phân tích thống kê (SHEET-ATP-001); thiếu biểu đồ đầu ra |
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
| Transition | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | — | corpus anim-*, slide-edit-transitions-all (Choice + Fallback hợp lệ), transitionFx test, slide-show-media.spec | ✅ | 55 hiệu ứng (§19.3.1 + [MS-PPTX] p14/p15 prstTrans), biến thể hướng, thời lượng, âm thanh WAV/dừng, áp dụng tất cả; Morph theo đối tượng (vị trí/cỡ/xoay/màu; chưa theo từ/ký tự) |
| Animation | ✅ | ✅ | 🟡 | ✅ | ✅ | ✅ | — | corpus slide-edit-animations-all, anim_play/anim_catalog tests, effectFx test, slide-show-media.spec | 🟡 | phát mọi hiệu ứng trong file (bộ thông dịch §19.5); tạo 25 xuất hiện/biến mất, 7 nhấn mạnh, 13 đường chuyển động; chưa vẽ đường tuỳ ý, chưa hiện đổi màu, chưa theo từ/chữ |
| Morph | ✅ | ⬜ | ⬜ | ✅ | ⬜ | ⬜ | — | — | ⬜ | |
| Audio/video | ✅ | 🟡 | 🟡 | ✅ | ✅ | ✅ | — | corpus media-* | 🟡 | phát cơ bản |
| Trình chiếu + Presenter | — | ✅ | — | — | ✅ | ✅ | — | E2E | 🟡 | thiếu bút/laser, custom show |
| Ghi thuyết trình / xuất video | — | — | ⬜ | — | ⬜ | ⬜ | — | — | ⬜ | |
| Xuất PDF/PNG | — | ✅ | — | — | ✅ | ✅ | — | tests | 🟡 | thiếu handout, SVG |
| SmartArt | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | — | corpus, smartart.spec | ✅ | chèn từ thư viện bố cục, ngăn văn bản, bố cục/màu/kiểu, chuyển thành hình |
| Comment hiện đại | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | tests | ✅ | |
