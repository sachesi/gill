# gill

gill lets you type bash at a fish prompt. When you press Enter on a line that fish
cannot run, such as a `for ...; do ...; done` loop, a `FOO=bar` assignment or a
`source venv/bin/activate`, gill runs it in bash instead and brings the exported
variables and the working directory back into fish. Bash functions, aliases, plain
variables and shell options carry over from one bash line to the next, so the functions
`source ./tools.sh` defines are still there for the next line sent to bash.

Lines are never translated. Bash runs what you typed, so it behaves exactly as it would
in bash.

## How a line is routed

Fish decides first. A line goes to bash when:

- fish's parser reports a syntax error, as for `case`, `f() { }`, `${var:-default}` and
  `$((...))`;
- fish thinks the line is unfinished but `bash -n` accepts it as complete, as for bash
  loops and `if ...; then ...; fi`. A line starting with `begin` or `switch` stays in
  fish, as does one ending in a continuation backslash;
- fish would refuse the line before running any of it: a bare `NAME=value`, or a line
  starting with `[[`;
- it starts with `source` or `.` and a file that does not end in `.fish`.

Everything else runs in fish unchanged. Ctrl-Alt-B sends the current line to bash
whatever it contains. A line that fails at runtime in fish is never run a second time in
bash.

fish has no `unset`, `declare`, `shopt`, `local` or `readonly`. gill defines them as
functions that run the bash builtin and keep its result.

On screen, a bash line shows as `__gill_run_line # <your line>`. History stores the line
you typed.

## Requirements

fish 4.0 or newer and bash 5.0 or newer. gill keeps its state in `$XDG_RUNTIME_DIR`,
which must be set and private to you, as systemd sets it up for every login.

## Building and installing

    just build
    sudo just install        # or: just prefix=$HOME/.local build install

The build needs Rust 1.88 or newer and `just`. `just uninstall` removes the binary
again.

Then load gill from `~/.config/fish/config.fish`:

    if status is-interactive
        gill init fish | source
    end

## What does not carry over

Each bash line runs in a fresh bash that restores the state the previous line left, so
background jobs, traps and coprocesses end with the line that started them. Bash
functions and aliases stay on the bash side: fish cannot call them, and a bash line
cannot call fish functions. Variables flow back to fish only if they are exported.

If you define your own `fish_should_add_to_history`, it decides about the
`__gill_run_line` lines too.

## License

gill is free software under the GNU General Public License, version 3 or later. See
[LICENSE](LICENSE).
