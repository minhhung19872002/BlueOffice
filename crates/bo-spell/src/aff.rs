//! Affix file (`.aff`) and dictionary file (`.dic`) parsing, after the public hunspell(5) format
//! description: flags, affix classes with conditions, replacement / key / try tables, compounding and
//! the special flags (NOSUGGEST, KEEPCASE, FORBIDDENWORD, NEEDAFFIX, ONLYINCOMPOUND).

use std::collections::HashMap;

/// A flag value (character, two-character, numeric or UTF-8 flag, all mapped to a number).
pub type Flag = u32;

/// How flags are written (`FLAG` directive).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlagMode {
    /// One (ISO 8859-1 / ASCII) character per flag (default).
    #[default]
    Char,
    /// Two characters per flag (`FLAG long`).
    Long,
    /// Comma-separated decimal numbers (`FLAG num`).
    Num,
    /// One Unicode character per flag (`FLAG UTF-8`).
    Utf8,
}

impl FlagMode {
    /// Parses a flag string into flags.
    pub fn parse(self, s: &str) -> Vec<Flag> {
        match self {
            FlagMode::Char | FlagMode::Utf8 => s.chars().map(|c| c as Flag).collect(),
            FlagMode::Long => {
                let cs: Vec<char> = s.chars().collect();
                cs.chunks(2)
                    .map(|c| {
                        let a = c[0] as Flag;
                        let b = c.get(1).map_or(0, |&b| b as Flag);
                        (a << 16) | b
                    })
                    .collect()
            }
            FlagMode::Num => s
                .split(',')
                .filter_map(|n| n.trim().parse::<Flag>().ok())
                .collect(),
        }
    }
}

/// One element of an affix condition.
#[derive(Debug, Clone, PartialEq, Eq)]
enum CondChar {
    Any,
    Char(char),
    Set { neg: bool, chars: Vec<char> },
}

impl CondChar {
    fn matches(&self, c: char) -> bool {
        match self {
            CondChar::Any => true,
            CondChar::Char(x) => *x == c,
            CondChar::Set { neg, chars } => chars.contains(&c) != *neg,
        }
    }
}

/// An affix condition (a simplified regular expression of characters, `.` and `[...]` / `[^...]`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Condition(Vec<CondChar>);

impl Condition {
    /// Parses a condition (`.` = always).
    pub fn parse(s: &str) -> Condition {
        if s == "." {
            return Condition(Vec::new());
        }
        let mut out = Vec::new();
        let mut it = s.chars().peekable();
        while let Some(c) = it.next() {
            match c {
                '.' => out.push(CondChar::Any),
                '[' => {
                    let neg = it.peek() == Some(&'^');
                    if neg {
                        it.next();
                    }
                    let mut chars = Vec::new();
                    for d in it.by_ref() {
                        if d == ']' {
                            break;
                        }
                        chars.push(d);
                    }
                    out.push(CondChar::Set { neg, chars });
                }
                c => out.push(CondChar::Char(c)),
            }
        }
        Condition(out)
    }

    /// The condition matches the start of `word` (prefixes).
    pub fn matches_start(&self, word: &str) -> bool {
        let cs: Vec<char> = word.chars().collect();
        cs.len() >= self.0.len() && self.0.iter().zip(&cs).all(|(k, &c)| k.matches(c))
    }

    /// The condition matches the end of `word` (suffixes).
    pub fn matches_end(&self, word: &str) -> bool {
        let cs: Vec<char> = word.chars().collect();
        if cs.len() < self.0.len() {
            return false;
        }
        let tail = &cs[cs.len() - self.0.len()..];
        self.0.iter().zip(tail).all(|(k, &c)| k.matches(c))
    }
}

/// One affix rule of a class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Affix {
    /// Class flag.
    pub flag: Flag,
    /// Combines with affixes of the other kind (`Y` in the class header).
    pub cross: bool,
    /// Characters removed from the root before adding.
    pub strip: String,
    /// Characters added.
    pub add: String,
    /// Continuation flags (affixes that may follow this one).
    pub cont: Vec<Flag>,
    /// Condition on the root.
    pub cond: Condition,
}

/// Parsed affix file.
#[derive(Debug, Clone, Default)]
pub struct AffixFile {
    /// How flags are written (`FLAG`).
    pub flag_mode: FlagMode,
    /// Prefix rules (`PFX`).
    pub prefixes: Vec<Affix>,
    /// Suffix rules (`SFX`).
    pub suffixes: Vec<Affix>,
    /// Characters tried for suggestions (`TRY`).
    pub try_chars: String,
    /// Keyboard rows for neighbour suggestions (`KEY`).
    pub key: Vec<String>,
    /// Replacement table (`REP`): common misspellings.
    pub rep: Vec<(String, String)>,
    /// Extra word characters (`WORDCHARS`).
    pub wordchars: String,
    /// Correct but never suggested (`NOSUGGEST`).
    pub nosuggest: Option<Flag>,
    /// Accepted only with the dictionary capitalisation (`KEEPCASE`).
    pub keepcase: Option<Flag>,
    /// Explicitly wrong word (`FORBIDDENWORD`).
    pub forbidden: Option<Flag>,
    /// Root accepted only with an affix (`NEEDAFFIX`).
    pub needaffix: Option<Flag>,
    /// Root accepted only inside compounds (`ONLYINCOMPOUND`).
    pub onlyincompound: Option<Flag>,
    /// Any position in a compound (`COMPOUNDFLAG`).
    pub compound_flag: Option<Flag>,
    /// First part of a compound (`COMPOUNDBEGIN`).
    pub compound_begin: Option<Flag>,
    /// Middle part of a compound (`COMPOUNDMIDDLE`).
    pub compound_middle: Option<Flag>,
    /// Last part of a compound (`COMPOUNDEND`).
    pub compound_end: Option<Flag>,
    /// Minimum length of compound parts (`COMPOUNDMIN`, default 3).
    pub compound_min: usize,
    /// `AF` flag vector aliases (1-based in the .dic file).
    pub aliases: Vec<Vec<Flag>>,
}

/// Decodes a file given its `SET` encoding (UTF-8 and ISO 8859-1 supported).
pub fn decode(bytes: &[u8]) -> Result<String, String> {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]).into_owned();
    let set = head
        .lines()
        .find_map(|l| {
            l.trim()
                .strip_prefix("SET ")
                .map(|s| s.trim().to_ascii_uppercase())
        })
        .unwrap_or_else(|| "UTF-8".into());
    let body = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    match set.as_str() {
        "UTF-8" => std::str::from_utf8(body)
            .map(str::to_owned)
            .map_err(|e| format!("not UTF-8: {e}")),
        "ISO8859-1" | "ISO-8859-1" => Ok(body.iter().map(|&b| char::from(b)).collect()),
        other => Err(format!("unsupported encoding {other}")),
    }
}

impl AffixFile {
    /// Parses an affix file.
    pub fn parse(text: &str) -> Result<AffixFile, String> {
        let mut a = AffixFile {
            compound_min: 3,
            ..AffixFile::default()
        };
        // FLAG must be known before any flag is read.
        for line in text.lines() {
            let mut f = line.split_whitespace();
            if f.next() == Some("FLAG") {
                a.flag_mode = match f.next().unwrap_or("") {
                    "long" => FlagMode::Long,
                    "num" => FlagMode::Num,
                    "UTF-8" => FlagMode::Utf8,
                    _ => FlagMode::Char,
                };
            }
        }
        let mode = a.flag_mode;
        let one = |s: Option<&str>| s.and_then(|v| mode.parse(v).first().copied());
        let mut lines = text.lines().peekable();
        while let Some(line) = lines.next() {
            let line = line.split('#').next().unwrap_or("").trim_end();
            let f: Vec<&str> = line.split_whitespace().collect();
            let Some(&key) = f.first() else { continue };
            match key {
                "TRY" => a.try_chars = f.get(1).copied().unwrap_or("").to_owned(),
                "WORDCHARS" => a.wordchars = f.get(1).copied().unwrap_or("").to_owned(),
                "KEY" => {
                    a.key = f
                        .get(1)
                        .copied()
                        .unwrap_or("")
                        .split('|')
                        .map(str::to_owned)
                        .collect();
                }
                "NOSUGGEST" => a.nosuggest = one(f.get(1).copied()),
                "KEEPCASE" => a.keepcase = one(f.get(1).copied()),
                "FORBIDDENWORD" => a.forbidden = one(f.get(1).copied()),
                "NEEDAFFIX" | "PSEUDOROOT" => a.needaffix = one(f.get(1).copied()),
                "ONLYINCOMPOUND" => a.onlyincompound = one(f.get(1).copied()),
                "COMPOUNDFLAG" => a.compound_flag = one(f.get(1).copied()),
                "COMPOUNDBEGIN" => a.compound_begin = one(f.get(1).copied()),
                "COMPOUNDMIDDLE" => a.compound_middle = one(f.get(1).copied()),
                "COMPOUNDEND" => a.compound_end = one(f.get(1).copied()),
                "COMPOUNDMIN" => {
                    a.compound_min = f.get(1).and_then(|v| v.parse().ok()).unwrap_or(3).max(1);
                }
                "REP" if f.len() >= 3 => {
                    a.rep.push((f[1].replace('_', " "), f[2].replace('_', " ")));
                }
                "AF" if f.len() >= 2 && f[1].parse::<usize>().is_err() => {
                    a.aliases.push(mode.parse(f[1]));
                }
                "PFX" | "SFX" if f.len() >= 4 => {
                    // Header: PFX flag cross count; rules: PFX flag strip add[/cont] [cond].
                    let flag = one(Some(f[1])).ok_or("affix flag")?;
                    let cross = f[2] == "Y";
                    let count: usize = f[3].parse().map_err(|_| format!("affix count: {line}"))?;
                    for _ in 0..count {
                        let Some(rule) = lines.next() else { break };
                        let r: Vec<&str> = rule.split_whitespace().collect();
                        if r.len() < 4 || r[0] != key {
                            return Err(format!("bad affix rule: {rule}"));
                        }
                        let strip = if r[2] == "0" {
                            String::new()
                        } else {
                            r[2].to_owned()
                        };
                        let (add, cont) = match r[3].split_once('/') {
                            Some((a2, c)) => {
                                let cont = if a.aliases.is_empty() || c.parse::<usize>().is_err() {
                                    mode.parse(c)
                                } else {
                                    c.parse::<usize>()
                                        .ok()
                                        .and_then(|i| a.aliases.get(i.wrapping_sub(1)).cloned())
                                        .unwrap_or_default()
                                };
                                (a2, cont)
                            }
                            None => (r[3], Vec::new()),
                        };
                        let add = if add == "0" {
                            String::new()
                        } else {
                            add.to_owned()
                        };
                        let cond = Condition::parse(r.get(4).copied().unwrap_or("."));
                        let affix = Affix {
                            flag,
                            cross,
                            strip,
                            add,
                            cont,
                            cond,
                        };
                        if key == "PFX" {
                            a.prefixes.push(affix);
                        } else {
                            a.suffixes.push(affix);
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(a)
    }

    /// Parses a `.dic` file into word → flag sets (homonyms keep separate sets).
    pub fn parse_dic(&self, text: &str) -> HashMap<String, Vec<Vec<Flag>>> {
        let mut words: HashMap<String, Vec<Vec<Flag>>> = HashMap::new();
        for (i, line) in text.lines().enumerate() {
            let line = line.trim_end();
            if line.is_empty() || (i == 0 && line.trim().parse::<usize>().is_ok()) {
                continue;
            }
            // Morphological fields follow a tab (or a space before "xx:" fields).
            let entry = line.split('\t').next().unwrap_or("");
            let entry = entry
                .split(' ')
                .take_while(|p| !(p.len() > 3 && p.as_bytes().get(2) == Some(&b':')))
                .collect::<Vec<_>>()
                .join(" ");
            let (word, flags) = split_word_flags(&entry);
            if word.is_empty() {
                continue;
            }
            let flags = match flags {
                None => Vec::new(),
                Some(f) if !self.aliases.is_empty() && f.parse::<usize>().is_ok() => f
                    .parse::<usize>()
                    .ok()
                    .and_then(|i| self.aliases.get(i.wrapping_sub(1)).cloned())
                    .unwrap_or_default(),
                Some(f) => self.flag_mode.parse(f),
            };
            words.entry(word).or_default().push(flags);
        }
        words
    }
}

/// `word/flags` with `\/` escaping a slash inside the word.
fn split_word_flags(entry: &str) -> (String, Option<&str>) {
    let bytes = entry.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if bytes[i] == b'/' && i > 0 {
            return (entry[..i].replace("\\/", "/"), Some(&entry[i + 1..]));
        }
        i += 1;
    }
    (entry.replace("\\/", "/"), None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conditions() {
        assert!(Condition::parse("[^y]").matches_end("walk"));
        assert!(!Condition::parse("[^y]").matches_end("carry"));
        assert!(Condition::parse("[^aeiou]y").matches_end("carry"));
        assert!(!Condition::parse("[^aeiou]y").matches_end("play"));
        assert!(Condition::parse(".").matches_end(""));
        assert!(Condition::parse("un").matches_start("undo"));
    }

    #[test]
    fn flag_modes() {
        assert_eq!(FlagMode::Num.parse("12,3"), vec![12, 3]);
        assert_eq!(FlagMode::Long.parse("AaBb").len(), 2);
        assert_eq!(FlagMode::Char.parse("AB"), vec![65, 66]);
    }

    #[test]
    fn dic_entries_with_slash_and_morphology() {
        let a = AffixFile::parse("FLAG long\n").unwrap();
        let w = a.parse_dic("3\nand\\/or\nwalk/AaBb po:verb\nfoo/Aa\tis:x\n");
        assert!(w.contains_key("and/or"));
        assert_eq!(w["walk"][0].len(), 2);
        assert_eq!(w["foo"][0].len(), 1);
    }
}
