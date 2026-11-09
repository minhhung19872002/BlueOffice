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
- [ ] **UI-TOKENS-004** (P1) Font UI Be Vietnam Pro / Inter (OFL) đóng gói, fallback hệ thống. *Xong khi:* ghi
  THIRD_PARTY_NOTICES; test font được nạp.
- [ ] **UI-TOKENS-005** (P1) Chuyển động 120–200ms, tắt khi `prefers-reduced-motion`. *Xong khi:* test CSS media query.

## Icon (§4)

- [x] **UI-ICONS-001** (P0) Hạ tầng icon `packages/ui/icons`: component `<Icon name>` tree-shakable theo command id, cỡ
  16/20/24/32, hai tông (nét chữ + điểm nhấn app), bản đơn sắc cho high-contrast, `SOURCES.md`. *Xong khi:* test mọi icon có
  nguồn trong SOURCES.md; test viewBox/kích thước. **Đã có:** `packages/ui/src/icons` — `<Icon name size label>` (16/20/24/32, nét 1.5, trang trí ⇒ `aria-hidden`), 271 id lệnh gom từ ribbon 3 app (Lucide ISC, chọn một icon duy nhất cho mỗi id), icon tự vẽ `custom.tsx` (dấu app nháp, phân cách nghìn, ₫) có phần nhấn `.bo-icon-accent` theo `--bo-app-accent`, đơn sắc ở hc; `packages/ui/icons/SOURCES.md` sinh tự động; test `test/icons.test.tsx`.
- [ ] **UI-ICONS-002** (P0) Icon chung + định dạng chữ + đoạn văn (danh mục §4). *Xong khi:* có icon cho mọi command id của
  nhóm; test không thiếu icon trên ribbon Trang đầu.
- [ ] **UI-ICONS-003** (P1) Icon chèn, bảng tính, trình chiếu. *Xong khi:* test phủ icon cho ribbon 3 app.
- [ ] **UI-ICONS-004** (P1) Bản nháp logo BlueOffice + 3 icon app trong `assets/brand/drafts/` (cùng khung, khác ký hiệu, không
  kiểu chữ cái lớn chồng tờ giấy). *Xong khi:* SVG nháp + mục NEEDS_HUMAN duyệt bản chính thức.

## Khung cửa sổ (§5)

- [ ] **UI-FRAME-001** (P0) Component `AppFrame` dùng chung: thanh tiêu đề 40px (icon app, truy cập nhanh, tên tài liệu + trạng
  thái lưu, khu phải: cộng tác, Bình luận, Chia sẻ, tài khoản), hàng tab, thân ribbon, vùng làm việc, thanh trạng thái 24px.
  *Xong khi:* 3 app dùng chung khung; test unit vai trò ARIA landmark; E2E đo chiều cao.
- [ ] **UI-FRAME-002** (P1) Thanh truy cập nhanh tùy biến (thêm/bớt lệnh, nhớ theo người dùng). *Xong khi:* test thêm/bớt.
- [ ] **UI-FRAME-003** (P0) Ô tìm lệnh Ctrl+Q / Alt+Q: tìm trong registry (nhãn, từ khóa, không dấu), chạy lệnh, gợi ý tìm
  trong tài liệu. *Xong khi:* unit test xếp hạng; E2E định dạng chữ bằng ô tìm lệnh.
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
- [ ] **UI-RIBBON-003** (P0) Chế độ ribbon Đầy đủ / Đơn giản (một hàng, lệnh ít dùng vào "…") / Tự ẩn; Ctrl+F1; nhớ lựa chọn.
  *Xong khi:* E2E đổi chế độ ribbon.
- [ ] **UI-RIBBON-004** (P0) Tab ngữ cảnh (nhãn nhóm "Công cụ bảng"…, cùng màu nhấn) hiện theo selection. *Xong khi:* E2E chọn
  bảng hiện tab Thiết kế bảng/Bố cục bảng.
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

- [ ] **UI-FILE-001** (P0) Màn hình Tệp toàn trang: cột trái (Quay lại, Trang chủ, Mới, Mở, Thông tin, Lưu, Lưu thành, Lịch sử
  phiên bản, In, Chia sẻ, Xuất, Đóng, Tài khoản, Phản hồi, Tùy chọn), Esc quay lại. *Xong khi:* E2E mở màn hình Tệp.
- [ ] **UI-FILE-002** (P1) Trang chủ/Mới: mẫu nổi bật (mẫu hành chính VN tự soạn), tìm mẫu, gần đây/ghim/được chia sẻ.
- [ ] **UI-FILE-003** (P1) Thông tin: thuộc tính, bảo vệ, kiểm tra tài liệu, phiên bản.
- [ ] **UI-FILE-004** (P1) In: thiết lập bên trái + xem trước có điều hướng/zoom bên phải.
- [ ] **UI-FILE-005** (P1) Xuất: PDF/PDF-A, định dạng khác, video (BlueSlide).
- [ ] **UI-FILE-006** (P1) Tùy chọn: hộp thoại danh mục (Chung, Hiển thị, Soát lỗi, Lưu, Ngôn ngữ, Khả năng tiếp cận, Nâng cao,
  Tùy biến ribbon, Thanh truy cập nhanh, Bổ trợ, Bảo mật).

## Thành phần tương tác (§8)

- [ ] **UI-COMP-001** (P0) Bảng chọn màu (theme 10×6, chuẩn, gần đây, HEX/RGB/HSL, hút màu, Tự động/Không màu).
- [ ] **UI-COMP-002** (P0) Bộ chọn bảng (lưới rê chuột, Chèn bảng…, chuyển văn bản thành bảng).
- [ ] **UI-COMP-003** (P1) Bộ chọn font (tên bằng chính font, font chủ đề, gần đây, tìm, cảnh báo font thay thế).
- [ ] **UI-COMP-004** (P1) Menu chuột phải từ registry + thanh định dạng nổi.
- [ ] **UI-COMP-005** (P1) Hộp thoại nhóm dạng tab có xem trước + Đặt làm mặc định (Phông chữ, Đoạn văn, Định dạng ô, Thiết lập
  trang).
- [ ] **UI-COMP-006** (P1) Ngăn Định dạng đối tượng (mục thu gọn: Tô, Đường viền, Hiệu ứng, Kích thước, Văn bản).
- [ ] **UI-COMP-007** (P1) Thanh thông báo dưới ribbon + toast.

## Bàn phím & a11y (§9)

- [ ] **UI-KEYTIPS-001** (P0) KeyTips: Alt hiện gợi ý trên tab rồi lệnh; tự gán không trùng trong phạm vi; bộ theo nhãn Việt +
  tùy chọn theo nhãn Anh. *Xong khi:* unit test gán không trùng; E2E định dạng chữ bằng KeyTips.
- [ ] **UI-A11Y-001** (P0) axe trong E2E cho khung 3 app (light/dark/hc), không vi phạm mức serious/critical.
- [ ] **UI-A11Y-002** (P0) F6 / Shift+F6 vòng giữa vùng; thứ tự focus; viền focus rõ.
- [ ] **UI-KEYS-001** (P1) Bổ sung phím tắt "dự kiến" trong [`shortcuts.md`](shortcuts.md).
- [ ] **UI-KEYS-002** (P2) Cấu hình lại phím tắt trong Tùy chọn.

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
