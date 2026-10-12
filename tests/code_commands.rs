mod common;

use common::TestEnv;
use predicates::prelude::*;

#[test]
fn code_prints_the_rfc_vector_for_a_fixed_time() {
    let env = TestEnv::new();
    env.add("ns");
    env.cmd()
        .args(["code", "ns", "--at", "59"])
        .assert()
        .success()
        .stdout("287082\n");
    env.cmd()
        .args(["code", "ns", "--at", "1111111109"])
        .assert()
        .success()
        .stdout("081804\n");
}

#[test]
fn code_json_includes_expiry() {
    let env = TestEnv::new();
    env.add("ns");
    env.cmd()
        .args(["--json", "code", "ns", "--at", "59"])
        .assert()
        .success()
        .stdout("{\"code\":\"287082\",\"expires_in\":1,\"name\":\"ns\",\"period\":30}\n");
}

#[test]
fn code_honours_eight_digits_and_sha256() {
    let env = TestEnv::new();
    // RFC 6238 SHA-256 key "12345678901234567890123456789012" (32 bytes) as base32.
    let sha256_key = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZA";
    env.cmd()
        .args([
            "add",
            "big",
            "--secret",
            sha256_key,
            "--digits",
            "8",
            "--algorithm",
            "sha256",
        ])
        .assert()
        .success();
    env.cmd()
        .args(["code", "big", "--at", "59"])
        .assert()
        .success()
        .stdout("46119246\n");
}

#[test]
fn min_remaining_rolls_to_the_next_period_when_the_code_is_about_to_expire() {
    let env = TestEnv::new();
    env.add("ns");
    // At t=59 the code expires in 1s; with --min-remaining 5 we get the t=60 code (HOTP counter 2).
    env.cmd()
        .args(["--json", "code", "ns", "--at", "59", "--min-remaining", "5"])
        .assert()
        .success()
        .stdout("{\"code\":\"359152\",\"expires_in\":30,\"name\":\"ns\",\"period\":30}\n");
    // At t=60 the code has 30s left, so nothing changes.
    env.cmd()
        .args(["code", "ns", "--at", "60", "--min-remaining", "5"])
        .assert()
        .success()
        .stdout("359152\n");
}

#[test]
fn min_remaining_at_or_above_the_period_is_a_usage_error() {
    let env = TestEnv::new();
    env.add("ns");
    env.cmd()
        .args(["code", "ns", "--at", "59", "--min-remaining", "30"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("--min-remaining"));
}

#[test]
fn unknown_account_is_exit_2() {
    let env = TestEnv::new();
    env.cmd()
        .args(["code", "nope"])
        .assert()
        .code(2)
        .stdout("")
        .stderr(predicate::str::contains("no account named 'nope'"));
}

#[test]
fn indexed_account_with_missing_secret_is_exit_3() {
    let env = TestEnv::new();
    env.add("ns");
    std::fs::remove_file(env.config_dir().join("secrets.toml")).unwrap();
    env.cmd()
        .args(["code", "ns"])
        .assert()
        .code(3)
        .stdout("")
        .stderr(predicate::str::contains("add ns --force"));
}

#[test]
fn code_without_at_uses_the_clock_and_prints_six_digits() {
    let env = TestEnv::new();
    env.add("ns");
    env.cmd()
        .args(["code", "ns"])
        .assert()
        .success()
        .stdout(predicate::str::is_match(r"^\d{6}\n$").unwrap());
}
