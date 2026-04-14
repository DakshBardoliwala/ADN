#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde_json::Value;
use tempfile::TempDir;

pub struct TestWorkspace {
    tempdir: TempDir,
}

impl TestWorkspace {
    pub fn new() -> Self {
        Self {
            tempdir: tempfile::tempdir().expect("temporary workdir should be created"),
        }
    }

    pub fn path(&self) -> &Path {
        self.tempdir.path()
    }

    pub fn fixture_root(&self, name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(name)
    }

    pub fn index_fixture(&self, name: &str) -> Output {
        Command::new(adn_binary())
            .args([
                "index",
                self.fixture_root(name)
                    .to_str()
                    .expect("fixture path should be valid utf-8"),
            ])
            .current_dir(self.path())
            .output()
            .expect("index command should run")
    }

    pub fn index_fixture_ok(&self, name: &str) {
        let output = self.index_fixture(name);
        assert_command_success("index command failed", &output);
    }

    pub fn run_cli(&self, args: &[&str]) -> Output {
        Command::new(adn_binary())
            .args(args)
            .current_dir(self.path())
            .output()
            .expect("cli command should run")
    }

    pub fn run_mcp_session(&self, messages: &[Value]) -> SessionOutput {
        let mut child = Command::new(adn_binary())
            .args(["mcp", "serve"])
            .current_dir(self.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("mcp server should start");

        {
            let stdin = child.stdin.as_mut().expect("stdin should be piped");
            for message in messages {
                serde_json::to_writer(&mut *stdin, message).expect("request should serialize");
                use std::io::Write;
                stdin.write_all(b"\n").expect("newline should be written");
                stdin.flush().expect("stdin should flush");
            }
        }

        let output = child
            .wait_with_output()
            .expect("mcp server should exit cleanly after stdin closes");

        assert_command_success("mcp server failed", &output);

        let stdout_text = String::from_utf8(output.stdout).expect("stdout should be utf-8");
        let responses = stdout_text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str::<Value>(line).expect("response should be valid json"))
            .collect::<Vec<_>>();

        SessionOutput {
            responses,
            stderr: String::from_utf8(output.stderr).expect("stderr should be utf-8"),
        }
    }
}

pub struct SessionOutput {
    pub responses: Vec<Value>,
    pub stderr: String,
}

pub fn assert_command_success(context: &str, output: &Output) {
    assert!(
        output.status.success(),
        "{context}\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn adn_binary() -> &'static str {
    env!("CARGO_BIN_EXE_adn")
}
