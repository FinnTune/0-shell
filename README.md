# 0-shell

[![CI](https://github.com/FinnTune/0-shell/actions/workflows/ci.yml/badge.svg)](https://github.com/FinnTune/0-shell/actions/workflows/ci.yml)

A minimal Unix-like shell written in Rust, reimplementing a handful of
standard built-in commands from scratch (no calls out to `/bin/ls`,
`/bin/rm`, etc.) on top of `std::fs` and raw `libc` calls for things Rust's
standard library doesn't expose, like looking up usernames/group names by
uid/gid.

## Supported commands

| Command | Notes |
| --- | --- |
| `cd [dir\|-]` | No argument goes to `$HOME`; `-` goes to the previous directory (and prints it, like bash). |
| `pwd` | |
| `echo [args...]` | |
| `cat [file...]` | With no file arguments, reads piped-in input if there is any. |
| `ls [-l] [-a] [-F] [-R] [path...]` | `-l` long format, `-a` show dotfiles, `-F` classify (`/` dir, `*` executable, `@` symlink, `\|` FIFO, `=` socket), `-R` recurse into subdirectories. Defaults to `.` when no path is given; multiple directory arguments (or `-R`) get `path:` headers. |
| `mkdir [-p] dir...` | `-p` creates missing parent directories and doesn't error if the target already exists. |
| `rm [-r] file...` | `-r` required to remove directories. |
| `cp src... dst` | With more than one source, `dst` must be an existing directory. A single source can go to an exact destination path. Errors if a source is a directory. |
| `mv src... dst` | Same multi-source rule as `cp`, but sources may be files or directories. |
| `exit` | Ctrl+D also exits. |

Arguments may be quoted with `'single'` or `"double"` quotes to include
spaces, e.g. `mkdir "my dir"` (no escape-sequence support like `\"`).

Wildcards `*` (any run of characters) and `?` (exactly one character) are
expanded against real filenames before a command runs, e.g. `rm *.log` or
`cp draft?.md backups/`. A pattern that matches nothing is left as a
literal argument, same as a shell with `nullglob` off. Dotfiles are
excluded from `*`/`?` unless the pattern itself starts with `.`.

Commands can be chained with `|` and the final command's output can be
redirected with `>` (truncate) or `>>` (append), e.g. `ls | cat` or
`echo hi > out.txt`. There's no input redirection (`<`), and since this
shell only has the built-ins above (no external command execution),
piping only does something useful when the downstream command reads
piped input — currently just `cat` with no file arguments.

Anything not in the table above prints `command not found`.

## Code layout

`src/main.rs` holds the REPL loop, pipeline execution, and command
dispatch. The supporting logic is split into modules:

| Module | Contents |
| --- | --- |
| `parser.rs` | `tokenize` (quoting-aware line splitting), `parse_flags`, `parse_pipeline` (`\|`/`>`/`>>` parsing) |
| `glob.rs` | `*`/`?` wildcard matching and expansion |
| `ls.rs` | The `ls` implementation: formatting, classify chars, block counting |
| `fileops.rs` | `rm`/`cp`/`mv`'s underlying `remove_item`/`copy_file`/`move_item` |
| `users.rs` | uid/gid-to-name lookups via raw `libc` calls |

## Building and running

```sh
cargo build --release
./target/release/zero_shell
```

or just `cargo run` for a debug build.

## Testing

```sh
cargo test
```

Each module carries unit tests for its own functions: tokenizing/flag
parsing/pipeline parsing, glob matching, `ls` formatting (permission
bits, classify characters, recursive listing) and `rm`/`cp`/`mv`
(recursive removal, copying/moving into a directory vs. an exact path,
refusing to copy a directory), all exercised against real filesystem
entries rather than mocks.

CI runs `cargo fmt --check`, `cargo clippy -D warnings`, `cargo build`,
and `cargo test` on every push and pull request to `master`, and
Dependabot checks weekly for Cargo and GitHub Actions dependency
updates.
