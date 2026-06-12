//! Integration tests for `flute completion <shell>`.
//!
//! These tests are network-free and credential-free — shell completions are
//! generated entirely from the Clap definition at compile time.

/// `flute completion bash` exits 0 and emits a script that references "flute".
#[test]
fn completion_bash_exits_zero_and_contains_binary_name() {
    let output = assert_cmd::Command::cargo_bin("flute")
        .unwrap()
        .args(["completion", "bash"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "`flute completion bash` must exit 0, got: {:?}",
        output.status
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("flute"),
        "bash completion script must reference 'flute': got {stdout:?}"
    );
}

/// `flute completion zsh` exits 0 and emits a script that references "flute".
#[test]
fn completion_zsh_exits_zero_and_contains_binary_name() {
    let output = assert_cmd::Command::cargo_bin("flute")
        .unwrap()
        .args(["completion", "zsh"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "`flute completion zsh` must exit 0, got: {:?}",
        output.status
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("flute"),
        "zsh completion script must reference 'flute': got {stdout:?}"
    );
}

/// `flute completion fish` exits 0 and emits a script that references "flute".
#[test]
fn completion_fish_exits_zero_and_contains_binary_name() {
    let output = assert_cmd::Command::cargo_bin("flute")
        .unwrap()
        .args(["completion", "fish"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "`flute completion fish` must exit 0, got: {:?}",
        output.status
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("flute"),
        "fish completion script must reference 'flute': got {stdout:?}"
    );
}

/// `flute update --help` exits 0, confirming the subcommand is wired.
#[test]
fn update_help_exits_zero() {
    assert_cmd::Command::cargo_bin("flute")
        .unwrap()
        .args(["update", "--help"])
        .assert()
        .success();
}

/// `flute --help` shows both `completion` and `update` in the subcommand list.
#[test]
fn root_help_lists_completion_and_update() {
    let output = assert_cmd::Command::cargo_bin("flute")
        .unwrap()
        .arg("--help")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("completion"),
        "`flute --help` must list `completion`: {stdout:?}"
    );
    assert!(
        stdout.contains("update"),
        "`flute --help` must list `update`: {stdout:?}"
    );
}
