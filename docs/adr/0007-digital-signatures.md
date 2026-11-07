# ADR-0007 — Ký số PDF (PAdES) và OOXML qua giao diện `Signer`

- Trạng thái: chấp nhận (2026-11-07)
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
- Chưa có dấu thời gian RFC 3161 (PAdES B-T) và kiểm tra chuỗi tin cậy tới Root CA quốc gia — để ở bước tích hợp CA.
- Crate `rsa` 0.9 có cảnh báo RUSTSEC-2023-0071 (rò rỉ thời gian khi **giải mã/ký** bằng khoá bí mật). Chỉ ảnh hưởng
  `SoftwareSigner`; đường ký sản xuất dùng token/ký từ xa. Nâng lên `rsa` 0.10 khi phát hành ổn định.
