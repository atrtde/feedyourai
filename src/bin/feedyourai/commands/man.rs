//! Implementation of the `man` subcommand.

use clap::CommandFactory;
use color_eyre::eyre::{Result, WrapErr};

use super::{Cli, Command};

/// If `cli` carries a `man` subcommand, writes a roff-formatted man page to
/// stdout and returns `Ok(true)`; otherwise returns `Ok(false)` so the
/// caller proceeds with a normal combine run.
pub fn handle_man_subcommand(cli: &Cli) -> Result<bool> {
    if let Some(Command::Man) = &cli.command {
        clap_mangen::Man::new(Cli::command())
            .render(&mut std::io::stdout())
            .wrap_err("failed to render man page")?;
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
        let result = handle_man_subcommand(&cli).expect("should not error");
        assert!(!result);
    }

    #[test]
    fn man_subcommand_returns_true() {
        let cli = cli_from_argv(&["fyai", "man"]);
        let result = handle_man_subcommand(&cli).expect("should succeed");
        assert!(result);
    }
}
