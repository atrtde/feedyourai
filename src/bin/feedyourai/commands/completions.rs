//! Implementation of the `completions` subcommand.

use clap::CommandFactory;
use color_eyre::eyre::Result;

use super::{Cli, Command};

/// If `cli` carries a `completions` subcommand, writes a shell completion
/// script for the requested shell to stdout and returns `Ok(true)`;
/// otherwise returns `Ok(false)` so the caller proceeds with a normal
/// combine run.
pub fn handle_completions_subcommand(cli: &Cli) -> Result<bool> {
    if let Some(Command::Completions { shell }) = &cli.command {
        let mut cmd = Cli::command();
        let name = cmd.get_name().to_string();
        clap_complete::generate(*shell, &mut cmd, name, &mut std::io::stdout());
        return Ok(true);
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::FromArgMatches;

    fn cli_from_argv(args: &[&str]) -> Cli {
        let matches = Cli::command()
            .try_get_matches_from(args)
            .expect("should parse");
        Cli::from_arg_matches(&matches).expect("should convert")
    }

    #[test]
    fn returns_false_when_no_subcommand() {
        let cli = cli_from_argv(&["fyai"]);
        let result = handle_completions_subcommand(&cli).expect("should not error");
        assert!(!result);
    }

    #[test]
    fn completions_subcommand_returns_true() {
        let cli = cli_from_argv(&["fyai", "completions", "bash"]);
        let result = handle_completions_subcommand(&cli).expect("should succeed");
        assert!(result);
    }
}
