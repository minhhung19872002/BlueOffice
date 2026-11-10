#!/usr/bin/env node
// Generates the insertable icon library (CORE-ICON-001): assets/icons/library/<id>.svg,
// assets/icons/library/manifest.json and packages/ui/src/iconLibrary/data.json (the same
// manifest and SVG documents in one file: the lazily loaded chunk of the icon dialog).
//
//   node scripts/gen-icon-library.mjs
//
// Every icon is an ORIGINAL drawing of the BlueOffice project, written by hand below on a
// 24 × 24 grid — no third-party icon set is used or traced. The drawings only use what the
// BlueOffice SVG reader converts (`bo_drawingml::svg`: path, rect, circle, ellipse, solid
// fills and strokes, `transform`), with explicit colours on every element (no `currentColor`,
// no CSS, no gradients, no `use`), so they survive "convert to shapes" and the PNG fallback.
import { mkdir, readdir, rm, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const OUT = join(dirname(fileURLToPath(import.meta.url)), '..', 'assets', 'icons', 'library');

// Palette of the set.
const D = '#1F3A5F'; // ink
const L = '#CFE0F3'; // light tone
const W = '#FFFFFF';
const R = '#D9534F';
const Y = '#F2C14E';
const G = '#5BAA6E';

const num = (v) => String(Math.round(v * 1000) / 1000);
const el = (tag, attrs) =>
  `<${tag} ${Object.entries(attrs)
    .filter(([, v]) => v !== undefined)
    .map(([k, v]) => `${k}="${typeof v === 'number' ? num(v) : v}"`)
    .join(' ')}/>`;
/** Fill + outline attributes; `o`: { s: stroke colour, w: stroke width, t: transform }. */
const paint = (f, o = {}) => ({
  fill: f,
  stroke: o.s ?? D,
  'stroke-width': o.s === 'none' ? undefined : (o.w ?? 1.5),
  transform: o.t,
});
/** Filled, outlined path. */
const P = (d, f = L, o) => el('path', { d, ...paint(f, o) });
/** Outline only. */
const S = (d, o) => el('path', { d, ...paint('none', o) });
/** Fill only. */
const F = (d, f = D, o = {}) => el('path', { d, ...paint(f, { ...o, s: 'none' }) });
const Rc = (x, y, width, height, rx = 0, f = L, o) =>
  el('rect', { x, y, width, height, rx: rx || undefined, ...paint(f, o) });
const C = (cx, cy, r, f = L, o) => el('circle', { cx, cy, r, ...paint(f, o) });
/** Dot: a filled circle without outline. */
const Dot = (cx, cy, r = 1, f = D) => el('circle', { cx, cy, r, ...paint(f, { s: 'none' }) });
const E = (cx, cy, rx, ry, f = L, o) => el('ellipse', { cx, cy, rx, ry, ...paint(f, o) });

const CATEGORIES = [
  ['arrows', 'Mũi tên', 'Arrows'],
  ['people', 'Con người', 'People'],
  ['business', 'Kinh doanh', 'Business'],
  ['communication', 'Liên lạc', 'Communication'],
  ['technology', 'Công nghệ', 'Technology'],
  ['education', 'Giáo dục', 'Education'],
  ['medical', 'Y tế', 'Medical'],
  ['transport', 'Giao thông', 'Transport'],
  ['weather', 'Thời tiết', 'Weather'],
  ['nature', 'Thiên nhiên', 'Nature'],
  ['food', 'Ẩm thực', 'Food'],
  ['buildings', 'Công trình', 'Buildings'],
  ['signs', 'Ký hiệu', 'Signs and symbols'],
  ['charts', 'Biểu đồ', 'Charts'],
  ['admin', 'Hành chính', 'Administration'],
];

/** [id, category, Vietnamese name, English name, Vietnamese keywords, English keywords, body] */
const ICONS = [];
let category = '';
const cat = (id) => {
  category = id;
};
const icon = (id, vi, en, kwVi, kwEn, ...body) =>
  ICONS.push({ id, category, vi, en, kwVi, kwEn, body: body.join('') });

// Shared pieces.
const AXES = S('M3 3v18h18');
const CLOUD = (f = W) => P('M7 15a4.5 4.5 0 0 1-.4-9A5.5 5.5 0 0 1 17.5 7a4 4 0 0 1 .5 8z', f);
const SHEET = P('M5 2h10l5 5v15H5z', W) + P('M15 2v5h5z', L);
const CLIPBOARD = Rc(4, 4, 16, 18, 2) + Rc(8, 2, 8, 4, 1, W);
const CALENDAR =
  Rc(3, 5, 18, 16, 2, W) + P('M3 10V7a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2v3z', R) + S('M8 3v4M16 3v4');
const BOWL = P('M2 11h20a10 10 0 0 1-6 9H8a10 10 0 0 1-6-9z');

// ---------------------------------------------------------------- arrows
cat('arrows');
icon(
  'arrow-right',
  'Mũi tên phải',
  'Arrow right',
  'mũi tên, phải, tiếp, sang phải',
  'arrow, right, next, forward',
  P('M3 10h11V6l7 6-7 6v-4H3z'),
);
icon(
  'arrow-left',
  'Mũi tên trái',
  'Arrow left',
  'mũi tên, trái, lùi, quay lại',
  'arrow, left, back, previous',
  P('M21 10H10V6l-7 6 7 6v-4h11z'),
);
icon(
  'arrow-up',
  'Mũi tên lên',
  'Arrow up',
  'mũi tên, lên, tăng',
  'arrow, up, increase',
  P('M10 21V10H6l6-7 6 7h-4v11z'),
);
icon(
  'arrow-down',
  'Mũi tên xuống',
  'Arrow down',
  'mũi tên, xuống, giảm, tải về',
  'arrow, down, decrease, download',
  P('M10 3v11H6l6 7 6-7h-4V3z'),
);
icon(
  'arrow-up-right',
  'Mũi tên chéo lên',
  'Arrow up right',
  'mũi tên, chéo, lên, tăng trưởng',
  'arrow, diagonal, up, growth',
  P('M8 5h11v11l-4-4-8 8-3-3 8-8z'),
);
icon(
  'arrow-down-right',
  'Mũi tên chéo xuống',
  'Arrow down right',
  'mũi tên, chéo, xuống, giảm',
  'arrow, diagonal, down, decline',
  P('M8 19h11V8l-4 4-8-8-3 3 8 8z'),
);
icon(
  'arrows-left-right',
  'Mũi tên hai chiều ngang',
  'Arrows left and right',
  'mũi tên, hai chiều, ngang, trao đổi',
  'arrow, both, horizontal, exchange, swap',
  P('M2 12l5-5v3h10V7l5 5-5 5v-3H7v3z'),
);
icon(
  'arrows-up-down',
  'Mũi tên hai chiều dọc',
  'Arrows up and down',
  'mũi tên, hai chiều, dọc, lên xuống',
  'arrow, both, vertical, up, down',
  P('M12 2l5 5h-3v10h3l-5 5-5-5h3V7H7z'),
);
icon(
  'arrow-turn-right',
  'Mũi tên rẽ phải',
  'Arrow turn right',
  'mũi tên, rẽ, quẹo, chuyển hướng',
  'arrow, turn, bend, redirect',
  P('M4 20v-9a3 3 0 0 1 3-3h7V4l7 6-7 6v-4H8v8z'),
);
icon(
  'arrow-return',
  'Mũi tên quay lại',
  'Arrow return',
  'mũi tên, quay lại, hoàn tác, trở về',
  'arrow, return, undo, u-turn, back',
  P('M9 4L3 9l6 5v-3h6a3 3 0 0 1 0 6H8v4h7a7 7 0 0 0 0-14H9z'),
);
icon(
  'arrows-rotate',
  'Mũi tên xoay vòng',
  'Arrows rotate',
  'mũi tên, xoay, làm mới, lặp lại, chu trình',
  'arrow, rotate, refresh, repeat, cycle',
  S('M5 12a7 7 0 0 1 12-5M19 12a7 7 0 0 1-12 5', { w: 2 }),
  P('M18 3v5h-5z', D),
  P('M6 21v-5h5z', D),
);
icon(
  'chevrons-right',
  'Mũi tên kép',
  'Double chevron',
  'mũi tên, kép, tiến trình, bước tiếp',
  'chevron, double, process, step, next',
  P('M3 5h4l6 7-6 7H3l6-7z'),
  P('M11 5h4l6 7-6 7h-4l6-7z', D),
);
icon(
  'arrows-expand',
  'Mũi tên bốn hướng',
  'Arrows four directions',
  'mũi tên, bốn hướng, di chuyển, mở rộng',
  'arrow, four, move, expand, directions',
  P('M12 2l3 4h-2v5h5V9l4 3-4 3v-2h-5v5h2l-3 4-3-4h2v-5H6v2l-4-3 4-3v2h5V6H9z'),
);
icon(
  'arrow-circle-right',
  'Mũi tên trong vòng tròn',
  'Arrow in circle',
  'mũi tên, vòng tròn, tiếp tục, đi tới',
  'arrow, circle, continue, go',
  C(12, 12, 10),
  P('M7 10.5h6V8l5 4-5 4v-2.5H7z', D),
);

// ---------------------------------------------------------------- people
cat('people');
icon(
  'person',
  'Người',
  'Person',
  'người, cá nhân, người dùng, nhân viên',
  'person, user, individual, employee',
  C(12, 7, 4),
  P('M4 21a8 8 0 0 1 16 0z'),
);
icon(
  'people',
  'Hai người',
  'Two people',
  'người, hai người, nhóm, đồng nghiệp',
  'people, two, group, colleagues',
  C(16.5, 9, 2.5, W),
  P('M12 20a5 5 0 0 1 10 0z', W),
  C(8.5, 8, 3),
  P('M2 20a6.5 6.5 0 0 1 13 0z'),
);
icon(
  'team',
  'Nhóm người',
  'Team',
  'nhóm, đội, tập thể, phòng ban, nhân sự',
  'team, group, staff, department, crew',
  C(5, 9, 2.5, W),
  P('M1 19a4 4 0 0 1 8 0z', W),
  C(19, 9, 2.5, W),
  P('M15 19a4 4 0 0 1 8 0z', W),
  C(12, 7, 3),
  P('M6 21a6 6 0 0 1 12 0z'),
);
icon(
  'person-add',
  'Thêm người',
  'Add person',
  'thêm người, tuyển dụng, thành viên mới',
  'add person, recruit, new member, invite',
  C(10, 7, 4),
  P('M2 21a8 8 0 0 1 16 0z'),
  S('M19 3v6M16 6h6', { w: 2, s: G }),
);
icon(
  'person-check',
  'Người đã xác nhận',
  'Person confirmed',
  'người, xác nhận, duyệt, điểm danh',
  'person, confirmed, approved, attendance',
  C(10, 7, 4),
  P('M2 21a8 8 0 0 1 16 0z'),
  S('M16 6l2 2 4-4', { w: 2, s: G }),
);
icon(
  'id-card',
  'Thẻ căn cước',
  'Identity card',
  'thẻ, căn cước, chứng minh, thẻ nhân viên, danh tính',
  'id, card, identity, badge, employee card',
  Rc(2, 5, 20, 14, 2),
  C(8, 10.5, 2.2, W),
  P('M4.5 16.5a3.5 3.5 0 0 1 7 0z', W),
  S('M14 10h5M14 13h5M14 16h3'),
);
icon(
  'presenter',
  'Người thuyết trình',
  'Presenter',
  'thuyết trình, giảng viên, báo cáo viên, đào tạo',
  'presenter, speaker, trainer, lecture',
  Rc(9, 3, 13, 10, 1, W),
  S('M12 6.5h7M12 9.5h4M15.5 13v4'),
  C(5, 9, 2.5),
  P('M1 20a4 4 0 0 1 8 0z'),
);
icon(
  'meeting',
  'Cuộc họp',
  'Meeting',
  'họp, cuộc họp, hội nghị, thảo luận',
  'meeting, conference, discussion, table',
  C(6, 6, 2.5),
  P('M3 12.5a3 3 0 0 1 6 0z'),
  C(18, 6, 2.5),
  P('M15 12.5a3 3 0 0 1 6 0z'),
  S('M6 16v5M18 16v5'),
  Rc(2, 12.5, 20, 3.5, 1, W),
);
icon(
  'accessibility',
  'Trợ năng',
  'Accessibility',
  'trợ năng, tiếp cận, hình người, khuyết tật',
  'accessibility, human figure, inclusive',
  C(12, 12, 10),
  Dot(12, 6.5, 1.7),
  S('M6.5 10h11M12 10v5M12 15l-3 4.5M12 15l3 4.5', { w: 1.8 }),
);
icon(
  'worker',
  'Công nhân',
  'Worker',
  'công nhân, kỹ sư, lao động, xây dựng, bảo hộ',
  'worker, engineer, labour, construction, helmet',
  P('M4 22a8 8 0 0 1 16 0z'),
  C(12, 11, 4, W),
  P('M6.5 9.5a5.5 5.5 0 0 1 11 0z', Y),
  S('M5 9.5h14'),
);
icon(
  'user-circle',
  'Ảnh đại diện',
  'Avatar',
  'ảnh đại diện, tài khoản, hồ sơ cá nhân',
  'avatar, account, profile, user',
  C(12, 12, 10),
  C(12, 9, 3, W),
  P('M6 18a6.5 6.5 0 0 1 12 0 9 9 0 0 1-12 0z', W),
);

// ---------------------------------------------------------------- business
cat('business');
icon(
  'briefcase',
  'Cặp tài liệu',
  'Briefcase',
  'cặp, công việc, kinh doanh, nghề nghiệp',
  'briefcase, work, business, job, career',
  S('M9 7V5a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2'),
  Rc(2, 7, 20, 13, 2),
  S('M2 13h20'),
  Rc(10.5, 11.5, 3, 3, 0.5, W),
);
icon(
  'banknote',
  'Tiền giấy',
  'Banknote',
  'tiền, tiền mặt, thanh toán, lương',
  'money, cash, banknote, payment, salary',
  Rc(2, 6, 20, 12, 1.5, '#D5EBD9'),
  C(12, 12, 3, W),
  Dot(5.5, 12),
  Dot(18.5, 12),
);
icon(
  'coins',
  'Đồng xu',
  'Coins',
  'xu, tiền, tiết kiệm, tài chính, ngân sách',
  'coins, money, savings, finance, budget',
  P('M5 12v5c0 1.7 3.1 3 7 3s7-1.3 7-3v-5z', Y),
  P('M5 7v5c0 1.7 3.1 3 7 3s7-1.3 7-3V7z', Y),
  E(12, 7, 7, 3, '#F9E2A6'),
);
icon(
  'wallet',
  'Ví tiền',
  'Wallet',
  'ví, tiền, chi tiêu, thanh toán',
  'wallet, money, spending, payment',
  S('M5 6l10-3v3'),
  Rc(2, 6, 19, 14, 2),
  P('M15 11h7v5h-7a2.5 2.5 0 0 1 0-5z', W),
  Dot(16, 13.5, 0.9),
);
icon(
  'credit-card',
  'Thẻ ngân hàng',
  'Bank card',
  'thẻ, ngân hàng, tín dụng, thanh toán',
  'card, bank, credit, debit, payment',
  Rc(2, 5, 20, 14, 2),
  F('M2 9h20v3H2z'),
  S('M5 16h5'),
);
icon(
  'target',
  'Mục tiêu',
  'Target',
  'mục tiêu, đích, chỉ tiêu, kế hoạch',
  'target, goal, objective, aim, bullseye',
  C(12, 12, 10, W),
  C(12, 12, 6.5, R),
  C(12, 12, 3, W),
  Dot(12, 12, 1),
);
icon(
  'calculator',
  'Máy tính bỏ túi',
  'Calculator',
  'máy tính, tính toán, kế toán, phép tính',
  'calculator, compute, accounting, math',
  Rc(4, 2, 16, 20, 2),
  Rc(7, 5, 10, 4, 0.5, W),
  Dot(8, 13),
  Dot(12, 13),
  Dot(16, 13),
  Dot(8, 17),
  Dot(12, 17),
  Dot(16, 17),
);
icon(
  'shopping-cart',
  'Giỏ hàng',
  'Shopping cart',
  'giỏ hàng, mua sắm, bán hàng, đơn hàng',
  'cart, shopping, sales, order, purchase',
  S('M2 4h3l1.5 3'),
  P('M6.5 7H21l-2 8H8z'),
  C(9.5, 19, 1.5, W),
  C(17.5, 19, 1.5, W),
);
icon(
  'price-tag',
  'Nhãn giá',
  'Price tag',
  'nhãn, giá, khuyến mãi, giảm giá',
  'tag, price, label, sale, discount',
  P('M3 3h8l10 10-8 8L3 11z', Y),
  C(7.5, 7.5, 1.5, W),
);
icon(
  'trophy',
  'Cúp',
  'Trophy',
  'cúp, giải thưởng, chiến thắng, thành tích',
  'trophy, award, winner, achievement, cup',
  S('M7 5H4a3 3 0 0 0 3 4M17 5h3a3 3 0 0 1-3 4M12 14v4'),
  P('M7 3h10v6a5 5 0 0 1-10 0z', Y),
  P('M8 18h8v3H8z'),
);
icon(
  'balance-scale',
  'Cán cân',
  'Balance scale',
  'cán cân, công lý, pháp luật, so sánh, cân bằng',
  'scale, balance, justice, law, compare',
  S('M12 3v18M5 7h14M7 21h10'),
  P('M2 14l3-7 3 7a3 3 0 0 1-6 0z'),
  P('M16 14l3-7 3 7a3 3 0 0 1-6 0z'),
);
icon(
  'light-bulb',
  'Bóng đèn',
  'Light bulb',
  'bóng đèn, ý tưởng, sáng kiến, giải pháp',
  'bulb, idea, innovation, solution, tip',
  P('M8 15a6 6 0 1 1 8 0v2H8z', Y),
  S('M9 20h6M10.5 22.5h3'),
);
icon(
  'gift',
  'Hộp quà',
  'Gift',
  'quà, quà tặng, khuyến mãi, phần thưởng',
  'gift, present, bonus, reward',
  S('M12 7c-2-4.5-6.5-3-4 0M12 7c2-4.5 6.5-3 4 0', { s: R }),
  Rc(3, 11, 18, 10, 1),
  Rc(2, 7, 20, 4, 1, R),
  S('M12 7v14'),
);
icon(
  'key',
  'Chìa khoá',
  'Key',
  'chìa khoá, khoá, bảo mật, quyền truy cập',
  'key, lock, security, access',
  P('M11 10.5h11v3h-2v2.5h-3v-2.5h-6z', Y),
  C(7.5, 12, 5, Y),
  C(7.5, 12, 1.5, W),
);

// ---------------------------------------------------------------- communication
cat('communication');
icon(
  'mail',
  'Thư',
  'Mail',
  'thư, email, thư điện tử, phong bì',
  'mail, email, envelope, letter, message',
  Rc(2, 5, 20, 14, 2),
  S('M2.5 6.5l9.5 7 9.5-7'),
);
icon(
  'mail-open',
  'Thư đã mở',
  'Open mail',
  'thư, đã mở, đã đọc, email',
  'mail, open, read, email',
  P('M2 10l10-7 10 7v10a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1z'),
  S('M2 10l10 6 10-6'),
);
icon(
  'phone',
  'Điện thoại',
  'Phone',
  'điện thoại, gọi, liên hệ, đường dây nóng',
  'phone, call, contact, hotline, telephone',
  P('M5 3h4l2 5-2.5 1.5a11 11 0 0 0 6 6L16 13l5 2v4a2 2 0 0 1-2 2A16 16 0 0 1 3 5a2 2 0 0 1 2-2z'),
);
icon(
  'chat',
  'Lời nhắn',
  'Chat bubble',
  'tin nhắn, trò chuyện, bình luận, ý kiến',
  'chat, message, comment, speech, bubble',
  P('M3 4h18v12H10l-5 4v-4H3z'),
  S('M7 8.5h10M7 11.5h6'),
);
icon(
  'chats',
  'Trao đổi',
  'Conversation',
  'trao đổi, thảo luận, hội thoại, phản hồi',
  'conversation, discussion, dialogue, feedback',
  P('M2 3h13v9H8l-3 3v-3H2z'),
  P('M10 9h12v9h-2v3l-4-3h-6z', W),
);
icon(
  'megaphone',
  'Loa thông báo',
  'Megaphone',
  'loa, thông báo, tuyên truyền, quảng cáo',
  'megaphone, announcement, promotion, broadcast',
  P('M6 14l1 6h3l-1-6z', W),
  P('M3 10h4l10-5v14L7 14H3z'),
  S('M19.5 9a4 4 0 0 1 0 6'),
);
icon(
  'bell',
  'Chuông',
  'Bell',
  'chuông, thông báo, nhắc nhở, cảnh báo',
  'bell, notification, reminder, alert',
  S('M12 2.5v2.5M10 20.5a2 2 0 0 0 4 0'),
  P('M6 17v-6a6 6 0 0 1 12 0v6l2 2H4z', Y),
);
icon(
  'send',
  'Gửi đi',
  'Send',
  'gửi, máy bay giấy, chuyển đi, phát hành',
  'send, paper plane, submit, dispatch',
  P('M2 11l20-8-7 18-3-7z'),
  S('M12 14L22 3'),
);
icon(
  'globe',
  'Địa cầu',
  'Globe',
  'địa cầu, thế giới, quốc tế, internet, web',
  'globe, world, international, internet, web',
  C(12, 12, 10),
  S('M2 12h20M12 2c3 3 4 6 4 10s-1 7-4 10M12 2c-3 3-4 6-4 10s1 7 4 10'),
);
icon(
  'link',
  'Liên kết',
  'Link',
  'liên kết, đường dẫn, kết nối, xích',
  'link, url, connect, chain, hyperlink',
  S(
    'M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1',
    { w: 2 },
  ),
);
icon(
  'share',
  'Chia sẻ',
  'Share',
  'chia sẻ, phân phối, mạng lưới, kết nối',
  'share, distribute, network, connect',
  S('M8.5 10.5l7-4M8.5 13.5l7 4'),
  C(18, 5, 3),
  C(6, 12, 3),
  C(18, 19, 3),
);
icon(
  'video-call',
  'Họp trực tuyến',
  'Video call',
  'họp trực tuyến, video, máy quay, hội nghị truyền hình',
  'video call, online meeting, camera, conference',
  Rc(2, 6, 14, 12, 2),
  P('M16 10l6-3v10l-6-3z'),
);
icon(
  'newspaper',
  'Báo chí',
  'Newspaper',
  'báo, tin tức, bản tin, truyền thông',
  'newspaper, news, bulletin, media, press',
  Rc(3, 4, 18, 16, 1, W),
  Rc(6, 7, 5, 5, 0, L),
  S('M13.5 8h4.5M13.5 11h4.5M6 15h12M6 17.5h12'),
);

// ---------------------------------------------------------------- technology
cat('technology');
icon(
  'laptop',
  'Máy tính xách tay',
  'Laptop',
  'máy tính xách tay, laptop, làm việc từ xa',
  'laptop, notebook, computer, remote work',
  Rc(4, 5, 16, 11, 1.5),
  Rc(6.5, 7.5, 11, 6, 0, W),
  P('M2 17h20l-1.5 3h-17z'),
);
icon(
  'desktop',
  'Màn hình máy tính',
  'Desktop monitor',
  'màn hình, máy tính để bàn, máy vi tính',
  'monitor, desktop, computer, screen, display',
  S('M12 17v3.5M8 21h8'),
  Rc(2, 4, 20, 13, 1.5),
  Rc(4.5, 6.5, 15, 8, 0, W),
);
icon(
  'smartphone',
  'Điện thoại thông minh',
  'Smartphone',
  'điện thoại, di động, ứng dụng, smartphone',
  'smartphone, mobile, app, cell phone',
  Rc(7, 2, 10, 20, 2),
  Rc(9, 5, 6, 11, 0, W),
  Dot(12, 19, 0.9),
);
icon(
  'tablet',
  'Máy tính bảng',
  'Tablet',
  'máy tính bảng, tablet, thiết bị',
  'tablet, device, pad',
  Rc(4, 3, 16, 18, 2),
  Rc(6.5, 5.5, 11, 11.5, 0, W),
  Dot(12, 19, 0.8),
);
icon(
  'printer',
  'Máy in',
  'Printer',
  'máy in, in ấn, bản in, văn phòng',
  'printer, print, hard copy, office',
  Rc(6, 3, 12, 5, 0.5, W),
  Rc(3, 8, 18, 9, 1.5),
  Rc(7, 14, 10, 7, 0.5, W),
  Dot(17.5, 11, 0.9),
);
icon(
  'wifi',
  'Wi-Fi',
  'Wi-Fi',
  'wifi, mạng không dây, sóng, kết nối',
  'wifi, wireless, network, signal, connection',
  S('M2.5 9a14 14 0 0 1 19 0M5.5 12.5a9.5 9.5 0 0 1 13 0M8.5 16a5 5 0 0 1 7 0', { w: 2 }),
  Dot(12, 19.5, 1.3),
);
icon(
  'cloud',
  'Đám mây',
  'Cloud',
  'đám mây, lưu trữ, điện toán đám mây',
  'cloud, storage, cloud computing',
  P('M7 19a5 5 0 0 1-.5-9.9A6 6 0 0 1 18 10.5a4.3 4.3 0 0 1 0 8.5z'),
);
icon(
  'database',
  'Cơ sở dữ liệu',
  'Database',
  'cơ sở dữ liệu, dữ liệu, lưu trữ, kho dữ liệu',
  'database, data, storage, warehouse',
  P('M4 6v12c0 1.7 3.6 3 8 3s8-1.3 8-3V6z'),
  S('M4 12c0 1.7 3.6 3 8 3s8-1.3 8-3'),
  E(12, 6, 8, 3, W),
);
icon(
  'server',
  'Máy chủ',
  'Server',
  'máy chủ, hạ tầng, trung tâm dữ liệu',
  'server, infrastructure, data centre, hosting',
  S('M12 10v4'),
  Rc(3, 3, 18, 7, 1.5),
  Rc(3, 14, 18, 7, 1.5),
  Dot(7, 6.5, 1.1, G),
  Dot(7, 17.5, 1.1, G),
  S('M11 6.5h6M11 17.5h6'),
);
icon(
  'chip',
  'Vi xử lý',
  'Processor chip',
  'vi xử lý, chip, bán dẫn, phần cứng',
  'chip, processor, cpu, semiconductor, hardware',
  S('M9 3v3M15 3v3M9 18v3M15 18v3M3 9h3M3 15h3M18 9h3M18 15h3'),
  Rc(6, 6, 12, 12, 1.5),
  Rc(9.5, 9.5, 5, 5, 0, W),
);
icon(
  'mouse',
  'Chuột máy tính',
  'Computer mouse',
  'chuột, nhấp, thiết bị ngoại vi',
  'mouse, click, peripheral',
  Rc(7, 2, 10, 20, 5),
  S('M12 6v4'),
);
icon(
  'keyboard',
  'Bàn phím',
  'Keyboard',
  'bàn phím, gõ, nhập liệu',
  'keyboard, typing, input',
  Rc(2, 6, 20, 12, 2),
  Dot(6, 10, 0.9),
  Dot(10, 10, 0.9),
  Dot(14, 10, 0.9),
  Dot(18, 10, 0.9),
  Dot(6, 13, 0.9),
  Dot(10, 13, 0.9),
  Dot(14, 13, 0.9),
  Dot(18, 13, 0.9),
  S('M8 16h8'),
);
icon(
  'camera',
  'Máy ảnh',
  'Camera',
  'máy ảnh, chụp ảnh, hình ảnh',
  'camera, photo, picture, photography',
  P('M3 7h4l2-3h6l2 3h4v13H3z'),
  C(12, 13, 4, W),
);
icon(
  'gear',
  'Bánh răng',
  'Gear',
  'bánh răng, cài đặt, thiết lập, cấu hình, kỹ thuật',
  'gear, settings, configuration, engineering, cog',
  ...[0, 45, 90, 135, 180, 225, 270, 315].map((a) =>
    Rc(10.25, 1.5, 3.5, 5, 0.5, L, { t: `rotate(${a} 12 12)` }),
  ),
  C(12, 12, 7),
  C(12, 12, 2.8, W),
);
icon(
  'lock',
  'Ổ khoá',
  'Lock',
  'ổ khoá, bảo mật, an toàn, mật khẩu',
  'lock, security, safe, password, private',
  S('M8 10V7a4 4 0 0 1 8 0v3'),
  Rc(4, 10, 16, 11, 2, Y),
  Dot(12, 15.5, 1.6),
);
icon(
  'shield-check',
  'Khiên bảo vệ',
  'Shield',
  'khiên, bảo vệ, an toàn, bảo hiểm, tin cậy',
  'shield, protection, safety, insurance, trusted',
  P('M12 2l8 3v6c0 5-3.5 9-8 11-4.5-2-8-6-8-11V5z'),
  S('M8 12l3 3 5-6', { w: 2, s: G }),
);
icon(
  'search',
  'Kính lúp',
  'Magnifier',
  'kính lúp, tìm kiếm, tra cứu, phóng to',
  'search, magnifier, find, look up, zoom',
  S('M15.5 15.5L21 21', { w: 2.6 }),
  C(10.5, 10.5, 6.5, W),
);
icon(
  'headphones',
  'Tai nghe',
  'Headphones',
  'tai nghe, hỗ trợ, tổng đài, âm thanh',
  'headphones, support, call centre, audio',
  S('M4 15v-3a8 8 0 0 1 16 0v3'),
  Rc(3, 14, 4, 7, 1.5),
  Rc(17, 14, 4, 7, 1.5),
);

// ---------------------------------------------------------------- education
cat('education');
icon(
  'book-open',
  'Sách mở',
  'Open book',
  'sách, đọc, học tập, tài liệu, giáo trình',
  'book, open, read, study, textbook',
  P('M2 5c3-1 7-1 10 1v14c-3-2-7-2-10-1z'),
  P('M22 5c-3-1-7-1-10 1v14c3-2 7-2 10-1z', W),
);
icon(
  'book',
  'Quyển sách',
  'Book',
  'sách, quyển sách, sổ tay, thư viện',
  'book, closed, manual, library',
  P('M5 3h14v18H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z'),
  S('M7 3v18M10.5 8h5.5'),
);
icon(
  'graduation-cap',
  'Mũ tốt nghiệp',
  'Graduation cap',
  'mũ cử nhân, tốt nghiệp, đại học, học vị',
  'graduation, cap, degree, university, graduate',
  P('M6 11v6c2 2 10 2 12 0v-6z', W),
  P('M12 4L1 9l11 5 11-5z'),
  S('M22 9v6'),
);
icon(
  'pencil',
  'Bút chì',
  'Pencil',
  'bút chì, viết, chỉnh sửa, soạn thảo',
  'pencil, write, edit, draft',
  P('M4 20l1-5L16 4l4 4L9 19z', Y),
  S('M14 6l4 4'),
  F('M4 20l.6-3 2.4 2.4z'),
);
icon(
  'ruler',
  'Thước kẻ',
  'Ruler',
  'thước, đo, kích thước, thiết kế',
  'ruler, measure, dimension, design',
  P('M3 17L17 3l4 4L7 21z', Y),
  S('M7 13l2 2M10 10l2 2M13 7l2 2'),
);
icon(
  'blackboard',
  'Bảng lớp học',
  'Blackboard',
  'bảng, lớp học, giảng dạy, bài giảng',
  'blackboard, classroom, teaching, lesson',
  S('M8 21l2-5M16 21l-2-5'),
  Rc(2, 3, 20, 13, 1, G),
  S('M6 7.5h7M6 11h4', { s: W }),
);
icon(
  'flask',
  'Bình thí nghiệm',
  'Flask',
  'bình thí nghiệm, hoá học, khoa học, nghiên cứu',
  'flask, chemistry, science, research, lab',
  P('M9.5 3h5v6l5.5 10a1.5 1.5 0 0 1-1.4 2H5.4A1.5 1.5 0 0 1 4 19l5.5-10z', W),
  P('M7 14.5h10l2.4 4.5a1.3 1.3 0 0 1-1.2 2H5.8a1.3 1.3 0 0 1-1.2-2z', G),
  S('M8 3h8'),
);
icon(
  'certificate',
  'Bằng chứng nhận',
  'Certificate',
  'bằng, chứng chỉ, chứng nhận, giấy khen',
  'certificate, diploma, award, credential',
  Rc(2, 4, 20, 13, 1, W),
  S('M6 8h8M6 11h5'),
  P('M14.5 15v6.5l2-1.5 2 1.5V15z', R),
  C(16.5, 13.5, 3, Y),
);
icon(
  'atom',
  'Nguyên tử',
  'Atom',
  'nguyên tử, vật lý, khoa học, hạt nhân',
  'atom, physics, science, nuclear',
  E(12, 12, 10, 4, 'none'),
  E(12, 12, 10, 4, 'none', { t: 'rotate(60 12 12)' }),
  E(12, 12, 10, 4, 'none', { t: 'rotate(120 12 12)' }),
  Dot(12, 12, 1.8, R),
);
icon(
  'notebook',
  'Sổ ghi chép',
  'Notebook',
  'sổ, vở, ghi chép, nhật ký',
  'notebook, notes, journal, diary',
  Rc(5, 3, 15, 18, 1.5),
  S('M3 7h4M3 12h4M3 17h4M10.5 8h6.5M10.5 11.5h6.5'),
);
icon(
  'palette',
  'Bảng màu',
  'Palette',
  'bảng màu, mỹ thuật, hội hoạ, thiết kế',
  'palette, art, painting, design, colours',
  P('M12 3a9 9 0 1 0 0 18c1.5 0 2-1 1.5-2-.8-1.6.2-3 2-3H18a3 3 0 0 0 3-3c0-5-4-10-9-10z', W),
  Dot(7.5, 11.5, 1.4, R),
  Dot(10, 7.5, 1.4, Y),
  Dot(14.5, 7.5, 1.4, G),
  Dot(17.5, 11, 1.4, D),
);
icon(
  'compass-drawing',
  'Com-pa',
  'Drawing compass',
  'com-pa, hình học, toán học, vẽ kỹ thuật',
  'compass, geometry, mathematics, drafting',
  S('M12 5.5L6 21M12 5.5L18 21M8 15a8 8 0 0 0 8 0'),
  C(12, 4, 1.8, W),
);

// ---------------------------------------------------------------- medical
cat('medical');
const HEART = 'M12 21C5 15 2 12 2 8a5 5 0 0 1 10-1.5A5 5 0 0 1 22 8c0 4-3 7-10 13z';
icon(
  'heart',
  'Trái tim',
  'Heart',
  'trái tim, tim, yêu thích, sức khoẻ',
  'heart, love, favourite, health',
  P(HEART, R),
);
icon(
  'heart-pulse',
  'Nhịp tim',
  'Heartbeat',
  'nhịp tim, điện tim, tim mạch, sức khoẻ',
  'heartbeat, pulse, cardiology, health, ecg',
  P(HEART, '#F6CFCB'),
  S('M3 12h5l2-4 3 7 2-3h6', { w: 1.8 }),
);
icon(
  'medical-cross',
  'Chữ thập y tế',
  'Medical cross',
  'chữ thập, y tế, cấp cứu, bệnh viện',
  'cross, medical, emergency, hospital, health',
  P('M9 3h6v6h6v6h-6v6H9v-6H3V9h6z', R),
);
icon(
  'first-aid-kit',
  'Túi cứu thương',
  'First aid kit',
  'cứu thương, sơ cứu, túi thuốc, y tế',
  'first aid, kit, emergency, medical bag',
  S('M9 7V5a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2'),
  Rc(2, 7, 20, 13, 2, W),
  F('M11 9.5h2V12h2.5v2H13v2.5h-2V14H8.5v-2H11z', R),
);
icon(
  'pill',
  'Viên thuốc',
  'Pill',
  'thuốc, viên thuốc, dược, đơn thuốc',
  'pill, medicine, drug, pharmacy, prescription',
  Rc(2, 8, 20, 8, 4, W, { t: 'rotate(-45 12 12)' }),
  P('M12 8h6a4 4 0 0 1 0 8h-6z', R, { t: 'rotate(-45 12 12)' }),
);
icon(
  'syringe',
  'Ống tiêm',
  'Syringe',
  'ống tiêm, tiêm chủng, vắc-xin, kim tiêm',
  'syringe, vaccine, injection, needle',
  S('M16.5 1.5l6 6M5 19l-3 3M19.5 4.5L16 8'),
  P('M13 5l6 6-8 8H5v-6z', W),
  S('M9 9l2 2M11.5 6.5l2 2'),
);
icon(
  'stethoscope',
  'Ống nghe',
  'Stethoscope',
  'ống nghe, bác sĩ, khám bệnh, phòng khám',
  'stethoscope, doctor, examination, clinic',
  S('M5 3v6a5 5 0 0 0 10 0V3M10 14v2a5 5 0 0 0 10 0v-2', { w: 1.8 }),
  C(20, 12, 2, W),
  Dot(5, 3, 1.2),
  Dot(15, 3, 1.2),
);
icon(
  'thermometer',
  'Nhiệt kế',
  'Thermometer',
  'nhiệt kế, nhiệt độ, sốt, đo nhiệt',
  'thermometer, temperature, fever',
  P('M10 4a2 2 0 0 1 4 0v9.5a4.5 4.5 0 1 1-4 0z', W),
  S('M12 16V8', { s: R, w: 1.8 }),
  Dot(12, 17, 2.3, R),
);
icon(
  'tooth',
  'Răng',
  'Tooth',
  'răng, nha khoa, nha sĩ, răng miệng',
  'tooth, dental, dentist, oral care',
  P(
    'M7 3c2 0 3 1 5 1s3-1 5-1c3 0 4 3 3.5 6-.4 2.5-1.5 4-2 7-.3 2-.8 5-2 5-1.5 0-1-5-4.5-5S9 21 7.5 21c-1.2 0-1.7-3-2-5-.5-3-1.6-4.5-2-7C3 6 4 3 7 3z',
    W,
  ),
);
icon(
  'bandage',
  'Băng cá nhân',
  'Bandage',
  'băng, băng cá nhân, vết thương, sơ cứu',
  'bandage, plaster, wound, first aid',
  Rc(1, 8, 22, 8, 4, '#F3D9B8', { t: 'rotate(-45 12 12)' }),
  Rc(8.5, 8, 7, 8, 0, W, { t: 'rotate(-45 12 12)' }),
  Dot(12, 12, 0.9),
);
icon(
  'eye',
  'Con mắt',
  'Eye',
  'mắt, nhìn, thị lực, nhãn khoa, xem',
  'eye, view, vision, ophthalmology, watch',
  P('M1 12c3-5 7-7 11-7s8 2 11 7c-3 5-7 7-11 7S4 17 1 12z', W),
  C(12, 12, 4),
  Dot(12, 12, 1.6),
);
icon(
  'blood-drop',
  'Giọt máu',
  'Blood drop',
  'máu, giọt máu, hiến máu, xét nghiệm',
  'blood, drop, donation, test',
  P('M12 2c4 5 7 8.5 7 12a7 7 0 0 1-14 0c0-3.5 3-7 7-12z', R),
);
icon(
  'clipboard-medical',
  'Bệnh án',
  'Medical record',
  'bệnh án, hồ sơ bệnh nhân, phiếu khám',
  'medical record, patient chart, clipboard',
  CLIPBOARD,
  F('M11 10h2v3h3v2h-3v3h-2v-3H8v-2h3z', R),
);

// ---------------------------------------------------------------- transport
cat('transport');
icon(
  'car',
  'Ô tô',
  'Car',
  'ô tô, xe hơi, xe con, phương tiện',
  'car, automobile, vehicle',
  P('M2 16v-3l3-1 2-4h8l4 4 3 1v3z'),
  P('M8 9h3v3H6.5zM12.5 9h2.2l3 3h-5.2z', W, { w: 1 }),
  C(7, 16.5, 2.2, W),
  C(17, 16.5, 2.2, W),
);
icon(
  'bus',
  'Xe buýt',
  'Bus',
  'xe buýt, xe khách, giao thông công cộng',
  'bus, coach, public transport',
  Rc(3, 4, 18, 14, 2, Y),
  Rc(5, 6.5, 14, 5, 0.5, W),
  S('M3 14.5h18'),
  C(7.5, 18.5, 1.8, W),
  C(16.5, 18.5, 1.8, W),
);
icon(
  'truck',
  'Xe tải',
  'Truck',
  'xe tải, vận chuyển, giao hàng, hậu cần',
  'truck, lorry, delivery, shipping, logistics',
  Rc(1, 6, 13, 10, 1),
  P('M14 9h4l4 4v3h-8z', W),
  C(6, 17, 2.2, W),
  C(17.5, 17, 2.2, W),
);
icon(
  'motorbike',
  'Xe máy',
  'Motorbike',
  'xe máy, mô tô, xe gắn máy, giao hàng',
  'motorbike, motorcycle, scooter, delivery',
  S('M5 16l4-6h5l5 6M14 10l-2-4H9.5M9 10h7', { w: 1.8 }),
  C(5, 16, 3.5, W),
  C(19, 16, 3.5, W),
);
icon(
  'bicycle',
  'Xe đạp',
  'Bicycle',
  'xe đạp, đạp xe, thể thao',
  'bicycle, bike, cycling, sport',
  C(5.5, 16, 4, W),
  C(18.5, 16, 4, W),
  S('M5.5 16l4-8h6l3 8M9.5 8l3.5 8h-7.5M15.5 8l-1-3H12M8 8h3'),
);
icon(
  'airplane',
  'Máy bay',
  'Airplane',
  'máy bay, hàng không, chuyến bay, du lịch',
  'airplane, plane, flight, aviation, travel',
  P(
    'M12 2c1 0 1.5 1 1.5 2.5V9l8.5 5v2.5L13.5 14v4.5l2.5 2V22l-4-1-4 1v-1.5l2.5-2V14L2 16.5V14l8.5-5V4.5C10.5 3 11 2 12 2z',
  ),
);
icon(
  'ship',
  'Tàu thuỷ',
  'Ship',
  'tàu thuỷ, tàu biển, hàng hải, cảng',
  'ship, boat, vessel, maritime, port',
  S('M12 9V4M9 6h6'),
  P('M6 15V9h12v6z', W),
  P('M3 15h18l-2.5 6h-13z'),
);
icon(
  'train',
  'Tàu hoả',
  'Train',
  'tàu hoả, xe lửa, đường sắt, tàu điện',
  'train, railway, rail, metro',
  S('M8 18l-2 4M16 18l2 4'),
  Rc(5, 2, 14, 16, 3),
  Rc(7.5, 5, 9, 5, 0.5, W),
  Dot(9, 14.5),
  Dot(15, 14.5),
);
icon(
  'fuel-pump',
  'Trạm xăng',
  'Fuel pump',
  'xăng, trạm xăng, nhiên liệu, đổ xăng',
  'fuel, petrol, gas station, pump',
  S('M14 10h2.5a1.5 1.5 0 0 1 1.5 1.5V16a1.5 1.5 0 0 0 3 0V8l-2.5-2.5M2 21h14'),
  Rc(4, 3, 10, 18, 1),
  Rc(6, 5.5, 6, 4, 0, W),
);
icon(
  'map-pin',
  'Ghim vị trí',
  'Map pin',
  'vị trí, địa điểm, ghim bản đồ, địa chỉ',
  'pin, location, place, map marker, address',
  P('M12 22c5-6 7-9.5 7-13a7 7 0 0 0-14 0c0 3.5 2 7 7 13z', R),
  C(12, 9, 2.5, W),
);
icon(
  'map',
  'Bản đồ',
  'Map',
  'bản đồ, chỉ đường, lộ trình, địa lý',
  'map, directions, route, geography',
  P('M2 6l6-2 8 2 6-2v14l-6 2-8-2-6 2z', '#D5EBD9'),
  S('M8 4v14M16 6v14'),
);
icon(
  'traffic-light',
  'Đèn giao thông',
  'Traffic light',
  'đèn giao thông, đèn tín hiệu, trạng thái',
  'traffic light, signal, status',
  Rc(8, 2, 8, 20, 3, D),
  Dot(12, 6.5, 2, R),
  Dot(12, 12, 2, Y),
  Dot(12, 17.5, 2, G),
);
icon(
  'rocket',
  'Tên lửa',
  'Rocket',
  'tên lửa, khởi động, ra mắt, khởi nghiệp',
  'rocket, launch, startup, release',
  F('M10.5 18h3l-1.5 4.5z', R),
  P('M7 13l-3 3v3l4-2z', R),
  P('M17 13l3 3v3l-4-2z', R),
  P('M12 2c4 3 5 7 5 11l-2 4H9l-2-4c0-4 1-8 5-11z', W),
  C(12, 9, 2),
);
icon(
  'ambulance',
  'Xe cứu thương',
  'Ambulance',
  'xe cứu thương, cấp cứu, 115',
  'ambulance, emergency, rescue',
  Rc(1, 6, 13, 10, 1, W),
  P('M14 9h4l4 4v3h-8z'),
  F('M6.5 8h2v2h2v2h-2v2h-2v-2h-2v-2h2z', R),
  C(6, 17, 2.2, W),
  C(17.5, 17, 2.2, W),
);

// ---------------------------------------------------------------- weather
cat('weather');
icon(
  'sun',
  'Mặt trời',
  'Sun',
  'mặt trời, nắng, ban ngày, thời tiết',
  'sun, sunny, day, weather',
  S(
    'M12 1.5v3M12 19.5v3M1.5 12h3M19.5 12h3M4.6 4.6l2.1 2.1M17.3 17.3l2.1 2.1M4.6 19.4l2.1-2.1M17.3 6.7l2.1-2.1',
    { s: '#C98A1B', w: 1.8 },
  ),
  C(12, 12, 4.5, Y),
);
icon(
  'moon',
  'Mặt trăng',
  'Moon',
  'mặt trăng, trăng, ban đêm, tối',
  'moon, night, dark, crescent',
  P('M20 14.5A9 9 0 1 1 9.5 4a7 7 0 0 0 10.5 10.5z', Y),
);
icon(
  'cloud-sun',
  'Trời nhiều mây',
  'Partly cloudy',
  'nhiều mây, nắng nhẹ, mây, thời tiết',
  'partly cloudy, cloud, sun, weather',
  S('M8 1.5v2M1.5 8h2M3.4 3.4l1.4 1.4M12.6 3.4l-1.4 1.4', { s: '#C98A1B' }),
  C(8, 8, 3.5, Y),
  P('M9 20a4 4 0 0 1-.4-8A5 5 0 0 1 18 12.5a3.8 3.8 0 0 1 0 7.5z', W),
);
icon(
  'rain',
  'Mưa',
  'Rain',
  'mưa, trời mưa, mưa rào, thời tiết',
  'rain, rainy, shower, weather',
  CLOUD(),
  S('M8 18l-1 3M12 18l-1 3M16 18l-1 3', { s: '#3F7FBF', w: 1.8 }),
);
icon(
  'storm',
  'Dông sét',
  'Thunderstorm',
  'dông, sét, bão, sấm chớp',
  'storm, thunder, lightning, tempest',
  CLOUD(L),
  P('M13 12l-4 5h3l-1 5 4-6h-3z', Y),
);
icon(
  'snowflake',
  'Bông tuyết',
  'Snowflake',
  'tuyết, bông tuyết, lạnh, mùa đông',
  'snow, snowflake, cold, winter, freeze',
  S('M12 2v20M3.3 7l17.4 10M3.3 17l17.4-10M12 5.5l-2-2M12 5.5l2-2M12 18.5l-2 2M12 18.5l2 2', {
    s: '#3F7FBF',
    w: 1.6,
  }),
);
icon(
  'wind',
  'Gió',
  'Wind',
  'gió, gió thổi, không khí, thời tiết',
  'wind, breeze, air, weather',
  S('M2 9h11a3 3 0 1 0-3-3M2 13h16a3 3 0 1 1-3 3M2 17h8', { w: 1.8 }),
);
icon(
  'umbrella',
  'Cái ô',
  'Umbrella',
  'ô, dù, che mưa, bảo hiểm',
  'umbrella, rain cover, insurance',
  S('M12 12v7a2 2 0 0 1-4 0M12 2v1.5'),
  P('M2 12a10 10 0 0 1 20 0z', R),
);
icon(
  'rainbow',
  'Cầu vồng',
  'Rainbow',
  'cầu vồng, sau mưa, màu sắc',
  'rainbow, colours, after rain',
  S('M3 18a9 9 0 0 1 18 0', { s: R, w: 2.4 }),
  S('M6 18a6 6 0 0 1 12 0', { s: Y, w: 2.4 }),
  S('M9 18a3 3 0 0 1 6 0', { s: G, w: 2.4 }),
);
icon(
  'water-drop',
  'Giọt nước',
  'Water drop',
  'giọt nước, nước, độ ẩm, chất lỏng',
  'water, drop, humidity, liquid',
  P('M12 2c4 5 7 8.5 7 12a7 7 0 0 1-14 0c0-3.5 3-7 7-12z', '#9CC7EC'),
  S('M9 14a3 3 0 0 0 3 3', { s: W }),
);
icon(
  'sunrise',
  'Bình minh',
  'Sunrise',
  'bình minh, mặt trời mọc, buổi sáng',
  'sunrise, dawn, morning',
  S('M12 5.5v3M4.5 10.5l2 2M19.5 10.5l-2 2', { s: '#C98A1B', w: 1.8 }),
  P('M6 18a6 6 0 0 1 12 0z', Y),
  S('M2 18h20M5 21.5h14'),
);

// ---------------------------------------------------------------- nature
cat('nature');
icon(
  'tree',
  'Cây xanh',
  'Tree',
  'cây, cây xanh, môi trường, rừng',
  'tree, green, environment, forest',
  S('M12 14v8', { w: 2.4, s: '#8A5A2B' }),
  C(12, 9, 7, G),
);
icon(
  'pine-tree',
  'Cây thông',
  'Pine tree',
  'cây thông, rừng thông, giáng sinh',
  'pine, fir, conifer, christmas tree',
  Rc(11, 16, 2, 6, 0, '#8A5A2B'),
  P('M12 2l5 7h-3l4 7H6l4-7H7z', G),
);
icon(
  'leaf',
  'Chiếc lá',
  'Leaf',
  'lá, lá cây, sinh thái, hữu cơ, xanh',
  'leaf, ecology, organic, green, eco',
  P('M4 20C4 10 10 4 20 4c0 10-6 16-16 16z', G),
  S('M4 20L14 10'),
);
icon(
  'flower',
  'Bông hoa',
  'Flower',
  'hoa, bông hoa, mùa xuân, trang trí',
  'flower, blossom, spring, decoration',
  C(12, 6.5, 3, Y),
  C(17.2, 10.3, 3, Y),
  C(15.2, 16.4, 3, Y),
  C(8.8, 16.4, 3, Y),
  C(6.8, 10.3, 3, Y),
  C(12, 12, 2.6, R),
);
icon(
  'mountain',
  'Ngọn núi',
  'Mountain',
  'núi, đồi núi, phong cảnh, đỉnh cao',
  'mountain, hill, landscape, peak, summit',
  P('M2 20l7-12 4 6 2.5-3.5L22 20z'),
  P('M9 8l-2 3.5 2 .5 1.5-1.2 1 .4z', W),
);
icon(
  'waves',
  'Sóng nước',
  'Waves',
  'sóng, nước, biển, sông',
  'waves, water, sea, river',
  S(
    'M2 8c2.5-2 4.5-2 7 0s4.5 2 7 0 4-2 6-.5M2 13c2.5-2 4.5-2 7 0s4.5 2 7 0 4-2 6-.5M2 18c2.5-2 4.5-2 7 0s4.5 2 7 0 4-2 6-.5',
    { s: '#3F7FBF', w: 1.8 },
  ),
);
icon(
  'fire',
  'Ngọn lửa',
  'Fire',
  'lửa, cháy, nóng, phòng cháy chữa cháy',
  'fire, flame, hot, burn, fire safety',
  P('M12 2c1 4 6 6 6 12a6 6 0 0 1-12 0c0-2 1-4 2-5 0 2 1 3 2 3 0-4 0-7 2-10z', R),
  P('M12 13c1.5 1.5 2.5 2.5 2.5 4.5a2.5 2.5 0 0 1-5 0c0-1.5 1-2.5 2.5-4.5z', Y, { w: 1 }),
);
icon(
  'paw',
  'Dấu chân thú',
  'Paw print',
  'dấu chân, thú cưng, động vật, chó mèo',
  'paw, pet, animal, dog, cat',
  Dot(6.5, 10, 2.1),
  Dot(12, 6.5, 2.1),
  Dot(17.5, 10, 2.1),
  F('M12 11c3 0 5.5 3 5.5 5.5S15 20 12 19s-5.5 0-5.5-2.5S9 11 12 11z'),
);
icon(
  'fish',
  'Con cá',
  'Fish',
  'cá, thuỷ sản, hải sản, biển',
  'fish, seafood, aquatic, sea',
  P('M2 12c3-5 9-6 14-2l5-4v12l-5-4c-5 4-11 3-14-2z'),
  Dot(7, 11, 1),
);
icon(
  'sprout',
  'Mầm cây',
  'Sprout',
  'mầm, mầm cây, tăng trưởng, khởi đầu, nông nghiệp',
  'sprout, seedling, growth, start, agriculture',
  S('M12 21v-9', { w: 1.8 }),
  P('M12 12c0-4-3-6-8-6 0 4 3 6 8 6z', G),
  P('M12 14c0-3 3-5 8-5 0 3-3 5-8 5z', G),
);
icon(
  'lotus',
  'Hoa sen',
  'Lotus',
  'hoa sen, sen, thanh cao, Việt Nam',
  'lotus, flower, vietnam, purity',
  P('M3 10c4 0 7 3 9 7-5 0-8-3-9-7z', '#F4B8C4'),
  P('M21 10c-4 0-7 3-9 7 5 0 8-3 9-7z', '#F4B8C4'),
  P('M12 4c3 3 3 9 0 13-3-4-3-10 0-13z', '#F9D5DC'),
  S('M4 20.5h16', { s: G, w: 1.8 }),
);

// ---------------------------------------------------------------- food
cat('food');
icon(
  'coffee',
  'Tách cà phê',
  'Coffee cup',
  'cà phê, tách, giải lao, đồ uống',
  'coffee, cup, break, drink',
  S('M17 9h1.5a2.5 2.5 0 0 1 0 5H17M8 2v3M12 2v3M3 21.5h15'),
  P('M4 8h13v6a5 5 0 0 1-5 5H9a5 5 0 0 1-5-5z', W),
);
icon(
  'glass',
  'Ly nước',
  'Glass of water',
  'ly, cốc, nước, đồ uống',
  'glass, cup, water, drink',
  P('M6 3h12l-1.5 18h-9z', W),
  P('M6.5 9h11l-1 12h-9z', '#9CC7EC', { w: 1 }),
);
icon(
  'rice-bowl',
  'Bát cơm',
  'Rice bowl',
  'cơm, bát cơm, đũa, bữa ăn',
  'rice, bowl, chopsticks, meal',
  S('M13 9l7-7M16 9l6-5.5'),
  P('M5 11a7 4.5 0 0 1 14 0z', W),
  BOWL,
);
icon(
  'noodle-bowl',
  'Tô phở',
  'Noodle bowl',
  'phở, mì, bún, tô, món nước',
  'noodle, pho, soup, bowl',
  S('M9 3c-1 1.5 1 2.5 0 4.5M13 3c-1 1.5 1 2.5 0 4.5M17 3c-1 1.5 1 2.5 0 4.5'),
  P('M2 11h20a10 10 0 0 1-6 9H8a10 10 0 0 1-6-9z', Y),
);
icon(
  'apple',
  'Quả táo',
  'Apple',
  'táo, trái cây, hoa quả, dinh dưỡng',
  'apple, fruit, nutrition, healthy',
  P('M12 7c-2-2-7-1.5-7 4 0 5 3 10 5 10 1 0 1.5-.5 2-.5s1 .5 2 .5c2 0 5-5 5-10 0-5.5-5-6-7-4z', R),
  S('M12 7c0-2 1-4 3-5', { s: '#8A5A2B' }),
);
icon(
  'cake',
  'Bánh kem',
  'Cake',
  'bánh kem, sinh nhật, kỷ niệm, tiệc',
  'cake, birthday, anniversary, party',
  S('M8 12V8M12 12V8M16 12V8'),
  Dot(8, 6, 1.1, R),
  Dot(12, 6, 1.1, R),
  Dot(16, 6, 1.1, R),
  Rc(3, 12, 18, 9, 1, '#F9D5DC'),
  S('M3 16c2 2 4 2 6 0s4-2 6 0 4 2 6 0'),
);
icon(
  'fork-knife',
  'Dao nĩa',
  'Fork and knife',
  'dao nĩa, nhà hàng, ăn uống, bữa ăn',
  'fork, knife, restaurant, dining, meal',
  S('M7 2v20M4 2v6a3 3 0 0 0 6 0V2M16 12v10', { w: 1.6 }),
  P('M16 2c3 1 4 5 4 10h-4z'),
);
icon(
  'pizza',
  'Bánh pizza',
  'Pizza slice',
  'pizza, đồ ăn nhanh, tiệc',
  'pizza, fast food, slice, party',
  P('M12 22L3 6c5-3 13-3 18 0z', Y),
  Dot(10, 9, 1.4, R),
  Dot(14.5, 10, 1.4, R),
  Dot(12, 14.5, 1.4, R),
);
icon(
  'ice-cream',
  'Kem ốc quế',
  'Ice cream',
  'kem, kem ốc quế, tráng miệng, mùa hè',
  'ice cream, cone, dessert, summer',
  P('M7 11l5 11 5-11z', Y),
  P('M6 11a6 6 0 0 1 12 0z', '#F9D5DC'),
);
icon(
  'wine-glass',
  'Ly rượu',
  'Wine glass',
  'ly rượu, tiệc, chúc mừng, liên hoan',
  'wine, glass, toast, celebration, party',
  S('M12 12v8.5M8 21h8'),
  P('M6 2h12c0 6-2 10-6 10S6 8 6 2z', W),
  P('M6.5 6h11c-.6 3.6-2.5 6-5.5 6S7.1 9.6 6.5 6z', R, { w: 1 }),
);
icon(
  'bottle',
  'Chai nước',
  'Bottle',
  'chai, bình nước, đồ uống',
  'bottle, water bottle, drink',
  P('M10 2h4v4l2 3v12a1 1 0 0 1-1 1H9a1 1 0 0 1-1-1V9l2-3z', W),
  Rc(8, 12, 8, 5, 0, '#9CC7EC'),
);
icon(
  'fried-egg',
  'Trứng ốp la',
  'Fried egg',
  'trứng, trứng ốp la, bữa sáng',
  'egg, fried egg, breakfast',
  P('M12 3c4 0 9 3 9 9s-4 9-9 9-10-3-9-9 4-9 9-9z', W),
  C(12, 12, 3.5, Y),
);

// ---------------------------------------------------------------- buildings
cat('buildings');
icon(
  'home',
  'Ngôi nhà',
  'Home',
  'nhà, nhà ở, trang chủ, gia đình',
  'home, house, residence, family',
  P('M3 11l9-8 9 8v10H3z'),
  Rc(10, 14, 4, 7, 0, W),
);
icon(
  'office-building',
  'Toà nhà văn phòng',
  'Office building',
  'toà nhà, văn phòng, công ty, trụ sở, cơ quan',
  'office, building, company, headquarters',
  Rc(5, 2, 14, 20, 1),
  Rc(8, 5, 3, 3, 0, W, { w: 1 }),
  Rc(13, 5, 3, 3, 0, W, { w: 1 }),
  Rc(8, 10, 3, 3, 0, W, { w: 1 }),
  Rc(13, 10, 3, 3, 0, W, { w: 1 }),
  F('M10.5 16h3v6h-3z'),
);
icon(
  'factory',
  'Nhà máy',
  'Factory',
  'nhà máy, xưởng, sản xuất, công nghiệp',
  'factory, plant, manufacturing, industry',
  P('M2 21V11l6 3v-3l6 3V4h5l1 17z'),
  Rc(5, 16, 3, 2.5, 0, W, { w: 1 }),
  Rc(10.5, 16, 3, 2.5, 0, W, { w: 1 }),
);
icon(
  'hospital',
  'Bệnh viện',
  'Hospital',
  'bệnh viện, phòng khám, y tế, cơ sở y tế',
  'hospital, clinic, medical centre, healthcare',
  Rc(3, 7, 18, 14, 1),
  Rc(8, 2, 8, 7, 1, W),
  F('M11 3.5h2V5h1.5v2H13v1.5h-2V7H9.5V5H11z', R),
  Rc(10, 15, 4, 6, 0, W),
  Rc(5.5, 11, 2.5, 2.5, 0, W, { w: 1 }),
  Rc(16, 11, 2.5, 2.5, 0, W, { w: 1 }),
);
icon(
  'school',
  'Trường học',
  'School',
  'trường học, nhà trường, giáo dục, học đường',
  'school, education, campus, academy',
  S('M12 5V1.5'),
  F('M12 1.5h4l-1 1.3 1 1.2h-4z', R),
  P('M2 21V11l10-6 10 6v10z'),
  C(12, 11, 2, W),
  Rc(10, 16, 4, 5, 0, W),
);
icon(
  'bank',
  'Ngân hàng',
  'Bank',
  'ngân hàng, kho bạc, tài chính, cơ quan nhà nước',
  'bank, treasury, finance, institution, government',
  S('M6 9v9M10 9v9M14 9v9M18 9v9', { w: 1.8 }),
  P('M2 9l10-6 10 6z'),
  Rc(2, 18, 20, 3, 0.5),
);
icon(
  'store',
  'Cửa hàng',
  'Store',
  'cửa hàng, cửa tiệm, bán lẻ, quầy',
  'store, shop, retail, market',
  Rc(4, 9, 16, 12, 0),
  Rc(13, 13, 4, 8, 0, W),
  Rc(6.5, 12.5, 4.5, 4, 0, W),
  P('M3 9l2-6h14l2 6z', R),
);
icon(
  'pagoda',
  'Ngôi chùa',
  'Pagoda',
  'chùa, đền, đình, di tích, văn hoá',
  'pagoda, temple, heritage, culture',
  Rc(6.5, 10.5, 11, 4.5, 0, W),
  Rc(7.5, 16.5, 9, 4.5, 0, W),
  P('M2 9c4 0 8-2 10-6 2 4 6 6 10 6l-2 2H4z', R),
  P('M3 15h18l-1.5 2h-15z', R),
  S('M3 21h18'),
);
icon(
  'bridge',
  'Cây cầu',
  'Bridge',
  'cầu, cây cầu, hạ tầng, kết nối',
  'bridge, infrastructure, connection',
  S('M2 16h20M2 8c4 6 16 6 20 0M6 12v4M12 12.5v3.5M18 12v4M2 6v14M22 6v14', { w: 1.6 }),
);
icon(
  'warehouse',
  'Nhà kho',
  'Warehouse',
  'kho, nhà kho, kho hàng, lưu kho',
  'warehouse, storage, depot, inventory',
  P('M2 21V9l10-6 10 6v12z'),
  Rc(6, 12, 12, 9, 0, W),
  S('M6 15h12M6 18h12'),
);
icon(
  'door',
  'Cánh cửa',
  'Door',
  'cửa, cánh cửa, lối vào, lối ra',
  'door, entrance, exit, entry',
  Rc(4, 2, 16, 20, 1),
  Rc(7, 4.5, 10, 17.5, 0, W),
  Dot(14.5, 13.5, 1),
);

// ---------------------------------------------------------------- signs
cat('signs');
icon(
  'check-circle',
  'Dấu tích tròn',
  'Check mark',
  'dấu tích, đạt, hoàn thành, đồng ý, đúng',
  'check, done, complete, approve, correct, ok',
  C(12, 12, 10, G),
  S('M7 12.5l3.5 3.5 6.5-7.5', { s: W, w: 2.2 }),
);
icon(
  'x-circle',
  'Dấu gạch chéo tròn',
  'Cross mark',
  'dấu chéo, sai, không đạt, từ chối, huỷ',
  'cross, wrong, fail, reject, cancel, error',
  C(12, 12, 10, R),
  S('M8 8l8 8M16 8l-8 8', { s: W, w: 2.2 }),
);
icon(
  'warning',
  'Cảnh báo',
  'Warning',
  'cảnh báo, chú ý, nguy hiểm, lưu ý',
  'warning, caution, danger, attention, alert',
  P('M12 2.5L22.5 21h-21z', Y),
  S('M12 9v6', { w: 2 }),
  Dot(12, 18, 1.2),
);
icon(
  'info-circle',
  'Thông tin',
  'Information',
  'thông tin, ghi chú, hướng dẫn, chú thích',
  'information, info, note, guide, help',
  C(12, 12, 10),
  S('M12 11v6', { w: 2 }),
  Dot(12, 7.5, 1.3),
);
icon(
  'question-circle',
  'Dấu hỏi',
  'Question mark',
  'dấu hỏi, câu hỏi, trợ giúp, hỏi đáp',
  'question, help, faq, ask, unknown',
  C(12, 12, 10),
  S('M9 9.5a3 3 0 1 1 4.5 2.6c-1 .6-1.5 1.2-1.5 2.4', { w: 1.8 }),
  Dot(12, 17.5, 1.2),
);
icon(
  'prohibited',
  'Biển cấm',
  'Prohibited',
  'cấm, không được, nghiêm cấm, hạn chế',
  'prohibited, forbidden, no, ban, restricted',
  C(12, 12, 9.5, W, { s: R, w: 2.4 }),
  S('M5.5 5.5l13 13', { s: R, w: 2.4 }),
);
icon(
  'star',
  'Ngôi sao',
  'Star',
  'ngôi sao, sao, nổi bật, đánh giá, yêu thích',
  'star, favourite, rating, featured, highlight',
  P('M12 2l3 6.5 7 .9-5.2 4.8 1.4 7-6.2-3.5-6.2 3.5 1.4-7L2 9.4l7-.9z', Y),
);
icon(
  'flag',
  'Lá cờ',
  'Flag',
  'cờ, lá cờ, đánh dấu, cột mốc, mục tiêu',
  'flag, mark, milestone, goal',
  P('M5 3h14l-3 4.5 3 4.5H5z', R),
  S('M5 2v20', { w: 1.8 }),
);
icon(
  'stop-sign',
  'Biển dừng',
  'Stop sign',
  'dừng, dừng lại, biển báo, ngăn chặn',
  'stop, halt, sign, block',
  P('M8 2h8l6 6v8l-6 6H8l-6-6V8z', R),
  S('M7 12h10', { s: W, w: 2.6 }),
);
icon(
  'plus',
  'Dấu cộng',
  'Plus',
  'cộng, thêm, bổ sung, tạo mới',
  'plus, add, new, create',
  P('M10 3h4v7h7v4h-7v7h-4v-7H3v-4h7z', G),
);
icon(
  'minus',
  'Dấu trừ',
  'Minus',
  'trừ, bớt, giảm, xoá bớt',
  'minus, subtract, remove, less',
  Rc(3, 10, 18, 4, 1, R),
);
icon(
  'thumbs-up',
  'Tán thành',
  'Thumbs up',
  'thích, tán thành, đồng ý, khen ngợi, tốt',
  'thumbs up, like, agree, praise, good',
  Rc(2, 10, 4, 11, 0.5),
  P('M6 10l4-8c2 0 3 1.5 2.5 3.5L12 9h7a2 2 0 0 1 2 2.5l-1.5 7.5a2 2 0 0 1-2 2H6z', Y),
);
icon(
  'bookmark',
  'Dấu trang',
  'Bookmark',
  'dấu trang, đánh dấu, lưu lại',
  'bookmark, save, mark',
  P('M6 2h12v20l-6-5-6 5z', R),
);
icon(
  'pushpin',
  'Đinh ghim',
  'Pushpin',
  'ghim, đinh ghim, ghi nhớ, cố định',
  'pin, pushpin, remember, fixed, sticky',
  S('M12 14v8'),
  P('M9 2h6l-1 7 3 3v2H7v-2l3-3z', R),
);
icon(
  'clock',
  'Đồng hồ',
  'Clock',
  'đồng hồ, thời gian, giờ, thời hạn',
  'clock, time, hour, deadline, schedule',
  C(12, 12, 10, W),
  S('M12 6v6l4 2.5', { w: 1.8 }),
);
icon(
  'hourglass',
  'Đồng hồ cát',
  'Hourglass',
  'đồng hồ cát, chờ, thời hạn, tiến độ',
  'hourglass, wait, deadline, pending, timer',
  P('M6 2h12v4l-4 6 4 6v4H6v-4l4-6-4-6z', W),
  F('M8.5 20l3.5-4 3.5 4z', Y),
  S('M5 2h14M5 22h14', { w: 1.8 }),
);
icon(
  'trash',
  'Thùng rác',
  'Trash bin',
  'thùng rác, xoá, huỷ bỏ, loại bỏ',
  'trash, bin, delete, remove, discard',
  S('M9 7V3h6v4'),
  P('M5 7h14l-1 14H6z'),
  S('M3 7h18M10 11v7M14 11v7'),
);
icon(
  'power',
  'Nút nguồn',
  'Power',
  'nguồn, bật tắt, khởi động, năng lượng',
  'power, on, off, start, energy',
  S('M12 2v9M7 5.5a8 8 0 1 0 10 0', { w: 2.2 }),
);
icon(
  'lightning',
  'Tia chớp',
  'Lightning',
  'tia chớp, điện, nhanh, năng lượng',
  'lightning, bolt, electricity, fast, energy',
  P('M13 2L4 14h6l-1 8 9-12h-6z', Y),
);

// ---------------------------------------------------------------- charts
cat('charts');
icon(
  'bar-chart',
  'Biểu đồ cột',
  'Bar chart',
  'biểu đồ cột, thống kê, số liệu, báo cáo',
  'bar chart, column, statistics, figures, report',
  Rc(6, 12, 3.5, 9, 0, L),
  Rc(11, 6, 3.5, 15, 0, Y),
  Rc(16, 9, 3.5, 12, 0, G),
  AXES,
);
icon(
  'line-chart',
  'Biểu đồ đường',
  'Line chart',
  'biểu đồ đường, xu hướng, diễn biến',
  'line chart, trend, progress, graph',
  AXES,
  S('M5.5 16l4-5 4 3 6-8', { s: R, w: 1.8 }),
  Dot(9.5, 11, 1.4),
  Dot(13.5, 14, 1.4),
  Dot(19.5, 6, 1.4),
);
icon(
  'pie-chart',
  'Biểu đồ tròn',
  'Pie chart',
  'biểu đồ tròn, tỷ lệ, cơ cấu, phần trăm',
  'pie chart, share, proportion, percentage',
  C(12, 12, 9.5),
  P('M12 12V2.5A9.5 9.5 0 0 1 21.5 12z', R),
  P('M12 12h9.5a9.5 9.5 0 0 1-4 7.7z', Y),
);
icon(
  'donut-chart',
  'Biểu đồ vành khuyên',
  'Doughnut chart',
  'biểu đồ vành khuyên, tỷ lệ, cơ cấu',
  'doughnut chart, donut, ring, proportion',
  C(12, 12, 9.5),
  P('M12 12V2.5A9.5 9.5 0 0 1 20.2 16.8z', G),
  C(12, 12, 4.5, W),
);
icon(
  'area-chart',
  'Biểu đồ vùng',
  'Area chart',
  'biểu đồ vùng, miền, tích luỹ',
  'area chart, cumulative, volume',
  P('M3 21v-7l6-5 4 4 7-8v16z'),
  AXES,
);
icon(
  'growth',
  'Tăng trưởng',
  'Growth',
  'tăng trưởng, tăng, phát triển, doanh thu',
  'growth, increase, rise, revenue, up',
  Rc(4, 15, 3.5, 6, 0),
  Rc(10, 12, 3.5, 9, 0),
  Rc(16, 9, 3.5, 12, 0),
  S('M4 10l6-4 4 2 5-4.5', { s: G, w: 1.8 }),
  F('M21 2v4.5l-4-1.5z', G),
);
icon(
  'decline',
  'Sụt giảm',
  'Decline',
  'sụt giảm, giảm, đi xuống, thua lỗ',
  'decline, decrease, fall, loss, down',
  Rc(4, 9, 3.5, 12, 0),
  Rc(10, 12, 3.5, 9, 0),
  Rc(16, 15, 3.5, 6, 0),
  S('M4 3l6 4 4-2 5 4.5', { s: R, w: 1.8 }),
  F('M21 11.5V7l-4 1.5z', R),
);
icon(
  'table',
  'Bảng số liệu',
  'Data table',
  'bảng, bảng số liệu, lưới, danh sách',
  'table, grid, data, list, spreadsheet',
  Rc(2, 4, 20, 16, 1.5, W),
  P('M2 8.5V5.5A1.5 1.5 0 0 1 3.5 4h17A1.5 1.5 0 0 1 22 5.5v3z'),
  S('M2 12.5h20M2 16.5h20M9 4v16M15.5 4v16', { w: 1 }),
);
icon(
  'gantt',
  'Biểu đồ tiến độ',
  'Gantt chart',
  'tiến độ, kế hoạch, lịch trình, dự án',
  'gantt, schedule, timeline, project, plan',
  Rc(5, 4.5, 8, 3.5, 0.5, L),
  Rc(9, 10, 9, 3.5, 0.5, Y),
  Rc(13, 15.5, 8, 3.5, 0.5, G),
  S('M3 2v20'),
);
icon(
  'org-chart',
  'Sơ đồ tổ chức',
  'Organisation chart',
  'sơ đồ tổ chức, cơ cấu, phòng ban, phân cấp',
  'org chart, organisation, hierarchy, structure',
  S('M12 7v4.5M5 16v-4.5h14V16'),
  Rc(8.5, 2, 7, 5, 1, Y),
  Rc(1.5, 16, 7, 5, 1),
  Rc(15.5, 16, 7, 5, 1),
);
icon(
  'flowchart',
  'Lưu đồ',
  'Flowchart',
  'lưu đồ, quy trình, sơ đồ, các bước',
  'flowchart, process, workflow, diagram, steps',
  S('M10 5.5h3M17 10v6'),
  Rc(2, 3, 8, 5, 1),
  P('M17 1.5l5 4-5 4-5-4z', Y),
  Rc(12.5, 16, 9, 5, 1, G),
);
icon(
  'gauge',
  'Đồng hồ chỉ số',
  'Gauge',
  'chỉ số, KPI, hiệu suất, đo lường',
  'gauge, kpi, performance, meter, dashboard',
  P('M2 17a10 10 0 0 1 20 0z'),
  S('M12 17l5-6', { s: R, w: 2 }),
  Dot(12, 17, 1.6),
);
icon(
  'scatter-chart',
  'Biểu đồ phân tán',
  'Scatter chart',
  'biểu đồ phân tán, tương quan, điểm dữ liệu',
  'scatter chart, correlation, points, plot',
  AXES,
  Dot(7, 16, 1.5, R),
  Dot(10, 12, 1.5, G),
  Dot(13, 14, 1.5, R),
  Dot(15.5, 8.5, 1.5, G),
  Dot(19, 6, 1.5, R),
);
icon(
  'funnel',
  'Phễu',
  'Funnel',
  'phễu, lọc, sàng lọc, chuyển đổi',
  'funnel, filter, conversion, pipeline',
  P('M2 4h20l-7 9v7l-6-2v-5z'),
);
icon(
  'presentation-chart',
  'Bảng trình chiếu',
  'Presentation board',
  'trình chiếu, thuyết trình, báo cáo, màn chiếu',
  'presentation, slide, board, report, screen',
  S('M12 16v3M8 22l4-3 4 3M1 3h22'),
  Rc(2, 3, 20, 13, 1, W),
  Rc(6, 10, 2.5, 4, 0, L, { w: 1 }),
  Rc(10.75, 7, 2.5, 7, 0, Y, { w: 1 }),
  Rc(15.5, 9, 2.5, 5, 0, G, { w: 1 }),
);

// ---------------------------------------------------------------- admin
cat('admin');
const LINES = S('M8 11h8M8 14h8M8 17h5');
const SEAL_STAR = 'M12 7.5l1.4 2.9 3.2.4-2.4 2.2.7 3.2-2.9-1.6-2.9 1.6.7-3.2L7.4 10.8l3.2-.4z';
icon(
  'document',
  'Văn bản',
  'Document',
  'văn bản, tài liệu, công văn, giấy tờ',
  'document, paper, file, text, letter',
  SHEET,
  LINES,
);
icon(
  'documents',
  'Bộ tài liệu',
  'Documents',
  'tài liệu, bản sao, hồ sơ, nhiều văn bản',
  'documents, copies, papers, files',
  Rc(8, 2, 13, 16, 1),
  Rc(3, 6, 13, 16, 1, W),
  S('M6 11h7M6 14h7M6 17h4'),
);
icon(
  'round-seal',
  'Con dấu tròn',
  'Round seal',
  'con dấu, dấu tròn, đóng dấu, dấu đỏ, xác nhận',
  'seal, round stamp, official, red stamp, certified',
  C(12, 12, 10, W, { s: R, w: 1.8 }),
  C(12, 12, 7, 'none', { s: R, w: 1 }),
  F(SEAL_STAR, R),
);
icon(
  'rubber-stamp',
  'Dấu đóng',
  'Rubber stamp',
  'dấu, đóng dấu, phê duyệt, duyệt',
  'stamp, rubber stamp, approve, approval',
  P('M9 3h6v4l-1 5h-4L9 7z'),
  Rc(5, 12, 14, 5, 1),
  S('M4 20.5h16', { s: R, w: 2.2 }),
);
icon(
  'signature',
  'Chữ ký',
  'Signature',
  'chữ ký, ký tên, ký duyệt, ký nháy',
  'signature, sign, autograph, approve',
  S('M2 16c2-6 4-9 5.5-9s0 9 2 9 2-5 3.5-5 .5 4 2 4 2-2 3-2 2 1 4 1', { w: 1.6 }),
  S('M2 20.5h20', { w: 1 }),
);
icon(
  'signed-document',
  'Văn bản đã ký',
  'Signed document',
  'văn bản đã ký, ký văn bản, ký số, ban hành',
  'signed document, sign off, e-signature, issue',
  SHEET,
  S('M8 11h8'),
  S('M8 17c1-2.5 2-3.5 2.5-3.5s0 3.5 1 3.5 1.5-2 2.5-2 1 1.5 2 1.5', { s: '#2F5FA8' }),
  S('M8 19.5h8', { w: 1 }),
);
icon(
  'calendar',
  'Lịch',
  'Calendar',
  'lịch, ngày tháng, lịch công tác, lịch hẹn',
  'calendar, date, schedule, appointment',
  CALENDAR,
  Dot(8, 14),
  Dot(12, 14),
  Dot(16, 14),
  Dot(8, 17.5),
  Dot(12, 17.5),
);
icon(
  'calendar-check',
  'Lịch đã xác nhận',
  'Calendar confirmed',
  'lịch, xác nhận ngày, đã hẹn, đúng hạn',
  'calendar, confirmed, booked, on time',
  CALENDAR,
  S('M8 15.5l3 3 5-5.5', { s: G, w: 2 }),
);
icon(
  'clipboard-list',
  'Bảng ghi chép',
  'Clipboard',
  'bảng ghi, danh sách, biên bản, ghi chép',
  'clipboard, list, minutes, notes',
  CLIPBOARD,
  S('M8 11h8M8 14.5h8M8 18h5'),
);
icon(
  'checklist',
  'Danh mục kiểm tra',
  'Checklist',
  'danh mục kiểm tra, việc cần làm, nghiệm thu, rà soát',
  'checklist, to-do, tasks, acceptance, review',
  CLIPBOARD,
  S('M7.5 11l1.5 1.5 2.5-3M7.5 16.5l1.5 1.5 2.5-3', { s: G, w: 1.8 }),
  S('M14 11.5h3M14 17h3'),
);
icon(
  'folder',
  'Thư mục',
  'Folder',
  'thư mục, cặp hồ sơ, lưu trữ',
  'folder, directory, files',
  P('M2 5h7l2 3h11v12H2z', Y),
);
icon(
  'folder-open',
  'Thư mục mở',
  'Open folder',
  'thư mục mở, hồ sơ, mở hồ sơ',
  'open folder, case file, dossier',
  P('M2 20V5h7l2 3h9v3H5z', '#D9A52E'),
  P('M2 20l3-9h18l-3 9z', Y),
);
icon(
  'archive-box',
  'Hộp lưu trữ',
  'Archive box',
  'lưu trữ, hộp hồ sơ, kho lưu trữ, văn thư',
  'archive, box, records, storage',
  P('M4 9h16v11H4z'),
  Rc(2, 4, 20, 5, 1, Y),
  S('M9.5 13.5h5', { w: 1.8 }),
);
icon(
  'contract',
  'Hợp đồng',
  'Contract',
  'hợp đồng, thoả thuận, cam kết, giao kết',
  'contract, agreement, deal, commitment',
  SHEET,
  S('M8 11h8M8 14h4'),
  C(15.5, 17, 3, W, { s: R }),
  Dot(15.5, 17, 1.1, R),
);
icon(
  'medal',
  'Huân chương',
  'Medal',
  'huân chương, huy chương, khen thưởng, thi đua',
  'medal, award, honour, commendation',
  P('M7 2h4l3 7h-4z', R),
  P('M17 2h-4l-3 7h4z', '#2F5FA8'),
  C(12, 15, 6, Y),
  C(12, 15, 3, W),
);
icon(
  'gavel',
  'Búa quyết định',
  'Gavel',
  'búa, quyết định, phán quyết, pháp lý, nghị quyết',
  'gavel, decision, ruling, legal, resolution',
  S('M12 10l-9 9', { w: 2.6, s: '#8A5A2B' }),
  P('M13 2l7 7-3 3-7-7z'),
  Rc(11, 19, 11, 3, 1),
);
icon(
  'inbox',
  'Công văn đến',
  'Incoming mail',
  'công văn đến, hộp thư đến, tiếp nhận',
  'inbox, incoming, received, intake',
  P('M2 13l3-9h14l3 9v7H2z'),
  P('M2 13h6l1 3h6l1-3h6v7H2z', W),
  S('M12 5v6M9.5 8.5L12 11l2.5-2.5', { s: G, w: 1.8 }),
);
icon(
  'outbox',
  'Công văn đi',
  'Outgoing mail',
  'công văn đi, hộp thư đi, phát hành, gửi đi',
  'outbox, outgoing, sent, dispatch',
  P('M2 13l3-9h14l3 9v7H2z'),
  P('M2 13h6l1 3h6l1-3h6v7H2z', W),
  S('M12 11V5M9.5 7.5L12 5l2.5 2.5', { s: R, w: 1.8 }),
);
icon(
  'paperclip',
  'Kẹp giấy',
  'Paperclip',
  'kẹp giấy, đính kèm, phụ lục, tệp kèm',
  'paperclip, attachment, attach, appendix',
  S('M8 12l6-6a3.5 3.5 0 0 1 5 5l-8.5 8.5a5 5 0 0 1-7-7L12 4', { w: 1.8 }),
);
icon(
  'form',
  'Biểu mẫu',
  'Form',
  'biểu mẫu, mẫu đơn, tờ khai, phiếu',
  'form, application, declaration, template',
  Rc(4, 2, 16, 20, 1.5, W),
  Rc(7, 6, 3, 3, 0, L, { w: 1 }),
  Rc(7, 11, 3, 3, 0, L, { w: 1 }),
  Rc(7, 16, 3, 3, 0, L, { w: 1 }),
  S('M12.5 7.5h4.5M12.5 12.5h4.5M12.5 17.5h4.5'),
);
icon(
  'fountain-pen',
  'Bút ký',
  'Fountain pen',
  'bút ký, bút máy, ký kết, viết',
  'fountain pen, pen, signing, write',
  P('M12 22l-4-11 4-9 4 9z'),
  S('M12 22v-9'),
  C(12, 11.5, 1.4, W),
);
icon(
  'report',
  'Báo cáo',
  'Report',
  'báo cáo, tổng kết, thống kê, số liệu',
  'report, summary, statistics, review',
  SHEET,
  Rc(8, 15, 2, 4, 0, L, { w: 1 }),
  Rc(11, 12, 2, 7, 0, Y, { w: 1 }),
  Rc(14, 10, 2, 9, 0, G, { w: 1 }),
);

// ---------------------------------------------------------------- output
/** Lower-case and strip Vietnamese diacritics (đ → d). */
const fold = (s) =>
  s.normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/đ/g, 'd').replace(/Đ/g, 'D').toLowerCase().trim();
const list = (s) =>
  s
    .split(',')
    .map((k) => k.trim())
    .filter(Boolean);
const unique = (a) => [...new Set(a)];

const xmlEscape = (s) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

function svg(icon) {
  if (/\.\d00\b/.test(icon.body)) throw new Error(`${icon.id}: unnormalised number in body`);
  return (
    `<svg xmlns="http://www.w3.org/2000/svg" width="96" height="96" viewBox="0 0 24 24" fill="none" stroke-linecap="round" stroke-linejoin="round">` +
    `<title>${xmlEscape(icon.en)}</title>${icon.body}</svg>\n`
  );
}

const ids = new Set();
for (const i of ICONS) {
  if (ids.has(i.id)) throw new Error(`duplicate id ${i.id}`);
  if (!/^[a-z][a-z0-9-]*$/.test(i.id)) throw new Error(`bad id ${i.id}`);
  ids.add(i.id);
}

await mkdir(OUT, { recursive: true });
for (const f of await readdir(OUT)) if (f.endsWith('.svg')) await rm(join(OUT, f));
for (const i of ICONS) await writeFile(join(OUT, `${i.id}.svg`), svg(i));

const manifest = {
  version: 1,
  license:
    'Original work of the BlueOffice project (Bluestar), drawn for this library. Not derived from any third-party icon set.',
  grid: 24,
  categories: CATEGORIES.map(([id, vi, en]) => ({ id, name: { vi, en } })),
  icons: ICONS.map((i) => {
    const vi = list(i.kwVi);
    return {
      id: i.id,
      category: i.category,
      file: `${i.id}.svg`,
      name: { vi: i.vi, en: i.en },
      keywords: {
        // Accented forms first, then the unaccented ones people type without a Vietnamese IME.
        vi: unique([...vi, ...vi.map(fold), fold(i.vi)]).filter((k) => k !== i.vi),
        en: unique(list(i.kwEn).map((k) => k.toLowerCase())),
      },
    };
  }),
};
await writeFile(join(OUT, 'manifest.json'), `${JSON.stringify(manifest, null, 1)}\n`);
const BUNDLE = join(OUT, '..', '..', '..', 'packages', 'ui', 'src', 'iconLibrary', 'data.json');
const bundle = { manifest, svg: Object.fromEntries(ICONS.map((i) => [i.id, svg(i)])) };
await writeFile(BUNDLE, `${JSON.stringify(bundle)}\n`);
console.log(`${ICONS.length} icons in ${CATEGORIES.length} categories → ${OUT}`);
