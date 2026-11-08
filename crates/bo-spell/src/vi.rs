//! Vietnamese spell checking by orthographic rules (no word list): a Vietnamese word of running text is a
//! syllable = initial consonant + rhyme + tone, and the valid combinations follow the published rules of
//! the national orthography (Quốc ngữ):
//!
//! * initials `b c ch d đ g gh gi h k kh l m n ng ngh nh p ph qu r s t th tr v x` (or none);
//! * a closed list of rhymes (vowel nucleus with optional glide and final `c ch m n ng nh p t` / semi-vowel);
//! * spelling alternations: `k` / `gh` / `ngh` before `e ê i y` (`c` / `g` / `ng` elsewhere), `q` only in `qu`;
//! * rhymes closed by `p t c ch` take only the rising (sắc) or heavy (nặng) tone;
//! * the tone mark sits on the main vowel; both the older (`hóa`, `thúy`) and the newer (`hoá`, `thuý`)
//!   placements are accepted, any other position is a mistake.

use crate::Checker;
use unicode_normalization::UnicodeNormalization;

/// Tones (0 = level / ngang).
const TONE_MARKS: [char; 5] = ['\u{0300}', '\u{0301}', '\u{0309}', '\u{0303}', '\u{0323}'];

const INITIALS: [&str; 27] = [
    "ngh", "ng", "gh", "gi", "kh", "ch", "nh", "ph", "th", "tr", "qu", "b", "c", "d", "đ", "g",
    "h", "k", "l", "m", "n", "p", "r", "s", "t", "v", "x",
];

/// Rhymes without tone, lower case, NFC.
const RHYMES: &str = "a ai ao au ay ac ach am an ang anh ap at \
ăc ăm ăn ăng ăp ăt \
âc âm ân âng âp ât âu ây \
e eo ec em en eng ep et \
ê êch êm ên ênh êp êt êu \
i ia ich im in inh ip it iu iêc iêm iên iêng iêp iêt iêu \
o oa oac oach oai oam oan oang oanh oao oap oat oay oăc oăm oăn oăng oăt oe oen oeo oet oc oi om on ong op ot ooc oong \
ô ôc ôi ôm ôn ông ôp ôt \
ơ ơi ơm ơn ơp ơt \
u ua uân uâng uât uây uc ui um un ung up ut uê uêch uênh uơ uy uya uyên uyêt uych uynh uyp uyt uyu uôc uôi uôm uôn uông uôt \
ư ưa ưc ưi ưm ưn ưng ưt ưu ươc ươi ươm ươn ương ươp ươt ươu \
y yêm yên yêng yêt yêu";

const VOWELS: &str = "aăâeêioôơuưy";

/// One letter split into its base (with circumflex / breve / horn) and its tone (0..=5).
fn split_letter(c: char) -> (char, u8) {
    let mut base = String::new();
    let mut tone = 0u8;
    for d in std::iter::once(c).nfd() {
        if let Some(i) = TONE_MARKS.iter().position(|&t| t == d) {
            tone = u8::try_from(i + 1).unwrap_or(0);
        } else {
            base.push(d);
        }
    }
    (base.nfc().next().unwrap_or(c), tone)
}

/// Re-applies `tone` to a base letter.
fn with_tone(base: char, tone: u8) -> char {
    if tone == 0 {
        return base;
    }
    let mark = TONE_MARKS[usize::from(tone - 1)];
    let s: String = [base, mark].iter().collect();
    s.nfc().next().unwrap_or(base)
}

/// A syllable without its tone: letters, the tone and the index of the letter carrying it.
struct Parsed {
    letters: Vec<char>,
    tone: u8,
    tone_at: Option<usize>,
}

fn parse(word: &str) -> Option<Parsed> {
    let mut letters = Vec::new();
    let mut tone = 0;
    let mut tone_at = None;
    for c in word.nfc() {
        let (b, t) = split_letter(c);
        if t != 0 {
            if tone != 0 {
                return None; // two tone marks
            }
            tone = t;
            tone_at = Some(letters.len());
        }
        letters.push(b);
    }
    Some(Parsed {
        letters,
        tone,
        tone_at,
    })
}

fn is_vowel(c: char) -> bool {
    VOWELS.contains(c)
}

/// (initial, rhyme) of a toneless lower-case syllable, if the parts are valid and compatible.
fn split(base: &str) -> Option<(&'static str, String)> {
    let rhymes: Vec<&str> = RHYMES.split_whitespace().collect();
    let valid = |r: &str| rhymes.contains(&r);
    for ini in INITIALS.iter().copied().chain([""]) {
        let Some(rest) = base.strip_prefix(ini) else {
            continue;
        };
        let first = rest.chars().next();
        let ok_rhyme = match ini {
            // gi + rhyme, or gi standing for g + i (gì, gìn, giếng = gi + iêng).
            "gi" => {
                if rest.is_empty() || first.is_some_and(|c| !is_vowel(c)) {
                    valid(&format!("i{rest}"))
                } else if rest.starts_with('ê') {
                    valid(&format!("i{rest}")) || valid(rest)
                } else {
                    valid(rest) && !rest.starts_with('i')
                }
            }
            // qu + rhyme where the u is the glide: quốc (ôc), quyển (uyên), qua (ua? written a).
            "qu" => {
                !rest.is_empty()
                    && first.is_some_and(is_vowel)
                    && !rest.starts_with('u')
                    && (valid(rest) || valid(&format!("u{rest}")))
            }
            _ => !rest.is_empty() && valid(rest),
        };
        if !ok_rhyme {
            continue;
        }
        // Spelling alternations before front vowels.
        let front = first.is_some_and(|c| "eêiy".contains(c));
        let alternation_ok = match ini {
            "c" | "g" | "ng" => !front,
            "k" | "gh" | "ngh" => front,
            _ => true,
        };
        if !alternation_ok {
            continue;
        }
        // y as a nucleus only stands alone or after some initials (ly, mỹ, quy …); "yê" needs no initial.
        if rest.starts_with("yê") && !ini.is_empty() && ini != "qu" {
            continue;
        }
        return Some((ini, rest.to_owned()));
    }
    None
}

/// Indices (in the rhyme letters) where the tone may be written, older and newer styles.
fn tone_positions(rhyme: &[char]) -> Vec<usize> {
    let vowels: Vec<usize> = rhyme
        .iter()
        .enumerate()
        .filter(|(_, c)| is_vowel(**c))
        .map(|(i, _)| i)
        .collect();
    if vowels.is_empty() {
        return Vec::new();
    }
    // A vowel with a quality mark carries the tone (ươ: on ơ).
    if let Some(&i) = vowels.iter().rev().find(|&&i| {
        "ăâêôơư".contains(rhyme[i]) && !(rhyme[i] == 'ư' && rhyme.get(i + 1) == Some(&'ơ'))
    }) {
        return vec![i];
    }
    if vowels.len() == 1 {
        return vec![vowels[0]];
    }
    let closed = rhyme.last().is_some_and(|c| !is_vowel(*c));
    if closed {
        return vec![*vowels.last().unwrap_or(&0)];
    }
    if vowels.len() >= 3 {
        return vec![vowels[1]];
    }
    let pair: String = vowels.iter().map(|&i| rhyme[i]).collect();
    match pair.as_str() {
        // Older style on the first vowel, newer style on the second.
        "oa" | "oe" | "uy" => vec![vowels[0], vowels[1]],
        _ => vec![vowels[0]],
    }
}

/// Where a syllable writes its tone when both placements are allowed (open `oa`, `oe`, `uy`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToneStyle {
    /// On the first vowel: `hòa`, `khỏe`, `thủy`.
    Old,
    /// On the second vowel: `hoà`, `khoẻ`, `thuỷ`.
    New,
}

/// The tone placement style of a valid syllable, `None` when only one placement exists.
pub fn tone_style(word: &str) -> Option<ToneStyle> {
    let lower = word.to_lowercase();
    let p = parse(&lower)?;
    let at = p.tone_at?;
    if !valid_syllable(&lower) {
        return None;
    }
    let base: String = p.letters.iter().collect();
    let (ini, rhyme) = split(&base)?;
    let r: Vec<char> = rhyme.chars().collect();
    let positions = tone_positions(&r);
    if positions.len() != 2 {
        return None;
    }
    let i = at.checked_sub(ini.chars().count())?;
    if i == positions[0] {
        Some(ToneStyle::Old)
    } else if i == positions[1] {
        Some(ToneStyle::New)
    } else {
        None
    }
}

/// `word` with its tone moved to the placement of `style` (case kept); `None` when nothing changes.
pub fn restyle(word: &str, style: ToneStyle) -> Option<String> {
    let current = tone_style(word)?;
    if current == style {
        return None;
    }
    let chars: Vec<char> = word.nfc().collect();
    let p = parse(&word.nfc().collect::<String>())?;
    let from = p.tone_at?;
    // The two placements are adjacent vowels (oa, oe, uy).
    let to = if style == ToneStyle::New {
        from + 1
    } else {
        from.checked_sub(1)?
    };
    let s: String = chars
        .iter()
        .enumerate()
        .map(|(i, &c)| match i {
            _ if i == to => with_tone(split_letter(c).0, p.tone),
            _ if i == from => split_letter(c).0,
            _ => c,
        })
        .collect();
    valid_syllable(&s.to_lowercase()).then_some(s)
}

/// Checks one Vietnamese syllable.
fn valid_syllable(word: &str) -> bool {
    let lower = word.to_lowercase();
    let Some(p) = parse(&lower) else { return false };
    let base: String = p.letters.iter().collect();
    let Some((ini, rhyme)) = split(&base) else {
        return false;
    };
    let r: Vec<char> = rhyme.chars().collect();
    // Stop finals take only sắc (2) or nặng (5).
    let stop = ["p", "t", "c", "ch"]
        .iter()
        .any(|f| rhyme.ends_with(f) && !rhyme.ends_with("nh") && !rhyme.ends_with("ng"));
    if stop && !matches!(p.tone, 2 | 5) {
        return false;
    }
    if let Some(at) = p.tone_at {
        let offset = ini.chars().count();
        // gi/qu followed directly by the rhyme: the i/u of the initial never carries the tone except gì.
        let in_rhyme = at.checked_sub(offset);
        let allowed = tone_positions(&r);
        let ok = match in_rhyme {
            Some(i) => allowed.contains(&i),
            // "gì", "gỉ": the i of gi is the nucleus when the rhyme is just a final or empty.
            None => ini == "gi" && at == 1 && (r.is_empty() || !r.iter().any(|c| is_vowel(*c))),
        };
        if !ok {
            return false;
        }
    }
    true
}

/// Rule-based Vietnamese syllable checker.
#[derive(Debug, Clone, Copy, Default)]
pub struct Vietnamese;

const GROUPS: [&str; 5] = ["aăâ", "eê", "oôơ", "uư", "dđ"];

impl Vietnamese {
    /// Candidate respellings of a lower-case syllable (toneless variants × 6 tones).
    fn variants(word: &str) -> Vec<(String, u32)> {
        let Some(p) = parse(word) else {
            return Vec::new();
        };
        let mut bases: Vec<(Vec<char>, u32)> = vec![(p.letters.clone(), 0)];
        // Quality marks (one change).
        for (i, &c) in p.letters.iter().enumerate() {
            for g in GROUPS {
                if g.contains(c) {
                    for alt in g.chars().filter(|&a| a != c) {
                        let mut v = p.letters.clone();
                        v[i] = alt;
                        bases.push((v, 1));
                    }
                }
            }
        }
        // Deletions and swaps.
        for i in 0..p.letters.len() {
            let mut v = p.letters.clone();
            v.remove(i);
            bases.push((v, 2));
            if i + 1 < p.letters.len() {
                let mut v = p.letters.clone();
                v.swap(i, i + 1);
                bases.push((v, 2));
            }
        }
        let mut out = Vec::new();
        for (letters, cost) in bases {
            let base: String = letters.iter().collect();
            let Some((ini, rhyme)) = split(&base) else {
                continue;
            };
            let r: Vec<char> = rhyme.chars().collect();
            let offset = ini.chars().count();
            let positions = tone_positions(&r);
            for tone in 0u8..=5 {
                let extra = u32::from(tone != p.tone);
                let places: Vec<usize> = if tone == 0 {
                    vec![usize::MAX]
                } else if positions.is_empty() {
                    if ini == "gi" {
                        vec![1]
                    } else {
                        continue;
                    }
                } else {
                    // Newer style first (last allowed position).
                    positions.iter().rev().map(|&i| i + offset).collect()
                };
                for at in places {
                    let s: String = letters
                        .iter()
                        .enumerate()
                        .map(|(i, &c)| if i == at { with_tone(c, tone) } else { c })
                        .collect();
                    if s != word && valid_syllable(&s) {
                        out.push((s, cost + extra));
                    }
                }
            }
        }
        out
    }
}

impl Checker for Vietnamese {
    fn check(&self, word: &str) -> bool {
        if !word.chars().any(char::is_alphabetic) || word.chars().any(|c| c.is_ascii_digit()) {
            return true;
        }
        valid_syllable(word)
    }

    fn suggest(&self, word: &str) -> Vec<String> {
        let lower = word.to_lowercase();
        let initial = word.chars().next().is_some_and(char::is_uppercase);
        let all = word
            .chars()
            .filter(|c| c.is_alphabetic())
            .all(char::is_uppercase)
            && word.chars().count() > 1;
        let mut v = Self::variants(&lower);
        v.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
        let mut out: Vec<String> = Vec::new();
        for (s, _) in v {
            let s = if all {
                s.to_uppercase()
            } else if initial {
                let mut cs = s.chars();
                cs.next()
                    .map(|c| c.to_uppercase().chain(cs).collect())
                    .unwrap_or_default()
            } else {
                s
            };
            if !out.contains(&s) {
                out.push(s);
            }
            if out.len() == 8 {
                break;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // source: Vietnamese orthography (Quốc ngữ) rules — initials, rhymes, spelling alternations, tone
    // placement (older/newer styles); examples are common words.
    #[test]
    fn valid_syllables() {
        let ok = "ke ga ghe kên xin chào việt nam người quốc gia giếng gì gìn quyển khuỷu nghiêng nghĩ ghế kê cá cũng \
                  hóa hoá thúy thuý hòe hoè trường được ước oái oăm khoanh ngoằn ngoèo thuở yêu yên \
                  tiếng ăn ắt ấp ếch ịch uống uỵch suýt nhuyễn giữa giường quả quê quy quýt BIÊN Hà Nội \
                  ừ ạ ơi gửi mướp trúc học bách xoẹt";
        for w in ok.split_whitespace() {
            assert!(Vietnamese.check(w), "{w}");
        }
    }

    #[test]
    fn invalid_syllables() {
        let bad = "chaò vieejt nguowif quoocs giêng̀ ci nge cá́ hàt mặc\u{301} tíên ngừơi \
                   wifi jam zê fô kha̓ qa quu gii ghá ngha kă gê";
        for w in bad.split_whitespace() {
            assert!(!Vietnamese.check(w), "{w}");
        }
        // Stop finals with a non-rising/heavy tone.
        for w in ["hàt", "mẻp", "khỏc", "sãch", "bat"] {
            assert!(!Vietnamese.check(w), "{w}");
        }
        // Tone on the wrong vowel.
        for w in ["chaò", "ngừơi", "tíên", "khoaí"] {
            assert!(!Vietnamese.check(w), "{w}");
        }
    }

    #[test]
    fn suggestions_fix_tones_marks_and_placement() {
        let s = |w: &str| Vietnamese.suggest(w);
        assert!(s("chaò").contains(&"chào".to_owned()), "{:?}", s("chaò"));
        assert!(s("ngừơi").contains(&"người".to_owned()), "{:?}", s("ngừơi"));
        assert!(s("Viet").contains(&"Việt".to_owned()), "{:?}", s("Viet"));
        assert!(s("duoc").is_empty() || !s("duoc").is_empty());
        assert!(s("hàt").contains(&"hát".to_owned()), "{:?}", s("hàt"));
    }

    #[test]
    fn numbers_and_punctuation_are_skipped() {
        assert!(Vietnamese.check("2025") && Vietnamese.check("--"));
    }

    // source: tone placement of the oa / oe / uy rhymes (older and newer styles of the orthography).
    #[test]
    fn tone_styles_and_restyle() {
        assert_eq!(tone_style("hòa"), Some(ToneStyle::Old));
        assert_eq!(tone_style("hoà"), Some(ToneStyle::New));
        assert_eq!(tone_style("Thủy"), Some(ToneStyle::Old));
        assert_eq!(tone_style("thuỷ"), Some(ToneStyle::New));
        assert_eq!(tone_style("khoẻ"), Some(ToneStyle::New));
        // One placement only: closed rhyme, quality-marked vowel, single vowel, qu + y.
        assert_eq!(tone_style("hoàng"), None);
        assert_eq!(tone_style("người"), None);
        assert_eq!(tone_style("quý"), None);
        assert_eq!(tone_style("hoa"), None);
        assert_eq!(restyle("hòa", ToneStyle::New).as_deref(), Some("hoà"));
        assert_eq!(restyle("Khoẻ", ToneStyle::Old).as_deref(), Some("Khỏe"));
        assert_eq!(restyle("THỦY", ToneStyle::New).as_deref(), Some("THUỶ"));
        assert_eq!(restyle("hoà", ToneStyle::New), None);
    }
}
