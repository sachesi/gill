# Changelog

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions
follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Pressing Enter on a line fish cannot run, such as a bash loop, a `FOO=bar` assignment,
  `a[0]=1` or `source venv/bin/activate`, runs it in bash and brings exported variables
  and the working directory back into fish.
- Ctrl-Alt-B sends the current line to bash whatever it contains.
- Bash variables, functions, aliases and options last from one bash line to the next.
- Fish can call bash functions and aliases by name, in pipelines and scripts too.
- `unset`, `declare`, `typeset`, `shopt`, `local`, `readonly` and `let` work at the fish
  prompt.
- `gill init fish --bashrc` imports the functions and aliases of `~/.bashrc`, and
  `GILL_IMPORT_VARS` names the variables fish takes from bash.
