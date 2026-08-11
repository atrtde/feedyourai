//! Argument parsing: the [`Cli`] struct, its `init` subcommand, and
//! conversion of parsed `clap` matches into a library
//! [`PartialConfig`](feedyourai::config::PartialConfig).

use clap::{ArgAction, Parser, Subcommand, parser::ValueSource};

use color_eyre::eyre::{Result, eyre};
use feedyourai::config::PartialConfig;

/// The `completions` subcommand: prints a shell completion script.
pub mod completions;
/// The `init` subcommand: writes a starter `fyai.toml`.
pub mod init;
/// The `man` subcommand: prints a roff-formatted man page.
pub mod man;

/// Top-level command-line arguments.
#[derive(Parser, Debug)]
#[command(
    name = "fyai",
    version = env!("CARGO_PKG_VERSION"),
    about = "A tool to combine text files for LLM processing with flexible filtering options.\n\nEXAMPLE:\n  fyai -i ./src --include-ext rs,toml -o combined.txt\n\nRun `fyai --help` for the full flag reference, config precedence, and more examples.",
    long_about = "A tool to combine text files for LLM processing with flexible filtering options.\n\nEXAMPLES:\n  fyai -i ./src --include-ext rs,toml -o combined.txt\n  fyai --repo https://github.com/owner/repo.git --tree-only\n  fyai --json -q -o out.txt\n  fyai init\n\nCONFIG FILE SUPPORT:\n  - You can specify options in a config file (TOML format).\n  - Local config: ./fyai.toml (used if present in current directory)\n  - Global config: system config directory (used if no local config found).\n    Honors $XDG_CONFIG_HOME (any platform, if set to an absolute path),\n    else the platform default. Run `fyai init --global` to see the exact path.\n  - Precedence: CLI flags > FYAI_* environment variables > config file > built-in defaults.\n  - You can also drop a .fyaiignore file (gitignore syntax) to exclude paths.\n  - See README for details and examples.\n\nMAN PAGE:\n  Run `fyai man` to print a roff-formatted man page.\n\nSHELL COMPLETIONS:\n  Run `fyai completions <bash|zsh|fish|powershell|elvish>` to print a completion script.\n\nSUPPORT:\n  Report issues at https://github.com/alexandretrotel/feedyourai/issues"
)]
pub struct Cli {
    /// Sets the input directory.
    #[arg(
        short = 'i',
        long = "input",
        value_name = "DIR",
        default_value = ".",
        help = "Sets the input directory"
    )]
    pub input: String,

    /// Sets the output file.
    #[arg(
        short = 'o',
        long = "output",
        value_name = "FILE",
        default_value = "fyai.txt",
        help = "Sets the output file"
    )]
    pub output: String,

    /// Sets the git repository URL to clone and scan instead of a local
    /// directory.
    #[arg(
        long = "repo",
        value_name = "URL",
        conflicts_with = "input",
        help = "Sets the git repository URL"
    )]
    pub repo: Option<String>,

    /// Sets the git repository branch or tag to check out. Requires `--repo`.
    #[arg(
        long = "repo-branch",
        value_name = "BRANCH",
        requires = "repo",
        help = "Sets the git repository branch or tag"
    )]
    pub repo_branch: Option<String>,

    /// Sets the git repository commit SHA to check out. Requires `--repo`.
    #[arg(
        long = "repo-commit",
        value_name = "COMMIT",
        requires = "repo",
        help = "Sets the git repository commit SHA"
    )]
    pub repo_commit: Option<String>,

    /// Sets the directories to include (e.g., `src,tests`).
    #[arg(
        long = "include-dirs",
        value_name = "DIRS",
        help = "Sets the directories to include (e.g., src,tests)"
    )]
    pub include_dirs: Option<String>,

    /// Sets the directories to exclude (e.g., `node_modules,target`).
    #[arg(
        long = "exclude-dirs",
        value_name = "DIRS",
        help = "Sets the directories to exclude (e.g., node_modules,target)"
    )]
    pub exclude_dirs: Option<String>,

    /// Sets the file extensions to include (e.g., `.json,.toml`).
    #[arg(
        long = "include-ext",
        value_name = "EXT",
        help = "Sets the file extensions to include (e.g., .json,.toml)"
    )]
    pub include_ext: Option<String>,

    /// Sets the file extensions to exclude (e.g., `.json,.toml`).
    #[arg(
        long = "exclude-ext",
        value_name = "EXT",
        help = "Sets the file extensions to exclude (e.g., .json,.toml)"
    )]
    pub exclude_ext: Option<String>,

    /// Sets the file names to include (e.g., `README.md,main.rs`).
    #[arg(
        long = "include-files",
        value_name = "FILES",
        help = "Sets the file names to include (e.g., README.md,main.rs)"
    )]
    pub include_files: Option<String>,

    /// Sets the file names to exclude (e.g., `LICENSE,config.json`).
    #[arg(
        long = "exclude-files",
        value_name = "FILES",
        help = "Sets the file names to exclude (e.g., LICENSE,config.json)"
    )]
    pub exclude_files: Option<String>,

    /// Excludes files smaller than this size in bytes.
    #[arg(
        short = 'n',
        long = "min-size",
        value_name = "BYTES",
        help = "Exclude files smaller than this size in bytes"
    )]
    pub min_size: Option<u64>,

    /// Excludes files larger than this size in bytes.
    #[arg(
        short = 'm',
        long = "max-size",
        value_name = "BYTES",
        help = "Exclude files larger than this size in bytes"
    )]
    pub max_size: Option<u64>,

    /// Sets whether to skip hidden files/directories (dot-files) \[default:
    /// true\].
    #[arg(
        long = "no-hidden",
        action = ArgAction::SetTrue,
        help = "Sets whether to skip hidden files/directories (dot-files) [default: true]"
    )]
    pub no_hidden: bool,

    /// Sets whether to respect `.gitignore`/`.git/info/exclude`/parent
    /// `.gitignore` files \[default: true\]. `.fyaiignore` is always
    /// respected regardless of this flag.
    #[arg(
        long = "no-gitignore",
        action = ArgAction::SetTrue,
        help = "Sets whether to respect .gitignore/.git/info/exclude/parent .gitignore files [default: true] (.fyaiignore is always respected)"
    )]
    pub no_gitignore: bool,

    /// Sets whether to respect plain `.ignore` files (the ripgrep/ag
    /// convention), independent of `.gitignore` \[default: true\].
    #[arg(
        long = "no-ignore-files",
        action = ArgAction::SetTrue,
        help = "Sets whether to respect plain .ignore files, independent of .gitignore [default: true]"
    )]
    pub no_ignore_files: bool,

    /// Sets whether to respect git's global excludes file \[default: true\].
    #[arg(
        long = "no-git-global",
        action = ArgAction::SetTrue,
        help = "Sets whether to respect git's global excludes file [default: true]"
    )]
    pub no_git_global: bool,

    /// Follows symbolic links while walking \[default: false\].
    #[arg(
        long = "follow-links",
        action = ArgAction::SetTrue,
        help = "Follow symbolic links while walking [default: false]"
    )]
    pub follow_links: bool,

    /// Only outputs the project directory tree, no file contents.
    #[arg(long = "tree-only", action = ArgAction::SetTrue, help = "Only output the project directory tree, no file contents")]
    pub tree_only: bool,

    /// Renders the directory tree with `tree`-style connector glyphs instead
    /// of the minimal two-space indent.
    #[arg(long = "human", action = ArgAction::SetTrue, help = "Render the directory tree with tree-style connector glyphs")]
    pub human: bool,

    /// Copies the combined output to the system clipboard \[default: false\].
    #[arg(
        short = 'c',
        long = "clipboard",
        action = ArgAction::SetTrue,
        help = "Copy the combined output to the system clipboard [default: false]"
    )]
    pub clipboard: bool,

    /// Runs in test mode.
    #[arg(short = 't', long = "test", action = ArgAction::SetTrue, help = "Run in test mode")]
    pub test: bool,

    /// Suppresses non-essential status output (config-loaded notice, size
    /// breakdown, success messages) \[default: false\]. Errors and warnings
    /// still print to stderr.
    #[arg(
        short = 'q',
        long = "quiet",
        action = ArgAction::SetTrue,
        help = "Suppress non-essential status output [default: false]"
    )]
    pub quiet: bool,

    /// Prints the run summary as a single line of JSON to stdout instead of
    /// human-readable text \[default: false\]. Implies the same output
    /// suppression as `--quiet` for the human-readable lines.
    #[arg(
        long = "json",
        action = ArgAction::SetTrue,
        help = "Print the run summary as JSON instead of human-readable text [default: false]"
    )]
    pub json: bool,

    /// Disables colored error/panic output \[default: false\]. Also honored
    /// via the `NO_COLOR` environment variable or `TERM=dumb`.
    #[arg(
        long = "no-color",
        action = ArgAction::SetTrue,
        help = "Disable colored error/panic output [default: false] (also honors NO_COLOR, TERM=dumb)"
    )]
    pub no_color: bool,

    /// Overwrites an existing output file without prompting \[default:
    /// false\]. Required when stdin isn't a terminal, since there's no one
    /// to prompt.
    #[arg(
        short = 'f',
        long = "force",
        action = ArgAction::SetTrue,
        help = "Overwrite an existing output file without prompting [default: false]"
    )]
    pub force: bool,

    /// Optional subcommand (currently `init` and `man`).
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Subcommands available alongside the default combine behavior.
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Writes a starter `fyai.toml` config file.
    Init {
        /// Generates the config in the system config directory instead of
        /// the current directory.
        #[arg(
            long = "global",
            action = ArgAction::SetTrue,
            help = "Generate config in the system config directory (see `fyai --help`)"
        )]
        global: bool,

        /// Overwrites an existing config file if present.
        #[arg(long = "force", action = ArgAction::SetTrue, help = "Overwrite existing config file if present")]
        force: bool,
    },

    /// Prints a roff-formatted man page to stdout.
    ///
    /// e.g. `fyai man > /usr/local/share/man/man1/fyai.1`
    Man,

    /// Prints a shell completion script to stdout.
    ///
    /// e.g. `fyai completions zsh > "${fpath[1]}/_fyai"`
    Completions {
        /// Shell to generate the completion script for.
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

/// Converts parsed `clap` matches into a [`PartialConfig`], leaving a field
/// `None` unless it was explicitly set on the command line — so an unset
/// flag can't shadow a `fyai.toml` value when [`merge_config`] reconciles
/// the two.
///
/// [`merge_config`]: feedyourai::config::merge_config
///
/// Comma-separated list options (`include_dirs`, `exclude_ext`, ...) are
/// split, trimmed, lower-cased, and emptied entries dropped.
pub fn config_from_matches(matches: clap::ArgMatches) -> Result<PartialConfig> {
    let directory = explicit_string(&matches, "input");
    let output = explicit_string(&matches, "output");

    let include_dirs = match matches.try_get_one::<String>("include_dirs") {
        Ok(opt) => opt.map(|dirs| {
            dirs.split(',')
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
        }),
        Err(_) => None,
    };

    let exclude_dirs = match matches.try_get_one::<String>("exclude_dirs") {
        Ok(opt) => opt.map(|dirs| {
            dirs.split(',')
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
        }),
        Err(_) => None,
    };

    let include_ext = match matches.try_get_one::<String>("include_ext") {
        Ok(opt) => opt.map(|ext| {
            ext.split(',')
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
        }),
        Err(_) => None,
    };

    let exclude_ext = match matches.try_get_one::<String>("exclude_ext") {
        Ok(opt) => opt.map(|ext| {
            ext.split(',')
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
        }),
        Err(_) => None,
    };

    let include_files = match matches.try_get_one::<String>("include_files") {
        Ok(opt) => opt.map(|files| {
            files
                .split(',')
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
        }),
        Err(_) => None,
    };

    let exclude_files = match matches.try_get_one::<String>("exclude_files") {
        Ok(opt) => opt.map(|files| {
            files
                .split(',')
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
        }),
        Err(_) => None,
    };

    let min_size = match matches.try_get_one::<u64>("min_size") {
        Ok(Some(value)) => Some(*value),
        Ok(None) | Err(_) => match matches.try_get_one::<String>("min_size") {
            Ok(Some(s)) => Some(s.parse::<u64>().map_err(|_| eyre!("invalid min-size"))?),
            Ok(None) | Err(_) => None,
        },
    };

    let max_size = match matches.try_get_one::<u64>("max_size") {
        Ok(Some(value)) => Some(*value),
        Ok(None) | Err(_) => match matches.try_get_one::<String>("max_size") {
            Ok(Some(s)) => Some(s.parse::<u64>().map_err(|_| eyre!("invalid max-size"))?),
            Ok(None) | Err(_) => None,
        },
    };

    // The `--no-*` flags are negated: the `Config` field is the opposite of
    // whatever was passed.
    let hidden = explicit_flag(&matches, "no_hidden").map(|no_hidden| !no_hidden);
    let gitignore = explicit_flag(&matches, "no_gitignore").map(|no_gitignore| !no_gitignore);
    let ignore_files =
        explicit_flag(&matches, "no_ignore_files").map(|no_ignore_files| !no_ignore_files);
    let git_global = explicit_flag(&matches, "no_git_global").map(|no_git_global| !no_git_global);
    let follow_links = explicit_flag(&matches, "follow_links");
    let tree_only = explicit_flag(&matches, "tree_only");
    let human = explicit_flag(&matches, "human");

    Ok(PartialConfig {
        directory,
        output,
        include_dirs,
        exclude_dirs,
        include_ext,
        exclude_ext,
        include_files,
        exclude_files,
        min_size,
        max_size,
        hidden,
        gitignore,
        ignore_files,
        git_global,
        follow_links,
        tree_only,
        human,
    })
}

/// Returns `matches`' string value for `id`, but only if it was passed
/// explicitly on the command line — a `clap` `default_value` doesn't count.
fn explicit_string(matches: &clap::ArgMatches, id: &str) -> Option<String> {
    if matches.value_source(id) != Some(ValueSource::CommandLine) {
        return None;
    }
    matches.get_one::<String>(id).cloned()
}

/// Returns `matches`' `SetTrue` flag value for `id`, but only if it was
/// passed explicitly on the command line.
fn explicit_flag(matches: &clap::ArgMatches, id: &str) -> Option<bool> {
    if matches.value_source(id) != Some(ValueSource::CommandLine) {
        return None;
    }
    Some(matches.get_flag(id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{CommandFactory, FromArgMatches};

    /// Parses `argv` (with a fake `fyai` program name automatically
    /// prepended by the caller) through the real [`Cli`] struct.
    fn parse(argv: &[&str]) -> clap::error::Result<clap::ArgMatches> {
        Cli::command().try_get_matches_from(argv)
    }

    fn parse_ok(argv: &[&str]) -> clap::ArgMatches {
        parse(argv).unwrap_or_else(|e| panic!("expected parse to succeed: {e}"))
    }

    // ---- input / output ----------------------------------------------

    #[test]
    fn input_explicit_is_some() {
        let matches = parse_ok(&["fyai", "--input", "somedir"]);
        let config = config_from_matches(matches).unwrap();
        assert_eq!(config.directory, Some("somedir".to_string()));
    }

    #[test]
    fn input_short_flag_is_some() {
        let matches = parse_ok(&["fyai", "-i", "somedir"]);
        let config = config_from_matches(matches).unwrap();
        assert_eq!(config.directory, Some("somedir".to_string()));
    }

    #[test]
    fn input_not_passed_is_none() {
        let matches = parse_ok(&["fyai"]);
        let config = config_from_matches(matches).unwrap();
        assert_eq!(config.directory, None);
    }

    #[test]
    fn output_explicit_is_some() {
        let matches = parse_ok(&["fyai", "--output", "out.txt"]);
        let config = config_from_matches(matches).unwrap();
        assert_eq!(config.output, Some("out.txt".to_string()));
    }

    #[test]
    fn output_short_flag_is_some() {
        let matches = parse_ok(&["fyai", "-o", "out.txt"]);
        let config = config_from_matches(matches).unwrap();
        assert_eq!(config.output, Some("out.txt".to_string()));
    }

    #[test]
    fn output_not_passed_is_none() {
        let matches = parse_ok(&["fyai"]);
        let config = config_from_matches(matches).unwrap();
        assert_eq!(config.output, None);
    }

    // ---- comma-separated list options ---------------------------------

    #[test]
    fn include_dirs_parses_trims_lowercases_and_drops_empty() {
        let matches = parse_ok(&["fyai", "--include-dirs", "a,b, c ,,d"]);
        let config = config_from_matches(matches).unwrap();
        assert_eq!(
            config.include_dirs,
            Some(vec![
                "a".to_string(),
                "b".to_string(),
                "c".to_string(),
                "d".to_string(),
            ])
        );
    }

    #[test]
    fn include_dirs_mixed_case_and_blank_entries() {
        let matches = parse_ok(&["fyai", "--include-dirs", "Src, ,Test,"]);
        let config = config_from_matches(matches).unwrap();
        assert_eq!(
            config.include_dirs,
            Some(vec!["src".to_string(), "test".to_string()])
        );
    }

    #[test]
    fn include_dirs_not_passed_is_none() {
        let matches = parse_ok(&["fyai"]);
        let config = config_from_matches(matches).unwrap();
        assert_eq!(config.include_dirs, None);
    }

    #[test]
    fn exclude_dirs_parses_and_defaults_to_none() {
        let matches = parse_ok(&["fyai", "--exclude-dirs", "node_modules, TARGET ,,"]);
        let config = config_from_matches(matches).unwrap();
        assert_eq!(
            config.exclude_dirs,
            Some(vec!["node_modules".to_string(), "target".to_string()])
        );

        let matches = parse_ok(&["fyai"]);
        let config = config_from_matches(matches).unwrap();
        assert_eq!(config.exclude_dirs, None);
    }

    #[test]
    fn include_ext_parses_and_defaults_to_none() {
        let matches = parse_ok(&["fyai", "--include-ext", ".JSON, .toml ,,"]);
        let config = config_from_matches(matches).unwrap();
        assert_eq!(
            config.include_ext,
            Some(vec![".json".to_string(), ".toml".to_string()])
        );

        let matches = parse_ok(&["fyai"]);
        let config = config_from_matches(matches).unwrap();
        assert_eq!(config.include_ext, None);
    }

    #[test]
    fn exclude_ext_parses_and_defaults_to_none() {
        let matches = parse_ok(&["fyai", "--exclude-ext", ".LOCK, .bak ,,"]);
        let config = config_from_matches(matches).unwrap();
        assert_eq!(
            config.exclude_ext,
            Some(vec![".lock".to_string(), ".bak".to_string()])
        );

        let matches = parse_ok(&["fyai"]);
        let config = config_from_matches(matches).unwrap();
        assert_eq!(config.exclude_ext, None);
    }

    #[test]
    fn include_files_parses_and_defaults_to_none() {
        let matches = parse_ok(&["fyai", "--include-files", "README.md, Main.rs ,,"]);
        let config = config_from_matches(matches).unwrap();
        assert_eq!(
            config.include_files,
            Some(vec!["readme.md".to_string(), "main.rs".to_string()])
        );

        let matches = parse_ok(&["fyai"]);
        let config = config_from_matches(matches).unwrap();
        assert_eq!(config.include_files, None);
    }

    #[test]
    fn exclude_files_parses_and_defaults_to_none() {
        let matches = parse_ok(&["fyai", "--exclude-files", "LICENSE, Config.json ,,"]);
        let config = config_from_matches(matches).unwrap();
        assert_eq!(
            config.exclude_files,
            Some(vec!["license".to_string(), "config.json".to_string()])
        );

        let matches = parse_ok(&["fyai"]);
        let config = config_from_matches(matches).unwrap();
        assert_eq!(config.exclude_files, None);
    }

    // ---- min-size / max-size ------------------------------------------

    #[test]
    fn min_size_explicit_is_some() {
        let matches = parse_ok(&["fyai", "--min-size", "1024"]);
        let config = config_from_matches(matches).unwrap();
        assert_eq!(config.min_size, Some(1024));
    }

    #[test]
    fn min_size_short_flag_is_some() {
        let matches = parse_ok(&["fyai", "-n", "512"]);
        let config = config_from_matches(matches).unwrap();
        assert_eq!(config.min_size, Some(512));
    }

    #[test]
    fn min_size_not_passed_is_none() {
        let matches = parse_ok(&["fyai"]);
        let config = config_from_matches(matches).unwrap();
        assert_eq!(config.min_size, None);
    }

    #[test]
    fn max_size_explicit_is_some() {
        let matches = parse_ok(&["fyai", "--max-size", "2048"]);
        let config = config_from_matches(matches).unwrap();
        assert_eq!(config.max_size, Some(2048));
    }

    #[test]
    fn max_size_short_flag_is_some() {
        let matches = parse_ok(&["fyai", "-m", "4096"]);
        let config = config_from_matches(matches).unwrap();
        assert_eq!(config.max_size, Some(4096));
    }

    #[test]
    fn max_size_not_passed_is_none() {
        let matches = parse_ok(&["fyai"]);
        let config = config_from_matches(matches).unwrap();
        assert_eq!(config.max_size, None);
    }

    #[test]
    fn min_size_invalid_value_is_rejected_by_clap() {
        // clap validates the `u64` value type itself before
        // `config_from_matches` ever runs, so a non-numeric value never
        // reaches the string-parse fallback branch in `config_from_matches`.
        let result = parse(&["fyai", "--min-size", "not-a-number"]);
        assert!(result.is_err());
    }

    // ---- negated boolean flags -----------------------------------------

    #[test]
    fn no_hidden_flag_negates_to_some_false() {
        let matches = parse_ok(&["fyai", "--no-hidden"]);
        let config = config_from_matches(matches).unwrap();
        assert_eq!(config.hidden, Some(false));
    }

    #[test]
    fn no_hidden_not_passed_is_none() {
        let matches = parse_ok(&["fyai"]);
        let config = config_from_matches(matches).unwrap();
        assert_eq!(config.hidden, None);
    }

    #[test]
    fn no_gitignore_flag_negates_to_some_false() {
        let matches = parse_ok(&["fyai", "--no-gitignore"]);
        let config = config_from_matches(matches).unwrap();
        assert_eq!(config.gitignore, Some(false));
    }

    #[test]
    fn no_gitignore_not_passed_is_none() {
        let matches = parse_ok(&["fyai"]);
        let config = config_from_matches(matches).unwrap();
        assert_eq!(config.gitignore, None);
    }

    #[test]
    fn no_ignore_files_flag_negates_to_some_false() {
        let matches = parse_ok(&["fyai", "--no-ignore-files"]);
        let config = config_from_matches(matches).unwrap();
        assert_eq!(config.ignore_files, Some(false));
    }

    #[test]
    fn no_ignore_files_not_passed_is_none() {
        let matches = parse_ok(&["fyai"]);
        let config = config_from_matches(matches).unwrap();
        assert_eq!(config.ignore_files, None);
    }

    #[test]
    fn no_git_global_flag_negates_to_some_false() {
        let matches = parse_ok(&["fyai", "--no-git-global"]);
        let config = config_from_matches(matches).unwrap();
        assert_eq!(config.git_global, Some(false));
    }

    #[test]
    fn no_git_global_not_passed_is_none() {
        let matches = parse_ok(&["fyai"]);
        let config = config_from_matches(matches).unwrap();
        assert_eq!(config.git_global, None);
    }

    // ---- direct (non-negated) boolean flags -----------------------------

    #[test]
    fn follow_links_flag_is_some_true() {
        let matches = parse_ok(&["fyai", "--follow-links"]);
        let config = config_from_matches(matches).unwrap();
        assert_eq!(config.follow_links, Some(true));
    }

    #[test]
    fn follow_links_not_passed_is_none() {
        let matches = parse_ok(&["fyai"]);
        let config = config_from_matches(matches).unwrap();
        assert_eq!(config.follow_links, None);
    }

    #[test]
    fn tree_only_flag_is_some_true() {
        let matches = parse_ok(&["fyai", "--tree-only"]);
        let config = config_from_matches(matches).unwrap();
        assert_eq!(config.tree_only, Some(true));
    }

    #[test]
    fn tree_only_not_passed_is_none() {
        let matches = parse_ok(&["fyai"]);
        let config = config_from_matches(matches).unwrap();
        assert_eq!(config.tree_only, None);
    }

    #[test]
    fn human_flag_is_some_true() {
        let matches = parse_ok(&["fyai", "--human"]);
        let config = config_from_matches(matches).unwrap();
        assert_eq!(config.human, Some(true));
    }

    #[test]
    fn human_not_passed_is_none() {
        let matches = parse_ok(&["fyai"]);
        let config = config_from_matches(matches).unwrap();
        assert_eq!(config.human, None);
    }

    // ---- --repo conflicts / requires ------------------------------------

    #[test]
    fn repo_conflicts_with_input() {
        let result = parse(&[
            "fyai",
            "--repo",
            "https://example.com/x.git",
            "--input",
            "y",
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn repo_alone_parses_fine() {
        let matches = parse_ok(&["fyai", "--repo", "https://example.com/x.git"]);
        let cli = Cli::from_arg_matches(&matches).unwrap();
        assert_eq!(cli.repo, Some("https://example.com/x.git".to_string()));
    }

    #[test]
    fn repo_branch_requires_repo() {
        let result = parse(&["fyai", "--repo-branch", "main"]);
        assert!(result.is_err());
    }

    #[test]
    fn repo_branch_with_repo_parses_fine() {
        let matches = parse_ok(&[
            "fyai",
            "--repo",
            "https://example.com/x.git",
            "--repo-branch",
            "main",
        ]);
        let cli = Cli::from_arg_matches(&matches).unwrap();
        assert_eq!(cli.repo_branch, Some("main".to_string()));
    }

    #[test]
    fn repo_commit_requires_repo() {
        let result = parse(&["fyai", "--repo-commit", "deadbeef"]);
        assert!(result.is_err());
    }

    #[test]
    fn repo_commit_with_repo_parses_fine() {
        let matches = parse_ok(&[
            "fyai",
            "--repo",
            "https://example.com/x.git",
            "--repo-commit",
            "deadbeef",
        ]);
        let cli = Cli::from_arg_matches(&matches).unwrap();
        assert_eq!(cli.repo_commit, Some("deadbeef".to_string()));
    }

    // ---- init subcommand -------------------------------------------------

    #[test]
    fn init_subcommand_defaults_false() {
        let matches = parse_ok(&["fyai", "init"]);
        let cli = Cli::from_arg_matches(&matches).unwrap();
        match cli.command {
            Some(Command::Init { global, force }) => {
                assert!(!global);
                assert!(!force);
            }
            other => panic!("expected Some(Command::Init {{ .. }}), got {other:?}"),
        }
    }

    #[test]
    fn init_subcommand_with_flags() {
        let matches = parse_ok(&["fyai", "init", "--global", "--force"]);
        let cli = Cli::from_arg_matches(&matches).unwrap();
        match cli.command {
            Some(Command::Init { global, force }) => {
                assert!(global);
                assert!(force);
            }
            other => panic!("expected Some(Command::Init {{ .. }}), got {other:?}"),
        }
    }

    #[test]
    fn no_subcommand_is_none() {
        let matches = parse_ok(&["fyai"]);
        let cli = Cli::from_arg_matches(&matches).unwrap();
        assert!(cli.command.is_none());
    }

    // ---- test flag ---------------------------------------------------------

    #[test]
    fn test_flag_true_when_passed() {
        let matches = parse_ok(&["fyai", "-t"]);
        let cli = Cli::from_arg_matches(&matches).unwrap();
        assert!(cli.test);

        let matches = parse_ok(&["fyai", "--test"]);
        let cli = Cli::from_arg_matches(&matches).unwrap();
        assert!(cli.test);
    }

    #[test]
    fn test_flag_false_when_not_passed() {
        let matches = parse_ok(&["fyai"]);
        let cli = Cli::from_arg_matches(&matches).unwrap();
        assert!(!cli.test);
    }

    // ---- explicit_string / explicit_flag helpers (direct) -----------------

    #[test]
    fn explicit_string_ignores_default_value() {
        // `--input` has a clap `default_value`, so when omitted, `get_one`
        // would return `Some(".")`, but `explicit_string` must still report
        // `None` because it wasn't passed on the command line.
        let matches = parse_ok(&["fyai"]);
        assert_eq!(explicit_string(&matches, "input"), None);
        assert_eq!(
            matches.get_one::<String>("input").map(String::as_str),
            Some(".")
        );
    }

    #[test]
    fn explicit_string_returns_value_when_passed() {
        let matches = parse_ok(&["fyai", "--input", "somedir"]);
        assert_eq!(
            explicit_string(&matches, "input"),
            Some("somedir".to_string())
        );
    }

    #[test]
    fn explicit_flag_none_when_not_passed() {
        let matches = parse_ok(&["fyai"]);
        assert_eq!(explicit_flag(&matches, "no_hidden"), None);
    }

    #[test]
    fn explicit_flag_some_true_when_passed() {
        let matches = parse_ok(&["fyai", "--no-hidden"]);
        assert_eq!(explicit_flag(&matches, "no_hidden"), Some(true));
    }

    // ---- full end-to-end sanity check -------------------------------------

    #[test]
    fn all_fields_together() {
        let matches = parse_ok(&[
            "fyai",
            "--input",
            "src",
            "--output",
            "out.txt",
            "--include-dirs",
            "a,b",
            "--exclude-dirs",
            "c,d",
            "--include-ext",
            ".rs,.toml",
            "--exclude-ext",
            ".lock",
            "--include-files",
            "main.rs",
            "--exclude-files",
            "LICENSE",
            "--min-size",
            "10",
            "--max-size",
            "20",
            "--no-hidden",
            "--no-gitignore",
            "--no-ignore-files",
            "--no-git-global",
            "--follow-links",
            "--tree-only",
            "--human",
            "-t",
        ]);
        let config = config_from_matches(matches).unwrap();

        assert_eq!(config.directory, Some("src".to_string()));
        assert_eq!(config.output, Some("out.txt".to_string()));
        assert_eq!(
            config.include_dirs,
            Some(vec!["a".to_string(), "b".to_string()])
        );
        assert_eq!(
            config.exclude_dirs,
            Some(vec!["c".to_string(), "d".to_string()])
        );
        assert_eq!(
            config.include_ext,
            Some(vec![".rs".to_string(), ".toml".to_string()])
        );
        assert_eq!(config.exclude_ext, Some(vec![".lock".to_string()]));
        assert_eq!(config.include_files, Some(vec!["main.rs".to_string()]));
        assert_eq!(config.exclude_files, Some(vec!["license".to_string()]));
        assert_eq!(config.min_size, Some(10));
        assert_eq!(config.max_size, Some(20));
        assert_eq!(config.hidden, Some(false));
        assert_eq!(config.gitignore, Some(false));
        assert_eq!(config.ignore_files, Some(false));
        assert_eq!(config.git_global, Some(false));
        assert_eq!(config.follow_links, Some(true));
        assert_eq!(config.tree_only, Some(true));
        assert_eq!(config.human, Some(true));
    }

    #[test]
    fn no_flags_all_none() {
        let matches = parse_ok(&["fyai"]);
        let config = config_from_matches(matches).unwrap();

        assert_eq!(config.directory, None);
        assert_eq!(config.output, None);
        assert_eq!(config.include_dirs, None);
        assert_eq!(config.exclude_dirs, None);
        assert_eq!(config.include_ext, None);
        assert_eq!(config.exclude_ext, None);
        assert_eq!(config.include_files, None);
        assert_eq!(config.exclude_files, None);
        assert_eq!(config.min_size, None);
        assert_eq!(config.max_size, None);
        assert_eq!(config.hidden, None);
        assert_eq!(config.gitignore, None);
        assert_eq!(config.ignore_files, None);
        assert_eq!(config.git_global, None);
        assert_eq!(config.follow_links, None);
        assert_eq!(config.tree_only, None);
        assert_eq!(config.human, None);
    }
}
