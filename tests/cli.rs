//! Black-box end-to-end tests for the `fyai`/`feedyourai` binaries, driven
//! through the compiled executables via `assert_cmd`. These exercise
//! `app::run()` (the shared CLI implementation in
//! `src/bin/feedyourai/app.rs`), which isn't reachable from unit tests
//! since it parses real `std::env::args()` and calls `color_eyre::install()`
//! once per process.
//!
//! Every test spawns a fresh subprocess with its own `--current-dir`/env,
//! so no `#[serial]` coordination with other test files is needed.

use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;

fn fyai() -> Command {
    Command::cargo_bin("fyai").expect("fyai binary should build")
}

#[test]
fn combines_files_and_writes_output() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.rs"), "fn main() {}\n").unwrap();
    fs::write(dir.path().join("b.md"), "# Hello\n").unwrap();

    let output = dir.path().join("out.txt");

    fyai()
        .arg("-i")
        .arg(dir.path())
        .arg("-o")
        .arg(&output)
        .assert()
        .success()
        .stdout(predicate::str::contains("Files combined successfully into"))
        .stdout(predicate::str::contains("Total size walked:"))
        // No `--clipboard` flag passed: the clipboard must not be touched,
        // so no clipboard-related message should appear.
        .stdout(predicate::str::contains("clipboard").not());

    let contents = fs::read_to_string(&output).unwrap();
    assert!(contents.contains("a.rs"));
    assert!(contents.contains("fn main()"));
    assert!(contents.contains("b.md"));
    assert!(contents.contains("# Hello"));
}

#[test]
fn quiet_suppresses_status_output() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "hello").unwrap();
    let output = dir.path().join("out.txt");

    fyai()
        .arg("-i")
        .arg(dir.path())
        .arg("-o")
        .arg(&output)
        .arg("-q")
        .assert()
        .success()
        .stdout(predicate::str::is_empty());

    assert!(output.exists());
}

#[test]
fn json_flag_prints_single_line_json_summary() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "hello").unwrap();
    let output = dir.path().join("out.txt");

    let assert = fyai()
        .arg("-i")
        .arg(dir.path())
        .arg("-o")
        .arg(&output)
        .arg("--json")
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(
        lines.len(),
        1,
        "expected exactly one JSON line, got: {stdout:?}"
    );

    let json: serde_json::Value = serde_json::from_str(lines[0]).expect("valid JSON");
    assert_eq!(json["tree_only"], false);
    assert_eq!(json["total_size"], 5);
    assert_eq!(json["written_size"], 5);
}

#[test]
fn no_color_flag_disables_ansi_in_error_output() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("does-not-exist");
    let output = dir.path().join("out.txt");

    let assert = fyai()
        .arg("-i")
        .arg(&missing)
        .arg("-o")
        .arg(&output)
        .arg("--no-color")
        .assert()
        .failure();

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).to_string();
    assert!(
        !stderr.contains('\u{1b}'),
        "expected no ANSI escapes in stderr, got: {stderr:?}"
    );
}

#[test]
fn no_color_env_var_disables_ansi_in_error_output() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("does-not-exist");
    let output = dir.path().join("out.txt");

    let assert = fyai()
        .arg("-i")
        .arg(&missing)
        .arg("-o")
        .arg(&output)
        .env("NO_COLOR", "1")
        .assert()
        .failure();

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).to_string();
    assert!(
        !stderr.contains('\u{1b}'),
        "expected no ANSI escapes in stderr, got: {stderr:?}"
    );
}

#[test]
fn existing_output_without_force_fails_noninteractively() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "hello").unwrap();
    let output = dir.path().join("out.txt");
    fs::write(&output, "old contents").unwrap();

    fyai()
        .arg("-i")
        .arg(dir.path())
        .arg("-o")
        .arg(&output)
        // A piped, empty stdin is never a terminal, exercising the same
        // "no one to prompt" path as a real non-interactive script.
        .write_stdin("")
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"))
        .stderr(predicate::str::contains("--force"));

    assert_eq!(fs::read_to_string(&output).unwrap(), "old contents");
}

#[test]
fn force_flag_overwrites_existing_output_without_prompting() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "hello").unwrap();
    let output = dir.path().join("out.txt");
    fs::write(&output, "old contents").unwrap();

    fyai()
        .arg("-i")
        .arg(dir.path())
        .arg("-o")
        .arg(&output)
        .arg("--force")
        .assert()
        .success();

    let contents = fs::read_to_string(&output).unwrap();
    assert!(contents.contains("hello"));
    assert!(!contents.contains("old contents"));
}

#[test]
fn man_subcommand_prints_roff_man_page() {
    fyai()
        .arg("man")
        .assert()
        .success()
        .stdout(predicate::str::contains(".TH fyai 1"))
        .stdout(predicate::str::contains(".SH NAME"));
}

#[test]
fn tree_only_skips_file_contents() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("secret.txt"), "top-secret-body").unwrap();

    let output = dir.path().join("out.txt");

    fyai()
        .arg("-i")
        .arg(dir.path())
        .arg("-o")
        .arg(&output)
        .arg("--tree-only")
        .assert()
        .success()
        .stdout(predicate::str::contains("Project tree written to"));

    let contents = fs::read_to_string(&output).unwrap();
    assert!(contents.contains("secret.txt"));
    assert!(!contents.contains("top-secret-body"));
}

#[test]
fn nonexistent_input_directory_fails() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("does-not-exist");
    let output = dir.path().join("out.txt");

    fyai()
        .arg("-i")
        .arg(&missing)
        .arg("-o")
        .arg(&output)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "failed to process local directory",
        ));
}

#[test]
fn init_writes_local_config_and_run_loads_it() {
    let dir = tempfile::tempdir().unwrap();

    fyai()
        .arg("init")
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Template config file written to"));

    let config_path = dir.path().join("fyai.toml");
    assert!(config_path.exists());
    let template = fs::read_to_string(&config_path).unwrap();
    assert!(template.contains("directory = \".\""));

    // A plain run from that directory should pick up the local fyai.toml.
    fyai()
        .current_dir(dir.path())
        .env("CI", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("Loaded config from:"));
}

#[test]
fn init_without_force_refuses_to_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("fyai.toml"), "directory = \".\"\n").unwrap();

    fyai()
        .arg("init")
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn init_with_force_overwrites() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("fyai.toml"), "directory = \"old\"\n").unwrap();

    fyai()
        .args(["init", "--force"])
        .current_dir(dir.path())
        .assert()
        .success();

    let template = fs::read_to_string(dir.path().join("fyai.toml")).unwrap();
    assert!(template.contains("directory = \".\""));
}

#[test]
fn invalid_config_file_warns_and_falls_back_to_defaults() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("fyai.toml"), "not = [valid toml").unwrap();
    fs::write(dir.path().join("a.txt"), "hello").unwrap();

    fyai()
        .arg("-o")
        .arg("out.txt")
        .current_dir(dir.path())
        .env("CI", "1")
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "Warning: Failed to load config file",
        ));

    assert!(dir.path().join("out.txt").exists());
}

#[test]
fn clipboard_warning_or_success_reported_in_ci() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "hello").unwrap();
    let output = dir.path().join("out.txt");

    // Force the "ignore clipboard errors" path so the assertion is
    // deterministic regardless of the sandbox's real clipboard access.
    // Success is printed to stdout, the fallback warning to stderr.
    let assert = fyai()
        .arg("-i")
        .arg(dir.path())
        .arg("-o")
        .arg(&output)
        .arg("--clipboard")
        .env("CI", "1")
        .assert()
        .success();

    let out = assert.get_output();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stdout.contains("Output copied to clipboard successfully!")
            || stderr.contains("clipboard unavailable; skipping copy"),
        "expected clipboard success or warning, got stdout={stdout:?} stderr={stderr:?}"
    );
}

#[test]
fn repo_flag_clones_and_combines_local_repo() {
    let repo_dir = tempfile::tempdir().unwrap();
    let repo_path = repo_dir.path();

    let git = |args: &[&str]| {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(repo_path)
            .args(args)
            .status()
            .expect("git should run");
        assert!(status.success(), "git {args:?} failed");
    };

    git(&["init", "-q"]);
    fs::write(repo_path.join("readme.md"), "# Repo Fixture\n").unwrap();
    git(&["add", "."]);
    git(&[
        "-c",
        "user.email=test@test.com",
        "-c",
        "user.name=Test",
        "commit",
        "-q",
        "-m",
        "init",
    ]);

    let workdir = tempfile::tempdir().unwrap();
    let output = workdir.path().join("out.txt");

    fyai()
        .arg("--repo")
        .arg(repo_path)
        .arg("-o")
        .arg(&output)
        .env("CI", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("Files combined successfully into"));

    let contents = fs::read_to_string(&output).unwrap();
    assert!(contents.contains("readme.md"));
    assert!(contents.contains("# Repo Fixture"));
}

#[test]
fn repo_flag_with_bogus_url_fails() {
    let workdir = tempfile::tempdir().unwrap();
    let output = workdir.path().join("out.txt");

    fyai()
        .arg("--repo")
        .arg("/nonexistent/path/that/does/not/exist")
        .arg("-o")
        .arg(&output)
        .assert()
        .failure()
        .stderr(predicate::str::contains("failed to process git repository"));
}

#[test]
fn repo_and_input_conflict_is_rejected_by_clap() {
    fyai()
        .args(["--repo", "https://example.com/x.git", "-i", "somedir"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn feedyourai_binary_alias_behaves_the_same() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "hello").unwrap();
    let output = dir.path().join("out.txt");

    Command::cargo_bin("feedyourai")
        .expect("feedyourai binary should build")
        .arg("-i")
        .arg(dir.path())
        .arg("-o")
        .arg(&output)
        .env("CI", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("Files combined successfully into"));
}
