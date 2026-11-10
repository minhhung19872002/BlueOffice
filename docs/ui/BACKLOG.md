# UI BACKLOG

Nguồn: [`BlueOffice-UI-PROMPT.md`](../../BlueOffice-UI-PROMPT.md). Trạng thái: `[ ]` chưa làm · `[~]` đang làm · `[x]` xong (có
test) · `[p]` xong một phần (ghi phần thiếu) · `[!]` chặn (ghi `docs/parity/NEEDS_HUMAN.md`). Ưu tiên P0 → P2. Thứ tự làm theo
§13: tokens → icon → khung → registry & ribbon → ribbon từng app → Tệp → hộp thoại & ngăn → KeyTips & a11y → Home → hoàn thiện
icon. Làm xen kẽ với `docs/parity/BACKLOG.md`.

## Tokens & theme (§3)

- [x] **UI-TOKENS-001** (P0) Bộ token `packages/ui/src/tokens` (CSS `--bo-*` + `tokens.ts`): brand `#1F6FEB` (tạm), accent theo
  app qua `[data-app=doc|sheet|slide]`, thang xám 12 bậc, trạng thái 3 bậc, cỡ chữ, khoảng cách 4px, bo góc 4/8, bóng 3 mức,
  thời lượng chuyển động. *Xong khi:* test so khớp token CSS ↔ TS; test tương phản WCAG AA cho chữ trên nền/nhấn ở mọi theme. **Đã có:** `packages/ui/src/tokens/tokens.ts` → `tokens.css` (sinh tự động, test đồng bộ), `theme.css` import; light/dark/hc (hc dùng cho UI-TOKENS-002), `[data-app]` → `--bo-app-accent*` + `--bo-accent`, `[data-density]`, reduced motion; test tương phản WCAG (`test/tokens.test.ts`).
- [x] **UI-TOKENS-002** (P0) Theme high-contrast (`[data-theme=hc]` + `forced-colors`), lựa chọn theme Light/Dark/Tương phản
  cao/Theo hệ thống. *Xong khi:* test tương phản ≥ 7:1 cho chữ chính; E2E đổi theme. **Đã có:** `ThemeSetting` thêm `hc`; `resolveTheme` (auto theo `forced-colors: active` / `prefers-contrast: more` trước rồi mới tới dark); lệnh Xem › Sáng / Tối / Tương phản cao xoay vòng; `@media (forced-colors: active)` giữ viền focus Highlight; SDK `EditorTheme` nhận `hc`; test `theme.test.ts`, test tương phản hc ≥ 7:1, E2E open-sample.
- [x] **UI-TOKENS-003** (P1) Mật độ Thường/Gọn/Cảm ứng (`[data-density]`), nhớ theo người dùng. *Xong khi:* test kích thước nút
  ribbon theo mật độ; vùng chạm ≥ 40px ở Cảm ứng. **Đã có:** `packages/ui/src/density.ts` (`setDensity`/`useDensity`, nhớ `localStorage` có try/catch, đặt `data-density` trên `<html>` nên áp cho cả 3 app), ribbon dùng `--bo-control-h`/`--bo-ribbon-large-h`, nút nhỏ ≥ 40×40 ở Cảm ứng; lệnh `view.density` (BlueDoc › Xem); chọn trong Tùy chọn sẽ có ở UI-FILE-006. Test `density.test.ts`, editor test, E2E `ui-density.spec.ts`.
- [x] **UI-TOKENS-004** (P1) Font UI Be Vietnam Pro / Inter (OFL) đóng gói, fallback hệ thống. *Xong khi:* ghi
  THIRD_PARTY_NOTICES; test font được nạp. **Đã có:** Be Vietnam Pro 400/500/600/700 (google/fonts, OFL) trong `packages/ui/fonts/` + `@font-face` (`font-display: swap`) trong `theme.css`; font UI tách khỏi font tài liệu (không đụng FaceId); test `fonts.test.ts`.
- [x] **UI-TOKENS-005** (P1) Chuyển động 120–200ms, tắt khi `prefers-reduced-motion`. *Xong khi:* test CSS media query. **Đã có:** tab/nút ribbon chuyển nền theo `--bo-duration-fast`, hộp thoại fade `--bo-duration-normal`, fade slide `--bo-duration-slow`; token về 0ms dưới `prefers-reduced-motion`; test token + E2E `ui-motion.spec.ts` (emulateMedia reducedMotion).

## Icon (§4)

- [x] **UI-ICONS-001** (P0) Hạ tầng icon `packages/ui/icons`: component `<Icon name>` tree-shakable theo command id, cỡ
  16/20/24/32, hai tông (nét chữ + điểm nhấn app), bản đơn sắc cho high-contrast, `SOURCES.md`. *Xong khi:* test mọi icon có
  nguồn trong SOURCES.md; test viewBox/kích thước. **Đã có:** `packages/ui/src/icons` — `<Icon name size label>` (16/20/24/32, nét 1.5, trang trí ⇒ `aria-hidden`), 271 id lệnh gom từ ribbon 3 app (Lucide ISC, chọn một icon duy nhất cho mỗi id), icon tự vẽ `custom.tsx` (dấu app nháp, phân cách nghìn, ₫) có phần nhấn `.bo-icon-accent` theo `--bo-app-accent`, đơn sắc ở hc; `packages/ui/icons/SOURCES.md` sinh tự động; test `test/icons.test.tsx`.
- [x] **UI-ICONS-002** (P0) Icon chung + định dạng chữ + đoạn văn (danh mục §4). *Xong khi:* có icon cho mọi command id của
  nhóm; test không thiếu icon trên ribbon Trang đầu. **Đã có:** `Ribbon` lấy icon theo command id từ bộ icon (bộ icon thắng icon riêng của app; id ngoài bộ giữ icon riêng) — 24px nút lớn, 20px nút nhỏ và chế độ gọn; nút nhỏ có icon từ bộ vẫn hiện nhãn; ở mức gọn các nút lớn xếp thành lưới hai hàng ô vuông icon (nhóm Tệp của BlueSheet 371 → 189px ở 1280px, các nhóm Số/Căn lề không còn bị thu lại); nhãn ẩn chuyển vào tooltip; thêm 10 id còn thiếu (chủ đề, mẫu, xuất, bảo vệ, kết hợp ký tự, phông, hướng dẫn phiên âm); test `ribbon.test.tsx`, E2E `ribbon-icons.spec.ts` (thẻ Trang đầu cả 3 app không còn nút thiếu icon).
- [ ] **UI-ICONS-003** (P1) Icon chèn, bảng tính, trình chiếu. *Xong khi:* test phủ icon cho ribbon 3 app.
- [ ] **UI-ICONS-004** (P1) Bản nháp logo BlueOffice + 3 icon app trong `assets/brand/drafts/` (cùng khung, khác ký hiệu, không
  kiểu chữ cái lớn chồng tờ giấy). *Xong khi:* SVG nháp + mục NEEDS_HUMAN duyệt bản chính thức.

## Khung cửa sổ (§5)

- [x] **UI-FRAME-001** (P0) Component `AppFrame` dùng chung: thanh tiêu đề 40px (icon app, truy cập nhanh, tên tài liệu + trạng
  thái lưu, khu phải: cộng tác, Bình luận, Chia sẻ, tài khoản), hàng tab, thân ribbon, vùng làm việc, thanh trạng thái 24px.
  *Xong khi:* 3 app dùng chung khung; test unit vai trò ARIA landmark; E2E đo chiều cao. **Đã có:** `TitleBar` (`packages/ui`, `<header>` 40px trên dải màu nhấn `--bo-app-accent-strong`: dấu app, thanh truy cập nhanh Lưu/Hoàn tác/Làm lại từ registry, tên tài liệu + trạng thái lưu, chỗ cho ô tìm lệnh và nút bên phải), hàng thẻ ribbon nối cùng dải màu nhấn, icon ribbon theo màu nhấn app, `StatusBar` 24px nền `--bo-surface-2`; dùng ở cả BlueDoc/BlueSheet/BlueSlide (ẩn khi trình chiếu); test `titlebar.test.tsx`, E2E `ui-frame.spec.ts` (chiều cao, màu nhấn từng app, hoàn tác từ thanh truy cập nhanh). Ô tìm lệnh: UI-FRAME-003; avatar cộng tác/Chia sẻ/tài khoản trong khung: theo từng app khi có.
- [ ] **UI-FRAME-002** (P1) Thanh truy cập nhanh tùy biến (thêm/bớt lệnh, nhớ theo người dùng). *Xong khi:* test thêm/bớt.
- [x] **UI-FRAME-003** (P0) Ô tìm lệnh Ctrl+Q / Alt+Q: tìm trong registry (nhãn, từ khóa, không dấu), chạy lệnh, gợi ý tìm
  trong tài liệu. *Xong khi:* unit test xếp hạng; E2E định dạng chữ bằng ô tìm lệnh. **Đã có:** `CommandSearch` + `searchRank` (`packages/ui`): lệnh lấy từ ribbon (một lần mỗi id, bỏ control tuỳ biến), xếp hạng đầu nhãn > đầu từ > chứa > mọi từ trong nhãn/đường dẫn/id, bỏ dấu tiếng Việt (đ → d), lệnh bật trước lệnh tắt; Ctrl+Q / Alt+Q và ô trên thanh tiêu đề ở cả 3 app; ↑/↓/Enter/Esc, ARIA combobox/listbox; test `command-search.test.tsx`, E2E `ui-frame.spec.ts` (Alt+Q → "dam" → Enter in đậm; mở từ thanh tiêu đề BlueSheet). **CORE-UI-003:** trước khi gõ hiện "Dùng gần đây" (lệnh đã chạy từ ô tìm, nhớ theo trình duyệt), sau kết quả là "Lệnh liên quan" (lệnh cùng nhóm ribbon với kết quả đầu; `relatedCommands`). Gợi ý tìm trong tài liệu/trợ giúp: chưa có.
- [ ] **UI-FRAME-004** (P1) Thanh trạng thái theo app (trang/từ, ngôn ngữ, ô chọn + thống kê, chế độ xem, thanh trượt zoom),
  chuột phải chọn mục hiển thị. *Xong khi:* test bật/tắt mục.
- [ ] **UI-FRAME-005** (P1) Ngăn tác vụ phải dock/resize, nhiều ngăn xếp tab. *Xong khi:* test resize + đóng/mở.
- [ ] **UI-FRAME-006** (P2) Desktop: thanh tiêu đề tùy biến tích hợp nút cửa sổ (Tauri `decorations: false`). *Xong khi:* E2E
  desktop thu nhỏ/phóng to.

## Command registry & ribbon engine (§6, §12)

- [ ] **UI-CMD-001** (P0) Mở rộng registry: nhãn ngắn/dài, mô tả, icon, phím tắt, KeyTip, kích thước mặc định, ưu tiên co giãn,
  từ khóa tìm kiếm; dùng chung cho ribbon, menu chuột phải, phím tắt, ô tìm lệnh, KeyTips. *Xong khi:* test mỗi lệnh có nhãn
  vi+en và mô tả.
- [ ] **UI-RIBBON-001** (P0) Ribbon khai báo dữ liệu `apps/web/src/<app>/ribbon.config.ts`; trạng thái enabled/checked/value từ
  selection. *Xong khi:* không còn JSX từng nút; test cấu hình hợp lệ (id tồn tại trong registry).
- [ ] **UI-RIBBON-002** (P0) Co giãn theo độ rộng (lớn → nhỏ → chỉ icon → thu nhóm vào nút thả xuống) với test 1920/1440/1280/
  1024/768. *Xong khi:* unit test mốc độ rộng + visual.
- [~] **UI-RIBBON-003** (P0) Chế độ ribbon Đầy đủ / Đơn giản (một hàng, lệnh ít dùng vào "…") / Tự ẩn; Ctrl+F1; nhớ lựa chọn.
  *Xong khi:* E2E đổi chế độ ribbon. **Đã có (CORE-UI-001):** `ribbonMode.ts` (`useRibbonPrefs`, nhớ `localStorage` `bo.ribbon`
  có try/catch, dùng chung 3 app); nút "Ribbon đơn giản" + Thu gọn/Mở rộng ở cuối hàng tab; Đơn giản = một hàng ~40px, mỗi nhóm
  giữ lệnh gắn `simplified` (mặc định 3 lệnh đầu) + "…" chứa phần còn lại, vẫn co giãn bằng `planRibbonLayout`; Ctrl+F1 / nhấp đúp
  tab thu gọn chỉ còn hàng tab, nhấp tab hiện lệnh nổi trên tài liệu (đóng khi chạy lệnh, Esc, nhấp ngoài). Test
  `ribbonContext.test.tsx`, E2E `ribbon-context.spec.ts`. Chưa có: chế độ Tự ẩn cả hàng tab; chọn lệnh `simplified` theo tần suất.
- [x] **UI-RIBBON-004** (P0) Tab ngữ cảnh (nhãn nhóm "Công cụ bảng"…, cùng màu nhấn) hiện theo selection. *Xong khi:* E2E chọn
  bảng hiện tab Thiết kế bảng/Bố cục bảng. **Đã có (CORE-UI-001):** `RibbonTab.context`
  (`table` · `picture` · `chart` · `shape` · `headerFooter` · `smartArt` · `equation` · `ink` · `object` · `media` · `sparkline`,
  `ribbonContext.ts`): tab ngữ cảnh xếp sau tab thường, theo từng nhóm có nhãn "Công cụ …" (`ribbon.context.*`) và dải màu nhấn
  app; tự kích hoạt một lần khi ngữ cảnh xuất hiện (`autoActivate`; ngữ cảnh theo con trỏ — ô bảng, hình đang gõ chữ, sparkline —
  chỉ hiện), ẩn khi bỏ chọn và quay về tab thường trước đó; cùng một `tablist` nên ←/→/Home/End tới được. BlueDoc: Bảng, Định dạng
  ảnh, SmartArt, Đầu & chân trang (mới, lệnh `hf.close`); BlueSheet: Thiết kế bảng, Sparkline, SmartArt, Thiết kế biểu đồ;
  BlueSlide: Hình, SmartArt, Biểu đồ, Bảng, Định dạng ảnh, Media. Test `ribbonContext.test.tsx` + test từng editor, E2E
  `ribbon-context.spec.ts`. Tab phương trình / mực / đối tượng: khai báo được, chưa editor nào suy ra ngữ cảnh đó.
- [ ] **UI-RIBBON-005** (P1) Nút tách, gallery có xem trước trực tiếp, nút mở hộp thoại nhóm ↘, tooltip giàu nội dung. *Xong
  khi:* test hover áp dụng tạm rồi hoàn tác.
- [ ] **UI-RIBBON-DOC-001** (P0) Ribbon BlueDoc đủ tab §6.1 (Tệp, Trang đầu, Chèn, Vẽ, Thiết kế, Bố cục, Tham chiếu, Trộn thư,
  Xem lại, Xem, Trợ giúp + tab ngữ cảnh); lệnh chưa có engine thì ẩn. *Xong khi:* E2E định dạng chữ bằng ribbon và phím tắt;
  chèn bảng.
- [ ] **UI-RIBBON-SHEET-001** (P0) Ribbon BlueSheet §6.2 + thanh công thức (Hộp tên có danh sách tên, hủy/xác nhận/chèn hàm, ô
  nhiều dòng, gợi ý cú pháp) + thanh trang tính. *Xong khi:* E2E.
- [ ] **UI-RIBBON-SLIDE-001** (P0) Ribbon BlueSlide §6.3 + bố cục vùng làm việc (cột thu nhỏ có phần, ngăn ghi chú kéo giãn).
  *Xong khi:* E2E.

## Màn hình Tệp (§7)

- [p] **UI-FILE-001** (P0) Màn hình Tệp toàn trang: cột trái (Quay lại, Trang chủ, Mới, Mở, Thông tin, Lưu, Lưu thành, Lịch sử
  phiên bản, In, Chia sẻ, Xuất, Đóng, Tài khoản, Phản hồi, Tùy chọn), Esc quay lại. *Xong khi:* E2E mở màn hình Tệp. **Đã có
  (CORE-UI-002):** `Backstage` (`packages/ui/src/Backstage.tsx`, dialog toàn cửa sổ, cột điều hướng phím mũi tên/Home/End, Esc
  và Quay lại đóng) + `FileBackstage` (`packages/editor`) dùng chung cho 3 app, nút Tệp đầu dải tab (`Ribbon.fileMenu`; tab Tệp
  cũ của BlueDoc chuyển vào đây, Tìm lệnh vẫn thấy); Mới, Mở, Thông tin, Lưu, Lưu thành, In, Chia sẻ, Xuất, Đóng (khi host có),
  Tùy chọn; lệnh trang chạy sau khi đóng màn hình. Test `ui/test/backstage.test.tsx`, `editor/test/file-backstage.test.tsx`, E2E
  `file-backstage.spec.ts`. *Thiếu:* Trang chủ, Tài khoản, Phản hồi; Lịch sử phiên bản là lệnh trong Thông tin.
- [p] **UI-FILE-002** (P1) Trang chủ/Mới: mẫu nổi bật (mẫu hành chính VN tự soạn), tìm mẫu, gần đây/ghim/được chia sẻ. **Đã có:**
  Mới (trống, mở hộp mẫu DOC-TPL-001); Mở: đã ghim + gần đây (ghim/bỏ ghim lưu theo người dùng qua `shell.getSetting`/
  `setSetting`, web: `localStorage`), mở theo đường dẫn khi nền tảng có `openPath`, duyệt máy này. *Thiếu:* lưới mẫu nổi bật ngay
  trên trang, tìm mẫu, được chia sẻ / BlueDrive.
- [x] **UI-FILE-003** (P1) Thông tin: thuộc tính, bảo vệ, kiểm tra tài liệu, phiên bản. **Đã có:** thuộc tính sửa được (tiêu đề,
  chủ đề, tác giả, từ khóa, ghi chú, thể loại, trạng thái, đơn vị, người quản lý) + chỉ đọc (người lưu, ngày tạo/sửa, số lần sửa)
  qua lệnh `docProperties`/`setDocProperties` (`bo_engine::props`, core.xml/app.xml, docx/xlsx/pptx, test round-trip
  `crates/bo-engine/tests/props.rs`); nút Bảo vệ tệp, Lưu kèm mật khẩu, Kiểm tra tài liệu, Lịch sử phiên bản, Ký số. E2E sửa tiêu
  đề → lưu → mở lại.
- [p] **UI-FILE-004** (P1) In: thiết lập bên trái + xem trước có điều hướng/zoom bên phải. **Đã có:** trang In mở bảng In
  (CORE-PRINT-001, thiết lập + xem trước) và In ngay. *Thiếu:* bảng In nằm ngay trong trang (hiện là hộp thoại).
- [p] **UI-FILE-005** (P1) Xuất: PDF/PDF-A, định dạng khác, video (BlueSlide). **Đã có:** trang Xuất (PDF, PDF/A…, PDF có ký
  số, PNG, video/ảnh động, tài liệu phát tay, CSV, tệp chủ đề — lệnh có sẵn của từng app); Lưu thành liệt kê mọi kiểu tài liệu +
  định dạng khác. *Thiếu:* chọn thư mục/đích lưu trong trang.
- [p] **UI-FILE-006** (P1) Tùy chọn: hộp thoại danh mục (Chung, Hiển thị, Soát lỗi, Lưu, Ngôn ngữ, Khả năng tiếp cận, Nâng cao,
  Tùy biến ribbon, Thanh truy cập nhanh, Bổ trợ, Bảo mật). **Đã có:** trang Tùy chọn của màn hình Tệp gom các thiết lập đang có
  (ngôn ngữ giao diện, sáng/tối/tương phản cao, mật độ — app không có lệnh `view.density` thì chọn ngay trong trang, tự động sửa,
  soát chính tả khi gõ, ngôn ngữ soát lỗi, kiểm tra cập nhật). *Thiếu:* hộp thoại danh mục đầy đủ.

## Thành phần tương tác (§8)

- [ ] **UI-COMP-001** (P0) Bảng chọn màu (theme 10×6, chuẩn, gần đây, HEX/RGB/HSL, hút màu, Tự động/Không màu).
- [ ] **UI-COMP-002** (P0) Bộ chọn bảng (lưới rê chuột, Chèn bảng…, chuyển văn bản thành bảng).
- [ ] **UI-COMP-003** (P1) Bộ chọn font (tên bằng chính font, font chủ đề, gần đây, tìm, cảnh báo font thay thế).
- [x] **UI-COMP-004** (P1) Menu chuột phải từ registry + thanh định dạng nổi. **Đã có (CORE-UI-003):** `ContextMenu`
  (`packages/ui`, `role="menu"`, ↑/↓/Home/End bỏ qua lệnh tắt, Enter/Space chạy, Esc/Tab/nhấp ngoài đóng; phím hoạt động cả khi
  tiêu điểm ở ô nhập của editor) + bảng menu theo ngữ cảnh `contextMenus.ts` (`DOC_MENUS` văn bản/bảng/ảnh/hình/đầu-chân
  trang/bình luận/chính tả, `SHEET_MENUS` ô/tiêu đề hàng/tiêu đề cột/thẻ trang tính/biểu đồ/ảnh trong ô, `SLIDE_MENUS`
  hình/chữ đang sửa/ảnh/bảng/biểu đồ/thu nhỏ trang chiếu/vùng trống); mục là lệnh registry lấy nhãn, trạng thái bật, phím tắt
  từ ribbon (`ribbonMenuLookup`, lệnh không có trên ribbon qua `registryMenuLookup`). Chuột phải chọn đối tượng dưới con trỏ
  nếu nó chưa nằm trong vùng chọn; Shift+F10 / phím menu mở ở con trỏ soạn thảo / vùng chọn. BlueDoc: gợi ý chính tả gộp vào
  đầu menu (`spell-menu`), bình luận khi chuột phải vào bóng bình luận. BlueSheet: thẻ trang tính có "Màu tab…" (bảng màu cũ).
  Thanh định dạng nổi `MiniToolbar` khi thả chuột sau khi chọn chữ (BlueDoc: phông, cỡ, tăng/giảm, đậm/nghiêng/gạch chân, màu
  chữ, tô sáng, dấu đầu dòng, đánh số, chổi định dạng, Kiểu; BlueSlide khi đang sửa chữ trong hình), mờ dần theo khoảng cách
  con trỏ, đóng khi gõ / nhấp nơi khác. Test `ui/test/contextMenu.test.tsx`, `miniToolbar.test.tsx`, E2E `command-ui.spec.ts`.
  *Thiếu:* BlueSheet/BlueSlide vẫn dùng menu chính tả riêng (`SpellMenu`) cho từ sai; menu con (cascading); hình trong BlueDoc
  chỉ SmartArt (chưa có hình vẽ tự do).
- [ ] **UI-COMP-005** (P1) Hộp thoại nhóm dạng tab có xem trước + Đặt làm mặc định (Phông chữ, Đoạn văn, Định dạng ô, Thiết lập
  trang).
- [ ] **UI-COMP-006** (P1) Ngăn Định dạng đối tượng (mục thu gọn: Tô, Đường viền, Hiệu ứng, Kích thước, Văn bản).
- [ ] **UI-COMP-007** (P1) Thanh thông báo dưới ribbon + toast.

## Bàn phím & a11y (§9)

- [x] **UI-KEYTIPS-001** (P0) KeyTips: Alt hiện gợi ý trên tab rồi lệnh; tự gán không trùng trong phạm vi; bộ theo nhãn Việt +
  tùy chọn theo nhãn Anh. *Xong khi:* unit test gán không trùng; E2E định dạng chữ bằng KeyTips. **Đã có (CORE-UI-004):**
  `assignKeyTips` (`packages/ui/src/keyTips.ts`: chữ cái lấy từ nhãn đang hiển thị theo ngôn ngữ giao diện, bỏ dấu; ưu tiên chữ
  đầu mỗi từ; không trùng và không là tiền tố của nhau trong một phạm vi; hai ký tự khi hết chữ; ổn định khi thêm mục cuối;
  bảng `KEYTIP_OVERRIDES` — Tệp = F, Đậm/Nghiêng/Gạch chân = 1/2/3 — và prop `keyTipOverrides`) + `KeyTipsLayer` trong `Ribbon`:
  nhấn-thả Alt (hoặc F10) hiện chữ trên nút Tệp và các tab, gõ chữ của tab mở tab (cả khi ribbon thu gọn) rồi hiện chữ trên lệnh,
  nhóm thu gọn và nút "…"; ô nhập (phông, cỡ chữ) được đưa tiêu điểm; Esc lùi một bậc; Alt / nhấp chuột / rời cửa sổ tắt;
  Alt+phím khác (Alt+Q) không bật. Test `ui/test/keyTips.test.tsx`, E2E `command-ui.spec.ts`.
- [ ] **UI-A11Y-001** (P0) axe trong E2E cho khung 3 app (light/dark/hc), không vi phạm mức serious/critical.
- [ ] **UI-A11Y-002** (P0) F6 / Shift+F6 vòng giữa vùng; thứ tự focus; viền focus rõ.
- [ ] **UI-KEYS-001** (P1) Bổ sung phím tắt "dự kiến" trong [`shortcuts.md`](shortcuts.md).
- [x] **UI-KEYS-002** (P2) Cấu hình lại phím tắt trong Tùy chọn. **Đã có (CORE-UI-004):** lệnh `options.shortcuts` (Tệp ›
  Tùy chọn › "Tùy biến phím tắt…") mở `ShortcutsDialog` (`packages/editor`): mọi lệnh ribbon + lệnh có phím tắt mặc định, tìm
  theo tên/phím, Đổi (nhấn tổ hợp mới), Bỏ, Mặc định, Khôi phục tất cả; tổ hợp đã dùng báo tên lệnh đang giữ và chỉ gán khi xác
  nhận (lệnh kia mất phím); phím không hợp lệ (thiếu Ctrl/Alt, Tab/Enter/Shift+F10…) bị từ chối. `CommandRegistry` giữ phím
  người dùng đè lên mặc định (`setShortcutOverrides`, `conflicts`, `defaultShortcut`); lưu theo người dùng, theo app
  (`blueoffice.shortcuts.<app>` qua `shell.getSetting`/`setSetting`, web: `localStorage`); xuất/nhập tệp JSON
  (`blueoffice-shortcuts`, từ chối tệp của app khác). Test `editor/test/shortcuts.test.tsx`, E2E `command-ui.spec.ts`.

## Đa ngôn ngữ (§10)

- [ ] **UI-I18N-001** (P1) Test tràn chữ (nhãn dài nhất vi/en trong nút ribbon), dùng nhãn ngắn khi thiếu chỗ; rà chuỗi theo
  [`glossary.md`](glossary.md).

## Home & điểm vào (§11)

- [ ] **UI-HOME-001** (P1) BlueOffice Home: tạo nhanh 3 loại, mẫu nổi bật, gần đây/ghim/chia sẻ, kéo thả file, tìm tài liệu.
- [ ] **UI-HOME-002** (P2) Màn hình chờ (logo + tên app + thanh tiến trình mảnh ≤ ~1s).
- [ ] **UI-HOME-003** (P1) Tài liệu trống mặc định: BlueDoc A4 theo thể thức văn bản VN; BlueSheet vi-VN; BlueSlide 16:9 theme
  BlueOffice.

## Hạ tầng kiểm thử (§12)

- [ ] **UI-TEST-001** (P1) Storybook cho component (light/dark/hc × vi/en × 3 mật độ).
- [ ] **UI-TEST-002** (P1) Visual regression khung 3 app ở 1920/1440/1280/1024/768.
- [ ] **UI-PERF-001** (P2) Đo: đổi tab ribbon < 50ms, gallery < 100ms.

## Kết thúc

- [ ] **UI-REPORT-001** `docs/ui/REPORT.md` + ảnh chụp khung 3 app (light/dark, đầy đủ/đơn giản) + danh sách cần duyệt.
