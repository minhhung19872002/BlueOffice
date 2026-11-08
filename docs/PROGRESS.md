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

## Kết quả kiểm thử (Windows 11, 2026-11-08, `main`)

| Bộ | Kết quả |
|---|---|
| `cargo test --workspace` | **658** test ✅ (proptest undo/redo doc + slide + track changes, proptest hội tụ cộng tác, round-trip corpus, ~920 dòng bảng công thức) |
| `cargo clippy --workspace --all-targets -D warnings` (pedantic) · `cargo fmt --check` · clippy wasm32 · `cargo deny` | ✅ |
| `pnpm test` (Vitest) | **109** test ✅ — gồm parity WASM ↔ native, 2 engine + 2 bản sao `bo-collab` thật qua relay (doc, sheet, slide) |
| `pnpm lint` · `pnpm typecheck` · `pnpm license-check` | ✅ |
| Playwright E2E (web) | **23** test ✅ — doc (gõ tiếng Việt, định dạng, font/Format Painter, bảng, header, thước kẻ, page setup, mẫu NĐ30, track changes/comment, chú thích sửa trên trang, chèn hàng có theo dõi), bảng tính (SUM, định dạng, sort, pivot, nhóm ngày/bố cục/% tổng), slide (kéo shape, định dạng một phần chữ, copy/paste, ô bảng trên canvas, dán slide sang deck khác) — đều lưu rồi mở lại |
| Playwright E2E cộng tác (server thật) | **4** test ✅ — `BlueOffice.Api` + BlueDrive giả lập + web sau proxy, 2 trình duyệt: gõ/định dạng/caret, lưu thành version (editors 2 người), offline rồi gộp lại, thu hồi quyền giữa chừng, xung đột offline; công thức BlueSheet và kéo shape BlueSlide |
| E2E desktop (Tauri WebDriver, `bluedoc.exe` thật) | **5** test ✅ (5 lần chạy liên tiếp) — mở file từ tham số dòng lệnh, gõ chữ, công thức BlueSheet (engine native), BlueSlide, deep link `bluedoc://` → phiên server → cộng tác với người dùng web |
| `dotnet test` | **108** test ✅ (BOHP với BlueDrive giả lập, 2 người cùng phiên, thu hồi quyền, xung đột offline, WOPI, job chuyển đổi) |
| Fidelity ([`fidelity-report.md`](fidelity-report.md)) | mở + lưu 100 % corpus; OpenXmlValidator: docx 54/55, xlsx 57/60, pptx 35/35; visual diff 0 trang vượt ngưỡng so với baseline — phần còn lại là hạn chế đã biết của validator (`tests/fidelity/known-issues.txt`) |

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
   bảng, biểu đồ, vùng in, pivot, freeze pane) theo id hàng/cột; server một instance (phòng **và** phiên trong bộ nhớ;
   thiết kế nhiều instance trong `plan/phase-6.md`).
2. **Hiệu năng**: đổi style mặc định, style/danh sách dùng trong header/footer/chú thích/comment/text box, thiết lập
   trang của section đầu tiên, số trang bắt đầu hay thuộc tính chú thích của section vẫn dàn lại toàn bộ; đổi style,
   numbering khác hoặc khổ giấy/lề/cột/kiểu ngắt của section sau chỉ dàn lại từ chỗ bị ảnh hưởng
   (`bo_edit::ops::record_ops`). NUMPAGES,
   header/footer và chú thích đánh số theo trang đã tăng dần, chỉ dàn lại toàn bộ khi số trang / chiều cao header /
   trang của chú thích đổi.
3. **BlueDoc**: thông báo "tiếp theo" của chú thích tràn trang được ghi lại khi viết lại phần chú thích (không chia sẻ khi cộng tác); wrap tight lấy bề ngang của đa
   giác trong dải mỗi dòng (đa giác lõm được coi như bao lồi theo chiều ngang).
4. **BlueSheet**: ngày gõ thiếu năm (`15 Mar`) cố ý để dạng chữ (kết quả sẽ phụ thuộc ngày hiện tại).
5. **BlueSlide**: PDF vẽ blur/glow bằng ảnh mặt nạ (không vector); chưa dựng hình 3D (`scene3d`/`sp3d`).
6. **Desktop**: E2E Tauri WebDriver 15 test (`pnpm test:e2e:desktop`); bộ test Rust của `bo-desktop` cần GTK trên Linux.
7. **Fidelity**: visual diff đã chạy (`docs/fidelity-visual.md`, baseline tự sinh 134 trang); so với phần mềm gốc cần người dùng đặt ảnh vào `tests/fidelity/reference/`.
8. **Ký số**: đã có PAdES B-T / XAdES-T, kiểm tra chuỗi tới CA gốc cấu hình, thu hồi qua CRL và OCSP (nạp sẵn
   hoặc tải/hỏi trực tuyến theo chứng thư), PAdES B-LT / B-LTA (`bo-cli ltv [--tsa]`: DSS + dấu thời gian
   tài liệu; không ghi VRI vì tuỳ chọn); web kiểm tra thu hồi qua proxy của server (CORS); dấu thời gian tài liệu làm bằng
   chứng tồn tại cho chữ ký không có dấu thời gian riêng; các quy tắc POE còn lại của ETSI EN 319 102-1 (thuật toán
   yếu theo thời gian, POE cho dữ liệu thu hồi) chưa làm (ADR-0007).
9. **Server**: một instance `api` (phòng cộng tác trong bộ nhớ). Schema bằng EF Core migrations (PostgreSQL + SQLite;
   database cũ tạo bằng `EnsureCreated` được nhận tự động, `docs/deploy/upgrade.md`). Npgsql dùng PostgreSQL License — chấp nhận theo [ADR-0009](adr/0009-npgsql-postgresql-license.md).
10. **Mã hoá**: Agile Encryption đã kiểm chéo hai chiều với một cài đặt độc lập (msoffcrypto-tool, `crates/bo-crypto/tests/interop.rs`).

## Lịch sử
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
