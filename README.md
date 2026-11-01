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

## Packages

Fedora 44, 45 and Rawhide, from the Copr project
[sachesi/software](https://copr.fedorainfracloud.org/coprs/sachesi/software/):

```
sudo dnf copr enable sachesi/software
sudo dnf install gill
```

openSUSE Tumbleweed and Slowroll, from the OBS project
[home:sachesi:software](https://build.opensuse.org/project/show/home:sachesi:software); for
Slowroll the address has `openSUSE_Slowroll` in it, and on aarch64 `openSUSE_Factory_ARM`:

```
sudo zypper addrepo https://download.opensuse.org/repositories/home:sachesi:software/openSUSE_Tumbleweed/home:sachesi:software.repo
sudo zypper install gill
```

Debian testing and Ubuntu 26.04, from the same OBS project; for Ubuntu the addresses
have `xUbuntu_26.04` in place of `Debian_Testing`:

```
sudo install -d /etc/apt/keyrings
curl -fsSL https://download.opensuse.org/repositories/home:sachesi:software/Debian_Testing/Release.key | sudo gpg --dearmor -o /etc/apt/keyrings/sachesi-software.gpg
echo 'deb [signed-by=/etc/apt/keyrings/sachesi-software.gpg] https://download.opensuse.org/repositories/home:sachesi:software/Debian_Testing/ /' | sudo tee /etc/apt/sources.list.d/sachesi-software.list
sudo apt update
sudo apt install gill
```

Arch Linux: the AUR package `gill`, built from
[packaging/aur/PKGBUILD](packaging/aur/PKGBUILD), which each release tag updates.

The same packages are attached to each [release](https://github.com/sachesi/gill/releases).
A package only installs the program: gill stays off until you load it, as below.

## Building and installing

    just build
    sudo just install        # or: just prefix=$HOME/.local build install

The build needs Rust 1.88 or newer and `just`. `just uninstall` removes the binary
again.

Then load gill from `~/.config/fish/config.fish`:

    if status is-interactive
        gill init fish | source
    end

## Loading ~/.bashrc

gill leaves `~/.bashrc` alone by default. With `gill init fish --bashrc`, or
`GILL_BASHRC=1` in the environment, it sources the file before the next bash line, once
per fish session and again when the content changes. Its functions, aliases and options
become available to bash lines and to fish. Its exports stay out of fish, so your fish
config keeps control of `PATH`. `GILL_IMPORT_VARS`, a space-separated list, names the
variables fish takes from bash all the same.

The file runs in a non-interactive bash. One that returns early there, for example with
`[[ $- == *i* ]] || return` at the top, defines nothing, and gill prints a warning when
bash ends up with no functions or aliases.

## What does not carry over

Each bash line runs in a fresh bash that restores the state the previous line left, so
traps and coprocesses end with the line that started them. A background job keeps
running, but the next bash line cannot `wait` for it or bring it back with `fg`. A bash
line cannot call fish functions. Variables flow back to fish only if they are exported.

## License

gill is free software under the GNU General Public License, version 3 or later. See
[LICENSE](LICENSE).
