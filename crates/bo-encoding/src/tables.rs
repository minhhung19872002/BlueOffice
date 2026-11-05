//! Code tables for TCVN3 (TCVN 5712:1993 VN3) and VNI-Windows.

/// The twelve Vietnamese vowels, lower case. Columns: no tone, huyền (grave),
/// hỏi (hook above), ngã (tilde), sắc (acute), nặng (dot below).
pub const VOWELS: [[char; 6]; 12] = [
    ['a', 'à', 'ả', 'ã', 'á', 'ạ'],
    ['ă', 'ằ', 'ẳ', 'ẵ', 'ắ', 'ặ'],
    ['â', 'ầ', 'ẩ', 'ẫ', 'ấ', 'ậ'],
    ['e', 'è', 'ẻ', 'ẽ', 'é', 'ẹ'],
    ['ê', 'ề', 'ể', 'ễ', 'ế', 'ệ'],
    ['i', 'ì', 'ỉ', 'ĩ', 'í', 'ị'],
    ['o', 'ò', 'ỏ', 'õ', 'ó', 'ọ'],
    ['ô', 'ồ', 'ổ', 'ỗ', 'ố', 'ộ'],
    ['ơ', 'ờ', 'ở', 'ỡ', 'ớ', 'ợ'],
    ['u', 'ù', 'ủ', 'ũ', 'ú', 'ụ'],
    ['ư', 'ừ', 'ử', 'ữ', 'ứ', 'ự'],
    ['y', 'ỳ', 'ỷ', 'ỹ', 'ý', 'ỵ'],
];

/// Upper-case counterpart of [`VOWELS`].
pub const VOWELS_UPPER: [[char; 6]; 12] = [
    ['A', 'À', 'Ả', 'Ã', 'Á', 'Ạ'],
    ['Ă', 'Ằ', 'Ẳ', 'Ẵ', 'Ắ', 'Ặ'],
    ['Â', 'Ầ', 'Ẩ', 'Ẫ', 'Ấ', 'Ậ'],
    ['E', 'È', 'Ẻ', 'Ẽ', 'É', 'Ẹ'],
    ['Ê', 'Ề', 'Ể', 'Ễ', 'Ế', 'Ệ'],
    ['I', 'Ì', 'Ỉ', 'Ĩ', 'Í', 'Ị'],
    ['O', 'Ò', 'Ỏ', 'Õ', 'Ó', 'Ọ'],
    ['Ô', 'Ồ', 'Ổ', 'Ỗ', 'Ố', 'Ộ'],
    ['Ơ', 'Ờ', 'Ở', 'Ỡ', 'Ớ', 'Ợ'],
    ['U', 'Ù', 'Ủ', 'Ũ', 'Ú', 'Ụ'],
    ['Ư', 'Ừ', 'Ử', 'Ữ', 'Ứ', 'Ự'],
    ['Y', 'Ỳ', 'Ỷ', 'Ỹ', 'Ý', 'Ỵ'],
];

// ---------------------------------------------------------------------------
// TCVN3 — TCVN 5712:1993, table VN3.
// ---------------------------------------------------------------------------

/// TCVN3 bytes of the five toned forms (grave, hook, tilde, acute, dot) of each
/// row of [`VOWELS`].
const TCVN3_TONED: [[u8; 5]; 12] = [
    [0xB5, 0xB6, 0xB7, 0xB8, 0xB9], // a
    [0xBB, 0xBC, 0xBD, 0xBE, 0xC6], // ă
    [0xC7, 0xC8, 0xC9, 0xCA, 0xCB], // â
    [0xCC, 0xCE, 0xCF, 0xD0, 0xD1], // e
    [0xD2, 0xD3, 0xD4, 0xD5, 0xD6], // ê
    [0xD7, 0xD8, 0xDC, 0xDD, 0xDE], // i
    [0xDF, 0xE1, 0xE2, 0xE3, 0xE4], // o
    [0xE5, 0xE6, 0xE7, 0xE8, 0xE9], // ô
    [0xEA, 0xEB, 0xEC, 0xED, 0xEE], // ơ
    [0xEF, 0xF1, 0xF2, 0xF3, 0xF4], // u
    [0xF5, 0xF6, 0xF7, 0xF8, 0xF9], // ư
    [0xFA, 0xFB, 0xFC, 0xFD, 0xFE], // y
];

/// TCVN3 single letters without tone.
const TCVN3_LETTERS: [(u8, char); 14] = [
    (0xA1, 'Ă'),
    (0xA2, 'Â'),
    (0xA3, 'Ê'),
    (0xA4, 'Ô'),
    (0xA5, 'Ơ'),
    (0xA6, 'Ư'),
    (0xA7, 'Đ'),
    (0xA8, 'ă'),
    (0xA9, 'â'),
    (0xAA, 'ê'),
    (0xAB, 'ô'),
    (0xAC, 'ơ'),
    (0xAD, 'ư'),
    (0xAE, 'đ'),
];

/// Decode a TCVN3 byte; `None` for bytes the table does not assign (ASCII and
/// the unassigned high bytes keep their Windows-1252 meaning).
pub fn tcvn3_decode(b: u8) -> Option<char> {
    if let Some(&(_, c)) = TCVN3_LETTERS.iter().find(|(x, _)| *x == b) {
        return Some(c);
    }
    for (row, codes) in TCVN3_TONED.iter().enumerate() {
        if let Some(tone) = codes.iter().position(|&x| x == b) {
            return Some(VOWELS[row][tone + 1]);
        }
    }
    None
}

/// Encode one character as a TCVN3 high byte.
pub fn tcvn3_encode(c: char) -> Option<u8> {
    if let Some(&(b, _)) = TCVN3_LETTERS.iter().find(|(_, x)| *x == c) {
        return Some(b);
    }
    for (row, forms) in VOWELS.iter().enumerate() {
        if let Some(tone) = forms.iter().position(|&x| x == c) {
            return if tone == 0 {
                None // plain a e i o u y are ASCII; ă â ê ô ơ ư handled above
            } else {
                Some(TCVN3_TONED[row][tone - 1])
            };
        }
    }
    None
}

// ---------------------------------------------------------------------------
// VNI-Windows. Shown as the Windows-1252 characters of each byte.
// ---------------------------------------------------------------------------

/// Tone marks (grave, hook, tilde, acute, dot) after a plain vowel.
const VNI_TONE: [char; 5] = ['ø', 'û', 'õ', 'ù', 'ï'];
const VNI_TONE_UP: [char; 5] = ['Ø', 'Û', 'Õ', 'Ù', 'Ï'];
/// Circumflex (no tone, grave, hook, tilde, acute, dot) after a / e / o.
const VNI_CIRC: [char; 6] = ['â', 'à', 'å', 'ã', 'á', 'ä'];
const VNI_CIRC_UP: [char; 6] = ['Â', 'À', 'Å', 'Ã', 'Á', 'Ä'];
/// Breve (no tone, grave, hook, tilde, acute, dot) after a.
const VNI_BREVE: [char; 6] = ['ê', 'è', 'ú', 'ü', 'é', 'ë'];
const VNI_BREVE_UP: [char; 6] = ['Ê', 'È', 'Ú', 'Ü', 'É', 'Ë'];

/// Single-byte VNI letters: (byte as Windows-1252 char, Unicode).
const VNI_SINGLE: [(char, char); 16] = [
    ('ñ', 'đ'),
    ('Ñ', 'Đ'),
    ('ô', 'ơ'),
    ('Ô', 'Ơ'),
    ('ö', 'ư'),
    ('Ö', 'Ư'),
    ('ì', 'ì'),
    ('Ì', 'Ì'),
    ('æ', 'ỉ'),
    ('Æ', 'Ỉ'),
    ('ó', 'ĩ'),
    ('Ó', 'Ĩ'),
    ('í', 'í'),
    ('Í', 'Í'),
    ('ò', 'ị'),
    ('Ò', 'Ị'),
];

/// `ỵ` / `Ỵ` also have single-byte forms.
const VNI_Y_DOT: [(char, char); 2] = [('î', 'ỵ'), ('Î', 'Ỵ')];

/// Row index into [`VOWELS`] and case for a VNI base character.
fn vni_base(c: char) -> Option<(usize, bool)> {
    let (row, upper) = match c {
        'a' => (0, false),
        'A' => (0, true),
        'e' => (3, false),
        'E' => (3, true),
        'o' => (6, false),
        'O' => (6, true),
        'ô' => (8, false), // ơ
        'Ô' => (8, true),
        'u' => (9, false),
        'U' => (9, true),
        'ö' => (10, false), // ư
        'Ö' => (10, true),
        'y' => (11, false),
        'Y' => (11, true),
        _ => return None,
    };
    Some((row, upper))
}

fn pick(row: usize, col: usize, upper: bool) -> char {
    if upper {
        VOWELS_UPPER[row][col]
    } else {
        VOWELS[row][col]
    }
}

/// Combine a VNI base and following mark into one letter.
pub fn vni_pair(base: char, mark: char) -> Option<char> {
    let (row, upper) = vni_base(base)?;
    let pos = |set: &[char]| set.iter().position(|&m| m == mark);
    if let Some(t) = pos(&VNI_TONE).or_else(|| pos(&VNI_TONE_UP)) {
        return Some(pick(row, t + 1, upper));
    }
    if matches!(row, 0 | 3 | 6) {
        if let Some(t) = pos(&VNI_CIRC).or_else(|| pos(&VNI_CIRC_UP)) {
            // a→â, e→ê, o→ô rows are 2, 4, 7.
            let target = match row {
                0 => 2,
                3 => 4,
                _ => 7,
            };
            return Some(pick(target, t, upper));
        }
    }
    if row == 0 {
        if let Some(t) = pos(&VNI_BREVE).or_else(|| pos(&VNI_BREVE_UP)) {
            return Some(pick(1, t, upper));
        }
    }
    None
}

/// A VNI single-byte letter.
pub fn vni_single(c: char) -> Option<char> {
    VNI_SINGLE
        .iter()
        .chain(VNI_Y_DOT.iter())
        .find(|(k, _)| *k == c)
        .map(|&(_, v)| v)
}

/// Whether `c` is one of the VNI combining mark bytes.
pub fn is_vni_mark(c: char) -> bool {
    VNI_TONE
        .iter()
        .chain(&VNI_TONE_UP)
        .chain(&VNI_CIRC)
        .chain(&VNI_CIRC_UP)
        .chain(&VNI_BREVE)
        .chain(&VNI_BREVE_UP)
        .any(|&m| m == c)
}

/// Encode one Unicode letter as one or two VNI characters.
pub fn vni_encode(c: char) -> Option<(char, Option<char>)> {
    if let Some(&(k, _)) = VNI_SINGLE.iter().find(|(_, v)| *v == c) {
        return Some((k, None));
    }
    if let Some(&(k, _)) = VNI_Y_DOT.iter().find(|(_, v)| *v == c) {
        return Some((k, None));
    }
    for (upper, table) in [(false, &VOWELS), (true, &VOWELS_UPPER)] {
        for (row, forms) in table.iter().enumerate() {
            let Some(col) = forms.iter().position(|&x| x == c) else {
                continue;
            };
            let (tone, circ, breve) = if upper {
                (&VNI_TONE_UP, &VNI_CIRC_UP, &VNI_BREVE_UP)
            } else {
                (&VNI_TONE, &VNI_CIRC, &VNI_BREVE)
            };
            let base = |b: char| if upper { b.to_ascii_uppercase() } else { b };
            return match row {
                1 => Some((base('a'), Some(breve[col]))),
                2 => Some((base('a'), Some(circ[col]))),
                4 => Some((base('e'), Some(circ[col]))),
                7 => Some((base('o'), Some(circ[col]))),
                5 => None, // i-forms are all single bytes (VNI_SINGLE) or ASCII
                _ if col == 0 => match row {
                    8 => Some((if upper { 'Ô' } else { 'ô' }, None)),
                    10 => Some((if upper { 'Ö' } else { 'ö' }, None)),
                    _ => None, // ASCII vowel
                },
                _ => {
                    let b = match row {
                        8 => {
                            if upper {
                                'Ô'
                            } else {
                                'ô'
                            }
                        }
                        10 => {
                            if upper {
                                'Ö'
                            } else {
                                'ö'
                            }
                        }
                        _ => base(forms[0].to_ascii_lowercase()),
                    };
                    Some((b, Some(tone[col - 1])))
                }
            };
        }
    }
    None
}
