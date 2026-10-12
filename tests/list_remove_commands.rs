mod common;

use common::TestEnv;
use predicates::prelude::*;

#[test]
fn remove_deletes_the_account_and_its_secret() {
    let env = TestEnv::new();
    env.add("a");
    env.add("b");
    env.cmd()
        .args(["remove", "a"])
        .assert()
        .success()
        .stdout("removed a\n");
    env.cmd().arg("list").assert().success().stdout("b\n");
    env.cmd().args(["code", "a"]).assert().code(2);
    let secrets = std::fs::read_to_string(env.config_dir().join("secrets.toml")).unwrap();
    assert!(!secrets.contains("\"a\"") && !secrets.starts_with("a = "));
    assert!(secrets.contains("b = "));
}

#[test]
fn remove_json_shape() {
    let env = TestEnv::new();
    env.add("a");
    env.cmd()
        .args(["--json", "remove", "a"])
        .assert()
        .success()
        .stdout("{\"name\":\"a\",\"removed\":true}\n");
}

#[test]
fn remove_unknown_account_is_exit_2() {
    let env = TestEnv::new();
    env.cmd()
        .args(["remove", "ghost"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("no account named 'ghost'"));
}

#[test]
fn remove_succeeds_when_the_secret_is_already_gone() {
    let env = TestEnv::new();
    env.add("a");
    std::fs::remove_file(env.config_dir().join("secrets.toml")).unwrap();
    env.cmd().args(["remove", "a"]).assert().success();
    env.cmd().arg("list").assert().success().stdout("");
}
