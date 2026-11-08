# Phase 6 / 6A — Server, cộng tác, tích hợp BlueDrive (phía .NET)

Trạng thái: **server hoàn thành — chờ duyệt**. Prompt: [`BlueOffice-PROMPT.md`](../../BlueOffice-PROMPT.md) §6 Phase 6, 6A.
Phần Rust `bo-collab` và `@blueoffice/editor-sdk` / TS làm riêng. Giao thức: [`docs/bohp.md`](../bohp.md) ·
Triển khai: [`docs/deploy/`](../deploy/README.md).

## Đã làm

- **Kiến trúc**: `BlueOffice.Domain` (entity, quyền, hợp đồng host `IHostStorage`, job) · `BlueOffice.Infrastructure`
  (EF Core SQLite/PostgreSQL, object store filesystem/MinIO, hàng đợi in-memory/Redis, `BohpClient`, `WopiClient`,
  JWKS validator, webhook dispatcher, `ConversionJobProcessor`) · `BlueOffice.Api` (Minimal API, WebSocket, background services).
  Một image chạy `Role=all|api|worker`.
- **File/thư mục**: CRUD, upload/download (giới hạn kích thước, SHA-256, tên an toàn), phiên bản + khôi phục, `If-Match`/409,
  thùng rác mềm, chia sẻ owner/editor/commenter/viewer (kế thừa theo thư mục), link chia sẻ có hạn, audit log.
- **Xác thực**: OIDC/JWT (Keycloak), token host xác minh bằng JWKS, token dịch vụ của host; dev key bị chặn ở Production.
- **Cộng tác**: một room cho mỗi file (`host/fileId`), relay y-protocols sync + awareness, seeder, nhật ký update lưu
  snapshot/incremental vào object store và phát lại cho phiên sau, compaction, presence (`collaborators-changed`), quyền
  view/comment không được sửa, khoá BOHP thuộc room, autosave 45 s (debounce) + lưu khi người cuối rời, version kèm danh sách
  người sửa, restore → `reload`.
- **Phase 6A**: phiên editor (`/edit`, `/api/sessions`) cho local / BOHP / WOPI, refresh token + đọc lại quyền (định kỳ và
  qua `POST /api/hosts/{host}/notify`) → `mode-changed`/`session-revoked`, audit chuyển về host, webhook ký HMAC
  (`file.saved`, `file.conflict-copy`, `session.started/ended`, `audit`), sửa offline → bản "(xung đột - người dùng - thời gian)",
  WOPI client theo [MS-WOPI] (CheckFileInfo, GetFile, PutFile, Lock/Unlock/RefreshLock, RenameFile, PutRelativeFile, discovery).
- **Dịch vụ phụ trợ**: `POST /convert`, `/thumbnail`, `/preview`, `/extract-text` qua hàng đợi (Redis) + worker `bo-cli`;
  `?wait=true` hoặc poll `/jobs/{id}`.
- **Mock BlueDrive** (`tests/mock-bluedrive`): BOHP v1, JWKS RS256, khoá, phiên bản, webhook receiver, endpoint `/_admin/*`
  để điều khiển test; chạy độc lập `dotnet run --project tests/mock-bluedrive`.
- **Triển khai**: `deploy/Dockerfile`, `deploy/docker-compose.yml` (api, worker, postgres, minio, redis, keycloak tuỳ chọn),
  hướng dẫn cài đặt / sao lưu / nâng cấp tiếng Việt.

## Endpoint

| Nhóm | Endpoint |
|---|---|
| Hệ thống | `GET /health`, `/health/ready`, `/api/version` |
| Người dùng | `GET /api/me` |
| Thư mục | `GET/POST /api/folders`, `GET/PATCH/DELETE /api/folders/{id}`, `GET /api/shared` |
| File | `POST /api/files?name=&folderId=`, `GET/PATCH/DELETE /api/files/{id}`, `GET/PUT /api/files/{id}/content[?version=]`, `GET /api/files/{id}/versions`, `POST /api/files/{id}/versions/{n}/restore` |
| Chia sẻ | `GET/POST /api/files/{id}/shares`, `GET/POST /api/folders/{id}/shares`, `DELETE /api/shares/{id}`, `POST /api/files/{id}/links`, `GET /api/links/{token}[/content]` |
| Audit | `GET/POST /api/files/{id}/audit` |
| Phiên editor | `GET/POST /edit`, `POST /api/sessions`, `GET/DELETE /api/sessions/{id}`, `GET …/contents`, `POST …/checkpoint`, `…/refresh`, `…/audit`, `GET …/versions`, `POST …/versions/{v}/restore`, `…/rename`, `…/save-as`, `…/sync` |
| Host | `POST /api/hosts/{host}/notify`, `GET /hosting/discovery`, `GET /api/hosts`, `GET /api/hosts/{host}/me`, `…/folders/{id}/children`, `…/shared`, `…/recent`, `…/resolve?path=` (proxy BOHP §A2 cho desktop) |
| Cộng tác | `WS /collab/{fileId}?session=&token=` (hoặc `?access_token=` cho file local) |
| Chuyển đổi | `POST /convert`, `/thumbnail`, `/preview`, `/extract-text`, `GET /jobs/{id}`, `/jobs/{id}/result`, `POST /api/convert/pdf` (Phase 1) |

## Kiểm thử

`dotnet test BlueOffice.sln` — xUnit + WebApplicationFactory, SQLite tạm + object store filesystem + hàng đợi in-memory,
`bo-cli` giả (`FakeRunner`). Gồm: ma trận quyền, vòng đời file, link hết hạn, audit, relay Yjs/awareness, snapshot và phát lại,
hạ quyền giữa phiên, các kịch bản BlueDrive (mở file, 2 người cùng phiên + một khoá, lưu tạo version kèm người sửa, autosave,
lưu khi rời phiên, thu hồi quyền, sửa offline → bản xung đột, khoá bởi client khác), WOPI host giả, job chuyển đổi.

## Cộng tác thời gian thực (BlueDoc, BlueSheet, BlueSlide)

Cả ba ứng dụng dùng chung một replica `bo-collab` (Yrs, offset UTF-16 như Yjs) trong chế độ nhúng
(`packages/hosted` → `openHosted` / `startCollab`, dùng chung cho web nhúng và deep link desktop). Binding TS ở
`packages/collab`: `collabDocHost`, `collabSlideHost`
(bọc `EngineHost`, cùng khuôn với `collabSheetHost`). Lõi ánh xạ ở `bo-engine`: `collab.rs` (BlueDoc) và
`slide_edit/collab.rs` (BlueSlide); WASM `WasmEngine.collab*` / `WasmCollab.applyLocal|takeRemote|fullState|seed`,
Tauri `engine_collab_*`.

**Đồng bộ những gì**
- BlueDoc — `body` Y.Text: mỗi block cấp section là một dòng kết thúc bằng `\n`. Định dạng run là thuộc tính:
  `bold`, `italic`, `underline`, `strike`, `size` (half-point), `color`, `font`, `highlight` + `r` (JSON phần `RunProps`
  còn lại, gồm cả dấu track changes). Thuộc tính đoạn nằm trên `\n`: `style`, `align`, `list` + `p` (phần `ParaProps`
  còn lại); `\n` cuối mỗi section mang `sect` (JSON sectPr: khổ giấy, lề, cột, header/footer). Tab, ngắt dòng/trang,
  ảnh, field, bookmark… là embed một vị trí `{"c": JSON}`; bảng cấp thân bài là embed `{"t": khoá}` + `\n` (chia sẻ từng ô, xem
  "Cấu trúc đồng bộ trực tiếp"), bảng lồng và block lạ là embed `{"b": JSON}` + `\n`.
- Sau mỗi lệnh cục bộ engine so mô hình với *shadow* của văn bản chung (so block, rồi tiền tố/hậu tố theo ký tự) và
  trả `collab.change` (delta Quill) trong `EditResult`; delta từ xa áp vào shadow rồi dựng lại các block đổi, áp như
  một transaction **ngoài lịch sử cục bộ** (`bo_edit::Session::apply_external`); con trỏ của mình được dời theo delta.
- Undo/redo khi cộng tác đi qua `UndoManager` của replica (chỉ hoàn tác thay đổi của chính mình).
- BlueSlide — `slides` Y.Map (khoá `sldId`, giá trị `{"pos","slide"}`: thuộc tính slide + vị trí) và `shapes` Y.Map
  (khoá `"<sldId>/<cNvPr id>"`, giá trị `{"z","node"}`), last-writer-wins theo đối tượng; slide thêm trong phiên được
  cấp `sldId` theo client id. Ảnh/part mới (doc media, pptx part) đi qua `blobs` (base64).
- Người vào đầu tiên nhận `seed` và đăng toàn bộ thân văn bản/deck; người vào sau thay nội dung bằng trạng thái chung
  sau lần đồng bộ đầu (`fullState`). Khung nhận được khi engine đang bận được giữ lại (`LiveQueue`) để engine và
  replica luôn thấy thay đổi theo cùng thứ tự.
- Presence: BlueDoc gửi vùng chọn (offset UTF-16) → caret màu kèm tên (`DocumentView.setPeers`, lệnh
  `view.collaborators` bật/tắt); BlueSlide gửi đối tượng đang chọn → khung nét đứt màu trên slide.

**Hạn chế** (xem thêm mục "Cấu trúc đồng bộ trực tiếp" bên dưới)
- Vị trí caret của người khác tính theo offset tuyệt đối (chưa dùng relative position của Yjs) nên có thể lệch tạm thời
  cho tới khi họ gửi presence mới; caret trong ô bảng hiển thị ở đầu bảng.
- Sửa trước lần đồng bộ đầu tiên của người vào sau không được chia sẻ (bị nội dung chung thay thế).
- Mỗi lệnh cục bộ so toàn bộ danh sách block với shadow (O(số block), không cấp phát) — đủ nhanh cho tài liệu
  thường; tài liệu rất dài cần đánh dấu block bẩn từ op.
- Server không giữ Y.Doc: sau khi mất kết nối, client đăng lại **toàn bộ** trạng thái replica (`snapshot`) nếu đã sinh
  thay đổi cục bộ kể từ lần kết nối trước (nhật ký room lớn thêm cỡ một tài liệu; compaction thu gọn lại).

## Cấu trúc đồng bộ trực tiếp (2026-11-08)

Replica `bo-collab` có thêm hai cấu trúc dùng chung cho cả ba ứng dụng, đều nằm trong phạm vi `UndoManager` (hoàn tác
chỉ đảo thay đổi của chính người dùng, kể cả các loại mới):

- `meta` — Y.Map khoá → chuỗi JSON, **last-writer-wins theo khoá** (định nghĩa, cấu trúc bảng, thuộc tính sheet…).
- `texts` — Y.Map khoá → **Y.Text lồng** ("story"), hợp nhất như văn bản thân bài. Phong bì `Change` mang
  `texts: {khoá: {reset, deltas} | null}` (`reset` = tạo lại từ rỗng; observer `observe_deep` báo delta theo khoá, tạo
  mới/thay thế → đọc lại toàn bộ), cùng `meta` và `cells`. `fullState()` gồm mọi thứ cho người vào sau; server lưu
  `snapshot()` như cũ nên người vào sau nhận đủ các cấu trúc mới.

Thứ tự (hàng/cột bảng, hàng/cột sheet, sheet, slide) dùng **khoá thứ tự** (`bo_engine::collab::order`): mỗi phần tử là
một mục LWW riêng mang số thứ tự, so sánh `(khoá, id)`; chèn/di chuyển chỉ ghi phần tử mới/được chuyển (giữ dãy tăng dài
nhất), nên chèn/di chuyển đồng thời các phần tử *khác nhau* đều được giữ, mọi replica sắp như nhau.

**BlueDoc** (`crates/bo-engine/src/collab/parts.rs`)
- Bảng cấp thân bài: dòng thân bài là embed `{"t": khoá}` + `\n`. `meta`: `tb:<t>` (tblPr), `tr:<t>:<hàng>` =
  `{"o", "p": trPr}`, `tl:<t>:<cột>` = `{"o", "w": gridCol}`, `tc:<t>:<hàng>:<cột>` (tcPr); nội dung mỗi ô là story
  `c:<t>:<hàng>:<cột>` (cùng định dạng dòng với thân bài). Hai người sửa hai ô — hoặc cùng một ô — đều được giữ; chèn/xoá
  hàng/cột hội tụ. Một hàng gồm các ô có khoá `tc`, theo thứ tự cột. Nhận diện bảng/hàng/cột khi so mô hình với shadow:
  tiền tố/hậu tố bằng nhau rồi ghép theo thứ tự (cột so bằng độ rộng + nội dung ô ở cột đó).
- Story: chú thích chân trang `fn:<id>` / cuối `en:<id>`, nội dung comment `cm:<id>`, header/footer `hf:<section>:<kind>`
  (`hd hf he fd ff fe`); JSON `sect` trên `\n` cuối section không còn chứa nội dung header/footer.
- Định nghĩa LWW theo khoá: style `st:<id>`; numbering của file gốc `nm:<numId>` / `an:<abstractNumId>`; metadata
  comment `cm:<id>` (`{"o", "c"}`: tác giả, ngày, luồng trả lời, `done`) — thêm/trả lời/đánh dấu xong/xoá đều tới
  người khác; `doc` = `evenAndOddHeaders`. Thuộc tính section vẫn là thuộc tính `sect` của `\n` (LWW theo section).
- Người gieo (seeder) = so mô hình với shadow rỗng (`collab_start(seed, clientId)`); id bảng/hàng/cột mới lấy theo client
  id của replica (WASM `collabStart(seed, clientId)`, Tauri `engine_collab_start { seed, client }`).
- Sửa header/footer và chú thích tại chỗ (story session) được chia sẻ ngay sau mỗi lệnh.
- Caret của mình trong ô bảng được giữ theo (bảng, id hàng, id cột, đoạn, offset) khi thay đổi từ xa dựng lại bảng
  (E2E phát hiện: trước đó caret nhảy về đầu bảng nên chữ gõ tiếp dính vào ô đầu).

**BlueSheet** (`crates/bo-engine/src/sheet/collab.rs`, binding `packages/collab/src/sheet.ts`)
- **Id hàng/cột ổn định**: hàng `i` của file đã gieo có id `"i"`, khoá `i` (dãy nền vô hạn, không lưu); hàng chèn trong
  phiên có id của replica và khoá nằm giữa hai hàng kề (`ra:<sheet>:<id>`), hàng xoá được đánh dấu (`rd:<sheet>:<id>`);
  cột: `ca:` / `cd:`. Vị trí = thứ hạng giữa các hàng còn sống (`Axis`).
- Ô: map `cells`, khoá `"<sheet>!<id hàng>,<id cột>"` (giá trị hoặc công thức); định dạng `meta` `fm:<cùng khoá>` theo
  **giá trị** (font, fill, border, numFmt, alignment — không phải chỉ số xf), nên giá trị và định dạng đặt đồng thời
  đều giữ. **Công thức lưu tham chiếu dạng id hàng/cột** (template + bảng tham chiếu): mỗi replica dựng lại chữ A1
  theo hàng/cột hiện tại, nên A chèn hàng trong khi B gõ `=A3*10` thì mọi nơi đều thành `=A4*10`; tham chiếu tới hàng
  bị xoá thành `#REF!`. Thay đổi cấu trúc từ xa áp bằng `bo_sheet::ops` (điều chỉnh công thức như sửa cục bộ) rồi
  đối chiếu mọi ô dùng chung của sheet.
- `rp:<sheet>:<hàng>` (row), `cp:<sheet>:<cột đầu>` (col + số cột), gộp ô `mg:<sheet>!<góc trên trái>` = góc dưới phải
  (theo id), sheet `sh:<sheet>` = khoá thứ tự + sheet không có ô (tên, ẩn, view, page setup…): thêm/đổi tên/xoá/sắp
  xếp hội tụ; hai sheet cùng tên thêm đồng thời → sheet sau được đánh số "(2)". `cf:<sheet>` (kèm dxf dùng tới),
  `dv:<sheet>` LWW theo sheet; biểu đồ `ch:<sheet>:<i>`, pivot `pv:<sheet>:<i>` + cache `pc:<i>` LWW theo đối tượng;
  tên định nghĩa `wb`.
- Binding TS: host có `collabStart/collabLocal/collabApply` (WASM `WasmSheet.collab*`, Tauri `sheet_call` cùng tên)
  → sau mỗi lệnh sửa (`setInputs`, `exec`, `structural`, thêm/đổi tên/xoá sheet) lấy `collabLocal()` đưa vào replica
  (mỗi lệnh một bước hoàn tác); `undo`/`redo` đi qua replica. Host không có binding vẫn dùng đường cũ (chỉ giá trị ô).

**BlueSlide** (`slide_edit/collab.rs`)
- `pos` của slide là khoá thứ tự: di chuyển chỉ ghi slide được chuyển, hai người chuyển hai slide khác nhau cùng lúc
  thì cả hai lần chuyển được giữ (cùng một slide: người ghi sau thắng).
- Ghi chú người trình bày: chữ của placeholder body trong notes là story `n:<sldId>` (văn bản thuần, hợp nhất); phần còn
  lại của notes đi theo slide.
- Master / layout / theme: `sm:<key>`, `sl:<key>`, `th:<key>` + `pp` (quan hệ presentation), LWW theo part. Part của file
  mở khoá theo chỉ số; part tạo trong phiên (dán slide từ deck khác) có khoá riêng theo client nên hai người nhập thiết
  kế cùng lúc giữ cả hai (test `designs_imported_at_once_by_two_peers_are_both_kept`). Tham chiếu slide → layout →
  master → theme đi theo khoá (`lk`, `@m`, `@t`, `@l`) và được đổi lại thành chỉ số ở mỗi replica. Giải mã master
  (text styles vài chục KB) chạy trên luồng có stack lớn ở bản native (bản debug tràn stack 1–2 MB).

**Kiểm thử**: `crates/bo-collab` (story lồng: reset, delta đồng thời, undo riêng, full state, xoá);
`crates/bo-engine/tests/collab.rs` (ô bảng cùng lúc, chèn hàng/cột + xoá hàng đồng thời, style/comment/footnote,
header, proptest thêm thao tác bảng, di chuyển slide đồng thời, ghi chú hợp nhất, master LWW);
`crates/bo-engine/tests/collab_sheet.rs` (người vào sau, chèn hàng + công thức đồng thời, xoá hàng được tham chiếu +
chèn cột, định dạng + giá trị, kích thước, gộp ô, sheet thêm/đổi tên/xoá, CF, undo riêng, proptest nhập/công thức/
chèn-xoá hàng-cột/in đậm); `packages/collab/test/sheet-engine.test.ts` (hai WasmSheet thật); Playwright
`tests/e2e/specs/collab-structures.spec.ts` với server thật (hai người gõ vào hai ô của cùng một bảng cùng lúc; chèn hàng
dời ô và công thức của người kia; ghi chú slide).

**Hạn chế còn lại (thật)**
- Bảng lồng trong ô bảng: chia sẻ từng ô như bảng thân (dòng `{"t":key}` trong story ô `c:<t>:<r>:<c>`, đệ quy; dựng lại
  bảng cha khi ô con đổi — `rebuild_nested`). Client cũ cùng phòng không hiểu dòng `{"t"}` lồng.
- Bảng trong header/footer/chú thích/comment: vẫn là một embed `{"b"}` (LWW cả bảng).
- Chèn hàng và chèn cột đồng thời trong cùng bảng BlueDoc: hàng mới được thêm ô trống ở cột mới (mỗi hàng ghi các cột
  cố ý để trống, `RowEntry::x`, nên hàng ngắn do "xoá ô đẩy trái" vẫn giữ nguyên). Xoá hàng/cột trong khi người khác gõ vào ô của nó: chữ gõ mất cùng hàng/cột.
- Id comment/chú thích mới: khi cộng tác, mỗi replica lấy id ≡ hạt giống riêng (từ client id Yrs, mod 65 536,
  `Session::set_id_seed`) nên hai người thêm **đúng cùng lúc** không còn trùng (test
  `concurrent_comments_and_notes_get_distinct_ids`; trước đó comment của một người bị mất). Danh sách tạo trong phiên vẫn chia sẻ theo kiểu (`bullet`/`number`).
  Header/footer khoá theo số thứ tự section: chèn section break đồng thời với sửa header có thể gán nhầm section.
  `docDefaults`, `defaultTabStop`, latent styles không có lệnh sửa trong BlueDoc nên không cần đồng bộ (mọi replica
  mở cùng một file).
- BlueSheet: vùng của CF và DV đi kèm id hàng/cột (`cfa:`/`dva:`, client cũ bỏ qua) nên dịch theo hàng/cột người khác
  chèn/xoá đồng thời (test `rules_added_during_a_row_insert_follow_the_rows`); hyperlink, autofilter, bảng (ListObject)
  cũng vậy (`sha:`, test `a_filter_turned_on_during_a_row_insert_follows_the_rows`), và biểu đồ (ô neo + tham chiếu
  series, `cha:`), vùng in / tiêu đề in (công thức theo id trong `sha:`), pivot (vị trí `pva:`, nguồn cache `pca:`).
  Freeze pane: ranh giới đóng băng theo id hàng/cột (`sha:` `fz`). Mọi phần của sheet giờ theo id hàng/cột. Đổi tên sheet không sửa công
  thức tham chiếu tên cũ (như engine hiện tại). Biểu đồ/pivot/cache khoá theo chỉ số: thêm đồng thời trùng chỉ số →
  LWW. Ô trong hàng bị xoá vẫn nằm trong map (để hoàn tác khôi phục) cho tới khi lưu version. Kiểu ô lưu `xfId`
  (style có tên) theo chỉ số. Chèn liên tiếp > ~50 lần đúng một chỗ làm cạn độ chính xác khoá (thứ tự giữa các hàng mới
  chèn cùng chỗ khi đó theo id).
- BlueSlide: tên part mới (ảnh, media, slide…) của replica đang cộng tác có thêm 5 chữ cái theo client
  (`imagekqzta1.png`, `build::salt_names`) nên hai người thêm ảnh cùng lúc không trùng tên (test
  `pictures_pasted_at_once_by_two_peers_keep_their_own_parts`; trước đó mỗi bên thấy ảnh của mình). Part gói khác
  (media đã có, notes master…) không đồng bộ ngoài `blobs`; định dạng chữ ghi chú khi gộp hai bản sửa lấy theo bản
  ghi notes thắng, chữ thì hợp nhất.
- Hoàn tác BlueSheet chỉ đi qua replica khi host có binding engine; lịch sử cục bộ của engine bị xoá khi nhận thay đổi
  từ xa (như BlueSlide).
## Kiểm chứng end-to-end (server thật + mock BlueDrive)

Tiêu chí §6A ("mở file từ web, mở từ desktop qua deep link, 2 người cùng sửa, lưu tạo version, thu hồi quyền giữa chừng,
sửa offline rồi đồng bộ") được kiểm bằng E2E chạy **server thật**, không giả lập giao thức:

| Thành phần | Cách chạy trong E2E |
|---|---|
| `tests/mock-bluedrive` | `dotnet run` — BOHP, JWKS RS256, `/_admin/*` (tạo file, quyền, token, xem version) |
| `apps/server/BlueOffice.Api` | `dotnet run`, `Development`, SQLite + object store filesystem tạm, hàng đợi in-memory, host `bluedrive` trỏ tới mock (JWKS, webhook, `AllowedOrigins` = origin trang host; `Cors:Origins` thêm `http://tauri.localhost`) |
| Web editor | `apps/web/dist` qua `vite preview` với `BLUEOFFICE_API_PROXY` (proxy `/api`, `/hosting`, `/collab` WebSocket — cùng sơ đồ với `deploy/web/nginx.conf`) |
| Trang host | `tests/e2e/collab/host` (vite dev) nhúng editor bằng `@blueoffice/editor-sdk` `BlueOffice.open`; token đi qua `postMessage` |

Bộ khởi động: `tests/e2e/collab/stack.mjs` (cổng `COLLAB_PORT`, mặc định `E2E_PORT + 20`: editor, host, server, mock).
Kịch bản (hai browser context = hai người dùng của mock):

- `specs/collab-doc.spec.ts`: A (Nguyễn Văn An) gõ tiếng Việt → B (Trần Thị Bình) thấy; B in đậm một cụm từ → A thấy
  trạng thái đậm; caret kèm tên của người kia hiển thị ở cả hai phía; lệnh host `save` → version mới trong mock với
  `editors = [u1, u2]`, nội dung docx có đoạn đã gõ và run đậm; B `setOffline(true)` → banner "Mất kết nối", B sửa,
  A không thấy; B online lại → A nhận thay đổi offline, sửa tiếp của A tới B; mock thu hồi quyền sửa của B +
  `POST /api/hosts/bluedrive/notify` → B chuyển sang **chỉ xem** (banner, gõ phím không đổi tài liệu, lệnh `save` bị
  từ chối) nhưng vẫn thấy A sửa trực tiếp. Đường lưu offline cũ (`POST …/sync?baseVersion=` cũ) → bản
  "(xung đột - Trần Thị Bình - yyyy-MM-dd HH-mm)" cùng thư mục, bản gốc không bị ghi đè.
- `specs/collab-sheet-slide.spec.ts`: BlueSheet — công thức `=2*21` và chữ tiếng Việt qua lại giữa hai người, lưu từ host
  → version có `<f>2*21</f>` và cả hai người sửa; BlueSlide — A chọn và kéo placeholder tiêu đề, B thấy khung chọn của A
  di chuyển và chọn được hình ở vị trí mới (trùng vị trí bên A).
- `tests/e2e-desktop/desktop.test.mjs` ("desktop deep link …"): khởi động cùng stack, người dùng web u1 mở file trong
  trang host; `bluedoc.exe` được mở bằng tham số `bluedoc://open?host=bluedrive&file=…&token=…` với
  `BLUEOFFICE_SERVER` → tạo phiên server, tải file, vào cùng room; chữ gõ trên web hiện ở desktop và ngược lại;
  Ctrl+S trên desktop lưu checkpoint → version mới với `editors = [u1, u2]`.

Chạy:

```bash
pnpm build:wasm && pnpm --filter @blueoffice/web build          # web dist dùng cho preview
pnpm --filter @blueoffice/e2e test:collab                        # chỉ E2E cộng tác (tự khởi động stack)
pnpm test:e2e                                                    # E2E web thường + cộng tác
node tests/e2e/collab/stack.mjs                                  # stack để thử tay (COLLAB_STACK_VERBOSE=1 xem log)
# desktop: build bluedoc.exe rồi
MSEDGEDRIVER=<msedgedriver.exe khớp WebView2> pnpm test:e2e:desktop   # COLLAB_E2E=0 bỏ ca deep link
```

**Desktop**: `apps/desktop/src/main.tsx` nghe `ShellIntegration.onOpenLink` (tham số dòng lệnh, instance thứ hai, deep
link) → `openHosted` của `@blueoffice/hosted` với server lấy từ `BLUEOFFICE_SERVER` (lệnh Tauri `server_url`) hoặc thiết
lập đã lưu (`Platform.shell.serverUrl/setServerUrl`). Replica `bo-collab` chạy bằng WASM trong web view (CSP thêm
`'wasm-unsafe-eval'`); engine là engine native. CSP `connect-src` cho phép `https:`/`wss:` mọi máy chủ và
`http`/`ws` tới `localhost`/`127.0.0.1` (máy chủ HTTP nội bộ khác: thêm origin vào CSP trong `tauri.conf.json` khi
build). Server phải cho origin web view vào CORS: `Cors:Origins` gồm `http://tauri.localhost` (Windows/Linux) và
`tauri://localhost` (macOS).

**Chế độ chỉ xem**: `SessionMode` theo `welcome.mode`, `mode-changed`, `session-revoked`/4403; `guardPlatform` chặn mọi
lệnh sửa của BlueDoc/BlueSheet/BlueSlide (điều hướng, chọn, sao chép, tìm, xuất vẫn chạy), không trả lời
`save-request`, `saveFile` từ chối; `HostedBanner` báo chỉ xem / mất quyền / mất kết nối.

**Lỗi đã sửa khi chạy E2E thật**: người vào đầu tiên không bao giờ seed room (bỏ qua `welcome.seed`); `mode-changed`
không được xử lý (không chuyển chỉ xem); sửa offline không bao giờ tới server (server không có Y.Doc để trả lời sync step
1 bằng phần chênh — giờ client đăng lại snapshot sau khi kết nối lại, và đóng socket ngay khi trình duyệt báo `offline`);
nền tảng nhúng spread instance class nên mất các method `shell` (`recentFiles`…); whitelist origin `postMessage` chỉ
có lúc build (`VITE_EMBED_ORIGINS`) — giờ lấy từ `GET /api/hosting/origins` khi không đặt; Ctrl+S / nút Lưu của editor
đi qua `Platform.storage` nên tải file về thay vì lưu lên host — `hostedPlatform` giờ thay cả `storage`; tauri-driver
nhận deep link dạng `--bluedoc://…` (msedgedriver thêm `--`) nên app bỏ qua; ca E2E desktop cũ để mồ côi msedgedriver
giữ cổng 4445.

**Hạn chế còn lại (quan sát trong E2E)**
- **Đã sửa (2026-11-08) — lỗi chập chờn ca deep link desktop**: nguyên nhân không nằm ở engine native. Khi người vào
  sau nhận nội dung chung (chèn toàn bộ tại offset 0), `transform_index` đẩy caret qua phần chèn, nên caret nhảy xuống
  **cuối tài liệu**; chữ gõ ở đó rơi sau khối cuối và bị bỏ. Ca "gõ ngay khi chưa bấm" luôn hỏng. Khi người dùng bấm
  trước lúc nội dung chung tới thì chỉ hỏng thỉnh thoảng (phụ thuộc thời điểm), và view cũng cuộn xuống cuối. Giờ caret
  được đo trên bản sao riêng của người vào sau rồi kẹp vào độ dài văn bản chung
  (`late_joiner_typing_at_its_first_caret_is_shared` kiểm vị trí chèn).
  Phần chập chờn còn lại là lỗi của test: WebDriver gõ từng phím, test lấy số từ trung gian ở desktop rồi chờ web
  khớp con số đó trong khi hai bên đã hội tụ ở số cuối. Test giờ chờ số từ ổn định và in log cảnh báo của cả hai
  editor khi quá giờ; `joiner_typing_anywhere_in_a_real_file_converges` quét gõ ở mọi đoạn (tab, ngắt dòng, bảng) của
  file mẫu. Kết quả: E2E desktop 5/5 lần chạy liên tiếp xanh.
- Người vào sau thấy tài liệu ở trạng thái "Chưa lưu" và view cuộn tới caret sau khi nội dung chung thay thế bản cục bộ.
- CSP desktop không cho `http://` tới máy chủ khác `localhost` (cần HTTPS hoặc build lại với origin trong CSP).

## Hoàn thiện Phase 6A phía desktop (2026-11-08)

| Mục | Cài đặt | Kiểm thử |
|---|---|---|
| **Màn hình Start** | `packages/hosted/src/start/StartScreen.tsx`: Trang chủ (tài liệu/bảng tính/trình chiếu trống, mở tệp trên máy, tệp gần đây), tab **BlueDrive** (đăng nhập SSO hoặc dán token, Tệp của tôi theo thư mục, Gần đây, Được chia sẻ với tôi, mở → phiên server + cộng tác), **Cài đặt** (máy chủ BlueOffice — thay cho chỉ biến môi trường; thư mục đồng bộ). Hiện khi desktop khởi động không kèm file/deep link; `Alt+Home` quay lại; `?start=1` cho cửa sổ mới | Vitest `start.test.tsx`, `drive.test.ts`; E2E desktop "Start screen: BlueDrive sign-in…" |
| **Duyệt BlueDrive** | BOHP §A2 (`/me`, `/folders/{id}/children`, `/shared`, `/me/recent`, `/sync/resolve`) trong mock; server proxy `/api/hosts/{host}/…` (desktop không gọi thẳng host; CSP/CORS chỉ cần server) + `GET /api/hosts` | .NET `HostBrowseTests` |
| **SSO** | OAuth 2.0 code + PKCE S256 (`signin.ts`), trình duyệt hệ thống (`open_url`, không qua shell), redirect `bluedoc://auth` (`shell.rs::parse_auth_callback`, sự kiện `auth-callback`); mock có `/oauth/authorize` + `/oauth/token` | Vitest (PKCE, state), .NET (mã một lần, sai verifier), Rust `recognises_sign_in_redirects` |
| **Thư mục đồng bộ** | `syncFolders.ts` (thư mục cụ thể nhất, không phân biệt hoa thường trên Windows, chặn `..`); file mở từ hệ điều hành, Ctrl+O, tệp gần đây nằm trong thư mục đồng bộ + đã đăng nhập BlueDrive → `resolve` → hộp thoại `SyncPrompt` → phiên cộng tác | Vitest; E2E desktop (tệp gần đây trong thư mục đồng bộ → hộp thoại → phiên thứ hai trên server) |
| **Sửa offline desktop** | `offline.ts`: Lưu khi server không truy cập được → bản sao + version gốc trong `<app data>/offline` (`DesktopDiskCache`, lệnh Tauri `cache_*`, ghi tạm rồi đổi tên); có mạng lại trong phiên → CRDT gửi lại, bỏ bản sao, lưu version; mở lại sau khi đóng app → `…/sync?baseVersion=` → version mới hoặc "(xung đột - người dùng - thời gian)"; mất mạng lúc mở → mở bản tải gần nhất | Vitest `offline.test.ts`; E2E desktop "offline…" (proxy TCP cắt mạng thật, tải version ngoài, mở lại → bản xung đột, bản gốc giữ nguyên) |
| **Webhook / audit** | Thử lại cả khi timeout, `Webhooks:MaxAttempts/RetryBaseDelay`; editor nhúng và desktop báo `print`/`export` qua `…/audit` (trước đây không client nào gọi) | .NET `WebhooksAreRetried…`, `WebhooksGiveUp…`, `EditorAuditEventsAreForwardedToTheHost`; Vitest |
| **Khoá / autosave** | Đã có; thêm kiểm thử gia hạn đúng lockId ở nửa thời hạn, không autosave khi không đổi | .NET `SessionLockIsRefreshedAndAutosaveOnlyRunsAfterChanges` |
| **Dịch vụ phụ trợ** | Ma trận định dạng `ConversionFormats` (docx/docm/doc/odt/txt/md → pdf/docx/odt/txt; xlsx/xlsm/xlsb/xls/ods → pdf/xlsx/ods/csv; pptx → pdf/pptx; ppt/odp → odp/txt); `extract-text` bảng tính = CSV từng sheet; stdout `bo-cli` đọc UTF-8 (lỗi font chữ Việt trên Windows); `bo-cli convert` nhận `.txt/.md` → PDF | .NET `BoCliEndToEndTests` chạy `bo-cli` thật (`BO_CLI` hoặc `$CARGO_TARGET_DIR`) |
| **Deep link BlueSheet/BlueSlide** | `bluesheet://` `blueslide://` → ứng dụng theo loại file trong cùng shell | E2E desktop: người dùng web gõ công thức, desktop (BlueSheet) lưu version có `<f>2*21</f>`; `blueslide://` mở BlueSlide |

Hạn chế còn lại: SSO chưa có E2E tự động (cần trình duyệt hệ thống) — kiểm bằng .NET/Vitest với mock; token BlueDrive chỉ
giữ trong bộ nhớ (đóng app phải đăng nhập lại, token mock sống 10 phút, chưa có refresh token); nhận diện thư mục đồng bộ cần
đã đăng nhập tab BlueDrive; nút quay lại Start chỉ là phím tắt `Alt+Home`; mock dùng một cây thư mục chung cho mọi người dùng.

## Còn lại (ngoài phạm vi lần này)

- **Ký số** (Phase 6): ký PDF PAdES và OOXML XML-DSig; điểm tích hợp dự kiến: một `ISignatureProvider` (USB token qua agent
  cục bộ trên desktop, ký số từ xa của CA Việt Nam qua REST) + job `sign` trong worker; chưa code.
- `bo-collab` (Yrs) và editor SDK/TS (postMessage, nút Share/Close, chế độ offline) — luồng chính.
- Nhiều instance `api`: room **và** phiên (`SessionStore`) đều ở bộ nhớ một tiến trình; các endpoint REST tìm phiên
  theo id phiên. Thiết kế dự kiến: (1) phiên lưu ở PostgreSQL/Redis; (2) id phiên mang id instance sở hữu phòng và
  `nginx` định tuyến `/collab/{fileId}` bằng `hash $fileId consistent`, `/api/sessions/{id}` theo tiền tố id;
  (3) khoá sở hữu phòng trong Redis (`SET NX PX`, gia hạn) để hai instance không cùng mở một phòng — instance không
  sở hữu đóng WebSocket mã 4009 cho client kết nối lại; (4) test bằng hai `TestApp` dùng chung Redis giả.
- ~~EF Core migrations~~ (đã làm: `Data/Migrations/{Postgres,Sqlite}`, `DatabaseMigrator`); Testcontainers cho PostgreSQL/MinIO/Redis (hiện test dùng SQLite/filesystem/in-memory).
- `ppt`/`odp` → `pptx`/PDF/ảnh: `bo-cli` chưa có đường từ mô hình ODF/[MS-PPT] sang PresentationML (chỉ `odp`, `txt`);
  `/thumbnail` và `/preview` của hai định dạng này trả 422.
- Server chưa giữ Y.Doc (không merge được bản offline dạng CRDT ở server): khi xung đột luôn tạo bản sao.
