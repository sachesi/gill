//! gill inside a real interactive fish, in a private tmux server, for what only shows at
//! a prompt: key bindings, the line editor and history.

use std::fs::{self, Permissions};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use tempfile::TempDir;

const GILL: &str = env!("CARGO_BIN_EXE_gill");

struct Terminal {
    dir: TempDir,
}

impl Terminal {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("a temporary directory can be created");
        let runtime = dir.path().join("run");
        let home = dir.path().join("home");
        let work = dir.path().join("work");
        let config = home.join(".config/fish");
        fs::create_dir_all(&config).expect("the temporary directory is ours");
        fs::create_dir(&work).expect("the temporary directory is ours");
        fs::create_dir(&runtime).expect("the temporary directory is ours");
        fs::set_permissions(&runtime, Permissions::from_mode(0o700))
            .expect("the temporary directory is ours");
        fs::write(
            config.join("config.fish"),
            "function fish_prompt; echo -n '> '; end
             gill init fish | source
            ",
        )
        .expect("the temporary directory is ours");

        let bin_dir = Path::new(GILL)
            .parent()
            .expect("the gill binary is in a directory");
        let path = std::env::join_paths(std::iter::once(bin_dir.to_path_buf()).chain(
            std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
        ))
        .expect("PATH entries contain no colons");

        let terminal = Self { dir };
        let started = terminal
            .tmux()
            .args([
                "-f",
                "/dev/null",
                "new-session",
                "-d",
                "-x",
                "120",
                "-y",
                "40",
            ])
            .arg("-c")
            .arg(&work)
            .args(["env", "-i", "TERM=xterm-256color", "LANG=C.UTF-8"])
            .arg(env_pair("HOME", &home))
            .arg(env_pair("XDG_RUNTIME_DIR", &runtime))
            .arg(env_pair("PATH", Path::new(&path)))
            .arg("fish")
            .output()
            .expect("tmux is installed");
        assert!(started.status.success(), "{started:?}");
        terminal.wait_for("> ");
        terminal
    }

    fn tmux(&self) -> Command {
        let mut command = Command::new("tmux");
        command.arg("-S").arg(self.dir.path().join("tmux"));
        command
    }

    fn send(&self, args: &[&str]) {
        let sent = self
            .tmux()
            .arg("send-keys")
            .args(args)
            .output()
            .expect("tmux is installed");
        assert!(sent.status.success(), "{args:?}: {sent:?}");
    }

    fn type_text(&self, text: &str) {
        self.send(&["-l", "--", text]);
    }

    fn press(&self, key: &str) {
        self.send(&[key]);
    }

    fn enter_line(&self, line: &str) {
        self.type_text(line);
        self.press("Enter");
    }

    fn write_file(&self, name: &str, content: &str) {
        fs::write(self.dir.path().join("work").join(name), content)
            .expect("the temporary directory is ours");
    }

    fn screen(&self) -> String {
        let output = self
            .tmux()
            .args(["capture-pane", "-p", "-J", "-S", "-"])
            .output()
            .expect("tmux is installed");
        String::from_utf8_lossy(&output.stdout).into_owned()
    }

    /// Returns the screen once it shows `text`.
    fn wait_for(&self, text: &str) -> String {
        let start = Instant::now();
        loop {
            let screen = self.screen();
            if screen.contains(text) {
                return screen;
            }
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "waiting for {text:?} on:\n{screen}"
            );
            thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        // The server is already gone if fish has exited.
        let _ = self.tmux().arg("kill-server").output();
    }
}

fn env_pair(name: &str, value: &Path) -> String {
    format!("{name}={}", value.display())
}

#[test]
fn line_typed_while_a_command_runs_reaches_bash() {
    let terminal = Terminal::new();
    terminal.write_file("env.sh", "case x in x) echo bash-$((40 + 2));; esac\n");
    // Once the command has printed, it holds the terminal, and the line waits in the queue.
    terminal.enter_line("bash -c 'echo started-$((1 + 1)); sleep 0.5'");
    terminal.wait_for("started-2");
    terminal.enter_line("source ./env.sh");
    terminal.wait_for("bash-42");
}

#[test]
fn ctrl_j_routes_like_enter() {
    let terminal = Terminal::new();
    terminal.type_text("echo bash-$((40 + 2))");
    terminal.press("C-j");
    terminal.wait_for("bash-42");
}

#[test]
fn function_typed_over_several_lines_is_defined() {
    let terminal = Terminal::new();
    terminal.enter_line("gill_f() {");
    terminal.enter_line("echo called-$((40 + 2))");
    terminal.enter_line("}");
    terminal.enter_line("gill_f");
    let screen = terminal.wait_for("called-42");
    assert!(!screen.contains("syntax error"), "{screen}");
}

#[test]
fn here_document_body_stays_out_of_fish() {
    let terminal = Terminal::new();
    terminal.enter_line("cat <<EOF");
    terminal.enter_line("doc-body-$((40 + 2))");
    terminal.enter_line("EOF");
    let screen = terminal.wait_for("doc-body-42");
    assert!(!screen.contains("wanted `EOF'"), "{screen}");
}

#[test]
fn enter_in_vi_normal_mode_returns_to_insert_mode() {
    let terminal = Terminal::new();
    terminal.enter_line("fish_vi_key_bindings");
    terminal.wait_for("[I]");
    terminal.type_text("echo bash-$((40 + 2))");
    terminal.press("Escape");
    terminal.wait_for("[N]");
    terminal.press("Enter");
    terminal.wait_for("bash-42");
    terminal.enter_line("echo mode-(echo $fish_bind_mode)");
    terminal.wait_for("mode-insert");
}

#[test]
fn enter_keeps_the_mode_of_the_default_key_bindings() {
    let terminal = Terminal::new();
    terminal.enter_line("echo bash-$((40 + 2))");
    terminal.wait_for("bash-42");
    terminal.enter_line("echo mode-(echo $fish_bind_mode)");
    terminal.wait_for("mode-default");
}

#[test]
fn up_arrow_recalls_the_bash_line_as_typed() {
    let terminal = Terminal::new();
    terminal.enter_line("for gill_x in 1; do echo bash-$((40 + 2)); done");
    terminal.wait_for("bash-42");
    terminal.press("Up");
    terminal.wait_for("> for gill_x in 1");
}
