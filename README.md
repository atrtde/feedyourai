# feedyourai

A command-line tool to combine files from a directory into a single file for LLM processing, with flexible filtering options.

![Demo: fyai combining files in a terminal](https://raw.githubusercontent.com/alexandretrotel/feedyourai/main/assets/fyai.gif)

## Features

- Combines multiple text files into one output file
- Can process a remote git repository in a temporary directory
- Supports configuration via CLI options and config files (TOML)
- Filters files by:
  - Size
  - File extensions (e.g., `.txt`, `.md`)
  - Directory inclusion/exclusion
  - File inclusion/exclusion
  - Independently controllable walk rules, each on by default (except symlink-following): hidden files/directories (`--no-hidden`), `.gitignore`/`.git/info/exclude`/parent `.gitignore` (`--no-gitignore`), plain `.ignore` files (`--no-ignore-files`), git's global excludes file (`--no-git-global`), and symlink traversal (`--follow-links`)
  - Always respects a `.fyaiignore` file (gitignore syntax), regardless of the walk rules above
- Preserves file boundaries with headers showing filename and size
- Customizable input directory and output file

## CLI Behavior

- `-q`/`--quiet` suppresses non-essential status output (config-loaded notice, size breakdown, success/clipboard messages); errors and warnings always still print to stderr.
- `--json` prints a single-line JSON run summary to stdout instead of human-readable text, for scripting.
- `--no-color` disables colored error/panic output; also honored via the `NO_COLOR` environment variable or `TERM=dumb`.
- A progress spinner is shown during `--repo` clones and local scans when stdout is a real terminal and neither `--quiet` nor `--json` was passed.
- Overwriting an existing output file prompts for confirmation interactively; pass `-f`/`--force` to skip the prompt, which is required in non-interactive contexts (scripts, CI).
- Ctrl-C cleans up an in-progress `--repo` clone's temporary directory before exiting.
- `fyai man` prints a roff-formatted man page to stdout, e.g. `fyai man > /usr/local/share/man/man1/fyai.1`.

## Installation

### Install via Cargo

```bash
cargo install feedyourai
```

Or,

```bash
cargo install --git https://github.com/alexandretrotel/feedyourai.git
```

This installs the `feedyourai` binary (and its `fyai` alias) to `~/.cargo/bin/`. Ensure this directory is in your `PATH`.

## Configuration

### Config File

You can specify options in a config file (TOML format):

- **Local config:** `./fyai.toml` (used if present in current directory)
- **Global config:** System config directory, used if no local config found — `$XDG_CONFIG_HOME` if set to an absolute path (any platform), otherwise the platform default (e.g. `~/.config` on Linux, `~/Library/Application Support` on macOS)
- **Precedence:** CLI flags > `FYAI_*` environment variables > local config > global config > built-in defaults.

To see the exact global config path on your system, run:

```bash
fyai init --global
```

### Environment Variables

Every config option can also be set via a `FYAI_*` environment variable, taking precedence over `fyai.toml` but not over CLI flags:

| Variable | Corresponds to |
| --- | --- |
| `FYAI_DIRECTORY` | `-i`/`--input` |
| `FYAI_OUTPUT` | `-o`/`--output` |
| `FYAI_INCLUDE_DIRS` | `--include-dirs` |
| `FYAI_EXCLUDE_DIRS` | `--exclude-dirs` |
| `FYAI_INCLUDE_EXT` | `--include-ext` |
| `FYAI_EXCLUDE_EXT` | `--exclude-ext` |
| `FYAI_INCLUDE_FILES` | `--include-files` |
| `FYAI_EXCLUDE_FILES` | `--exclude-files` |
| `FYAI_MIN_SIZE` | `-n`/`--min-size` |
| `FYAI_MAX_SIZE` | `-m`/`--max-size` |
| `FYAI_HIDDEN` | `--no-hidden` (inverse; `true`/`false`) |
| `FYAI_GITIGNORE` | `--no-gitignore` (inverse; `true`/`false`) |
| `FYAI_IGNORE_FILES` | `--no-ignore-files` (inverse; `true`/`false`) |
| `FYAI_GIT_GLOBAL` | `--no-git-global` (inverse; `true`/`false`) |
| `FYAI_FOLLOW_LINKS` | `--follow-links` (`true`/`false`) |
| `FYAI_TREE_ONLY` | `--tree-only` (`true`/`false`) |
| `FYAI_HUMAN` | `--human` (`true`/`false`) |

List-valued variables use the same comma-separated format as their CLI counterparts (e.g. `FYAI_INCLUDE_EXT=rs,toml`).

#### Example `fyai.toml`

```toml
directory = "./src"
output = "combined.txt"
include_ext = ["md", "txt"]
exclude_dirs = ["node_modules", "dist"]
min_size = 10240
max_size = 512000
hidden = true
gitignore = true
ignore_files = true
git_global = true
follow_links = false
tree_only = false
human = false
```

All CLI options can be set in the config file. CLI flags always take precedence.

### Path Exclusion via `.fyaiignore`

Drop a `.fyaiignore` file (gitignore syntax) anywhere under the scanned directory to exclude matching paths, as an alternative or complement to `exclude_dirs`/`exclude_files`. Unlike `.gitignore`, it's always respected — none of the walk-rule flags (`--no-hidden`, `--no-gitignore`, `--no-ignore-files`, `--no-git-global`, `--follow-links`) affect it, since it's fyai's own dedicated exclude mechanism rather than a git one.

## Usage

### Basic Usage

```bash
fyai            # combine everything in the current directory into fyai.txt
fyai --help     # show all options
```

### Examples

| Goal                                    | Command                                                             |
| ---------------------------------------- | -------------------------------------------------------------------- |
| Only `.txt`/`.md` files, from `./docs`   | `fyai -i ./docs --include-ext txt,md`                                 |
| Exclude `.log`/`.tmp` files              | `fyai --exclude-ext log,tmp`                                          |
| Only specific files, from specific dirs  | `fyai --include-dirs src,docs --include-files README.md,main.rs`      |
| Exclude specific files everywhere        | `fyai --exclude-files LICENSE,config.json`                            |
| Size window: 10KB–500KB, custom output   | `fyai -n 10240 -m 512000 -o ai_input.txt -x dist,node_modules`         |
| Tree only, no file contents              | `fyai --tree-only -o tree.txt`                                        |
| Tree with `tree`-style connector glyphs  | `fyai --tree-only --human -o tree.txt`                                |
| Include hidden files, still respect `.gitignore` | `fyai --no-hidden`                                              |
| Include `.gitignore`-excluded files, still skip hidden files | `fyai --no-gitignore`                                      |
| Include everything (hidden + gitignored + `.ignore`d + globally-excluded) | `fyai --no-hidden --no-gitignore --no-ignore-files --no-git-global` |
| Follow symlinks while walking             | `fyai --follow-links`                                                 |
| Remote repo, specific branch             | `fyai --repo https://github.com/owner/repo.git --repo-branch main`    |
| Remote repo, specific commit             | `fyai --repo https://github.com/owner/repo.git --repo-commit 1234abcd` |
| Generate a config template               | `fyai init`                                                            |
| Suppress status output                   | `fyai -q`                                                              |
| Machine-readable run summary             | `fyai --json`                                                         |
| Disable colored error output             | `fyai --no-color`                                                     |
| Overwrite an existing output without prompting | `fyai -o out.txt --force`                                       |
| Print a roff man page                    | `fyai man > /usr/local/share/man/man1/fyai.1`                         |

## Output Format

Every run starts with a `- Tree Structure` section. By default it uses a minimal two-space indent:

```
- Tree Structure

src/
  main.rs
  utils/
    helper.rs
```

Pass `--human` (or `human = true` in `fyai.toml`) for `tree`-style connector glyphs instead:

```
- Tree Structure

src
├── main.rs
└── utils/
    └── helper.rs
```

Each source file follows as a heading plus a language-tagged, fenced code block, with a human-readable size:

````
### src/main.rs (1.2 KB)

```rust
fn main() {}
```

### notes.md (66.3 KB)

```markdown
[contents of notes.md]
```
````

The fence widens to four backticks for any file whose own content contains a triple backtick, so the block's end is never ambiguous.

## Performance

The directory is walked once, in parallel, and every file is read and UTF-8-checked in parallel too; output is written through a single buffered writer. Nothing to configure — it's just how `fyai` scans.

## License

GPL-3.0 or later. See [LICENSE](LICENSE) for more details.
