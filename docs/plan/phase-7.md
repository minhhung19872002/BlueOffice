# Phase 7 — BlueForm & BlueNote (kế hoạch, chưa code)

Bắt đầu sau Phase 6A. Tuân theo quy tắc đặt tên trong `CLAUDE.md`.

## BlueForm — biểu mẫu / khảo sát
- Trình thiết kế biểu mẫu (câu hỏi ngắn/dài, lựa chọn một/nhiều, thang điểm, ngày, tệp đính kèm, logic rẽ nhánh đơn giản).
- Chia sẻ qua link (dùng phân quyền & link có hạn của server Phase 6), hỗ trợ ẩn danh hoặc đăng nhập SSO.
- Kết quả trả lời ghi vào một workbook **BlueSheet** (mỗi biểu mẫu một sheet, mỗi câu hỏi một cột) qua `bo-xlsx` + API server; cập nhật thời gian thực bằng `bo-collab`.
- Lưu định nghĩa biểu mẫu dạng JSON trong BlueDrive; xuất PDF bản trả lời.

## BlueNote — sổ ghi chú
- Sổ → mục → trang; mỗi trang là một tài liệu `bo-model` (dùng lại `bo-layout`, `bo-edit`, `canvas-host` của BlueDoc) ở chế độ trang không giới hạn chiều dài.
- Cộng tác và sửa offline dùng `bo-collab`; tìm kiếm toàn văn qua `/extract-text`.
- Xuất trang/sổ ra `.docx` và PDF.

## Phụ thuộc
Phase 2 (bo-edit), Phase 3 (BlueSheet), Phase 6 (server, collab), Phase 6A (BlueDrive).
