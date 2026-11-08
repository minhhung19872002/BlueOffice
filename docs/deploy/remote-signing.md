# Ký số: dịch vụ ký từ xa, dấu thời gian, USB token, chuỗi tin cậy

Tài liệu cho quản trị viên và đơn vị tích hợp CA. Quyết định kiến trúc: [ADR-0007](../adr/0007-digital-signatures.md).

BlueOffice không bao giờ cầm khoá bí mật của người dùng (trừ khi người dùng tự chọn file `.p12`/`.pem`):
mọi bộ ký chỉ nhận **digest SHA-256** và trả về chữ ký **RSASSA-PKCS1-v1_5**.

| Bộ ký | Desktop (BlueDoc) | Web | `bo-cli` |
|---|---|---|---|
| File chứng thư `.p12` / `.pfx` / `.pem` | ✅ | — | `--p12` / `--key --cert` |
| USB token (PKCS#11) | ✅ | — | — |
| Dịch vụ ký từ xa (giao thức JSON dưới đây) | ✅ | ✅ (duy nhất) | `--remote` |

## 1. Giao thức ký từ xa (generic JSON, v1)

Mỗi CA Việt Nam (ký số từ xa / remote signing) có API riêng, nên BlueOffice định nghĩa một giao thức chung
tối giản; đơn vị tích hợp dựng một **gateway** chuyển giao thức này sang API của CA (hoặc viết adapter Rust
mới cài trait `RemoteSigningApi` trong `crates/bo-sign/src/remote.rs`).

- Mọi request là `POST`, `Content-Type: application/json`, UTF-8; base URL cấu hình được, ví dụ
  `https://sign.example.vn/api/v1`.
- Xác thực: `Authorization: Bearer <access token>` nếu có cấu hình token (OIDC của đơn vị, token do CA cấp…).
- Lỗi: HTTP 4xx/5xx, thân `{"message": "…"}` (hiển thị cho người dùng).
- Base64 là base64 chuẩn (RFC 4648 §4), có padding.

### `POST {base}/certificates`

```json
{ "credentialId": "cred-123" }
```
`credentialId` có thể `null` khi người dùng chỉ có một khoá. Trả về:
```json
{ "certificates": ["<DER base64 của chứng thư ký>", "<CA trung gian>", "..."] }
```
Phần tử đầu tiên là chứng thư ký; các phần tử sau là chuỗi CA trung gian (nhúng vào chữ ký để kiểm tra offline).

### `POST {base}/sign`

```json
{
  "credentialId": "cred-123",
  "hashAlgorithm": "SHA-256",
  "signatureAlgorithm": "RSASSA-PKCS1-v1_5",
  "digest": "<32 byte SHA-256, base64>",
  "documentName": "hop-dong.docx"
}
```
`digest` là giá trị băm cần ký **trực tiếp** (PKCS#1 v1.5 với DigestInfo SHA-256, RFC 8017 §9.2) — không băm lại.
Trả về một trong ba dạng:
```json
{ "status": "signed", "signature": "<chữ ký RSA, base64>" }
{ "status": "pending", "transactionId": "tx-1", "retryAfterMs": 2000 }
{ "status": "rejected", "message": "Người dùng từ chối" }
```
`pending` dùng khi CA yêu cầu người dùng xác nhận trên ứng dụng điện thoại.

### `POST {base}/sign/status`

```json
{ "transactionId": "tx-1" }
```
Trả về cùng dạng như `/sign`. Client hỏi lại sau `retryAfterMs` (10 ms–10 s, mặc định 2 s), tối đa 120 s.

Client kiểm tra chữ ký trả về bằng khoá công khai của chứng thư — dịch vụ ký nhầm khoá sẽ bị báo lỗi.

**Trình duyệt** gọi dịch vụ trực tiếp bằng `fetch`, nên gateway phải bật CORS cho origin của BlueOffice
(`Access-Control-Allow-Origin`, `Access-Control-Allow-Headers: Authorization, Content-Type`).

## 2. Dấu thời gian RFC 3161 (PAdES B-T, XAdES-T)

- Cấu hình URL máy chủ dấu thời gian (TSA) của CA: `bo-cli sign … --tsa <url>` hoặc biến `BO_TSA_URL`;
  desktop/web: ô "Máy chủ dấu thời gian" trong hộp thoại Ký số (giá trị mặc định cấu hình như §5).
- Giao thức: RFC 3161 §3.4 (HTTP `POST`, `application/timestamp-query` → `application/timestamp-reply`),
  `certReq = TRUE`, nonce, thuật toán băm SHA-256.
- PDF: token nằm trong thuộc tính không ký `signature-time-stamp` của CMS (ETSI EN 319 142-1 baseline B-T).
  OOXML: `xd:SignatureTimeStamp` trong `xd:UnsignedSignatureProperties` (ETSI TS 101 903 §7.3), băm trên
  phần tử `SignatureValue` đã chuẩn hoá C14N.
- Khi xác minh, nếu token hợp lệ và TSA được tin cậy, chứng thư người ký được kiểm tra **tại thời điểm dấu
  thời gian** — chữ ký vẫn hợp lệ sau khi chứng thư hết hạn.
- Trình duyệt chỉ dùng được TSA có bật CORS; nếu không, để trống ô TSA (chữ ký B-B) hoặc ký trên desktop.

## 3. USB token (PKCS#11, chỉ desktop)

- Cài driver token của CA; chỉ định đường dẫn module PKCS#11 (ví dụ `C:\Windows\System32\<vendor>_p11.dll`)
  trong hộp thoại Ký số (được nhớ cho lần sau) hoặc biến môi trường `BO_PKCS11_MODULE`.
- BlueDoc liệt kê chứng thư trên token (không cần PIN), người dùng chọn chứng thư và nhập PIN; ký bằng
  `C_Sign` / `CKM_RSA_PKCS` trên DigestInfo SHA-256. PIN không được lưu.
- Bản desktop build với feature `pkcs11` của `bo-sign` (crate `cryptoki`, Apache-2.0).

## 4. Chuỗi tin cậy (Root CA quốc gia)

BlueOffice **không cài sẵn** chứng thư gốc nào. Quản trị viên tải chứng thư Root CA quốc gia (và CA khác được
chấp nhận) từ nguồn chính thức, kiểm tra dấu vân tay, rồi cấu hình:

- `bo-cli verify file.pdf --trust root-ca.pem` (lặp lại được; file PEM nhiều chứng thư, file DER, hoặc thư mục
  chứa `.pem/.crt/.cer/.der`), hoặc biến `BO_TRUST_ANCHORS` (danh sách đường dẫn, phân tách bằng `;` trên Windows,
  `:` trên Linux). `--json` in báo cáo máy đọc được.
- Desktop: biến `BO_TRUST_ANCHORS` như trên.
- Web: file PEM đặt tại `apps/web/public/trust/anchors.pem` (hoặc URL cấu hình `VITE_BO_TRUST_ANCHORS_URL`).

Kiểm tra: dựng đường dẫn theo tên issuer/subject, chữ ký từng chứng thư (RSA SHA-256/384/512), thời hạn,
`basicConstraints`/`pathLenConstraint`, `keyUsage` (keyCertSign cho CA; digitalSignature/nonRepudiation cho người
ký), `id-kp-timeStamping` cho TSA. **Chưa kiểm tra thu hồi (CRL/OCSP).**

## 5. Giá trị mặc định cho người dùng

| Cấu hình | Desktop | Web (build-time) |
|---|---|---|
| URL dịch vụ ký từ xa | `BO_REMOTE_SIGN_URL` | `VITE_BO_REMOTE_SIGN_URL` |
| URL máy chủ dấu thời gian | `BO_TSA_URL` | `VITE_BO_TSA_URL` |
| Module PKCS#11 | `BO_PKCS11_MODULE` | — |
| Trust anchors | `BO_TRUST_ANCHORS` | `VITE_BO_TRUST_ANCHORS_URL` (mặc định `/trust/anchors.pem`) |

Người dùng có thể sửa URL trong hộp thoại; lựa chọn gần nhất được nhớ trên máy (không nhớ PIN, mật khẩu, token).
