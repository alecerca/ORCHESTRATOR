//! Working-tree file writes for the diff/edit panel.
//!
//! The only mutation this crate performs on the user's repository is writing a
//! text document back to its working-tree path. [`write_text_file`] takes a
//! repository-relative path and refuses anything that could escape `dir`
//! (empty, absolute, or containing `..`), mirroring the guard the shell applies
//! in `git_show_file` before it hands a path to git.

use std::path::Path;

/// Validate a repository-relative path, returning a readable error string.
///
/// Rejects an empty (or whitespace-only) path, an absolute path (leading `/`),
/// and any path containing `..` — the same rules `git_show_file` enforces, so a
/// write can never address a file outside `dir`.
fn validate_relative(rel_path: &str) -> Result<(), String> {
    if rel_path.trim().is_empty() {
        return Err("invalid file path: empty".to_string());
    }
    if rel_path.starts_with('/') {
        return Err("invalid file path: must be repository-relative".to_string());
    }
    if rel_path.contains("..") {
        return Err("invalid file path: must not contain '..'".to_string());
    }
    Ok(())
}

/// Write `content` to `dir/rel_path`, replacing any existing file.
///
/// The path is validated first (see [`validate_relative`]); the write truncates
/// and overwrites, so callers must only pass a path the user has opened.
pub fn write_text_file(dir: &Path, rel_path: &str, content: &str) -> Result<(), String> {
    validate_relative(rel_path)?;
    std::fs::write(dir.join(rel_path), content.as_bytes())
        .map_err(|err| format!("failed to write {rel_path}: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Monotonic suffix so parallel tests never share a temp directory.
    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    /// A unique temp directory, removed on drop (no `tempfile` dependency),
    /// mirroring the helper in `git.rs`.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!(
                "orquestador-files-{}-{label}-{unique}",
                std::process::id()
            ));
            std::fs::create_dir_all(&dir).expect("create temp dir");
            Self(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn writes_a_new_file() {
        let dir = TempDir::new("write-new");
        write_text_file(dir.path(), "note.txt", "hello").expect("write");
        let written = std::fs::read_to_string(dir.path().join("note.txt")).expect("read back");
        assert_eq!(written, "hello");
    }

    #[test]
    fn overwrites_an_existing_file() {
        let dir = TempDir::new("write-overwrite");
        std::fs::write(dir.path().join("note.txt"), "old").expect("seed");
        write_text_file(dir.path(), "note.txt", "new").expect("write");
        let written = std::fs::read_to_string(dir.path().join("note.txt")).expect("read back");
        assert_eq!(written, "new");
    }

    #[test]
    fn writes_a_nested_path() {
        // The parent directory must already exist (a real tab only opens paths
        // the explorer listed), so create it first.
        let dir = TempDir::new("write-nested");
        std::fs::create_dir_all(dir.path().join("sub")).expect("mkdir");
        write_text_file(dir.path(), "sub/note.txt", "nested").expect("write");
        let written = std::fs::read_to_string(dir.path().join("sub/note.txt")).expect("read back");
        assert_eq!(written, "nested");
    }

    #[test]
    fn rejects_a_parent_traversal_path() {
        let dir = TempDir::new("write-dotdot");
        let err = write_text_file(dir.path(), "../escape.txt", "x").expect_err("reject");
        assert!(err.contains(".."), "unexpected error: {err}");
    }

    #[test]
    fn rejects_an_absolute_path() {
        let dir = TempDir::new("write-absolute");
        let err = write_text_file(dir.path(), "/etc/passwd", "x").expect_err("reject");
        assert!(
            err.contains("repository-relative"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn rejects_an_empty_path() {
        let dir = TempDir::new("write-empty");
        let err = write_text_file(dir.path(), "   ", "x").expect_err("reject");
        assert!(err.contains("empty"), "unexpected error: {err}");
    }
}
