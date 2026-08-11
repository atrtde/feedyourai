//! Shared CLI implementation for the `feedyourai` binary and its `fyai`
//! alias. Included by both `main.rs` files via `#[path]`, so this same
//! source compiles twice, once per binary target.

use std::io::IsTerminal;
use std::time::Duration;

use clap::{CommandFactory, FromArgMatches};
use color_eyre::eyre::{Result, WrapErr};
use indicatif::ProgressBar;

use self::commands::Cli;
use feedyourai::{config, run_git, run_local};

/// System-clipboard access for copying the combined output.
mod clipboard;
/// Argument parsing and the `init` subcommand.
mod commands;

/// Runs the CLI end to end: installs `color_eyre`'s error/panic hooks, then
/// parses the real process arguments and delegates to [`execute`].
pub(crate) fn run() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    install_error_hooks(color_disabled(&args))?;
    execute(args)
}

/// Whether colored error/panic output should be disabled: an explicit
/// `--no-color` flag, the `NO_COLOR` convention (<https://no-color.org/>),
/// or a `dumb` terminal that can't render ANSI escapes.
///
/// Checked against the raw argument list rather than the parsed [`Cli`],
/// since the decision has to be made before `color_eyre`'s hooks are
/// installed — before any error could be reported through them.
fn color_disabled<T: AsRef<std::ffi::OsStr>>(args: &[T]) -> bool {
    args.iter().any(|a| a.as_ref() == "--no-color")
        || std::env::var_os("NO_COLOR").is_some()
        || std::env::var("TERM").is_ok_and(|term| term == "dumb")
}

/// Installs `color_eyre`'s panic/error hooks, with an uncolored theme when
/// `no_color` is set.
fn install_error_hooks(no_color: bool) -> Result<()> {
    let mut builder = color_eyre::config::HookBuilder::default();
    if no_color {
        builder = builder.theme(color_eyre::config::Theme::new());
    }
    builder.install()
}

/// Parses `args`, resolves configuration (merging any `fyai.toml` with CLI
/// flags), runs the combine, and reports the result to stdout/stderr,
/// including a best-effort clipboard copy when `--clipboard` was passed.
///
/// Split out from [`run`] so it can be exercised directly, with an explicit
/// argument list, from tests — `run` itself can't be called more than once
/// per process, since `color_eyre::install()` errors on a second call.
fn execute<I, T>(args: I) -> Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let matches = Cli::command().get_matches_from(args);
    let cli = Cli::from_arg_matches(&matches).wrap_err("failed to parse arguments")?;

    if commands::init::handle_init_subcommand(&cli)? {
        return Ok(());
    }

    let repo_url = cli.repo.clone();
    let repo_branch = cli.repo_branch.clone();
    let repo_commit = cli.repo_commit.clone();

    let cli_config = commands::config_from_matches(matches)?;

    let file_config = match config::discover_config_file() {
        Some(path) => match config::PartialConfig::from_path(&path) {
            Ok(cfg) => {
                if !cli.quiet && !cli.json {
                    println!("Loaded config from: {}", path.display());
                }
                cfg
            }
            Err(e) => {
                eprintln!(
                    "Warning: Failed to load config file ({}): {}",
                    path.display(),
                    e
                );
                config::PartialConfig::default()
            }
        },
        None => config::PartialConfig::default(),
    };

    let config = config::merge_config(file_config, config::env_config(), cli_config);
    let output_path = config.output.clone();
    let tree_only = config.tree_only;

    let show_progress = !cli.quiet && !cli.json && std::io::stdout().is_terminal();
    let _spinner = show_progress.then(|| {
        let pb = ProgressBar::new_spinner();
        pb.enable_steady_tick(Duration::from_millis(100));
        pb.set_message(if repo_url.is_some() {
            "Cloning repository..."
        } else {
            "Scanning directory..."
        });
        SpinnerGuard(pb)
    });

    let stats = if let Some(repo_url) = repo_url {
        run_git(
            &repo_url,
            repo_branch.as_deref(),
            repo_commit.as_deref(),
            config,
        )
        .wrap_err("failed to process git repository")?
    } else {
        run_local(config).wrap_err("failed to process local directory")?
    };

    if tree_only {
        if cli.json {
            let summary = RunSummary {
                output: output_path.display().to_string(),
                tree_only: true,
                total_size: stats.total_size,
                written_size: None,
                binary_size: None,
                size_filtered: None,
                clipboard: None,
            };
            println!(
                "{}",
                serde_json::to_string(&summary).wrap_err("failed to serialize JSON summary")?
            );
        } else if !cli.quiet {
            println!("Project tree written to {}", output_path.display());
            println!("Total size walked: {}", format_size(stats.total_size));
        }
        return Ok(());
    }

    let output_contents = std::fs::read_to_string(&output_path)
        .wrap_err_with(|| format!("failed to read output file {}", output_path.display()))?;

    let size_filtered = stats.size_filtered();

    // A clipboard failure that's expected in this environment (CI, headless
    // Linux) is downgraded to a stderr warning, always shown regardless of
    // `--quiet`/`--json`, rather than aborting the run.
    let clipboard_copied = if cli.clipboard {
        match clipboard::copy_to_clipboard(&output_contents) {
            Ok(()) => Some(true),
            Err(err) if clipboard::should_ignore_clipboard_error() => {
                eprintln!("Warning: clipboard unavailable; skipping copy. {}", err);
                Some(false)
            }
            Err(err) => return Err(err),
        }
    } else {
        None
    };

    if cli.json {
        let summary = RunSummary {
            output: output_path.display().to_string(),
            tree_only: false,
            total_size: stats.total_size,
            written_size: Some(stats.written_size),
            binary_size: Some(stats.binary_size),
            size_filtered: Some(size_filtered),
            clipboard: clipboard_copied,
        };
        println!(
            "{}",
            serde_json::to_string(&summary).wrap_err("failed to serialize JSON summary")?
        );
    } else if !cli.quiet {
        println!("Files combined successfully into {}", output_path.display());
        println!("Total size walked: {}", format_size(stats.total_size));
        println!(
            "  Non-binary (written): {}",
            format_size(stats.written_size)
        );
        println!("  Binary (skipped): {}", format_size(stats.binary_size));
        if size_filtered > 0 {
            println!("  Skipped by size filter: {}", format_size(size_filtered));
        }
        if clipboard_copied == Some(true) {
            println!("Output copied to clipboard successfully!");
        }
    }

    Ok(())
}

/// Clears its spinner on drop, so it disappears whether the run it's
/// tracking succeeds or bails out early through `?`.
struct SpinnerGuard(ProgressBar);

impl Drop for SpinnerGuard {
    fn drop(&mut self) {
        self.0.finish_and_clear();
    }
}

/// A single-line JSON run summary, printed to stdout when `--json` is
/// passed instead of the human-readable status lines.
#[derive(serde::Serialize)]
struct RunSummary {
    output: String,
    tree_only: bool,
    total_size: u64,
    written_size: Option<u64>,
    binary_size: Option<u64>,
    size_filtered: Option<u64>,
    clipboard: Option<bool>,
}

/// Formats `bytes` as a human-readable size (`"512 B"`, `"1.2 KB"`, `"3.4
/// MB"`, ...), using 1024 as the unit step.
///
/// Mirrors the library's own (crate-private) `scanner::process::format_size`
/// used for per-file headings; kept separate since the library exposes no
/// public formatting API for binaries to share.
fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];

    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::path::PathBuf;

    /// Restores the process's working directory on drop, even on panic —
    /// `execute` reads `./fyai.toml` relative to cwd, so tests that don't
    /// want a config file picked up (or want a specific one) must isolate
    /// their cwd.
    struct CwdGuard {
        original: PathBuf,
    }

    impl CwdGuard {
        fn enter(dir: &std::path::Path) -> Self {
            let original = std::env::current_dir().unwrap();
            std::env::set_current_dir(dir).unwrap();
            Self { original }
        }
    }

    impl Drop for CwdGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.original);
        }
    }

    /// Restores the `CI` env var on drop — `should_ignore_clipboard_error`
    /// reads it, and this sandbox has no real clipboard, so combine runs
    /// that reach the clipboard step need `CI` set for a deterministic
    /// `Ok(())`.
    struct CiEnvGuard {
        original: Option<std::ffi::OsString>,
    }

    impl CiEnvGuard {
        fn set() -> Self {
            let original = std::env::var_os("CI");
            unsafe { std::env::set_var("CI", "1") };
            Self { original }
        }
    }

    impl Drop for CiEnvGuard {
        fn drop(&mut self) {
            match &self.original {
                Some(value) => unsafe { std::env::set_var("CI", value) },
                None => unsafe { std::env::remove_var("CI") },
            }
        }
    }

    #[test]
    #[serial(env)]
    fn execute_init_subcommand_writes_config_and_returns_ok() {
        let dir = tempfile::tempdir().unwrap();
        let _cwd = CwdGuard::enter(dir.path());

        let result = execute(["fyai", "init"]);

        assert!(result.is_ok());
        assert!(dir.path().join("fyai.toml").exists());
    }

    #[test]
    #[serial(env)]
    fn execute_init_subcommand_fails_when_config_already_exists() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("fyai.toml"), "directory = \".\"\n").unwrap();
        let _cwd = CwdGuard::enter(dir.path());

        let err = execute(["fyai", "init"]).unwrap_err();

        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    #[serial(env)]
    fn execute_combines_local_directory_without_touching_clipboard_by_default() {
        let scan_dir = tempfile::tempdir().unwrap();
        std::fs::write(scan_dir.path().join("a.txt"), "hello").unwrap();
        let cwd_dir = tempfile::tempdir().unwrap();
        let output = cwd_dir.path().join("out.txt");
        let _cwd = CwdGuard::enter(cwd_dir.path());

        // No CiEnvGuard here: the clipboard step is only reached with
        // `--clipboard`, so this must succeed regardless of CI/clipboard
        // availability.
        let result = execute([
            "fyai".into(),
            "-i".into(),
            scan_dir.path().as_os_str().to_owned(),
            "-o".into(),
            output.as_os_str().to_owned(),
        ]);

        assert!(result.is_ok(), "{result:?}");
        let contents = std::fs::read_to_string(&output).unwrap();
        assert!(contents.contains("a.txt"));
        assert!(contents.contains("hello"));
    }

    #[test]
    #[serial(env)]
    fn execute_combines_local_directory_and_copies_to_clipboard_when_flag_passed() {
        let scan_dir = tempfile::tempdir().unwrap();
        std::fs::write(scan_dir.path().join("a.txt"), "hello").unwrap();
        let cwd_dir = tempfile::tempdir().unwrap();
        let output = cwd_dir.path().join("out.txt");
        let _cwd = CwdGuard::enter(cwd_dir.path());
        let _ci = CiEnvGuard::set();

        let result = execute([
            "fyai".into(),
            "-i".into(),
            scan_dir.path().as_os_str().to_owned(),
            "-o".into(),
            output.as_os_str().to_owned(),
            "--clipboard".into(),
        ]);

        assert!(result.is_ok(), "{result:?}");
        let contents = std::fs::read_to_string(&output).unwrap();
        assert!(contents.contains("a.txt"));
        assert!(contents.contains("hello"));
    }

    #[test]
    #[serial(env)]
    fn execute_tree_only_returns_ok_without_touching_clipboard() {
        let scan_dir = tempfile::tempdir().unwrap();
        std::fs::write(scan_dir.path().join("a.txt"), "hello").unwrap();
        let cwd_dir = tempfile::tempdir().unwrap();
        let output = cwd_dir.path().join("out.txt");
        let _cwd = CwdGuard::enter(cwd_dir.path());

        // No CiEnvGuard here: tree-only returns before the clipboard step
        // is ever reached, so this must succeed regardless of CI/clipboard
        // availability.
        let result = execute([
            "fyai".into(),
            "-i".into(),
            scan_dir.path().as_os_str().to_owned(),
            "-o".into(),
            output.as_os_str().to_owned(),
            "--tree-only".into(),
        ]);

        assert!(result.is_ok(), "{result:?}");
        assert!(output.exists());
    }

    #[test]
    #[serial(env)]
    fn execute_loads_valid_local_config_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "hello").unwrap();
        std::fs::write(
            dir.path().join("fyai.toml"),
            "output = \"out.txt\"\ntree_only = true\n",
        )
        .unwrap();
        let _cwd = CwdGuard::enter(dir.path());

        let result = execute(["fyai"]);

        assert!(result.is_ok(), "{result:?}");
        assert!(dir.path().join("out.txt").exists());
    }

    #[test]
    #[serial(env)]
    fn execute_warns_and_falls_back_on_invalid_local_config_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "hello").unwrap();
        std::fs::write(dir.path().join("fyai.toml"), "not = [valid toml").unwrap();
        let _cwd = CwdGuard::enter(dir.path());
        let _ci = CiEnvGuard::set();

        let result = execute(["fyai", "-o", "out.txt"]);

        assert!(result.is_ok(), "{result:?}");
        assert!(dir.path().join("out.txt").exists());
    }

    #[test]
    #[serial(env)]
    fn execute_propagates_local_directory_error() {
        let cwd_dir = tempfile::tempdir().unwrap();
        let _cwd = CwdGuard::enter(cwd_dir.path());
        let missing = cwd_dir.path().join("does-not-exist");

        let err = execute([
            "fyai".into(),
            "-i".into(),
            missing.as_os_str().to_owned(),
            "-o".into(),
            cwd_dir.path().join("out.txt").as_os_str().to_owned(),
        ])
        .unwrap_err();

        assert!(
            err.to_string()
                .contains("failed to process local directory")
        );
    }

    #[test]
    #[serial(env)]
    fn execute_propagates_git_repo_error() {
        let cwd_dir = tempfile::tempdir().unwrap();
        let _cwd = CwdGuard::enter(cwd_dir.path());

        let err = execute([
            "fyai",
            "--repo",
            "/nonexistent/path/that/does/not/exist",
            "-o",
            "out.txt",
        ])
        .unwrap_err();

        assert!(err.to_string().contains("failed to process git repository"));
    }

    // ---- format_size ----

    #[test]
    fn format_size_stays_bytes_under_1024() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(1023), "1023 B");
    }

    #[test]
    fn format_size_kb_mb_gb() {
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1024 * 1024), "1.0 MB");
        assert_eq!(format_size(1024 * 1024 * 1024), "1.0 GB");
    }
}
