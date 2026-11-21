//! BlueOffice's own short list of English words the bundled SCOWL dictionary lacks but that the
//! desktop word processor accepts (BlueDoc round 9, measured on the reference with COM
//! `SpellingErrors` on a probe text): key names (Ctrl, Alt, Esc, Ins), file types written in
//! small letters (pdf, docx, xlsx, pptx), a few product spellings. Probe words the reference
//! flagged stay flagged here: `PgUp`, `PgDn`, `Fn`, `Cmd`, `ok`, plurals such as `Ctrls`.

/// Words accepted in small letters, with a capital first letter or in capitals.
const PLAIN: &[&str] = &[
    "ctrl", "alt", "esc", "ins", "pdf", "docx", "xlsx", "pptx", "dotx", "xlsm", "docm", "csv",
    "bluestar",
];

/// Words accepted only as written (mixed case).
const EXACT: &[&str] = &["iPhone", "iPad", "iOS", "macOS", "Wi-Fi", "Wi", "Fi"];

/// `w` is one of the extra words in an accepted case.
pub(crate) fn accepted(w: &str) -> bool {
    if EXACT.contains(&w) {
        return true;
    }
    let lower = w.to_lowercase();
    if !PLAIN.contains(&lower.as_str()) {
        return false;
    }
    let mut cs = w.chars();
    let first_upper = cs.next().is_some_and(char::is_uppercase);
    let rest_lower = cs.clone().all(|c| !c.is_uppercase());
    let rest_upper = cs.all(|c| !c.is_lowercase());
    w == lower || (first_upper && (rest_lower || rest_upper))
}

#[cfg(test)]
mod tests {
    use super::accepted;

    // source: COM SpellingErrors of the reference on "Press Ctrl and Alt and Shift with Esc Del
    // Tab Enter PgUp PgDn Ins Fn Cmd Win key. … ctrl alt esc xlsx pdf Ctrls" (round 9).
    #[test]
    fn key_names_and_file_types_in_the_cases_the_reference_accepts() {
        for w in [
            "Ctrl", "ctrl", "CTRL", "Alt", "alt", "Esc", "esc", "Ins", "pdf", "xlsx", "iPhone",
        ] {
            assert!(accepted(w), "{w}");
        }
        for w in ["Ctrls", "cTRL", "PgUp", "Fn", "Cmd", "iphone"] {
            assert!(!accepted(w), "{w}");
        }
    }
}
