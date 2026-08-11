# Changelog

All notable changes to this project will be documented in this file.

## 2026-08-11 - 3.3.0

CLI improvements against the [clig.dev](https://clig.dev/) command-line interface guidelines.

Added
- `-q`/`--quiet` flag to suppress non-essential status output (config-loaded notice, size breakdown, success/clipboard messages); errors and warnings still print to stderr.
- `--json` flag to print a single-line JSON run summary (output path, size breakdown, clipboard outcome) to stdout instead of human-readable text.
- `--no-color` flag to disable colored error/panic output; also honored via `NO_COLOR` and `TERM=dumb`.
- A progress spinner during `--repo` clones and local scans, shown when stdout is a real terminal and neither `--quiet` nor `--json` was passed.
- `FYAI_*` environment variables (`FYAI_OUTPUT`, `FYAI_MIN_SIZE`, `FYAI_HIDDEN`, ...) as a config layer between CLI flags and `fyai.toml`.
- Ctrl-C now cleans up an in-progress `--repo` clone's temporary directory before exiting, instead of leaving it behind.
- `man` subcommand: prints a roff-formatted man page to stdout, e.g. `fyai man > /usr/local/share/man/man1/fyai.1`.
- `-f`/`--force` flag; without it, overwriting an existing output file prompts interactively, and fails with guidance in non-interactive contexts.

Changed
- `-h`/`--help` now leads with a short description, one example, and a support/issues link; `--help` carries the full reference with more examples.
- **Breaking:** `config::merge_config` now takes three `PartialConfig`s (`file`, `env`, `cli`) instead of two, to fold in the new `FYAI_*` environment-variable layer while staying a pure function.

## 2026-08-07 - 3.2.0

Added
- The full-combine run now breaks down `Total size walked` into `Non-binary (written)` and `Binary (skipped)` size, plus a `Skipped by size filter` line when `min_size`/`max_size` excluded anything, so it's clear how much of the walked size actually made it into the output.
- `scan`/`run_local`/`run_git` now return a `ScanStats` struct (`total_size`, `written_size`, `binary_size`, plus a `size_filtered()` helper) instead of a bare `u64`.

Changed
- **Breaking:** `run_local`/`run_git` now return `Result<ScanStats>` instead of `Result<u64>`.

## 2026-08-07 - 3.1.0

Added
- `scan`/`run_local`/`run_git` now return the total size in bytes of every file the walk collected, printed by the `fyai`/`feedyourai` binaries as `Total size walked: <human size>` once a run finishes (both the tree-only and full-combine paths).
- `-c`/`--clipboard` flag to explicitly opt into copying the combined output to the system clipboard.

Changed
- **Breaking:** The combined output is no longer copied to the clipboard by default; pass `-c`/`--clipboard` to opt in.
- `run_local`/`run_git` now return `Result<u64>` (the total walked size) instead of `Result<()>`.

## 2026-08-07 - 3.0.0

Added
- `fyai` as a second binary target, aliasing `feedyourai` (same CLI, no behavior difference).
- `error` module with a `FyaiError` type (via `thiserror`), replacing `color-eyre` in the library crate.
- `--human` flag (and `human` config-file key) to render the directory tree with `tree`-style connector glyphs (`├──`, `└──`, `│`) instead of the default minimal two-space indent.
- `system_config_dir` now honors `$XDG_CONFIG_HOME` (when set to an absolute path) on every platform, not just Linux, before falling back to the platform default.
- `.fyaiignore` support: a gitignore-syntax file, honored in any directory it appears in, for path-based exclusion as an alternative to `exclude_dirs`/`exclude_files`. Unlike `.gitignore`/`.ignore`, it's *always* respected, even with `--no-gitignore`.

Changed
- **Breaking:** CLI wiring (`cli`, `init`, clipboard) moved out of the library into the `fyai`/`feedyourai` binaries; the library no longer prints to stdout/stderr or copies to the clipboard, and no longer depends on `color-eyre`.
- **Breaking:** Config file format switched from YAML to TOML: `fyai.yaml` → `fyai.toml`, `serde_yaml`/`yaml_serde` dependency replaced with `toml`.
- Replaced `directories-next` with `dirs` for locating system config directories.
- `documentation` field in `Cargo.toml` now points to `docs.rs` instead of the GitHub repo.
- README demo GIF now uses a raw GitHub link so it renders on crates.io.
- CI split into `ci.yml` (fmt, clippy, machete, test), `build-binaries.yml`, and `release.yml`, each triggered on `push` to `main` in addition to pull requests.
- `release.yml` now uploads both `fyai` and `feedyourai` binaries per target.
- `get_directory_tree` now dispatches between two renderers based on `config.human`: a minimal two-space indent by default, or `tree`-style ASCII connectors when set.
- **Breaking:** Per-file output format changed from `- File: name (size bytes)` + raw content to a `### relative/path (human size)` heading followed by a language-tagged, fenced code block (language inferred from extension via the new `scanner::lang` module). The fence widens from ```` ``` ```` to ```` ```` ```` when the file's own content contains a triple backtick, so the block's end is never ambiguous — the old format had no closing delimiter at all.
- **Breaking:** `merge_config` now takes two `PartialConfig`s (`file`, `cli`) instead of a `PartialConfig` plus a fully-resolved `Config` and a separate `ExplicitFlags`. CLI parsing (`config_from_matches`) now returns a `PartialConfig` directly, leaving a field `None` unless it was explicitly passed, instead of always resolving to a concrete value and tracking "was this explicit" on the side.
- **Breaking:** `get_directory_tree`/`process_files`/`build_walker`/`PathFilter::new` no longer take an `ignored_dirs` parameter, since there's no longer a hardcoded default-ignore list — see Removed.
- **Breaking:** `respect_gitignore` (config-file key) and its single `--no-gitignore` switch are gone, replaced by five independently controllable `ignore`-crate knobs: `hidden` (dot-files), `gitignore` (`.gitignore`/`.git/info/exclude`/parent `.gitignore`, kept as one group), `ignore_files` (plain `.ignore`), `git_global` (git's global excludes file), and `follow_links` (symlink traversal, not previously exposed at all) — each defaulting to `true` except `follow_links` (`false`), matching `ignore`'s own defaults. CLI gains `--no-hidden`, `--no-ignore-files`, `--no-git-global`, and `--follow-links`; `--no-gitignore` is kept but now only ever affects the `.gitignore` group (it used to also disable hidden-file filtering). `build_walker` now sets each `ignore::WalkBuilder` filter from its own `Config` field instead of the previous all-or-nothing `standard_filters(true)` + conditional bulk-disable.
- Wording: replaced "AI" with "LLM" throughout descriptions and doc comments (binary names `feedyourai`/`fyai` unchanged).
- `fyai`/`feedyourai`'s CLI module moved from `cli.rs` + `cli/init.rs` to `commands/mod.rs` + `commands/init.rs` (the directory-only module layout).
- Library restructuring: `src/commands.rs` renamed to `src/runner.rs` (its `run` function renamed `run_local`); `src/git.rs` merged into it (`run` renamed `run_git`), so both entry points live in one module instead of two files that each exported a function ambiguously named `run`. `lib.rs` now just re-exports `runner::{run_local, run_git}` instead of wrapping them.

Performance
- The directory is now walked exactly once, in parallel (`ignore::WalkParallel`), instead of twice sequentially (once for the tree, once for file contents) — `get_directory_tree`/`process_files` were merged into a single `scanner::scan`.
- The run's own output file is no longer detected by `canonicalize`-ing every walked path (a full symlink-resolving `stat` chain per entry); a cheap file-name comparison now gates a `same-file`-based identity check, so almost every entry costs zero extra syscalls.
- File reads and UTF-8 validation now run in parallel across files (`rayon`), with SIMD-accelerated validation (`simdutf8`) instead of `std`'s scalar `from_utf8`.
- Output is written through a single buffered writer instead of many small unbuffered writes.
- New dependencies: `rayon`, `same-file`, `simdutf8`.

Fixed
- `--repo`'s `conflicts_with` referenced a nonexistent `directory` arg id (the actual id is `input`), which made every CLI invocation panic at startup.
- `--no-gitignore` detection looked up the wrong arg id (`respect_gitignore`, which doesn't exist) and tried to parse it as a string; the flag was never actually read. Now correctly negates the `no_gitignore` `SetTrue` flag.
- CLI `--help` and `init --global`'s help text hardcoded `~/.config/fyai.yaml` as the global config path, which is wrong on macOS/Windows and ignores `$XDG_CONFIG_HOME`. Now describes the actual resolution and points to `fyai init --global` for the exact path.

Removed
- `IGNORED_FILES` and `IGNORED_DIRS` constants (and the `constants` module entirely) — no more hardcoded default-ignore list. Exclusion is now fully explicit via `exclude_dirs`, `exclude_files`, `.gitignore`/`.ignore`, or the new `.fyaiignore`.
- `ExplicitFlags` struct — superseded by `PartialConfig`-based CLI parsing (see Changed).

Added (crate metadata)
- `keywords`, `categories`, and `readme` fields for crates.io discoverability.

## 2.1.3 - 2026-07-31

Changed
- The published crate now uses an explicit `include` allowlist instead of an `exclude` denylist, so only `src/`, `Cargo.toml`, `README.md`, `LICENSE`, and `CHANGELOG.md` are shipped. `.gitignore` and any future non-source files no longer end up in the package.

## 2.1.0 - 2026-07-18

Changed
- Switched error handling across the crate to `eyre` (with `color-eyre` reporting in the `fyai` binary), using native `eyre` macros (`eyre!`, `bail!`, `wrap_err`, `ok_or_eyre`).
- `run_local` and `run_git` now return `eyre::Result<()>`.

Removed
- Removed the `thiserror` dependency and the `errors` module (`AppError` / `AppResult`).

## 2.0.3 - 2026-05-13

Changed
- Removed unused `anyhow` dependency.

## 2.0.2 - 2026-02-18

Changed
- Split CLI-specific logic into a `cli` module folder.
- Exposed a library API with `run_local` and `run_git`, keeping lower-level modules public.
- Removed `clap` usage from the library crate.

## 2.0.1 - 2026-02-17

Fixed
- Added a tree structure header to the output.
- Use `--input` instead of `--directory` in CLI handling.

Changed
- README updates and corrections.

## 2.0.0 - 2026-02-18

Changed
- Switched traversal to `ignore::WalkBuilder` with standard ignore filters, which now honors `.gitignore`, `.git/info/exclude`, global gitignore, `.ignore`, and hidden files by default.
- Replaced `dirs` with `directories-next` for locating system config directories.

## 1.7.2 - 2026-02-09

Changed
- Replaced `clipboard` with `arboard` to avoid the `xcb` dependency and CMake build step.

## 1.7.1 - 2026-02-09

Changed
- Updated dependencies: thiserror 1.0 -> 2.0.18.

## 1.7.0 - 2026-02-09

Added
- `--repo` to process a remote git repository in a temporary directory.
- `--repo-branch` to checkout a branch or tag when using `--repo`.
- `--repo-commit` to checkout a specific commit when using `--repo`.
- Repository integration tests covering clone, cleanup, and commit checkout.

Changed
- Error handling now uses typed `AppError` variants (via `thiserror`), removing string-based checks for clipboard and config errors.
- When `--repo-commit` is used, cloning no longer uses `--depth 1` to ensure the commit is available.
- Documentation updated with a remote-repo usage example.
- Applied a formatting pass to keep code style consistent.

Fixed
- CLI now prevents using `--repo` and `--dir` together.
