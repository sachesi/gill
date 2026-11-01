use std::fs::{self, Permissions};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::Duration;

use tempfile::TempDir;

/// A private `XDG_RUNTIME_DIR` and a working directory for one fish session's commands.
struct Runtime {
    dir: TempDir,
    cwd: TempDir,
    home: TempDir,
}

impl Runtime {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("a temporary directory can be created");
        fs::set_permissions(dir.path(), Permissions::from_mode(0o700))
            .expect("the temporary directory is ours");
        Self {
            dir,
            cwd: tempfile::tempdir().expect("a temporary directory can be created"),
            home: tempfile::tempdir().expect("a temporary directory can be created"),
        }
    }

    fn write_bashrc(&self, content: &str) {
        fs::write(self.home.path().join(".bashrc"), content).expect("the home directory is ours");
    }

    fn gill(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_gill"));
        command
            .env("XDG_RUNTIME_DIR", self.dir.path())
            .env("PWD", self.cwd.path())
            .env("HOME", self.home.path())
            .current_dir(self.cwd.path());
        command
    }

    fn run(&self, line: &str) -> Output {
        self.gill()
            .args(["run", "--session", "1", "-c", line])
            .output()
            .expect("gill was built")
    }

    fn records_path(&self) -> PathBuf {
        self.dir.path().join("gill/1/records")
    }

    fn records(&self) -> Vec<(String, String, String)> {
        let bytes = fs::read(self.records_path()).expect("gill wrote records");
        let fields: Vec<String> = bytes
            .split(|&b| b == 0)
            .map(|field| String::from_utf8(field.to_vec()).expect("records are UTF-8"))
            .collect();
        fields
            .chunks_exact(3)
            .map(|triple| (triple[0].clone(), triple[1].clone(), triple[2].clone()))
            .collect()
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("the output is UTF-8")
}

fn record(op: &str, name: &str, value: &str) -> (String, String, String) {
    (op.into(), name.into(), value.into())
}

fn no_commands() -> (String, String, String) {
    record("commands", "", "")
}

#[test]
fn functions_variables_aliases_and_options_survive_between_commands() {
    let runtime = Runtime::new();
    let setup =
        runtime.run("greet() { echo \"hi $1\"; }; kept=1; alias hey='echo hey'; shopt -s globstar");
    assert!(setup.status.success(), "{setup:?}");

    let output =
        runtime.run("greet you; echo \"kept=$kept\"; hey; shopt -q globstar && echo globstar");
    assert_eq!(stdout(&output), "hi you\nkept=1\nhey\nglobstar\n");
}

#[test]
fn exported_value_comes_back_byte_for_byte() {
    let runtime = Runtime::new();
    runtime.run(r#"export TRICKY=$'a b\nc\\'"'"'$(x)'"#);
    assert_eq!(
        runtime.records(),
        [record("set", "TRICKY", "a b\nc\\'$(x)"), no_commands()]
    );
}

#[test]
fn unset_exported_variable_comes_back_as_erase() {
    let runtime = Runtime::new();
    let output = runtime
        .gill()
        .env("GILL_TEST_VAR", "1")
        .args(["run", "--session", "1", "--", "unset", "GILL_TEST_VAR"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        runtime.records(),
        [record("erase", "GILL_TEST_VAR", ""), no_commands()]
    );
}

#[test]
fn directory_change_comes_back_as_cd() {
    let runtime = Runtime::new();
    let target = tempfile::tempdir().unwrap();
    runtime.run(&format!("cd '{}'", target.path().display()));
    assert_eq!(
        runtime.records(),
        [
            record("cd", "", &target.path().display().to_string()),
            no_commands()
        ]
    );
}

#[test]
fn assignment_to_pwd_does_not_move_fish() {
    let runtime = Runtime::new();
    runtime.run("PWD=/; export PWD; true");
    assert_eq!(runtime.records(), [no_commands()]);
}

#[test]
fn errors_count_lines_from_the_command() {
    let runtime = Runtime::new();
    let output = runtime.run("true\ngill_no_such_command");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("line 2: gill_no_such_command"), "{stderr}");
}

#[test]
fn command_without_changes_only_lists_commands() {
    let runtime = Runtime::new();
    runtime.run("true");
    assert_eq!(runtime.records(), [no_commands()]);
}

#[test]
fn value_exported_from_fish_wins_over_the_snapshot() {
    let runtime = Runtime::new();
    runtime.run("SHADOW=old");
    let output = runtime
        .gill()
        .env("SHADOW", "new")
        .args(["run", "--session", "1", "-c", "echo \"$SHADOW\""])
        .output()
        .unwrap();
    assert_eq!(stdout(&output), "new\n");
}

#[test]
fn exit_status_is_the_commands_and_exit_still_saves_state() {
    let runtime = Runtime::new();
    let output = runtime.run("saved=1; exit 3");
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(stdout(&runtime.run("echo \"$saved\"")), "1\n");
}

#[test]
fn command_that_replaces_the_exit_trap_still_saves_state() {
    let runtime = Runtime::new();
    runtime.run("trap 'echo bye' EXIT; trapped=2");
    assert_eq!(stdout(&runtime.run("echo \"$trapped\"")), "2\n");
}

#[test]
fn exec_keeps_the_state_from_before() {
    let runtime = Runtime::new();
    runtime.run("before=1");
    let output = runtime.run("before=2; exec true");
    assert!(output.status.success(), "{output:?}");
    assert!(!runtime.records_path().exists());
    assert_eq!(stdout(&runtime.run("echo \"$before\"")), "1\n");
}

#[test]
fn errexit_carries_over_to_the_next_command() {
    let runtime = Runtime::new();
    runtime.run("set -e");
    let output = runtime.run("false; echo not reached");
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout(&output), "");
}

#[test]
fn argv_is_run_as_words_without_reparsing() {
    let runtime = Runtime::new();
    let output = runtime
        .gill()
        .args([
            "run",
            "--session",
            "1",
            "--",
            "printf",
            "%s|",
            "a b",
            "$HOME",
            "*",
        ])
        .output()
        .unwrap();
    assert_eq!(stdout(&output), "a b|$HOME|*|");
}

#[test]
fn argv_declare_persists() {
    let runtime = Runtime::new();
    runtime
        .gill()
        .args(["run", "--session", "1", "--", "declare", "-A", "map"])
        .status()
        .unwrap();
    assert_eq!(stdout(&runtime.run("map[k]=v; echo \"${map[k]}\"")), "v\n");
}

#[test]
fn interrupted_command_still_saves_state() {
    let runtime = Runtime::new();
    let started = runtime.dir.path().join("started");
    let mut child = runtime
        .gill()
        .args(["run", "--session", "1", "-c"])
        .arg(format!(
            "interrupted=1; : > '{}'; sleep 30",
            started.display()
        ))
        .process_group(0)
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    while !started.exists() {
        thread::sleep(Duration::from_millis(10));
    }
    // As Ctrl-C does: to the whole process group, through bash's builtin kill.
    let kill = format!("kill -INT -- -{}", child.id());
    assert!(
        Command::new("bash")
            .args(["-c", &kill])
            .status()
            .unwrap()
            .success()
    );

    assert_eq!(child.wait().unwrap().code(), Some(130));
    assert_eq!(stdout(&runtime.run("echo \"$interrupted\"")), "1\n");
}

#[test]
fn session_id_that_could_name_another_path_is_refused() {
    let runtime = Runtime::new();
    let output = runtime
        .gill()
        .args(["run", "--session", "../1", "-c", "true"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(125));
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid session id"));
}

#[test]
fn runtime_dir_others_can_enter_is_refused() {
    let runtime = Runtime::new();
    fs::set_permissions(runtime.dir.path(), Permissions::from_mode(0o755)).unwrap();
    let output = runtime.run("true");
    assert_eq!(output.status.code(), Some(125));
    assert!(!runtime.dir.path().join("gill").exists());
}

#[test]
fn end_removes_the_session() {
    let runtime = Runtime::new();
    runtime.run("x=1");
    let status = runtime
        .gill()
        .args(["end", "--session", "1"])
        .status()
        .unwrap();
    assert!(status.success());
    assert!(!runtime.dir.path().join("gill/1").exists());
}

#[test]
fn alias_expands_in_argv() {
    let runtime = Runtime::new();
    runtime.run("alias gill_hi='echo hi from'");
    let output = runtime
        .gill()
        .args(["run", "--session", "1", "--", "gill_hi", "a b"])
        .output()
        .unwrap();
    assert_eq!(stdout(&output), "hi from a b\n");
}

#[test]
fn bash_functions_and_aliases_are_listed_for_fish() {
    let runtime = Runtime::new();
    runtime.run("gill_f() { :; }; alias gill_a=:");
    assert_eq!(
        runtime.records(),
        [record("commands", "", "gill_a\ngill_f")]
    );
}

fn run_with_bashrc(runtime: &Runtime, line: &str) -> Output {
    runtime
        .gill()
        .args(["run", "--bashrc", "--session", "1", "-c", line])
        .output()
        .expect("gill was built")
}

#[test]
fn bashrc_functions_and_aliases_are_available_when_imported() {
    let runtime = Runtime::new();
    runtime.write_bashrc("rc_greet() { echo from-rc; }\nalias rc_alias='echo aliased'\n");
    let output = run_with_bashrc(&runtime, "rc_greet; rc_alias");
    assert_eq!(stdout(&output), "from-rc\naliased\n");
}

#[test]
fn bashrc_is_sourced_again_only_when_its_content_changes() {
    let runtime = Runtime::new();
    let log = runtime.cwd.path().join("log");
    runtime.write_bashrc(&format!("echo sourced >> {}\n", log.display()));
    run_with_bashrc(&runtime, "true");
    run_with_bashrc(&runtime, "true");
    assert_eq!(fs::read_to_string(&log).unwrap(), "sourced\n");

    runtime.write_bashrc(&format!("echo sourced again >> {}\n", log.display()));
    run_with_bashrc(&runtime, "true");
    assert_eq!(
        fs::read_to_string(&log).unwrap(),
        "sourced\nsourced again\n"
    );
}

#[test]
fn bashrc_exports_stay_out_of_fish_and_bash_env() {
    let runtime = Runtime::new();
    runtime.write_bashrc("export RC_EXPORT=1 PATH=/rc:$PATH\n");
    let output = run_with_bashrc(
        &runtime,
        "echo ${RC_EXPORT:-unset}; env | grep -c '^RC_EXPORT='; case $PATH in /rc:*) echo rc;; *) echo kept;; esac",
    );
    assert_eq!(stdout(&output), "1\n0\nkept\n");
    assert!(
        !runtime
            .records()
            .iter()
            .any(|(op, name, _)| op == "set" && (name == "RC_EXPORT" || name == "PATH"))
    );
}

#[test]
fn named_variable_from_bashrc_reaches_fish() {
    let runtime = Runtime::new();
    runtime.write_bashrc("IMPORTED=yes\nHIDDEN=no\n");
    let output = runtime
        .gill()
        .env("GILL_BASHRC", "1")
        .env("GILL_IMPORT_VARS", "IMPORTED")
        .args(["run", "--session", "1", "-c", "true"])
        .output()
        .expect("gill was built");
    assert!(output.status.success(), "{output:?}");
    let records = runtime.records();
    assert!(records.contains(&record("set", "IMPORTED", "yes")));
    assert!(!records.iter().any(|(_, name, _)| name == "HIDDEN"));
}

#[test]
fn import_name_fish_manages_is_refused() {
    let runtime = Runtime::new();
    let output = runtime
        .gill()
        .env("GILL_IMPORT_VARS", "PWD")
        .args(["run", "--session", "1", "-c", "true"])
        .output()
        .expect("gill was built");
    assert_eq!(output.status.code(), Some(125));
    assert!(String::from_utf8_lossy(&output.stderr).contains("PWD"));
}
