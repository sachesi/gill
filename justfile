# gill build and install tasks.
#
# `build` needs cargo; `install` only copies what is already in target/release.
#
#   just build
#   sudo just install              (prefix /usr/local)
#   just prefix=$HOME/.local build install

set shell := ["bash", "-euo", "pipefail", "-c"]

crate := "gill"
prefix := env("PREFIX", "/usr/local")
destdir := env("DESTDIR", "")
bindir := destdir + prefix + "/bin"
release := "target/release"

default:
    @just --list

build:
    cargo build --release --locked

# Run a debug build uninstalled.
run *args:
    cargo run -- {{args}}

# Formatting, clippy, and the syntax of the embedded bash and fish code.
check:
    cargo fmt --check
    cargo clippy --all-targets --locked -- -D warnings
    bash -n shell/prelude.bash
    fish --no-config --no-execute shell/gill.fish

test:
    cargo test --locked

# Advisories, licences and sources of the dependencies.
deny:
    cargo deny check

# Install the release build. Does not build: run `just build` first.
install:
    @test -x {{release}}/{{crate}} || { echo "error: {{release}}/{{crate}} missing; run 'just build' first" >&2; exit 1; }
    install -Dm755 {{release}}/{{crate}} {{bindir}}/{{crate}}

uninstall:
    rm -f {{bindir}}/{{crate}}
