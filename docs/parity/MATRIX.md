# Ma trận tương đương tính năng (Parity MATRIX)

Mức: ✅ đầy đủ · 🟡 một phần · ⬜ chưa có · — không áp dụng. Nguồn đối chiếu: corpus tự sinh + OpenXmlValidator + ảnh
mốc tự sinh (`docs/fidelity-visual.md`); **chưa có ảnh mốc từ phần mềm tham chiếu** (xem `REFERENCE_GUIDE.md`).
Cập nhật sau mỗi hạng mục BACKLOG.

## Tổng hợp (theo hàng tính năng bên dưới; ✅ = 1, 🟡 = 0,5)

Tính lại khi rà soát định kỳ 2026-11-10 (lần 5, sau 63 mục `[x]`) từ cột Trạng thái (hàng "— (chủ ý)" không tính). Fidelity (`pnpm fidelity`,
2026-11-10): .docx 86 file, 98,8 % hợp lệ sau khi lưu (1 lỗi đã biết); .xlsx 161 file, 98,1 % (3 đã biết); .pptx 64 file, 100 %; ảnh mốc tự
sinh 311 trang — 15 trang vượt ngưỡng đều do thay đổi có chủ đích (data bar gradient 2006, kiểu bảng, slicer, in tiêu đề hàng/cột + header/footer,
tô màu đoạn di chuyển) → làm mới baseline, 0 vượt ngưỡng. Chưa có ảnh mốc từ phần mềm tham chiếu.

| App / nhóm | Hàng | Điểm | % |
|---|---|---|---|
| Chung (CORE) | 34 | 18,5 | 54 % |
| BlueDoc (DOC) | 18 | 12,5 | 69 % |
| BlueSheet (SHEET) | 21 | 12 | 57 % |
| BlueSlide (SLIDE) | 12 | 8 | 67 % |
| Hàm bảng tính | 515 hàm | 508 | 98,6 % (`functions.md`) |

## Chung

| Tính năng | App | Đọc file | Hiển thị | Tạo/sửa | Round-trip | Web | Desktop | Cộng tác | Test | Trạng thái | Ghi chú |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Phương trình OMML | Doc | ✅ | ✅ | ✅ | ✅ | ✅ | 🟡 | ✅ | corpus math-0*, math::tests, equation.spec | 🟡 | bảng MATH (STIX Two Math), oMathPara theo jc, nhập UnicodeMath/LaTeX, sửa, tuyến tính ↔ chuyên nghiệp; chưa soạn trực tiếp trên công thức, thư viện mẫu |
| Phương trình trong Slide/Sheet | Slide/Sheet | ✅ | ⬜ | ⬜ | ✅ | ⬜ | ⬜ | — | — | ⬜ | giữ nguyên |
| Soát chính tả Việt/Anh | all | — | ✅ | ✅ | — | ✅ | 🟡 | — | bo-spell, spelling.spec, spelling-sheet-slide.spec, proof.rs | ✅ | BlueDoc theo w:lang (CORE-SPELL-005); ô BlueSheet + text BlueSlide, F7 toàn sổ/bài (CORE-SPELL-006) |
| Ngữ pháp/văn phong | all | — | 🟡 | ✅ | — | ✅ | 🟡 | — | bo-spell grammar, spelling.spec | 🟡 | BlueDoc gạch xanh: dấu cách, dấu câu, viết hoa đầu câu, lặp từ, ngoặc/nháy, kiểu dấu thanh; chưa phân tích cú pháp |
| AutoCorrect | all | — | — | ✅ | — | ✅ | ✅ | — | — | 🟡 | khi gõ trong thân văn bản BlueDoc (CORE-AUTOCORR-001); thiếu header/footer/text box |
| Tìm/thay (regex, định dạng) | all | — | ✅ | ✅ | — | ✅ | ✅ | — | E2E | ✅ | ký tự đặc biệt ^p ^t…, wildcard, tô sáng tất cả (CORE-FIND-001) |
| SmartArt | all | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | — | corpus smartart-*, diagram_layout, smartart.spec, smartart-doc-sheet.spec | ✅ | hiển thị: fallback hoặc tự dựng (7 nhóm); tạo/sửa qua ngăn văn bản, bố cục/màu/kiểu ở cả 3 app (CORE-SMART-005); chuyển thành hình chỉ BlueSlide |
| Biểu đồ c: cơ bản (9 loại) | all | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🟡 | corpus charts-* | ✅ | một bộ vẽ chung cho Sheet/Slide/Doc; kiểu/màu/bố cục nhanh/mẫu biểu đồ (CORE-CHART-007, Sheet + Slide) |
| Biểu đồ c: nâng cao (bubble, radar, stock, surface, 3D…) | all | ✅ | 🟡 | ✅ | ✅ | ✅ | ✅ | — | corpus writer-charts-*, slide_show | 🟡 | 100 %, bubble, radar, stock HLC/OHLC, of-pie, tách miếng, surface (nhìn trên), 3D (cột/thanh/trụ/nón/kim tự tháp, đường, vùng, tròn, bề mặt; khối thật); thiếu VHLC/VOHLC |
| Biểu đồ cx: (waterfall, treemap…) | all | ✅ | ✅ | 🟡 | ✅ | ✅ | ✅ | — | corpus writer-chartex-*, slide-chartex-* | 🟡 | 7 loại đọc/vẽ ở mọi app; tạo trong BlueSheet; chưa regionMap |
| Nhúng/liên kết workbook, OLE | Doc/Slide | ✅ | 🟡 | 🟡 | ✅ | 🟡 | ✅ | — | bo-docx ole, bo-engine ole | 🟡 | CORE-EMBED-001: ảnh preview, giữ nguyên byte, chèn/sửa lưới, cập nhật liên kết (desktop); chưa EMF/WMF, chưa mở bằng BlueSheet |
| Shape preset/custom geometry | all | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 187 preset, snapshots | ✅ | |
| Edit points / merge shapes | all | — | — | ⬜ | — | ⬜ | ⬜ | — | — | ⬜ | |
| Hiệu ứng: shadow, glow | all | ✅ | ✅ | 🟡 | ✅ | ✅ | ✅ | — | snapshots | 🟡 | PDF blur dạng ảnh mặt nạ |
| Hiệu ứng: reflection, soft edge | all | ✅ | 🟡 | 🟡 | ✅ | 🟡 | 🟡 | — | shape_effects.rs | 🟡 | BlueSlide vẽ + sửa + xoay 3D (CORE-SHAPE-002); Doc/Sheet chưa |
| 3D (scene3d/sp3d) | all | ✅ | ✅ | ⬜ | ✅ | ✅ | ✅ | — | corpus slide-shapes-3d, mesh3d::tests | 🟡 | khối thật: đùn, bevel trên/dưới, camera/phối cảnh, ánh sáng + material (phần mềm, ADR-0010 §5); chưa có UI chỉnh 3D |
| WordArt / text warp | all | ✅ | 🟡 | 🟡 | ✅ | 🟡 | 🟡 | — | shape_effects.rs | 🟡 | 40 preset prstTxWarp, viền/tô chữ ở BlueSlide (CORE-SHAPE-002); Doc/Sheet chưa |
| Ảnh: crop, xoay, alpha | all | ✅ | ✅ | 🟡 | ✅ | ✅ | ✅ | ✅ | corpus images-* | 🟡 | thiếu chỉnh màu, xoá nền |
| Ink | all | 🟡 | ⬜ | ⬜ | ✅ | ⬜ | ⬜ | — | — | ⬜ | giữ nguyên |
| Mô hình 3D | all | ✅ | 🟡 | 🟡 | ✅ | ✅ | ✅ | — | bo-model3d, slide_model3d.rs, slide-model3d.spec | 🟡 | glTF/GLB/OBJ/STL, am3d đọc/ghi, xoay/zoom + trình chiếu ở BlueSlide (CORE-3DMODEL-001); Doc chỉ hiện/giữ, Sheet chỉ giữ |
| Cộng tác thời gian thực | all | — | ✅ | ✅ | — | ✅ | ✅ | ✅ | collab tests + E2E, collab_objects.rs | ✅ | đối tượng mới + hiện ai đang sửa đoạn/ô/slide (CORE-COLLAB-001, `docs/collab-coverage.md`); cùng một part/thiết lập: người ghi sau thắng |
| Comment luồng | all | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | tests, mentions.rs, mentions.test.tsx, server tests | ✅ | @mention (people part docx, mentions xlsx, ext pptx), thông báo host/e-mail, giao việc (CORE-COMMENT-001) |
| Lịch sử phiên bản | all | — | ✅ | ✅ | — | ✅ | ✅ | — | server tests, bo-sheet diff, slide_compare, showChanges/compareVersions tests | 🟡 | so sánh Doc/Sheet/Slide + Hiện thay đổi (CORE-VERSION-001); chưa tách theo từng phiên bản trung gian |
| Mật khẩu / bảo vệ | all | ✅ | — | ✅ | ✅ | ✅ | ✅ | — | interop, protection.rs | 🟡 | + đánh dấu bản cuối, khuyến nghị chỉ đọc, vùng được sửa, nhãn độ mật (CORE-SEC-001); thiếu mật khẩu kiểu cũ w:hash |
| Ký số | all | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | — | bo-sign, sigline.rs | 🟡 | + dòng chữ ký BlueDoc, nhiều chữ ký (CORE-SIGLINE-001); thiếu Sheet/Slide |
| Kiểm tra tài liệu / tiếp cận | all | — | ✅ | ✅ | — | ✅ | ✅ | — | inspect.rs | 🟡 | rà soát nội dung, trợ năng, tương thích (CORE-INSPECT-001); thiếu ngăn bên, sửa nhanh Sheet |
| VBA | all | ✅ | — | — | ✅ | — | — | — | test | — (chủ ý) | giữ nguyên, cảnh báo, không chạy |
| BlueScript / recorder | all | — | — | ✅ | — | ✅ | ✅ | ✅ | packages/script tests, bluescript.spec | 🟡 | sandbox QuickJS trong Worker, API Doc/Sheet/Slide, ngăn soạn mã, một lần chạy = một bước hoàn tác (CORE-SCRIPT-001); chưa recorder, chưa lưu trong tệp |
| Add-in | all | — | — | ⬜ | — | ⬜ | ⬜ | — | — | ⬜ | |
| Dịch / đọc chính tả / đọc to | all | — | 🟡 | ✅ | — | ✅ | ✅ | — | read-aloud tests, translate tests + translate.spec, TranslationTests (.NET) | 🟡 | đọc to + tô từ (CORE-LANG-003); dịch qua dịch vụ cấu hình được ở 3 app giữ định dạng (CORE-LANG-001); đọc chính tả qua bộ nhận dạng trình duyệt/dịch vụ cấu hình + lệnh dấu câu (CORE-LANG-002); chưa model cục bộ |
| Định dạng mẫu/macro (.dotx…) | all | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | — | variants.rs, bo-cli tests | 🟡 | 14 biến thể theo content type, lưu thành giữ/bỏ VBA, mở mẫu thành tài liệu mới, .thmx (CORE-FMT-001); thiếu bỏ XLM macro |
| PDF/A, PDF có thẻ | all | — | — | ✅ | — | ✅ | ✅ | — | bo-pdf/tests/pdfa.rs, bo-engine/tests/pdf_export.rs | 🟡 | PDF/A-1b/2b/3b, cây cấu trúc, dấu trang từ đề mục/slide/sheet, hộp thoại Xuất PDF (CORE-FMT-002); chưa kiểm bằng veraPDF |
| In nâng cao | all | — | 🟡 | — | — | 🟡 | 🟡 | — | E2E print | 🟡 | |
| Ribbon: tab ngữ cảnh, ribbon đơn giản, thu gọn | all | — | ✅ | — | — | ✅ | ✅ | — | ribbonContext.test.tsx, ribbon-context.spec | ✅ | CORE-UI-001; ngữ cảnh phương trình/ink/đối tượng chưa phát hiện từ vùng chọn |
| Ngăn tác vụ dock/float | all | — | ✅ | — | — | ✅ | ✅ | — | taskPanes.test.tsx, ui-task-panes.spec | 🟡 | CORE-UI-006: dock trái/phải, nổi, tab, đổi cỡ, nhớ bố cục, F6; ngăn Styles (Doc), Selection (Slide); ngăn sửa đổi chưa chuyển, chưa có Format Shape |

## BlueDoc

| Tính năng | Đọc | Hiển thị | Tạo/sửa | Round-trip | Web | Desktop | Cộng tác | Test | Trạng thái | Ghi chú |
|---|---|---|---|---|---|---|---|---|---|---|
| Định dạng ký tự/đoạn cơ bản | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | corpus text-*, para-* | ✅ | |
| Thuộc tính ký tự nâng cao (emboss, ruby, ligature…) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | — | char_props.rs, corpus text-04..07 | 🟡 | w14 effects, OpenType, ruby, combine (DOC-FMT-001); combine chỉ 1 run |
| Style, danh sách đa cấp | ✅ | ✅ | 🟡 | ✅ | ✅ | ✅ | ✅ | corpus styles-*, lists-* | 🟡 | định nghĩa danh sách mới, liên kết heading, đặt giá trị (DOC-LIST-001); thiếu restart cấp con, legal-style |
| Section, cột, header/footer | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | corpus sections-* | ✅ | |
| Drop cap, đánh số dòng, gạch nối, chữ dọc | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | — | corpus layout-0* | 🟡 | DOC-LAYOUT-001; gạch nối tiếng Anh gần đúng, chưa cân cột |
| Viền trang, nền, watermark | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | — | corpus page-* | 🟡 | viền thường + nghệ thuật tự vẽ, màu trang, hình mờ chữ/ảnh, chủ đề (DOC-PAGE-001); thiếu in không màu trang, w:frame |
| Bảng (gộp, lồng, lặp tiêu đề) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | corpus tables-* | 🟡 | + sắp xếp, công thức, AutoFit, văn bản↔bảng, chữ dọc, bảng nổi (DOC-TABLE-001); thiếu vẽ/tẩy bảng |
| Ảnh & wrap | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | corpus images-* | ✅ | |
| Footnote/endnote, TOC, caption, cross-ref | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | tests bo-edit | ✅ | |
| Chỉ mục, TOA, trích dẫn | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | — | refs_tests, references.rs | ✅ | mục lục hình, INDEX/XE, TOA/TA, chuyển chú thích (DOC-REF-001) |
| Field đầy đủ | ✅ | 🟡 | 🟡 | ✅ | 🟡 | 🟡 | 🟡 | tests fields | 🟡 | một phần danh sách |
| Content controls | ✅ | 🟡 | 🟡 | ✅ | ✅ | ✅ | — | corpus controls-01/03, tests bo-docx/bo-edit/bo-engine, content-controls.spec | 🟡 | Custom XML + data binding hai chiều (DOC-CC-002); chèn + điền text/rich/hộp kiểm/danh sách/kết hợp/ngày; thiếu tạo ảnh/phần lặp, hộp thoại thuộc tính, khung viền; + trường biểu mẫu cũ & chế độ chỉ điền (DOC-FORM-001) |
| Track changes | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | tests + E2E, track_moves.rs | 🟡 | chế độ hiển thị + bong bóng + ngăn danh sách (DOC-REVIEW-001); cặp di chuyển, chi tiết định dạng, khoá mật khẩu (DOC-REVIEW-002); cắt-dán chưa thành di chuyển |
| So sánh / kết hợp | — | — | ✅ | — | ✅ | ✅ | — | compare::tests, compare_with_the_original_document, combine_two_reviewers_copies, review.spec | ✅ | so sánh với bản gốc → track changes (đoạn, từ, định dạng, bảng); kết hợp bản sửa của hai người (trộn 3 chiều theo đoạn/từ, theo tác giả) |
| Mail merge | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | — | tests | 🟡 | thiếu SQL, email, nhãn |
| Chế độ xem (Read, Outline, Draft…) | — | 🟡 | — | — | 🟡 | 🟡 | — | E2E doc-views | 🟡 | đọc, web, nháp, dàn ý, tập trung, ngăn điều hướng, ký hiệu định dạng (DOC-VIEW-001) |
| Mẫu hành chính NĐ 30 | — | ✅ | ✅ | ✅ | ✅ | ✅ | — | E2E, templates tests, admin_format.rs | ✅ | 8 mẫu + kiểm tra thể thức (DOC-TPL-001); kiểm tra tài liệu bất kỳ + sửa một/sửa tất cả (DOC-TPL-002) |
| Phương trình | ✅ | 🟡 | ⬜ | ✅ | 🟡 | 🟡 | ✅ | corpus math-* | 🟡 | xem CORE |

## BlueSheet

| Tính năng | Đọc | Hiển thị | Tạo/sửa | Round-trip | Web | Desktop | Cộng tác | Test | Trạng thái | Ghi chú |
|---|---|---|---|---|---|---|---|---|---|---|
| Hàm (515) | ✅ | ✅ | 🟡 | ✅ | ✅ | ✅ | ✅ | functions.md | 🟡 | 508/515 tính được; 7 hàm Cube giữ công thức + giá trị cache (không có nguồn OLAP, gõ mới → #NAME?); RTD giữ giá trị cache; WEBSERVICE cần provider |
| Mảng động, LET, LAMBDA | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | tests | ✅ | |
| @, trim ref, liên kết ngoài | 🟡 | ⬜ | ⬜ | ✅ | ⬜ | ⬜ | — | — | ⬜ | |
| Định dạng số | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | tests | 🟡 | thiếu [DBNum], lịch âm |
| CF / DV | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | corpus cf-*, writer-cf-*, dv-* | ✅ | data bar âm/trục, 20 bộ biểu tượng + tuỳ chỉnh, Stop If True, CF pivot (SHEET-CF-001) |
| Sort / filter | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | E2E, advfilter tests | ✅ | theo màu/biểu tượng/danh sách, top N, trung bình, ngày động, Advanced Filter (SHEET-DATA-001) |
| Pivot | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🟡 | tests + E2E | 🟡 | thiếu drill-down |
| PivotChart | ✅ | ✅ | 🟡 | ✅ | ✅ | ✅ | — | tests | 🟡 | |
| Slicer / Timeline | ✅ | 🟡 | 🟡 | ✅ | ✅ | ✅ | — | slicers.rs, timelines.rs, writer-slicers-*, timeline-1, sheet-slicer/timeline.spec | 🟡 | slicer bảng tạo/lọc/lưu; slicer pivot và timeline đọc/lọc/lưu lựa chọn (chưa tạo mới) |
| Goal Seek / Data Table / Solver | — | — | ✅ | — | ✅ | ✅ | — | — | 🟡 | + Scenario Manager & báo cáo (SHEET-WHATIF-003); Solver phi tuyến/tiến hoá/`dif` + 3 báo cáo (SHEET-SOLVER-002) |
| Kiểm tra công thức / tuỳ chọn tính | — | — | ✅ | ✅ | ✅ | ✅ | — | audit.rs, audit.test | 🟡 | truy vết, đánh giá từng bước, kiểm tra lỗi, cửa sổ theo dõi, thủ công/lặp/precision (SHEET-AUDIT-001) |
| Lấy & biến đổi dữ liệu / data model | 🟡 | ✅ | ✅ | ✅ | ✅ | ✅ | — | bo-query, sheet_query.rs, sheet-query.spec | 🟡 | truy vấn CSV/JSON/XML/sổ tính/thư mục/web + bước biến đổi, làm mới (SHEET-PQ-001); DataMashup của phần mềm khác giữ nguyên; data model: quan hệ, measure, pivot nhiều bảng (SHEET-DM-001, một phần) |
| Flash Fill, Text to Columns | — | — | ✅ | — | ✅ | ✅ | — | — | 🟡 | Flash Fill (SHEET-FLASH-001), Text to Columns + xoá trùng + Consolidate + Subtotal + dàn ý (SHEET-DATA-002); thiếu gợi ý khi gõ, thanh dàn ý |
| Analysis ToolPak | — | — | ✅ | — | ✅ | ✅ | — | — | 🟡 | 19 công cụ phân tích thống kê (SHEET-ATP-001); thiếu biểu đồ đầu ra |
| Sparklines | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | — | corpus writer-sparklines-* | 🟡 | đọc/ghi/vẽ/sửa (SHEET-SPARK-001); chưa dịch theo chèn hàng/cột |
| Bảng (ListObject) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | tests | 🟡 | thiếu total row tự động |
| Comment luồng / notes | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | tests | ✅ | |
| Bảo vệ sheet/workbook | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | — | tests | ✅ | |
| Form controls | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | — | corpus writer-form-controls-* | 🟡 | 9 loại, ô liên kết hai chiều (SHEET-FORMCTL-001); chưa dịch theo chèn hàng/cột |
| In bảng tính | ✅ | ✅ | 🟡 | ✅ | ✅ | ✅ | — | tests, view_cmd_tests, sheet-views.spec, print_job.rs | 🟡 | + Page Break Preview kéo ngắt, Page Layout view, Custom Views, Show Formulas (SHEET-VIEW-001); bảng In chung, nhiều trang/tờ, chọn trang (CORE-PRINT-001) |
| Hiệu năng 1 triệu ô | — | ✅ | ✅ | — | ✅ | ✅ | — | bench | 🟡 | chưa đo 10 triệu ô, pivot 1 triệu dòng |

## BlueSlide

| Tính năng | Đọc | Hiển thị | Tạo/sửa | Round-trip | Web | Desktop | Cộng tác | Test | Trạng thái | Ghi chú |
|---|---|---|---|---|---|---|---|---|---|---|
| Shape, text, bảng, ảnh | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | corpus + E2E | ✅ | |
| Master/layout/theme | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | tests, slide_master.rs | 🟡 | sửa master/layout, theme + biến thể, kích thước slide (SLIDE-MASTER-001); chưa co cỡ chữ khi đổi kích thước |
| Chế độ xem | — | 🟡 | — | — | ✅ | ✅ | — | E2E | 🟡 | Normal, Sorter, Reading, Presenter |
| Transition | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | — | corpus anim-*, slide-edit-transitions-all (Choice + Fallback hợp lệ), transitionFx test, slide-show-media.spec | ✅ | 55 hiệu ứng (§19.3.1 + [MS-PPTX] p14/p15 prstTrans), biến thể hướng, thời lượng, âm thanh WAV/dừng, áp dụng tất cả; Morph theo đối tượng (vị trí/cỡ/xoay/màu; chưa theo từ/ký tự) |
| Animation | ✅ | ✅ | 🟡 | ✅ | ✅ | ✅ | — | corpus slide-edit-animations-all, anim_play/anim_catalog tests, effectFx test, slide-show-media.spec | 🟡 | phát mọi hiệu ứng trong file (bộ thông dịch §19.5); tạo 25 xuất hiện/biến mất, 7 nhấn mạnh, 13 đường chuyển động; chưa vẽ đường tuỳ ý, chưa hiện đổi màu, chưa theo từ/chữ |
| Morph | ✅ | ⬜ | ⬜ | ✅ | ⬜ | ⬜ | — | — | ⬜ | |
| Audio/video | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | — | corpus media-*, slide_media.rs | ✅ | trim, fade, bookmark, phụ đề WebVTT, chế độ bắt đầu (SLIDE-MEDIA-001) |
| Trình chiếu + Presenter | — | ✅ | ✅ | ✅ | ✅ | ✅ | — | E2E, slide_present.rs | 🟡 | custom show, tập dượt, bút/laser, phóng, kiosk (SLIDE-SHOW-001); thiếu thuyết minh |
| Ghi thuyết trình / xuất video | ✅ | ✅ | ✅ | ✅ | ✅ | 🟡 | — | slide_video.rs, video.test, slide_record.rs, slide-record.spec | 🟡 | ghi thuyết minh/camera/nét vẽ/thời gian vào .pptx, phát lại (SLIDE-REC-001); xuất MP4/WebM có tiếng + camera (web), GIF/PNG mọi nơi; desktop chưa thử thiết bị thật |
| Xuất PDF/PNG | — | ✅ | — | — | ✅ | ✅ | — | tests, slide_export.rs | 🟡 | + handout 1–9, trang ghi chú, JPEG/SVG, gói (SLIDE-EXPORT-002); thiếu media liên kết |
| SmartArt | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | — | corpus, smartart.spec | ✅ | chèn từ thư viện bố cục, ngăn văn bản, bố cục/màu/kiểu, chuyển thành hình |
| Comment hiện đại | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | tests | ✅ | |
