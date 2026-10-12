use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::atomic_file;
use crate::error::CliError;
use crate::totp::Algorithm;

pub const INDEX_FILE: &str = "accounts.toml";
const CONFIG_DIR_ENV: &str = "TOTP_CLI_CONFIG_DIR";
const MAX_NAME_LENGTH: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issuer: Option<String>,
    pub digits: u32,
    pub period: u64,
    pub algorithm: Algorithm,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Index {
    #[serde(default)]
    pub accounts: BTreeMap<String, AccountParams>,
}

impl Index {
    pub fn load(config_dir: &Path) -> Result<Index, CliError> {
        let path = config_dir.join(INDEX_FILE);
        let contents = match std::fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(io_error) if io_error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Index::default());
            }
            Err(io_error) => {
                return Err(CliError::Io(format!(
                    "cannot read {}: {io_error}",
                    path.display()
                )));
            }
        };
        toml::from_str(&contents).map_err(|toml_error| {
            CliError::Io(format!("cannot parse {}: {toml_error}", path.display()))
        })
    }

    pub fn save(&self, config_dir: &Path) -> Result<(), CliError> {
        let contents = toml::to_string_pretty(self)
            .map_err(|toml_error| CliError::Io(format!("cannot serialise index: {toml_error}")))?;
        atomic_file::write_private(&config_dir.join(INDEX_FILE), &contents).map_err(CliError::Io)
    }
}

impl AccountParams {
    pub fn validate(&self) -> Result<(), CliError> {
        if !(6..=8).contains(&self.digits) {
            return Err(CliError::Usage(format!(
                "digits must be 6, 7 or 8 (got {})",
                self.digits
            )));
        }
        if self.period == 0 {
            return Err(CliError::Usage("period must be at least 1 second".into()));
        }
        Ok(())
    }
}

pub fn config_dir() -> Result<PathBuf, CliError> {
    if let Some(explicit) = std::env::var_os(CONFIG_DIR_ENV).filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(explicit));
    }
    directories::ProjectDirs::from("com", "CreativePlanning", "totp-cli")
        .map(|dirs| dirs.config_dir().to_path_buf())
        .ok_or_else(|| {
            CliError::Io("cannot determine a config directory; set TOTP_CLI_CONFIG_DIR".into())
        })
}

pub fn validate_name(name: &str) -> Result<(), CliError> {
    let allowed =
        |character: char| character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-');
    if name.is_empty() || name.len() > MAX_NAME_LENGTH || !name.chars().all(allowed) {
        return Err(CliError::Usage(format!(
            "invalid account name '{name}': use 1-{MAX_NAME_LENGTH} characters from A-Z, a-z, 0-9, '.', '_' and '-'"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_params() -> AccountParams {
        AccountParams {
            issuer: Some("NetSuite".into()),
            digits: 6,
            period: 30,
            algorithm: Algorithm::Sha1,
        }
    }

    #[test]
    fn load_of_a_missing_file_is_an_empty_index() {
        let dir = tempfile::tempdir().unwrap();
        let index = Index::load(dir.path()).unwrap();
        assert!(index.accounts.is_empty());
    }

    #[test]
    fn save_then_load_round_trips_and_sorts_names() {
        let dir = tempfile::tempdir().unwrap();
        let mut index = Index::default();
        index.accounts.insert("zeta".into(), sample_params());
        index.accounts.insert(
            "alpha".into(),
            AccountParams {
                issuer: None,
                ..sample_params()
            },
        );
        index.save(dir.path()).unwrap();

        let written = std::fs::read_to_string(dir.path().join(INDEX_FILE)).unwrap();
        assert!(written.contains("[accounts.alpha]"));
        assert!(!written.contains("issuer = \"\""));

        let loaded = Index::load(dir.path()).unwrap();
        assert_eq!(
            loaded.accounts.keys().collect::<Vec<_>>(),
            vec!["alpha", "zeta"]
        );
        assert_eq!(loaded.accounts["zeta"], sample_params());
    }

    #[test]
    fn save_creates_the_config_dir_and_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("deeper").join("still");
        Index::default().save(&nested).unwrap();
        let names: Vec<String> = std::fs::read_dir(&nested)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec![INDEX_FILE.to_string()]);
    }

    #[cfg(unix)]
    #[test]
    fn saved_index_is_owner_readable_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        Index::default().save(dir.path()).unwrap();
        let mode = std::fs::metadata(dir.path().join(INDEX_FILE))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn corrupt_index_is_an_io_error_naming_the_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(INDEX_FILE), "this is = not [toml").unwrap();
        let error = Index::load(dir.path()).unwrap_err();
        assert!(matches!(error, CliError::Io(_)));
        assert!(error.to_string().contains(INDEX_FILE));
    }

    #[test]
    fn config_dir_honours_the_env_override() {
        temp_env_var("TOTP_CLI_CONFIG_DIR", "/tmp/totp-cli-test", || {
            assert_eq!(config_dir().unwrap(), PathBuf::from("/tmp/totp-cli-test"));
        });
    }

    #[test]
    fn names_are_restricted_to_safe_characters() {
        assert!(validate_name("netsuite").is_ok());
        assert!(validate_name("ns.prod_2-b").is_ok());
        assert!(matches!(validate_name(""), Err(CliError::Usage(_))));
        assert!(matches!(
            validate_name("has space"),
            Err(CliError::Usage(_))
        ));
        assert!(matches!(validate_name("../etc"), Err(CliError::Usage(_))));
        assert!(matches!(
            validate_name(&"x".repeat(65)),
            Err(CliError::Usage(_))
        ));
    }

    #[test]
    fn params_enforce_digit_and_period_ranges() {
        assert!(sample_params().validate().is_ok());
        assert!(matches!(
            AccountParams {
                digits: 5,
                ..sample_params()
            }
            .validate(),
            Err(CliError::Usage(_))
        ));
        assert!(matches!(
            AccountParams {
                digits: 9,
                ..sample_params()
            }
            .validate(),
            Err(CliError::Usage(_))
        ));
        assert!(matches!(
            AccountParams {
                period: 0,
                ..sample_params()
            }
            .validate(),
            Err(CliError::Usage(_))
        ));
    }

    /// Env vars are process-global; set, run, restore so parallel tests are not disturbed.
    fn temp_env_var(key: &str, value: &str, body: impl FnOnce()) {
        let previous = std::env::var_os(key);
        // SAFETY: only this test touches the environment, and it restores it before returning.
        unsafe { std::env::set_var(key, value) };
        body();
        match previous {
            Some(old) => unsafe { std::env::set_var(key, old) },
            None => unsafe { std::env::remove_var(key) },
        }
    }
}
