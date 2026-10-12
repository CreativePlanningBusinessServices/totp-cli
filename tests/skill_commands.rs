use totp_cli::commands::skill::{self, EMBEDDED_SKILL};

#[test]
fn install_writes_embedded_skill_to_dir_override() {
    let dir = tempfile::tempdir().unwrap();
    let result = skill::install(Some(dir.path())).unwrap();
    assert_eq!(result["installed"], true);
    let written = std::fs::read_to_string(dir.path().join("SKILL.md")).unwrap();
    assert_eq!(written, EMBEDDED_SKILL);
    assert_eq!(
        result["path"],
        dir.path().join("SKILL.md").display().to_string()
    );
}

#[test]
fn second_install_reports_already_current() {
    let dir = tempfile::tempdir().unwrap();
    skill::install(Some(dir.path())).unwrap();
    let again = skill::install(Some(dir.path())).unwrap();
    assert_eq!(again["installed"], false);
    assert_eq!(again["reason"], "already current");
}

#[test]
fn install_skips_a_symlinked_target_file() {
    let dir = tempfile::tempdir().unwrap();
    let real = dir.path().join("real-SKILL.md");
    std::fs::write(&real, "stale").unwrap();
    let link = dir.path().join("SKILL.md");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&real, &link).unwrap();
    #[cfg(windows)]
    if std::os::windows::fs::symlink_file(&real, &link).is_err() {
        return; // symlink creation needs Developer Mode on Windows runners; nothing to test
    }
    let result = skill::install(Some(dir.path())).unwrap();
    assert_eq!(result["installed"], false);
    assert!(result["reason"].as_str().unwrap().contains("symlink"));
    assert_eq!(std::fs::read_to_string(&real).unwrap(), "stale");
}

#[test]
fn cli_skill_install_reports_json_path() {
    let dir = tempfile::tempdir().unwrap();
    let output = assert_cmd::Command::cargo_bin("totp-cli")
        .unwrap()
        .args(["skill", "install", "--dir"])
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["skill"], "totp-cli");
    assert_eq!(json["installed"], true);
}
