use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::error::CliError;
use crate::secret::Secret;

pub const SECRETS_FILE: &str = "secrets.toml";
const STORE_ENV: &str = "TOTP_CLI_SECRET_STORE";
const KEYRING_SERVICE: &str = "totp-cli";

pub trait SecretStore {
    fn get(&self, name: &str) -> Result<Option<Secret>, CliError>;
    fn set(&self, name: &str, secret: &Secret) -> Result<(), CliError>;
    /// Deleting an entry that does not exist is not an error.
    fn delete(&self, name: &str) -> Result<(), CliError>;
}

pub fn open_store(config_dir: &Path) -> Result<Box<dyn SecretStore>, CliError> {
    let env_value = std::env::var(STORE_ENV).ok();
    match StoreKind::from_env_value(env_value.as_deref())? {
        StoreKind::File => Ok(Box::new(FileStore::new(config_dir))),
        StoreKind::Keyring => {
            KeyringStore::open().map(|store| Box::new(store) as Box<dyn SecretStore>)
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum StoreKind {
    Keyring,
    File,
}

impl StoreKind {
    pub fn from_env_value(value: Option<&str>) -> Result<StoreKind, CliError> {
        match value.map(str::trim) {
            None | Some("") | Some("keyring") => Ok(StoreKind::Keyring),
            Some("file") => Ok(StoreKind::File),
            Some(other) => Err(CliError::Usage(format!(
                "unknown {STORE_ENV} value '{other}'; use 'keyring' or 'file'"
            ))),
        }
    }
}

// ---------- OS credential store (macOS Keychain, Windows Credential Manager) ----------

pub struct KeyringStore;

impl KeyringStore {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    pub fn open() -> Result<KeyringStore, CliError> {
        Ok(KeyringStore)
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    pub fn open() -> Result<KeyringStore, CliError> {
        Err(CliError::Store(format!(
            "no OS credential store backend on this platform; set {STORE_ENV}=file"
        )))
    }

    fn entry(name: &str) -> Result<keyring::Entry, CliError> {
        keyring::Entry::new(KEYRING_SERVICE, name).map_err(|keyring_error| {
            CliError::Store(format!("credential store unavailable: {keyring_error}"))
        })
    }
}

impl SecretStore for KeyringStore {
    fn get(&self, name: &str) -> Result<Option<Secret>, CliError> {
        match Self::entry(name)?.get_password() {
            Ok(stored) => Secret::from_stored(&stored).map(Some),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(keyring_error) => Err(CliError::Store(format!(
                "credential store read failed: {keyring_error}"
            ))),
        }
    }

    fn set(&self, name: &str, secret: &Secret) -> Result<(), CliError> {
        Self::entry(name)?
            .set_password(secret.as_base32())
            .map_err(|keyring_error| {
                CliError::Store(format!("credential store write failed: {keyring_error}"))
            })
    }

    fn delete(&self, name: &str) -> Result<(), CliError> {
        match Self::entry(name)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(keyring_error) => Err(CliError::Store(format!(
                "credential store delete failed: {keyring_error}"
            ))),
        }
    }
}

// ---------- plain-file store (tests, headless Linux) ----------

pub struct FileStore {
    path: PathBuf,
}

impl FileStore {
    pub fn new(config_dir: &Path) -> FileStore {
        FileStore {
            path: config_dir.join(SECRETS_FILE),
        }
    }

    fn read_all(&self) -> Result<BTreeMap<String, String>, CliError> {
        match std::fs::read_to_string(&self.path) {
            Ok(contents) => toml::from_str(&contents).map_err(|toml_error| {
                CliError::Store(format!(
                    "cannot parse {}: {toml_error}",
                    self.path.display()
                ))
            }),
            Err(io_error) if io_error.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
            Err(io_error) => Err(CliError::Store(format!(
                "cannot read {}: {io_error}",
                self.path.display()
            ))),
        }
    }

    fn write_all(&self, entries: &BTreeMap<String, String>) -> Result<(), CliError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|io_error| {
                CliError::Store(format!("cannot create {}: {io_error}", parent.display()))
            })?;
        }
        let contents = toml::to_string(entries).map_err(|toml_error| {
            CliError::Store(format!("cannot serialise secrets: {toml_error}"))
        })?;
        std::fs::write(&self.path, contents).map_err(|io_error| {
            CliError::Store(format!("cannot write {}: {io_error}", self.path.display()))
        })?;
        restrict_permissions(&self.path)
    }
}

impl SecretStore for FileStore {
    fn get(&self, name: &str) -> Result<Option<Secret>, CliError> {
        self.read_all()?
            .get(name)
            .map(|stored| Secret::from_stored(stored))
            .transpose()
    }

    fn set(&self, name: &str, secret: &Secret) -> Result<(), CliError> {
        let mut entries = self.read_all()?;
        entries.insert(name.to_string(), secret.as_base32().to_string());
        self.write_all(&entries)
    }

    fn delete(&self, name: &str) -> Result<(), CliError> {
        let mut entries = self.read_all()?;
        if entries.remove(name).is_some() {
            self.write_all(&entries)?;
        }
        Ok(())
    }
}

#[cfg(unix)]
fn restrict_permissions(path: &Path) -> Result<(), CliError> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).map_err(|io_error| {
        CliError::Store(format!(
            "cannot set permissions on {}: {io_error}",
            path.display()
        ))
    })
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &Path) -> Result<(), CliError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secret(base32: &str) -> Secret {
        Secret::parse(base32).unwrap()
    }

    #[test]
    fn file_store_round_trips_a_secret() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileStore::new(dir.path());
        assert!(store.get("netsuite").unwrap().is_none());
        store.set("netsuite", &secret("GEZDGNBVGY3TQOJQ")).unwrap();
        assert_eq!(
            store.get("netsuite").unwrap().unwrap().as_base32(),
            "GEZDGNBVGY3TQOJQ"
        );
    }

    #[test]
    fn file_store_set_replaces_and_delete_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileStore::new(dir.path());
        store.set("a", &secret("GEZDGNBVGY3TQOJQ")).unwrap();
        store
            .set("a", &secret("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ"))
            .unwrap();
        assert_eq!(store.get("a").unwrap().unwrap().as_base32().len(), 32);
        store.delete("a").unwrap();
        store.delete("a").unwrap();
        assert!(store.get("a").unwrap().is_none());
    }

    #[test]
    fn file_store_keeps_other_entries_when_one_is_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileStore::new(dir.path());
        store.set("a", &secret("GEZDGNBVGY3TQOJQ")).unwrap();
        store.set("b", &secret("GEZDGNBVGY3TQOJQ")).unwrap();
        store.delete("a").unwrap();
        assert!(store.get("b").unwrap().is_some());
    }

    #[cfg(unix)]
    #[test]
    fn file_store_file_is_owner_readable_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        FileStore::new(dir.path())
            .set("a", &secret("GEZDGNBVGY3TQOJQ"))
            .unwrap();
        let mode = std::fs::metadata(dir.path().join(SECRETS_FILE))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn store_kind_parses_env_values() {
        assert!(matches!(
            StoreKind::from_env_value(None),
            Ok(StoreKind::Keyring)
        ));
        assert!(matches!(
            StoreKind::from_env_value(Some("file")),
            Ok(StoreKind::File)
        ));
        assert!(matches!(
            StoreKind::from_env_value(Some("keyring")),
            Ok(StoreKind::Keyring)
        ));
        assert!(matches!(
            StoreKind::from_env_value(Some("vault")),
            Err(CliError::Usage(_))
        ));
    }
}
