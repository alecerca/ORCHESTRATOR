//! Git helpers — repository introspection with one deliberate mutation.
//!
//! The OpenCode server API does not expose the git branch, so Orquestador
//! derives it locally from a session's `directory`. Every operation here is
//! **read-only** except [`checkout`], the single deliberate exception: it
//! switches the repository to another local branch at the user's explicit
//! request. Nothing else may mutate a repository.

use serde::Serialize;
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

/// One commit in the repository's history (newest-first when read via
/// [`overview`]).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Commit {
    /// Full object id (`%H`).
    pub hash: String,
    /// Abbreviated object id (`%h`), for display.
    pub short: String,
    /// First line of the commit message (`%s`).
    pub subject: String,
    /// Author name (`%an`).
    pub author: String,
    /// Author date in ISO-8601 strict form (`%ad`).
    pub date: String,
}

/// One path changed by a commit, with the single status letter Git reports
/// (`A` added, `M` modified, `D` deleted, `R` renamed, `C` copied, `T` type
/// change, …). For renames and copies `path` is the destination path.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CommitFile {
    pub path: String,
    pub status: String,
}

/// One local branch of the repository, as listed by the Git panel's switcher.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Branch {
    /// Short branch name (`%(refname:short)`).
    pub name: String,
    /// Whether this is the repository's current branch.
    pub current: bool,
}

/// The current branch and its recent commits, as shown by the Git panel.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RepoLog {
    /// Current branch of the repository, or `None` when it cannot be resolved
    /// (detached HEAD resolves to `"HEAD"`; presentation maps it to a label).
    pub branch: Option<String>,
    /// Commits, newest first.
    pub commits: Vec<Commit>,
}

/// The current branch and up to `limit` most recent commits of the repository
/// containing `dir`. Read-only (`git log`).
pub fn overview(dir: &Path, limit: usize) -> Result<RepoLog, String> {
    // Field separator (`\x1f`) between the five fields, record separator
    // (`\x1e`) after each commit so multi-line subjects can never confuse the
    // parse — each commit is one record.
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .arg("log")
        .arg(format!("--max-count={limit}"))
        .arg("--date=iso-strict")
        .arg("--pretty=format:%H%x1f%h%x1f%an%x1f%ad%x1f%s%x1e")
        .output()
        .map_err(|err| format!("failed to run git: {err}"))?;

    if !output.status.success() {
        return Err(friendly_git_error(&String::from_utf8_lossy(&output.stderr)));
    }

    Ok(RepoLog {
        branch: branch_for_dir(dir),
        commits: parse_log(&String::from_utf8_lossy(&output.stdout)),
    })
}

/// The files changed by commit `hash`, each with its status letter. Read-only
/// (`git show --name-status`). `--first-parent` makes merge commits list their
/// own diff rather than printing nothing; `core.quotepath=false` keeps non-ASCII
/// paths literal instead of `\`-escaped.
pub fn commit_files(dir: &Path, hash: &str) -> Result<Vec<CommitFile>, String> {
    let output = Command::new("git")
        .arg("-c")
        .arg("core.quotepath=false")
        .arg("-C")
        .arg(dir)
        .arg("show")
        .arg("--name-status")
        .arg("--format=")
        .arg("--find-renames")
        .arg("--first-parent")
        .arg(hash)
        .output()
        .map_err(|err| format!("failed to run git: {err}"))?;

    if !output.status.success() {
        return Err(friendly_git_error(&String::from_utf8_lossy(&output.stderr)));
    }

    let mut files = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if line.trim().is_empty() {
            continue;
        }
        // `<status>\t<path>` or, for renames/copies, `<status>\t<old>\t<new>`;
        // the destination path is always the last field.
        let mut fields = line.split('\t');
        let Some(status) = fields.next().and_then(|field| field.chars().next()) else {
            continue;
        };
        let Some(path) = fields.last().filter(|path| !path.is_empty()) else {
            continue;
        };
        files.push(CommitFile {
            path: path.to_string(),
            status: status.to_string(),
        });
    }
    Ok(files)
}

/// Every local branch of the repository containing `dir`, with the current
/// branch marked and returned first; the remaining branches are sorted by name.
/// Read-only (`git for-each-ref`). An empty repository yields an empty list.
pub fn branches(dir: &Path) -> Result<Vec<Branch>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["for-each-ref", "--format=%(refname:short)", "refs/heads"])
        .output()
        .map_err(|err| format!("failed to run git: {err}"))?;

    if !output.status.success() {
        return Err(friendly_git_error(&String::from_utf8_lossy(&output.stderr)));
    }

    let current = branch_for_dir(dir);
    let mut branches: Vec<Branch> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|name| Branch {
            name: name.to_string(),
            current: current.as_deref() == Some(name),
        })
        .collect();
    // Current branch first (a `true` sorts before `false`), then by name.
    branches.sort_by(|a, b| b.current.cmp(&a.current).then_with(|| a.name.cmp(&b.name)));
    Ok(branches)
}

/// Switch the repository containing `dir` to local branch `name`.
///
/// This is the module's **only mutating** git operation: a plain
/// `git -C <dir> checkout <name>` with no force, discard-changes, `-B` or stash
/// flags, so a dirty worktree that conflicts fails instead of losing work.
/// `name` is validated before git is spawned and never becomes part of a shell
/// string.
pub fn checkout(dir: &Path, name: &str) -> Result<(), String> {
    if is_invalid_branch_name(name) {
        return Err("invalid branch name".to_string());
    }

    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["checkout", name])
        .output()
        .map_err(|err| format!("failed to run git: {err}"))?;

    if !output.status.success() {
        return Err(friendly_git_error(&String::from_utf8_lossy(&output.stderr)));
    }

    Ok(())
}

/// Whether `name` is unusable as a single checkout argument: empty or
/// whitespace-only, starting with `-` (which git would read as a flag), or
/// containing any whitespace or control character.
fn is_invalid_branch_name(name: &str) -> bool {
    name.trim().is_empty()
        || name.starts_with('-')
        || name.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// Parse the `--pretty=format:` output of [`overview`]: records split on
/// `\x1e`, fields on `\x1f`, in `%H %h %an %ad %s` order. Empty records (the
/// trailing separator and git's inter-record newline) are skipped.
fn parse_log(stdout: &str) -> Vec<Commit> {
    let mut commits = Vec::new();
    for record in stdout.split('\u{1e}') {
        let record = record.trim_matches(['\n', '\r']);
        if record.is_empty() {
            continue;
        }
        let mut fields = record.split('\u{1f}');
        let hash = fields.next().unwrap_or_default();
        let short = fields.next().unwrap_or_default();
        let author = fields.next().unwrap_or_default();
        let date = fields.next().unwrap_or_default();
        let subject = fields.next().unwrap_or_default();
        if hash.is_empty() {
            continue;
        }
        commits.push(Commit {
            hash: hash.to_string(),
            short: short.to_string(),
            subject: subject.to_string(),
            author: author.to_string(),
            date: date.to_string(),
        });
    }
    commits
}

/// Turn git's stderr into a short, human-friendly reason, falling back to the
/// raw message when nothing more specific is recognized.
fn friendly_git_error(stderr: &str) -> String {
    let lower = stderr.to_ascii_lowercase();
    if lower.contains("not a git repository") {
        "not a git repository".to_string()
    } else if lower.contains("does not have any commits") || lower.contains("unknown revision") {
        "no commits yet".to_string()
    } else if lower.contains("would be overwritten by checkout") {
        "local changes would be overwritten by checkout — commit or stash them first".to_string()
    } else {
        let trimmed = stderr.trim();
        if trimmed.is_empty() {
            "git command failed".to_string()
        } else {
            trimmed.to_string()
        }
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

    #[test]
    fn overview_lists_commits_newest_first_with_fields() {
        let dir = TempDir::new("overview");
        git(dir.path(), &["init", "-b", "main"]);
        std::fs::write(dir.path().join("a.txt"), "one\n").expect("write");
        git(dir.path(), &["add", "."]);
        git(dir.path(), &["commit", "-m", "first commit"]);
        std::fs::write(dir.path().join("b.txt"), "two\n").expect("write");
        git(dir.path(), &["add", "."]);
        git(dir.path(), &["commit", "-m", "second commit"]);

        let log = overview(dir.path(), 10).expect("overview succeeds");
        assert_eq!(log.branch, Some("main".to_string()));
        assert_eq!(log.commits.len(), 2);
        assert_eq!(log.commits[0].subject, "second commit");
        assert_eq!(log.commits[1].subject, "first commit");
        assert_eq!(log.commits[0].author, "Orquestador Test");
        assert!(log.commits[0].hash.starts_with(&log.commits[0].short));
        assert!(log.commits[0].short.len() >= 7);
        assert!(!log.commits[0].date.is_empty());
    }

    #[test]
    fn commit_files_reports_added_and_modified() {
        let dir = TempDir::new("commit-files");
        git(dir.path(), &["init", "-b", "main"]);
        std::fs::write(dir.path().join("a.txt"), "one\n").expect("write");
        std::fs::write(dir.path().join("b.txt"), "two\n").expect("write");
        git(dir.path(), &["add", "."]);
        git(dir.path(), &["commit", "-m", "base"]);
        std::fs::write(dir.path().join("a.txt"), "one changed\n").expect("write");
        std::fs::write(dir.path().join("c.txt"), "new\n").expect("write");
        git(dir.path(), &["add", "."]);
        git(dir.path(), &["commit", "-m", "change"]);

        let head = overview(dir.path(), 1).expect("overview").commits[0]
            .hash
            .clone();
        let files = commit_files(dir.path(), &head).expect("commit_files succeeds");

        let status = |path: &str| {
            files
                .iter()
                .find(|file| file.path == path)
                .map(|file| file.status.clone())
        };
        assert_eq!(status("a.txt"), Some("M".to_string()));
        assert_eq!(status("c.txt"), Some("A".to_string()));
    }

    #[test]
    fn commit_files_reports_rename_with_new_path() {
        let dir = TempDir::new("rename");
        git(dir.path(), &["init", "-b", "main"]);
        std::fs::write(dir.path().join("old.txt"), "content\n").expect("write");
        git(dir.path(), &["add", "."]);
        git(dir.path(), &["commit", "-m", "base"]);
        git(dir.path(), &["mv", "old.txt", "new.txt"]);
        git(dir.path(), &["commit", "-m", "rename"]);

        let head = overview(dir.path(), 1).expect("overview").commits[0]
            .hash
            .clone();
        let files = commit_files(dir.path(), &head).expect("commit_files succeeds");

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].status, "R");
        assert_eq!(files[0].path, "new.txt");
    }

    #[test]
    fn overview_errors_outside_a_repo() {
        let dir = TempDir::new("overview-not-a-repo");
        assert!(overview(dir.path(), 10).is_err());
    }

    #[test]
    fn overview_errors_on_empty_repo() {
        let dir = TempDir::new("overview-empty");
        git(dir.path(), &["init", "-b", "main"]);
        assert!(overview(dir.path(), 10).is_err());
    }

    #[test]
    fn branches_lists_local_branches_current_first() {
        let dir = TempDir::new("branches");
        git(dir.path(), &["init", "-b", "main"]);
        git(dir.path(), &["commit", "--allow-empty", "-m", "initial"]);
        git(dir.path(), &["branch", "feature"]);
        git(dir.path(), &["branch", "alpha"]);

        let branches = branches(dir.path()).expect("branches succeeds");

        let current: Vec<_> = branches.iter().filter(|b| b.current).collect();
        assert_eq!(current.len(), 1);
        assert_eq!(current[0].name, "main");
        // The current branch is first; the rest are sorted by name.
        assert_eq!(branches[0].name, "main");
        let rest: Vec<&str> = branches[1..].iter().map(|b| b.name.as_str()).collect();
        assert_eq!(rest, vec!["alpha", "feature"]);
    }

    #[test]
    fn branches_errors_outside_a_repo() {
        let dir = TempDir::new("branches-not-a-repo");
        assert!(branches(dir.path()).is_err());
    }

    #[test]
    fn checkout_switches_to_an_existing_branch() {
        let dir = TempDir::new("checkout");
        git(dir.path(), &["init", "-b", "main"]);
        git(dir.path(), &["commit", "--allow-empty", "-m", "initial"]);
        git(dir.path(), &["branch", "feature"]);

        checkout(dir.path(), "feature").expect("checkout succeeds");

        assert_eq!(branch_for_dir(dir.path()), Some("feature".to_string()));
    }

    #[test]
    fn checkout_rejects_invalid_names() {
        let dir = TempDir::new("checkout-invalid");
        git(dir.path(), &["init", "-b", "main"]);
        git(dir.path(), &["commit", "--allow-empty", "-m", "initial"]);

        for name in ["", "   ", "-x", "a b", "a\tb"] {
            assert_eq!(
                checkout(dir.path(), name),
                Err("invalid branch name".to_string()),
                "name {name:?} must be rejected before spawning git",
            );
        }
        // A rejected call must never move HEAD (git was not spawned).
        assert_eq!(branch_for_dir(dir.path()), Some("main".to_string()));
    }

    #[test]
    fn checkout_keeps_branch_on_dirty_worktree_conflict() {
        let dir = TempDir::new("checkout-dirty");
        git(dir.path(), &["init", "-b", "main"]);
        std::fs::write(dir.path().join("file.txt"), "base\n").expect("write");
        git(dir.path(), &["add", "."]);
        git(dir.path(), &["commit", "-m", "base"]);
        git(dir.path(), &["branch", "feature"]);
        // A committed change on `feature` that conflicts with an uncommitted
        // change on `main`.
        git(dir.path(), &["checkout", "feature"]);
        std::fs::write(dir.path().join("file.txt"), "feature\n").expect("write");
        git(dir.path(), &["commit", "-am", "feature change"]);
        git(dir.path(), &["checkout", "main"]);
        std::fs::write(dir.path().join("file.txt"), "local change\n").expect("write");

        let result = checkout(dir.path(), "feature");

        assert!(result.is_err(), "a dirty checkout must fail");
        // The rejected checkout left the working branch untouched.
        assert_eq!(branch_for_dir(dir.path()), Some("main".to_string()));
    }

    #[test]
    fn friendly_error_explains_an_overwritten_checkout() {
        // The mapping is matched on git's (English) stderr; assert the case
        // directly so the test does not depend on the host's git locale.
        let message = friendly_git_error(
            "error: Your local changes to the following files would be overwritten by checkout:\n\tfile.txt\n",
        );
        assert!(message.contains("commit or stash"), "unexpected: {message}");
    }
}
