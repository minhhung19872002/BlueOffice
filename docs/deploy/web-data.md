# Proxy trang web cho "Lấy & biến đổi dữ liệu → Từ web"

Trình duyệt không đọc được trang của site khác (CORS), nên bản web của BlueSheet chỉ lấy bảng HTML từ web qua máy chủ
BlueOffice, endpoint `POST /api/data/web` (`apps/server/BlueOffice.Api/WebData/WebDataProxy.cs`). **Mặc định tắt**:
endpoint trả 404 và không gửi gì đi. Bản desktop gọi thẳng, không cần proxy.

```json
// appsettings.json
"WebData": {
  "Enabled": true,
  "AllowPrivateNetworks": false,   // true: cho phép trang nội bộ (loopback, 10/8, 192.168/16, …)
  "MaxBytes": 20971520,            // trang lớn nhất (20 MB)
  "Timeout": "00:00:30"
}
```

- Chỉ người đã đăng nhập hoặc phiên soạn thảo hợp lệ (`session` + header `X-Session-Token`).
- Chỉ GET `http`/`https`, không có thông tin đăng nhập trong URL; tối đa 3 lần chuyển hướng.
- Địa chỉ riêng/loopback/link-local bị chặn lúc mở kết nối (chống DNS rebinding), trừ khi `AllowPrivateNetworks`.
- Trình soạn thảo gọi máy chủ cùng origin, hoặc địa chỉ trong `VITE_BO_WEBDATA_URL` lúc build; `off` = không có "Từ web".
