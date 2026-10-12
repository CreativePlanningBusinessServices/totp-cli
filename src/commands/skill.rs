use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::error::CliError;

pub const EMBEDDED_SKILL: &str = include_str!("../../skills/totp-cli/SKILL.md");

const SKILL_NAME: &str = "totp-cli";
const SKILL_FILE: &str = "SKILL.md";

#[derive(Debug)]
pub enum SkillTarget {
    Write(PathBuf),
    Skip(&'static str),
}

pub fn install(dir_override: Option<&Path>) -> Result<Value, CliError> {
    let config_base = config_base_dir();
    let base_exists = config_base.as_deref().is_some_and(Path::exists);
    let path = match resolve_target(dir_override, config_base.as_deref(), base_exists) {
        SkillTarget::Skip(reason) => {
            eprintln!(
                "skill not installed ({reason}); place it yourself with `totp-cli skill install --dir <your skills dir>`"
            );
            return Ok(skip_json(reason));
        }
        SkillTarget::Write(path) => path,
    };

    if is_symlink(&path) || path.parent().is_some_and(is_symlink) {
        return Ok(skip_json("symlink — skill tracks its source repo via git"));
    }
    if std::fs::read_to_string(&path).is_ok_and(|existing| existing == EMBEDDED_SKILL) {
        return Ok(installed_json(&path, false, Some("already current")));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|io_error| {
            CliError::Io(format!("cannot create {}: {io_error}", parent.display()))
        })?;
    }
    std::fs::write(&path, EMBEDDED_SKILL)
        .map_err(|io_error| CliError::Io(format!("cannot write {}: {io_error}", path.display())))?;
    Ok(installed_json(&path, true, None))
}

pub fn resolve_target(
    dir_override: Option<&Path>,
    config_base: Option<&Path>,
    base_exists: bool,
) -> SkillTarget {
    if let Some(dir) = dir_override {
        let path = if dir.file_name().is_some_and(|name| name == SKILL_FILE) {
            dir.to_path_buf()
        } else {
            dir.join(SKILL_FILE)
        };
        return SkillTarget::Write(path);
    }
    match config_base {
        Some(base) if base_exists => {
            SkillTarget::Write(base.join("skills").join(SKILL_NAME).join(SKILL_FILE))
        }
        _ => SkillTarget::Skip("no Claude config dir"),
    }
}

fn config_base_dir() -> Option<PathBuf> {
    if let Some(explicit) = std::env::var_os("CLAUDE_CONFIG_DIR").filter(|value| !value.is_empty())
    {
        return Some(PathBuf::from(explicit));
    }
    directories::UserDirs::new().map(|dirs| dirs.home_dir().join(".claude"))
}

fn is_symlink(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink())
}

fn skip_json(reason: &str) -> Value {
    json!({"skill": SKILL_NAME, "installed": false, "reason": reason})
}

fn installed_json(path: &Path, installed: bool, reason: Option<&str>) -> Value {
    let mut result =
        json!({"skill": SKILL_NAME, "path": path.display().to_string(), "installed": installed});
    if let Some(reason) = reason {
        result["reason"] = json!(reason);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_skill_carries_the_real_frontmatter() {
        let normalised: String = EMBEDDED_SKILL
            .chars()
            .filter(|character| *character != '\r')
            .collect();
        assert!(normalised.starts_with("---\nname: totp-cli"));
        assert!(EMBEDDED_SKILL.len() > 500);
    }

    #[test]
    fn resolve_target_builds_the_conventional_path_under_an_existing_base() {
        match resolve_target(None, Some(Path::new("/home/x/.claude")), true) {
            SkillTarget::Write(path) => assert_eq!(
                path,
                PathBuf::from("/home/x/.claude/skills/totp-cli/SKILL.md")
            ),
            other => panic!("expected Write, got {other:?}"),
        }
    }

    #[test]
    fn resolve_target_skips_when_base_missing() {
        assert!(matches!(
            resolve_target(None, Some(Path::new("/none")), false),
            SkillTarget::Skip("no Claude config dir")
        ));
    }
}
