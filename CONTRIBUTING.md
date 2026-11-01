# Contributing

A change passes `just check` and `just test` before it is committed; CI runs the same
two recipes. The tests need bash, fish and tmux installed. They run both shells for
real, and tmux gives `tests/terminal.rs` a terminal to type at an interactive fish in.

## Where things are

- `src/run.rs` starts bash for one command and turns what it left behind into records.
- `src/session.rs` owns the private directory under `$XDG_RUNTIME_DIR/gill/`.
- `src/dump.rs` reads what bash writes after the command: its directory, environment
  and the names of its functions and aliases. `src/record.rs` diffs that against fish's
  environment and encodes the changes fish applies.
- `shell/prelude.bash` is the bash side: restore the snapshot and define the save. The
  `RUN` line in `src/run.rs` evaluates it, runs the command, then saves.
- `shell/gill.fish` is what `gill init fish` prints: routing, the Enter binding, the
  function that applies the records, and the fish functions standing in for bash ones.

## Before changing the tricky parts

Data never becomes code. Values from bash reach fish as NUL-separated fields that
`__gill_apply` hands to `set` and `cd` as arguments, and nothing is ever escaped into
fish source. The only code generated from data is the bash snapshot, which bash writes
itself with `declare -p`, `declare -f` and `alias -p` and only bash reads. A change that
builds fish or bash code from a value with string formatting breaks this.

The prelude runs at top level on purpose: `declare` in the snapshot and in the
command must create globals, so neither the restore nor the command may move into a
function. It calls every builtin through `builtin`, because the snapshot it restores
may define functions with any name.

The order of the snapshot matters: shell options first, so functions that need
`extglob` parse; functions before aliases, because their bodies are stored with aliases
expanded; errexit, nounset, verbose and xtrace only right before the next command.
