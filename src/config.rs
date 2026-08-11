//! Configuration types (CLI-agnostic) and config-file discovery/merging.
//!
//! [`Config`](crate::config::Config) is what the scanning/combining logic
//! actually runs on.
//! [`PartialConfig`](crate::config::PartialConfig) is its optional,
//! partially-specified counterpart: one instance loaded from a `fyai.toml`
//! file, another built from CLI flags (`None` for anything not explicitly
//! passed, so an unset CLI flag can't shadow a config-file value).
//! [`merge_config`](crate::config::merge_config) reconciles the two, CLI
//! winning over file, file winning over the built-in default.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{FyaiError, Result};

/// Fully-resolved configuration for a single combine run.
#[derive(Debug, PartialEq, Clone)]
pub struct Config {
    /// Directory to scan.
    pub directory: PathBuf,
    /// File the combined output is written to.
    pub output: PathBuf,
    /// If set, only directories whose name matches one of these are walked.
    pub include_dirs: Option<Vec<String>>,
    /// Directory names to skip.
    pub exclude_dirs: Option<Vec<String>>,
    /// If set, only files with one of these extensions are included.
    pub include_ext: Option<Vec<String>>,
    /// File extensions to skip.
    pub exclude_ext: Option<Vec<String>>,
    /// If set, only files with one of these names are included.
    pub include_files: Option<Vec<String>>,
    /// File names to skip.
    pub exclude_files: Option<Vec<String>>,
    /// Files smaller than this many bytes are skipped.
    pub min_size: Option<u64>,
    /// Files larger than this many bytes are skipped.
    pub max_size: Option<u64>,
    /// Whether to skip hidden files and directories (dot-files). Independent
    /// of `gitignore`/`ignore_files`/`git_global`.
    pub hidden: bool,
    /// Whether to honor `.gitignore` files: the local `.gitignore`,
    /// `.git/info/exclude`, and `.gitignore` files in directories above
    /// `directory`.
    pub gitignore: bool,
    /// Whether to honor plain `.ignore` files (the ripgrep/ag convention),
    /// independent of `.gitignore`.
    pub ignore_files: bool,
    /// Whether to honor git's global excludes file (`core.excludesFile`,
    /// typically `~/.config/git/ignore`).
    pub git_global: bool,
    /// Whether to follow symbolic links while walking.
    pub follow_links: bool,
    /// If true, only the directory tree is written; file contents are skipped.
    pub tree_only: bool,
    /// If true, renders the directory tree with `tree`-style connector
    /// glyphs (`├──`, `└──`, `│`) instead of the minimal two-space indent.
    pub human: bool,
}

/// Partially-specified configuration, either loaded from a `fyai.toml` file
/// or built from CLI flags. Every field is optional; unset fields fall back
/// to the other source, then to a built-in default, when merged via
/// [`merge_config`].
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct PartialConfig {
    /// See [`Config::directory`].
    pub directory: Option<String>,
    /// See [`Config::output`].
    pub output: Option<String>,
    /// See [`Config::include_dirs`].
    pub include_dirs: Option<Vec<String>>,
    /// See [`Config::exclude_dirs`].
    pub exclude_dirs: Option<Vec<String>>,
    /// See [`Config::include_ext`].
    pub include_ext: Option<Vec<String>>,
    /// See [`Config::exclude_ext`].
    pub exclude_ext: Option<Vec<String>>,
    /// See [`Config::include_files`].
    pub include_files: Option<Vec<String>>,
    /// See [`Config::exclude_files`].
    pub exclude_files: Option<Vec<String>>,
    /// See [`Config::min_size`].
    pub min_size: Option<u64>,
    /// See [`Config::max_size`].
    pub max_size: Option<u64>,
    /// See [`Config::hidden`].
    pub hidden: Option<bool>,
    /// See [`Config::gitignore`].
    pub gitignore: Option<bool>,
    /// See [`Config::ignore_files`].
    pub ignore_files: Option<bool>,
    /// See [`Config::git_global`].
    pub git_global: Option<bool>,
    /// See [`Config::follow_links`].
    pub follow_links: Option<bool>,
    /// See [`Config::tree_only`].
    pub tree_only: Option<bool>,
    /// See [`Config::human`].
    pub human: Option<bool>,
}

impl PartialConfig {
    /// Reads and parses a `fyai.toml`-style config file from `path`.
    ///
    /// # Errors
    ///
    /// Returns [`FyaiError::ReadConfig`] if the file can't be read, or
    /// [`FyaiError::ParseConfig`] if its contents aren't valid TOML for this
    /// type.
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let content = fs::read_to_string(path).map_err(|source| FyaiError::ReadConfig {
            path: path.to_path_buf(),
            source,
        })?;
        let config: PartialConfig =
            toml::from_str(&content).map_err(|source| FyaiError::ParseConfig {
                path: path.to_path_buf(),
                source,
            })?;
        Ok(config)
    }
}

/// Looks for a config file, preferring a local `./fyai.toml` over the
/// system-wide one returned by [`system_config_dir`].
///
/// Returns `None` if neither exists.
pub fn discover_config_file() -> Option<PathBuf> {
    let local = PathBuf::from("./fyai.toml");
    if local.exists() {
        return Some(local);
    }
    if let Some(config_dir) = system_config_dir() {
        let global = config_dir.join("fyai.toml");
        if global.exists() {
            return Some(global);
        }
    }
    None
}

/// Returns the platform's config directory, where the global `fyai.toml`
/// lives: `$XDG_CONFIG_HOME` if set to an absolute path (honored on every
/// platform, not just Linux, matching the XDG Base Directory spec), else
/// the platform default (e.g. `~/.config` on Linux, `~/Library/Application
/// Support` on macOS).
pub fn system_config_dir() -> Option<PathBuf> {
    if let Some(xdg_config_home) = std::env::var_os("XDG_CONFIG_HOME") {
        let path = PathBuf::from(xdg_config_home);
        if path.is_absolute() {
            return Some(path);
        }
    }
    dirs::config_dir()
}

/// Reads `FYAI_*` environment variables into a [`PartialConfig`], mirroring
/// the corresponding CLI flags: `FYAI_DIRECTORY`, `FYAI_OUTPUT`,
/// `FYAI_INCLUDE_DIRS`, `FYAI_EXCLUDE_DIRS`, `FYAI_INCLUDE_EXT`,
/// `FYAI_EXCLUDE_EXT`, `FYAI_INCLUDE_FILES`, `FYAI_EXCLUDE_FILES`,
/// `FYAI_MIN_SIZE`, `FYAI_MAX_SIZE`, `FYAI_HIDDEN`, `FYAI_GITIGNORE`,
/// `FYAI_IGNORE_FILES`, `FYAI_GIT_GLOBAL`, `FYAI_FOLLOW_LINKS`,
/// `FYAI_TREE_ONLY`, `FYAI_HUMAN`.
///
/// List-valued variables use the same comma-separated, trimmed,
/// lower-cased, empty-entry-dropped format as their CLI counterparts.
/// Boolean variables accept `true`/`false` (case-insensitive); anything
/// else, like an unset or unparseable variable, is treated as unset.
///
/// Reads the real process environment, so pass its result into
/// [`merge_config`] explicitly rather than having `merge_config` call it
/// internally — that keeps `merge_config` a pure function of its
/// arguments, which is what lets its own tests run in parallel without
/// mutating shared process state.
pub fn env_config() -> PartialConfig {
    PartialConfig {
        directory: env_string("FYAI_DIRECTORY"),
        output: env_string("FYAI_OUTPUT"),
        include_dirs: env_list("FYAI_INCLUDE_DIRS"),
        exclude_dirs: env_list("FYAI_EXCLUDE_DIRS"),
        include_ext: env_list("FYAI_INCLUDE_EXT"),
        exclude_ext: env_list("FYAI_EXCLUDE_EXT"),
        include_files: env_list("FYAI_INCLUDE_FILES"),
        exclude_files: env_list("FYAI_EXCLUDE_FILES"),
        min_size: env_u64("FYAI_MIN_SIZE"),
        max_size: env_u64("FYAI_MAX_SIZE"),
        hidden: env_bool("FYAI_HIDDEN"),
        gitignore: env_bool("FYAI_GITIGNORE"),
        ignore_files: env_bool("FYAI_IGNORE_FILES"),
        git_global: env_bool("FYAI_GIT_GLOBAL"),
        follow_links: env_bool("FYAI_FOLLOW_LINKS"),
        tree_only: env_bool("FYAI_TREE_ONLY"),
        human: env_bool("FYAI_HUMAN"),
    }
}

fn env_string(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.is_empty())
}

fn env_list(key: &str) -> Option<Vec<String>> {
    env_string(key).map(|v| parse_comma_list(&v))
}

/// Splits a comma-separated list into trimmed, lower-cased entries, dropping
/// any that are empty — the format shared by list-valued CLI flags (e.g.
/// `--include-dirs`) and their `FYAI_*` environment-variable counterparts.
pub fn parse_comma_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

fn env_bool(key: &str) -> Option<bool> {
    env_string(key).and_then(|v| v.to_lowercase().parse::<bool>().ok())
}

fn env_u64(key: &str) -> Option<u64> {
    env_string(key).and_then(|v| v.parse::<u64>().ok())
}

/// Fills any field left unset in `primary` with the corresponding field
/// from `fallback`.
fn or_partial(primary: PartialConfig, fallback: PartialConfig) -> PartialConfig {
    PartialConfig {
        directory: primary.directory.or(fallback.directory),
        output: primary.output.or(fallback.output),
        include_dirs: primary.include_dirs.or(fallback.include_dirs),
        exclude_dirs: primary.exclude_dirs.or(fallback.exclude_dirs),
        include_ext: primary.include_ext.or(fallback.include_ext),
        exclude_ext: primary.exclude_ext.or(fallback.exclude_ext),
        include_files: primary.include_files.or(fallback.include_files),
        exclude_files: primary.exclude_files.or(fallback.exclude_files),
        min_size: primary.min_size.or(fallback.min_size),
        max_size: primary.max_size.or(fallback.max_size),
        hidden: primary.hidden.or(fallback.hidden),
        gitignore: primary.gitignore.or(fallback.gitignore),
        ignore_files: primary.ignore_files.or(fallback.ignore_files),
        git_global: primary.git_global.or(fallback.git_global),
        follow_links: primary.follow_links.or(fallback.follow_links),
        tree_only: primary.tree_only.or(fallback.tree_only),
        human: primary.human.or(fallback.human),
    }
}

/// Merges three [`PartialConfig`]s into a final [`Config`]: `cli`'s value
/// wins wherever set, then `env`'s (see [`env_config`]), then `file`'s,
/// otherwise the built-in default.
pub fn merge_config(file: PartialConfig, env: PartialConfig, cli: PartialConfig) -> Config {
    let cli = or_partial(cli, env);

    let directory = cli
        .directory
        .or(file.directory)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    let output = cli
        .output
        .or(file.output)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("fyai.txt"));

    let hidden = cli.hidden.or(file.hidden).unwrap_or(true);
    let gitignore = cli.gitignore.or(file.gitignore).unwrap_or(true);
    let ignore_files = cli.ignore_files.or(file.ignore_files).unwrap_or(true);
    let git_global = cli.git_global.or(file.git_global).unwrap_or(true);
    let follow_links = cli.follow_links.or(file.follow_links).unwrap_or(false);
    let tree_only = cli.tree_only.or(file.tree_only).unwrap_or(false);
    let human = cli.human.or(file.human).unwrap_or(false);

    Config {
        directory,
        output,
        include_dirs: cli.include_dirs.or(file.include_dirs),
        exclude_dirs: cli.exclude_dirs.or(file.exclude_dirs),
        include_ext: cli.include_ext.or(file.include_ext),
        exclude_ext: cli.exclude_ext.or(file.exclude_ext),
        include_files: cli.include_files.or(file.include_files),
        exclude_files: cli.exclude_files.or(file.exclude_files),
        min_size: cli.min_size.or(file.min_size),
        max_size: cli.max_size.or(file.max_size),
        hidden,
        gitignore,
        ignore_files,
        git_global,
        follow_links,
        tree_only,
        human,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::io::Write;

    // ---- PartialConfig::from_path ----

    #[test]
    fn from_path_reads_valid_toml() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("fyai.toml");
        let mut file = fs::File::create(&path).expect("create");
        writeln!(
            file,
            r#"
            directory = "src"
            output = "out.txt"
            hidden = false
            include_ext = ["rs", "toml"]
            min_size = 10
            "#
        )
        .expect("write");

        let config = PartialConfig::from_path(&path).expect("should parse");
        assert_eq!(config.directory, Some("src".to_string()));
        assert_eq!(config.output, Some("out.txt".to_string()));
        assert_eq!(config.hidden, Some(false));
        assert_eq!(
            config.include_ext,
            Some(vec!["rs".to_string(), "toml".to_string()])
        );
        assert_eq!(config.min_size, Some(10));
    }

    #[test]
    fn from_path_missing_file_returns_read_config_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("does-not-exist.toml");

        let err = PartialConfig::from_path(&path).expect_err("should fail");
        match err {
            FyaiError::ReadConfig {
                path: err_path,
                source: _,
            } => {
                assert_eq!(err_path, path);
            }
            other => panic!("expected ReadConfig, got {other:?}"),
        }
    }

    #[test]
    fn from_path_invalid_toml_returns_parse_config_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("fyai.toml");
        fs::write(&path, "this is not = valid = toml =").expect("write");

        let err = PartialConfig::from_path(&path).expect_err("should fail");
        match err {
            FyaiError::ParseConfig {
                path: err_path,
                source: _,
            } => {
                assert_eq!(err_path, path);
            }
            other => panic!("expected ParseConfig, got {other:?}"),
        }
    }

    // ---- cwd/env test helpers ----

    /// Restores the process's original working directory on drop, even if
    /// the test panics, so tests that mutate the cwd don't leak state into
    /// other tests running in the same process.
    struct CwdGuard {
        original: PathBuf,
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

    /// Restores the original `XDG_CONFIG_HOME` value (set or unset) on drop.
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

    // ---- discover_config_file ----

    #[test]
    #[serial(env)]
    fn discover_config_file_returns_none_when_neither_exists() {
        let _cwd_guard = CwdGuard::new();
        let _env_guard = EnvVarGuard::new("XDG_CONFIG_HOME");

        let dir = tempfile::tempdir().expect("tempdir");
        std::env::set_current_dir(dir.path()).expect("set_current_dir");

        // Point XDG_CONFIG_HOME at an empty directory so the global fallback
        // also misses.
        let xdg_dir = tempfile::tempdir().expect("tempdir");
        unsafe { std::env::set_var("XDG_CONFIG_HOME", xdg_dir.path()) };

        assert_eq!(discover_config_file(), None);
    }

    #[test]
    #[serial(env)]
    fn discover_config_file_finds_local_file() {
        let _cwd_guard = CwdGuard::new();
        let _env_guard = EnvVarGuard::new("XDG_CONFIG_HOME");

        let dir = tempfile::tempdir().expect("tempdir");
        std::env::set_current_dir(dir.path()).expect("set_current_dir");

        let xdg_dir = tempfile::tempdir().expect("tempdir");
        unsafe { std::env::set_var("XDG_CONFIG_HOME", xdg_dir.path()) };

        fs::write(dir.path().join("fyai.toml"), "directory = \".\"").expect("write");

        let found = discover_config_file();
        assert_eq!(found, Some(PathBuf::from("./fyai.toml")));
    }

    #[test]
    #[serial(env)]
    fn discover_config_file_falls_back_to_system_dir_when_local_absent() {
        let _cwd_guard = CwdGuard::new();
        let _env_guard = EnvVarGuard::new("XDG_CONFIG_HOME");

        let dir = tempfile::tempdir().expect("tempdir");
        std::env::set_current_dir(dir.path()).expect("set_current_dir");

        let xdg_dir = tempfile::tempdir().expect("tempdir");
        unsafe { std::env::set_var("XDG_CONFIG_HOME", xdg_dir.path()) };
        fs::write(xdg_dir.path().join("fyai.toml"), "directory = \".\"").expect("write");

        let found = discover_config_file();
        assert_eq!(found, Some(xdg_dir.path().join("fyai.toml")));
    }

    // ---- system_config_dir ----

    #[test]
    #[serial(env)]
    fn system_config_dir_uses_absolute_xdg_config_home() {
        let _env_guard = EnvVarGuard::new("XDG_CONFIG_HOME");

        let dir = tempfile::tempdir().expect("tempdir");
        unsafe { std::env::set_var("XDG_CONFIG_HOME", dir.path()) };

        assert_eq!(system_config_dir(), Some(dir.path().to_path_buf()));
    }

    #[test]
    #[serial(env)]
    fn system_config_dir_ignores_relative_xdg_config_home() {
        let _env_guard = EnvVarGuard::new("XDG_CONFIG_HOME");

        unsafe { std::env::set_var("XDG_CONFIG_HOME", "relative/path") };

        assert_eq!(system_config_dir(), dirs::config_dir());
    }

    #[test]
    #[serial(env)]
    fn system_config_dir_falls_back_when_unset() {
        let _env_guard = EnvVarGuard::new("XDG_CONFIG_HOME");

        unsafe { std::env::remove_var("XDG_CONFIG_HOME") };

        assert_eq!(system_config_dir(), dirs::config_dir());
    }

    // ---- env_config ----

    #[test]
    #[serial(env)]
    fn env_config_reads_string_and_list_and_numeric_and_bool_vars() {
        let _output_guard = EnvVarGuard::new("FYAI_OUTPUT");
        let _dirs_guard = EnvVarGuard::new("FYAI_INCLUDE_DIRS");
        let _min_guard = EnvVarGuard::new("FYAI_MIN_SIZE");
        let _hidden_guard = EnvVarGuard::new("FYAI_HIDDEN");
        unsafe {
            std::env::set_var("FYAI_OUTPUT", "env-out.txt");
            std::env::set_var("FYAI_INCLUDE_DIRS", "Src, ,Test,");
            std::env::set_var("FYAI_MIN_SIZE", "1024");
            std::env::set_var("FYAI_HIDDEN", "FALSE");
        }

        let env = env_config();
        assert_eq!(env.output, Some("env-out.txt".to_string()));
        assert_eq!(
            env.include_dirs,
            Some(vec!["src".to_string(), "test".to_string()])
        );
        assert_eq!(env.min_size, Some(1024));
        assert_eq!(env.hidden, Some(false));
    }

    #[test]
    #[serial(env)]
    fn env_config_ignores_unset_and_unparseable_vars() {
        let _output_guard = EnvVarGuard::new("FYAI_OUTPUT");
        let _min_guard = EnvVarGuard::new("FYAI_MIN_SIZE");
        unsafe {
            std::env::remove_var("FYAI_OUTPUT");
            std::env::set_var("FYAI_MIN_SIZE", "not-a-number");
        }

        let env = env_config();
        assert_eq!(env.output, None);
        assert_eq!(env.min_size, None);
    }

    // ---- merge_config ----

    fn empty_partial() -> PartialConfig {
        PartialConfig::default()
    }

    #[test]
    fn merge_config_env_wins_over_file_when_cli_unset() {
        let file = PartialConfig {
            output: Some("file-out.txt".to_string()),
            ..empty_partial()
        };
        let env = PartialConfig {
            output: Some("env-out.txt".to_string()),
            ..empty_partial()
        };
        let config = merge_config(file, env, empty_partial());
        assert_eq!(config.output, PathBuf::from("env-out.txt"));
    }

    #[test]
    fn merge_config_cli_wins_over_env() {
        let env = PartialConfig {
            output: Some("env-out.txt".to_string()),
            ..empty_partial()
        };
        let cli = PartialConfig {
            output: Some("cli-out.txt".to_string()),
            ..empty_partial()
        };
        let config = merge_config(empty_partial(), env, cli);
        assert_eq!(config.output, PathBuf::from("cli-out.txt"));
    }

    #[test]
    fn merge_config_all_defaults_when_nothing_set() {
        let config = merge_config(empty_partial(), empty_partial(), empty_partial());
        assert_eq!(config.directory, PathBuf::from("."));
        assert_eq!(config.output, PathBuf::from("fyai.txt"));
        assert_eq!(config.include_dirs, None);
        assert_eq!(config.exclude_dirs, None);
        assert_eq!(config.include_ext, None);
        assert_eq!(config.exclude_ext, None);
        assert_eq!(config.include_files, None);
        assert_eq!(config.exclude_files, None);
        assert_eq!(config.min_size, None);
        assert_eq!(config.max_size, None);
        assert!(config.hidden);
        assert!(config.gitignore);
        assert!(config.ignore_files);
        assert!(config.git_global);
        assert!(!config.follow_links);
        assert!(!config.tree_only);
        assert!(!config.human);
    }

    #[test]
    fn merge_config_directory_cli_wins_over_file() {
        let file = PartialConfig {
            directory: Some("from-file".to_string()),
            ..empty_partial()
        };
        let cli = PartialConfig {
            directory: Some("from-cli".to_string()),
            ..empty_partial()
        };
        let config = merge_config(file, empty_partial(), cli);
        assert_eq!(config.directory, PathBuf::from("from-cli"));
    }

    #[test]
    fn merge_config_directory_file_wins_when_cli_unset() {
        let file = PartialConfig {
            directory: Some("from-file".to_string()),
            ..empty_partial()
        };
        let config = merge_config(file, empty_partial(), empty_partial());
        assert_eq!(config.directory, PathBuf::from("from-file"));
    }

    #[test]
    fn merge_config_output_cli_wins_over_file() {
        let file = PartialConfig {
            output: Some("file-out.txt".to_string()),
            ..empty_partial()
        };
        let cli = PartialConfig {
            output: Some("cli-out.txt".to_string()),
            ..empty_partial()
        };
        let config = merge_config(file, empty_partial(), cli);
        assert_eq!(config.output, PathBuf::from("cli-out.txt"));
    }

    #[test]
    fn merge_config_output_file_wins_when_cli_unset() {
        let file = PartialConfig {
            output: Some("file-out.txt".to_string()),
            ..empty_partial()
        };
        let config = merge_config(file, empty_partial(), empty_partial());
        assert_eq!(config.output, PathBuf::from("file-out.txt"));
    }

    macro_rules! bool_field_tests {
        ($field:ident, $default:expr, $cli_wins:ident, $file_wins:ident, $default_test:ident) => {
            #[test]
            fn $cli_wins() {
                let file = PartialConfig {
                    $field: Some(!$default),
                    ..empty_partial()
                };
                let cli = PartialConfig {
                    $field: Some($default),
                    ..empty_partial()
                };
                let config = merge_config(file, empty_partial(), cli);
                assert_eq!(config.$field, $default);
            }

            #[test]
            fn $file_wins() {
                let file = PartialConfig {
                    $field: Some(!$default),
                    ..empty_partial()
                };
                let config = merge_config(file, empty_partial(), empty_partial());
                assert_eq!(config.$field, !$default);
            }

            #[test]
            fn $default_test() {
                let config = merge_config(empty_partial(), empty_partial(), empty_partial());
                assert_eq!(config.$field, $default);
            }
        };
    }

    bool_field_tests!(
        hidden,
        true,
        merge_config_hidden_cli_wins,
        merge_config_hidden_file_wins,
        merge_config_hidden_default
    );
    bool_field_tests!(
        gitignore,
        true,
        merge_config_gitignore_cli_wins,
        merge_config_gitignore_file_wins,
        merge_config_gitignore_default
    );
    bool_field_tests!(
        ignore_files,
        true,
        merge_config_ignore_files_cli_wins,
        merge_config_ignore_files_file_wins,
        merge_config_ignore_files_default
    );
    bool_field_tests!(
        git_global,
        true,
        merge_config_git_global_cli_wins,
        merge_config_git_global_file_wins,
        merge_config_git_global_default
    );
    bool_field_tests!(
        follow_links,
        false,
        merge_config_follow_links_cli_wins,
        merge_config_follow_links_file_wins,
        merge_config_follow_links_default
    );
    bool_field_tests!(
        tree_only,
        false,
        merge_config_tree_only_cli_wins,
        merge_config_tree_only_file_wins,
        merge_config_tree_only_default
    );
    bool_field_tests!(
        human,
        false,
        merge_config_human_cli_wins,
        merge_config_human_file_wins,
        merge_config_human_default
    );

    macro_rules! vec_field_tests {
        ($field:ident, $cli_wins:ident, $file_wins:ident, $default_test:ident) => {
            #[test]
            fn $cli_wins() {
                let file = PartialConfig {
                    $field: Some(vec!["file-value".to_string()]),
                    ..empty_partial()
                };
                let cli = PartialConfig {
                    $field: Some(vec!["cli-value".to_string()]),
                    ..empty_partial()
                };
                let config = merge_config(file, empty_partial(), cli);
                assert_eq!(config.$field, Some(vec!["cli-value".to_string()]));
            }

            #[test]
            fn $file_wins() {
                let file = PartialConfig {
                    $field: Some(vec!["file-value".to_string()]),
                    ..empty_partial()
                };
                let config = merge_config(file, empty_partial(), empty_partial());
                assert_eq!(config.$field, Some(vec!["file-value".to_string()]));
            }

            #[test]
            fn $default_test() {
                let config = merge_config(empty_partial(), empty_partial(), empty_partial());
                assert_eq!(config.$field, None);
            }
        };
    }

    vec_field_tests!(
        include_dirs,
        merge_config_include_dirs_cli_wins,
        merge_config_include_dirs_file_wins,
        merge_config_include_dirs_default
    );
    vec_field_tests!(
        exclude_dirs,
        merge_config_exclude_dirs_cli_wins,
        merge_config_exclude_dirs_file_wins,
        merge_config_exclude_dirs_default
    );
    vec_field_tests!(
        include_ext,
        merge_config_include_ext_cli_wins,
        merge_config_include_ext_file_wins,
        merge_config_include_ext_default
    );
    vec_field_tests!(
        exclude_ext,
        merge_config_exclude_ext_cli_wins,
        merge_config_exclude_ext_file_wins,
        merge_config_exclude_ext_default
    );
    vec_field_tests!(
        include_files,
        merge_config_include_files_cli_wins,
        merge_config_include_files_file_wins,
        merge_config_include_files_default
    );
    vec_field_tests!(
        exclude_files,
        merge_config_exclude_files_cli_wins,
        merge_config_exclude_files_file_wins,
        merge_config_exclude_files_default
    );

    macro_rules! u64_field_tests {
        ($field:ident, $cli_wins:ident, $file_wins:ident, $default_test:ident) => {
            #[test]
            fn $cli_wins() {
                let file = PartialConfig {
                    $field: Some(100),
                    ..empty_partial()
                };
                let cli = PartialConfig {
                    $field: Some(200),
                    ..empty_partial()
                };
                let config = merge_config(file, empty_partial(), cli);
                assert_eq!(config.$field, Some(200));
            }

            #[test]
            fn $file_wins() {
                let file = PartialConfig {
                    $field: Some(100),
                    ..empty_partial()
                };
                let config = merge_config(file, empty_partial(), empty_partial());
                assert_eq!(config.$field, Some(100));
            }

            #[test]
            fn $default_test() {
                let config = merge_config(empty_partial(), empty_partial(), empty_partial());
                assert_eq!(config.$field, None);
            }
        };
    }

    u64_field_tests!(
        min_size,
        merge_config_min_size_cli_wins,
        merge_config_min_size_file_wins,
        merge_config_min_size_default
    );
    u64_field_tests!(
        max_size,
        merge_config_max_size_cli_wins,
        merge_config_max_size_file_wins,
        merge_config_max_size_default
    );
}
