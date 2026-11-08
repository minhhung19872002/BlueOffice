# ADR-0007 — Ký số PDF (PAdES) và OOXML qua giao diện `Signer`

- Trạng thái: chấp nhận (2026-11-07, bổ sung 2026-11-08)
- Bối cảnh: prompt §6 Phase 6 — ký PDF (PAdES) và ký OOXML (XML-DSig theo spec), chuẩn bị điểm tích hợp
  USB token / ký số từ xa của CA Việt Nam.

## Quyết định
- Crate mới `bo-sign` (native: server, desktop, CLI; không cần cho WASM).
- Mọi thao tác ký đi qua trait `Signer { certificate_der, chain_der, sign_sha256(digest) }`: chỉ đưa **digest SHA-256**
  ra ngoài, nhận lại chữ ký PKCS#1 v1.5. Đây là đúng hình dạng API của USB token (PKCS#11 `C_Sign` với
  `CKM_RSA_PKCS` trên DigestInfo) và của các dịch vụ ký số từ xa (gửi hash, nhận chữ ký) → khoá bí mật không bao giờ
  vào tiến trình BlueOffice. `SoftwareSigner` (khoá RSA trong bộ nhớ) chỉ dùng cho test và máy chủ tự giữ khoá.
- PDF: PAdES baseline B-B (ETSI EN 319 142-1) — `/SubFilter /ETSI.CAdES.detached`, CMS detached với
  content-type, message-digest, signing-certificate-v2; thời điểm ký ở `/M`. Ghi bằng **incremental update**
  (ISO 32000-1 §7.5.6) nên chữ ký trước vẫn hợp lệ (đã test ký 2 lần).
- OOXML: ECMA-376 Part 2 §12 — `_xmlsignatures/origin.sigs`, `sigN.xml`, Manifest gồm mọi part (SHA-256) và mọi
  relationships part qua Relationship Transform + C14N, `SignatureTime`. XML được sinh sẵn ở dạng Canonical XML 1.0
  nên không cần thư viện C14N tổng quát.

## Bổ sung 2026-11-08 — dấu thời gian, chuỗi tin cậy, USB token, ký từ xa, UI
- **Dấu thời gian RFC 3161** (`bo-sign::timestamp`): `TimeStampReq` SHA-256 + nonce + `certReq`, kiểm tra
  `TimeStampResp` (status, imprint, nonce, chữ ký token, `id-kp-timeStamping`). Vận chuyển qua trait
  `TimestampClient`: `HttpTimestampClient` (RFC 3161 §3.4) trên trait `HttpTransport`, bản native `UreqTransport`
  (feature `http`, ureq + rustls); `testing::LocalTsa` là TSA trong tiến trình cho test (không cần mạng).
  - PDF → **PAdES B-T**: token ở thuộc tính không ký `signature-time-stamp` (id-aa 2.14) của SignerInfo,
    băm trên giá trị chữ ký (ETSI EN 319 142-1 §6.3).
  - OOXML → **XAdES-T**: thêm `Object` chứa `xd:QualifyingProperties` ([MS-OFFCRYPTO] §2.5.2, ETSI TS 101 903):
    `SignedProperties` (SigningTime + SigningCertificate, tham chiếu từ `SignedInfo`) — tức XAdES-BES — và
    `UnsignedProperties/SignatureTimeStamp` băm trên phần tử `SignatureValue` đã C14N.
- **Chuỗi tin cậy** (`bo-sign::trust`): trust anchor là cấu hình (`--trust`, `BO_TRUST_ANCHORS`, PEM/DER/thư mục;
  web: `/trust/anchors.pem`), **không** nhúng chứng thư Root CA nào vào mã nguồn. Kiểm tra theo RFC 5280 §6 rút gọn:
  dựng đường theo tên, chữ ký RSA SHA-256/384/512, thời hạn, basicConstraints/pathLen, keyUsage, EKU TSA.
  `report::verify_document` gộp kiểm tra mật mã + dấu thời gian + chuỗi cho cả CLI, desktop, web (JSON).
  Có dấu thời gian tin cậy thì chứng thư người ký được kiểm tra **tại genTime** (mô hình ETSI EN 319 102-1).
- **Ký hai pha** (`PreparedSignature` / `prepare_pdf` / `prepare_ooxml` → `digest()` → `finish(sig, token)`) để bên gọi
  lấy chữ ký bất đồng bộ (trình duyệt + dịch vụ ký từ xa); `sign_document` là đường đồng bộ qua `Signer`.
- **USB token**: `pkcs11::Pkcs11Signer` (feature `pkcs11`, crate `cryptoki` Apache-2.0 — FFI nằm trong dependency,
  crate của ta giữ `forbid(unsafe_code)`), `C_Sign`/`CKM_RSA_PKCS` trên DigestInfo SHA-256; chỉ bật ở desktop.
  Đường dẫn module PKCS#11 là cấu hình (`BO_PKCS11_MODULE` hoặc nhập trong hộp thoại).
- **Ký từ xa**: `remote::RemoteSigner` trên trait adapter `RemoteSigningApi`; `JsonSigningApi` cài giao thức JSON
  chung ([docs/deploy/remote-signing.md](../deploy/remote-signing.md): `/certificates`, `/sign`, `/sign/status` với
  trạng thái `pending` chờ người dùng xác nhận). Mỗi CA Việt Nam có API khác nhau → gateway/adapter riêng, không
  gắn cứng một nhà cung cấp. Chữ ký trả về được kiểm lại bằng khoá công khai của chứng thư.
- **File chứng thư**: `.p12/.pfx` qua `p12-keystore` (feature `pkcs12`, PBES2/AES và 3DES/RC2 cũ), `.pem` gộp khoá+chuỗi.
- **UI BlueDoc**: lệnh `file.sign` (lưu bản .docx đã ký), `file.exportSignedPdf`, `file.verifySignatures` trong
  `CommandRegistry`; hộp thoại chọn phương thức ký (file / token / từ xa) + TSA; thanh trạng thái chữ ký khi mở
  file đã ký. Mọi lời gọi qua `Platform.signing`: desktop = lệnh Tauri (`sign_cmds.rs`, cả ba phương thức); web =
  chỉ ký từ xa — WASM (`bo-wasm` `signPrepare`/`verifySignatures`) tính digest, `fetch` gọi dịch vụ ký và TSA.
  PIN, mật khẩu, access token không bao giờ được lưu.

## Phương án đã loại
- **OpenSSL / ring bindings**: phụ thuộc C, khó build MSVC + cross; RustCrypto thuần Rust, license Apache/MIT.
- **Ký trong .NET (System.Security.Cryptography.Xml, iText)**: iText là AGPL (cấm); ký trong .NET khiến desktop
  offline không ký được.
- **Thư viện C14N/XML-DSig tổng quát**: không có crate thuần Rust license phù hợp và đủ chín; sinh XML canonical trực tiếp
  đơn giản và kiểm chứng được bằng test.

## Hệ quả / giới hạn
- PDF dùng cross-reference stream (§7.5.8) chưa ký được (`Unsupported`); `bo-pdf` sinh xref cổ điển nên không ảnh hưởng.
- `verify_ooxml` chỉ kiểm chữ ký do BlueOffice tạo (dạng canonical); chữ ký từ phần mềm khác báo `Unsupported`
  (vẫn được giữ nguyên khi lưu nhờ round-trip lossless). Chưa thêm `idOfficeObject`/XAdES của [MS-OFFCRYPTO] §2.5.
- Đã có PAdES B-T / XAdES-T, kiểm tra chuỗi tới trust anchor cấu hình và kiểm tra thu hồi bằng CRL nạp sẵn
  (RFC 5280 §6.3: `--crl`, file `.crl` trong thư mục `--trust`, khối PEM `X509 CRL`; CRL phải do đúng CA cấp chứng
  thư ký, chứng thư bị thu hồi **sau** thời điểm ký có dấu thời gian vẫn hợp lệ). **Chưa** có OCSP, chưa tự tải CRL
  từ `cRLDistributionPoints`, chưa nhúng dữ liệu xác minh dài hạn (PAdES B-LT/B-LTA: DSS, document time-stamp).
- `verify_ooxml` dựa trên dạng canonical do BlueOffice sinh; chữ ký XAdES của phần mềm khác vẫn báo `Unsupported`.
- Chỉ khoá RSA (PKCS#1 v1.5); chứng thư/TSA dùng ECDSA báo `Unsupported`.
- Trình duyệt chỉ dùng được dịch vụ ký / TSA có bật CORS.
- Ký lại một .docx đã ký rồi sửa: chữ ký cũ không còn hợp lệ và được báo `invalid` (chưa tự gỡ chữ ký cũ).
- Crate `rsa` 0.9 có cảnh báo RUSTSEC-2023-0071 (rò rỉ thời gian khi **giải mã/ký** bằng khoá bí mật). Chỉ ảnh hưởng
  `SoftwareSigner`; đường ký sản xuất dùng token/ký từ xa. Nâng lên `rsa` 0.10 khi phát hành ổn định.
