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

/// Metadata for one available skill.
///
/// `name` is the identifier used for selection (the slug/file stem for
/// app-owned skills, the directory name for native ones) and `path` is the
/// absolute path of the backing markdown. `source` records where the skill came
/// from — `"app"` for Orquestador-owned files, otherwise the active backend id
/// (e.g. `"opencode"`) — and `editable` is true only for app-owned skills,
/// because the app must never mutate another tool's configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillInfo {
    pub name: String,
    pub path: String,
    pub source: String,
    pub editable: bool,
}

/// Merge app-owned and native skills into the single list the panel shows.
///
/// `app` entries come first, then `native`; duplicates are removed by `name`
/// keeping the first occurrence, so an app skill shadows a same-named native
/// one and an earlier native root shadows a later one. The result is sorted by
/// name for a stable display order.
pub fn merge_skills(app: Vec<SkillInfo>, native: Vec<SkillInfo>) -> Vec<SkillInfo> {
    let mut seen = std::collections::HashSet::new();
    let mut merged: Vec<SkillInfo> = Vec::with_capacity(app.len() + native.len());
    for skill in app.into_iter().chain(native) {
        if seen.insert(skill.name.clone()) {
            merged.push(skill);
        }
    }
    merged.sort_by(|a, b| a.name.cmp(&b.name));
    merged
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

    /// Build a `SkillInfo` with minimal ceremony for the merge tests.
    fn skill(name: &str, source: &str, editable: bool) -> SkillInfo {
        SkillInfo {
            name: name.to_string(),
            path: format!("/{name}.md"),
            source: source.to_string(),
            editable,
        }
    }

    #[test]
    fn merge_skills_keeps_app_entry_when_names_collide() {
        let app = vec![skill("shared", "app", true), skill("only-app", "app", true)];
        let native = vec![
            skill("shared", "opencode", false),
            skill("only-native", "opencode", false),
        ];
        let merged = merge_skills(app, native);

        assert_eq!(
            merged.len(),
            3,
            "the colliding native entry must be dropped"
        );
        let shared = merged.iter().find(|s| s.name == "shared").unwrap();
        assert_eq!(
            shared.source, "app",
            "app must shadow a same-named native skill"
        );
        assert!(shared.editable);
    }

    #[test]
    fn merge_skills_preserves_distinct_names_from_both_sides() {
        let app = vec![skill("alpha", "app", true)];
        let native = vec![skill("beta", "opencode", false)];
        let merged = merge_skills(app, native);

        assert_eq!(merged.len(), 2);
        assert!(merged.iter().any(|s| s.name == "alpha"));
        assert!(merged.iter().any(|s| s.name == "beta"));
    }

    #[test]
    fn merge_skills_sorts_output_by_name() {
        let app = vec![skill("zeta", "app", true)];
        let native = vec![
            skill("alpha", "opencode", false),
            skill("mid", "opencode", false),
        ];
        let merged = merge_skills(app, native);

        let names: Vec<&str> = merged.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "mid", "zeta"]);
    }

    #[test]
    fn merge_skills_preserves_source_and_editable() {
        let native = vec![skill("beta", "opencode", false)];
        let merged = merge_skills(Vec::new(), native);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].source, "opencode");
        assert!(!merged[0].editable);
    }

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
        assert!(
            !preamble.contains("### Skill: blank"),
            "empty skills must be skipped"
        );
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
