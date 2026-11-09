/**
 * Vietnamese search keywords of the symbol picker (BlueOffice's own list). A keyword
 * finds a few characters (`chars`) and/or whole groups (`blocks`, ids of `data.ts` or
 * `vietnamese`). Matching ignores case and diacritics, so "mui ten" finds "mũi tên".
 */
export interface SymbolKeyword {
  words: readonly string[];
  chars?: string;
  blocks?: readonly string[];
}

export const VI_KEYWORDS: readonly SymbolKeyword[] = [
  // Money.
  { words: ['đồng', 'tiền việt', 'việt nam đồng', 'vnd'], chars: '₫' },
  { words: ['tiền', 'tiền tệ', 'ký hiệu tiền'], chars: '₫$€£¥¢', blocks: ['currency'] },
  { words: ['đô la', 'đô'], chars: '$' },
  { words: ['ơ rô', 'euro'], chars: '€' },
  { words: ['bảng anh'], chars: '£' },
  { words: ['yên', 'nhân dân tệ'], chars: '¥' },
  // Arrows.
  { words: ['mũi tên'], blocks: ['arrows', 'supArrowsA', 'supArrowsB', 'miscSymbolsArrows'] },
  { words: ['mũi tên phải', 'sang phải'], chars: '→⇒⟶➔➜' },
  { words: ['mũi tên trái', 'sang trái'], chars: '←⇐⟵' },
  { words: ['mũi tên lên', 'đi lên'], chars: '↑⇑' },
  { words: ['mũi tên xuống', 'đi xuống'], chars: '↓⇓' },
  { words: ['hai chiều'], chars: '↔⇔⟷' },
  // Mathematics.
  { words: ['toán', 'toán học', 'phép toán'], blocks: ['mathOperators'] },
  { words: ['cộng trừ'], chars: '±∓' },
  { words: ['nhân', 'phép nhân'], chars: '×·' },
  { words: ['chia', 'phép chia'], chars: '÷∕' },
  { words: ['khác', 'không bằng'], chars: '≠' },
  { words: ['xấp xỉ', 'gần bằng'], chars: '≈≃' },
  { words: ['nhỏ hơn hoặc bằng'], chars: '≤' },
  { words: ['lớn hơn hoặc bằng'], chars: '≥' },
  { words: ['vô cực', 'vô cùng'], chars: '∞' },
  { words: ['căn', 'căn bậc hai'], chars: '√∛∜' },
  { words: ['tổng', 'xích ma'], chars: '∑Σσ' },
  { words: ['tích phân'], chars: '∫∬∮' },
  { words: ['thuộc', 'tập hợp'], chars: '∈∉⊂⊃∪∩∅' },
  { words: ['độ', 'nhiệt độ'], chars: '°℃℉' },
  { words: ['phần trăm', 'phần nghìn'], chars: '%‰' },
  { words: ['phân số'], chars: '½⅓⅔¼¾⅕⅛' },
  { words: ['số la mã'], blocks: ['numberForms'] },
  { words: ['chỉ số trên', 'chỉ số dưới', 'số mũ', 'lũy thừa'], blocks: ['superSub'] },
  // Greek.
  { words: ['hy lạp', 'chữ hy lạp'], blocks: ['greek'] },
  { words: ['anpha'], chars: 'αΑ' },
  { words: ['bêta'], chars: 'βΒ' },
  { words: ['gamma'], chars: 'γΓ' },
  { words: ['đenta'], chars: 'δΔ' },
  { words: ['lamđa'], chars: 'λΛ' },
  { words: ['muy'], chars: 'μΜ' },
  { words: ['ômêga'], chars: 'ωΩ' },
  { words: ['pi'], chars: 'πΠ' },
  // Vietnamese letters.
  {
    words: ['tiếng việt', 'chữ việt', 'chữ có dấu', 'dấu thanh'],
    blocks: ['vietnamese'],
  },
  // Punctuation and marks.
  { words: ['gạch ngang', 'gạch dài'], chars: '–—―' },
  { words: ['gạch nối'], chars: '‐‑' },
  { words: ['ba chấm'], chars: '…' },
  { words: ['ngoặc kép'], chars: '“”„«»' },
  { words: ['ngoặc đơn'], chars: '‘’‚‹›' },
  { words: ['chấm tròn', 'gạch đầu dòng'], chars: '•◦▪‣' },
  { words: ['đoạn văn'], chars: '¶' },
  { words: ['điều khoản', 'mục'], chars: '§' },
  { words: ['bản quyền'], chars: '©' },
  { words: ['nhãn hiệu', 'thương hiệu'], chars: '™®' },
  { words: ['dấu tích', 'đánh dấu', 'đúng'], chars: '✓✔☑' },
  { words: ['dấu nhân', 'sai'], chars: '✗✘☒' },
  { words: ['ô vuông', 'hộp kiểm'], chars: '☐☑☒□■' },
  { words: ['ngôi sao', 'sao'], chars: '★☆✦✩' },
  { words: ['trái tim', 'tim'], chars: '♥♡❤' },
  { words: ['điện thoại'], chars: '☎✆' },
  { words: ['thư', 'phong bì'], chars: '✉' },
  { words: ['cái kéo', 'kéo'], chars: '✂' },
  { words: ['bút'], chars: '✎✏✒' },
  { words: ['đồng hồ'], chars: '⌚⏰⌛' },
  { words: ['nốt nhạc', 'âm nhạc'], chars: '♩♪♫♬' },
  { words: ['mặt cười'], chars: '☺☻' },
  { words: ['thời tiết', 'mặt trời', 'mây', 'tuyết'], chars: '☀☁☂❄' },
  { words: ['bàn tay'], chars: '☚☛☜☝☞☟' },
  { words: ['kẻ khung', 'khung'], blocks: ['boxDrawing'] },
  { words: ['hình học', 'hình'], blocks: ['geometricShapes'] },
  { words: ['khoảng trắng', 'dấu cách'], chars: '     ' },
];
