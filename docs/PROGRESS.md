# Tiến độ

Cập nhật 2026-11-08. Theo yêu cầu của chủ dự án, các phase được làm liên tục, **không dừng chờ duyệt**
giữa các phase; ưu tiên bộ ứng dụng văn phòng (BlueDoc/BlueSheet/BlueSlide). Tích hợp BlueDrive giữ ở mức đã làm
(BOHP/WOPI + BlueDrive giả lập), không phát triển thêm cho tới khi có API thật.

## Tổng quan

| Phase | Nội dung | Trạng thái | Kế hoạch / hạn chế |
|---|---|---|---|
| 1 | Nền tảng: workspace, OPC, text, layout, render, PDF/PNG, WASM, PAL, shell UI, CI | ✅ | [`plan/phase-1.md`](plan/phase-1.md) |
| 2 | BlueDoc MVP: `.docx` lossless, layout đầy đủ, `bo-edit`, editor (IME, clipboard, tìm/thay, lưu, PDF, tự lưu) | ✅ | [`plan/phase-2.md`](plan/phase-2.md) |
| 3 | BlueSheet: `.xlsx` lossless, định dạng số, `bo-formula` (276 hàm), lưới canvas, định dạng ô, sort/filter, CF/DV, chart, tìm/thay, in | ✅ | [`plan/phase-3-xlsx.md`](plan/phase-3-xlsx.md), [`plan/phase-3-formula.md`](plan/phase-3-formula.md) |
| 4 | BlueSlide: `.pptx` lossless, 187 preset shape, render, chỉnh sửa (di chuyển/đổi cỡ/xoay/nhóm/căn/smart guide/sửa chữ), các chế độ xem, trình chiếu, presenter | ✅ | [`plan/phase-4.md`](plan/phase-4.md) |
| 5 | `.doc/.xls/.xlsb/.ppt` đọc, ODF r/w, mật khẩu, VBA giữ nguyên, TCVN3/VNI; track changes, comment, TOC/caption/cross-ref, mail merge, SmartArt fallback, bảng tổng hợp (pivot) | ✅ | [`plan/phase-5.md`](plan/phase-5.md) |
| 6 | Server ASP.NET Core (file, version, chia sẻ, audit, OIDC, Yjs relay, worker chuyển đổi), `bo-collab`, ký số PAdES/OOXML, desktop (deep link, nhiều cửa sổ, updater), on-prem Docker | ✅ (cộng tác trực tiếp: văn bản BlueDoc, ô BlueSheet, đối tượng BlueSlide) | [`plan/phase-6.md`](plan/phase-6.md), [`deploy/`](deploy/README.md) |
| 6A | BOHP + WOPI, BlueDrive giả lập, `editor-sdk` (postMessage), chế độ nhúng `/edit`, deep link desktop | ✅ theo mô tả prompt | [`bohp.md`](bohp.md) |
| 7 | BlueForm, BlueNote | 📝 chỉ kế hoạch | [`plan/phase-7.md`](plan/phase-7.md) |
| Parity | Đối chiếu tính năng bộ office thương mại (`BlueOffice-PARITY-PROMPT.md`): xong chính tả Việt/Anh + ngữ pháp, phương trình (font toán STIX Two Math + bảng MATH, căn jc, nhập UnicodeMath/LaTeX, sửa, tuyến tính ↔ chuyên nghiệp), SmartArt (tự dựng từ data model; BlueSlide: tạo/sửa/màu/chuyển hình), biểu đồ (một bộ vẽ chung; 100 %, bubble, radar, stock, of-pie, surface, chartex, thành phần, 3D), hình 3D thật (đùn, bevel, phối cảnh, ánh sáng), BlueDoc: so sánh/kết hợp, field (công thức, định dạng, khoá, mã trường), content control biểu mẫu | 🔄 đang làm | [`parity/BACKLOG.md`](parity/BACKLOG.md), [`parity/MATRIX.md`](parity/MATRIX.md) |

## Kết quả kiểm thử (Windows 11, 2026-11-08, `main`)

| Bộ | Kết quả |
|---|---|
| `cargo test --workspace` | **898** test ✅ (proptest undo/redo doc + slide + track changes, proptest hội tụ cộng tác, round-trip corpus, ~920 dòng bảng công thức) |
| `cargo clippy --workspace --all-targets -D warnings` (pedantic) · `cargo fmt --check` · clippy wasm32 · `cargo deny` | ✅ |
| `pnpm test` (Vitest) | **211** test ✅ — gồm parity WASM ↔ native, 2 engine + 2 bản sao `bo-collab` thật qua relay (doc, sheet, slide) |
| `pnpm lint` · `pnpm typecheck` · `pnpm license-check` | ✅ |
| Playwright E2E (web) | **48** test ✅ — doc (gõ tiếng Việt, định dạng, font/Format Painter, bảng, header, thước kẻ, page setup, mẫu NĐ30, track changes/comment, chú thích sửa trên trang, chèn hàng có theo dõi), bảng tính (SUM, định dạng, sort, pivot, nhóm ngày/bố cục/% tổng), slide (kéo shape, định dạng một phần chữ, copy/paste, ô bảng trên canvas, dán slide sang deck khác) — đều lưu rồi mở lại |
| Playwright E2E cộng tác (server thật) | **9** test ✅ — `BlueOffice.Api` + BlueDrive giả lập + web sau proxy, 2 trình duyệt: gõ/định dạng/caret, lưu thành version (editors 2 người), offline rồi gộp lại, thu hồi quyền giữa chừng, xung đột offline; công thức BlueSheet và kéo shape BlueSlide |
| E2E desktop (Tauri WebDriver, `bluedoc.exe` thật) | **15** test ✅ — mở file từ tham số dòng lệnh, gõ chữ, công thức BlueSheet (engine native), BlueSlide, deep link `bluedoc://` → phiên server → cộng tác với người dùng web |
| `dotnet test` | **155** test ✅ (BOHP với BlueDrive giả lập, 2 người cùng phiên, thu hồi quyền, xung đột offline, WOPI, job chuyển đổi) |
| Fidelity ([`fidelity-report.md`](fidelity-report.md)) | mở + lưu 100 % corpus; OpenXmlValidator (file ra): docx 66/67, xlsx 81/84, pptx 54/54; visual diff 206 trang, 0 vượt ngưỡng so với baseline — phần còn lại là hạn chế đã biết của validator (`tests/fidelity/known-issues.txt`) |

Round-trip lossless (XML chuẩn hoá): `.docx` 52/52, `.xlsx` 54/54, `.pptx` 100 % part (corpus tự sinh, README ghi nguồn).

## Benchmark
| Phép đo | Kết quả | Mục tiêu (§5) |
|---|---|---|
| Layout nguội 300 trang (criterion) | 0,73 s | < 3 s ✅ |
| Gõ phím / Enter / Backspace / đậm ở tài liệu 308 trang (layout tăng dần) | 1,4–2,0 ms native · 2,9–3,9 ms WASM (trước: 46–79 ms) | < 16 ms ✅ |
| Bảng tính 1 triệu ô / 100k công thức: mở + tính | 0,80 s (release; trước 1,96 s) | < 5 s ✅ |
| Sửa 1 ô (~5 000 phụ thuộc) | 5,2 ms (release) | < 100 ms ✅ |
| Sửa 1 ô trong sheet 100k công thức (`LiveCalc`) | ~6 ms (debug), chỉ 6 ô tính lại | < 100 ms ✅ |
| Sửa 1 ô có 10 000 phụ thuộc VLOOKUP / chuỗi 10 000 ô | 26 ms / 27 ms (release; trước 169 / 153 ms) | < 100 ms ✅ |

## Ảnh chụp
- BlueDoc: [`screenshots/phase2-web-editing.png`](screenshots/phase2-web-editing.png)
- BlueSheet: [`screenshots/bluesheet-web.png`](screenshots/bluesheet-web.png)
- BlueSlide: [`screenshots/blueslide-web.png`](screenshots/blueslide-web.png), presenter [`screenshots/blueslide-presenter.png`](screenshots/blueslide-presenter.png)
- Phase 1 web/desktop/CLI: `screenshots/phase1-*.png`

## Hạn chế đã biết (ưu tiên tiếp theo)
Rà soát lại theo code và test ngày 2026-11-08; các mục đã làm (bảng Word cộng tác từng ô, style/comment/chú thích/
master slide, định dạng và chèn-xoá hàng cột BlueSheet, ô địa chỉ server trên desktop, track changes cột/ô/gộp-tách/
header/chú thích/ảnh nổi, chú thích dài tràn trang, biểu đồ pivot, `calculatedItems`, nhóm thủ công, lọc ngày, kéo chart
neo tuyệt đối, dán slide sang deck khác, connector trong nhóm lật) đã bỏ khỏi danh sách.
1. **Cộng tác** (`plan/phase-6.md`): bảng ở thân văn bản, trong ô bảng, header/footer, chú thích và comment đều gộp theo từng ô;
   BlueSheet: mọi vùng (CF/DV, hyperlink, bộ lọc,
   bảng, biểu đồ, vùng in, pivot, freeze pane) theo id hàng/cột; nhiều instance server: xem mục 9.
2. **Hiệu năng**: đổi style mặc định, style/danh sách dùng trong header/footer/chú thích/comment/text box, thiết lập
   trang của section đầu tiên, số trang bắt đầu hay thuộc tính chú thích của section vẫn dàn lại toàn bộ; đổi style,
   numbering khác hoặc khổ giấy/lề/cột/kiểu ngắt của section sau chỉ dàn lại từ chỗ bị ảnh hưởng
   (`bo_edit::ops::record_ops`). NUMPAGES,
   header/footer và chú thích đánh số theo trang đã tăng dần, chỉ dàn lại toàn bộ khi số trang / chiều cao header /
   trang của chú thích đổi.
3. **BlueDoc**: thông báo "tiếp theo" của chú thích tràn trang được ghi lại và chia sẻ khi cộng tác; wrap tight lấy bề
   ngang của đa giác trong dải mỗi dòng (chữ vào được chỗ lõm mở sang hai bên, không vào lỗ kín bên trong hình).
4. **BlueSheet**: ngày gõ thiếu năm (`15 Mar`) cố ý để dạng chữ (kết quả sẽ phụ thuộc ngày hiện tại).
5. **BlueSlide**: PDF vẽ blur/glow bằng ảnh mặt nạ (không vector); hình 3D (`scene3d`/`sp3d`) dựng xấp xỉ 2D
   (`bo-pptx/src/three_d.rs`: mặt trước xoay theo camera, phần đùn, vát cạnh trên, contour, sáng/tối theo hướng
   `lightRig`, điểm sáng cho vật liệu bóng) — góc camera preset là ước lượng (spec chỉ nêu tên preset); không dựng
   lưới 3D thật (bevel dưới, mặt bên cong của bevel, phản xạ theo vật liệu chi tiết).
6. **Desktop**: E2E Tauri WebDriver 15/15 xanh (`pnpm test:e2e:desktop`); bộ test Rust của `bo-desktop` cần GTK trên Linux.
7. **Fidelity**: visual diff đã chạy (`docs/fidelity-visual.md`, baseline tự sinh 205 trang); `bo-cli validate` / `bo-cli fidelity-report`; so với phần mềm gốc cần người dùng đặt ảnh vào `tests/fidelity/reference/`.
8. **Ký số**: đã có PAdES B-T / XAdES-T, kiểm tra chuỗi tới CA gốc cấu hình, thu hồi qua CRL và OCSP (nạp sẵn
   hoặc tải/hỏi trực tuyến theo chứng thư), PAdES B-LT / B-LTA (`bo-cli ltv [--tsa]`: DSS + dấu thời gian
   tài liệu; không ghi VRI vì tuỳ chọn); web kiểm tra thu hồi qua proxy của server (CORS); dấu thời gian tài liệu làm bằng
   chứng tồn tại cho chữ ký không có dấu thời gian riêng; POE lan theo chuỗi gia hạn B-LTA; thuật toán yếu theo thời
   gian (SHA-1, RSA < 2048 trước 2014-01-01, `weakAlgorithm`, mốc cấu hình được `--sunset`); thu hồi xét tại thời
   điểm POE (ADR-0007). Ký và kiểm tra RSA và ECDSA P-256/P-384 (PAdES + OOXML, kiểm chéo `openssl cms -verify`);
   chữ ký OOXML của phần mềm khác kiểm được (C14N/Exclusive C14N, RelationshipTransform, kiểm chéo lxml + OpenSSL).
9. **Server**: chạy được nhiều instance `api` (lease phòng trong Redis, mã 4009, id phiên mang id instance, nginx băm
   theo file — `docker-compose.multi.yml`, `docs/deploy/README.md`); phiên vẫn ở bộ nhớ instance tạo ra nó. Schema bằng EF Core migrations (PostgreSQL + SQLite;
   database cũ tạo bằng `EnsureCreated` được nhận tự động, `docs/deploy/upgrade.md`). Npgsql dùng PostgreSQL License — chấp nhận theo [ADR-0009](adr/0009-npgsql-postgresql-license.md).
10. **Mã hoá**: Agile Encryption đã kiểm chéo hai chiều với một cài đặt độc lập (msoffcrypto-tool, `crates/bo-crypto/tests/interop.rs`).

## Lịch sử
- 2026-11-10 — rà soát định kỳ lần 4 (sau 51 mục): fidelity .docx 98,8 %, .xlsx 98,0 %, .pptx 100 % (lỗi còn lại đều đã biết); làm mới baseline các trang biểu đồ (bộ vẽ dùng định dạng trong file); MATRIX: Chung 40 %, BlueDoc 61 %, BlueSheet 55 %, BlueSlide 62 %; sửa `bo-cli roundtrip` ghi .xlsx/.pptx đúng định dạng (trước ghi thành .docx); BACKLOG thêm CORE-MISC-003/004/005 (clipboard bất đồng bộ, tên lệnh trùng, đối chiếu số mục spec, đối tượng neo ô khi chèn hàng/cột).
- 2026-11-09: giao diện (BlueOffice-UI-PROMPT): `docs/ui/` (DESIGN, glossary, shortcuts, BACKLOG); token thiết kế sinh tự động (brand tạm #1F6FEB, màu nhấn chàm/xanh ngọc/hồng san hô theo `data-app`, thang xám 12 bậc, trạng thái, light/dark/tương phản cao, mật độ, chuyển động), bộ icon theo command id + SOURCES.md, font UI Be Vietnam Pro. Parity: tìm/thay nâng cao (CORE-FIND-001), Flash Fill (SHEET-FLASH-001, một phần), công cụ phân tích thống kê (SHEET-ATP-001, một phần; nhãn kết quả viết lại theo cách diễn đạt riêng), danh sách đa cấp mới + liên kết heading + đặt giá trị (DOC-LIST-001, một phần). Đợt 2: Quản lý kịch bản + báo cáo (SHEET-WHATIF-003), Tự động sửa khi gõ (CORE-AUTOCORR-001, một phần). Đợt 3: công cụ bảng BlueDoc (DOC-TABLE-001, một phần), ENCODEURL/FILTERXML/WEBSERVICE (SHEET-FN-WEB, một phần; 508/515 hàm), bảng ký tự Unicode dùng chung 3 app (CORE-SYMBOL-001). Khung cửa sổ chung (UI-FRAME-001: thanh tiêu đề 40px theo màu nhấn app + thanh truy cập nhanh, thanh trạng thái 24px). Đợt 4: chế độ hiển thị sửa đổi, bong bóng lề, ngăn danh sách (DOC-REVIEW-001, một phần). Ô tìm lệnh Ctrl+Q/Alt+Q (UI-FRAME-003). Kiểu/màu/bố cục nhanh/mẫu biểu đồ (CORE-CHART-007, một phần), viền trang + màu trang + hình mờ + chủ đề (DOC-PAGE-001, một phần). Đợt 5: Solver phi tuyến/tiến hoá + báo cáo (SHEET-SOLVER-002), kiểm tra tài liệu/trợ năng/tương thích (CORE-INSPECT-001, một phần), trường biểu mẫu cũ + chỉ điền biểu mẫu (DOC-FORM-001, một phần), tuỳ chọn dán + lịch sử clipboard (CORE-PASTE-001, một phần). Đợt 6: chính tả theo w:lang + hộp thoại F7 (CORE-SPELL-005), 8 mẫu văn bản hành chính NĐ 30 + kiểm tra thể thức (DOC-TPL-001); một phần: điều khiển biểu mẫu BlueSheet (SHEET-FORMCTL-001), bảo vệ/nhãn độ mật (CORE-SEC-001), chế độ xem BlueDoc (DOC-VIEW-001), drop cap/cột/đánh số dòng/gạch nối/chữ dọc (DOC-LAYOUT-001), xuất video/GIF (SLIDE-EXPORT-001). Sửa: từ chối mật khẩu kiểu cũ của bảo vệ biểu mẫu thay vì chấp nhận mọi mật khẩu. Đợt 7: sắp xếp/lọc nâng cao + Advanced Filter (SHEET-DATA-001); một phần: công cụ dữ liệu + dàn ý (SHEET-DATA-002), sparkline (SHEET-SPARK-001), handout/ảnh/gói trình chiếu (SLIDE-EXPORT-002), dòng chữ ký (CORE-SIGLINE-001). Sửa: chữ gõ sớm khi tài liệu đang mở không còn bị mất; E2E dùng clipboard chạy tuần tự (project `clipboard`). Đợt 8: soát chính tả trong ô BlueSheet và text BlueSlide (CORE-SPELL-006), audio/video trim/fade/bookmark/phụ đề (SLIDE-MEDIA-001); một phần: custom show + công cụ trình chiếu (SLIDE-SHOW-001), thuộc tính ký tự nâng cao + ruby (DOC-FMT-001), reflection/soft edge/text warp (CORE-SHAPE-002). Sửa: ô sửa chữ slide bỏ qua kết quả engine dội về muộn. Đợt 9 (một phần): chế độ xem bảng tính — Page Break Preview, Page Layout, Custom Views, Show Formulas (SHEET-VIEW-001); theo dõi di chuyển + định dạng + khoá mật khẩu (DOC-REVIEW-002); mẫu/biến thể macro + .thmx (CORE-FMT-001); Custom XML + data binding (DOC-CC-002). Đợt 10 (một phần): trộn thư mở rộng (DOC-MERGE-001), checkbox/ảnh trong ô + IMAGE (SHEET-CELL-001), luồng in chung + bảng In (CORE-PRINT-001), ink InkML + thẻ Vẽ BlueSlide (CORE-INK-001). UI: ribbon hiện icon theo command id (UI-ICONS-002). Đợt 8: CF đầy đủ (SHEET-CF-001), tham chiếu nâng cao (DOC-REF-001), bảng (SHEET-TABLE-001); một phần: kiểm tra công thức (SHEET-AUDIT-001), sửa master/layout/theme (SLIDE-MASTER-001), công cụ ảnh (CORE-PIC-001), in bảng tính (SHEET-PRINT-001), các dạng xem BlueSlide (SLIDE-VIEW-001). Đợt 9: định dạng số lịch âm/DBNum/xoay chữ (SHEET-NUMFMT-001); một phần: sửa điểm + ghép hình + connector (CORE-SHAPE-001). Sửa: dạng xem dàn ý giữ dòng đang gõ khi engine trả lại chính thay đổi của nó; kiểm tra lúc biên dịch cho hình dạng lệnh sheet. Agent song song trong worktree (CARGO_TARGET_DIR riêng).
- 2026-11-08 (phiên cuối): hợp nhất nhánh `wip/misc-formats` còn dở (comment luồng BlueSheet [MS-XLSX] threadedComments,
  comment hiện đại BlueSlide [MS-PPTX] p188, vẽ SmartArt fallback trong BlueSheet/BlueSlide, chuyển TCVN3/VNI khi mở
  .docx/.xlsx, nhóm corpus slide-notes/theme/smartart ≥ 3 file) — giữ thanh cảnh báo macro, mật khẩu, bảo vệ sheet của
  `main`. `bo-cli validate` (OPC + OpenXmlValidator) và `bo-cli fidelity-report` (prompt §4). Cộng tác: bảng trong
  header/footer/chú thích/comment gộp theo từng ô. BlueDoc: giữ thông báo tiếp trang của chú thích khi ghi lại.
  Sửa: hàng chèn vào bảng có ô chờ xoá (track changes) không còn rỗng; Hộp tên BlueSheet chuyển tiêu điểm ngay (ký tự
  gõ liền sau Enter không bị mất); locator E2E "Xuất PDF". Baseline visual diff làm mới (bản render trước merge giống
  hệt từng pixel — lệch do các thay đổi layout trước đó). E2E cộng tác 9/9 (kể cả `collab-embed` export-pdf trước đây đỏ).
  Tiếp theo trong phiên: nhóm corpus `binary-doc` đủ 3 file; server chạy nhiều instance (lease phòng Redis, 4009,
  `docker-compose.multi.yml`); hình 3D slide (mặt xoay, đùn, vát, contour, ánh sáng); ký số: thuật toán yếu theo thời gian
  (`--sunset`), POE theo chuỗi gia hạn B-LTA, kiểm tra ECDSA P-256/P-384.
- 2026-11-08 (cuối phiên): Cộng tác — bảng lồng trong ô bảng BlueDoc gộp theo từng ô (`rebuild_nested`); BlueSlide
  master/layout/theme khoá theo id ổn định (part tạo trong phiên có khoá theo client, tham chiếu `lk`/`@m`/`@t`/`@l`) nên
  hai người dán slide từ deck khác cùng lúc giữ cả hai thiết kế; tên part mới (ảnh, media…) của replica cộng tác có hậu
  tố chữ theo client (`build::salt_names`) nên ảnh dán đồng thời không đè nhau. E2E cộng tác: 8/9 xanh; test chèn hàng
  sửa để mở nhóm ribbon bị thu gọn. **Còn đỏ**: `collab-embed.spec.ts` "export-pdf" — tên file tải về là `download`
  thay vì `Nhúng SDK.pdf` (có thể do bản Chromium trong container khác bản Playwright 1.64 cần; chưa xác minh trên CI).
  Việc tiếp theo: các mục 1, 5, 8, 9 của "Hạn chế đã biết".
- 2026-11-08 (sau cùng): BlueSheet nhận ngày gõ có tên tháng (`15 Mar 2024`, `March 15, 2024`, `15 tháng 3 năm 2024`,
  `15/Thg3/2024`, `tháng 3/2024`) → số serial + định dạng dựng sẵn 15/17/22. Ký số: kiểm tra thu hồi bằng CRL
  (`bo-cli verify --crl`). Gói zip ghi ra giống hệt nhau trên Windows và Linux (corpus slide hết lệch). Thanh công
  thức hiện ngày cho mọi định dạng ngày. Wrap tight ôm theo cạnh đa giác `wp:wrapPolygon` (giữ đủ đỉnh khi lưu).
  Rà soát lại "Hạn chế đã biết". Sửa lỗi cache layout không theo định nghĩa style (đổi style/undo/mở tài liệu khác hiện
  cỡ chữ cũ); đổi style / numbering / thiết lập trang của section chỉ dàn lại phần bị ảnh hưởng. Ký số: OCSP
  (`bo-cli verify --ocsp FILE | --ocsp-fetch`), kiểm chéo với `openssl ocsp`; tự tải CRL (`--crl-fetch`); PAdES B-LT
  (`bo-cli ltv`, kiểm chéo với pyHanko); đọc được CRL v1; PAdES B-LTA (`ltv --tsa`). Server: EF Core migrations
  thay `EnsureCreated`; `bo-desktop` build/test được trên Linux (gói GTK/WebKitGTK).
  Cộng tác: comment/chú thích thêm đồng thời không còn trùng id (trước đó một comment bị mất); chèn hàng + cột đồng
  thời không còn để hàng mới thiếu ô; BlueSheet: vùng CF/DV thêm trong lúc người khác chèn hàng dịch đúng.
  BlueSheet: chèn/xoá hàng cột giờ dịch cả biểu đồ (ô neo + tham chiếu series; trước đó biểu đồ trỏ sai dữ liệu), ghi
  lại vào file chỉ sửa `<c:f>` và `<xdr:row>/<xdr:col>` (lossless, OpenXmlValidator hợp lệ). Tương tự pivot: vị trí
  báo cáo và nguồn cache dịch theo (trước đó refresh ghi báo cáo đè lên vị trí cũ); freeze pane: chèn/xoá hàng cột
  trong vùng đóng băng đổi số hàng/cột đóng băng. Ký số trên web: CRL/OCSP qua `POST /api/revocation/fetch` (chống
  SSRF) — trình duyệt không gọi thẳng CA được.
- 2026-11-08 (khuya): pivot đầy đủ (tổng phụ, 3 bố cục, nhóm ngày/số, trường tính toán, 14 kiểu "hiển thị giá trị",
  lọc/top-N, kiểu); E2E cộng tác với server thật + desktop cộng tác qua deep link (sửa lỗi caret người vào sau nhảy cuối
  tài liệu); chú thích sửa trên trang, header/footer liên kết section trước, track changes bảng/ảnh/page setup; ô bảng
  slide trên canvas, connector giữ xoay/lật, dán slide sang deck khác, blur/glow thật trong PDF.
- 2026-11-05: Phase 1 hoàn thành; đổi tên sản phẩm BlueWrite → BlueDoc (ADR-0006).
- 2026-11-07: Phase 2–6A phần cốt lõi trên các nhánh song song, hợp nhất vào `main`; `bo-sign`, `bo-collab`,
  `editor-sdk`, `bo-validate`, fidelity CI.
- 2026-11-08 (tối): layout tăng dần (< 16 ms), công thức 10k phụ thuộc 26 ms, pivot, UI BlueDoc còn thiếu (font, Format
  Painter, thước kẻ, page setup, header/footer, bảng, mẫu NĐ30), BlueSlide (rich text, clipboard, connector, bảng,
  màn hình khán giả), visual diff, E2E desktop.
- 2026-11-08: BlueSheet/BlueSlide UI đầy đủ, chỉnh sửa slide, tính năng tài liệu Phase 5, chế độ nhúng, cộng tác trực tiếp
  doc/sheet/slide; build release desktop `bluedoc.exe` (33,7 MB) chạy thử mở `.docx` qua tham số dòng lệnh.
