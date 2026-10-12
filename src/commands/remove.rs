use std::path::Path;

use serde_json::json;

use crate::error::CliError;
use crate::index::Index;
use crate::output::Outcome;
use crate::store::SecretStore;

pub fn run(name: &str, config_dir: &Path, store: &dyn SecretStore) -> Result<Outcome, CliError> {
    let mut index = Index::load(config_dir)?;
    if !index.accounts.contains_key(name) {
        return Err(CliError::Usage(format!(
            "no account named '{name}'; run `totp-cli list`"
        )));
    }
    store.delete(name)?;
    index.accounts.remove(name);
    index.save(config_dir)?;
    Ok(Outcome {
        human: format!("removed {name}"),
        json: json!({"name": name, "removed": true}),
    })
}
