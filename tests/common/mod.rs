use std::path::Path;

use assert_cmd::Command;

/// RFC 4226 test key "12345678901234567890" as base32.
pub const RFC_SECRET: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";

pub struct TestEnv {
    pub dir: tempfile::TempDir,
}

impl TestEnv {
    pub fn new() -> TestEnv {
        TestEnv {
            dir: tempfile::tempdir().expect("temp dir"),
        }
    }

    pub fn config_dir(&self) -> &Path {
        self.dir.path()
    }

    /// A `totp-cli` invocation pointed at this env's temp config dir and the file-backed
    /// secret store, so no test ever touches a real keychain.
    pub fn cmd(&self) -> Command {
        let mut command = Command::cargo_bin("totp-cli").expect("binary built");
        command
            .env("TOTP_CLI_CONFIG_DIR", self.config_dir())
            .env("TOTP_CLI_SECRET_STORE", "file")
            .env_remove("CLAUDE_CONFIG_DIR");
        command
    }

    pub fn add(&self, name: &str) {
        self.cmd()
            .args(["add", name, "--secret", RFC_SECRET])
            .assert()
            .success();
    }
}
