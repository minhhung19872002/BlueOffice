//! English word forms and Soundex for Find's "Find all word forms" and "Sounds like" options
//! (BlueDoc round 10). Our own rules: regular inflections (-s / -es / -ies, -ed / -d / -ied,
//! -ing with a dropped e or a doubled final consonant), comparatives of short adjectives and a
//! small table of common irregular verbs; Soundex as published by the U.S. National Archives
//! ("The Soundex Indexing System").

/// Irregular verbs: base, past, past participle, third person (where irregular).
const IRREGULAR: &[&[&str]] = &[
    &["be", "am", "is", "are", "was", "were", "been", "being"],
    &["have", "has", "had", "having"],
    &["do", "does", "did", "done", "doing"],
    &["go", "goes", "went", "gone", "going"],
    &["run", "runs", "ran", "running"],
    &["see", "sees", "saw", "seen", "seeing"],
    &["come", "comes", "came", "coming"],
    &["take", "takes", "took", "taken", "taking"],
    &["make", "makes", "made", "making"],
    &["get", "gets", "got", "gotten", "getting"],
    &["give", "gives", "gave", "given", "giving"],
    &["know", "knows", "knew", "known", "knowing"],
    &["think", "thinks", "thought", "thinking"],
    &["say", "says", "said", "saying"],
    &["find", "finds", "found", "finding"],
    &["write", "writes", "wrote", "written", "writing"],
    &["begin", "begins", "began", "begun", "beginning"],
    &["bring", "brings", "brought", "bringing"],
    &["buy", "buys", "bought", "buying"],
    &["keep", "keeps", "kept", "keeping"],
    &["leave", "leaves", "left", "leaving"],
    &["meet", "meets", "met", "meeting"],
    &["pay", "pays", "paid", "paying"],
    &["send", "sends", "sent", "sending"],
    &["sit", "sits", "sat", "sitting"],
    &["speak", "speaks", "spoke", "spoken", "speaking"],
    &["stand", "stands", "stood", "standing"],
    &["tell", "tells", "told", "telling"],
    &["eat", "eats", "ate", "eaten", "eating"],
    &["fall", "falls", "fell", "fallen", "falling"],
    &["feel", "feels", "felt", "feeling"],
    &["hold", "holds", "held", "holding"],
    &["lose", "loses", "lost", "losing"],
    &["read", "reads", "reading"],
    &["sell", "sells", "sold", "selling"],
    &["teach", "teaches", "taught", "teaching"],
    &["win", "wins", "won", "winning"],
    &["child", "children"],
    &["man", "men"],
    &["woman", "women"],
    &["mouse", "mice"],
    &["foot", "feet"],
    &["tooth", "teeth"],
    &["person", "people"],
    &["good", "better", "best"],
    &["bad", "worse", "worst"],
];

fn vowel(c: char) -> bool {
    matches!(c, 'a' | 'e' | 'i' | 'o' | 'u')
}

/// Consonant–vowel–consonant ending of a short word (stop → stopping, stopped).
fn doubles(w: &str) -> bool {
    let c: Vec<char> = w.chars().collect();
    let n = c.len();
    n >= 3
        && !vowel(c[n - 1])
        && !matches!(c[n - 1], 'w' | 'x' | 'y')
        && vowel(c[n - 2])
        && !vowel(c[n - 3])
        && c.iter().filter(|c| vowel(**c)).count() == 1
}

/// Regular inflections of a base word.
fn inflect(base: &str) -> Vec<String> {
    let mut out = vec![base.to_owned()];
    let last = base.chars().last().unwrap_or(' ');
    let before = base.chars().rev().nth(1).unwrap_or(' ');
    let stem_y = base.strip_suffix('y').filter(|_| !vowel(before));
    // -s / -es / -ies
    if let Some(s) = stem_y {
        out.push(format!("{s}ies"));
        out.push(format!("{s}ied"));
    } else if base.ends_with(['s', 'x', 'z']) || base.ends_with("ch") || base.ends_with("sh") {
        out.push(format!("{base}es"));
    } else {
        out.push(format!("{base}s"));
    }
    // -ed / -ing / -er / -est
    if last == 'e' {
        let s = &base[..base.len() - 1];
        out.push(format!("{base}d"));
        out.push(format!("{s}ing"));
        out.push(format!("{base}r"));
        out.push(format!("{base}st"));
    } else if doubles(base) {
        out.push(format!("{base}{last}ed"));
        out.push(format!("{base}{last}ing"));
        out.push(format!("{base}{last}er"));
        out.push(format!("{base}{last}est"));
    } else if stem_y.is_none() {
        out.push(format!("{base}ed"));
        out.push(format!("{base}ing"));
        out.push(format!("{base}er"));
        out.push(format!("{base}est"));
    } else {
        out.push(format!("{base}ing"));
    }
    out
}

/// Base words a word may be an inflection of (itself included).
fn bases(w: &str) -> Vec<String> {
    let mut out = vec![w.to_owned()];
    let mut add = |s: String| {
        if s.chars().count() >= 2 && !out.contains(&s) {
            out.push(s);
        }
    };
    for (suffix, repl) in [
        ("ies", "y"),
        ("ied", "y"),
        ("es", ""),
        ("s", ""),
        ("ed", ""),
        ("ed", "e"),
        ("d", ""),
        ("ing", ""),
        ("ing", "e"),
        ("er", ""),
        ("est", ""),
    ] {
        if let Some(stem) = w.strip_suffix(suffix) {
            add(format!("{stem}{repl}"));
            // stopped → stop, running → run
            let c: Vec<char> = stem.chars().collect();
            if c.len() >= 2 && c[c.len() - 1] == c[c.len() - 2] && repl.is_empty() {
                add(c[..c.len() - 1].iter().collect());
            }
        }
    }
    out
}

/// Every form of the word `query` (lower case).
pub(crate) fn word_forms(query: &str) -> Vec<String> {
    let w = query.to_lowercase();
    if w.is_empty() || w.contains(char::is_whitespace) {
        return vec![w];
    }
    if let Some(set) = IRREGULAR.iter().find(|set| set.contains(&w.as_str())) {
        let mut out: Vec<String> = set.iter().map(|s| (*s).to_owned()).collect();
        // Regular forms of the base too (e.g. "reads").
        for f in inflect(set[0]) {
            if !out.contains(&f) && !matches!(set[0], "be" | "have" | "do" | "go") {
                out.push(f);
            }
        }
        return keep_real(out, set);
    }
    let mut out = Vec::new();
    // The query's own forms, then those of each base it may be an inflection of, as long as
    // regenerating the base's forms gives back the query (so "bus" does not become "bu").
    for b in bases(&w) {
        let forms = inflect(&b);
        if b == w || forms.contains(&w) {
            for f in forms {
                if !out.contains(&f) {
                    out.push(f);
                }
            }
        }
    }
    out
}

/// Irregular sets: drop regular forms that the set replaces (no "runned", "goed").
fn keep_real(forms: Vec<String>, set: &[&str]) -> Vec<String> {
    let past_irregular = set.len() > 2;
    forms
        .into_iter()
        .filter(|f| {
            set.contains(&f.as_str())
                || !(past_irregular
                    && (f.ends_with("ed") || f.ends_with("er") || f.ends_with("est")))
        })
        .collect()
}

/// American Soundex code of a word ("Robert" → R163); `None` without a letter.
pub(crate) fn soundex(word: &str) -> Option<String> {
    let code = |c: char| match c {
        'b' | 'f' | 'p' | 'v' => Some('1'),
        'c' | 'g' | 'j' | 'k' | 'q' | 's' | 'x' | 'z' => Some('2'),
        'd' | 't' => Some('3'),
        'l' => Some('4'),
        'm' | 'n' => Some('5'),
        'r' => Some('6'),
        _ => None,
    };
    let letters: Vec<char> = word
        .chars()
        .filter(char::is_ascii_alphabetic)
        .map(|c| c.to_ascii_lowercase())
        .collect();
    let first = *letters.first()?;
    let mut out = String::from(first.to_ascii_uppercase());
    let mut last = code(first);
    for &c in &letters[1..] {
        let d = code(c);
        match d {
            Some(d) if Some(d) != last => {
                out.push(d);
                if out.len() == 4 {
                    break;
                }
                last = Some(d);
            }
            Some(_) => {}
            // h and w do not separate letters with the same code; vowels do.
            None if matches!(c, 'h' | 'w') => {}
            None => last = None,
        }
    }
    while out.len() < 4 {
        out.push('0');
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    // source: the National Archives' Soundex examples (Robert, Rupert R163; Ashcraft A261;
    // Tymczak T522; Pfister P236; Honeyman H555).
    #[test]
    fn soundex_codes() {
        for (w, c) in [
            ("Robert", "R163"),
            ("Rupert", "R163"),
            ("Ashcraft", "A261"),
            ("Tymczak", "T522"),
            ("Pfister", "P236"),
            ("Honeyman", "H555"),
            ("Smith", "S530"),
            ("Smyth", "S530"),
        ] {
            assert_eq!(soundex(w).as_deref(), Some(c), "{w}");
        }
    }

    #[test]
    fn word_forms_of_regular_and_irregular_words() {
        let f = word_forms("run");
        for w in ["run", "runs", "ran", "running"] {
            assert!(f.contains(&w.to_owned()), "{w} in {f:?}");
        }
        assert!(!f.contains(&"runned".to_owned()));
        let f = word_forms("stopped");
        for w in ["stop", "stops", "stopped", "stopping"] {
            assert!(f.contains(&w.to_owned()), "{w} in {f:?}");
        }
        let f = word_forms("study");
        for w in ["studies", "studied", "studying"] {
            assert!(f.contains(&w.to_owned()), "{w} in {f:?}");
        }
        let f = word_forms("make");
        assert!(f.contains(&"made".to_owned()) && f.contains(&"making".to_owned()));
        let f = word_forms("box");
        assert!(f.contains(&"boxes".to_owned()));
    }
}
