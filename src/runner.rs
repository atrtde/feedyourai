//! Orchestrates a single combine run: builds the directory tree, then either
//! writes the tree only or writes the tree plus every matching file's
//! contents. Also handles cloning a remote git repository into a temporary
//! directory before running the same combine logic against it.

use std::path::PathBuf;
use std::process::{self, Command};
use std::sync::Mutex;

use tempfile::TempDir;

use crate::config::Config;
use crate::error::{FyaiError, Result};
use crate::scanner::{ScanStats, scan};

/// Root of the temporary directory backing the current `--repo` run, if
/// any, from the moment it's created until [`run_git`] returns (spanning
/// both the clone and the subsequent scan). Tracked so a Ctrl-C handler
/// installed by the binary (via [`cleanup_active_clone`]) can remove it: a
/// signal handler drives the process to exit outside of normal control
/// flow, so the `TempDir` guard's own `Drop` impl never gets a chance to
/// run.
static ACTIVE_CLONE_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Removes the temporary clone directory tracked in [`ACTIVE_CLONE_PATH`],
/// if a clone is currently in progress. Safe to call from a Ctrl-C handler;
/// a no-op if no clone is active.
pub fn cleanup_active_clone() {
    if let Ok(guard) = ACTIVE_CLONE_PATH.lock()
        && let Some(path) = guard.as_ref()
    {
        let _ = std::fs::remove_dir_all(path);
    }
}

/// Clears [`ACTIVE_CLONE_PATH`] on drop, regardless of which return path
/// [`clone_repository`] takes.
struct ActiveClonePathGuard;

impl Drop for ActiveClonePathGuard {
    fn drop(&mut self) {
        if let Ok(mut guard) = ACTIVE_CLONE_PATH.lock() {
            *guard = None;
        }
    }
}

/// Combines files from a local directory as described by `config`.
///
/// Writes the result to `config.output` (either the directory tree only, or
/// the tree plus file contents, depending on `config.tree_only`), and
/// returns a byte breakdown of every file the walk collected.
pub fn run_local(config: Config) -> Result<ScanStats> {
    let stats = scan(&config)?;
    Ok(stats)
}

/// Clones `repo_url` into a temporary directory, then runs the same combine
/// logic as [`run_local`] against the clone.
///
/// The temporary clone is removed before this function returns, regardless
/// of the run's outcome.
///
/// # Arguments
///
/// * `branch` - an optional branch or tag to check out.
/// * `commit` - an optional commit SHA to check out after cloning. When set,
///   the clone is not shallow, since the target commit may not be reachable
///   from a depth-1 clone.
/// * `config` - the combine configuration; `config.directory` is overwritten
///   with the path to the cloned repository.
pub fn run_git(
    repo_url: &str,
    branch: Option<&str>,
    commit: Option<&str>,
    config: Config,
) -> Result<ScanStats> {
    let temp_dir = tempfile::tempdir()?;

    if let Ok(mut guard) = ACTIVE_CLONE_PATH.lock() {
        *guard = Some(temp_dir.path().to_path_buf());
    }
    let _active_path_guard = ActiveClonePathGuard;

    let clone_path = clone_repository(&temp_dir, repo_url, branch, commit)?;

    let mut config = config;
    config.directory = clone_path;

    let result = run_local(config);
    drop(temp_dir);
    result
}

/// Clones `repo_url` into `temp_dir`, optionally checking out `branch`
/// and/or `commit`.
///
/// The clone is shallow (`--depth 1`) unless `commit` is set, since the
/// target commit may not be reachable from a depth-1 history.
///
/// Returns the path to the checked-out repository, a subdirectory of
/// `temp_dir`.
fn clone_repository(
    temp_dir: &TempDir,
    repo_url: &str,
    branch: Option<&str>,
    commit: Option<&str>,
) -> Result<PathBuf> {
    let clone_path = temp_dir.path().join("repo");

    let mut cmd = Command::new("git");
    cmd.arg("clone");
    if commit.is_none() {
        cmd.arg("--depth").arg("1");
    }
    if let Some(branch) = branch {
        cmd.args(["--branch", branch]);
    }
    cmd.arg(repo_url).arg(&clone_path);

    let output = cmd
        .output()
        .map_err(|e| FyaiError::Git(format!("failed to run git clone: {e}")))?;
    if !output.status.success() {
        return Err(FyaiError::Git(format!(
            "git clone failed: {}",
            command_error_details(&output)
        )));
    }

    if let Some(commit) = commit {
        let output = Command::new("git")
            .arg("-C")
            .arg(&clone_path)
            .args(["checkout", commit])
            .output()
            .map_err(|e| FyaiError::Git(format!("failed to run git checkout: {e}")))?;
        if !output.status.success() {
            return Err(FyaiError::Git(format!(
                "git checkout failed: {}",
                command_error_details(&output)
            )));
        }
    }

    Ok(clone_path)
}

/// Extracts a human-readable error message from a failed command's output,
/// preferring stderr over stdout, and falling back to a generic message if
/// both are empty.
fn command_error_details(output: &process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let details = if !stderr.trim().is_empty() {
        stderr.trim()
    } else {
        stdout.trim()
    };

    if details.is_empty() {
        "unknown error".to_string()
    } else {
        details.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::fs;
    use std::path::Path;

    // ---- test helpers ----

    /// Builds a minimal [`Config`] pointing at `directory`/`output`, with
    /// every optional filter unset and all booleans at their usual default.
    fn test_config(directory: PathBuf, output: PathBuf) -> Config {
        Config {
            directory,
            output,
            include_dirs: None,
            exclude_dirs: None,
            include_ext: None,
            exclude_ext: None,
            include_files: None,
            exclude_files: None,
            min_size: None,
            max_size: None,
            hidden: true,
            gitignore: true,
            ignore_files: true,
            git_global: true,
            follow_links: false,
            tree_only: false,
            human: false,
        }
    }

    /// Runs `git <args>` with `-C repo_path`, asserting it succeeds, and
    /// returns its stdout as a `String` (trimmed).
    fn run_git_cmd(repo_path: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(repo_path)
            .args(args)
            .output()
            .unwrap_or_else(|e| panic!("failed to run git {args:?}: {e}"));
        assert!(
            output.status.success(),
            "git {args:?} failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    /// Initializes a fresh git repo in a new tempdir with a single commit
    /// containing `file1.txt`. Returns the `TempDir` guard (keep it alive
    /// for as long as the repo path is needed).
    fn init_git_repo() -> TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path();

        run_git_cmd(path, &["init", "-q"]);
        fs::write(path.join("file1.txt"), "hello from file1").expect("write file1");
        run_git_cmd(path, &["add", "."]);
        run_git_cmd(
            path,
            &[
                "-c",
                "user.email=test@test.com",
                "-c",
                "user.name=Test",
                "commit",
                "-q",
                "-m",
                "initial commit",
            ],
        );

        dir
    }

    fn read_output(path: &Path) -> String {
        fs::read_to_string(path).expect("read output file")
    }

    // ---- cleanup_active_clone ----
    //
    // Tagged `#[serial(active_clone)]`, shared with the `run_git_*` tests
    // below: all of them read or write the same `ACTIVE_CLONE_PATH`
    // static, so they can't run concurrently without racing each other.

    #[test]
    #[serial(active_clone)]
    fn cleanup_active_clone_removes_tracked_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        let clone_path = dir.path().join("repo");
        fs::create_dir_all(&clone_path).expect("create_dir_all");
        fs::write(clone_path.join("f.txt"), "x").expect("write f.txt");

        {
            let mut guard = ACTIVE_CLONE_PATH.lock().expect("lock");
            *guard = Some(clone_path.clone());
        }

        cleanup_active_clone();

        assert!(!clone_path.exists());
        let mut guard = ACTIVE_CLONE_PATH.lock().expect("lock");
        *guard = None;
    }

    #[test]
    #[serial(active_clone)]
    fn cleanup_active_clone_is_noop_when_nothing_active() {
        {
            let mut guard = ACTIVE_CLONE_PATH.lock().expect("lock");
            *guard = None;
        }

        cleanup_active_clone();
    }

    // ---- run_local ----

    #[test]
    fn run_local_writes_combined_output_successfully() {
        let source_dir = tempfile::tempdir().expect("tempdir");
        fs::write(source_dir.path().join("a.txt"), "content of a").expect("write a.txt");
        fs::write(source_dir.path().join("b.txt"), "content of b").expect("write b.txt");

        let out_dir = tempfile::tempdir().expect("tempdir");
        let output_path = out_dir.path().join("combined.txt");

        let config = test_config(source_dir.path().to_path_buf(), output_path.clone());

        let result = run_local(config);
        assert!(result.is_ok(), "expected Ok, got {result:?}");
        let stats = result.unwrap();
        assert_eq!(stats.total_size, 24); // "content of a" + "content of b", 12 bytes each
        assert_eq!(stats.written_size, 24);
        assert_eq!(stats.binary_size, 0);

        let contents = read_output(&output_path);
        assert!(!contents.is_empty());
        assert!(contents.contains("a.txt"));
        assert!(contents.contains("b.txt"));
        assert!(contents.contains("content of a"));
        assert!(contents.contains("content of b"));
    }

    #[test]
    fn run_local_handles_empty_directory() {
        let source_dir = tempfile::tempdir().expect("tempdir");
        let out_dir = tempfile::tempdir().expect("tempdir");
        let output_path = out_dir.path().join("combined.txt");

        let config = test_config(source_dir.path().to_path_buf(), output_path.clone());

        let result = run_local(config);
        assert!(result.is_ok(), "expected Ok, got {result:?}");

        let contents = read_output(&output_path);
        assert!(contents.contains("The directory is empty."));
    }

    #[test]
    fn run_local_propagates_io_error_from_scan() {
        let source_dir = tempfile::tempdir().expect("tempdir");
        fs::write(source_dir.path().join("a.txt"), "content").expect("write a.txt");

        // Parent directory of the output path does not exist, so
        // `File::create` inside `scan` fails.
        let out_dir = tempfile::tempdir().expect("tempdir");
        let output_path = out_dir.path().join("missing-subdir").join("combined.txt");

        let config = test_config(source_dir.path().to_path_buf(), output_path);

        let result = run_local(config);
        assert!(matches!(result, Err(FyaiError::Io(_))));
    }

    // ---- run_git: success paths ----

    #[test]
    #[serial(active_clone)]
    fn run_git_clones_and_scans_default_branch() {
        let repo_dir = init_git_repo();
        let repo_path = repo_dir.path().to_str().expect("utf8 path").to_string();

        let out_dir = tempfile::tempdir().expect("tempdir");
        let output_path = out_dir.path().join("combined.txt");
        let placeholder_dir = tempfile::tempdir().expect("tempdir");

        let config = test_config(placeholder_dir.path().to_path_buf(), output_path.clone());

        let result = run_git(&repo_path, None, None, config);
        assert!(result.is_ok(), "expected Ok, got {result:?}");

        let contents = read_output(&output_path);
        assert!(contents.contains("file1.txt"));
        assert!(contents.contains("hello from file1"));

        // The test's own tempdirs are untouched/still present; the clone's
        // temp directory (internal to `run_git`) has been cleaned up, which
        // we can't observe directly, but the call returning successfully
        // without leaking into our fixtures is the externally-visible
        // guarantee.
        assert!(out_dir.path().exists());
        assert!(repo_dir.path().exists());
    }

    #[test]
    #[serial(active_clone)]
    fn run_git_with_branch_checks_out_branch_content() {
        let repo_dir = init_git_repo();
        let repo_path = repo_dir.path();

        run_git_cmd(repo_path, &["checkout", "-q", "-b", "my-branch"]);
        fs::write(repo_path.join("branch_only.txt"), "only on my-branch")
            .expect("write branch_only.txt");
        run_git_cmd(repo_path, &["add", "."]);
        run_git_cmd(
            repo_path,
            &[
                "-c",
                "user.email=test@test.com",
                "-c",
                "user.name=Test",
                "commit",
                "-q",
                "-m",
                "branch-only commit",
            ],
        );

        let repo_url = repo_path.to_str().expect("utf8 path").to_string();
        let out_dir = tempfile::tempdir().expect("tempdir");
        let output_path = out_dir.path().join("combined.txt");
        let placeholder_dir = tempfile::tempdir().expect("tempdir");
        let config = test_config(placeholder_dir.path().to_path_buf(), output_path.clone());

        let result = run_git(&repo_url, Some("my-branch"), None, config);
        assert!(result.is_ok(), "expected Ok, got {result:?}");

        let contents = read_output(&output_path);
        assert!(contents.contains("branch_only.txt"));
        assert!(contents.contains("only on my-branch"));
        // The original file, present on every branch, should still show up.
        assert!(contents.contains("file1.txt"));
    }

    #[test]
    #[serial(active_clone)]
    fn run_git_with_commit_pins_to_specific_commit() {
        let repo_dir = init_git_repo();
        let repo_path = repo_dir.path();

        let first_sha = run_git_cmd(repo_path, &["rev-parse", "HEAD"]);

        fs::write(repo_path.join("second.txt"), "added after first commit")
            .expect("write second.txt");
        run_git_cmd(repo_path, &["add", "."]);
        run_git_cmd(
            repo_path,
            &[
                "-c",
                "user.email=test@test.com",
                "-c",
                "user.name=Test",
                "commit",
                "-q",
                "-m",
                "second commit",
            ],
        );

        let repo_url = repo_path.to_str().expect("utf8 path").to_string();
        let out_dir = tempfile::tempdir().expect("tempdir");
        let output_path = out_dir.path().join("combined.txt");
        let placeholder_dir = tempfile::tempdir().expect("tempdir");
        let config = test_config(placeholder_dir.path().to_path_buf(), output_path.clone());

        let result = run_git(&repo_url, None, Some(&first_sha), config);
        assert!(result.is_ok(), "expected Ok, got {result:?}");

        let contents = read_output(&output_path);
        assert!(contents.contains("file1.txt"));
        assert!(
            !contents.contains("second.txt"),
            "output should not contain the file added in the second commit, got:\n{contents}"
        );
    }

    // ---- run_git: failure paths ----

    #[test]
    #[serial(active_clone)]
    fn run_git_invalid_repo_url_returns_git_error() {
        let out_dir = tempfile::tempdir().expect("tempdir");
        let output_path = out_dir.path().join("combined.txt");
        let placeholder_dir = tempfile::tempdir().expect("tempdir");
        let config = test_config(placeholder_dir.path().to_path_buf(), output_path);

        let result = run_git("/nonexistent/path/that/does/not/exist", None, None, config);

        match result {
            Err(FyaiError::Git(msg)) => {
                assert!(
                    msg.contains("git clone failed"),
                    "unexpected message: {msg}"
                );
            }
            other => panic!("expected Err(FyaiError::Git(_)), got {other:?}"),
        }
    }

    #[test]
    #[serial(active_clone)]
    fn run_git_invalid_commit_returns_git_error() {
        let repo_dir = init_git_repo();
        let repo_path = repo_dir.path().to_str().expect("utf8 path").to_string();

        let out_dir = tempfile::tempdir().expect("tempdir");
        let output_path = out_dir.path().join("combined.txt");
        let placeholder_dir = tempfile::tempdir().expect("tempdir");
        let config = test_config(placeholder_dir.path().to_path_buf(), output_path);

        let bogus_sha = "0".repeat(40);
        let result = run_git(&repo_path, None, Some(&bogus_sha), config);

        match result {
            Err(FyaiError::Git(msg)) => {
                assert!(
                    msg.contains("git checkout failed"),
                    "unexpected message: {msg}"
                );
            }
            other => panic!("expected Err(FyaiError::Git(_)), got {other:?}"),
        }
    }

    #[test]
    #[serial(active_clone)]
    fn run_git_propagates_scan_io_error() {
        let repo_dir = init_git_repo();
        let repo_path = repo_dir.path().to_str().expect("utf8 path").to_string();

        // Parent directory of the output path does not exist, so the
        // `run_local` call inside `run_git` fails with an I/O error.
        let out_dir = tempfile::tempdir().expect("tempdir");
        let output_path = out_dir.path().join("missing-subdir").join("combined.txt");
        let placeholder_dir = tempfile::tempdir().expect("tempdir");
        let config = test_config(placeholder_dir.path().to_path_buf(), output_path);

        let result = run_git(&repo_path, None, None, config);
        assert!(matches!(result, Err(FyaiError::Io(_))));
    }

    // ---- command_error_details ----

    /// Returns a real `ExitStatus` representing a failed process, so tests
    /// can build a custom `Output` around it (`ExitStatus` has no public
    /// constructor).
    fn failing_status() -> process::ExitStatus {
        Command::new("false")
            .output()
            .expect("failed to run `false`")
            .status
    }

    #[test]
    fn command_error_details_prefers_stderr_over_stdout() {
        let output = process::Output {
            status: failing_status(),
            stdout: b"stdout message".to_vec(),
            stderr: b"stderr message".to_vec(),
        };
        assert_eq!(command_error_details(&output), "stderr message");
    }

    #[test]
    fn command_error_details_falls_back_to_stdout_when_stderr_empty() {
        let output = process::Output {
            status: failing_status(),
            stdout: b"stdout message".to_vec(),
            stderr: Vec::new(),
        };
        assert_eq!(command_error_details(&output), "stdout message");
    }

    #[test]
    fn command_error_details_falls_back_to_stdout_when_stderr_whitespace_only() {
        let output = process::Output {
            status: failing_status(),
            stdout: b"stdout message".to_vec(),
            stderr: b"   \n\t  ".to_vec(),
        };
        assert_eq!(command_error_details(&output), "stdout message");
    }

    #[test]
    fn command_error_details_returns_unknown_error_when_both_empty() {
        let output = process::Output {
            status: failing_status(),
            stdout: Vec::new(),
            stderr: Vec::new(),
        };
        assert_eq!(command_error_details(&output), "unknown error");
    }

    #[test]
    fn command_error_details_returns_unknown_error_when_both_whitespace_only() {
        let output = process::Output {
            status: failing_status(),
            stdout: b"  \n  ".to_vec(),
            stderr: b"\t\t".to_vec(),
        };
        assert_eq!(command_error_details(&output), "unknown error");
    }
}
