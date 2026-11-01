//! The fish integration as `gill init fish` prints it, loaded into a fish without
//! configuration.

use std::fs::{self, Permissions};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

const GILL: &str = env!("CARGO_BIN_EXE_gill");

/// Runs fish code after loading the integration, with `args` as `$argv`.
fn fish(runtime: &TempDir, code: &str, args: &[&str]) -> Output {
    let bin_dir = Path::new(GILL)
        .parent()
        .expect("the gill binary is in a directory");
    let path = std::env::join_paths(std::iter::once(bin_dir.to_path_buf()).chain(
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
    ))
    .expect("PATH entries contain no colons");
    Command::new("fish")
        .args(["--no-config", "-c"])
        .arg(format!("gill init fish | source; {code}"))
        .args(args)
        .env("PATH", path)
        .env("XDG_RUNTIME_DIR", runtime.path())
        .output()
        .expect("fish is installed")
}

fn runtime() -> TempDir {
    let dir = tempfile::tempdir().expect("a temporary directory can be created");
    fs::set_permissions(dir.path(), Permissions::from_mode(0o700))
        .expect("the temporary directory is ours");
    dir
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("the output is UTF-8")
}

fn routes_to_bash(runtime: &TempDir, line: &str) -> bool {
    let output = fish(runtime, "__gill_verdict $argv[1]", &[line]);
    output.status.success()
}

#[test]
fn bash_only_lines_route_to_bash() {
    let runtime = runtime();
    for line in [
        "for i in 1 2; do echo $i; done",
        "if [ -f x ]; then echo y; fi",
        "while false; do :; done",
        "case $x in a) echo a;; esac",
        "f() { echo hi; }",
        "echo ${HOME:-none}",
        "echo $((1 + 2))",
        "FOO=bar",
        "x=$(date)",
        "arr=(a b c)",
        "echo a; FOO=bar",
        "[[ -f x ]] && echo y",
        "source venv/bin/activate",
        ". ./env.sh",
        "cat <<EOF\nbody\nEOF",
        "a[0]=1",
        "x+=1",
        "a[1]+=2",
        "declare -a m; m[0]=1",
        "echo a | b+=1",
        "echo a\nx+=1\necho b",
    ] {
        assert!(routes_to_bash(&runtime, line), "{line}");
    }
}

#[test]
fn lines_bash_needs_more_of_wait_for_the_next_line() {
    let runtime = runtime();
    for line in [
        "f() {",
        "case x in",
        "echo ${HOME",
        "cat <<EOF",
        "cat <<EOF\nbody",
    ] {
        let output = fish(&runtime, "__gill_verdict $argv[1]", &[line]);
        assert_eq!(output.status.code(), Some(2), "{line}");
    }
}

#[test]
fn fish_lines_stay_in_fish() {
    let runtime = runtime();
    for line in [
        "echo hi && echo ho",
        "FOO=bar echo x",
        "echo a=b",
        "set x 1",
        "echo $(date)",
        "ls | grep x",
        "begin",
        "switch $x",
        "for x in a b",
        "if test -f x",
        "echo a \\",
        "echo 'unterminated",
        "source conf.fish",
        "source",
        "source -",
        "echo a[1]=2",
        "echo x+=1",
        "echo 'q; x+=1'",
        "echo hi # ; x+=1",
    ] {
        assert!(!routes_to_bash(&runtime, line), "{line}");
    }
}

#[test]
fn exports_directory_and_unset_reach_fish() {
    let runtime = runtime();
    let target = tempfile::tempdir().unwrap();
    let target = target.path().canonicalize().unwrap();
    let output = fish(
        &runtime,
        "set -gx TARGET $argv[1]; set -gx GONE 1
         __gill_bash -c 'export SPACED=\"a b\" EXTRAPATH=/x:$PATH; cd \"$TARGET\"; unset GONE'
         printf '%s\\n' $SPACED (pwd) $EXTRAPATH[1]
         set -q GONE; or echo gone",
        &[target.to_str().unwrap()],
    );
    assert_eq!(
        stdout(&output),
        format!("a b\n{}\n/x\ngone\n", target.display())
    );
}

#[test]
fn bash_builtin_wrappers_keep_their_state() {
    let runtime = runtime();
    let output = fish(
        &runtime,
        "declare -A map; __gill_bash -c 'map[k]=v; echo \"${map[k]}\"'
         set -gx LEAVING 1; unset LEAVING; set -q LEAVING; or echo unset
         typeset gill_t=3; let 'gill_l = 1 + 1'; __gill_bash -c 'echo \"$gill_t$gill_l\"'",
        &[],
    );
    assert_eq!(stdout(&output), "v\nunset\n32\n");
}

#[test]
fn exit_status_reaches_fish() {
    let runtime = runtime();
    let output = fish(&runtime, "__gill_bash -c 'exit 4'; echo $status", &[]);
    assert_eq!(stdout(&output), "4\n");
}

#[test]
fn bash_functions_and_aliases_are_callable_from_fish() {
    let runtime = runtime();
    let output = fish(
        &runtime,
        "__gill_bash -c 'gill_greet() { echo \"hello $1\"; return 5; }; alias gill_shout=\"echo LOUD\"'
         gill_greet 'big world'; echo $status
         gill_shout quiet | string lower",
        &[],
    );
    assert_eq!(stdout(&output), "hello big world\n5\nloud quiet\n");
}

#[test]
fn bash_function_never_shadows_what_fish_knows() {
    let runtime = runtime();
    let output = fish(
        &runtime,
        "function gill_mine; echo fish; end
         __gill_bash -c 'gill_mine() { echo bash; }; printf() { echo bash; }'
         gill_mine; printf '%s\\n' fish",
        &[],
    );
    assert_eq!(stdout(&output), "fish\nfish\n");
}

#[test]
fn init_with_bashrc_imports_the_bashrc_functions_into_fish() {
    let runtime = runtime();
    let home = tempfile::tempdir().unwrap();
    fs::write(
        home.path().join(".bashrc"),
        "rc_fish_fn() { echo from-bashrc; }\n",
    )
    .unwrap();
    let bin_dir = Path::new(GILL)
        .parent()
        .expect("the gill binary is in a directory");
    let path = std::env::join_paths(std::iter::once(bin_dir.to_path_buf()).chain(
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
    ))
    .expect("PATH entries contain no colons");
    let output = Command::new("fish")
        .args(["--no-config", "-c"])
        .arg("gill init fish --bashrc | source; __gill_bash -c true; rc_fish_fn")
        .env("PATH", path)
        .env("XDG_RUNTIME_DIR", runtime.path())
        .env("HOME", home.path())
        .output()
        .expect("fish is installed");
    assert_eq!(stdout(&output), "from-bashrc\n");
}

#[test]
fn fish_function_that_replaced_a_bash_one_survives_its_removal() {
    let runtime = runtime();
    let output = fish(
        &runtime,
        "__gill_bash -c 'gill_taken() { echo bash; }'
         function gill_taken; echo fish; end
         __gill_bash -c 'unset -f gill_taken'; gill_taken",
        &[],
    );
    assert_eq!(stdout(&output), "fish\n");
}

#[test]
fn removed_bash_function_is_no_longer_callable() {
    let runtime = runtime();
    let output = fish(
        &runtime,
        "__gill_bash -c 'gill_gone() { :; }'; functions -q gill_gone; and echo defined
         __gill_bash -c 'unset -f gill_gone'; functions -q gill_gone; or echo removed",
        &[],
    );
    assert_eq!(stdout(&output), "defined\nremoved\n");
}
