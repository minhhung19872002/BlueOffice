//! The JSON bridges try `SecurityCommand` before each app's own command enum, so a `cmd` name
//! used by both would silently go to the wrong handler (this happened with `protectDocument`).
//! The variant names are read from the sources and compared after serde's camelCase renaming.

use std::collections::BTreeSet;

/// Variant names (camelCase, as `cmd` values) of `pub enum <name>` in `src`.
fn variants(src: &str, name: &str) -> BTreeSet<String> {
    let head = format!("pub enum {name} {{");
    let start = src
        .find(&head)
        .unwrap_or_else(|| panic!("enum {name} not found"))
        + head.len();
    let mut depth = 1usize;
    let mut out = BTreeSet::new();
    for line in src[start..].lines() {
        if depth == 1 {
            // A variant: four spaces, an upper-case identifier, then `{`, `,` or `(`.
            if let Some(rest) = line.strip_prefix("    ") {
                let ident: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                let after = rest[ident.len()..].trim_start();
                if ident.chars().next().is_some_and(|c| c.is_ascii_uppercase())
                    && (after.starts_with('{') || after.starts_with(',') || after.starts_with('('))
                {
                    let mut chars = ident.chars();
                    let first = chars.next().map(|c| c.to_ascii_lowercase());
                    out.insert(first.into_iter().chain(chars).collect());
                }
            }
        }
        for c in line.chars() {
            match c {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
        }
        if depth == 0 {
            break;
        }
    }
    assert!(!out.is_empty(), "no variants read for {name}");
    out
}

fn overlap(a: &BTreeSet<String>, b: &BTreeSet<String>) -> Vec<String> {
    a.intersection(b).cloned().collect()
}

#[test]
fn security_commands_do_not_shadow_app_commands() {
    let security = variants(include_str!("../src/security.rs"), "SecurityCommand");
    let doc = variants(include_str!("../src/edit.rs"), "Command");
    let sheet = variants(include_str!("../src/sheet/cmd.rs"), "SheetCommand");
    let slide = variants(include_str!("../src/slide_edit/mod.rs"), "SlideCommand");
    // Sanity: the reader sees well-known commands of each enum.
    assert!(doc.contains("insertText") && doc.contains("protectDocument"));
    assert!(sheet.contains("paste") && slide.contains("paste"));
    assert!(security.contains("restrictEditing"));
    assert_eq!(overlap(&security, &doc), Vec::<String>::new(), "BlueDoc");
    assert_eq!(
        overlap(&security, &sheet),
        Vec::<String>::new(),
        "BlueSheet"
    );
    assert_eq!(
        overlap(&security, &slide),
        Vec::<String>::new(),
        "BlueSlide"
    );
}

// CORE-SPELL-006: the spelling commands are parsed before the app commands as well.
#[test]
fn spelling_commands_do_not_shadow_app_commands() {
    let sheet = variants(include_str!("../src/sheet/cmd.rs"), "SheetCommand");
    let slide = variants(include_str!("../src/slide_edit/mod.rs"), "SlideCommand");
    let sheet_spell = variants(include_str!("../src/sheet/spell_cmd.rs"), "SpellCommand");
    let slide_spell = variants(
        include_str!("../src/slide_edit/spell.rs"),
        "SlideSpellCommand",
    );
    assert!(sheet_spell.contains("spellCheck") && slide_spell.contains("slideSpellCheck"));
    assert_eq!(
        overlap(&sheet_spell, &sheet),
        Vec::<String>::new(),
        "BlueSheet"
    );
    assert_eq!(
        overlap(&slide_spell, &slide),
        Vec::<String>::new(),
        "BlueSlide"
    );
}
