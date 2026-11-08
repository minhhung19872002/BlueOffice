# BlueOffice — Giai đoạn 2: Đạt tương đương tính năng (Feature Parity) — CHẠY TỰ ĐỘNG ĐẾN KHI XONG

> Đặt file này ở gốc repo. Đọc toàn bộ trước khi làm. File này bổ sung (không thay thế) `BlueOffice-PROMPT.md`; mọi nguyên tắc trong đó (clean-room, kiến trúc, quy ước code, đặt tên) vẫn giữ nguyên hiệu lực.

---

## 0. Mục tiêu

BlueDoc, BlueSheet, BlueSlide phải đạt **tương đương chức năng** với bộ office thương mại phổ biến nhất (dùng làm chuẩn so sánh hành vi), cho mọi tính năng mà người dùng văn phòng, cơ quan nhà nước và doanh nghiệp Việt Nam thực sự dùng. "Tương đương" nghĩa là: người dùng làm được cùng việc đó trong BlueOffice, file tạo ra mở trong phần mềm kia hiển thị và hoạt động đúng, và ngược lại.

Không yêu cầu giống hệt giao diện. Giao diện, icon, tên lệnh, văn bản hướng dẫn là của BlueOffice.

---

## 1. Chế độ làm việc tự động (BẮT BUỘC)

Bạn làm việc **liên tục, không dừng giữa chừng để hỏi hay báo cáo**, cho đến khi điều kiện kết thúc ở mục 1.6 thỏa mãn.

### 1.1. Nguồn sự thật duy nhất: `docs/parity/BACKLOG.md`
- Việc đầu tiên: tạo `docs/parity/BACKLOG.md` từ toàn bộ danh sách tính năng ở mục 4–7, chia thành hạng mục nhỏ (mỗi hạng mục làm xong trong khoảng vài giờ công, có thể kiểm thử độc lập).
- Mỗi hạng mục có: ID (vd `DOC-SPELL-003`), mô tả, độ ưu tiên (P0/P1/P2/P3), phụ thuộc, tiêu chí hoàn thành, trạng thái.
- Trạng thái: `[ ]` chưa làm · `[~]` đang làm · `[x]` xong · `[p]` xong một phần (ghi rõ phần thiếu và lý do) · `[!]` bị chặn (ghi lý do, chuyển sang `NEEDS_HUMAN.md`).
- Thứ tự làm: P0 → P1 → P2 → P3; trong cùng mức, ưu tiên hạng mục mở khóa nhiều hạng mục khác.

### 1.2. Vòng lặp cho mỗi hạng mục
1. Chọn hạng mục `[ ]` tiếp theo theo thứ tự, đánh dấu `[~]`, commit BACKLOG.
2. Viết kế hoạch ngắn (5–15 dòng) ngay dưới hạng mục trong BACKLOG.
3. Cài đặt. Viết test (unit + round-trip + visual nếu có hiển thị + E2E nếu có UI).
4. Chạy test liên quan → sửa đến xanh. Sau đó chạy bộ test hồi quy đầy đủ (Rust, TS, .NET, E2E web + desktop).
5. Chạy benchmark; không được chậm hơn mốc trước quá 5%. Nếu chậm hơn: tối ưu trước khi đi tiếp.
6. Đánh dấu `[x]` (hoặc `[p]`), cập nhật `docs/parity/MATRIX.md` và `docs/PROGRESS.md`.
7. Commit (conventional commits, mỗi hạng mục ít nhất 1 commit) và push.
8. Sang hạng mục tiếp theo ngay. KHÔNG dừng để báo cáo.

### 1.3. Khi gặp khó khăn
- **Không hỏi người dùng.** Tự quyết định theo cách hợp lý nhất, ghi lại trong `docs/adr/` rồi làm tiếp.
- Bị chặn thật sự (cần tài sản/ảnh mốc từ người dùng, cần tài khoản dịch vụ ngoài, license chưa rõ, cần quyết định pháp lý/kinh doanh): ghi vào `docs/parity/NEEDS_HUMAN.md` (ID, việc cần, vì sao, phương án tạm), đánh dấu `[!]`, chuyển sang hạng mục khác.
- Thử một cách 3 lần không được: ghi nhận xét vào hạng mục, thử hướng khác; vẫn không được thì đánh dấu `[p]` với phần đã làm và lý do, làm tiếp hạng mục khác, quay lại sau ở vòng rà soát.
- Test hỏng do thay đổi của mình: sửa code, không sửa test cho khớp. Chỉ sửa test khi chứng minh được test sai (ghi lý do trong commit).

### 1.4. Trung thực
- Không đánh dấu `[x]` nếu chưa có test chứng minh. Không làm giả kết quả, không bỏ qua test, không tắt test, không hard-code đầu ra để qua test.
- Tính năng chỉ giữ nguyên/hiển thị gần đúng thì ghi `[p]`, không ghi `[x]`.
- `MATRIX.md` phải phản ánh đúng thực tế để người dùng tin được.

### 1.5. Quản lý ngữ cảnh & phiên làm việc
- Sau mỗi hạng mục, `BACKLOG.md` + `PROGRESS.md` phải đủ để một phiên mới tiếp tục mà không cần lịch sử hội thoại.
- Khi bắt đầu (hoặc tiếp tục) một phiên: đọc `CLAUDE.md`, file này, `BACKLOG.md`, `PROGRESS.md`, `NEEDS_HUMAN.md`; tìm hạng mục `[~]` dở dang → làm tiếp; không có thì lấy hạng mục `[ ]` tiếp theo.
- Dùng subagent cho việc song song độc lập (nghiên cứu spec, viết test, rà soát code), nhưng chỉ một luồng chính được sửa BACKLOG.
- Giữ `CLAUDE.md` gọn: lệnh build/test, cấu trúc, quy tắc; chi tiết đưa vào `docs/`.

### 1.6. Điều kiện kết thúc
Chỉ dừng khi TẤT CẢ đúng:
1. Mọi hạng mục trong BACKLOG ở trạng thái `[x]`, hoặc `[p]`/`[!]` có lý do rõ ràng không thể giải quyết mà không có con người.
2. Đã chạy **vòng rà soát cuối** (mục 9) và mọi lỗi phát hiện đã sửa.
3. Toàn bộ test xanh, benchmark đạt mục tiêu, CI xanh, đã push.
4. Đã viết `docs/parity/FINAL_REPORT.md`: tổng kết theo MATRIX, những gì còn `[p]`/`[!]` và việc người dùng cần làm.

### 1.7. Rà soát định kỳ
Cứ mỗi 15 hạng mục hoàn thành:
- Chạy toàn bộ fidelity report, cập nhật số liệu trong MATRIX.
- Rà lại hạng mục `[p]` xem có thể hoàn thiện bằng nền tảng mới đã xây không.
- Rà nợ kỹ thuật (TODO, code trùng lặp, cảnh báo clippy) và xử lý những gì quan trọng.
- Bổ sung vào BACKLOG những tính năng phát hiện còn thiếu khi so sánh.

---

## 2. Nguyên tắc clean-room (nhắc lại, không ngoại lệ)

- KHÔNG decompile, disassemble, debug-attach, phân tích binary/DLL của bất kỳ phần mềm office thương mại nào.
- KHÔNG sao chép mã nguồn, icon, hình ảnh, font độc quyền, template, gallery, văn bản giao diện, file trợ giúp, từ điển độc quyền, thư viện SmartArt/biểu tượng/ảnh của họ.
- Nguồn hợp lệ: đặc tả công khai (ECMA-376, ISO/IEC 29500, Microsoft Open Specifications [MS-*], OASIS ODF, Unicode, W3C, IETF, ISO 32000 PDF), tài liệu công khai mô tả *hành vi* tính năng/hàm (tự cài đặt lại), quan sát hành vi bên ngoài (black-box).
- Thư viện bên thứ ba: chỉ MIT / Apache-2.0 / BSD / MPL-2.0 / Zlib / ISC / OFL / CC0 / CC-BY (có ghi công). Không GPL / AGPL / LGPL liên kết tĩnh. Mô hình AI/dữ liệu (từ điển, model ONNX, dữ liệu bản đồ…) cũng phải kiểm license. License không rõ → `[!]` + NEEDS_HUMAN.
- Tên lệnh, tên tính năng trong UI đặt theo cách của BlueOffice (vd "Gợi ý thiết kế" thay vì tên thương mại của bên khác).

---

## 3. Ma trận tương đương: `docs/parity/MATRIX.md`

Bảng theo dõi cho từng tính năng ở mục 4–7, cột: Tính năng · App · Đọc file · Hiển thị · Tạo/sửa · Lưu round-trip · Web · Desktop · Cộng tác · Test · Trạng thái · Ghi chú.

Mức đánh giá mỗi ô: ✅ đầy đủ · 🟡 một phần · ⬜ chưa có · — không áp dụng.

Đầu file có bảng tổng hợp % hoàn thành theo app và theo nhóm tính năng; cập nhật sau mỗi hạng mục.

---

## 4. Tính năng chung cho cả bộ (prefix `CORE-`)

### 4.1. Giao diện & thao tác
- Ribbon đầy đủ các tab theo ngữ cảnh (tab ngữ cảnh xuất hiện khi chọn bảng/ảnh/biểu đồ/shape/header…), thu gọn/mở rộng ribbon, chế độ ribbon đơn giản (một hàng).
- Màn hình File (backstage): Mới (từ mẫu), Mở (gần đây, đã ghim, BlueDrive, máy tính), Thông tin (thuộc tính tài liệu, bảo vệ, kiểm tra tài liệu, phiên bản), Lưu/Lưu thành (mọi định dạng), In (xem trước + thiết lập), Chia sẻ, Xuất, Đóng, Tùy chọn.
- Thanh truy cập nhanh tùy biến; tùy biến ribbon (thêm/bớt nhóm, lệnh, tab riêng), xuất/nhập cấu hình.
- Ô tìm lệnh ("Tìm kiếm thao tác"): gõ tên tính năng → chạy lệnh, kèm gợi ý.
- Menu chuột phải + thanh định dạng nổi (mini toolbar) theo ngữ cảnh.
- Bộ phím tắt đầy đủ cho mọi lệnh phổ biến, hỗ trợ phím tắt kiểu Alt-key (hiện chữ gợi ý trên ribbon), tùy biến phím tắt. Phím tắt chức năng tương thích thói quen người dùng (Ctrl+B, Ctrl+S, F4 lặp lệnh/đổi tham chiếu tuyệt đối, F2, F5 trình chiếu…).
- Ngăn tác vụ (task pane) dạng dock/float: Styles, Navigation, Selection, Animation, Format Shape, Comments, Version History…
- Thanh trạng thái tùy biến (số trang, số từ, ngôn ngữ, thống kê vùng chọn trong bảng tính, zoom, chế độ xem).
- Giao diện sáng/tối/theo hệ thống, độ tương phản cao, tỷ lệ hiển thị (DPI) chuẩn.
- Đa cửa sổ, chia màn hình cùng tài liệu (New Window, Arrange, Split), so sánh song song có cuộn đồng bộ.

### 4.2. Văn bản & chỉnh sửa dùng chung (engine dùng lại cho Doc, ô/text box Sheet, Slide)
- Soát lỗi chính tả (gạch đỏ) và gợi ý sửa cho **tiếng Việt và tiếng Anh** tối thiểu; kiến trúc plugin để thêm ngôn ngữ (Hunspell-compatible). Từ điển người dùng, danh sách loại trừ, bỏ qua tất cả, kiểm tra toàn tài liệu, đánh dấu ngôn ngữ cho đoạn văn. Từ điển phải license hợp lệ; nếu không có từ điển tiếng Việt license phù hợp → tự xây từ nguồn dữ liệu mở license hợp lệ và ghi nguồn.
- Kiểm tra ngữ pháp & văn phong cơ bản (gạch xanh): lỗi khoảng trắng, dấu câu, viết hoa đầu câu, lặp từ, dấu tiếng Việt đặt sai vị trí (hòa/hoà theo cấu hình), dấu ngoặc không đóng.
- AutoCorrect: danh sách thay thế tùy biến, viết hoa đầu câu, sửa hai chữ hoa đầu, ký hiệu ((c) → ©), dấu nháy thông minh, gạch ngang, phân số, tự động đánh số/gạch đầu dòng, tự định dạng đường link.
- Từ điển đồng nghĩa (tiếng Anh tối thiểu, tiếng Việt nếu có dữ liệu license phù hợp), đếm từ (kể cả textbox, chú thích).
- Tìm/thay nâng cao: ký tự đặc biệt, wildcard, regex, theo định dạng/style, highlight tất cả kết quả.
- Dán: giữ định dạng nguồn / gộp định dạng / chỉ văn bản / ảnh / liên kết, Paste Special đầy đủ; clipboard nhiều mục (lịch sử dán).
- Biểu tượng & ký tự đặc biệt (bảng Unicode có tìm kiếm), phương trình (xem 4.4).
- Gõ tiếng Việt: kiểm tra kỹ IME (Unikey, EVKey, bộ gõ macOS/Windows/Linux, bàn phím ảo mobile) trong mọi ô nhập; không mất ký tự, không nhảy con trỏ.

### 4.3. Đối tượng đồ họa dùng chung
- Shapes: toàn bộ preset geometry của DrawingML, custom geometry, chỉnh điểm (Edit Points), merge shapes (union/combine/fragment/intersect/subtract), connector bám điểm nối.
- Định dạng: fill (solid, gradient nhiều điểm, pattern, picture, texture, nền slide), line (nét, mũi tên, nối góc), hiệu ứng (shadow, reflection, glow, soft edges, bevel, 3D rotation, 3D format) — hiển thị đúng theo spec, kể cả hiệu ứng 3D (dựng hình 3D thật cho bevel/extrusion/rotation bằng WebGL, không vẽ gần đúng).
- WordArt / text effects: outline, fill gradient, transform (text warp theo preset), shadow, glow, 3D.
- Ảnh: crop (tự do, theo tỷ lệ, theo hình), xóa nền (mô hình phân đoạn license hợp lệ, chạy cục bộ), điều chỉnh độ sáng/tương phản/độ nét, màu (saturation, tone, recolor), hiệu ứng nghệ thuật, nén ảnh, đặt lại ảnh, thay ảnh, văn bản thay thế (alt text), định dạng SVG đầy đủ (chuyển SVG thành shape).
- Biểu tượng (icons) & thư viện ảnh mẫu: tự xây bộ riêng từ nguồn CC0/MIT, có tìm kiếm.
- Mô hình 3D: chèn glTF/GLB/OBJ/FBX(nếu có lib phù hợp)/STL, xoay/zoom trong tài liệu, đọc/ghi phần mô hình 3D trong OOXML, hiển thị trong trình chiếu.
- Biểu đồ (chung cho 3 app, xem 6.6): mọi loại biểu đồ, định dạng chi tiết, dữ liệu nhúng bảng tính.
- SmartArt (xem 4.4).
- Ink / vẽ tay: bút, bút dạ, tẩy, thước kẻ, chuyển nét vẽ thành shape/văn bản/toán, phát lại nét vẽ, hỗ trợ bút cảm ứng (pressure).
- Căn chỉnh, phân bố, xoay/lật, nhóm, thứ tự lớp, khung lựa chọn (Selection Pane: ẩn/hiện/đổi tên/sắp xếp), lưới & đường gióng, smart guides.
- Nhúng đối tượng giữa các app: bảng BlueSheet trong BlueDoc/BlueSlide (nhúng và liên kết, cập nhật liên kết), biểu đồ liên kết; OLE object trong file: hiển thị ảnh preview và giữ nguyên dữ liệu.

### 4.4. Phương trình, SmartArt, trích dẫn
- **Phương trình**: đọc/ghi OMML đầy đủ, hiển thị chính xác (phân số, căn, chỉ số, tích phân, tổng, ma trận, ngoặc co giãn, dấu nhấn, giới hạn, hàm, hộp, thanh…, theo quy tắc dàn trang toán học như OpenType MATH table); trình soạn phương trình trực quan; nhập bằng cú pháp tuyến tính kiểu UnicodeMath và LaTeX; thư viện phương trình mẫu (tự soạn); chuyển đổi chuyên nghiệp ↔ tuyến tính; phương trình inline và display; đánh số phương trình; font toán OFL (vd STIX Two, Latin Modern Math).
- **SmartArt**: đọc data model + layout definition + style + color trong OOXML và **tự dựng layout** (không chỉ dùng drawing fallback); tạo mới từ thư viện bố cục do BlueOffice tự thiết kế, phủ đủ các nhóm: danh sách, quy trình, chu trình, phân cấp (sơ đồ tổ chức), quan hệ, ma trận, kim tự tháp, ảnh; sửa nội dung qua ngăn văn bản, thêm/xóa/nâng/hạ cấp nút, đổi bố cục/màu/style; chuyển SmartArt thành shapes; ghi ra OOXML hợp lệ (data model + drawing fallback) để phần mềm khác mở được.
- **Trích dẫn & tài liệu tham khảo**: quản lý nguồn, chèn trích dẫn, danh mục tài liệu tham khảo theo kiểu APA, MLA, Chicago, IEEE, Harvard, ISO 690 và kiểu thông dụng ở Việt Nam; import BibTeX/RIS.

### 4.5. Ngôn ngữ, giọng nói, AI (kiến trúc provider, chạy được on-premise)
- Dịch văn bản/tài liệu: interface `TranslationProvider`; cài sẵn provider gọi API cấu hình được và provider chạy cục bộ (model license hợp lệ) cho Việt ↔ Anh.
- Đọc chính tả (speech-to-text): `DictationProvider`, có bản chạy cục bộ (vd model nhận dạng giọng nói license MIT/Apache), hỗ trợ tiếng Việt, lệnh dấu câu bằng giọng nói.
- Đọc to (text-to-speech): `SpeechProvider`, highlight từ đang đọc, tốc độ/giọng.
- Trình đọc tập trung (immersive reader): giãn dòng/chữ, tô sáng dòng, tách âm tiết, chế độ tập trung.
- Trợ lý AI "BlueAI" (provider LLM cấu hình được, mặc định tắt, bật được on-premise): viết lại, tóm tắt, soạn nháp, tạo slide từ dàn ý, gợi ý công thức, phân tích dữ liệu. Không gửi dữ liệu ra ngoài khi chưa được quản trị viên cho phép.
- Gợi ý thiết kế slide (rule-based + tùy chọn AI), gợi ý biểu đồ, phân tích nhanh vùng dữ liệu.

### 4.6. Cộng tác, chia sẻ, bảo mật
- Đồng tác giả thời gian thực (đã có) — mở rộng cho mọi đối tượng mới; hiển thị ai đang sửa đoạn/ô/slide nào.
- Comment luồng, @mention (thông báo qua BlueDrive/email), giao việc trong comment, giải quyết/mở lại; ghi chú (notes) kiểu cũ trong bảng tính.
- Lịch sử phiên bản: xem, so sánh, khôi phục; hiển thị thay đổi theo người & thời gian (Show Changes cho bảng tính).
- Bảo vệ: mật khẩu mở/sửa, chỉ đọc khuyến nghị, đánh dấu bản cuối, giới hạn chỉnh sửa theo vùng/người, bảo vệ sheet/workbook/ô khóa, nhãn phân loại tài liệu (nhãn độ mật tùy biến) gắn metadata.
- Kiểm tra tài liệu: xóa metadata/comment/nội dung ẩn/thông tin cá nhân; kiểm tra khả năng tiếp cận (alt text, thứ tự đọc, tương phản, tiêu đề bảng…) có đề xuất sửa; kiểm tra tương thích (tính năng sẽ mất khi lưu định dạng cũ).
- Ký số: đã có — bổ sung dòng chữ ký (signature line) trong văn bản, nhiều chữ ký, ký số theo quy định Việt Nam (chuẩn bị tích hợp USB token PKCS#11 và ký số từ xa qua provider), kiểm tra chuỗi chứng thư & thu hồi (OCSP/CRL), dấu thời gian (TSA).
- Tự động lưu, khôi phục sau sự cố, lưu bản tạm định kỳ.

### 4.7. Tự động hóa & mở rộng
- **VBA**: KHÔNG thực thi macro VBA trong tài liệu (an toàn) — giữ nguyên khi lưu, cảnh báo, hiển thị mã chỉ đọc. Ghi chú trong MATRIX là `— (chủ ý)`.
- **BlueScript**: ngôn ngữ tự động hóa riêng dựa trên TypeScript/JavaScript, chạy sandbox (Web Worker/QuickJS hoặc tương đương license hợp lệ), API đối tượng cho Doc/Sheet/Slide (đọc/sửa nội dung, định dạng, bảng, biểu đồ…), trình soạn mã có gợi ý, **ghi thao tác thành script** (macro recorder), gán script vào nút/phím tắt, chạy theo lịch (server).
- Công cụ chuyển đổi hỗ trợ: phân tích mã VBA → tạo bản nháp BlueScript tương đương (best-effort, người dùng duyệt).
- **Add-in**: nền tảng add-in web (manifest, task pane, lệnh ribbon, quyền truy cập), API JavaScript riêng của BlueOffice; marketplace nội bộ cho tổ chức.
- API server (REST) để hệ thống khác tạo/sửa/chuyển đổi tài liệu (mail merge hàng loạt, sinh báo cáo).

### 4.8. Định dạng file (bổ sung cho đủ)
- Mẫu và biến thể macro: `.dotx/.dotm/.docm`, `.xltx/.xltm/.xlsm/.xlam(giữ nguyên)`, `.potx/.potm/.pptm/.ppsx/.ppsm`, `.thmx` (theme).
- Flat OPC XML (Word XML, Excel XML 2003 Spreadsheet đọc), `.htm/.html/.mht` (lưu & mở), `.xps` (xuất, nếu khả thi), `.pdf` (xuất PDF/A-1b/2b/3b, PDF có thẻ cấu trúc cho khả năng tiếp cận, bookmark từ heading, liên kết), mở PDF trong BlueDoc để chỉnh sửa (chuyển PDF → văn bản có định dạng, best-effort).
- `.epub` (xuất từ BlueDoc), `.prn/.dif/.slk` (đọc/ghi bảng tính cũ), `.wpd/.wps` (nếu khả thi), `.dbf` (đọc).
- Xuất slide ra video MP4 (có thời gian, hiệu ứng, lời thuyết minh) và GIF động; xuất ảnh PNG/JPG/SVG từng slide.
- Kiểm tra lại mọi định dạng mới bằng round-trip + validator.

### 4.9. In ấn
- Xem trước in chính xác (dùng display list), chọn trang/vùng/sheet/slide, số bản, collate, in hai mặt (thủ công/tự động), nhiều trang trên một tờ, co giãn theo khổ, in nền/màu/ảnh ẩn, in comment/markup, in handout/notes cho slide, in tiêu đề hàng/cột và đường lưới cho bảng tính.

### 4.10. Hiệu năng & độ tin cậy (giữ mốc cũ, thêm)
- Mở .docx 1.000 trang có ảnh và bảng < 8 s; .xlsx 10 triệu ô (dữ liệu thô) mở được, cuộn mượt; .pptx 300 slide có video mở < 5 s.
- Không rò rỉ bộ nhớ khi mở/đóng 100 tài liệu liên tiếp (test tự động).
- Fuzzing chạy liên tục cho mọi parser mới.

---

## 5. BlueDoc — tương đương trình soạn thảo văn bản (prefix `DOC-`)

### 5.1. Chế độ xem
Print Layout, Read Mode (tự động chia cột, điều hướng trang), Web Layout, Outline (thu/mở cấp, kéo thả di chuyển đề mục), Draft, Focus mode, Navigation pane (heading, trang dạng thumbnail, kết quả tìm kiếm), hiển thị đa trang, ký tự ẩn (¶, khoảng trắng, tab, ngắt), thước, lưới, so sánh hai tài liệu cạnh nhau.

### 5.2. Định dạng & bố cục
- Mọi thuộc tính ký tự/đoạn của WordprocessingML (kể cả: ẩn chữ, emboss/engrave, outline/shadow chữ, khoảng cách ký tự, vị trí nâng/hạ, kerning, ligature/OpenType features, ruby/phonetic guide, combine characters, viền ký tự, character style liên kết).
- Drop cap, cột (không đều, đường kẻ giữa cột), ngắt cột/trang/section/ngắt dòng có điều kiện, đánh số dòng, gạch nối tự động & thủ công, kiểm soát dòng mồ côi/góa, chữ dọc (hướng văn bản), bidi.
- Viền trang (kể cả viền nghệ thuật — tự thiết kế bộ viền), màu nền trang, watermark (chữ/ảnh, tùy biến), theme tài liệu (màu/font/hiệu ứng), style set.
- Style: tạo/sửa/xóa, style pane, style inspector, quản lý style (import/export giữa tài liệu, organizer), kiểm tra định dạng (Reveal Formatting).
- Danh sách: định nghĩa danh sách đa cấp mới, liên kết cấp với heading style, thư viện bullet/số (tự thiết kế), tiếp tục/khởi động lại số, đặt giá trị số.
- Tab: mọi kiểu căn (trái/giữa/phải/thập phân/bar), ký tự dẫn.
- Header/footer: khác trang đầu, chẵn/lẻ, liên kết với section trước, các khối dựng sẵn (tự thiết kế).
- Building blocks / Quick Parts / AutoText: lưu và chèn đoạn nội dung tái sử dụng, thư viện khối (trang bìa, textbox, header…) tự thiết kế.

### 5.3. Bảng
Vẽ bảng, tẩy, chia/gộp ô và bảng, phân phối hàng/cột đều, AutoFit, style bảng (thư viện tự thiết kế + tùy biến tùy chọn hàng đầu/cuối, cột đầu/cuối, dải màu), sắp xếp dữ liệu trong bảng, công thức trong bảng (SUM(ABOVE)…), chuyển văn bản ↔ bảng, bảng lồng, bảng nổi (text wrap quanh bảng), văn bản dọc trong ô, lặp tiêu đề, cho/không cho ngắt hàng qua trang, căn lề ô, khoảng cách ô.

### 5.4. Tham chiếu
Mục lục (nhiều kiểu, tùy biến cấp, từ style tùy chọn, cập nhật số trang/toàn bộ), mục lục hình/bảng, chỉ mục (Index: đánh dấu mục, chỉ mục nhiều cấp, cập nhật), bảng trích dẫn pháp lý (table of authorities), chú thích chân trang/cuối văn bản (đánh số tùy biến, chuyển đổi qua lại, ký hiệu tùy chọn, tiếp nối), caption tự động, cross-reference, bookmark, trích dẫn/tài liệu tham khảo (4.4).

### 5.5. Field & form
- Hỗ trợ đầy đủ các field của WordprocessingML: ASK, AUTHOR, COMMENTS, CREATEDATE, DATE, DOCPROPERTY, DOCVARIABLE, EQ, FILENAME, FILESIZE, FILLIN, FORMTEXT/CHECKBOX/DROPDOWN, GOTOBUTTON, HYPERLINK, IF, INCLUDEPICTURE, INCLUDETEXT, INDEX, KEYWORDS, LASTSAVEDBY, LINK, LISTNUM, MACROBUTTON (không chạy), MERGEFIELD, MERGEREC, MERGESEQ, NEXT, NEXTIF, NOTEREF, NUMCHARS, NUMPAGES, NUMWORDS, PAGE, PAGEREF, PRINTDATE, QUOTE, REF, REVNUM, SAVEDATE, SECTION, SECTIONPAGES, SEQ, SET, SKIPIF, STYLEREF, SUBJECT, SYMBOL, TA, TC, TEMPLATE, TIME, TITLE, TOA, TOC, USERADDRESS, USERINITIALS, USERNAME, XE, = (biểu thức) — kèm định dạng chuyển đổi (\* \# \@), hiện mã/kết quả field (Alt+F9), cập nhật field, khóa field.
- Content controls: plain text, rich text, picture, combo box, dropdown, date picker, checkbox, repeating section, building block gallery; thuộc tính, khóa, liên kết dữ liệu XML tùy biến (custom XML parts).
- Biểu mẫu kiểu cũ (legacy form fields) và chế độ chỉ điền biểu mẫu.

### 5.6. Thư và nhãn
Mail merge (đã có) — bổ sung: nguồn từ BlueSheet/CSV/Excel/danh bạ/SQL, quy tắc (IF, Skip, Ask, Fill-in), xem trước, lọc/sắp xếp người nhận, kiểm tra lỗi, trộn ra tài liệu mới/máy in/email/PDF từng người; phong bì và nhãn (thư viện khổ nhãn phổ biến).

### 5.7. Review
Track changes (đã có) — bổ sung: các chế độ hiển thị (Simple/All/No markup/Original), bong bóng lề, ngăn Reviewing, khóa theo dõi bằng mật khẩu, theo dõi định dạng và di chuyển (move from/to); **So sánh hai tài liệu** (tạo tài liệu kết quả có track changes) và **Kết hợp** nhiều bản sửa; chặn tác giả; ẩn danh hóa tác giả.

### 5.8. Khác
Phong bì, bộ đếm từ chi tiết, ngôn ngữ đoạn văn, chèn tài liệu khác (Insert Text from File), chèn đối tượng, chèn video trực tuyến (liên kết, hiển thị poster), chụp màn hình/cắt màn hình (desktop), mẫu tài liệu (thư viện mẫu tự thiết kế, ưu tiên mẫu hành chính Việt Nam: công văn, quyết định, tờ trình, biên bản, hợp đồng, báo cáo, giấy mời, kế hoạch… theo Nghị định 30/2020 và văn bản liên quan), thể thức văn bản hành chính kiểm tra tự động (font, cỡ chữ, lề, vị trí các thành phần).

---

## 6. BlueSheet — tương đương bảng tính (prefix `SHEET-`)

### 6.1. Hàm — đủ toàn bộ thư viện hàm của bảng tính thương mại phổ biến (~500 hàm)
- Bước đầu: tạo `docs/parity/functions.md` liệt kê **toàn bộ** hàm theo tài liệu công khai, mỗi hàm một dòng: nhóm, cú pháp, trạng thái, test. Hiện có 276 → cài nốt phần còn lại.
- Các nhóm phải đủ: Math & Trig (gồm SERIESSUM, MULTINOMIAL, BASE/DECIMAL, ROMAN/ARABIC, MMULT/MINVERSE/MDETERM/MUNIT…), Statistical (mọi phân phối: BETA, BINOM, CHISQ, EXPON, F, GAMMA, HYPGEOM, LOGNORM, NEGBINOM, NORM, POISSON, T, WEIBULL, Z… cả .DIST/.INV/.TEST; LINEST, LOGEST, TREND, GROWTH, FORECAST.ETS và các biến thể, SKEW, KURT, FREQUENCY, PROB, STANDARDIZE, CONFIDENCE.*, COVARIANCE.*, PEARSON, RSQ, SLOPE, INTERCEPT, STEYX, DEVSQ, AVEDEV, TRIMMEAN, PERCENTRANK.*…), Financial (đủ nhóm trái phiếu & chứng khoán: ACCRINT, ACCRINTM, COUPDAYS*, COUPNCD, COUPNUM, COUPPCD, DISC, DURATION, MDURATION, INTRATE, ODDF*/ODDL*, PRICE*, RECEIVED, TBILL*, YIELD*, AMORDEGRC, AMORLINC, CUMIPMT, CUMPRINC, EFFECT, NOMINAL, MIRR, PDURATION, RRI, SYD, VDB, DOLLARDE/DOLLARFR, FVSCHEDULE, ISPMT), Engineering (BESSEL*, BIN/OCT/DEC/HEX chuyển đổi, BITAND/BITOR/BITXOR/BITLSHIFT/BITRSHIFT, COMPLEX và mọi hàm IM*, CONVERT với đầy đủ đơn vị, DELTA, GESTEP, ERF/ERFC(.PRECISE)), Database (DAVERAGE, DCOUNT, DCOUNTA, DGET, DMAX, DMIN, DPRODUCT, DSTDEV(P), DSUM, DVAR(P)), Text (đủ, gồm REGEXTEST, REGEXEXTRACT, REGEXREPLACE, ARRAYTOTEXT, VALUETOTEXT, FIXED, DOLLAR, BAHTTEXT, ASC/DBCS, PHONETIC, T, TEXTSPLIT…; thêm hàm riêng đọc số tiền bằng chữ tiếng Việt `BO.DOCSO` — đặt tiền tố để không đụng tên hàm chuẩn), Lookup & array (VSTACK, HSTACK, TAKE, DROP, CHOOSEROWS, CHOOSECOLS, EXPAND, TOCOL, TOROW, WRAPROWS, WRAPCOLS, GROUPBY, PIVOTBY, PERCENTOF, TRIMRANGE và toán tử trim reference, AREAS, HYPERLINK, GETPIVOTDATA, RTD(giữ nguyên), FORMULATEXT, ISFORMULA, IMAGE), LAMBDA helpers (MAP, REDUCE, SCAN, BYROW, BYCOL, MAKEARRAY, ISOMITTED), Information (CELL đầy đủ, INFO, ERROR.TYPE, SHEET, SHEETS, ISEVEN/ISODD…), Compatibility (mọi hàm cũ: BETADIST, CHIDIST, NORMDIST, PERCENTILE, RANK, STDEV, VAR, MODE, CONFIDENCE, COVAR, FDIST, TDIST…), Web (ENCODEURL; WEBSERVICE/FILTERXML: cài đặt nhưng mặc định tắt, quản trị viên bật), Cube (giữ nguyên công thức & giá trị cache; tính toán khi có nguồn dữ liệu được hỗ trợ).
- Mỗi hàm: test bảng giá trị (bình thường, biên, lỗi, kiểu dữ liệu lạ, mảng), so khớp ngữ nghĩa lỗi và ép kiểu; độ chính xác số học như chuẩn IEEE 754 + quy tắc hiển thị 15 chữ số.
- Công thức: toán tử @ (implicit intersection), toán tử trim (.:.), tham chiếu ngoài (external workbook link: cập nhật, sửa liên kết, ngắt liên kết), tham chiếu bảng có cấu trúc đầy đủ, mảng hằng số, công thức mảng kiểu cũ (CSE), dữ liệu kiểu cũ bị giữ nguyên.

### 6.2. Dữ liệu & phân tích
- Sort tùy biến (theo màu ô/chữ/icon, theo danh sách tùy biến, theo hàng), AutoFilter nâng cao (lọc theo màu, top 10, trên/dưới trung bình, ngày động), Advanced Filter (vùng điều kiện, sao chép kết quả).
- Text to Columns, Flash Fill (học mẫu từ ví dụ), Remove Duplicates, Data Validation đầy đủ (mọi loại, thông báo nhập/lỗi, khoanh dữ liệu không hợp lệ), Consolidate, Subtotal, Group/Outline tự động.
- What-if: Goal Seek, Scenario Manager (tóm tắt kịch bản), Data Table (1 và 2 biến), **Solver** (tuyến tính: simplex; phi tuyến: GRG; tiến hóa; ràng buộc nguyên/nhị phân/alldifferent; báo cáo answer/sensitivity/limits), Forecast Sheet.
- Bộ công cụ phân tích thống kê (tương đương Analysis ToolPak): ANOVA (1 và 2 nhân tố), tương quan, hiệp phương sai, thống kê mô tả, làm trơn mũ, F-test, Fourier, histogram, trung bình trượt, sinh số ngẫu nhiên, xếp hạng & phân vị, hồi quy, lấy mẫu, t-test các loại, z-test.
- PivotTable đầy đủ: tạo/sửa, nhiều trường hàng/cột/giá trị/lọc, mọi hàm tổng hợp, "show values as" đủ kiểu, trường/mục tính toán, nhóm (ngày, số, thủ công), sắp xếp/lọc (label, value, top N), layout (compact/outline/tabular, subtotal, grand total, lặp nhãn), style pivot, refresh, đổi nguồn, nhiều pivot chung cache, drill-down (double click tạo sheet chi tiết), GETPIVOTDATA, **Slicer** và **Timeline** (đọc/ghi OOXML, điều khiển nhiều pivot/bảng), **PivotChart**.
- Data model (tương đương Power Pivot ở mức cốt lõi): quan hệ giữa các bảng, pivot từ nhiều bảng, measure với ngôn ngữ biểu thức riêng (thiết kế ngôn ngữ BlueOffice, có thể lấy cảm hứng cú pháp DAX phổ biến nhưng tự cài đặt), đọc dữ liệu data model đã nhúng nếu spec công khai cho phép, nếu không thì giữ nguyên và dùng cache.
- **Lấy & biến đổi dữ liệu** (tương đương Power Query ở mức cốt lõi): kết nối CSV/TXT, Excel, JSON, XML, thư mục, web (bảng HTML), PostgreSQL/SQL Server/MySQL (qua server), OData, REST; trình biên tập bước biến đổi trực quan (lọc, đổi kiểu, tách/gộp cột, pivot/unpivot, nhóm, merge/append, cột tính toán); lưu truy vấn, refresh (thủ công/định kỳ); đọc định nghĩa truy vấn có trong file và giữ nguyên khi lưu.
- Data types liên kết (chứng khoán/địa lý): đọc/giữ nguyên; tính năng thực tế thông qua provider cấu hình được.

### 6.3. Định dạng & hiển thị
- Conditional formatting đầy đủ: mọi loại quy tắc, data bar (gradient/solid, âm/dương, trục), color scale 2/3 màu, icon set (mọi bộ, tùy biến ngưỡng, bộ icon tự thiết kế), công thức, quản lý thứ tự & Stop If True, áp dụng cho pivot.
- Định dạng ô: mọi mã định dạng số (kể cả lịch âm/locale, `[DBNum]`, định dạng phân số tùy chỉnh, ký tự lặp `*`, `_`), căn lề (xoay chữ mọi góc, chữ dọc, thụt lề, shrink to fit, wrap, merge across), viền mọi kiểu, fill pattern/gradient, cell styles (thư viện tự thiết kế + tùy biến), theme.
- Sparklines (line/column/win-loss, nhóm, trục, điểm đánh dấu), checkbox trong ô, ảnh trong ô (place in cell) và ảnh trên ô, hàm IMAGE.
- Tables: style, total row, cột tính toán tự lan, tự mở rộng, chuyển về range, slicer cho bảng.
- Chế độ xem: Normal, Page Layout (thước, header/footer trực quan), Page Break Preview (kéo ngắt trang), Custom Views, ẩn/hiện công thức, đường lưới, tiêu đề; freeze/split; zoom vùng chọn; Sheet View (chế độ xem riêng khi cộng tác).
- Công cụ công thức: Name Manager, Trace Precedents/Dependents (mũi tên, kể cả sheet khác), Remove Arrows, Show Formulas, Error Checking (quy tắc tùy chỉnh, bỏ qua), Evaluate Formula (từng bước), Watch Window, tùy chọn tính (tự động/thủ công/trừ data table, lặp, độ chính xác như hiển thị, đa luồng), gợi ý công thức & tooltip cú pháp khi gõ.
- Quick Analysis (định dạng, biểu đồ, tổng, bảng, sparkline gợi ý cho vùng chọn), gợi ý biểu đồ.
- Form controls (button, checkbox, combo, list, option, scroll bar, spinner, group box, label) đọc/ghi/hoạt động (liên kết ô); ActiveX: giữ nguyên, hiển thị ảnh.
- Chia sẻ & bảo vệ: khóa ô, ẩn công thức, cho phép sửa vùng theo người, bảo vệ cấu trúc workbook.

### 6.4. In bảng tính
Vùng in, tiêu đề in, vừa trang (fit to N×M), căn giữa trang, header/footer có mã (&P, &N, &D, &F, &A, ảnh), thứ tự trang, in đường lưới/tiêu đề hàng-cột, in lỗi dạng tùy chọn, in comment.

### 6.5. Hiệu năng bảng tính
1.048.576 hàng × 16.384 cột (giới hạn chuẩn), tính lại đa luồng, công thức mảng lớn, pivot trên 1 triệu dòng < 3 s, lọc/sort 1 triệu dòng < 2 s.

### 6.6. Biểu đồ (dùng chung cả bộ)
Đủ mọi loại: column/bar (clustered, stacked, 100%, 3D), line (đánh dấu, stacked, 3D), pie/doughnut (pie of pie, bar of pie, explode, 3D), area (stacked, 3D), XY scatter (đường thẳng/cong), bubble (3D), radar (đánh dấu, filled), surface (3D, wireframe, contour), stock (HLC, OHLC, VHLC, VOHLC), combo (trục phụ), **waterfall, treemap, sunburst, histogram, pareto, box & whisker, funnel**, **bản đồ (map chart)** dùng dữ liệu ranh giới license hợp lệ, có đủ Việt Nam cấp tỉnh (theo đơn vị hành chính hiện hành). Định dạng mọi thành phần (trục, lưới, nhãn, chú giải, bảng dữ liệu, thanh lỗi, đường xu hướng mọi loại kèm phương trình/R², đường cao-thấp, thanh up/down, đường thả), mẫu biểu đồ (lưu/áp dụng), style & màu biểu đồ, trục log/ngày/category, trục phụ, biểu đồ động theo bộ lọc. Đọc/ghi cả chart truyền thống (c:) và chart mở rộng (cx:) theo spec.

---

## 7. BlueSlide — tương đương trình chiếu (prefix `SLIDE-`)

### 7.1. Soạn thảo
- Chế độ xem: Normal, Outline, Slide Sorter, Notes Page, Reading View, Slide Master, Handout Master, Notes Master; sections (thêm/đổi tên/thu gọn/di chuyển).
- Slide Master: sửa master/layout, thêm layout, placeholder mọi loại, giữ (preserve) master, nhiều master, theme & variant (thư viện theme tự thiết kế), kích thước slide (16:9, 4:3, A4, tùy biến) với tùy chọn scale/ensure fit.
- Tái sử dụng slide từ file khác (giữ/không giữ định dạng nguồn), nhân bản, ẩn slide, nhảy đến slide, đánh số slide, ngày giờ, footer.
- Zoom: Summary Zoom, Section Zoom, Slide Zoom (thumbnail tương tác, quay lại).
- Hyperlink & action (chuyển slide, chạy chương trình — chỉ trong phạm vi an toàn, phát âm thanh, hover action).
- Bảng, biểu đồ (6.6), SmartArt (4.4), phương trình (4.4), mô hình 3D (4.3), ink, icons.
- Audio/video: chèn (file, liên kết), trim, fade in/out, bookmark, phát tự động/khi bấm, lặp, ẩn khi không phát, phát qua nhiều slide, poster frame, phụ đề (tệp WebVTT), ghi âm thanh, quay màn hình (desktop), video trực tuyến (liên kết).
- Gợi ý thiết kế (Designer-like, 4.5), photo album, outline import từ BlueDoc.

### 7.2. Hiệu ứng
- **Transition**: cài đủ các họ hiệu ứng phổ biến (nhẹ: cut, fade, push, wipe, split, reveal, random bars, shape, uncover, cover, flash; ấn tượng: fall over, drape, curtains, wind, prestige, fracture, crush, peel off, page curl, airplane, origami, dissolve, checkerboard, blinds, clock, ripple, honeycomb, glitter, vortex, shred, switch, flip, gallery, cube, doors, box, comb, zoom; nội dung động: pan, ferris wheel, conveyor, rotate, window, orbit, fly through) và **Morph** (khớp đối tượng theo tên/thuộc tính, nội suy vị trí/kích thước/xoay/màu/hình dạng/chữ theo ký tự-từ-đối tượng). Tùy chọn hướng, thời lượng, âm thanh, chuyển khi bấm/sau thời gian, áp dụng tất cả. Đọc/ghi đúng phần tử transition (kể cả p14/p15 extension).
- **Animation**: đủ 4 nhóm entrance / emphasis / exit / **motion path** (đường thẳng, cung, vòng, hình dạng, tùy vẽ, sửa điểm, khóa/mở, đảo hướng), tùy chọn hiệu ứng, theo đoạn/từ/chữ, chuỗi theo phần tử biểu đồ/SmartArt, **Animation Pane** (timeline, kéo thả thứ tự, thời gian bắt đầu: khi bấm/cùng/sau, delay, duration, lặp, tua lại, âm thanh, làm mờ sau hiệu ứng), **trigger** (khi bấm vào đối tượng, khi đến bookmark media), Animation Painter, xem trước. Đọc/ghi toàn bộ cấu trúc timing (p:timing) theo spec, phát lại đúng.

### 7.3. Trình chiếu
Từ đầu/từ slide hiện tại, trình chiếu tùy chỉnh (custom show), kiosk/tự chạy/do người xem duyệt, lặp, tập dượt thời gian (rehearse timings), **ghi lại bài thuyết trình** (thu tiếng, camera, bút vẽ, timing → lưu vào file và xuất video), Presenter View (đã có) — bổ sung: zoom vào vùng slide, bút/bút dạ/laser, đen/trắng màn hình, xem mọi slide, đồng hồ đếm, phụ đề trực tiếp (provider dictation + dịch, tùy chọn), điều khiển từ điện thoại (qua BlueDrive/mạng nội bộ), chiếu trên màn hình ngoài (desktop: chọn màn hình), chiếu trực tuyến qua link (web).

### 7.4. Xuất
Video MP4 (chọn độ phân giải, thời gian mặc định, kèm thuyết minh/ink) — dùng WebCodecs (web) và encoder license hợp lệ (desktop/server); GIF động; PDF/handout (1,2,3,4,6,9 slide/trang, có ghi chú); ảnh; gói trình chiếu (đóng gói kèm media & font được phép nhúng).

---

## 8. Kiểm thử độ trung thực với phần mềm tham chiếu

- Ảnh mốc từ phần mềm thương mại phải do **người dùng** cung cấp (họ có bản quyền). Tạo `docs/parity/REFERENCE_GUIDE.md` hướng dẫn người dùng: danh sách file trong `tests/corpus` cần mở, cách xuất PDF/PNG từng trang, nơi đặt (`tests/fidelity/reference/<app>/<file>/page-N.png`), quy ước tên. Ghi một mục vào `NEEDS_HUMAN.md`.
- Trong lúc chưa có ảnh mốc: tiếp tục dùng ảnh mốc tự sinh + kiểm tra số học (vị trí ngắt dòng/ngắt trang so với dữ liệu ngắt trang có sẵn trong file — `w:lastRenderedPageBreak`, giá trị cache của công thức, kích thước cache của chart…) để ước lượng độ lệch. Ghi rõ trong MATRIX nguồn đối chiếu.
- Khi ảnh mốc xuất hiện trong repo (kiểm tra mỗi vòng rà soát định kỳ): chạy so sánh, tạo hạng mục sửa lỗi cho mọi file lệch vượt ngưỡng, ưu tiên P0.
- Mỗi tính năng mới phải có ít nhất 3 file corpus (tự tạo bằng chính BlueOffice + tự viết XML tay theo spec), và test round-trip XML chuẩn hoá + OpenXmlValidator.
- Giá trị kỳ vọng cho hàm bảng tính: lấy từ giá trị cache có trong file corpus do người dùng cung cấp, hoặc từ tính toán độc lập (thư viện toán học tham chiếu license hợp lệ, công thức toán học chuẩn). Không tự bịa giá trị kỳ vọng.

---

## 9. Vòng rà soát cuối (trước khi kết thúc)

1. Đối chiếu từng dòng mục 4–7 với MATRIX; mọi dòng phải có trạng thái và test.
2. Chạy toàn bộ: unit, round-trip, validator, visual, E2E web + desktop, fuzz (tối thiểu 1 giờ mỗi parser), benchmark, kiểm tra rò rỉ bộ nhớ, kiểm tra license dependency.
3. Kiểm tra khả năng tiếp cận của chính giao diện BlueOffice (bàn phím, screen reader, tương phản) theo WCAG 2.2 AA.
4. Kiểm tra bảo mật: mở file độc hại mẫu (zip bomb, XML lồng sâu, link ngoài, macro, OLE), không thực thi gì ngoài ý muốn.
5. Sửa mọi lỗi phát hiện, chạy lại đến xanh.
6. Viết `docs/parity/FINAL_REPORT.md` và cập nhật README, tài liệu người dùng (`docs/user-guide/` tiếng Việt cho từng app).

---

## 10. Bắt đầu ngay

1. Đọc `CLAUDE.md`, `BlueOffice-PROMPT.md`, `docs/PROGRESS.md`.
2. Kiểm tra hiện trạng repo, so với mục 4–7, tạo `docs/parity/BACKLOG.md`, `MATRIX.md` (điền đúng hiện trạng các tính năng đã có), `functions.md`, `NEEDS_HUMAN.md`, `REFERENCE_GUIDE.md`.
3. Thêm vào `CLAUDE.md` một mục ngắn: "Đang ở giai đoạn Parity — làm theo vòng lặp trong BlueOffice-PARITY-PROMPT.md mục 1, không dừng giữa chừng".
4. Commit, push, rồi bắt đầu vòng lặp mục 1.2 với hạng mục P0 đầu tiên. Không dừng lại để báo cáo cho đến khi điều kiện ở mục 1.6 thỏa mãn.

### Gợi ý độ ưu tiên ban đầu
- **P0**: soát chính tả tiếng Việt/Anh, phương trình (xem + sửa), SmartArt (dựng layout + tạo/sửa), đủ loại biểu đồ, hàm bảng tính còn thiếu, hiệu ứng 3D hiển thị đúng, animation/transition đầy đủ + Morph, So sánh/Kết hợp tài liệu, field đầy đủ, content controls, Slicer/Timeline/PivotChart, Solver/Goal Seek/Data Table.
- **P1**: Power Query-like, data model, Flash Fill, Analysis ToolPak, BlueScript + recorder, ghi trình chiếu & xuất video, mô hình 3D, ink, form controls, mẫu hành chính & kiểm tra thể thức, dịch/đọc chính tả/đọc to.
- **P2**: add-in platform, BlueAI, gợi ý thiết kế, trích dẫn/tài liệu tham khảo, chỉ mục, map chart, định dạng file phụ (epub, xps, slk…).
- **P3**: phần còn lại.
