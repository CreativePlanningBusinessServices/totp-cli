use std::path::Path;

use serde_json::{Value, json};

use crate::error::CliError;

pub const REPO_OWNER: &str = "CreativePlanningBusinessServices";
pub const REPO_NAME: &str = "totp-cli";

pub fn run(check_only: bool, skip_skill: bool) -> Result<Value, CliError> {
    if check_only {
        check_for_update()
    } else {
        install_update(skip_skill)
    }
}

fn check_for_update() -> Result<Value, CliError> {
    let current_version = env!("CARGO_PKG_VERSION");
    let latest_release = build_updater()?
        .get_latest_release()
        .map_err(|update_error| CliError::Io(update_error.to_string()))?;
    Ok(json!({
        "current": current_version,
        "latest": latest_release.version,
        "updateAvailable": update_available(current_version, &latest_release.version)?,
    }))
}

/// Release tags carry a leading `v`; CARGO_PKG_VERSION never does. Compare as semver.
fn update_available(current: &str, latest: &str) -> Result<bool, CliError> {
    let latest_stripped = latest.strip_prefix(['v', 'V']).unwrap_or(latest);
    self_update::version::bump_is_greater(current, latest_stripped).map_err(|version_error| {
        CliError::Io(format!(
            "cannot compare versions '{current}' and '{latest}': {version_error}"
        ))
    })
}

fn install_update(skip_skill: bool) -> Result<Value, CliError> {
    // Resolve before the swap: on some platforms current_exe() is unreliable afterwards.
    let exe_path = std::env::current_exe();
    let status = build_updater()?
        .update()
        .map_err(|update_error| CliError::Io(update_error.to_string()))?;
    let mut result = json!({"updated": status.updated(), "version": status.version()});
    if !skip_skill && status.updated() {
        result["skill"] = match exe_path {
            Ok(exe) => refresh_skill_via_new_binary(&exe),
            Err(error) => skill_refresh_error(&format!("cannot locate updated binary: {error}")),
        };
    }
    Ok(result)
}

/// The running process still holds the OLD embedded skill, so run the freshly installed
/// binary's `skill install` instead. Never fatal.
fn refresh_skill_via_new_binary(exe: &Path) -> Value {
    match std::process::Command::new(exe)
        .args(["skill", "install"])
        .output()
    {
        Ok(done) if done.status.success() => {
            if !done.stderr.is_empty() {
                eprint!("{}", String::from_utf8_lossy(&done.stderr));
            }
            serde_json::from_slice(&done.stdout)
                .map(drop_skill_name)
                .unwrap_or_else(|_| skill_refresh_error("skill install produced no JSON"))
        }
        Ok(done) => skill_refresh_error(&format!(
            "skill install exited {}: {}",
            done.status,
            String::from_utf8_lossy(&done.stderr).trim()
        )),
        Err(error) => skill_refresh_error(&format!("could not run skill install: {error}")),
    }
}

fn drop_skill_name(mut value: Value) -> Value {
    if let Some(map) = value.as_object_mut() {
        map.remove("skill");
    }
    value
}

fn skill_refresh_error(message: &str) -> Value {
    eprintln!("binary updated; skill refresh skipped — {message}");
    json!({"installed": false, "reason": message})
}

fn build_updater() -> Result<Box<dyn self_update::update::ReleaseUpdate>, CliError> {
    let mut updater = self_update::backends::github::Update::configure();
    updater
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .bin_name("totp-cli")
        .show_download_progress(false)
        .current_version(env!("CARGO_PKG_VERSION"));
    if let Ok(github_token) = std::env::var("GITHUB_TOKEN") {
        updater.auth_token(&github_token);
    }
    updater
        .build()
        .map_err(|update_error| CliError::Io(update_error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_available_treats_v_prefixed_equal_versions_as_no_update() {
        assert!(!update_available("0.1.0", "v0.1.0").unwrap());
    }

    #[test]
    fn update_available_detects_a_newer_release() {
        assert!(update_available("0.1.0", "0.2.0").unwrap());
        assert!(!update_available("0.2.0", "0.1.0").unwrap());
    }

    #[test]
    fn drop_skill_name_removes_the_redundant_self_identifier() {
        assert_eq!(
            drop_skill_name(json!({"skill": "totp-cli", "installed": true})),
            json!({"installed": true})
        );
    }
}
