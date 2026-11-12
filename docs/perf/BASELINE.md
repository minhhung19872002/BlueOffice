# Desktop — mốc hiệu năng (bước 1 của chương trình tăng tốc desktop)

Số đo trên bản **release** của app desktop (Tauri 2 + WebView2, engine native), trước mọi tối
ưu của chương trình. Kết quả trước/sau từng bước: [`RESULTS.md`](RESULTS.md). Mục tiêu (máy văn
phòng tầm trung): khởi động < 1,5 s; mở .docx 100 trang < 2 s; phím → glyph trên màn hình < 16 ms;
cuộn 60 FPS; RAM với một tài liệu < 400 MB; chữ sắc nét ở 100/125/150/200 %.

## Máy đo

| | |
|---|---|
| CPU | AMD Ryzen 7 7435HS — 8 nhân / 16 luồng, 3,1 GHz |
| RAM | 31,8 GB |
| GPU | NVIDIA GeForce RTX 2050 (driver 32.0.15.6607) |
| Hệ điều hành | Windows 11 Home Single Language 10.0.26300 (build 26300) |
| Màn hình | 1920×1080, thang 125 % (AppliedDPI 120); `devicePixelRatio` trong app = 1,375 (125 % × zoom 110 % của WebView) |
| WebView2 | 157.0.4341.46 (msedgedriver cùng phiên bản) |
| Tải nền | máy dùng chung với 3 agent khác đang build (5–12 `rustc`); CPU 40–100 % lúc đo (ghi trong JSON) |

Vì tải nền dao động mạnh, mỗi ca chạy **5 lần** (khởi động / mở / RAM; cộng một lần khởi động
bỏ đi để làm nóng bộ đệm đĩa, hồ sơ WebView2, chỉ mục font) và **3 phiên** WebDriver (gõ phím,
cuộn, đếm render); báo cáo là **trung vị**. Chênh 2 lần giữa các phiên trên máy này là bình
thường (xem các lần chạy bên dưới).

## Bản build đo

```bash
export PATH="$USERPROFILE/.cargo/bin:$PATH" CARGO_TARGET_DIR=D:/bo-targets/perf CARGO_BUILD_JOBS=4
pnpm build:wasm                                             # WASM cho replica cộng tác; engine soạn thảo desktop là native
pnpm --filter @blueoffice/desktop tauri build --no-bundle   # → $CARGO_TARGET_DIR/release/bluedoc.exe
```

Mốc dùng `[profile.release]` cũ (`lto = "thin"`, `codegen-units = 4`) và frontend của `main`
lúc bắt đầu (một chunk JS 3,07 MB). Thời gian build lạnh (máy bận, 4 job): `bo-wasm` 11 ph 53 s,
desktop 25 ph 38 s.

## Cách đo — `pnpm perf:desktop`

`scripts/perf-desktop.mjs` (Windows) đo bản exe release và ghi `out/perf/desktop-perf.json` +
`desktop-perf.md`:

```bash
cargo install tauri-driver --locked
# msedgedriver đúng phiên bản WebView2 (https://msedgedriver.microsoft.com/<phiên bản>/edgedriver_win64.zip)
MSEDGEDRIVER=<msedgedriver.exe> pnpm perf:desktop -- --runs 5 [--exe <bluedoc.exe>] [--doc <file.docx>]
pnpm perf:desktop -- --profile <máy> --update-baseline     # ghi docs/perf/baselines/<máy>.json
```

- **Nhật ký trong app** (`BLUEOFFICE_PERF_LOG=<tệp>`, `apps/desktop/src-tauri/src/perf.rs`): tắt khi
  không đặt biến (một lần đọc biến môi trường lúc khởi động). Khi bật, app ghi mỗi sự kiện một dòng
  JSON có mốc thời gian epoch: `process-main` (đầu `main`), `setup`, `page-load-*`,
  `system-fonts-ready`, thời gian native của từng lệnh engine (`engine_open`, `engine_command`,
  `engine_page`, `engine_glyph_outlines`, lệnh sheet / slide), và các mốc của web view do script dò
  `perf-probe.js` gửi về (`perf_mark`). Script dò chỉ được tiêm khi bật nhật ký: đo thời điểm
  shell / ribbon hiện, trang đầu vẽ xong và hiện ra màn hình, editor sẵn sàng (trang đã vẽ + có
  caret), thời gian từng lệnh IPC (vá `fetch` của giao thức `ipc://`), từng lần vẽ canvas trang, và
  đếm component React render mỗi commit theo vùng UI (một hook DevTools giả, chỉ bật khi cần).
  Một lần chạy đo là phiên riêng (không dùng single-instance) với hồ sơ WebView2 riêng, nên một bản
  BlueDoc khác đang mở trên máy không cướp lần khởi động.
- **Khởi động / mở / RAM**: chạy thẳng exe (không WebDriver), mốc 0 = ngay trước `spawn`.
  - *Khởi động* = tới `editor-ready` với tệp nhỏ `tests/corpus/doc/phase1-sample.docx` trên dòng
    lệnh (mở từ Explorer); thêm *shell hiện* (`#root` có nội dung + một khung hình) và *Start screen*
    (khởi động không tệp).
  - *Mở 100 trang* = từ lúc web view bắt đầu đọc tệp (`read_file`) tới khung hình đầu tiên có trang 1
    (`page-presented`): đọc đĩa + chuyển byte qua IPC hai chiều + `Engine::open` (đọc gói, mô hình,
    dàn trang toàn bộ) + display list trang đầu + glyph + vẽ. Tệp: `bo-cli sample-big
    out/perf/perf-big-100.docx --pages 100 --rich` (15,2 MB, 100 trang: đoạn căn đều, chú thích
    cuối trang, bảng, 40 ảnh, đề mục, header/footer có trường).
  - *RAM* = tổng **private working set** (cột "Memory" của Task Manager) của `bluedoc.exe` và mọi
    tiến trình con WebView2 (browser, renderer, GPU, utility — 7 tiến trình), 3 s sau khi trang đầu
    hiện. Working set tổng (đếm trùng DLL dùng chung) ghi kèm.
- **Gõ phím / cuộn / render** (tauri-driver + msedgedriver, như `pnpm test:e2e:desktop`): mở tệp
  100 trang, End (cuối dòng đầu), 5 phím làm nóng rồi 27 phím đo bằng WebDriver Actions (phím thật
  qua CDP). *Phím → glyph* = từ `timeStamp` của `keydown` tới khung hình đầu tiên sau lần vẽ lại
  canvas trang (rAF + một tác vụ, như `tests/e2e/specs/office-feel-doc.spec.ts`). Chia nhỏ từng
  phím: trước IPC lệnh, IPC `engine_command` (và phần native của nó), tới khi xin trang, IPC
  `engine_page` (và phần native), vẽ canvas, vẽ xong → khung hình. *Cuộn* = cuộn bằng script 40 px
  mỗi khung hình trong 5 s (≈ 11–18 trang): thời gian khung hình, FPS, khung hình có trang trống
  (trang trong vùng nhìn chưa có canvas). *Render* = 10 phím với đếm render bật: số component đã
  render mỗi phím theo vùng `ribbon` / `status` / `title` / `pane` / `other`.
- **Độ sắc nét**: ghi `devicePixelRatio` và tỉ lệ pixel canvas / pixel CSS của trang (1,374 ≈ dpr 1,375:
  canvas trang vẽ đúng mật độ điểm ảnh thiết bị → chữ không bị co giãn mờ). Ở 100/150/200 % chưa
  đo tự động (cần đổi thang màn hình của Windows); công thức canvas giống nhau (`zoom × dpr`).

## CI

Job `desktop` (Windows) chạy `scripts/perf-desktop.mjs --runs 5` trên bản release WebDriver,
đưa JSON và báo cáo lên artifact `desktop-perf`. CI tải kết quả từ lần CI thành công gần nhất
trên `main` làm mốc cùng loại runner; lần chạy đầu hoặc khi artifact đã hết hạn chỉ báo cáo,
không có phép so sánh. Chênh lệch trên 5% được cảnh báo và trên 20% làm job thất bại, với
ngưỡng nhiễu tối thiểu 5 ms / 10 MB / 2 FPS / 0,5 render. Runner dùng chung có thể biến động;
cần đối chiếu nhiều lần trước khi kết luận hồi quy. Mốc máy dev trong bảng dưới là một phép đo
riêng và không được giả làm mốc CI.

## Kết quả mốc

Exe: frontend + `[profile.release]` của `main` (thin LTO) + nhật ký đo (tắt khi không có biến).
Tải CPU lúc đo 41 → 94 %.

| Chỉ số | Các lần chạy | Trung vị | Mục tiêu | Đạt |
|---|---|---|---|---|
| Khởi động → editor sẵn sàng (tệp nhỏ) | (hỏng¹) · 1 925 · 1 632 · 1 484 · 1 378 ms | **1 558 ms** | < 1 500 ms | không |
| Khởi động → shell hiện | 1 503 · 1 283 · 1 146 · 1 075 ms | **1 215 ms** | < 500 ms | không |
| Khởi động → Start screen (không tệp) | 1 048 · 1 189 · 1 107 · 882 · 1 075 ms | 1 075 ms | — | |
| Mở .docx 100 trang → trang 1 trên màn hình | 1 713 · 1 699 · 1 326 · 1 412 · 1 674 ms | **1 674 ms** | < 2 000 ms | có |
| — trong đó `Engine::open` native | 515 · 515 · 380 · 412 · 490 ms | 490 ms | | |
| — IPC `engine_open` (15 MB lên + DocInfo về) | 855 · 849 · 655 · 666 · 801 ms | 801 ms | | |
| Phím → glyph (trung vị phiên) | 38,3 · 90,3 · 71,1 ms | **71,1 ms** | < 16 ms | không |
| Phím → glyph (p95 phiên) | 48,2 · 144,7 · 95,4 ms | **95,4 ms** | < 16 ms | không |
| Cuộn (FPS / p95 khung hình / khung trống) | 101,8 / 20,8 / 0 · 43,9 / 104 / 59 · 62,2 / 27,8 / 0 | **62,2 FPS** | 60 FPS | có (sát) |
| RAM một tài liệu (private, app + WebView2) | 292 · 288 · 291 · 291 · 291 MB | **291 MB** | < 400 MB | có |
| — sau khi gõ + cuộn 100 trang | 491 · 538 · 472 MB | 491 MB | < 400 MB | không |
| Render ribbon mỗi phím | 489 · 636 · 489 | **489** | 0 | không |
| Render title bar / status bar / khác mỗi phím | 16 / 9 / 85 | | 0 / 0 / — | không |

¹ Một lần WebView2 không nạp trang trong 45 s (hồ sơ còn bị tiến trình trước giữ); bỏ ra, harness
nay đợi 3 s sau khi tắt.

Màn hình làm mới 144 Hz (khung hình trung vị 7 ms khi máy yên), nên FPS > 60 là có thể.

### Thời gian đi đâu — khởi động (trung vị)

| Mốc từ `spawn` | ms |
|---|---|
| `process-main` | 22 |
| `setup` (Tauri đã dựng event loop + plugin) | 627 |
| `page-load-started` (cửa sổ + WebView2 đã tạo) | 767 |
| first contentful paint (sau khi nạp + chạy 3,07 MB JS từ tài nguyên nhúng) | 1 232 |
| shell + ribbon hiện | 1 215 |
| editor sẵn sàng (trang đã vẽ + caret) | 1 558 |

~630 ms trước cả `setup` (khởi tạo Tauri/tao/WebView2 environment trên máy bận) và ~600 ms từ khi
trang bắt đầu nạp tới shell (phân tích + chạy JS, IPC cài đặt, `engine_init` ~210 ms vì lệnh đồng bộ
chạy trên luồng chính đang bận).

### Thời gian đi đâu — một phím trong tài liệu 100 trang (trung vị các phím, phiên 3)

| Đoạn | ms |
|---|---|
| keydown → gửi `engine_command` | 2,0 |
| IPC `engine_command` (`insertText`) | 17,9 |
| — phần native (`bo-engine`: sửa mô hình + dàn trang lại) | 13,4 |
| kết quả về → xin trang (`engine_page`): **render React đồng bộ của cả editor** | 11,4 |
| IPC `engine_page` (display list JSON) | 7,4 |
| — phần native | 0,3 |
| trang về → bắt đầu vẽ | 1,2 |
| vẽ canvas trang (Canvas 2D, `renderPage`) | 4,9 |
| vẽ xong → khung hình hiện ra (đợi vsync + việc JS còn lại trong khung) | 21,7 |
| **Tổng** | **≈ 71** |

Ba lệnh IPC mỗi phím (lệnh, trang, truy vấn screen-reader/chính tả); tổng native trong cửa sổ một
phím 25,5 ms. Render mỗi phím: 489 component trong ribbon, 16 title bar, 9 status bar, 85 phần còn lại
— cả cây `BlueEditor` render lại đồng bộ (`useStore(store)` = `useSyncExternalStore`) trước khi trang
được xin và vẽ.
