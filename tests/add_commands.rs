mod common;

use common::{RFC_SECRET, TestEnv};
use predicates::prelude::*;

#[test]
fn add_then_list_shows_the_name_and_never_the_secret() {
    let env = TestEnv::new();
    env.cmd()
        .args([
            "add", "netsuite", "--secret", RFC_SECRET, "--issuer", "NetSuite",
        ])
        .assert()
        .success()
        .stdout("added netsuite\n");
    env.cmd()
        .arg("list")
        .assert()
        .success()
        .stdout("netsuite\n");

    let listing = env.cmd().args(["--json", "list"]).output().unwrap();
    let json: serde_json::Value = serde_json::from_slice(&listing.stdout).unwrap();
    assert_eq!(
        json,
        serde_json::json!([{"name": "netsuite", "issuer": "NetSuite", "digits": 6, "period": 30, "algorithm": "sha1"}])
    );
    assert!(!String::from_utf8_lossy(&listing.stdout).contains(RFC_SECRET));
}

#[test]
fn list_with_no_accounts_prints_nothing_or_an_empty_array() {
    let env = TestEnv::new();
    env.cmd().arg("list").assert().success().stdout("");
    env.cmd()
        .args(["--json", "list"])
        .assert()
        .success()
        .stdout("[]\n");
}

#[test]
fn add_json_reports_the_stored_parameters() {
    let env = TestEnv::new();
    env.cmd()
        .args([
            "--json",
            "add",
            "ns",
            "--secret",
            RFC_SECRET,
            "--digits",
            "8",
            "--period",
            "60",
            "--algorithm",
            "sha256",
        ])
        .assert()
        .success()
        .stdout("{\"algorithm\":\"sha256\",\"digits\":8,\"name\":\"ns\",\"period\":60}\n");
}

#[test]
fn add_from_uri_takes_parameters_from_the_uri_and_flags_override() {
    let env = TestEnv::new();
    let uri = format!(
        "otpauth://totp/NetSuite:jon?secret={RFC_SECRET}&digits=8&period=60&algorithm=SHA512"
    );
    env.cmd()
        .args(["--json", "add", "ns", "--uri", &uri, "--digits", "7"])
        .assert()
        .success()
        .stdout(
            "{\"algorithm\":\"sha512\",\"digits\":7,\"issuer\":\"NetSuite\",\"name\":\"ns\",\"period\":60}\n",
        );
}

#[test]
fn duplicate_add_needs_force() {
    let env = TestEnv::new();
    env.add("ns");
    env.cmd()
        .args(["add", "ns", "--secret", RFC_SECRET])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("already exists"))
        .stderr(predicate::str::contains("--force"));
    env.cmd()
        .args(["add", "ns", "--secret", RFC_SECRET, "--force"])
        .assert()
        .success();
}

#[test]
fn add_rejects_bad_names_secrets_parameters_and_conflicting_sources() {
    let env = TestEnv::new();
    env.cmd()
        .args(["add", "bad name", "--secret", RFC_SECRET])
        .assert()
        .code(2);
    env.cmd()
        .args(["add", "ns", "--secret", "NOT!BASE32"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("base32"));
    env.cmd()
        .args(["add", "ns", "--secret", RFC_SECRET, "--digits", "10"])
        .assert()
        .code(2);
    env.cmd()
        .args(["add", "ns", "--secret", RFC_SECRET, "--period", "0"])
        .assert()
        .code(2);
    env.cmd()
        .args([
            "add",
            "ns",
            "--secret",
            RFC_SECRET,
            "--uri",
            "otpauth://totp/x?secret=GEZDGNBVGY3TQOJQ",
        ])
        .assert()
        .code(2);
    env.cmd().arg("list").assert().success().stdout("");
}

#[test]
fn json_errors_are_structured_on_stderr() {
    let env = TestEnv::new();
    env.cmd()
        .args(["--json", "add", "bad name", "--secret", RFC_SECRET])
        .assert()
        .code(2)
        .stdout("")
        .stderr(predicate::str::starts_with(
            "{\"error\":\"invalid account name",
        ))
        .stderr(predicate::str::contains("\"exit_code\":2"));
}
