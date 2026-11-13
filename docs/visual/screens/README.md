# Ảnh kiểm tra

Ảnh tại đây phải lấy từ ứng dụng thực, ghi kèm manifest gồm executable và SHA-256, hệ điều
hành, scale Windows, DPR WebView, viewport, tài liệu, zoom và theme. Mỗi bộ phải được rà và
liên kết trong [báo cáo](../REPORT.md). Không đặt tên DPI vật lý cho ảnh chỉ giả lập DPR.

Bộ ảnh desktop đã có ở [docs/ui/screens](../../ui/screens/); đó là mốc một phần, chưa bao
phủ tài liệu, zoom và DPI theo yêu cầu. Script hiện có:
`node scripts/capture-desktop-ui.mjs --out docs/visual/screens/<ten-bo>`; script này hiện
chỉ chụp khung/ribbon và một số menu, dialog, chưa phải bộ chụp tài liệu đầy đủ.
`node scripts/capture-visual-document.mjs --exe <ban-webdriver> --document <tep.docx> --out <thu-muc>`
chụp BlueDoc light/dark ở bốn mức zoom trên DPI hiện tại, dùng hồ sơ WebDriver cách ly và ghi
manifest. Nó chưa chụp các ứng dụng hay DPI khác.
