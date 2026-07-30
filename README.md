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
| `cd [dir]` | No argument goes to `$HOME`. |
| `pwd` | |
| `echo [args...]` | |
| `cat file...` | |
| `ls [-l] [-a] [-F] [path...]` | `-l` long format, `-a` show dotfiles, `-F` classify (`/` dir, `*` executable, `@` symlink, `\|` FIFO, `=` socket). Defaults to `.` when no path is given; multiple directory arguments get `path:` headers. |
| `mkdir dir...` | |
| `rm [-r] file...` | `-r` required to remove directories. |
| `cp src dst` | Single file only; errs if `src` is a directory. |
| `mv src dst` | Files or directories. |
| `exit` | Ctrl+D also exits. |

Arguments may be quoted with `'single'` or `"double"` quotes to include
spaces, e.g. `mkdir "my dir"`. There's no escape-sequence support (`\"`)
and no piping, redirection, globbing, or external command execution —
anything not in the table above prints `command not found`.

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

Unit tests cover the argument tokenizer/flag parser and the `ls`
formatting logic (permission bits, classify characters, entry
formatting) against real filesystem entries.

CI runs `cargo build` and `cargo test` on every push and pull request to
`master`, and Dependabot checks weekly for Cargo and GitHub Actions
dependency updates.
