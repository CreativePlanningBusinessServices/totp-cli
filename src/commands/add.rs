use std::path::Path;

use serde_json::json;

use crate::cli::AddArgs;
use crate::error::CliError;
use crate::index::{AccountParams, Index, validate_name};
use crate::otpauth;
use crate::output::Outcome;
use crate::secret::Secret;
use crate::store::SecretStore;
use crate::totp::Algorithm;

const DEFAULT_DIGITS: u32 = 6;
const DEFAULT_PERIOD: u64 = 30;

pub fn run(
    args: &AddArgs,
    config_dir: &Path,
    store: &dyn SecretStore,
) -> Result<Outcome, CliError> {
    validate_name(&args.name)?;
    let (secret, params) = resolve_secret_and_params(args)?;
    params.validate()?;

    let mut index = Index::load(config_dir)?;
    let already_exists = index.accounts.contains_key(&args.name);
    if already_exists && !args.force {
        return Err(CliError::Usage(format!(
            "account '{}' already exists; pass --force to replace it",
            args.name
        )));
    }
    let previous_secret = if already_exists {
        store.get(&args.name)?
    } else {
        None
    };

    store.set(&args.name, &secret)?;
    index.accounts.insert(args.name.clone(), params.clone());
    if let Err(save_error) = index.save(config_dir) {
        return Err(roll_back_store(
            store,
            &args.name,
            previous_secret,
            save_error,
        ));
    }

    Ok(Outcome {
        human: format!("added {}", args.name),
        json: account_json(&args.name, &params),
    })
}

/// The index save failed after the store was written. Put the store back the way it was
/// (previous secret restored, or the new entry removed) so the two never disagree.
fn roll_back_store(
    store: &dyn SecretStore,
    name: &str,
    previous_secret: Option<Secret>,
    save_error: CliError,
) -> CliError {
    let restored = match previous_secret {
        Some(old_secret) => store.set(name, &old_secret),
        None => store.delete(name),
    };
    match restored {
        Ok(()) => save_error,
        Err(rollback_error) => CliError::Io(format!(
            "{save_error}; additionally the credential store could not be rolled back ({rollback_error}), so '{name}' may now be out of step with the index"
        )),
    }
}

pub fn account_json(name: &str, params: &AccountParams) -> serde_json::Value {
    let mut value = json!({
        "name": name,
        "digits": params.digits,
        "period": params.period,
        "algorithm": params.algorithm,
    });
    if let Some(issuer) = &params.issuer {
        value["issuer"] = json!(issuer);
    }
    value
}

fn resolve_secret_and_params(args: &AddArgs) -> Result<(Secret, AccountParams), CliError> {
    let parsed_uri = args.uri.as_deref().map(otpauth::parse).transpose()?;
    let (secret, uri_issuer, uri_digits, uri_period, uri_algorithm) = match parsed_uri {
        Some(parsed) => (
            parsed.secret,
            parsed.issuer,
            parsed.digits,
            parsed.period,
            parsed.algorithm,
        ),
        None => (read_secret(args.secret.as_deref())?, None, None, None, None),
    };
    let params = AccountParams {
        issuer: args.issuer.clone().or(uri_issuer),
        digits: args.digits.or(uri_digits).unwrap_or(DEFAULT_DIGITS),
        period: args.period.or(uri_period).unwrap_or(DEFAULT_PERIOD),
        algorithm: args.algorithm.or(uri_algorithm).unwrap_or(Algorithm::Sha1),
    };
    Ok((secret, params))
}

fn read_secret(flag_value: Option<&str>) -> Result<Secret, CliError> {
    match flag_value {
        Some(value) => Secret::parse(value),
        None => {
            let typed = rpassword::prompt_password("Secret (base32): ").map_err(|io_error| {
                CliError::Usage(format!("cannot read secret from terminal: {io_error}"))
            })?;
            Secret::parse(&typed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::FileStore;

    fn args(name: &str, secret: &str, force: bool) -> AddArgs {
        AddArgs {
            name: name.into(),
            secret: Some(secret.into()),
            uri: None,
            digits: None,
            period: None,
            algorithm: None,
            issuer: None,
            force,
        }
    }

    /// With --force the new secret has already replaced the old one when the index save
    /// runs; a failed save must put the OLD secret back, not delete the only copy.
    #[cfg(unix)]
    #[test]
    fn failed_index_save_during_force_restores_the_previous_secret() {
        use std::os::unix::fs::PermissionsExt;
        let config = tempfile::tempdir().unwrap();
        let store_dir = tempfile::tempdir().unwrap();
        let store = FileStore::new(store_dir.path());
        run(
            &args("ns", "GEZDGNBVGY3TQOJQ", false),
            config.path(),
            &store,
        )
        .unwrap();

        std::fs::set_permissions(config.path(), std::fs::Permissions::from_mode(0o555)).unwrap();
        let result = run(
            &args("ns", "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ", true),
            config.path(),
            &store,
        );
        std::fs::set_permissions(config.path(), std::fs::Permissions::from_mode(0o755)).unwrap();

        assert!(matches!(result, Err(CliError::Io(_))));
        assert_eq!(
            store.get("ns").unwrap().unwrap().as_base32(),
            "GEZDGNBVGY3TQOJQ"
        );
    }
}
