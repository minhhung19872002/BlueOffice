---
name: blueoffice-app-parity-round
description: Use this skill when running BlueOffice native-app parity rounds — BlueDoc / BlueSheet / BlueSlide / BlueReader / BlueEditor compared side by side with Word / Excel / PowerPoint / Foxit Reader / Foxit Editor — launching one background agent per app, reviewing their side-by-side screenshots and sending them to the owner, merging `worktree-agent-*` branches into `work/desktop-performance`, building release exes and publishing them to `out\desktop`. Trigger words: "làm giống office", "so sánh với word/excel/powerpoint/foxit", "chạy agent song song", "vòng tiếp theo", "chụp hình tui coi", "chép đè exe".
---

# Vòng parity cho các app native (BlueOffice)

Quy trình chủ dự án đã chốt (memory `parallel-app-parity-workflow.md`): làm liên tục, không hỏi lại, mọi so sánh đều có
ảnh cho chủ dự án xem.

## Khi nào dùng
- Bắt đầu một phiên làm "giống Office/Foxit" cho các app native (bo-ui + apps/native-win).
- Một agent app báo xong vòng / gửi ảnh so sánh.
- Cần gộp nhánh agent vào `work/desktop-performance` và đưa exe mới vào `out\desktop`.

## Quy trình

### 1. Đầu phiên
1. `git log --oneline -15 work/desktop-performance`; liệt kê nhánh `worktree-agent-*` chưa gộp
   (`git branch --no-merged work/desktop-performance | grep worktree-agent`) — xem commit, gộp cái đã xong.
2. Đọc mục "còn lại / remaining" cuối mỗi `docs/ui/REPORT*.md` (REPORT = BlueDoc, -sheet, -slide, -reader, -editor).
3. Chép `tools/ui-compare/AGENT-BRIEF.md` vào scratchpad thành `brief-common.md`, thay `<SCRATCH>` bằng đường dẫn
   scratchpad và `<this session URL>` bằng URL phiên; tạo thư mục `scratchpad\show\`.
4. Kiểm tra đĩa (`df -h /c /d /e`) và RAM; mỗi app build vào `<ổ>:/bo-targets/<app>` riêng.

### 2. Mở agent cho từng app (tối đa ~4–5 agent Rust cùng lúc)
- Viết `brief-<app><N>.md` theo mẫu [references/example-app-brief.md](references/example-app-brief.md): build dir,
  `git reset --hard work/desktop-performance` + kiểm tra merge vòng trước, 5–8 việc đánh số (việc dở vòng trước + góp ý
  từ ảnh), ghi vòng mới vào REPORT, ảnh `scratchpad\show\<app>-NN-*.png` tiếp số cũ, exe ra `out/desktop/<app>-native.next.exe`.
- `Agent` với `isolation: "worktree"`, `run_in_background: true`; prompt ngắn: reset, đọc brief-common + brief vòng,
  chủ dự án muốn XEM ảnh (SendMessage "main" sau mỗi bước), sửa file dùng chung chỉ theo kiểu thêm vào, `-j 2`,
  không input thật, không đè exe đang chạy, `cargo fmt` trước mỗi commit, commit ít nhất mỗi giờ.

### 3. Khi agent gửi ảnh
- `Read` từng ảnh → `SendUserFile` (status proactive, display render) kèm chú thích tiếng Việt ngắn.
- Tự so: vị trí/kích thước (px), màu, nhãn (viết hoa, xuống dòng), nút thiếu, trạng thái bật/tắt, chữ đè nhau, đơn vị.
  Gửi `SendMessage` góp ý cụ thể cho agent. Icon trông như vẽ lại của bản tham chiếu → bắt vẽ lại theo mẫu riêng.
- Agent im lâu: `git log -1 worktree-agent-<id>` + `git -C .claude/worktrees/agent-<id> diff --stat`; nhiều dòng
  chưa commit → nhắc commit.

### 4. Gộp và đưa exe lên
1. Không gộp khi một bản release build trên nhánh chính đang chạy.
2. `git merge --no-ff worktree-agent-<id> -m "Merge <App> round N: …"` (dòng Co-Authored-By/Claude-Session ở cuối).
3. Xung đột thường gặp:
   - danh sách `mod`/`pub use` trong `lib.rs`: giữ cả hai;
   - locale JSON: gộp sâu bằng Python (`json` load hai phía `git show HEAD:` / `MERGE_HEAD:`, thêm khoá thiếu,
     `json.dumps(ensure_ascii=False, indent=2)`), kiểm tra hai tệp cùng bộ khoá;
   - hai agent cùng viết một module (vd. FDF/XFDF, nút hàng ribbon): giữ một bản, tách phần khác tên vào module mới,
     đổi tên hàm trùng; chạy test của cả hai app.
4. Kiểm tra (`CARGO_TARGET_DIR=E:/bo-targets/main CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0`):
   `cargo clippy -j 2 --workspace --exclude bo-desktop --all-targets -- -D warnings`, `cargo fmt --all -- --check`,
   `cargo test -j 2 -p <crate đụng tới>`, `cargo deny check licenses bans sources`. Đổi bo-engine/bo-render/chart →
   cập nhật snapshot insta có lý do + `pnpm build:wasm` + test platform/editor web.
5. `cargo build -j 2 --release -p bo-native-win --bin <app>-native` (bỏ `CARGO_PROFILE_DEV_DEBUG`, đừng đặt rỗng).
6. `powershell -File .claude/skills/blueoffice-app-parity-round/scripts/publish.ps1 -Apps <app>-native -Smoke` —
   mở thử (không chiếm focus), chụp, rồi chép vào `out\desktop` (exe đang chạy → đổi tên `.old.exe`). Xem ảnh smoke.
7. Mở vòng mới cho app đó ngay (bước 2).

## Template / Reference
- [references/example-app-brief.md](references/example-app-brief.md) — mẫu brief một vòng (BlueSheet vòng 7).
- `tools/ui-compare/` — `AGENT-BRIEF.md` (luật chung), `cap2.ps1` (PrintWindow), `drive.ps1`, `menu.ps1`, `type2.ps1`,
  `lat.ps1`, `wopen.ps1`/`wclose.ps1` (COM, chỉ trên bản sao tệp).
- [scripts/publish.ps1](scripts/publish.ps1) — mở thử + đưa exe vào `out\desktop`.

## Lưu ý / Pitfalls
- Phím/chuột thật (SendInput/keybd_event/SetCursorPos, `agent-desktop --headed`) CHỈ khi chủ dự án không dùng máy:
  `tools/ui-compare/physical-idle.ps1 -Start` rồi `-Check 10` trước mỗi lượt (chỉ đếm thao tác không giả lập; bận thì dừng
  ngay), một agent một lúc (`%TEMP%o-real-input.lock`), chỉ vào instance tham chiếu của mình trên bản sao; quy tắc đủ ở
  `tools/ui-compare/AGENT-BRIEF.md` (quyết định 2026-11-24). Ngoài lúc đó: thông điệp giả lập / UIA / COM; bản tham chiếu
  nào bỏ qua thông điệp giả lập (menu thả, Ctrl, kéo thả OLE) thì chấp nhận ảnh chỉ của mình, ghi rõ.
- Không chụp danh sách tệp gần đây của bản tham chiếu; lỡ chụp thì xoá ảnh.
- Tệp khoá `~$*.docx` trong `tests/corpus/doc` (Word đang mở tệp mẫu) làm đỏ `whole_corpus_lays_out_renders_and_exports`
  — không phải lỗi code.
- Ổ C:/D:/E: hay đầy: xoá `debug/` của agent đã xong và `bo-targets/main/debug` khi < 10 GB (kiểm tra không có cargo
  nào dùng thư mục đó).
- Agent báo "doc/slide không đổi" nhưng sửa renderer dùng chung → luôn chạy test engine trên nhánh chính.
- Đổi kiểu dữ liệu dùng chung (vd. `PdfDocument.doc` → `store::Document`) làm hỏng nhánh khác khi gộp: sửa trên nhánh
  chính rồi nhắn agent kia `git merge work/desktop-performance`.
