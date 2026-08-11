//! Implementation of the `init` subcommand.

use std::path::PathBuf;

use super::{Cli, Command};
use color_eyre::eyre::{OptionExt, Result, bail};

/// If `cli` carries an `init` subcommand, writes a starter `fyai.toml` and
/// returns `Ok(true)`; otherwise returns `Ok(false)` so the caller proceeds
/// with a normal combine run.
///
/// Fails if the target config file already exists and `--force` wasn't
/// passed.
pub fn handle_init_subcommand(cli: &Cli) -> Result<bool> {
    if let Some(Command::Init { global, force }) = &cli.command {
        let global = *global;
        let force = *force;

        let (path, display_path) = if global {
            let cfg_dir = feedyourai::config::system_config_dir()
                .ok_or_eyre("could not determine config directory")?;
            std::fs::create_dir_all(&cfg_dir)?;
            let cfg_path = cfg_dir.join("fyai.toml");
            (cfg_path.clone(), cfg_path.display().to_string())
        } else {
            let local = PathBuf::from("./fyai.toml");
            (local.clone(), local.display().to_string())
        };

        if path.exists() && !force {
            bail!("config file already exists at {display_path}. Use --force to overwrite.");
        }

        let template = r#"# fyai.toml - Configuration file for fyai
# All options are optional. CLI flags override config values.
# See README.md for details.
#
# For path-based exclusion you can also drop a .fyaiignore file (gitignore
# syntax) anywhere under the scanned directory, instead of exclude_dirs/
# exclude_files below.

directory = "."
output = "fyai.txt"
include_dirs = ["src", "docs"]
exclude_dirs = ["node_modules", "dist"]
include_ext = ["md", "txt"]
exclude_ext = ["log", "tmp"]
include_files = ["README.md", "main.rs"]
exclude_files = ["LICENSE", "config.json"]
min_size = 10240
max_size = 512000
hidden = true
gitignore = true
ignore_files = true
git_global = true
follow_links = false
tree_only = false
human = false
"#;

        std::fs::write(&path, template)?;
        if !cli.quiet {
            println!("Template config file written to {}", display_path);
        }
        return Ok(true);
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{CommandFactory, FromArgMatches};
    use serial_test::serial;
    use std::path::PathBuf as StdPathBuf;

    /// Restores the process's original working directory on drop, even if
    /// the test panics, so tests that mutate the cwd don't leak state into
    /// other tests running in the same process.
    struct CwdGuard {
        original: StdPathBuf,
    }

    impl CwdGuard {
        fn new() -> Self {
            let original = std::env::current_dir().expect("current_dir");
            Self { original }
        }
    }

    impl Drop for CwdGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.original);
        }
    }

    /// Restores the original value (set or unset) of an env var on drop.
    struct EnvVarGuard {
        key: &'static str,
        original: Option<std::ffi::OsString>,
    }

    impl EnvVarGuard {
        fn new(key: &'static str) -> Self {
            let original = std::env::var_os(key);
            Self { key, original }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            match &self.original {
                Some(val) => unsafe { std::env::set_var(self.key, val) },
                None => unsafe { std::env::remove_var(self.key) },
            }
        }
    }

    /// Builds a `Cli` by parsing real argv through clap, mirroring how
    /// `app.rs` constructs it.
    fn cli_from_argv(args: &[&str]) -> Cli {
        let matches = Cli::command()
            .try_get_matches_from(args)
            .expect("should parse");
        Cli::from_arg_matches(&matches).expect("should convert")
    }

    #[test]
    fn returns_false_when_no_subcommand() {
        let cli = cli_from_argv(&["fyai"]);
        assert!(cli.command.is_none());
        let result = handle_init_subcommand(&cli).expect("should not error");
        assert!(!result);
    }

    #[test]
    #[serial(env)]
    fn local_init_writes_template_when_absent() {
        let _cwd_guard = CwdGuard::new();
        let dir = tempfile::tempdir().expect("tempdir");
        std::env::set_current_dir(dir.path()).expect("set_current_dir");

        let cli = cli_from_argv(&["fyai", "init"]);
        let result = handle_init_subcommand(&cli).expect("should succeed");
        assert!(result);

        let contents = std::fs::read_to_string("./fyai.toml").expect("read");
        assert!(contents.contains("directory = \".\""));
        assert!(contents.contains("exclude_dirs = [\"node_modules\", \"dist\"]"));
        assert!(contents.contains("output = \"fyai.txt\""));
    }

    #[test]
    #[serial(env)]
    fn local_init_fails_when_file_exists_without_force() {
        let _cwd_guard = CwdGuard::new();
        let dir = tempfile::tempdir().expect("tempdir");
        std::env::set_current_dir(dir.path()).expect("set_current_dir");

        std::fs::write("./fyai.toml", "existing = true").expect("write");

        let cli = cli_from_argv(&["fyai", "init"]);
        let err = handle_init_subcommand(&cli).expect_err("should fail");
        assert!(err.to_string().contains("already exists"));

        // File should be untouched.
        let contents = std::fs::read_to_string("./fyai.toml").expect("read");
        assert_eq!(contents, "existing = true");
    }

    #[test]
    #[serial(env)]
    fn local_init_overwrites_when_force() {
        let _cwd_guard = CwdGuard::new();
        let dir = tempfile::tempdir().expect("tempdir");
        std::env::set_current_dir(dir.path()).expect("set_current_dir");

        std::fs::write("./fyai.toml", "existing = true").expect("write");

        let cli = cli_from_argv(&["fyai", "init", "--force"]);
        let result = handle_init_subcommand(&cli).expect("should succeed");
        assert!(result);

        let contents = std::fs::read_to_string("./fyai.toml").expect("read");
        assert!(contents.contains("directory = \".\""));
        assert!(!contents.contains("existing = true"));
    }

    #[test]
    #[serial(env)]
    fn global_init_creates_dir_and_writes_template_when_absent() {
        let _env_guard = EnvVarGuard::new("XDG_CONFIG_HOME");
        let dir = tempfile::tempdir().expect("tempdir");
        let cfg_dir = dir.path().join("cfgdir");
        assert!(!cfg_dir.exists());
        unsafe { std::env::set_var("XDG_CONFIG_HOME", &cfg_dir) };

        let cli = cli_from_argv(&["fyai", "init", "--global"]);
        let result = handle_init_subcommand(&cli).expect("should succeed");
        assert!(result);

        assert!(cfg_dir.is_dir());
        let cfg_path = cfg_dir.join("fyai.toml");
        let contents = std::fs::read_to_string(&cfg_path).expect("read");
        assert!(contents.contains("directory = \".\""));
        assert!(contents.contains("exclude_dirs = [\"node_modules\", \"dist\"]"));
    }

    #[test]
    #[serial(env)]
    fn global_init_fails_when_file_exists_without_force() {
        let _env_guard = EnvVarGuard::new("XDG_CONFIG_HOME");
        let dir = tempfile::tempdir().expect("tempdir");
        let cfg_dir = dir.path().join("cfgdir");
        std::fs::create_dir_all(&cfg_dir).expect("create_dir_all");
        std::fs::write(cfg_dir.join("fyai.toml"), "existing = true").expect("write");
        unsafe { std::env::set_var("XDG_CONFIG_HOME", &cfg_dir) };

        let cli = cli_from_argv(&["fyai", "init", "--global"]);
        let err = handle_init_subcommand(&cli).expect_err("should fail");
        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    #[serial(env)]
    fn global_init_overwrites_when_force() {
        let _env_guard = EnvVarGuard::new("XDG_CONFIG_HOME");
        let dir = tempfile::tempdir().expect("tempdir");
        let cfg_dir = dir.path().join("cfgdir");
        std::fs::create_dir_all(&cfg_dir).expect("create_dir_all");
        std::fs::write(cfg_dir.join("fyai.toml"), "existing = true").expect("write");
        unsafe { std::env::set_var("XDG_CONFIG_HOME", &cfg_dir) };

        let cli = cli_from_argv(&["fyai", "init", "--global", "--force"]);
        let result = handle_init_subcommand(&cli).expect("should succeed");
        assert!(result);

        let contents = std::fs::read_to_string(cfg_dir.join("fyai.toml")).expect("read");
        assert!(contents.contains("directory = \".\""));
        assert!(!contents.contains("existing = true"));
    }
}
