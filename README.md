# gill

gill lets you type bash at a fish prompt. When you press Enter on a line that fish
cannot run, such as a `for ...; do ...; done` loop, a `FOO=bar` assignment or a
`source venv/bin/activate`, gill runs it in bash instead and brings the exported
variables and the working directory back into fish. Bash functions, aliases, plain
variables and shell options carry over from one bash line to the next. Fish can call
bash functions and aliases too, so a function from `source ./tools.sh` can be typed at
the prompt.

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
- it assigns to an array element or appends to a variable, as `a[0]=1` and `x+=1` do,
  which fish reads as commands it cannot find;
- it starts with `source` or `.` and a file that does not end in `.fish`.

A line fish calls an error while bash still needs more of it, such as `f() {` or
`cat <<EOF`, gets another line from Enter, and goes to bash once bash can parse it.

Everything else runs in fish unchanged. Ctrl-Alt-B sends the current line to bash
whatever it contains. A line that fails at runtime in fish is never run a second time in
bash.

fish has no `unset`, `declare`, `typeset`, `shopt`, `local`, `readonly` or `let`. gill
defines them as functions that run the bash builtin and keep its result.

After each bash line, every bash function and alias whose name fish does not already
know gets a fish function of the same name. It runs that one command in bash with the
arguments fish expanded, so it works in fish pipelines and scripts and returns the bash
command's status. A name that is already a fish function, builtin or command is left to
fish.

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
background jobs, traps and coprocesses end with the line that started them. A bash line
cannot call fish functions. Variables flow back to fish only if they are exported.

## License

gill is free software under the GNU General Public License, version 3 or later. See
[LICENSE](LICENSE).
