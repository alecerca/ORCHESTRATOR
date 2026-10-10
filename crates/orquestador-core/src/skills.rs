//! Skills — app-owned markdown instruction files.
//!
//! A *skill* is a markdown file the user authors in the app or imports from
//! disk. The ones the user has switched on are folded into the **system prompt**
//! of every outbound prompt (see the Tauri shell's `send_message*` commands), so
//! the agent can apply whichever is relevant without the user restating them.
//!
//! This module is intentionally **pure** (no IO): it slugs names and renders the
//! injection block. Reading and writing the actual files lives in the shell so
//! this crate stays headless and unit-testable.

use serde::{Deserialize, Serialize};

/// Metadata for one stored skill. `name` is the slug and the file stem; `path`
/// is the absolute path of the backing `.md` file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillInfo {
    pub name: String,
    pub path: String,
}

/// Normalize a user-provided skill name into a filesystem-safe slug.
///
/// Lowercases ASCII input and collapses every run of characters that are not
/// `[a-z0-9]` into a single `-`, trimming leading/trailing separators. Returns an
/// empty string when nothing usable remains (e.g. an all-punctuation name), which
/// callers must treat as "invalid name".
pub fn slugify(name: &str) -> String {
    let mut out = String::new();
    let mut pending_sep = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            // Emit a single '-' for the run of separators before this character,
            // but never as a leading separator.
            if pending_sep && !out.is_empty() {
                out.push('-');
            }
            pending_sep = false;
            out.push(ch.to_ascii_lowercase());
        } else {
            pending_sep = true;
        }
    }
    // A trailing run of separators only set `pending_sep`; it was never emitted.
    out
}

/// Render the hidden system-instruction block for the given skills.
///
/// `skills` is an ordered list of `(name, content)` pairs. Skills whose content
/// is empty or whitespace-only are skipped, so a blank skill never injects an
/// empty section. Returns an empty string when no skill contributes content —
/// callers should then leave `PromptBody::system` unset.
pub fn build_preamble(skills: &[(String, String)]) -> String {
    let blocks: Vec<String> = skills
        .iter()
        .filter_map(|(name, content)| {
            let content = content.trim();
            if content.is_empty() {
                None
            } else {
                Some(format!("### Skill: {name}\n\n{content}"))
            }
        })
        .collect();

    if blocks.is_empty() {
        return String::new();
    }

    format!(
        "<skills>\nThe following skills are available. Apply whichever are relevant to the user's request.\n\n{}\n</skills>",
        blocks.join("\n\n")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_normalizes_to_kebab_case() {
        assert_eq!(slugify("Git Release!"), "git-release");
        assert_eq!(slugify("  My   Cool  Skill  "), "my-cool-skill");
        assert_eq!(slugify("release/notes.md"), "release-notes-md");
        assert_eq!(slugify("--already--slugged--"), "already-slugged");
    }

    #[test]
    fn slugify_rejects_names_with_no_usable_characters() {
        assert_eq!(slugify(""), "");
        assert_eq!(slugify("   "), "");
        assert_eq!(slugify("¡¿!?"), "");
        assert_eq!(slugify("ñ"), "");
    }

    #[test]
    fn preamble_skips_empty_skills_and_preserves_order() {
        let skills = vec![
            ("first".to_string(), "Do the first thing.".to_string()),
            ("blank".to_string(), "   \n\t ".to_string()),
            ("second".to_string(), "Do the second thing.".to_string()),
        ];
        let preamble = build_preamble(&skills);
        assert!(preamble.starts_with("<skills>"));
        assert!(preamble.trim_end().ends_with("</skills>"));
        let first = preamble.find("### Skill: first").unwrap();
        let second = preamble.find("### Skill: second").unwrap();
        assert!(first < second, "skills must keep their given order");
        assert!(!preamble.contains("### Skill: blank"), "empty skills must be skipped");
    }

    #[test]
    fn preamble_is_empty_without_content() {
        assert_eq!(build_preamble(&[]), "");
        assert_eq!(
            build_preamble(&[("only".to_string(), "  ".to_string())]),
            ""
        );
    }
}
