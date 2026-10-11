//! Git helpers — read-only repository introspection.
//!
//! The OpenCode server API does not expose the git branch, so Orquestador
//! derives it locally from a session's `directory`. This module is deliberately
//! limited to **read-only** git commands: the UI is informational only and must
//! never mutate a repository.

use std::path::Path;
use std::process::Command;

/// The current branch name of the repository containing `dir`, if any.
///
/// Runs `git -C <dir> rev-parse --abbrev-ref HEAD` and returns the trimmed
/// result. Returns `None` when the command fails, `dir` is not inside a git
/// repository, or the output is empty. A detached HEAD resolves to `"HEAD"` and
/// is returned as-is; presentation maps it to a human label.
pub fn branch_for_dir(dir: &Path) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if branch.is_empty() {
        None
    } else {
        Some(branch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Monotonic suffix so parallel tests never share a temp directory.
    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    /// A unique temp directory, removed on drop (no `tempfile` dependency).
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!(
                "orquestador-git-{}-{label}-{unique}",
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

    /// Run a git command inside `dir`, asserting it succeeds. `-c` identity is
    /// injected for the commit path so the tests never depend on host config.
    fn git(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.name=Orquestador Test"])
            .args(["-c", "user.email=test@orquestador.invalid"])
            .args(args)
            .status()
            .expect("failed to spawn git");
        assert!(status.success(), "git {args:?} failed in {dir:?}");
    }

    #[test]
    fn returns_named_branch_for_a_repo() {
        let dir = TempDir::new("named");
        git(dir.path(), &["init", "-b", "main"]);
        // A branch name is only resolvable once HEAD is born: on an unborn HEAD
        // `rev-parse --abbrev-ref HEAD` exits 128, so make one empty commit.
        git(dir.path(), &["commit", "--allow-empty", "-m", "initial"]);

        assert_eq!(branch_for_dir(dir.path()), Some("main".to_string()));
    }

    #[test]
    fn returns_none_outside_a_repo() {
        let dir = TempDir::new("not-a-repo");

        assert_eq!(branch_for_dir(dir.path()), None);
    }

    #[test]
    fn returns_head_on_detached_head() {
        let dir = TempDir::new("detached");
        git(dir.path(), &["init", "-b", "main"]);
        std::fs::write(dir.path().join("file.txt"), "hello\n").expect("write file");
        git(dir.path(), &["add", "."]);
        git(dir.path(), &["commit", "-m", "initial"]);
        git(dir.path(), &["checkout", "--detach"]);

        assert_eq!(branch_for_dir(dir.path()), Some("HEAD".to_string()));
    }
}
