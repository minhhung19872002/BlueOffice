//! Character references (HTML Living Standard §13.5 "Named character references"
//! — the commonly used subset — and numeric references, §13.2.5.80).

/// Named references understood by the importer.
const NAMED: &[(&str, &str)] = &[
    ("amp", "&"),
    ("lt", "<"),
    ("gt", ">"),
    ("quot", "\""),
    ("apos", "'"),
    ("nbsp", "\u{a0}"),
    ("iexcl", "¡"),
    ("cent", "¢"),
    ("pound", "£"),
    ("curren", "¤"),
    ("yen", "¥"),
    ("brvbar", "¦"),
    ("sect", "§"),
    ("uml", "¨"),
    ("copy", "©"),
    ("ordf", "ª"),
    ("laquo", "«"),
    ("not", "¬"),
    ("shy", "\u{ad}"),
    ("reg", "®"),
    ("macr", "¯"),
    ("deg", "°"),
    ("plusmn", "±"),
    ("sup2", "²"),
    ("sup3", "³"),
    ("acute", "´"),
    ("micro", "µ"),
    ("para", "¶"),
    ("middot", "·"),
    ("cedil", "¸"),
    ("sup1", "¹"),
    ("ordm", "º"),
    ("raquo", "»"),
    ("frac14", "¼"),
    ("frac12", "½"),
    ("frac34", "¾"),
    ("iquest", "¿"),
    ("Agrave", "À"),
    ("Aacute", "Á"),
    ("Acirc", "Â"),
    ("Atilde", "Ã"),
    ("Auml", "Ä"),
    ("Aring", "Å"),
    ("AElig", "Æ"),
    ("Ccedil", "Ç"),
    ("Egrave", "È"),
    ("Eacute", "É"),
    ("Ecirc", "Ê"),
    ("Euml", "Ë"),
    ("Igrave", "Ì"),
    ("Iacute", "Í"),
    ("Icirc", "Î"),
    ("Iuml", "Ï"),
    ("ETH", "Ð"),
    ("Ntilde", "Ñ"),
    ("Ograve", "Ò"),
    ("Oacute", "Ó"),
    ("Ocirc", "Ô"),
    ("Otilde", "Õ"),
    ("Ouml", "Ö"),
    ("times", "×"),
    ("Oslash", "Ø"),
    ("Ugrave", "Ù"),
    ("Uacute", "Ú"),
    ("Ucirc", "Û"),
    ("Uuml", "Ü"),
    ("Yacute", "Ý"),
    ("THORN", "Þ"),
    ("szlig", "ß"),
    ("agrave", "à"),
    ("aacute", "á"),
    ("acirc", "â"),
    ("atilde", "ã"),
    ("auml", "ä"),
    ("aring", "å"),
    ("aelig", "æ"),
    ("ccedil", "ç"),
    ("egrave", "è"),
    ("eacute", "é"),
    ("ecirc", "ê"),
    ("euml", "ë"),
    ("igrave", "ì"),
    ("iacute", "í"),
    ("icirc", "î"),
    ("iuml", "ï"),
    ("eth", "ð"),
    ("ntilde", "ñ"),
    ("ograve", "ò"),
    ("oacute", "ó"),
    ("ocirc", "ô"),
    ("otilde", "õ"),
    ("ouml", "ö"),
    ("divide", "÷"),
    ("oslash", "ø"),
    ("ugrave", "ù"),
    ("uacute", "ú"),
    ("ucirc", "û"),
    ("uuml", "ü"),
    ("yacute", "ý"),
    ("thorn", "þ"),
    ("yuml", "ÿ"),
    ("OElig", "Œ"),
    ("oelig", "œ"),
    ("Scaron", "Š"),
    ("scaron", "š"),
    ("Yuml", "Ÿ"),
    ("fnof", "ƒ"),
    ("circ", "ˆ"),
    ("tilde", "˜"),
    ("Alpha", "Α"),
    ("Beta", "Β"),
    ("Gamma", "Γ"),
    ("Delta", "Δ"),
    ("Theta", "Θ"),
    ("Lambda", "Λ"),
    ("Pi", "Π"),
    ("Sigma", "Σ"),
    ("Phi", "Φ"),
    ("Psi", "Ψ"),
    ("Omega", "Ω"),
    ("alpha", "α"),
    ("beta", "β"),
    ("gamma", "γ"),
    ("delta", "δ"),
    ("epsilon", "ε"),
    ("zeta", "ζ"),
    ("eta", "η"),
    ("theta", "θ"),
    ("iota", "ι"),
    ("kappa", "κ"),
    ("lambda", "λ"),
    ("mu", "μ"),
    ("nu", "ν"),
    ("xi", "ξ"),
    ("pi", "π"),
    ("rho", "ρ"),
    ("sigma", "σ"),
    ("tau", "τ"),
    ("upsilon", "υ"),
    ("phi", "φ"),
    ("chi", "χ"),
    ("psi", "ψ"),
    ("omega", "ω"),
    ("ensp", "\u{2002}"),
    ("emsp", "\u{2003}"),
    ("thinsp", "\u{2009}"),
    ("zwnj", "\u{200c}"),
    ("zwj", "\u{200d}"),
    ("lrm", "\u{200e}"),
    ("rlm", "\u{200f}"),
    ("ndash", "–"),
    ("mdash", "—"),
    ("lsquo", "‘"),
    ("rsquo", "’"),
    ("sbquo", "‚"),
    ("ldquo", "“"),
    ("rdquo", "”"),
    ("bdquo", "„"),
    ("dagger", "†"),
    ("Dagger", "‡"),
    ("bull", "•"),
    ("hellip", "…"),
    ("permil", "‰"),
    ("prime", "′"),
    ("Prime", "″"),
    ("lsaquo", "‹"),
    ("rsaquo", "›"),
    ("oline", "‾"),
    ("frasl", "⁄"),
    ("euro", "€"),
    ("trade", "™"),
    ("larr", "←"),
    ("uarr", "↑"),
    ("rarr", "→"),
    ("darr", "↓"),
    ("harr", "↔"),
    ("rArr", "⇒"),
    ("hArr", "⇔"),
    ("forall", "∀"),
    ("part", "∂"),
    ("exist", "∃"),
    ("empty", "∅"),
    ("nabla", "∇"),
    ("isin", "∈"),
    ("ni", "∋"),
    ("prod", "∏"),
    ("sum", "∑"),
    ("minus", "−"),
    ("radic", "√"),
    ("infin", "∞"),
    ("cap", "∩"),
    ("cup", "∪"),
    ("int", "∫"),
    ("there4", "∴"),
    ("asymp", "≈"),
    ("ne", "≠"),
    ("equiv", "≡"),
    ("le", "≤"),
    ("ge", "≥"),
    ("sub", "⊂"),
    ("sup", "⊃"),
    ("loz", "◊"),
    ("spades", "♠"),
    ("clubs", "♣"),
    ("hearts", "♥"),
    ("diams", "♦"),
];

/// The character of a numeric reference: C1 controls are read as Windows-1252
/// (HTML §13.2.5.80), NUL, surrogates and out-of-range values become U+FFFD.
fn numeric(code: u32) -> char {
    const C1: [u16; 32] = [
        0x20AC, 0x81, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160,
        0x2039, 0x0152, 0x8D, 0x017D, 0x8F, 0x90, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013,
        0x2014, 0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0x9D, 0x017E, 0x0178,
    ];
    match code {
        0 => '\u{FFFD}',
        0x80..=0x9F => char::from_u32(u32::from(C1[(code - 0x80) as usize])).unwrap_or('\u{FFFD}'),
        c => char::from_u32(c).unwrap_or('\u{FFFD}'),
    }
}

/// Text with its character references replaced. Unknown references and bare `&`
/// stay as they are.
pub fn decode(s: &str) -> String {
    if !s.contains('&') {
        return s.to_owned();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let tail = &rest[at + 1..];
        // The reference name: up to 32 ASCII alphanumerics (or `#…`).
        let end = tail
            .bytes()
            .take(34)
            .position(|b| !(b.is_ascii_alphanumeric() || b == b'#'))
            .unwrap_or(tail.len().min(34));
        let name = &tail[..end];
        let semi = usize::from(tail.as_bytes().get(end) == Some(&b';'));
        let rep: Option<String> = if let Some(num) = name.strip_prefix('#') {
            let code = match num.strip_prefix(['x', 'X']) {
                Some(hex) if !hex.is_empty() => u32::from_str_radix(hex, 16).ok(),
                None if !num.is_empty() => num.parse::<u32>().ok(),
                _ => None,
            };
            // Overflowing digits are still a (replaced) reference.
            match code {
                Some(c) => Some(numeric(c).to_string()),
                None if num.len() > 1 && num[1..].bytes().all(|b| b.is_ascii_hexdigit()) => {
                    Some('\u{FFFD}'.to_string())
                }
                None => None,
            }
        } else {
            NAMED
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| (*v).to_owned())
        };
        if let Some(r) = rep {
            out.push_str(&r);
            rest = &tail[end + semi..];
        } else {
            out.push('&');
            rest = tail;
        }
    }
    out.push_str(rest);
    out
}

/// Escape text for HTML element content.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\u{a0}' => out.push_str("&nbsp;"),
            _ => out.push(c),
        }
    }
    out
}

/// Escape text for a double-quoted attribute value.
pub fn escape_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references() {
        assert_eq!(
            decode("a &amp; b &lt;c&gt; &nbsp;&copy;&eacute;"),
            "a & b <c> \u{a0}©é"
        );
        assert_eq!(
            decode("&#65;&#x1EA0;&#X41;&#150;&#0;&#x110000;"),
            "AẠA–\u{FFFD}\u{FFFD}"
        );
        assert_eq!(
            decode("AT&T &bogus; & &#; &#x; &amp"),
            "AT&T &bogus; & &#; &#x; &"
        );
        assert_eq!(decode("&#99999999999999999999;"), "\u{FFFD}");
        assert_eq!(decode(&escape("a<b>&\u{a0}é")), "a<b>&\u{a0}é");
        assert_eq!(escape_attr("\"a\"&<"), "&quot;a&quot;&amp;&lt;");
    }
}
