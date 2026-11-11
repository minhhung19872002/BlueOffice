---
name: ci-run-triage
description: Use this skill when a GitHub Actions run of BlueOffice CI (`.github/workflows/ci.yml` — jobs Rust ubuntu/windows, cargo deny, Fidelity + visual-diff, Fuzz, Desktop windows/macos/ubuntu E2E with tauri-driver, Performance, Web E2E Playwright, .NET) is red, or before declaring "CI xanh". Covers reading failed job logs with `gh`, the known platform-dependent failure classes (libm float differences, stale corpus / insta snapshots, visual baselines, WebView2 msedgedriver), and iterating on a draft PR. Trigger words: "CI đỏ", "CI xanh", "gh run", "visual-diff failing", "corpus is stale", "tauri-driver".
---

# Chẩn đoán và sửa CI (BlueOffice)

## Khi nào dùng
- `gh run list` cho thấy run đỏ trên `main` hoặc trên PR.
- Trước khi kết luận một đợt việc "xong" (điều kiện kết thúc của Parity yêu cầu CI xanh).
- Test qua trên Windows local nhưng hỏng trên Linux/macOS của CI.

## Quy trình
1. `bash .claude/skills/ci-run-triage/scripts/failed-jobs.sh [run-id]` → bảng job + các dòng lỗi.
   Cần chi tiết: `gh run view <id> --log-failed | grep -A20 "<tên test>"`; artifact (ảnh visual diff, báo cáo Playwright):
   `gh run download <id> -D <thư mục trống trong scratchpad>`.
2. Phân loại theo bảng "Lỗi đã gặp" bên dưới; tái hiện local trước khi sửa (mỗi run CI ~35–60 phút).
3. Sửa trên nhánh riêng `ci/<việc>` rồi mở **một PR nháp** vào `main` (CI chạy trên `pull_request`); đẩy thêm commit để lặp.
   Không push thẳng `main`, không force-push. Gom nhiều sửa vào một lần đẩy.
4. Khi một run xanh toàn bộ job: gộp vào `main` (xem skill `blueoffice-merge-agent-branch`), push, kiểm tra run của `main`.

## Lỗi đã gặp (nguyên nhân → cách sửa)
| Triệu chứng | Nguyên nhân | Sửa |
|---|---|---|
| `…committed_corpus_is_up_to_date … is stale` chỉ trên Linux | `sin/cos/tan/pow` của libc khác bit cuối giữa UCRT và glibc | dùng crate `libm` (thuần Rust) trong mã sinh corpus / dựng hình; sinh lại corpus + snapshot |
| snapshot insta lệch chỉ trên một nền tảng | như trên (thứ tự vẽ mặt gần đồng phẳng) | như trên |
| `visual-diff.mjs`: N trang vượt ngưỡng | baseline cũ sau khi thêm tính năng hiển thị, hoặc corpus mới chưa có baseline | xem ảnh trong artifact; thay đổi có chủ đích → làm mới đúng các baseline đó, ghi lý do từng trang trong commit; không thì là hồi quy |
| Web E2E hỏng cả khi retry | thường là race thật (xem Pitfalls) | sửa mã; chỉ sửa test khi chứng minh được test sai |
| Desktop Windows: `timed out waiting for tauri-driver` | `${{ env.EDGEWEBDRIVER }}` rỗng (biến của runner, không thuộc context `env`); msedgedriver phải khớp phiên bản WebView2 | bước cài msedgedriver theo phiên bản WebView2 đọc từ registry |
| Desktop Windows: `DevToolsActivePort file doesn't exist` | WebView2 bỏ qua biến `WEBVIEW2_*` trong tiến trình elevated (runner là admin) | build E2E với feature cargo `webdriver` (`apps/desktop/src-tauri/src/automation.rs`), không có trong installer |
| `cargo test --workspace` dừng ở binary đầu tiên lỗi | thiếu `--no-fail-fast` | đã thêm; giữ nguyên |

## Lưu ý / Pitfalls
- `expect.poll` của Playwright **không thử lại khi callback ném lỗi** → callback phải trả giá trị, không ném.
- Phần tử được React tạo lại theo `key` (khung chọn, caret) có lúc chưa có `boundingBox()` → chờ hiển thị rồi mới đo.
- Effect hiển thị trạng thái mà test đọc ngay sau render → `useLayoutEffect`.
- Đừng dừng tiến trình `msedgewebview2` theo tên khi chẩn đoán desktop E2E: Teams / Search của máy dùng chung tên tiến trình.
- Rác build của CI local: `D:/bo-targets/ci` (xoá `*.pdb`, `incremental`).
